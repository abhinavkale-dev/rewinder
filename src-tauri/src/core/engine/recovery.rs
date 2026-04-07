use super::*;

impl Engine {
    pub(super) fn start_pipeline_recovery_worker(self: &Arc<Self>, app: AppHandle) {
        if self.recovery_worker.lock().is_some() {
            return;
        }

        self.recovery_stop.store(false, Ordering::Relaxed);
        let engine = Arc::clone(self);
        let stop = Arc::clone(&self.recovery_stop);

        let worker = thread::spawn(move || {
            let mut retry_delay_secs = 2_u64;

            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }

                if engine.is_capture_paused_by_user() {
                    engine.stop_pipeline_if_running();
                    engine.process_pending_fast_integrity_jobs(&app);
                    if wait_for_stop(&stop, Duration::from_millis(500)) {
                        break;
                    }
                    continue;
                }

                engine.refresh_runtime_readiness_from_pipeline();
                engine.evaluate_mic_signal_health(&app);
                engine.evaluate_mic_offline_watchdog(&app);
                engine.process_pending_save(&app);
                engine.process_pending_smooth_jobs(&app);
                engine.process_pending_fast_integrity_jobs(&app);

                let restart_reason = engine.restart_reason_if_needed();
                if let Some(transition) = engine.take_pending_guard_transition() {
                    events::emit_perf_guard_transition(
                        &app,
                        events::PerfGuardTransitionPayload {
                            action: transition.action,
                            guard_state: engine.state.lock().guard_state.clone(),
                            hard: transition.hard,
                            primary_reason_code: transition.primary_reason_code,
                            contributing_reason_codes: transition.contributing_reason_codes,
                            suppressed_reason_code: transition.suppressed_reason_code,
                            from_profile: transition.from_profile,
                            to_profile: transition.to_profile,
                            sampled_at_epoch_ms: transition.sampled_at_epoch_ms,
                        },
                    );
                    events::emit_engine_state(&app, &engine.get_engine_state());
                }
                let mut sleep_for = Duration::from_secs(2);

                if let Some(reason) = restart_reason {
                    if engine.is_capture_paused_by_user() {
                        engine.stop_pipeline_if_running();
                        if wait_for_stop(&stop, Duration::from_millis(500)) {
                            break;
                        }
                        continue;
                    }

                    if matches!(reason, CaptureRestartReason::UserStoppedSharing) {
                        engine.disarm_for_user_stopped_sharing(
                            &app,
                            Some(
                                "Screen recording was interrupted. Click Restart Capture to resume."
                                    .to_string(),
                            ),
                        );
                        retry_delay_secs = 2;
                        if wait_for_stop(&stop, Duration::from_millis(500)) {
                            break;
                        }
                        continue;
                    }

                    let mut pre_restart_delay: Option<Duration> = None;
                    if matches!(reason, CaptureRestartReason::CaptureStartInterrupted) {
                        let now = Instant::now();
                        if let Some((attempt, delay)) = engine.next_startup_interrupt_retry(now) {
                            engine.append_capture_runtime_marker(&format!(
                                "phase: startup_interrupted_retry attempt={attempt}"
                            ));
                            pre_restart_delay = Some(delay);
                        } else {
                            engine.append_capture_runtime_marker(
                                "phase: startup_interrupted_retry_exhausted",
                            );
                            events::emit_capture_health_changed(
                                &app,
                                "restarting",
                                Some(
                                    "Capture startup interrupted; backing off before retry."
                                        .to_string(),
                                ),
                            );
                            events::emit_engine_state(&app, &engine.get_engine_state());
                            retry_delay_secs = 2;
                            if wait_for_stop(
                                &stop,
                                Duration::from_secs(STARTUP_INTERRUPT_WINDOW_SECS / 2),
                            ) {
                                break;
                            }
                            continue;
                        }
                    } else {
                        engine.reset_startup_interrupt_retry_state();
                    }

                    if let Some(delay) = pre_restart_delay {
                        if wait_for_stop(&stop, delay) {
                            break;
                        }
                    }

                    let current_speed = {
                        let pipeline = engine.pipeline.lock();
                        pipeline
                            .as_ref()
                            .and_then(|handles| handles.capture.capture_speed_x())
                    };
                    let profile_transition =
                        match reason {
                            CaptureRestartReason::Overloaded => {
                                *engine.overload_since.lock() = None;
                                if let Some(handles) = engine.pipeline.lock().as_ref() {
                                    handles
                                        .capture
                                        .append_runtime_marker("phase: overload_detected");
                                }
                                let hard_stepdown = {
                                    let mut pending = engine.resource_hard_stepdown_pending.lock();
                                    let value = *pending;
                                    *pending = false;
                                    value
                                };
                                engine.append_capture_runtime_marker(&format!(
                                    "phase: perf_guard_enter speed={} hard_stepdown={}",
                                    current_speed
                                        .map(|value| format!("{value:.3}"))
                                        .unwrap_or_else(|| "unknown".to_string()),
                                    hard_stepdown
                                ));
                                engine.advance_runtime_profile_for_overload_steps(
                                    if hard_stepdown { 2 } else { 1 },
                                )
                            }
                            CaptureRestartReason::ProfileRecovered => {
                                *engine.recover_since.lock() = None;
                                engine.regress_runtime_profile_for_recovery()
                            }
                            _ => None,
                        };
                    if profile_transition.is_some() {
                        *engine.last_profile_change_at.lock() = Some(Instant::now());
                    }

                    if engine.record_restart_attempt() {
                        {
                            let mut state = engine.state.lock();
                            state.capture_crash_loop = true;
                            state.capture_health = CaptureHealthDto::Degraded;
                            state.audio_health = AudioHealthDto::Degraded;
                            state.last_error = Some(
                                "Capture is crash-looping; pausing restarts briefly.".to_string(),
                            );
                        }
                        events::emit_save_failed_code(
                            &app,
                            "capture_crash_loop",
                            "Capture helper is crash-looping; retrying after cooldown.",
                            Some(
                                "Recheck permissions/displays. Manual Save remains available."
                                    .to_string(),
                            ),
                        );
                        events::emit_engine_state(&app, &engine.get_engine_state());
                        sleep_for = Duration::from_secs(RESTART_LOOP_COOLDOWN_SECS);
                        if wait_for_stop(&stop, sleep_for) {
                            break;
                        }
                        continue;
                    }

                    engine.stop_pipeline_if_running();
                    {
                        let mut state = engine.state.lock();
                        state.capture_health = CaptureHealthDto::Restarting;
                        state.last_error = Some(match (&profile_transition, reason) {
                            (Some((from, to)), CaptureRestartReason::Overloaded) => {
                                format!("{} ({from} -> {to})", reason.as_message())
                            }
                            (Some((from, to)), CaptureRestartReason::ProfileRecovered) => {
                                format!("{} ({from} -> {to})", reason.as_message())
                            }
                            _ => match reason.detail() {
                                Some(detail) => format!("{} ({detail})", reason.as_message()),
                                None => reason.as_message().to_string(),
                            },
                        });
                        state.capture_restart_count = state.capture_restart_count.saturating_add(1);
                        state.capture_speed_x = None;
                        state.capture_load_state = "recovering".to_string();
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
                    }
                    events::emit_capture_health_changed(
                        &app,
                        "restarting",
                        Some(match reason.detail() {
                            Some(detail) => format!("{} ({detail})", reason.as_message()),
                            None => reason.as_message().to_string(),
                        }),
                    );
                    events::emit_capture_restarted(&app, reason.as_code());

                    if engine.is_capture_paused_by_user() {
                        engine.stop_pipeline_if_running();
                        if wait_for_stop(&stop, Duration::from_millis(500)) {
                            break;
                        }
                        continue;
                    }

                    match engine.start_pipeline_if_needed(AudioStartupStrategy::SystemFirst) {
                        Ok(()) => {
                            retry_delay_secs = 2;
                            {
                                let mut state = engine.state.lock();
                                state.capture_health = CaptureHealthDto::Running;
                                state.audio_health = AudioHealthDto::Ok;
                                state.capture_crash_loop = false;
                                state.capture_speed_x = None;
                                state.capture_load_state = "normal".to_string();
                                state.save_ready = false;
                                if state
                                    .last_error
                                    .as_deref()
                                    .map(|e| {
                                        is_transient_capture_error(e)
                                            || e.contains("Capture pipeline missing")
                                    })
                                    .unwrap_or(false)
                                {
                                    state.last_error = None;
                                }
                            }
                            engine.clear_restart_window();
                            engine.reset_startup_interrupt_retry_state();
                            let snapshot = engine.get_engine_state();
                            events::emit_mic_permission_changed(
                                &app,
                                snapshot.mic_permission_status.clone(),
                                snapshot.mic_permission_error.clone(),
                            );
                            if snapshot.audio_path_ready {
                                events::emit_audio_path_ready(
                                    &app,
                                    snapshot.active_audio_mode.clone(),
                                );
                            }
                            if snapshot.settings.audio_mode == "system_plus_mic"
                                && snapshot.settings.mic_enabled
                            {
                                if snapshot.mic_path_ready {
                                    *engine.last_mic_retry_at.lock() = None;
                                    events::emit_mic_path_recovered(
                                        &app,
                                        "Microphone path recovered.",
                                    );
                                } else if snapshot.settings.mic_failure_policy == "best_effort" {
                                    let message = if snapshot.mic_recovery_state == "retrying" {
                                        "Microphone unavailable; continuing replay and retrying mic."
                                    } else {
                                        "Microphone unavailable; continuing with system audio."
                                    };
                                    events::emit_mic_path_degraded(
                                        &app,
                                        "mic_backend_error",
                                        message,
                                        Some(
                                            "Check mic permission/device. Rewinder will keep replay active."
                                                .to_string(),
                                        ),
                                    );
                                }
                            }
                            if snapshot.active_audio_mode != snapshot.settings.audio_mode {
                                let audio_mode_reason = if snapshot
                                    .last_audio_mode_error
                                    .as_deref()
                                    .map(|message| {
                                        message.contains("mic_pipe_startup_stalled")
                                            || message.contains("mic_first_frame_startup_stalled")
                                    })
                                    .unwrap_or(false)
                                {
                                    "Microphone startup stalled; continuing with system audio only."
                                        .to_string()
                                } else {
                                    "Capture auto-degraded for stability.".to_string()
                                };
                                events::emit_capture_degraded(
                                    &app,
                                    format!(
                                        "audio mode degraded from {} to {}",
                                        snapshot.settings.audio_mode, snapshot.active_audio_mode
                                    ),
                                );
                                events::emit_audio_mode_changed(
                                    &app,
                                    snapshot.active_audio_mode.clone(),
                                    Some(audio_mode_reason),
                                );
                            }
                            if snapshot.audio_fallback_policy == "system_only_fallback"
                                && snapshot.settings.audio_mode != "video_only"
                                && snapshot.active_audio_mode == "video_only"
                            {
                                events::emit_audio_path_failed(
                                    &app,
                                    "Required audio path unavailable; capture reached video-only.",
                                    Some(
                                        "Grant audio permissions or switch fallback to allow_video_only."
                                            .to_string(),
                                    ),
                                );
                            } else if let Some(audio_err) = snapshot.last_audio_mode_error.clone() {
                                events::emit_audio_path_failed(
                                    &app,
                                    audio_err,
                                    Some(
                                        "Recheck Microphone/System Audio permissions in System Settings."
                                            .to_string(),
                                    ),
                                );
                            }
                            if let Some((from, to)) = profile_transition {
                                let (primary_reason_code, contributing_reason_codes) = {
                                    let state = engine.state.lock();
                                    (
                                        state.guard_primary_reason_code.clone(),
                                        state.guard_contributing_reason_codes.clone(),
                                    )
                                };
                                let hard_transition = matches!(
                                    primary_reason_code.as_deref(),
                                    Some(
                                        "thermal_critical"
                                            | "system_memory_pressure_critical"
                                            | "capture_stack_cpu_hard"
                                            | "capture_stack_rss_growth_hard"
                                    )
                                );
                                let guard_state =
                                    if matches!(reason, CaptureRestartReason::Overloaded) {
                                        "protecting"
                                    } else if *engine.runtime_profile_index.lock() > 0 {
                                        "protecting"
                                    } else {
                                        "monitoring"
                                    };
                                engine.record_guard_transition(
                                    if matches!(reason, CaptureRestartReason::Overloaded) {
                                        "step_down"
                                    } else {
                                        "step_up"
                                    },
                                    guard_state,
                                    hard_transition,
                                    primary_reason_code,
                                    contributing_reason_codes,
                                    None,
                                    Some(from.clone()),
                                    Some(to.clone()),
                                );
                                if let Some(transition) = engine.take_pending_guard_transition() {
                                    events::emit_perf_guard_transition(
                                        &app,
                                        events::PerfGuardTransitionPayload {
                                            action: transition.action,
                                            guard_state: engine.state.lock().guard_state.clone(),
                                            hard: transition.hard,
                                            primary_reason_code: transition.primary_reason_code,
                                            contributing_reason_codes: transition
                                                .contributing_reason_codes,
                                            suppressed_reason_code: transition
                                                .suppressed_reason_code,
                                            from_profile: transition.from_profile,
                                            to_profile: transition.to_profile,
                                            sampled_at_epoch_ms: transition.sampled_at_epoch_ms,
                                        },
                                    );
                                }
                                if let Some(handles) = engine.pipeline.lock().as_ref() {
                                    handles
                                        .capture
                                        .append_runtime_marker("phase: fallback_applied");
                                }
                                if matches!(reason, CaptureRestartReason::Overloaded) {
                                    engine.append_capture_runtime_marker(&format!(
                                        "phase: perf_guard_step_down from={} to={}",
                                        from, to
                                    ));
                                    events::emit_capture_profile_changed(
                                        &app,
                                        from.clone(),
                                        to.clone(),
                                        "capture_overloaded",
                                    );
                                    events::emit_capture_degraded(
                                        &app,
                                        format!(
                                            "Capture overloaded; auto-fallback applied ({from} -> {to})"
                                        ),
                                    );
                                } else if matches!(reason, CaptureRestartReason::ProfileRecovered) {
                                    engine.append_capture_runtime_marker(&format!(
                                        "phase: perf_guard_recover from={} to={}",
                                        from, to
                                    ));
                                    let reached_requested_profile =
                                        *engine.runtime_profile_index.lock() == 0;
                                    if reached_requested_profile
                                        && *engine.startup_bootstrap_pending.lock()
                                    {
                                        *engine.startup_bootstrap_pending.lock() = false;
                                        *engine.startup_bootstrap_until.lock() = None;
                                        engine.append_capture_runtime_marker(
                                            "phase: startup_bootstrap_profile_recovered",
                                        );
                                    }
                                    events::emit_capture_profile_recovered(
                                        &app,
                                        from.clone(),
                                        to.clone(),
                                        "capture_stable",
                                    );
                                }
                            }
                            events::emit_engine_state(&app, &snapshot);
                        }
                        Err(err) => {
                            let retry_message = format!("capture recovery retry failed: {err}");
                            let (code, action) = classify_capture_failure(&retry_message);
                            if code == "capture_start_interrupted" {
                                {
                                    let mut state = engine.state.lock();
                                    state.capture_interrupt_count =
                                        state.capture_interrupt_count.saturating_add(1);
                                    state.last_error = Some(retry_message.clone());
                                    state.capture_health = CaptureHealthDto::Restarting;
                                    state.audio_health = AudioHealthDto::Degraded;
                                    state.capture_load_state = "recovering".to_string();
                                }
                                engine.append_capture_runtime_marker(
                                    "phase: startup_interrupted_sc3805",
                                );
                                let now = Instant::now();
                                if let Some((attempt, delay)) =
                                    engine.next_startup_interrupt_retry(now)
                                {
                                    engine.append_capture_runtime_marker(&format!(
                                        "phase: startup_interrupted_retry attempt={attempt}"
                                    ));
                                    events::emit_capture_health_changed(
                                        &app,
                                        "restarting",
                                        Some(
                                            "Capture startup interrupted; retrying with backoff."
                                                .to_string(),
                                        ),
                                    );
                                    events::emit_engine_state(&app, &engine.get_engine_state());
                                    if wait_for_stop(&stop, delay) {
                                        break;
                                    }
                                    continue;
                                }

                                engine.append_capture_runtime_marker(
                                    "phase: startup_interrupted_retry_exhausted",
                                );
                                events::emit_capture_health_changed(
                                    &app,
                                    "restarting",
                                    Some(
                                        "Capture startup interrupted; backing off before retry."
                                            .to_string(),
                                    ),
                                );
                                events::emit_engine_state(&app, &engine.get_engine_state());
                                retry_delay_secs = 2;
                                if wait_for_stop(
                                    &stop,
                                    Duration::from_secs(STARTUP_INTERRUPT_WINDOW_SECS / 2),
                                ) {
                                    break;
                                }
                                continue;
                            }
                            if code == "user_stopped_sharing" {
                                engine.disarm_for_user_stopped_sharing(
                                    &app,
                                    Some(
                                        "Screen recording was interrupted. Click Restart Capture to resume."
                                            .to_string(),
                                    ),
                                );
                                continue;
                            }
                            if code == "capture_paused" {
                                engine.set_capture_paused_by_user(
                                    &app,
                                    true,
                                    Some(
                                        "Capture paused. Rewinder is not recording in the background. Click Resume Capture."
                                            .to_string(),
                                    ),
                                );
                                continue;
                            }
                            if code == "capture_owner_exists" {
                                engine.set_capture_paused_by_user(
                                    &app,
                                    true,
                                    Some(
                                        "Another Rewinder instance is already capturing. Close duplicate launches, then click Resume Capture."
                                            .to_string(),
                                    ),
                                );
                                continue;
                            }
                            if code == "output_dir_permission_required" {
                                {
                                    let mut state = engine.state.lock();
                                    state.last_error = Some(retry_message.clone());
                                    state.permission.output_dir_writable = false;
                                    state.permission.output_dir_permission_error =
                                        Some(err.clone());
                                    if state.permission.screen_recording_granted {
                                        state.permission.reason = Some(err.clone());
                                    }
                                    state.lifecycle_state = lifecycle::idle_state(
                                        &state.permission,
                                        state.settings.replay_enabled,
                                    );
                                    state.capture_health = CaptureHealthDto::Stopped;
                                    state.audio_health = AudioHealthDto::Unavailable;
                                    state.capture_load_state = "normal".to_string();
                                }
                                events::emit_save_failed_code(&app, code, retry_message, action);
                                events::emit_permission_required(&app, err.clone());
                                events::emit_capture_health_changed(
                                    &app,
                                    "stopped",
                                    Some(
                                        "Capture paused until Downloads folder access is granted."
                                            .to_string(),
                                    ),
                                );
                                events::emit_engine_state(&app, &engine.get_engine_state());
                                continue;
                            }
                            {
                                let mut state = engine.state.lock();
                                state.last_error = Some(retry_message.clone());
                                state.lifecycle_state = lifecycle::idle_state(
                                    &state.permission,
                                    state.settings.replay_enabled,
                                );
                                state.capture_health = CaptureHealthDto::Degraded;
                                state.audio_health = if matches!(
                                    code,
                                    "system_audio_unavailable"
                                        | "audio_start_timeout"
                                        | "mic_pipe_startup_stalled"
                                        | "mic_first_frame_startup_stalled"
                                        | "mic_start_timeout"
                                ) {
                                    AudioHealthDto::Unavailable
                                } else {
                                    AudioHealthDto::Degraded
                                };
                                state.capture_load_state = "stressed".to_string();
                                if matches!(
                                    code,
                                    "system_audio_unavailable"
                                        | "audio_start_timeout"
                                        | "mic_pipe_startup_stalled"
                                        | "mic_first_frame_startup_stalled"
                                        | "mic_start_timeout"
                                ) {
                                    state.last_audio_mode_error = Some(retry_message.clone());
                                }
                            }
                            events::emit_save_failed_code(&app, code, retry_message, action);
                            if matches!(
                                code,
                                "system_audio_unavailable"
                                    | "audio_start_timeout"
                                    | "mic_pipe_startup_stalled"
                                    | "mic_first_frame_startup_stalled"
                                    | "mic_start_timeout"
                            ) {
                                events::emit_audio_path_failed(
                                    &app,
                                    "Audio pipeline is unavailable for the selected policy.",
                                    Some(
                                        "Fix audio permissions/devices or update audio fallback policy."
                                            .to_string(),
                                    ),
                                );
                            }
                            events::emit_capture_health_changed(
                                &app,
                                "degraded",
                                Some("Capture restart failed; retrying with backoff.".to_string()),
                            );
                            events::emit_engine_state(&app, &engine.get_engine_state());
                            sleep_for = Duration::from_secs(retry_delay_secs);
                            retry_delay_secs = (retry_delay_secs.saturating_mul(2)).min(15);
                        }
                    }
                } else {
                    retry_delay_secs = 2;
                }

                let pending_sleep_cap = {
                    let pending_guard = engine.pending_save.lock();
                    pending_guard.as_ref().map(|_| Duration::from_millis(500))
                };
                if let Some(cap) = pending_sleep_cap {
                    sleep_for = sleep_for.min(cap);
                }
                if !engine.pending_smooth_jobs.lock().is_empty() {
                    sleep_for = sleep_for.min(Duration::from_millis(250));
                }

                if wait_for_stop(&stop, sleep_for) {
                    break;
                }
            }
        });

        *self.recovery_worker.lock() = Some(worker);
    }
}
