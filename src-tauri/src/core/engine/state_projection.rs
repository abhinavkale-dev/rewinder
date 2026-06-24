use super::*;
impl Engine {
    pub fn get_engine_state(&self) -> EngineStateDto {
        self.clear_expired_pending_save();
        let (
            pending_save,
            pending_full_window,
            pending_full_window_deadline_epoch_ms,
            full_window_wait_remaining_ms,
            warmup_eta_ms,
        ) = self.pending_save_snapshot();
        let (
            lifecycle_state,
            capture_health,
            audio_health,
            save_stage,
            state_system_audio_path_ready,
            state_mic_path_ready,
            state_mic_frames_seen,
            state_mic_level_dbfs,
            state_mic_permission_status,
            state_mic_permission_error,
            state_mic_capture_session_running,
            state_mic_samples_per_sec,
            state_mic_attach_state,
            state_mic_recovery_state,
            state_selected_microphone_name,
            state_last_mic_error_code,
            state_last_mic_error_message,
            state_audio_path_ready,
            state_first_audio_frame_seen,
            state_capture_speed_x,
            state_capture_load_state,
            state_guard_state,
            state_guard_primary_reason_code,
            state_guard_contributing_reason_codes,
            state_guard_suppressed_reason_code,
            state_guard_last_transition_at_epoch_ms,
            state_live_queue_profile,
            state_save_ready,
            hotkey_status,
            active_audio_mode,
            effective_audio_mode,
            capture_backend,
            mic_backend_in_use,
            mic_mix_gain_db,
            requested_video_resolution,
            requested_fps,
            requested_video_bitrate_kbps,
            effective_video_resolution,
            effective_fps,
            effective_video_bitrate_kbps,
            audio_fallback_policy,
            degrade_reason,
            audio_degrade_reason,
            last_audio_mode_error,
            capture_restart_count,
            capture_interrupt_count,
            state_video_smooth_state,
            state_capture_dropped_frames,
            state_capture_queue_overflows,
            state_effective_output_fps,
            state_concurrent_session_count,
            state_capture_owner_pid,
            state_system_memory_pressure_level,
            capture_crash_loop,
            is_armed,
            is_saving,
            arm_blocker,
            arm_blocker_code,
            arm_blocker_action,
            audio_warmup_grace_ms,
            state_last_error,
            dropped_video_packets,
            dropped_audio_packets,
            last_contiguity_break_code,
            permission,
            settings,
        ) = {
            let mut state = self.state.lock();
            let blocker = self.save_blocker_with_runtime(&state);
            let blocker_message = blocker.as_ref().map(|b| b.message.clone());
            let blocker_code = blocker.as_ref().map(|b| b.code.to_string());
            let blocker_action = blocker.as_ref().and_then(|b| b.action.clone());
            let audio_warmup_grace_ms = self.current_audio_warmup_grace_ms(&state);
            state.arm_blocker = blocker_message.clone();
            state.arm_blocker_code = blocker_code.clone();
            state.arm_blocker_action = blocker_action.clone();
            (
                state.lifecycle_state,
                state.capture_health,
                state.audio_health,
                state.save_stage,
                state.system_audio_path_ready,
                state.mic_path_ready,
                state.mic_frames_seen,
                state.mic_level_dbfs,
                state.mic_permission_status.clone(),
                state.mic_permission_error.clone(),
                state.mic_capture_session_running,
                state.mic_samples_per_sec,
                state.mic_attach_state,
                state.mic_recovery_state.clone(),
                state.selected_microphone_name.clone(),
                state.last_mic_error_code.clone(),
                state.last_mic_error_message.clone(),
                state.audio_path_ready,
                state.first_audio_frame_seen,
                state.capture_speed_x,
                state.capture_load_state.clone(),
                state.guard_state.clone(),
                state.guard_primary_reason_code.clone(),
                state.guard_contributing_reason_codes.clone(),
                state.guard_suppressed_reason_code.clone(),
                state.guard_last_transition_at_epoch_ms,
                state.live_queue_profile.clone(),
                state.save_ready,
                state.hotkey_status,
                state.active_audio_mode.clone(),
                state.effective_audio_mode.clone(),
                state.capture_backend.clone(),
                state.mic_backend_in_use.clone(),
                state.mic_mix_gain_db,
                state.requested_video_resolution,
                state.requested_fps,
                state.requested_video_bitrate_kbps,
                state.effective_video_resolution,
                state.effective_fps,
                state.effective_video_bitrate_kbps,
                state.audio_fallback_policy.clone(),
                state.degrade_reason.clone(),
                state.audio_degrade_reason.clone(),
                state.last_audio_mode_error.clone(),
                state.capture_restart_count,
                state.capture_interrupt_count,
                state.video_smooth_state,
                state.capture_dropped_frames,
                state.capture_queue_overflows,
                state.effective_output_fps,
                state.concurrent_session_count,
                state.capture_owner_pid,
                state.system_memory_pressure_level.clone(),
                state.capture_crash_loop,
                state.is_armed(),
                state.is_saving,
                blocker_message,
                blocker_code,
                blocker_action,
                audio_warmup_grace_ms,
                state.last_error.clone(),
                state.dropped_video_packets,
                state.dropped_audio_packets,
                state.last_contiguity_break_code.clone(),
                state.permission.clone(),
                state.settings.clone(),
            )
        };

        let (
            buffer_fill_secs,
            replay_fill_secs,
            rolling_fill_secs,
            capture_error,
            last_capture_log_tail,
            capture_speed_x,
            playback_realtime_x,
            playback_stability,
            save_ready,
            system_audio_path_ready,
            mic_path_ready,
            mic_frames_seen,
            mic_level_dbfs,
            mic_capture_session_running,
            mic_samples_per_sec,
            mic_attach_runtime_state,
            runtime_mic_recovery_state,
            runtime_selected_microphone_name,
            runtime_last_mic_error_code,
            runtime_last_mic_error_message,
            audio_path_ready,
            first_audio_frame_seen,
            live_queue_profile,
            queue_starvation_detected,
            capture_dropped_frames,
            capture_queue_overflows,
            effective_output_fps,
            system_memory_pressure_level,
            helper_thermal_state,
            concurrent_session_count,
            capture_owner_pid,
        ) = {
            let pipeline = self.pipeline.lock();
            if let Some(handles) = pipeline.as_ref() {
                let rolling_fill_secs = handles.capture.rolling_fill_secs();
                let replay_fill_secs = handles
                    .capture
                    .replay_fill_secs(settings.replay_duration_secs);
                let playback_realtime_x = handles.capture.playback_realtime_x();
                let playback_stability = handles.capture.playback_stability().to_string();
                (
                    rolling_fill_secs,
                    replay_fill_secs,
                    rolling_fill_secs,
                    handles.capture.last_error(),
                    handles.capture.capture_log_tail(12),
                    handles.capture.capture_speed_x(),
                    playback_realtime_x,
                    playback_stability,
                    handles.capture.save_ready(),
                    handles.capture.system_audio_path_ready(),
                    handles.capture.mic_path_ready(),
                    handles.capture.mic_frames_seen(),
                    handles.capture.mic_level_dbfs(),
                    handles.capture.mic_capture_session_running(),
                    handles.capture.mic_samples_per_sec(),
                    handles.capture.mic_attach_runtime_state(),
                    handles.capture.mic_recovery_state(),
                    handles.capture.selected_microphone_name(),
                    handles
                        .capture
                        .last_mic_backend_error()
                        .map(|(code, _)| code),
                    handles
                        .capture
                        .last_mic_backend_error()
                        .map(|(_, message)| message),
                    handles.capture.audio_path_ready(),
                    handles.capture.first_audio_frame_seen(),
                    handles.capture.live_queue_profile().as_str().to_string(),
                    handles.capture.queue_starvation_detected(),
                    handles.capture.capture_dropped_frames(),
                    handles.capture.capture_queue_overflows(),
                    handles.capture.effective_output_fps(),
                    handles.capture.system_memory_pressure_level(),
                    handles.capture.helper_thermal_state(),
                    handles
                        .capture
                        .concurrent_session_count(settings.replay_duration_secs),
                    handles.capture.capture_owner_pid(),
                )
            } else {
                (
                    0.0,
                    0.0,
                    0.0,
                    None,
                    None,
                    state_capture_speed_x,
                    None,
                    "recovering".to_string(),
                    state_save_ready,
                    state_system_audio_path_ready,
                    state_mic_path_ready,
                    state_mic_frames_seen,
                    state_mic_level_dbfs,
                    state_mic_capture_session_running,
                    state_mic_samples_per_sec,
                    None,
                    Some(state_mic_recovery_state.clone()),
                    state_selected_microphone_name.clone(),
                    state_last_mic_error_code.clone(),
                    state_last_mic_error_message.clone(),
                    state_audio_path_ready,
                    state_first_audio_frame_seen,
                    state_live_queue_profile.clone(),
                    false,
                    state_capture_dropped_frames,
                    state_capture_queue_overflows,
                    state_effective_output_fps,
                    state_system_memory_pressure_level.clone(),
                    None,
                    state_concurrent_session_count,
                    state_capture_owner_pid,
                )
            }
        };
        let capture_load_state = derive_capture_load_state(
            capture_speed_x,
            playback_realtime_x,
            queue_starvation_detected,
            effective_output_fps,
            &state_capture_load_state,
            effective_video_resolution,
            requested_video_resolution,
            effective_fps,
            requested_fps,
        );
        let capture_start_phase = if matches!(capture_health, CaptureHealthDto::Restarting) {
            None
        } else {
            detect_capture_start_phase(last_capture_log_tail.as_deref())
        };
        let (
            app_rss_mb,
            app_cpu_percent,
            capture_stack_rss_mb,
            capture_stack_cpu_percent,
            capture_stack_rss_delta_mb,
            sampled_thermal_state,
            power_source,
        ) = self.current_process_diagnostics();
        let thermal_state = helper_thermal_state.or(sampled_thermal_state);
        let current_profile_idx = *self.runtime_profile_index.lock();
        let guard_state =
            if !settings.replay_enabled || matches!(capture_health, CaptureHealthDto::Stopped) {
                "idle".to_string()
            } else if state_guard_state == "suppressed" {
                "suppressed".to_string()
            } else if current_profile_idx > 0 {
                "protecting".to_string()
            } else {
                "monitoring".to_string()
            };
        let last_error_for_health = state_last_error.as_deref().or(capture_error.as_deref());
        let (operator_health_state, operator_health_message) = derive_operator_health_state(
            lifecycle_state,
            capture_health,
            audio_health,
            arm_blocker_code.as_deref(),
            arm_blocker.as_deref(),
            &guard_state,
            save_ready,
            effective_video_resolution,
            requested_video_resolution,
            effective_fps,
            requested_fps,
            &playback_stability,
            runtime_mic_recovery_state
                .as_deref()
                .unwrap_or(state_mic_recovery_state.as_str()),
            last_error_for_health,
        );
        let mic_attach_state = derive_mic_attach_state(
            &settings,
            &active_audio_mode,
            mic_path_ready,
            mic_frames_seen,
            mic_attach_runtime_state,
            state_mic_attach_state,
        );
        let mic_recovery_state = derive_mic_recovery_state(
            &settings,
            &active_audio_mode,
            mic_path_ready,
            runtime_mic_recovery_state
                .as_deref()
                .or(Some(state_mic_recovery_state.as_str())),
        );
        let selected_microphone_name =
            runtime_selected_microphone_name.or(state_selected_microphone_name);
        let last_mic_error_code = runtime_last_mic_error_code.or(state_last_mic_error_code);
        let last_mic_error_message =
            runtime_last_mic_error_message.or(state_last_mic_error_message);
        let effective_save_stage = if is_saving {
            SaveStageDto::SavingFast
        } else if pending_save {
            SaveStageDto::Queued
        } else if matches!(save_stage, SaveStageDto::Queued | SaveStageDto::SavingFast) {
            SaveStageDto::Idle
        } else {
            save_stage
        };

        EngineStateDto {
            lifecycle_state,
            capture_health,
            audio_health,
            save_stage: effective_save_stage,
            system_audio_path_ready,
            system_audio_ready: system_audio_path_ready,
            mic_path_ready,
            mic_ready: mic_path_ready,
            mic_frames_seen,
            mic_level_dbfs,
            mic_permission_status: state_mic_permission_status,
            mic_permission_error: state_mic_permission_error,
            mic_capture_session_running,
            mic_samples_per_sec,
            mic_attach_state,
            mic_recovery_state,
            mic_signal_silent: *self.mic_signal_warning_emitted.lock(),
            selected_microphone_id: settings.selected_microphone_id.clone(),
            selected_microphone_name,
            last_mic_error_code,
            last_mic_error_message,
            audio_path_ready,
            first_audio_frame_seen,
            capture_speed_x,
            encoder_throughput_x: capture_speed_x,
            playback_realtime_x,
            playback_stability,
            capture_load_state,
            operator_health_state,
            operator_health_message,
            guard_state,
            guard_primary_reason_code: state_guard_primary_reason_code,
            guard_contributing_reason_codes: state_guard_contributing_reason_codes,
            guard_suppressed_reason_code: state_guard_suppressed_reason_code,
            guard_last_transition_at_epoch_ms: state_guard_last_transition_at_epoch_ms,
            live_queue_profile,
            save_ready,
            hotkey_status,
            active_audio_mode,
            effective_audio_mode,
            capture_backend,
            mic_backend_in_use,
            mic_mix_gain_db,
            requested_video_resolution,
            requested_fps,
            requested_video_bitrate_kbps,
            effective_video_resolution,
            effective_fps,
            effective_video_bitrate_kbps,
            audio_fallback_policy,
            degrade_reason,
            audio_degrade_reason,
            last_audio_mode_error,
            capture_restart_count,
            capture_interrupt_count,
            video_smooth_state: state_video_smooth_state,
            capture_dropped_frames,
            capture_queue_overflows,
            effective_output_fps,
            concurrent_session_count,
            capture_owner_pid,
            app_rss_mb,
            app_cpu_percent,
            capture_stack_rss_mb,
            capture_stack_cpu_percent,
            capture_stack_rss_delta_mb,
            system_memory_pressure_level,
            thermal_state,
            power_source,
            capture_crash_loop,
            is_armed,
            is_saving,
            arm_blocker,
            arm_blocker_code,
            arm_blocker_action,
            pending_save,
            pending_full_window,
            pending_full_window_deadline_epoch_ms,
            full_window_wait_remaining_ms,
            warmup_eta_ms,
            audio_warmup_grace_ms,
            buffer_fill_secs,
            replay_fill_secs,
            replay_target_secs: settings.replay_duration_secs,
            rolling_fill_secs,
            rolling_target_secs: settings.buffer_duration_secs,
            last_error: state_last_error.or(capture_error),
            last_capture_log_tail,
            capture_start_phase,
            dropped_video_packets,
            dropped_audio_packets,
            last_contiguity_break_code,
            permission,
            settings,
        }
    }
}
