use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProfileGuardSignals {
    pub output_fps_under_target: bool,
    pub playback_realtime_low: bool,
    pub capture_speed_low: bool,
    pub speed_below_threshold: bool,
    pub speed_below_emergency: bool,
    pub speed_recovering: bool,
    pub drop_trigger: bool,
    pub overflow_trigger: bool,
    pub system_memory_pressure_warning: bool,
    pub system_memory_pressure_critical: bool,
    pub thermal_serious: bool,
    pub thermal_critical: bool,
    pub overload_signal_count: u8,
    pub overload_signal_requirement: u8,
    pub sustained_overload_signal_requirement: u8,
    pub recovery_signal: bool,
}

pub(super) fn evaluate_profile_guard_signals(
    current_profile_fps: u16,
    effective_output_fps: Option<f32>,
    playback_realtime_x: Option<f32>,
    capture_speed_x: Option<f32>,
    drop_delta: u64,
    overflow_delta: u64,
    resource_soft_signal: bool,
    resource_hard_signal: bool,
    system_memory_pressure_level: Option<&str>,
    thermal_state: Option<&str>,
    quality_preference: &str,
) -> ProfileGuardSignals {
    let output_fps_under_target = effective_output_fps
        .map(|fps| {
            fps < f32::from(current_profile_fps.max(1)) * OVERLOAD_OUTPUT_FPS_RATIO_THRESHOLD
        })
        .unwrap_or(false);
    let playback_realtime_low = playback_realtime_x
        .map(|speed| speed < PLAYBACK_OVERLOAD_THRESHOLD_X)
        .unwrap_or(false);
    let capture_speed_low = capture_speed_x
        .map(|speed| speed < PLAYBACK_OVERLOAD_THRESHOLD_X)
        .unwrap_or(false);
    let speed_below_threshold = playback_realtime_low || capture_speed_low;
    let speed_below_emergency = playback_realtime_x
        .map(|speed| speed < PLAYBACK_EMERGENCY_OVERLOAD_THRESHOLD_X)
        .unwrap_or(false)
        || capture_speed_x
            .map(|speed| speed < PLAYBACK_EMERGENCY_OVERLOAD_THRESHOLD_X)
            .unwrap_or(false);
    let speed_recovering = playback_realtime_x
        .map(|speed| speed >= PLAYBACK_RECOVER_THRESHOLD_X)
        .unwrap_or(true)
        && capture_speed_x
            .map(|speed| speed >= PLAYBACK_RECOVER_THRESHOLD_X)
            .unwrap_or(true);
    let drop_trigger = drop_delta >= OVERLOAD_DROP_DELTA_THRESHOLD;
    let overflow_trigger = overflow_delta >= 4;
    let system_memory_pressure_warning = matches!(system_memory_pressure_level, Some("warning"));
    let system_memory_pressure_critical = matches!(system_memory_pressure_level, Some("critical"));
    let thermal_serious = matches!(thermal_state, Some("serious"));
    let thermal_critical = matches!(thermal_state, Some("critical"));
    let overload_signal_count = u8::from(speed_below_threshold)
        + u8::from(output_fps_under_target)
        + u8::from(drop_trigger)
        + u8::from(overflow_trigger);
    let overload_signal_requirement = required_overload_signal_count(quality_preference);
    let sustained_overload_signal_requirement = overload_signal_requirement;
    let recovery_signal = speed_recovering
        && !output_fps_under_target
        && drop_delta < 4
        && overflow_delta < 2
        && !resource_soft_signal
        && !resource_hard_signal;
    let recovery_signal = recovery_signal
        && !system_memory_pressure_warning
        && !system_memory_pressure_critical
        && !thermal_serious
        && !thermal_critical;

    ProfileGuardSignals {
        output_fps_under_target,
        playback_realtime_low,
        capture_speed_low,
        speed_below_threshold,
        speed_below_emergency,
        speed_recovering,
        drop_trigger,
        overflow_trigger,
        system_memory_pressure_warning,
        system_memory_pressure_critical,
        thermal_serious,
        thermal_critical,
        overload_signal_count,
        overload_signal_requirement,
        sustained_overload_signal_requirement,
        recovery_signal,
    }
}

fn push_guard_reason(codes: &mut Vec<String>, condition: bool, code: &str) {
    if condition && !codes.iter().any(|existing| existing == code) {
        codes.push(code.to_string());
    }
}

fn select_primary_guard_reason_code(codes: &[String]) -> Option<String> {
    const ORDER: &[&str] = &[
        "thermal_critical",
        "system_memory_pressure_critical",
        "capture_stack_cpu_hard",
        "capture_stack_rss_growth_hard",
        "queue_overflow_spike",
        "frame_drop_spike",
        "playback_realtime_low",
        "capture_speed_low",
        "output_fps_low",
        "thermal_serious",
        "system_memory_pressure_warning",
        "capture_stack_cpu_soft",
        "capture_stack_rss_growth_soft",
    ];

    for code in ORDER {
        if codes.iter().any(|value| value == code) {
            return Some((*code).to_string());
        }
    }

    codes.first().cloned()
}

impl Engine {
    pub(super) fn take_pending_guard_transition(&self) -> Option<PerfGuardTransitionRecord> {
        self.pending_guard_transition.lock().take()
    }

    pub(super) fn update_guard_reason_context(
        &self,
        primary_reason_code: Option<String>,
        contributing_reason_codes: Vec<String>,
        suppressed_reason_code: Option<String>,
    ) {
        let mut state = self.state.lock();
        state.guard_primary_reason_code = primary_reason_code;
        state.guard_contributing_reason_codes = contributing_reason_codes;
        state.guard_suppressed_reason_code = suppressed_reason_code;
    }

    pub(super) fn record_guard_transition(
        &self,
        action: &str,
        guard_state: &str,
        hard: bool,
        primary_reason_code: Option<String>,
        contributing_reason_codes: Vec<String>,
        suppressed_reason_code: Option<String>,
        from_profile: Option<String>,
        to_profile: Option<String>,
    ) {
        let sampled_at_epoch_ms = to_epoch_ms_i64(SystemTime::now());
        {
            let mut state = self.state.lock();
            let is_duplicate = state.guard_state == guard_state
                && state.guard_primary_reason_code == primary_reason_code
                && state.guard_contributing_reason_codes == contributing_reason_codes
                && state.guard_suppressed_reason_code == suppressed_reason_code;
            if is_duplicate {
                return;
            }
            state.guard_state = guard_state.to_string();
            state.guard_primary_reason_code = primary_reason_code.clone();
            state.guard_contributing_reason_codes = contributing_reason_codes.clone();
            state.guard_suppressed_reason_code = suppressed_reason_code.clone();
            state.guard_last_transition_at_epoch_ms = Some(sampled_at_epoch_ms);
        }
        *self.pending_guard_transition.lock() = Some(PerfGuardTransitionRecord {
            action: action.to_string(),
            hard,
            primary_reason_code,
            contributing_reason_codes,
            suppressed_reason_code,
            from_profile,
            to_profile,
            sampled_at_epoch_ms,
        });
    }

    pub(super) fn should_suppress_initial_startup_fallback(
        &self,
        in_startup_window: bool,
        current_session_has_stable_segment: bool,
        resource_hard_triggered: bool,
    ) -> bool {
        let current_profile_idx = *self.runtime_profile_index.lock();
        in_startup_window
            && !current_session_has_stable_segment
            && current_profile_idx == 0
            && !resource_hard_triggered
    }

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
            // Prefer explicit user-intent semantics for Stop Sharing / SCK interruptions
            // to avoid restart loops that can keep macOS sharing UI active.
            if is_user_stopped_sharing_error(&error) {
                return Some(CaptureRestartReason::UserStoppedSharing);
            }
            // Startup-only interruption remains retryable when no user-stop signature is present.
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
        ) = self.current_process_diagnostics();
        let thermal_state = helper_thermal_state.or(sampled_thermal_state);
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
                                resource_hard_triggered,
                            );
                        let startup_stepdown_blocked =
                            in_startup_window && current_profile_idx >= 1;
                        if startup_initial_guard_blocked {
                            self.record_guard_transition(
                                "suppressed",
                                "suppressed",
                                false,
                                primary_reason_code.clone(),
                                contributing_reason_codes.clone(),
                                Some("startup_initial_guard".to_string()),
                                None,
                                None,
                            );
                            handles.capture.append_runtime_marker(&format!(
                                "phase: overload_transition_suppressed reason=startup_initial_guard primary_reason_code={} contributing_reason_codes={} signal_count={} required_signals={} current_profile_idx={} effective_target_fps={} requested_fps={} hard_triggered={} system_memory_pressure={} thermal_state={}",
                                primary_reason_code
                                    .as_deref()
                                    .unwrap_or("none"),
                                if contributing_reason_codes.is_empty() {
                                    "none".to_string()
                                } else {
                                    contributing_reason_codes.join(",")
                                },
                                guard_signals.overload_signal_count,
                                guard_signals.sustained_overload_signal_requirement,
                                current_profile_idx,
                                current_effective_profile.fps,
                                settings_snapshot.fps,
                                resource_hard_triggered,
                                system_memory_pressure_level
                                    .clone()
                                    .unwrap_or_else(|| "none".to_string()),
                                thermal_state.clone().unwrap_or_else(|| "none".to_string())
                            ));
                            *overload_since = None;
                        } else if startup_stepdown_blocked {
                            self.record_guard_transition(
                                "suppressed",
                                "suppressed",
                                false,
                                primary_reason_code.clone(),
                                contributing_reason_codes.clone(),
                                Some("startup_stepdown_guard".to_string()),
                                None,
                                None,
                            );
                            handles.capture.append_runtime_marker(&format!(
                                "phase: overload_transition_suppressed reason=startup_stepdown_guard primary_reason_code={} contributing_reason_codes={} signal_count={} required_signals={} current_profile_idx={} effective_target_fps={} requested_fps={} hard_triggered={} system_memory_pressure={} thermal_state={}",
                                primary_reason_code
                                    .as_deref()
                                    .unwrap_or("none"),
                                if contributing_reason_codes.is_empty() {
                                    "none".to_string()
                                } else {
                                    contributing_reason_codes.join(",")
                                },
                                guard_signals.overload_signal_count,
                                guard_signals.sustained_overload_signal_requirement,
                                current_profile_idx,
                                current_effective_profile.fps,
                                settings_snapshot.fps,
                                resource_hard_triggered,
                                system_memory_pressure_level
                                    .clone()
                                    .unwrap_or_else(|| "none".to_string()),
                                thermal_state.clone().unwrap_or_else(|| "none".to_string())
                            ));
                            *overload_since = None;
                        } else {
                            let emergency_trigger = guard_signals.speed_below_emergency
                                || guard_signals.overflow_trigger
                                || drop_delta >= OVERLOAD_DROP_EMERGENCY_DELTA_THRESHOLD
                                || resource_hard_triggered;
                            let sustained_overload = current_session_has_stable_segment
                                && guard_signals.overload_signal_count
                                    >= guard_signals.sustained_overload_signal_requirement
                                && now.duration_since(*started_at)
                                    >= Duration::from_secs(PLAYBACK_OVERLOAD_HOLD_SECS);
                            if emergency_trigger || sustained_overload {
                                let non_critical_transition_blocked = in_profile_dwell
                                    || in_profile_cooldown
                                    || non_critical_restart_suppressed_for_save;
                                let transition_blocked =
                                    non_critical_transition_blocked && !resource_hard_triggered;
                                if !transition_blocked {
                                    *self.resource_hard_stepdown_pending.lock() =
                                        resource_hard_triggered;
                                    let reason = if resource_hard_triggered {
                                        "resource_hard"
                                    } else if emergency_trigger {
                                        "emergency"
                                    } else {
                                        "metric"
                                    };
                                    let startup_guard = if in_startup_window
                                        && !current_session_has_stable_segment
                                        && current_profile_idx == 0
                                        && resource_hard_triggered
                                    {
                                        "allowed_hard_resource"
                                    } else {
                                        "not_applicable"
                                    };
                                    handles.capture.append_runtime_marker(&format!(
                                        "phase: overload_stepdown reason={} startup_guard={} primary_reason_code={} contributing_reason_codes={} signal_count={} required_signals={} current_profile_idx={} drop_delta={} overflow_delta={} output_fps={} effective_target_fps={} requested_fps={} stack_rss_mb={} stack_cpu_percent={} stack_rss_delta_mb={} system_memory_pressure={} thermal_state={}",
                                        reason,
                                        startup_guard,
                                        primary_reason_code
                                            .as_deref()
                                            .unwrap_or("none"),
                                        if contributing_reason_codes.is_empty() {
                                            "none".to_string()
                                        } else {
                                            contributing_reason_codes.join(",")
                                        },
                                        guard_signals.overload_signal_count,
                                        guard_signals.sustained_overload_signal_requirement,
                                        current_profile_idx,
                                        drop_delta,
                                        overflow_delta,
                                        effective_output_fps
                                            .map(|value| format!("{value:.2}"))
                                            .unwrap_or_else(|| "none".to_string()),
                                        current_effective_profile.fps,
                                        settings_snapshot.fps,
                                        capture_stack_rss_mb
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "none".to_string()),
                                        capture_stack_cpu_percent
                                            .map(|value| format!("{value:.1}"))
                                            .unwrap_or_else(|| "none".to_string()),
                                        capture_stack_rss_delta_mb
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "none".to_string()),
                                        system_memory_pressure_level
                                            .clone()
                                            .unwrap_or_else(|| "none".to_string()),
                                        thermal_state.clone().unwrap_or_else(|| "none".to_string())
                                    ));
                                    self.update_guard_reason_context(
                                        primary_reason_code.clone(),
                                        contributing_reason_codes.clone(),
                                        None,
                                    );
                                    return Some(CaptureRestartReason::Overloaded);
                                }
                                let suppression_reason = if in_profile_dwell {
                                    "profile_dwell"
                                } else if in_profile_cooldown {
                                    "profile_cooldown"
                                } else if non_critical_restart_suppressed_for_save {
                                    "recent_save_suppression"
                                } else {
                                    "unknown"
                                };
                                self.record_guard_transition(
                                    "suppressed",
                                    "suppressed",
                                    false,
                                    primary_reason_code.clone(),
                                    contributing_reason_codes.clone(),
                                    Some(suppression_reason.to_string()),
                                    None,
                                    None,
                                );
                                handles.capture.append_runtime_marker(&format!(
                                    "phase: overload_transition_suppressed reason={} primary_reason_code={} contributing_reason_codes={} signal_count={} required_signals={} hard_triggered={} system_memory_pressure={} thermal_state={}",
                                    suppression_reason,
                                    primary_reason_code
                                        .as_deref()
                                        .unwrap_or("none"),
                                    if contributing_reason_codes.is_empty() {
                                        "none".to_string()
                                    } else {
                                        contributing_reason_codes.join(",")
                                    },
                                    guard_signals.overload_signal_count,
                                    guard_signals.sustained_overload_signal_requirement,
                                    resource_hard_triggered,
                                    system_memory_pressure_level
                                        .clone()
                                        .unwrap_or_else(|| "none".to_string()),
                                    thermal_state.clone().unwrap_or_else(|| "none".to_string())
                                ));
                            }
                            keep_overload_timer = true;
                        }
                    } else {
                        *overload_since = None;
                    }
                } else {
                    *self.overload_since.lock() = None;
                    if guard_signals.recovery_signal {
                        let mut recover_since = self.recover_since.lock();
                        let started_at = recover_since.get_or_insert(now);
                        if self.can_step_up_profile() {
                            let sustained_recovery = now.duration_since(*started_at)
                                >= Duration::from_secs(PLAYBACK_RECOVER_HOLD_SECS);
                            let non_critical_transition_blocked = in_profile_dwell
                                || in_profile_cooldown
                                || non_critical_restart_suppressed_for_save;
                            if sustained_recovery && !non_critical_transition_blocked {
                                self.update_guard_reason_context(None, Vec::new(), None);
                                return Some(CaptureRestartReason::ProfileRecovered);
                            }
                            keep_recover_timer = true;
                        } else {
                            *recover_since = None;
                        }
                    } else {
                        *self.recover_since.lock() = None;
                    }
                }
            } else {
                *self.overload_since.lock() = None;
                *self.recover_since.lock() = None;
            }
        } else {
            *self.overload_since.lock() = None;
            *self.recover_since.lock() = None;
        }

        let segment_stall_threshold_ms = {
            let settings = self.state.lock().settings.clone();
            segment_stall_threshold_ms(settings.segment_time_ms)
        };
        let pipeline_grace = self
            .last_pipeline_started_at
            .lock()
            .map(|started| {
                now.duration_since(started) < Duration::from_millis(POST_PIPELINE_START_GRACE_MS)
            })
            .unwrap_or(false);
        let save_grace = self
            .last_save_started_at
            .lock()
            .map(|started| {
                now.duration_since(started) < Duration::from_millis(POST_SAVE_START_GRACE_MS)
            })
            .unwrap_or(false);
        if pipeline_grace || save_grace || !current_session_has_stable_segment {
            *self.no_segments_miss_count.lock() = 0;
        } else {
            let segment_age_ms = handles.capture.latest_segment_age_ms();
            let adjusted_segment_stall_threshold_ms = self
                .last_pipeline_started_at
                .lock()
                .map(|started| {
                    if now.duration_since(started)
                        < Duration::from_millis(POST_PIPELINE_START_GRACE_MS.saturating_mul(4))
                    {
                        segment_stall_threshold_ms.max(STARTUP_NO_SEGMENTS_EXTRA_THRESHOLD_MS)
                    } else {
                        segment_stall_threshold_ms
                    }
                })
                .unwrap_or(segment_stall_threshold_ms);
            let stalled = segment_age_ms
                .map(|age| age > adjusted_segment_stall_threshold_ms)
                .unwrap_or(true);
            if stalled {
                let miss_count = {
                    let mut misses = self.no_segments_miss_count.lock();
                    *misses = misses.saturating_add(1);
                    *misses
                };
                handles.capture.append_runtime_marker(&format!(
                    "phase: no_segments_miss age_ms={} threshold_ms={} miss_count={}",
                    segment_age_ms
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    adjusted_segment_stall_threshold_ms,
                    miss_count
                ));
                if miss_count >= NO_SEGMENTS_MISS_REQUIRED {
                    *self.overload_since.lock() = None;
                    *self.recover_since.lock() = None;
                    *self.no_segments_miss_count.lock() = 0;
                    handles.capture.append_runtime_marker(&format!(
                        "phase: no_segments_restart age_ms={} threshold_ms={} miss_count={}",
                        segment_age_ms
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "none".to_string()),
                        adjusted_segment_stall_threshold_ms,
                        miss_count
                    ));
                    return Some(CaptureRestartReason::NoSegments {
                        segment_age_ms,
                        threshold_ms: adjusted_segment_stall_threshold_ms,
                        miss_count,
                    });
                }
            } else {
                *self.no_segments_miss_count.lock() = 0;
            }
        }

        if !keep_overload_timer {
            *self.overload_since.lock() = None;
        }
        if !keep_recover_timer {
            *self.recover_since.lock() = None;
        }
        None
    }

    pub(super) fn evaluate_mic_signal_health(&self, app: &AppHandle) {
        let runtime = {
            let pipeline = self.pipeline.lock();
            pipeline.as_ref().map(|handles| {
                (
                    handles.capture.mic_path_ready(),
                    handles.capture.mic_frames_seen(),
                    handles.capture.mic_level_dbfs(),
                )
            })
        };

        let Some((mic_path_ready, mic_frames_seen, mic_level_dbfs)) = runtime else {
            self.reset_mic_signal_observer();
            return;
        };

        let (should_observe_mic, mic_failure_policy) = {
            let mut state = self.state.lock();
            state.mic_path_ready = mic_path_ready;
            state.mic_frames_seen = mic_frames_seen;
            state.mic_level_dbfs = mic_level_dbfs;

            (
                state.settings.mic_enabled
                    && state.settings.audio_mode == "system_plus_mic"
                    && state.active_audio_mode == "system_plus_mic",
                state.settings.mic_failure_policy.clone(),
            )
        };

        if !should_observe_mic || !mic_path_ready || !mic_frames_seen {
            self.reset_mic_signal_observer();
            self.state.lock().mic_signal_silent = false;
            return;
        }

        let Some(level_dbfs) = mic_level_dbfs else {
            return;
        };

        if level_dbfs > MIC_SIGNAL_SILENT_DBFS_THRESHOLD {
            self.reset_mic_signal_observer();
            self.state.lock().mic_signal_silent = false;
            return;
        }

        let now = Instant::now();
        let elapsed = {
            let mut silence_since = self.mic_signal_silence_since.lock();
            let started = silence_since.get_or_insert(now);
            now.saturating_duration_since(*started)
        };

        if elapsed < Duration::from_secs(MIC_SIGNAL_SILENCE_HOLD_SECS) {
            return;
        }

        let mut warning_emitted = self.mic_signal_warning_emitted.lock();
        if *warning_emitted {
            return;
        }
        *warning_emitted = true;
        self.state.lock().mic_signal_silent = true;

        let action = if mic_failure_policy == "required" {
            "Unmute or raise mic input level, or switch mic policy to best_effort."
        } else {
            "Unmute or raise mic input level in macOS Sound settings."
        };
        events::emit_save_warning(
            app,
            "mic_signal_missing",
            "Microphone connected but near-silent signal detected.",
            Some(action.to_string()),
        );
    }

    pub(super) fn evaluate_mic_offline_watchdog(&self, app: &AppHandle) {
        let (mic_expected, mic_path_ready, mic_retry_interval_secs) = {
            let state = self.state.lock();
            let expected = state.settings.mic_enabled
                && state.settings.audio_mode == "system_plus_mic"
                && state.active_audio_mode == "system_plus_mic";
            (
                expected,
                state.mic_path_ready,
                state.settings.mic_retry_interval_secs,
            )
        };

        if !mic_expected || mic_path_ready {
            *self.mic_offline_since.lock() = None;
            *self.mic_offline_watchdog_warned.lock() = false;
            return;
        }

        let now = Instant::now();
        let elapsed = {
            let mut offline_since = self.mic_offline_since.lock();
            let started = offline_since.get_or_insert(now);
            now.saturating_duration_since(*started)
        };

        let watchdog_timeout = Duration::from_secs(
            u64::from(mic_retry_interval_secs.max(5)) * MIC_OFFLINE_WATCHDOG_MULTIPLIER,
        );
        if elapsed < watchdog_timeout {
            return;
        }

        let mut warned = self.mic_offline_watchdog_warned.lock();
        if *warned {
            return;
        }
        *warned = true;

        self.append_capture_runtime_marker(&format!(
            "phase: mic_offline_watchdog_triggered elapsed_secs={}",
            elapsed.as_secs()
        ));
        events::emit_save_warning(
            app,
            "mic_offline_watchdog",
            "Microphone has been offline for an extended period despite retry attempts.",
            Some("Check mic device/permissions or set mic policy to best_effort.".to_string()),
        );
    }

    pub(super) fn reset_mic_signal_observer(&self) {
        *self.mic_signal_silence_since.lock() = None;
        *self.mic_signal_warning_emitted.lock() = false;
        *self.mic_offline_since.lock() = None;
        *self.mic_offline_watchdog_warned.lock() = false;
    }

    pub(super) fn stop_pipeline_if_running(&self) {
        let Some(mut handles) = self.pipeline.lock().take() else {
            return;
        };

        handles
            .capture
            .append_runtime_marker("phase: pipeline_stop_requested");
        handles.capture.stop();
        self.pending_smooth_jobs.lock().clear();
        self.reset_overload_metric_counters();
        self.reset_resource_pressure_tracking();
        *self.no_segments_miss_count.lock() = 0;
        *self.last_pipeline_started_at.lock() = None;
        *self.startup_requested_profile_hold_logged.lock() = false;
        self.reset_system_audio_readiness_tracking();
        self.reset_mic_signal_observer();
    }

    pub(super) fn can_step_down_profile(&self) -> bool {
        let settings = self.state.lock().settings.clone();
        if settings.quality_policy != "adaptive_recover" {
            return false;
        }
        let current_idx = *self.runtime_profile_index.lock();
        if current_idx >= MAX_RUNTIME_PROFILE_INDEX {
            return false;
        }
        let current = effective_profile_for_index(&settings, current_idx);
        let next = effective_profile_for_index(&settings, current_idx.saturating_add(1));
        current != next
    }

    pub(super) fn can_step_up_profile(&self) -> bool {
        let current_idx = *self.runtime_profile_index.lock();
        current_idx > 0
    }

    pub(super) fn advance_runtime_profile_for_overload(&self) -> Option<(String, String)> {
        let settings = self.state.lock().settings.clone();
        let mut profile_idx = self.runtime_profile_index.lock();
        let current = effective_profile_for_index(&settings, *profile_idx);
        if *profile_idx >= MAX_RUNTIME_PROFILE_INDEX {
            return None;
        }
        let next_idx = profile_idx.saturating_add(1);
        let next = effective_profile_for_index(&settings, next_idx);
        if next == current {
            return None;
        }
        *profile_idx = next_idx;
        Some((current.label(), next.label()))
    }

    pub(super) fn advance_runtime_profile_for_overload_steps(
        &self,
        steps: usize,
    ) -> Option<(String, String)> {
        let mut transition: Option<(String, String)> = None;
        for _ in 0..steps.max(1) {
            let Some((from, to)) = self.advance_runtime_profile_for_overload() else {
                break;
            };
            transition = Some(match transition {
                Some((original_from, _)) => (original_from, to),
                None => (from, to),
            });
        }
        transition
    }

    pub(super) fn regress_runtime_profile_for_recovery(&self) -> Option<(String, String)> {
        let settings = self.state.lock().settings.clone();
        let mut profile_idx = self.runtime_profile_index.lock();
        if *profile_idx == 0 {
            return None;
        }
        let current = effective_profile_for_index(&settings, *profile_idx);
        let next_idx = profile_idx.saturating_sub(1);
        let next = effective_profile_for_index(&settings, next_idx);
        if next == current {
            return None;
        }
        *profile_idx = next_idx;
        Some((current.label(), next.label()))
    }

    pub(super) fn record_restart_attempt(&self) -> bool {
        let now = Instant::now();
        let mut history = self.restart_history.lock();
        history
            .retain(|at| now.duration_since(*at) <= Duration::from_secs(RESTART_LOOP_WINDOW_SECS));
        history.push(now);
        if history.len() > RESTART_LOOP_MAX_ATTEMPTS {
            *self.crash_loop_cooldown_until.lock() =
                Some(now + Duration::from_secs(RESTART_LOOP_COOLDOWN_SECS));
            return true;
        }
        false
    }

    pub(super) fn clear_restart_window(&self) {
        self.restart_history.lock().clear();
        *self.crash_loop_cooldown_until.lock() = None;
    }

    pub(super) fn rollback_settings(&self, previous_settings: &SettingsDto) {
        *self.runtime_profile_index.lock() = 0;
        *self.overload_since.lock() = None;
        *self.recover_since.lock() = None;
        *self.last_profile_change_at.lock() = None;
        *self.last_mic_retry_at.lock() = None;
        *self.next_queue_profile.lock() = LiveQueueProfile::Small;
        *self.startup_bootstrap_until.lock() = None;
        *self.startup_bootstrap_pending.lock() = false;
        *self.startup_requested_profile_hold_logged.lock() = false;
        self.reset_startup_interrupt_retry_state();
        self.reset_system_audio_readiness_tracking();
        self.reset_mic_signal_observer();
        self.pending_smooth_jobs.lock().clear();
        let mut state = self.state.lock();
        state.settings = previous_settings.clone();
        state.lifecycle_state =
            lifecycle::idle_state(&state.permission, previous_settings.replay_enabled);
        state.capture_health = if previous_settings.replay_enabled {
            CaptureHealthDto::Starting
        } else {
            CaptureHealthDto::Stopped
        };
        state.audio_health = if previous_settings.replay_enabled {
            AudioHealthDto::Degraded
        } else {
            AudioHealthDto::Unavailable
        };
        state.active_audio_mode = previous_settings.audio_mode.clone();
        state.effective_audio_mode = previous_settings.audio_mode.clone();
        state.audio_fallback_policy = previous_settings.audio_fallback_policy.clone();
        state.capture_speed_x = None;
        state.capture_load_state = "normal".to_string();
        state.live_queue_profile = LiveQueueProfile::Small.as_str().to_string();
        state.save_ready = false;
        state.system_audio_path_ready = false;
        state.mic_path_ready = false;
        state.mic_frames_seen = false;
        state.mic_level_dbfs = None;
        state.mic_capture_session_running = false;
        state.mic_samples_per_sec = None;
        state.mic_attach_state = MicAttachStateDto::Inactive;
        state.mic_signal_silent = false;
        state.concurrent_session_count = None;
        state.capture_owner_pid = None;
        state.audio_path_ready = false;
        state.first_audio_frame_seen = false;
        state.save_stage = SaveStageDto::Idle;
        state.video_smooth_state = VideoSmoothStateDto::Idle;
        state.mic_backend_in_use = previous_settings.mic_capture_backend.clone();
        state.mic_mix_gain_db = previous_settings.mic_mix_gain_db;
        state.requested_video_resolution = previous_settings.video_resolution;
        state.requested_fps = previous_settings.fps;
        state.requested_video_bitrate_kbps = previous_settings.video_bitrate_kbps;
        state.effective_video_resolution = previous_settings.video_resolution;
        state.effective_fps = previous_settings.fps;
        state.effective_video_bitrate_kbps = previous_settings.video_bitrate_kbps;
        state.degrade_reason = None;
        state.audio_degrade_reason = None;
        state.last_audio_mode_error = None;
    }
}
