use super::*;

impl Engine {
    pub(super) fn clear_user_stop_disarm(&self) {
        *self.user_stop_disarmed_reason.lock() = None;
    }

    pub(crate) fn shutdown_for_app_exit(&self, reason: &str) {
        self.append_capture_runtime_marker(&format!(
            "phase: app_shutdown_requested reason={reason}"
        ));
        self.recovery_stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.recovery_worker.lock().take() {
            let _ = worker.join();
        }

        *self.capture_paused_by_user.lock() = false;
        *self.capture_pause_reason.lock() = None;
        *self.user_stop_disarmed_reason.lock() = None;
        self.pending_save.lock().take();
        self.pending_smooth_jobs.lock().clear();
        self.pending_fast_verify_jobs.lock().clear();
        self.fast_verify_inflight.store(false, Ordering::Relaxed);
        *self.runtime_profile_index.lock() = 0;
        *self.overload_since.lock() = None;
        *self.recover_since.lock() = None;
        *self.last_mic_retry_at.lock() = None;
        *self.last_pipeline_started_at.lock() = None;
        *self.last_save_started_at.lock() = None;
        *self.last_profile_change_at.lock() = None;
        *self.next_queue_profile.lock() = LiveQueueProfile::Small;
        *self.startup_bootstrap_until.lock() = None;
        *self.startup_bootstrap_pending.lock() = false;
        *self.startup_requested_profile_hold_logged.lock() = false;
        *self.capture_stack_rss_baseline_mb.lock() = None;
        self.process_diagnostics_cache.lock().take();
        self.pending_guard_transition.lock().take();
        self.reset_resource_pressure_tracking();
        *self.no_segments_miss_count.lock() = 0;
        self.reset_startup_interrupt_retry_state();
        self.reset_system_audio_readiness_tracking();
        self.reset_mic_signal_observer();
        *self.mic_device_not_found_warned.lock() = false;

        self.stop_pipeline_if_running();

        {
            let mut state = self.state.lock();
            state.settings.replay_enabled = false;
            state.lifecycle_state = lifecycle::idle_state(&state.permission, false);
            state.capture_health = CaptureHealthDto::Stopped;
            state.audio_health = AudioHealthDto::Unavailable;
            state.capture_speed_x = None;
            state.capture_load_state = "normal".to_string();
            state.save_ready = false;
            state.system_audio_path_ready = false;
            state.mic_path_ready = false;
            state.mic_frames_seen = false;
            state.mic_level_dbfs = None;
            state.mic_capture_session_running = false;
            state.mic_samples_per_sec = None;
            state.mic_attach_state = MicAttachStateDto::Inactive;
            state.concurrent_session_count = None;
            state.capture_owner_pid = None;
            state.audio_path_ready = false;
            state.first_audio_frame_seen = false;
            state.save_stage = SaveStageDto::Idle;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.capture_crash_loop = false;
            state.degrade_reason = Some(format!("App shutdown requested ({reason})."));
            state.audio_degrade_reason = None;
            state.last_error = Some(format!("App exiting ({reason})."));
        }
    }

    pub(super) fn requires_system_audio(state: &ClipperState) -> bool {
        state.settings.audio_fallback_policy == "system_only_fallback"
            && state.settings.audio_mode != "video_only"
    }

    pub(super) fn is_capture_paused_by_user(&self) -> bool {
        *self.capture_paused_by_user.lock()
    }

    pub(super) fn set_capture_paused_by_user(
        &self,
        app: &AppHandle,
        paused: bool,
        reason: Option<String>,
    ) {
        self.clear_user_stop_disarm();
        *self.capture_paused_by_user.lock() = paused;
        *self.capture_pause_reason.lock() = reason.clone();

        if paused {
            let pause_message = reason.unwrap_or_else(|| {
                "Capture paused. Rewinder is not recording in the background.".to_string()
            });
            self.stop_pipeline_if_running();
            self.pending_save.lock().take();
            self.pending_smooth_jobs.lock().clear();
            *self.no_segments_miss_count.lock() = 0;
            *self.overload_since.lock() = None;
            *self.recover_since.lock() = None;
            *self.last_mic_retry_at.lock() = None;
            {
                let mut state = self.state.lock();
                state.capture_health = CaptureHealthDto::Stopped;
                state.audio_health = AudioHealthDto::Unavailable;
                state.capture_speed_x = None;
                state.capture_load_state = "normal".to_string();
                state.guard_state = "idle".to_string();
                state.guard_primary_reason_code = None;
                state.guard_contributing_reason_codes.clear();
                state.guard_suppressed_reason_code = None;
                state.save_ready = false;
                state.system_audio_path_ready = false;
                state.mic_path_ready = false;
                state.mic_frames_seen = false;
                state.mic_level_dbfs = None;
                state.mic_capture_session_running = false;
                state.mic_samples_per_sec = None;
                state.mic_attach_state = MicAttachStateDto::Inactive;
                state.mic_recovery_state = "ok".to_string();
                state.mic_signal_silent = false;
                state.selected_microphone_name = None;
                state.last_mic_error_code = None;
                state.last_mic_error_message = None;
                state.concurrent_session_count = None;
                state.capture_owner_pid = None;
                state.audio_path_ready = false;
                state.first_audio_frame_seen = false;
                state.save_stage = SaveStageDto::Idle;
                state.video_smooth_state = VideoSmoothStateDto::Idle;
                state.last_error = Some(pause_message.clone());
                state.degrade_reason = Some("Capture paused by user action.".to_string());
            }
            self.append_capture_runtime_marker("phase: capture_paused_by_user");
            events::emit_capture_health_changed(app, "stopped", Some(pause_message.clone()));
            events::emit_capture_paused(app, pause_message);
            events::emit_engine_state(app, &self.get_engine_state());
            return;
        }

        self.append_capture_runtime_marker("phase: capture_resume_requested");
        let _ = reason;
    }

    pub(super) fn disarm_for_user_stopped_sharing(&self, app: &AppHandle, reason: Option<String>) {
        let disarm_message = reason.unwrap_or_else(|| {
            "Screen recording was interrupted. Click Restart Capture to resume.".to_string()
        });
        *self.capture_paused_by_user.lock() = false;
        *self.capture_pause_reason.lock() = None;
        *self.user_stop_disarmed_reason.lock() = Some(disarm_message.clone());
        self.stop_pipeline_if_running();
        self.pending_save.lock().take();
        self.pending_smooth_jobs.lock().clear();
        self.reset_resource_pressure_tracking();
        *self.no_segments_miss_count.lock() = 0;
        *self.overload_since.lock() = None;
        *self.recover_since.lock() = None;
        *self.last_mic_retry_at.lock() = None;
        self.reset_startup_interrupt_retry_state();
        {
            let mut state = self.state.lock();
            state.settings.replay_enabled = false;
            state.lifecycle_state = lifecycle::idle_state(&state.permission, false);
            state.capture_health = CaptureHealthDto::Stopped;
            state.audio_health = AudioHealthDto::Unavailable;
            state.capture_speed_x = None;
            state.capture_load_state = "normal".to_string();
            state.guard_state = "idle".to_string();
            state.guard_primary_reason_code = None;
            state.guard_contributing_reason_codes.clear();
            state.guard_suppressed_reason_code = None;
            state.save_ready = false;
            state.system_audio_path_ready = false;
            state.mic_path_ready = false;
            state.mic_frames_seen = false;
            state.mic_level_dbfs = None;
            state.mic_capture_session_running = false;
            state.mic_samples_per_sec = None;
            state.mic_attach_state = MicAttachStateDto::Inactive;
            state.mic_recovery_state = "ok".to_string();
            state.mic_signal_silent = false;
            state.selected_microphone_name = None;
            state.last_mic_error_code = None;
            state.last_mic_error_message = None;
            state.concurrent_session_count = None;
            state.capture_owner_pid = None;
            state.audio_path_ready = false;
            state.first_audio_frame_seen = false;
            state.save_stage = SaveStageDto::Idle;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.last_error = Some(disarm_message.clone());
            state.degrade_reason =
                Some("Capture stopped by user action from macOS controls.".to_string());
        }
        self.append_capture_runtime_marker("phase: user_stop_disarmed");
        self.append_capture_runtime_marker("phase: user_stop_restart_suppressed");
        events::emit_capture_health_changed(app, "stopped", Some(disarm_message));
        events::emit_engine_state(app, &self.get_engine_state());
    }

    pub(super) fn pause_for_capture_owner_conflict(&self, app: &AppHandle, code: &str) {
        if code != "capture_owner_exists" {
            return;
        }
        self.set_capture_paused_by_user(
            app,
            true,
            Some(
                "Another Rewinder instance is already capturing. Close duplicate launches, then click Resume Capture."
                    .to_string(),
            ),
        );
    }

    pub(super) fn reset_system_audio_readiness_tracking(&self) {
        *self.system_audio_not_ready_since.lock() = None;
        *self.last_system_audio_ready_at.lock() = None;
        *self.system_audio_hard_unavailable_logged.lock() = false;
    }

    pub(super) fn reset_startup_interrupt_retry_state(&self) {
        *self.startup_interrupt_window_started_at.lock() = None;
        *self.startup_interrupt_retry_count.lock() = 0;
    }

    pub(super) fn next_startup_interrupt_retry(&self, now: Instant) -> Option<(u8, Duration)> {
        {
            let mut window_started = self.startup_interrupt_window_started_at.lock();
            let mut retry_count = self.startup_interrupt_retry_count.lock();
            let window_expired = window_started
                .map(|started| {
                    now.saturating_duration_since(started)
                        > Duration::from_secs(STARTUP_INTERRUPT_WINDOW_SECS)
                })
                .unwrap_or(true);
            if window_expired {
                *window_started = Some(now);
                *retry_count = 0;
            }

            if *retry_count >= STARTUP_INTERRUPT_MAX_RETRIES {
                return None;
            }

            *retry_count = retry_count.saturating_add(1);
            let attempt = *retry_count;
            let delay_secs = match attempt {
                1 => 1,
                _ => 2,
            };
            Some((attempt, Duration::from_secs(delay_secs)))
        }
    }

    pub(super) fn current_audio_warmup_grace_ms(&self, state: &ClipperState) -> Option<u32> {
        if !Self::requires_system_audio(state) || state.system_audio_path_ready {
            return None;
        }
        let started_at = self.system_audio_not_ready_since.lock().to_owned();
        let remaining = started_at
            .map(|instant| {
                Duration::from_millis(SYSTEM_AUDIO_HARD_FAIL_AFTER_MS)
                    .saturating_sub(Instant::now().saturating_duration_since(instant))
            })
            .unwrap_or_else(|| Duration::from_millis(SYSTEM_AUDIO_HARD_FAIL_AFTER_MS));
        Some(remaining.as_millis().min(u128::from(u32::MAX)) as u32)
    }

    pub(super) fn save_blocker_with_runtime(&self, state: &ClipperState) -> Option<SaveBlocker> {
        if self.is_capture_paused_by_user() {
            return Some(SaveBlocker {
                code: "capture_paused",
                message: self.capture_pause_reason.lock().clone().unwrap_or_else(|| {
                    "Capture paused. Rewinder is not recording in the background. Click Resume Capture."
                        .to_string()
                }),
                action: Some("Click Resume Capture to continue.".to_string()),
                retryable: false,
            });
        }
        if !state.settings.replay_enabled {
            if let Some(message) = self.user_stop_disarmed_reason.lock().clone() {
                return Some(SaveBlocker {
                    code: "user_stopped_sharing",
                    message,
                    action: Some("Click Restart Capture to resume.".to_string()),
                    retryable: false,
                });
            }
        }

        if let Some(blocker) = save_blocker(state) {
            if blocker.code != "system_audio_unavailable" || !Self::requires_system_audio(state) {
                return Some(blocker);
            }

            let now = Instant::now();
            let not_ready_since = self.system_audio_not_ready_since.lock().to_owned();
            let elapsed = not_ready_since
                .map(|started| now.saturating_duration_since(started))
                .unwrap_or(Duration::ZERO);
            if elapsed >= Duration::from_millis(SYSTEM_AUDIO_HARD_FAIL_AFTER_MS) {
                return Some(SaveBlocker {
                    code: "system_audio_unavailable",
                    message: "System audio path unavailable for current source.".to_string(),
                    action: Some(
                        "Check source/output route, then retry. You can switch fallback to allow_video_only if needed."
                            .to_string(),
                    ),
                    retryable: false,
                });
            }

            let capture_warming = matches!(
                state.capture_health,
                CaptureHealthDto::Starting
                    | CaptureHealthDto::Restarting
                    | CaptureHealthDto::Stopped
            );
            let recent_reconnect = self
                .last_system_audio_ready_at
                .lock()
                .map(|instant| {
                    now.saturating_duration_since(instant)
                        < Duration::from_millis(SYSTEM_AUDIO_DROPOUT_GRACE_MS)
                })
                .unwrap_or(false);
            let message = if capture_warming
                && elapsed < Duration::from_millis(SYSTEM_AUDIO_STARTUP_GRACE_MS)
            {
                "System audio path is starting. Try again in a moment.".to_string()
            } else if recent_reconnect {
                "System audio path is reconnecting. Try again in a moment.".to_string()
            } else {
                "System audio path is recovering. Replay will auto-save when ready.".to_string()
            };
            return Some(SaveBlocker {
                code: "audio_warming_up",
                message,
                action: None,
                retryable: true,
            });
        }

        None
    }

    pub(super) fn refresh_runtime_readiness_from_pipeline(&self) {
        let replay_duration_secs = self.state.lock().settings.replay_duration_secs;
        let runtime = {
            let pipeline = self.pipeline.lock();
            pipeline.as_ref().map(|handles| {
                (
                    handles.capture.capture_speed_x(),
                    handles.capture.playback_realtime_x(),
                    handles.capture.save_ready(),
                    handles.capture.system_audio_path_ready(),
                    handles.capture.mic_path_ready(),
                    handles.capture.mic_frames_seen(),
                    handles.capture.mic_level_dbfs(),
                    handles.capture.mic_capture_session_running(),
                    handles.capture.mic_samples_per_sec(),
                    handles.capture.mic_attach_runtime_state(),
                    handles.capture.audio_path_ready(),
                    handles.capture.first_audio_frame_seen(),
                    handles.capture.live_queue_profile().as_str().to_string(),
                    handles.capture.queue_starvation_detected(),
                    handles.capture.capture_dropped_frames(),
                    handles.capture.capture_queue_overflows(),
                    handles.capture.effective_output_fps(),
                    handles
                        .capture
                        .concurrent_session_count(replay_duration_secs),
                    handles.capture.capture_owner_pid(),
                    handles.capture.mic_selected_device_not_found(),
                )
            })
        };

        let Some((
            capture_speed_x,
            playback_realtime_x,
            save_ready,
            system_audio_path_ready,
            mic_path_ready,
            mic_frames_seen,
            mic_level_dbfs,
            mic_capture_session_running,
            mic_samples_per_sec,
            mic_attach_runtime_state,
            audio_path_ready,
            first_audio_frame_seen,
            live_queue_profile,
            queue_starvation_detected,
            capture_dropped_frames,
            capture_queue_overflows,
            effective_output_fps,
            concurrent_session_count,
            capture_owner_pid,
            mic_selected_device_not_found,
        )) = runtime
        else {
            return;
        };

        let (requires_system_audio, capture_health) = {
            let state = self.state.lock();
            (Self::requires_system_audio(&state), state.capture_health)
        };

        let now = Instant::now();
        if requires_system_audio {
            if system_audio_path_ready {
                let recovered = self.system_audio_not_ready_since.lock().take().is_some();
                *self.last_system_audio_ready_at.lock() = Some(now);
                *self.system_audio_hard_unavailable_logged.lock() = false;
                if recovered {
                    self.append_capture_runtime_marker("phase: system_audio_ready_recovered");
                }
            } else {
                let mut not_ready_since = self.system_audio_not_ready_since.lock();
                if not_ready_since.is_none() {
                    *not_ready_since = Some(now);
                    *self.system_audio_hard_unavailable_logged.lock() = false;
                    self.append_capture_runtime_marker("phase: system_audio_not_ready_start");
                }
                let elapsed = now.saturating_duration_since((*not_ready_since).unwrap_or(now));
                if elapsed >= Duration::from_millis(SYSTEM_AUDIO_HARD_FAIL_AFTER_MS) {
                    let mut hard_logged = self.system_audio_hard_unavailable_logged.lock();
                    if !*hard_logged
                        && !matches!(
                            capture_health,
                            CaptureHealthDto::Starting
                                | CaptureHealthDto::Restarting
                                | CaptureHealthDto::Stopped
                        )
                    {
                        *hard_logged = true;
                        self.append_capture_runtime_marker(&format!(
                            "phase: system_audio_hard_unavailable elapsed_ms={}",
                            elapsed.as_millis()
                        ));
                    }
                }
            }
        } else {
            self.reset_system_audio_readiness_tracking();
        }

        if queue_starvation_detected && live_queue_profile == LiveQueueProfile::Small.as_str() {
            let mut next_profile = self.next_queue_profile.lock();
            if *next_profile != LiveQueueProfile::Elevated {
                *next_profile = LiveQueueProfile::Elevated;
                self.append_capture_runtime_marker(
                    "phase: queue_profile_escalated reason=thread_queue_blocking",
                );
            }
        }

        let marker_count = {
            let mut state = self.state.lock();
            let previous_concurrent_session_count = state.concurrent_session_count;
            state.capture_speed_x = capture_speed_x;
            state.save_ready = save_ready;
            state.system_audio_path_ready = system_audio_path_ready;
            state.mic_path_ready = mic_path_ready;
            state.mic_frames_seen = mic_frames_seen;
            state.mic_level_dbfs = mic_level_dbfs;
            state.mic_capture_session_running = mic_capture_session_running;
            state.mic_samples_per_sec = mic_samples_per_sec;
            state.mic_attach_state = derive_mic_attach_state(
                &state.settings,
                &state.active_audio_mode,
                mic_path_ready,
                mic_frames_seen,
                mic_attach_runtime_state,
                state.mic_attach_state,
            );
            state.audio_path_ready = audio_path_ready;
            state.first_audio_frame_seen = first_audio_frame_seen;
            state.live_queue_profile = live_queue_profile.to_string();
            state.capture_dropped_frames = capture_dropped_frames;
            state.capture_queue_overflows = capture_queue_overflows;
            state.effective_output_fps = effective_output_fps;
            state.concurrent_session_count = concurrent_session_count;
            state.capture_owner_pid = capture_owner_pid;
            state.capture_load_state = derive_capture_load_state(
                capture_speed_x,
                playback_realtime_x,
                queue_starvation_detected,
                effective_output_fps,
                &state.capture_load_state,
                state.effective_video_resolution,
                state.requested_video_resolution,
                state.effective_fps,
                state.requested_fps,
            );
            concurrent_session_count
                .filter(|count| *count > 1 && previous_concurrent_session_count != Some(*count))
        };
        if let Some(count) = marker_count {
            self.append_capture_runtime_marker(&format!(
                "phase: concurrent_sessions_detected count={count}"
            ));
        }

        if mic_selected_device_not_found {
            let mut warned = self.mic_device_not_found_warned.lock();
            if !*warned {
                *warned = true;
                self.append_capture_runtime_marker("phase: mic_selected_device_not_found_fallback");
            }
        }
    }
}
