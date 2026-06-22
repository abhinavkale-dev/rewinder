use super::*;
use super::profile::{evaluate_profile_guard_signals, push_guard_reason, select_primary_guard_reason_code};
impl Engine {
    pub(super) fn restart_reason_if_needed(&self) -> Option<CaptureRestartReason> {
        if self.is_capture_paused_by_user() {
            *self.no_segments_miss_count.lock() = 0;
            *self.resource_soft_pressure_since.lock() = None;
            *self.resource_hard_pressure_since.lock() = None;
            return None;
        }

        let should_run = {
            let state = self.state.lock();
            state.lifecycle_state == LifecycleState::Armed
                || state.lifecycle_state == LifecycleState::SavingReplay
        };

        if !should_run {
            *self.no_segments_miss_count.lock() = 0;
            *self.resource_soft_pressure_since.lock() = None;
            *self.resource_hard_pressure_since.lock() = None;
            return None;
        }

        let pipeline = self.pipeline.lock();
        let Some(handles) = pipeline.as_ref() else {
            *self.no_segments_miss_count.lock() = 0;
            return Some(CaptureRestartReason::MissingPipeline);
        };
        if handles
            .capture
            .has_display_changed(Duration::from_millis(DISPLAY_CHANGE_DEBOUNCE_MS))
        {
            return Some(CaptureRestartReason::DisplayChanged);
        }
        if let Some(error) = handles.capture.last_error() {
            *self.overload_since.lock() = None;
            *self.recover_since.lock() = None;
            *self.no_segments_miss_count.lock() = 0;
            if is_user_stopped_sharing_error(&error) {
                return Some(CaptureRestartReason::UserStoppedSharing);
            }
            if is_capture_start_interrupted_error(&error) {
                {
                    let mut state = self.state.lock();
                    state.capture_interrupt_count = state.capture_interrupt_count.saturating_add(1);
                }
                self.append_capture_runtime_marker("phase: startup_interrupted_sc3805");
                return Some(CaptureRestartReason::CaptureStartInterrupted);
            }
            return Some(CaptureRestartReason::CaptureProcessExited);
        }
        let now = Instant::now();
        let in_startup_window = self
            .last_pipeline_started_at
            .lock()
            .map(|started| {
                now.duration_since(started) < Duration::from_secs(STARTUP_PERF_GUARD_SECS)
            })
            .unwrap_or(false);
        let settings_snapshot = self.state.lock().settings.clone();
        let quality_policy = settings_snapshot.quality_policy.clone();
        let quality_preference = settings_snapshot.quality_preference.clone();
        let perf_guard_enabled = settings_snapshot.performance_guard_enabled;
        let current_profile_idx = *self.runtime_profile_index.lock();
        let current_effective_profile =
            effective_profile_for_index(&settings_snapshot, current_profile_idx);
        let playback_realtime_x = handles.capture.playback_realtime_x();
        let encoder_speed_x = handles.capture.capture_speed_x();
        let capture_dropped_frames = handles.capture.capture_dropped_frames();
        let capture_queue_overflows = handles.capture.capture_queue_overflows();
        let effective_output_fps = handles.capture.effective_output_fps();
        let system_memory_pressure_level = handles.capture.system_memory_pressure_level();
        let helper_thermal_state = handles.capture.helper_thermal_state();
        let (
            _app_rss_mb,
            _app_cpu_percent,
            capture_stack_rss_mb,
            capture_stack_cpu_percent,
            capture_stack_rss_delta_mb,
            sampled_thermal_state,
            power_source,
        ) = self.current_process_diagnostics();
        let thermal_state = helper_thermal_state.or(sampled_thermal_state);
        let on_battery = matches!(power_source.as_deref(), Some("battery"));
        let battery_floor = battery_floor_index(
            &settings_snapshot,
            on_battery,
            settings_snapshot.battery_guard_enabled,
            settings_snapshot.battery_max_fps,
        );
        self.state.lock().system_memory_pressure_level = system_memory_pressure_level.clone();
        let (drop_delta, overflow_delta) =
            self.update_overload_metric_deltas(capture_dropped_frames, capture_queue_overflows);
        let current_session_has_stable_segment =
            handles.capture.current_session_has_stable_segment();
        let non_critical_restart_suppressed_for_save = {
            let is_saving = self.state.lock().is_saving;
            let recent_save = self
                .last_save_started_at
                .lock()
                .map(|started| {
                    now.duration_since(started)
                        < Duration::from_millis(NON_CRITICAL_SAVE_RESTART_SUPPRESSION_MS)
                })
                .unwrap_or(false);
            is_saving || recent_save
        };
        let in_startup_profile_stabilization_freeze = current_session_has_stable_segment
            && self
                .last_pipeline_started_at
                .lock()
                .map(|started| {
                    now.duration_since(started)
                        < Duration::from_secs(STARTUP_PROFILE_STABILIZATION_FREEZE_SECS)
                })
                .unwrap_or(false);
        let mut keep_overload_timer = false;
        let mut keep_recover_timer = false;
        let speed_sample = playback_realtime_x.or(encoder_speed_x);
        let stack_soft_delta_budget_mb = capture_stack_rss_delta_soft_budget_mb(
            settings_snapshot.video_resolution,
            settings_snapshot.fps,
        );
        let stack_hard_delta_budget_mb = capture_stack_rss_delta_hard_budget_mb(
            settings_snapshot.video_resolution,
            settings_snapshot.fps,
        );
        let memory_soft_signal = capture_stack_rss_delta_mb
            .map(|delta| delta >= stack_soft_delta_budget_mb)
            .unwrap_or(false);
        let memory_hard_signal = capture_stack_rss_delta_mb
            .map(|delta| delta >= stack_hard_delta_budget_mb)
            .unwrap_or(false);
        let cpu_soft_signal = capture_stack_cpu_percent
            .map(|cpu| cpu >= CAPTURE_STACK_CPU_SOFT_THRESHOLD_PCT)
            .unwrap_or(false);
        let cpu_hard_signal = capture_stack_cpu_percent
            .map(|cpu| cpu >= CAPTURE_STACK_CPU_HARD_THRESHOLD_PCT)
            .unwrap_or(false);
        let system_memory_pressure_critical =
            matches!(system_memory_pressure_level.as_deref(), Some("critical"));
        let thermal_critical = matches!(thermal_state.as_deref(), Some("critical"));
        let resource_soft_signal = memory_soft_signal || cpu_soft_signal;
        let resource_hard_signal = memory_hard_signal || cpu_hard_signal;
        let resource_soft_triggered = if resource_soft_signal {
            let mut since = self.resource_soft_pressure_since.lock();
            let started = since.get_or_insert(now);
            now.saturating_duration_since(*started)
                >= Duration::from_secs(RESOURCE_SOFT_PRESSURE_HOLD_SECS)
        } else {
            *self.resource_soft_pressure_since.lock() = None;
            false
        };
        let mut resource_hard_triggered =
            if resource_hard_signal || system_memory_pressure_critical || thermal_critical {
                let mut since = self.resource_hard_pressure_since.lock();
                let started = since.get_or_insert(now);
                now.saturating_duration_since(*started)
                    >= Duration::from_secs(RESOURCE_HARD_PRESSURE_HOLD_SECS)
            } else {
                *self.resource_hard_pressure_since.lock() = None;
                false
            };
        if resource_soft_triggered {
            let mut soft_history = self.resource_soft_trigger_timestamps.lock();
            soft_history.retain(|at| {
                now.saturating_duration_since(*at)
                    <= Duration::from_secs(RESOURCE_SOFT_TRIGGER_WINDOW_SECS)
            });
            let should_record = soft_history
                .last()
                .map(|at| {
                    now.saturating_duration_since(*at)
                        >= Duration::from_secs(RESOURCE_SOFT_PRESSURE_HOLD_SECS)
                })
                .unwrap_or(true);
            if should_record {
                soft_history.push(now);
            }
            if soft_history.len() >= RESOURCE_HARD_TRIGGER_REPEAT_COUNT {
                resource_hard_triggered = true;
            }
        }
        if resource_soft_triggered {
            handles.capture.append_runtime_marker(&format!(
                "phase: perf_guard_memory_soft stack_rss_mb={} stack_cpu_percent={} stack_rss_delta_mb={} soft_budget_mb={}",
                capture_stack_rss_mb
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                capture_stack_cpu_percent
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "none".to_string()),
                capture_stack_rss_delta_mb
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                stack_soft_delta_budget_mb
            ));
        }
        if resource_hard_triggered {
            handles.capture.append_runtime_marker(&format!(
                "phase: perf_guard_memory_hard stack_rss_mb={} stack_cpu_percent={} stack_rss_delta_mb={} hard_budget_mb={}",
                capture_stack_rss_mb
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                capture_stack_cpu_percent
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "none".to_string()),
                capture_stack_rss_delta_mb
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                stack_hard_delta_budget_mb
            ));
        } else {
            *self.resource_hard_stepdown_pending.lock() = false;
        }
        let force_stepdown = std::path::Path::new("/tmp/rewinder-force-stepdown").exists();
        let resource_hard_signal = resource_hard_signal || force_stepdown;
        let resource_hard_triggered = resource_hard_triggered || force_stepdown;
        let guard_signals = evaluate_profile_guard_signals(
            current_effective_profile.fps,
            effective_output_fps,
            playback_realtime_x,
            encoder_speed_x,
            drop_delta,
            overflow_delta,
            resource_soft_signal,
            resource_hard_signal,
            system_memory_pressure_level.as_deref(),
            thermal_state.as_deref(),
            &quality_preference,
        );
        let mut contributing_reason_codes = Vec::new();
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.playback_realtime_low,
            "playback_realtime_low",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.capture_speed_low,
            "capture_speed_low",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.output_fps_under_target,
            "output_fps_low",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.drop_trigger,
            "frame_drop_spike",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.overflow_trigger,
            "queue_overflow_spike",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            cpu_soft_signal,
            "capture_stack_cpu_soft",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            cpu_hard_signal,
            "capture_stack_cpu_hard",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            memory_soft_signal,
            "capture_stack_rss_growth_soft",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            memory_hard_signal,
            "capture_stack_rss_growth_hard",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.system_memory_pressure_warning,
            "system_memory_pressure_warning",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.system_memory_pressure_critical,
            "system_memory_pressure_critical",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.thermal_serious,
            "thermal_serious",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            guard_signals.thermal_critical,
            "thermal_critical",
        );
        push_guard_reason(
            &mut contributing_reason_codes,
            on_battery && battery_floor > 0,
            "on_battery",
        );
        let primary_reason_code = select_primary_guard_reason_code(&contributing_reason_codes);
        let overload_signal = resource_hard_triggered
            || guard_signals.overload_signal_count >= guard_signals.overload_signal_requirement;
        if !in_startup_window && current_profile_idx == 0 {
            let mut startup_hold_logged = self.startup_requested_profile_hold_logged.lock();
            if !*startup_hold_logged {
                handles.capture.append_runtime_marker(&format!(
                    "phase: startup_requested_profile_held current_profile_idx={} effective_target_fps={} requested_fps={}",
                    current_profile_idx,
                    current_effective_profile.fps,
                    settings_snapshot.fps
                ));
                *startup_hold_logged = true;
            }
        }
        if speed_sample.is_some()
            || guard_signals.output_fps_under_target
            || guard_signals.drop_trigger
            || guard_signals.overflow_trigger
            || resource_soft_signal
            || resource_hard_signal
        {
            let profile_change_age = self
                .last_profile_change_at
                .lock()
                .map(|started| now.saturating_duration_since(started))
                .unwrap_or(Duration::from_secs(u64::MAX));
            let in_profile_cooldown =
                profile_change_age < Duration::from_secs(PROFILE_CHANGE_COOLDOWN_SECS);
            let in_profile_dwell =
                profile_change_age < Duration::from_secs(PROFILE_CHANGE_DWELL_SECS);

            if quality_policy == "adaptive_recover" && perf_guard_enabled {
                if in_startup_profile_stabilization_freeze {
                    *self.overload_since.lock() = None;
                    *self.recover_since.lock() = None;
                } else if overload_signal {
                    *self.recover_since.lock() = None;
                    let mut overload_since = self.overload_since.lock();
                    let started_at = overload_since.get_or_insert(now);
                    if self.can_step_down_profile() {
                        let startup_initial_guard_blocked = self
                            .should_suppress_initial_startup_fallback(
                                in_startup_window,
                                current_session_has_stable_segment,
