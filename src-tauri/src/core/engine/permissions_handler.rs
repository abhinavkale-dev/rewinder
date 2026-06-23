use super::*;

impl Engine {
    pub fn recheck_permissions(&self, app: &Arc<dyn EngineHost>) -> PermissionStateDto {
        let _runtime_mutation_gate = self.runtime_mutation_gate.lock();
        let output_dir = self.state.lock().settings.output_dir_path();
        let permission = permissions::detect_permissions_for_output_dir(output_dir.as_path());
        let mic_probe = permissions::probe_microphone_permission(false);
        let mic_status = mic_probe.status.as_str().to_string();
        let mic_error = mic_probe.error.clone();
        {
            let mut state = self.state.lock();
            state.permission = permission.clone();
            state.mic_permission_status = mic_status.clone();
            state.mic_permission_error = mic_error.clone();
            state.lifecycle_state =
                lifecycle::idle_state(&state.permission, state.settings.replay_enabled);
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
            state.degrade_reason = None;
            state.audio_degrade_reason = None;
            state.last_audio_mode_error = None;
        }
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
        self.reset_mic_signal_observer();
        self.pending_smooth_jobs.lock().clear();

        self.stop_pipeline_if_running();
        if let Err(err) = self.ensure_pipeline_for_state() {
            let (code, action) = classify_capture_failure(&err);
            if code == "capture_owner_exists" {
                thread::sleep(Duration::from_millis(350));
                if let Err(retry_err) = self.ensure_pipeline_for_state() {
                    let (retry_code, retry_action) = classify_capture_failure(&retry_err);
                    {
                        let mut state = self.state.lock();
                        state.last_error = Some(retry_err.clone());
                        state.capture_health = CaptureHealthDto::Degraded;
                        state.audio_health = AudioHealthDto::Degraded;
                    }
                    self.pause_for_capture_owner_conflict(app, retry_code);
                    events::emit_save_failed_code(app, retry_code, retry_err, retry_action);
                }
            } else {
                {
                    let mut state = self.state.lock();
                    state.last_error = Some(err.clone());
                    state.capture_health = CaptureHealthDto::Degraded;
                    state.audio_health = AudioHealthDto::Degraded;
                }
                self.pause_for_capture_owner_conflict(app, code);
                events::emit_save_failed_code(app, code, err, action);
            }
        }

        events::emit_mic_permission_changed(app, mic_status, mic_error);
        let snapshot = self.get_engine_state();
        if snapshot.audio_path_ready {
            events::emit_audio_path_ready(app, snapshot.active_audio_mode.clone());
        }
        if snapshot.lifecycle_state == LifecycleState::PermissionRequired {
            events::emit_permission_required(
                app,
                snapshot
                    .permission
                    .reason
                    .clone()
                    .unwrap_or_else(|| "Permission required".to_string()),
            );
        }
        events::emit_engine_state(app, &snapshot);
        permission
    }

    pub fn request_microphone_permission(&self, app: &Arc<dyn EngineHost>) -> PermissionStateDto {
        let mic_probe = permissions::probe_microphone_permission(true);
        let output_dir = self.state.lock().settings.output_dir_path();
        let permission = permissions::detect_permissions_for_output_dir(output_dir.as_path());
        let mic_status = mic_probe.status.as_str().to_string();
        let mic_error = mic_probe.error.clone();

        {
            let mut state = self.state.lock();
            state.permission = permission.clone();
            state.mic_permission_status = mic_status.clone();
            state.mic_permission_error = mic_error.clone();
            if mic_probe.status == MicrophonePermissionStatus::Granted
                && state
                    .last_audio_mode_error
                    .as_deref()
                    .map(|reason| reason.to_ascii_lowercase().contains("microphone"))
                    .unwrap_or(false)
            {
                state.last_audio_mode_error = None;
            }
            state.lifecycle_state =
                lifecycle::idle_state(&state.permission, state.settings.replay_enabled);
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
            state.degrade_reason = None;
            state.audio_degrade_reason = None;
        }

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
        self.reset_mic_signal_observer();
        self.pending_smooth_jobs.lock().clear();

        self.stop_pipeline_if_running();
        if let Err(err) = self.ensure_pipeline_for_state() {
            let (code, action) = classify_capture_failure(&err);
            if code == "capture_owner_exists" {
                thread::sleep(Duration::from_millis(350));
                if let Err(retry_err) = self.ensure_pipeline_for_state() {
                    let (retry_code, retry_action) = classify_capture_failure(&retry_err);
                    {
                        let mut state = self.state.lock();
                        state.last_error = Some(retry_err.clone());
                        state.capture_health = CaptureHealthDto::Degraded;
                        state.audio_health = AudioHealthDto::Degraded;
                    }
                    self.pause_for_capture_owner_conflict(app, retry_code);
                    events::emit_save_failed_code(app, retry_code, retry_err, retry_action);
                }
            } else {
                {
                    let mut state = self.state.lock();
                    state.last_error = Some(err.clone());
                    state.capture_health = CaptureHealthDto::Degraded;
                    state.audio_health = AudioHealthDto::Degraded;
                }
                self.pause_for_capture_owner_conflict(app, code);
                events::emit_save_failed_code(app, code, err, action);
            }
        }

        events::emit_mic_permission_changed(app, mic_status.clone(), mic_error.clone());
        if mic_probe.status != MicrophonePermissionStatus::Granted {
            events::emit_mic_path_degraded(
                app,
                "mic_permission_denied",
                "Microphone permission denied; continuing with system audio.",
                Some(
                    "Enable Rewinder in System Settings > Privacy & Security > Microphone."
                        .to_string(),
                ),
            );
        }

        let snapshot = self.get_engine_state();
        events::emit_engine_state(app, &snapshot);
        permission
    }

    pub fn grant_output_dir_access(&self, app: &Arc<dyn EngineHost>) -> GrantOutputDirAccessResultDto {
        self.append_capture_runtime_marker("phase: output_dir_permission_assist_requested");
        let output_dir = self.state.lock().settings.output_dir_path();
        let access_probe = permissions::probe_output_dir_access(output_dir.as_path());
        let mut opened_settings = false;
        let mut message = if access_probe.writable {
            format!("Downloads access confirmed for {}.", output_dir.display())
        } else {
            access_probe.error.clone().unwrap_or_else(|| {
                format!(
                    "Downloads folder access is denied for Rewinder ({}).",
                    output_dir.display()
                )
            })
        };

        if !access_probe.writable {
            opened_settings = permissions::open_downloads_permission_settings();
            if opened_settings {
                self.append_capture_runtime_marker("phase: output_dir_settings_opened");
                message = "Downloads access is still denied. Opened System Settings > Privacy & Security > Files and Folders."
                    .to_string();
            } else {
                message = "Downloads access is denied. Open System Settings > Privacy & Security > Files and Folders and enable Downloads for Rewinder (and Terminal in dev mode)."
                    .to_string();
            }
        }

        let permission = self.recheck_permissions(app);
        if permission.output_dir_writable {
            message = "Access granted, capture resumed.".to_string();
            opened_settings = false;
        }

        GrantOutputDirAccessResultDto {
            permission,
            opened_settings,
            message,
        }
    }

    pub fn grant_screen_recording_access(
        &self,
        app: &Arc<dyn EngineHost>,
    ) -> GrantScreenRecordingAccessResultDto {
        self.append_capture_runtime_marker("phase: screen_permission_assist_requested");
        let mut opened_settings = false;
        let mut message;
        let granted = permissions::probe_screen_recording_permission(true);
        if granted {
            message = "Screen Recording access confirmed. Recovering capture.".to_string();
        } else {
            opened_settings = permissions::open_screen_recording_permission_settings();
            if opened_settings {
                self.append_capture_runtime_marker("phase: screen_permission_settings_opened");
                message = "Screen Recording access is still not granted. Opened System Settings > Privacy & Security > Screen Recording."
                    .to_string();
            } else {
                message = "Screen Recording access is denied. Open System Settings > Privacy & Security > Screen Recording and enable Rewinder (and Terminal in dev mode)."
                    .to_string();
            }
        }

        let permission = self.recheck_permissions(app);
        if permission.screen_recording_granted {
            message = "Screen Recording access granted, capture resumed.".to_string();
            opened_settings = false;
        }

        GrantScreenRecordingAccessResultDto {
            permission,
            opened_settings,
            message,
        }
    }

    pub fn grant_microphone_access(
        &self,
        app: &Arc<dyn EngineHost>,
        open_settings_if_denied: bool,
    ) -> GrantMicrophoneAccessResultDto {
        self.append_capture_runtime_marker("phase: mic_permission_assist_requested");
        let mic_probe = permissions::probe_microphone_permission(open_settings_if_denied);
        let mut opened_settings = false;
        let mut message = match mic_probe.status {
            MicrophonePermissionStatus::Granted => {
                "Microphone access confirmed. Recovering microphone capture.".to_string()
            }
            MicrophonePermissionStatus::Denied => {
                "Microphone access is denied for Rewinder.".to_string()
            }
            MicrophonePermissionStatus::Restricted => {
                "Microphone access is restricted by system policy.".to_string()
            }
            MicrophonePermissionStatus::NotDetermined => {
                "Microphone access is pending. Approve permission when prompted.".to_string()
            }
            MicrophonePermissionStatus::Unknown => mic_probe.error.clone().unwrap_or_else(|| {
                "Microphone permission state could not be determined.".to_string()
            }),
        };

        if mic_probe.status != MicrophonePermissionStatus::Granted && open_settings_if_denied {
            opened_settings = permissions::open_microphone_permission_settings();
            if opened_settings {
                self.append_capture_runtime_marker("phase: mic_permission_settings_opened");
                message =
                    "Microphone access is still not granted. Opened System Settings > Privacy & Security > Microphone."
                        .to_string();
            } else if mic_probe.status == MicrophonePermissionStatus::Denied
                || mic_probe.status == MicrophonePermissionStatus::Restricted
            {
                message = "Microphone access is denied. Open System Settings > Privacy & Security > Microphone and enable Rewinder (and Terminal in dev mode)."
                    .to_string();
            }
        }

        let (permission, mic_status, mic_error) =
            if mic_probe.status == MicrophonePermissionStatus::Granted {
                let permission = self.recheck_permissions(app);
                let snapshot = self.get_engine_state();
                (
                    permission,
                    snapshot.mic_permission_status.clone(),
                    snapshot.mic_permission_error.clone(),
                )
            } else {
                let mut permission = self.state.lock().permission.clone();
                if permission.reason.is_none() {
                    permission.reason = mic_probe.error.clone();
                }
                {
                    let mut state = self.state.lock();
                    state.mic_permission_status = mic_probe.status.as_str().to_string();
                    state.mic_permission_error = mic_probe.error.clone();
                    if state.last_audio_mode_error.is_none() {
                        state.last_audio_mode_error = mic_probe.error.clone();
                    }
                }
                events::emit_mic_permission_changed(
                    app,
                    mic_probe.status.as_str().to_string(),
                    mic_probe.error.clone(),
                );
                (
                    permission,
                    mic_probe.status.as_str().to_string(),
                    mic_probe.error,
                )
            };

        if mic_status == MicrophonePermissionStatus::Granted.as_str() {
            opened_settings = false;
            message = "Microphone access granted, capture resumed.".to_string();
            events::emit_mic_path_recovered(app, "Microphone path recovered.");
        } else {
            events::emit_mic_path_degraded(
                app,
                "mic_permission_denied",
                "Microphone permission denied; continuing with system audio.",
                Some(
                    "Enable Rewinder in System Settings > Privacy & Security > Microphone."
                        .to_string(),
                ),
            );
        }
        events::emit_engine_state(app, &self.get_engine_state());
        GrantMicrophoneAccessResultDto {
            permission,
            mic_permission_status: mic_status,
            mic_permission_error: mic_error,
            opened_settings,
            message,
        }
    }
}
