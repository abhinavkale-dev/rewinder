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

pub(super) fn push_guard_reason(codes: &mut Vec<String>, condition: bool, code: &str) {
    if condition && !codes.iter().any(|existing| existing == code) {
        codes.push(code.to_string());
    }
}

pub(super) fn select_primary_guard_reason_code(codes: &[String]) -> Option<String> {
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
        "on_battery",
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

    pub(super) fn evaluate_mic_signal_health(&self, app: &Arc<dyn EngineHost>) {
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

    pub(super) fn evaluate_mic_offline_watchdog(&self, app: &Arc<dyn EngineHost>) {
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

    pub(super) fn battery_floor_now(&self) -> usize {
        let settings = self.state.lock().settings.clone();
        let (.., power_source) = self.current_process_diagnostics();
        let on_battery = matches!(power_source.as_deref(), Some("battery"));
        battery_floor_index(
            &settings,
            on_battery,
            settings.battery_guard_enabled,
            settings.battery_max_fps,
        )
    }

    pub(super) fn set_runtime_profile_at_least_floor(
        &self,
        floor: usize,
    ) -> Option<(String, String)> {
        let settings = self.state.lock().settings.clone();
        let target_idx = floor.min(MAX_RUNTIME_PROFILE_INDEX);
        let mut profile_idx = self.runtime_profile_index.lock();
        if *profile_idx >= target_idx {
            return None;
        }
        let current = effective_profile_for_index(&settings, *profile_idx);
        let next = effective_profile_for_index(&settings, target_idx);
        *profile_idx = target_idx;
        if next == current {
            return None;
        }
        Some((current.label(), next.label()))
    }

    pub(super) fn lower_runtime_profile_to_floor(&self, floor: usize) -> Option<(String, String)> {
        let settings = self.state.lock().settings.clone();
        let target_idx = floor.min(MAX_RUNTIME_PROFILE_INDEX);
        let mut profile_idx = self.runtime_profile_index.lock();
        if *profile_idx <= target_idx {
            return None;
        }
        let current = effective_profile_for_index(&settings, *profile_idx);
        let next = effective_profile_for_index(&settings, target_idx);
        *profile_idx = target_idx;
        if next == current {
            return None;
        }
        Some((current.label(), next.label()))
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
