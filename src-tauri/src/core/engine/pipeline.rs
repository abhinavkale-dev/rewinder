use super::*;

impl Engine {
    pub(super) fn ensure_pipeline_for_state(&self) -> Result<(), String> {
        if self.is_capture_paused_by_user() {
            self.stop_pipeline_if_running();
            let mut state = self.state.lock();
            state.capture_health = CaptureHealthDto::Stopped;
            state.audio_health = AudioHealthDto::Unavailable;
            state.save_ready = false;
            state.system_audio_path_ready = false;
            state.mic_path_ready = false;
            state.audio_path_ready = false;
            state.first_audio_frame_seen = false;
            state.save_stage = SaveStageDto::Idle;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.capture_speed_x = None;
            state.capture_load_state = "normal".to_string();
            return Ok(());
        }

        let should_run = {
            let state = self.state.lock();
            state.lifecycle_state == LifecycleState::Armed
                || state.lifecycle_state == LifecycleState::SavingReplay
        };

        if should_run {
            self.start_pipeline_if_needed(AudioStartupStrategy::SystemFirst)
        } else {
            self.stop_pipeline_if_running();
            self.pending_smooth_jobs.lock().clear();
            *self.runtime_profile_index.lock() = 0;
            *self.overload_since.lock() = None;
            *self.recover_since.lock() = None;
            *self.last_profile_change_at.lock() = None;
            *self.last_mic_retry_at.lock() = None;
            *self.next_queue_profile.lock() = LiveQueueProfile::Small;
            *self.startup_bootstrap_until.lock() = None;
            *self.startup_bootstrap_pending.lock() = false;
            self.reset_startup_interrupt_retry_state();
            self.reset_system_audio_readiness_tracking();
            let mut state = self.state.lock();
            state.capture_health = CaptureHealthDto::Stopped;
            state.audio_health = AudioHealthDto::Unavailable;
            state.active_audio_mode = state.settings.audio_mode.clone();
            state.effective_audio_mode = state.settings.audio_mode.clone();
            state.audio_fallback_policy = state.settings.audio_fallback_policy.clone();
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
            state.mic_recovery_state = "ok".to_string();
            state.selected_microphone_name = None;
            state.last_mic_error_code = None;
            state.last_mic_error_message = None;
            state.concurrent_session_count = None;
            state.capture_owner_pid = None;
            state.audio_path_ready = false;
            state.first_audio_frame_seen = false;
            state.save_stage = SaveStageDto::Idle;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.capture_dropped_frames = 0;
            state.capture_queue_overflows = 0;
            state.effective_output_fps = None;
            state.mic_backend_in_use = state.settings.mic_capture_backend.clone();
            state.mic_mix_gain_db = state.settings.mic_mix_gain_db;
            state.requested_video_resolution = state.settings.video_resolution;
            state.requested_fps = state.settings.fps;
            state.requested_video_bitrate_kbps = state.settings.video_bitrate_kbps;
            state.effective_video_resolution = state.settings.video_resolution;
            state.effective_fps = state.settings.fps;
            state.effective_video_bitrate_kbps = state.settings.video_bitrate_kbps;
            state.degrade_reason = None;
            state.audio_degrade_reason = None;
            state.last_audio_mode_error = None;
            Ok(())
        }
    }

    pub(super) fn start_pipeline_if_needed(
        &self,
        startup_strategy: AudioStartupStrategy,
    ) -> Result<(), String> {
        let _pipeline_transition = self.pipeline_transition.lock();
        if self.pipeline.lock().is_some() {
            return Ok(());
        }
        let output_dir = self.state.lock().settings.output_dir_path();
        let output_dir_permission_recovered;
        if let Err(err) = permissions::ensure_output_dir_access(output_dir.as_path()) {
            let message = format!("output_dir_permission_required: {err}");
            self.stop_pipeline_if_running();
            self.pending_smooth_jobs.lock().clear();
            *self.runtime_profile_index.lock() = 0;
            *self.overload_since.lock() = None;
            *self.recover_since.lock() = None;
            *self.last_profile_change_at.lock() = None;
            *self.last_mic_retry_at.lock() = None;
            *self.next_queue_profile.lock() = LiveQueueProfile::Small;
            *self.startup_bootstrap_until.lock() = None;
            *self.startup_bootstrap_pending.lock() = false;
            self.reset_startup_interrupt_retry_state();
            self.reset_system_audio_readiness_tracking();
            self.append_capture_runtime_marker(&format!(
                "phase: output_dir_permission_denied path={}",
                output_dir.display()
            ));
            {
                let mut state = self.state.lock();
                state.permission.output_dir_writable = false;
                state.permission.output_dir_permission_error = Some(err.clone());
                if state.permission.screen_recording_granted {
                    state.permission.reason = Some(err.clone());
                }
                state.lifecycle_state =
                    lifecycle::idle_state(&state.permission, state.settings.replay_enabled);
                state.capture_health = CaptureHealthDto::Stopped;
                state.audio_health = AudioHealthDto::Unavailable;
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
                state.mic_recovery_state = "ok".to_string();
                state.selected_microphone_name = None;
                state.last_mic_error_code = None;
                state.last_mic_error_message = None;
                state.concurrent_session_count = None;
                state.capture_owner_pid = None;
                state.audio_path_ready = false;
                state.first_audio_frame_seen = false;
                state.save_stage = SaveStageDto::Idle;
                state.video_smooth_state = VideoSmoothStateDto::Idle;
                state.capture_dropped_frames = 0;
                state.capture_queue_overflows = 0;
                state.effective_output_fps = None;
                state.active_audio_mode = state.settings.audio_mode.clone();
                state.effective_audio_mode = state.settings.audio_mode.clone();
                state.last_error = Some(message.clone());
                state.degrade_reason =
                    Some("Capture paused until output-folder permission is granted.".to_string());
            }
            return Err(message);
        } else {
            let mut state = self.state.lock();
            output_dir_permission_recovered = !state.permission.output_dir_writable;
            state.permission.output_dir_writable = true;
            state.permission.output_dir_permission_error = None;
            if state.permission.screen_recording_granted {
                state.permission.reason = None;
            }
            state.lifecycle_state =
                lifecycle::idle_state(&state.permission, state.settings.replay_enabled);
        }
        self.reset_system_audio_readiness_tracking();
        self.reset_resource_pressure_tracking();
        let settings_for_bootstrap = self.state.lock().settings.clone();
        let replay_duration_secs = settings_for_bootstrap.replay_duration_secs;
        let battery_floor = self.battery_floor_now();
        let (profile_index, apply_startup_bootstrap) = {
            let mut profile_index_guard = self.runtime_profile_index.lock();
            let mut selected = *profile_index_guard;
            let apply = should_apply_startup_bootstrap(&settings_for_bootstrap, selected);
            if apply {
                selected = STARTUP_BOOTSTRAP_PROFILE_INDEX.min(MAX_RUNTIME_PROFILE_INDEX);
                *profile_index_guard = selected;
                *self.startup_bootstrap_until.lock() =
                    Some(Instant::now() + Duration::from_secs(STARTUP_BOOTSTRAP_SECS));
                *self.startup_bootstrap_pending.lock() = true;
            }
            let floor_binding = battery_floor > selected;
            if floor_binding {
                selected = battery_floor.min(MAX_RUNTIME_PROFILE_INDEX);
                *profile_index_guard = selected;
            }
            {
                let mut engaged = self.battery_floor_engaged.lock();
                if floor_binding {
                    *engaged = true;
                } else if battery_floor < selected {
                    *engaged = false;
                }
            }
            (selected, apply)
        };
        let queue_profile_for_start = {
            let mut next = self.next_queue_profile.lock();
            let selected = *next;
            *next = LiveQueueProfile::Small;
            selected
        };
        let (
            requested_audio_mode,
            requested_profile,
            effective_profile,
            capture_settings,
            audio_fallback_policy,
            mic_failure_policy,
            requested_mic_enabled,
            mic_auto_request_permission,
        ) = {
            let mut state = self.state.lock();
            state.capture_health = CaptureHealthDto::Starting;
            state.capture_crash_loop = false;
            state.capture_speed_x = None;
            state.capture_load_state = if apply_startup_bootstrap {
                "recovering".to_string()
            } else {
                "normal".to_string()
            };
            state.guard_state = "monitoring".to_string();
            state.guard_primary_reason_code = None;
            state.guard_contributing_reason_codes.clear();
            state.guard_suppressed_reason_code = None;
            state.live_queue_profile = queue_profile_for_start.as_str().to_string();
            state.save_ready = false;
            state.system_audio_path_ready = false;
            state.mic_path_ready = false;
            state.mic_frames_seen = false;
            state.mic_level_dbfs = None;
            state.mic_capture_session_running = false;
            state.mic_samples_per_sec = None;
            state.mic_attach_state = MicAttachStateDto::Inactive;
            state.mic_recovery_state = "ok".to_string();
            state.selected_microphone_name = None;
            state.last_mic_error_code = None;
            state.last_mic_error_message = None;
            state.concurrent_session_count = None;
            state.capture_owner_pid = None;
            state.audio_path_ready = false;
            state.first_audio_frame_seen = false;
            state.save_stage = SaveStageDto::Idle;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.capture_dropped_frames = 0;
            state.capture_queue_overflows = 0;
            state.effective_output_fps = None;
            let requested = state.settings.clone();
            let mut capture_settings = requested.clone();
            let requested_profile = profile_from_settings(&requested);
            let effective_profile = effective_profile_for_index(&requested, profile_index);
            capture_settings.video_resolution = effective_profile.video_resolution;
            capture_settings.fps = effective_profile.fps;
            capture_settings.video_bitrate_kbps = effective_profile.video_bitrate_kbps;
            state.requested_video_resolution = requested.video_resolution;
            state.requested_fps = requested.fps;
            state.requested_video_bitrate_kbps = requested.video_bitrate_kbps;
            state.audio_fallback_policy = requested.audio_fallback_policy.clone();
            state.mic_backend_in_use = requested.mic_capture_backend.clone();
            state.mic_mix_gain_db = requested.mic_mix_gain_db;
            (
                requested.audio_mode.clone(),
                requested_profile,
                effective_profile,
                capture_settings,
                requested.audio_fallback_policy,
                requested.mic_failure_policy,
                requested.mic_enabled,
                requested.mic_auto_request_permission,
            )
        };
        let mut mic_permission_error = None;
        let mut mic_permission_status = None;
        if capture_settings.mic_enabled && capture_settings.audio_mode == "system_plus_mic" {
            let probe = permissions::probe_microphone_permission(mic_auto_request_permission);
            mic_permission_status = Some(probe.status);
            mic_permission_error = probe.error.clone();
            {
                let mut state = self.state.lock();
                state.mic_permission_status = probe.status.as_str().to_string();
                state.mic_permission_error = probe.error.clone();
            }
            if probe.status != MicrophonePermissionStatus::Granted {
                let err = probe.error.clone().unwrap_or_else(|| {
                    "Microphone permission unavailable for capture.".to_string()
                });
                if mic_failure_policy != "best_effort" {
                    return Err(format!("mic_required_unavailable: {err}"));
                }
            }
        }
        let capture =
            CaptureEngine::start(capture_settings, queue_profile_for_start, startup_strategy)?;
        let active_audio_mode = capture.active_audio_mode().to_string();
        let effective_audio_mode = active_audio_mode.clone();
        let mic_backend_in_use = capture.mic_backend_in_use().to_string();
        let audio_degraded = requested_audio_mode != active_audio_mode;
        let profile_degraded = effective_profile != requested_profile;
        let capture_speed_x = capture.capture_speed_x();
        let playback_realtime_x = capture.playback_realtime_x();
        let save_ready = capture.save_ready();
        let system_audio_path_ready = capture.system_audio_path_ready();
        let mic_path_ready = capture.mic_path_ready();
        let mic_frames_seen = capture.mic_frames_seen();
        let mic_level_dbfs = capture.mic_level_dbfs();
        let mic_capture_session_running = capture.mic_capture_session_running();
        let mic_samples_per_sec = capture.mic_samples_per_sec();
        let mic_attach_runtime_state = capture.mic_attach_runtime_state();
        let runtime_mic_recovery_state = capture.mic_recovery_state();
        let selected_microphone_name = capture.selected_microphone_name();
        let last_mic_backend_error = capture.last_mic_backend_error();
        let startup_fallback_error = capture.startup_fallback_error();
        let audio_path_ready = system_audio_path_ready;
        let first_audio_frame_seen = capture.first_audio_frame_seen();
        let queue_profile = capture.live_queue_profile().as_str().to_string();
        let queue_starvation_detected = capture.queue_starvation_detected();
        let capture_dropped_frames = capture.capture_dropped_frames();
        let capture_queue_overflows = capture.capture_queue_overflows();
        let effective_output_fps = capture.effective_output_fps();
        let concurrent_session_count = capture.concurrent_session_count(replay_duration_secs);
        let capture_owner_pid = capture.capture_owner_pid();
        if apply_startup_bootstrap {
            capture.append_runtime_marker(&format!(
                "phase: startup_bootstrap_profile_applied from={} to={}",
                requested_profile.label(),
                effective_profile.label()
            ));
        }

        *self.pipeline.lock() = Some(PipelineHandles { capture });
        *self.last_drop_total.lock() = capture_dropped_frames;
        *self.last_overflow_total.lock() = capture_queue_overflows;
        if output_dir_permission_recovered {
            self.append_capture_runtime_marker("phase: output_dir_permission_recovered");
        }
        *self.no_segments_miss_count.lock() = 0;
        *self.last_pipeline_started_at.lock() = Some(Instant::now());
        *self.startup_requested_profile_hold_logged.lock() = false;
        {
            let mut state = self.state.lock();
            state.capture_health = CaptureHealthDto::Running;
            state.capture_speed_x = capture_speed_x;
            state.capture_load_state = derive_capture_load_state(
                capture_speed_x,
                playback_realtime_x,
                queue_starvation_detected,
                effective_output_fps,
                &state.capture_load_state,
                effective_profile.video_resolution,
                requested_profile.video_resolution,
                effective_profile.fps,
                requested_profile.fps,
            );
            state.live_queue_profile = queue_profile;
            state.save_ready = save_ready;
            state.system_audio_path_ready = system_audio_path_ready;
            state.mic_path_ready = mic_path_ready;
            state.mic_frames_seen = mic_frames_seen;
            state.mic_level_dbfs = mic_level_dbfs;
            state.mic_capture_session_running = mic_capture_session_running;
            state.mic_samples_per_sec = mic_samples_per_sec;
            state.mic_attach_state = derive_mic_attach_state(
                &state.settings,
                &active_audio_mode,
                mic_path_ready,
                mic_frames_seen,
                mic_attach_runtime_state,
                state.mic_attach_state,
            );
            state.mic_recovery_state = derive_mic_recovery_state(
                &state.settings,
                &active_audio_mode,
                mic_path_ready,
                runtime_mic_recovery_state.as_deref(),
            );
            state.selected_microphone_name = selected_microphone_name;
            state.last_mic_error_code = last_mic_backend_error
                .as_ref()
                .map(|(code, _)| code.clone());
            state.last_mic_error_message = last_mic_backend_error
                .as_ref()
                .map(|(_, message)| message.clone());
            state.audio_path_ready = audio_path_ready;
            state.first_audio_frame_seen = first_audio_frame_seen;
            state.capture_dropped_frames = capture_dropped_frames;
            state.capture_queue_overflows = capture_queue_overflows;
            state.effective_output_fps = effective_output_fps;
            state.concurrent_session_count = concurrent_session_count;
            state.capture_owner_pid = capture_owner_pid;
            state.active_audio_mode = active_audio_mode.clone();
            state.effective_audio_mode = effective_audio_mode;
            state.mic_backend_in_use = mic_backend_in_use;
            state.last_audio_mode_error = startup_fallback_error
                .clone()
                .or(mic_permission_error.clone());
            if let Some(status) = mic_permission_status {
                state.mic_permission_status = status.as_str().to_string();
                state.mic_permission_error = mic_permission_error.clone();
            }
            state.effective_video_resolution = effective_profile.video_resolution;
            state.effective_fps = effective_profile.fps;
            state.effective_video_bitrate_kbps = effective_profile.video_bitrate_kbps;

            let required_system_audio_unavailable = audio_fallback_policy == "system_only_fallback"
                && requested_audio_mode != "video_only"
                && !system_audio_path_ready;
            let required_mic_unavailable = requested_audio_mode == "system_plus_mic"
                && requested_mic_enabled
                && mic_failure_policy == "required"
                && !mic_path_ready;
            let best_effort_mic_degraded = requested_audio_mode == "system_plus_mic"
                && requested_mic_enabled
                && mic_failure_policy == "best_effort"
                && !mic_path_ready;
            let fallback_system_only = requested_audio_mode == "system_plus_mic"
                && requested_mic_enabled
                && active_audio_mode == "system_only";

            state.audio_health = if required_system_audio_unavailable
                || required_mic_unavailable
                || (active_audio_mode == "video_only"
                    && audio_fallback_policy == "system_only_fallback")
            {
                AudioHealthDto::Unavailable
            } else if audio_degraded || best_effort_mic_degraded || fallback_system_only {
                AudioHealthDto::Degraded
            } else {
                AudioHealthDto::Ok
            };

            let mut audio_reason_parts: Vec<String> = Vec::new();
            if audio_degraded {
                audio_reason_parts.push(format!(
                    "audio mode degraded from {} to {}",
                    requested_audio_mode, active_audio_mode
                ));
            }
            if best_effort_mic_degraded {
                audio_reason_parts
                    .push("microphone unavailable; continuing replay and retrying mic".to_string());
            }
            if fallback_system_only {
                audio_reason_parts.push(
                    if startup_fallback_error
                        .as_deref()
                        .map(|message| {
                            message.contains("mic_pipe_startup_stalled")
                                || message.contains("mic_first_frame_startup_stalled")
                        })
                        .unwrap_or(false)
                    {
                        "mixed microphone startup stalled; continuing with system audio only"
                            .to_string()
                    } else {
                        "microphone unavailable; continuing with system audio only".to_string()
                    },
                );
            }
            if required_mic_unavailable {
                audio_reason_parts.push("required microphone path unavailable".to_string());
            }
            if required_system_audio_unavailable {
                audio_reason_parts.push("required system-audio path unavailable".to_string());
            }
            state.audio_degrade_reason = if audio_reason_parts.is_empty() {
                None
            } else {
                Some(audio_reason_parts.join("; "))
            };

            if profile_degraded {
                state.degrade_reason = Some(format!(
                    "Runtime fallback active ({}) to keep capture realtime.",
                    effective_profile.label()
                ));
            } else {
                state.degrade_reason = None;
            }
            if audio_degraded {
                state.last_error = Some(
                    if startup_fallback_error
                        .as_deref()
                        .map(|message| {
                            message.contains("mic_pipe_startup_stalled")
                                || message.contains("mic_first_frame_startup_stalled")
                        })
                        .unwrap_or(false)
                    {
                        "Microphone startup stalled; continuing with system audio only.".to_string()
                    } else {
                        format!(
                            "Capture degraded from {} to {} for stability.",
                            requested_audio_mode, active_audio_mode
                        )
                    },
                );
                let audio_reason = format!("audio degraded to {active_audio_mode}");
                state.degrade_reason = Some(match state.degrade_reason.clone() {
                    Some(existing) => format!("{existing}; {audio_reason}"),
                    None => audio_reason,
                });
                if audio_fallback_policy == "system_only_fallback"
                    && active_audio_mode == "video_only"
                {
                    state.last_audio_mode_error = Some(
                        "Audio fallback policy requires audio, but capture reached video-only."
                            .to_string(),
                    );
                }
            } else if let Some((_, message)) = last_mic_backend_error {
                state.last_error = Some(message);
            } else if let Some(reason) = state.audio_degrade_reason.clone() {
                state.degrade_reason = Some(match state.degrade_reason.clone() {
                    Some(existing) => format!("{existing}; {reason}"),
                    None => reason,
                });
            } else if state
                .last_error
                .as_deref()
                .map(|e| {
                    is_transient_capture_error(e) || e.contains("Capture pipeline missing")
                })
                .unwrap_or(false)
            {
                state.last_error = None;
            }
        }
        Ok(())
    }
}
