use super::*;

impl Engine {
    pub fn apply_runtime_patch(
        self: &Arc<Self>,
        app: &Arc<dyn EngineHost>,
        mut patch: SettingsPatchDto,
        source: &str,
    ) -> Result<EngineStateDto, String> {
        let _runtime_mutation_gate = self.runtime_mutation_gate.lock();
        let previous_settings = self.get_settings();
        normalize_patch_for_runtime(&previous_settings, &mut patch);
        let patch_for_message = patch.clone();

        let mut candidate = previous_settings.clone();
        candidate.apply_patch(patch)?;

        let mut capture_probe = candidate.clone();
        capture_probe.replay_duration_secs = previous_settings.replay_duration_secs;
        let settings_force_restart = capture_probe != previous_settings;

        let pipeline_present = self.pipeline.lock().is_some();
        let paused_by_user = self.is_capture_paused_by_user();

        let restart_needed;
        {
            let mut state = self.state.lock();
            if patch_for_message.replay_enabled.is_some() {
                self.clear_user_stop_disarm();
            }

            state.settings = candidate.clone();
            state.lifecycle_state =
                lifecycle::idle_state(&state.permission, candidate.replay_enabled);
            state.last_error = None;

            let should_run = matches!(
                state.lifecycle_state,
                LifecycleState::Armed | LifecycleState::SavingReplay
            ) && !paused_by_user;
            restart_needed = settings_force_restart || (should_run != pipeline_present);

            if restart_needed {
                state.capture_crash_loop = false;
                state.capture_health = if !candidate.replay_enabled {
                    CaptureHealthDto::Stopped
                } else if pipeline_present {
                    CaptureHealthDto::Restarting
                } else {
                    CaptureHealthDto::Starting
                };
                state.active_audio_mode = candidate.audio_mode.clone();
                state.effective_audio_mode = candidate.audio_mode.clone();
                state.audio_fallback_policy = candidate.audio_fallback_policy.clone();
                state.capture_speed_x = None;
                state.capture_load_state = "normal".to_string();
                state.guard_state = if candidate.replay_enabled {
                    "monitoring".to_string()
                } else {
                    "idle".to_string()
                };
                state.guard_primary_reason_code = None;
                state.guard_contributing_reason_codes.clear();
                state.guard_suppressed_reason_code = None;
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
                state.requested_video_resolution = candidate.video_resolution;
                state.requested_fps = candidate.fps;
                state.requested_video_bitrate_kbps = candidate.video_bitrate_kbps;
                state.effective_video_resolution = candidate.video_resolution;
                state.effective_fps = candidate.fps;
                state.effective_video_bitrate_kbps = candidate.video_bitrate_kbps;
                state.degrade_reason = None;
                state.audio_degrade_reason = None;
                state.mic_backend_in_use = candidate.mic_capture_backend.clone();
                state.mic_mix_gain_db = candidate.mic_mix_gain_db;
                state.last_audio_mode_error = None;
            }
        }

        if restart_needed {
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
            *self.mic_device_not_found_warned.lock() = false;
            self.pending_smooth_jobs.lock().clear();
        }

        if let Err(err) = self.register_hotkeys(app, source) {
            self.rollback_settings(&previous_settings);
            let _ = self.register_hotkeys(app, "rollback");
            let rollback_error = format!("{source}: hotkey update failed: {err}");
            {
                let mut state = self.state.lock();
                state.last_error = Some(rollback_error.clone());
                state.hotkey_status = HotkeyStatusDto::Conflict;
            }
            events::emit_save_failed_code(
                app,
                "hotkey_conflict",
                rollback_error.clone(),
                Some("Pick another shortcut or use tray Save Replay.".to_string()),
            );
            events::emit_engine_state(app, &self.get_engine_state());
            return Err(rollback_error);
        }

        if restart_needed {
            self.restart_pending.store(true, Ordering::Release);
            self.spawn_settings_restart_worker(app, &previous_settings);
        }

        let snapshot = self.get_engine_state();
        let message = build_settings_updated_message(source, &patch_for_message);
        events::emit_settings_updated(app, message);
        if snapshot.audio_path_ready {
            events::emit_audio_path_ready(app, snapshot.active_audio_mode.clone());
        }
        events::emit_engine_state(app, &snapshot);
        Ok(snapshot)
    }

    pub fn update_settings(
        self: &Arc<Self>,
        app: &Arc<dyn EngineHost>,
        patch: SettingsPatchDto,
    ) -> Result<SettingsDto, String> {
        let snapshot = self.apply_runtime_patch(app, patch, "settings")?;
        Ok(snapshot.settings)
    }

    pub fn set_replay_enabled(
        self: &Arc<Self>,
        app: &Arc<dyn EngineHost>,
        enabled: bool,
    ) -> Result<EngineStateDto, String> {
        self.apply_runtime_patch(
            app,
            SettingsPatchDto {
                replay_enabled: Some(enabled),
                ..SettingsPatchDto::default()
            },
            "replay toggle",
        )
    }

    pub fn resume_capture(&self, app: &Arc<dyn EngineHost>) -> Result<EngineStateDto, String> {
        if !self.is_capture_paused_by_user() {
            let snapshot = self.get_engine_state();
            events::emit_engine_state(app, &snapshot);
            return Ok(snapshot);
        }

        self.set_capture_paused_by_user(app, false, Some("Capture resumed by user.".to_string()));
        self.reset_startup_interrupt_retry_state();
        self.pending_smooth_jobs.lock().clear();

        {
            let mut state = self.state.lock();
            state.last_error = None;
            state.degrade_reason = None;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.lifecycle_state =
                lifecycle::idle_state(&state.permission, state.settings.replay_enabled);
            state.capture_health = if state.settings.replay_enabled {
                CaptureHealthDto::Starting
            } else {
                CaptureHealthDto::Stopped
            };
        }

        if let Err(err) = self.ensure_pipeline_for_state() {
            {
                let mut state = self.state.lock();
                state.last_error = Some(err.clone());
                state.capture_health = CaptureHealthDto::Degraded;
                state.audio_health = AudioHealthDto::Degraded;
            }
            let (code, action) = classify_capture_failure(&err);
            self.pause_for_capture_owner_conflict(app, code);
            events::emit_save_failed_code(app, code, err.clone(), action);
            events::emit_engine_state(app, &self.get_engine_state());
            return Err(err);
        }

        let snapshot = self.get_engine_state();
        events::emit_capture_resumed(app, "Capture resumed.");
        events::emit_capture_health_changed(app, "running", Some("Capture resumed.".to_string()));
        events::emit_engine_state(app, &snapshot);
        Ok(snapshot)
    }

    fn spawn_settings_restart_worker(
        self: &Arc<Self>,
        app: &Arc<dyn EngineHost>,
        previous_settings: &SettingsDto,
    ) {
        if self.restart_in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        *self.restart_rollback_target.lock() = Some(previous_settings.clone());

        let engine = Arc::clone(self);
        let app = Arc::clone(app);
        thread::spawn(move || loop {
            while engine.restart_pending.swap(false, Ordering::AcqRel) {
                engine.run_settings_restart_cycle(&app);
            }
            engine.restart_in_flight.store(false, Ordering::Release);
            if !engine.restart_pending.load(Ordering::Acquire) {
                break;
            }
            if engine.restart_in_flight.swap(true, Ordering::AcqRel) {
                break;
            }
        });
    }

    fn run_settings_restart_cycle(self: &Arc<Self>, app: &Arc<dyn EngineHost>) {
        self.stop_pipeline_if_running();
        if let Err(err) = self.ensure_pipeline_for_state() {
            if let Some(previous) = self.restart_rollback_target.lock().clone() {
                self.rollback_settings(&previous);
                let _ = self.register_hotkeys(app, "rollback");
                self.stop_pipeline_if_running();
            }
            let rollback_restart_err = self.ensure_pipeline_for_state().err();
            let full_err: String = match rollback_restart_err {
                Some(restart_err) => {
                    format!("settings: apply failed: {err}; rollback restart failed: {restart_err}")
                }
                None => format!("settings: apply failed: {err}"),
            };
            {
                let mut state = self.state.lock();
                state.last_error = Some(full_err.clone());
                state.capture_health = CaptureHealthDto::Degraded;
                state.audio_health = AudioHealthDto::Degraded;
            }
            let (code, action) = classify_capture_failure(full_err.as_str());
            self.pause_for_capture_owner_conflict(app, code);
            events::emit_save_failed_code(app, code, full_err.clone(), action);
        }
        events::emit_engine_state(app, &self.get_engine_state());
    }
}
