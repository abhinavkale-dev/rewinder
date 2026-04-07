use super::*;

impl Engine {
    pub(super) fn reset_overload_metric_counters(&self) {
        *self.last_drop_total.lock() = 0;
        *self.last_overflow_total.lock() = 0;
    }

    pub(super) fn update_overload_metric_deltas(
        &self,
        drop_total: u64,
        overflow_total: u64,
    ) -> (u64, u64) {
        let mut last_drop = self.last_drop_total.lock();
        let mut last_overflow = self.last_overflow_total.lock();
        let drop_delta = drop_total.saturating_sub(*last_drop);
        let overflow_delta = overflow_total.saturating_sub(*last_overflow);
        *last_drop = drop_total;
        *last_overflow = overflow_total;
        (drop_delta, overflow_delta)
    }

    pub(super) fn reset_resource_pressure_tracking(&self) {
        *self.capture_stack_rss_baseline_mb.lock() = None;
        *self.resource_soft_pressure_since.lock() = None;
        *self.resource_hard_pressure_since.lock() = None;
        self.resource_soft_trigger_timestamps.lock().clear();
        *self.resource_hard_stepdown_pending.lock() = false;
        *self.process_diagnostics_cache.lock() = None;
        self.state.lock().system_memory_pressure_level = None;
    }

    pub(super) fn capture_stack_pids(&self) -> Vec<u32> {
        let mut pids = vec![std::process::id()];
        let live_dir = self
            .state
            .lock()
            .settings
            .output_dir_path()
            .join(".rewinder-live");
        for pid_file in ["ffmpeg-capture.pid", "sck-capture.pid"] {
            let path = live_dir.join(pid_file);
            let Ok(raw) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(pid) = raw.trim().parse::<u32>() else {
                continue;
            };
            if pid > 0 {
                pids.push(pid);
            }
        }
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    pub(super) fn current_process_diagnostics(
        &self,
    ) -> (
        Option<u32>,
        Option<f32>,
        Option<u32>,
        Option<f32>,
        Option<u32>,
        Option<String>,
    ) {
        let now = Instant::now();
        if let Some(snapshot) = self.process_diagnostics_cache.lock().as_ref() {
            if now.duration_since(snapshot.sampled_at)
                < Duration::from_millis(PROCESS_DIAGNOSTICS_SAMPLE_INTERVAL_MS)
            {
                return (
                    snapshot.app_rss_mb,
                    snapshot.app_cpu_percent,
                    snapshot.capture_stack_rss_mb,
                    snapshot.capture_stack_cpu_percent,
                    snapshot.capture_stack_rss_delta_mb,
                    snapshot.thermal_state.clone(),
                );
            }
        }

        let (app_rss_mb, app_cpu_percent) = sample_process_ps_metrics(std::process::id());
        let capture_stack_pids = self.capture_stack_pids();
        let (capture_stack_rss_mb, capture_stack_cpu_percent) =
            sample_process_ps_metrics_for_pids(&capture_stack_pids);
        let capture_stack_rss_delta_mb = capture_stack_rss_mb.and_then(|rss_mb| {
            let mut baseline = self.capture_stack_rss_baseline_mb.lock();
            let baseline_value = baseline.get_or_insert(rss_mb);
            Some(rss_mb.saturating_sub(*baseline_value))
        });
        let thermal_state = sample_thermal_state();
        *self.process_diagnostics_cache.lock() = Some(ProcessDiagnosticsSnapshot {
            sampled_at: now,
            app_rss_mb,
            app_cpu_percent,
            capture_stack_rss_mb,
            capture_stack_cpu_percent,
            capture_stack_rss_delta_mb,
            thermal_state: thermal_state.clone(),
        });
        (
            app_rss_mb,
            app_cpu_percent,
            capture_stack_rss_mb,
            capture_stack_cpu_percent,
            capture_stack_rss_delta_mb,
            thermal_state,
        )
    }

    pub(super) fn clear_expired_pending_save(&self) {
        let now = Instant::now();
        let mut pending = self.pending_save.lock();
        if pending
            .as_ref()
            .map(|request| now >= request.expires_at)
            .unwrap_or(false)
        {
            let expired = pending.take();
            if let Some(request) = expired {
                self.append_capture_runtime_marker(&format!(
                    "phase: pending_save_expired reason=ttl_elapsed source={}",
                    trigger_source_label(&request.source)
                ));
                if !self.state.lock().is_saving {
                    self.state.lock().save_stage = SaveStageDto::Idle;
                }
            }
        }
    }

    pub(super) fn pending_save_snapshot(
        &self,
    ) -> (bool, bool, Option<i64>, Option<u32>, Option<u32>) {
        let now = Instant::now();
        let pending = self.pending_save.lock();
        let Some(request) = pending.as_ref() else {
            return (false, false, None, None, None);
        };
        let remaining = request.expires_at.saturating_duration_since(now);
        let eta = remaining.as_millis().min(u128::from(u32::MAX)) as u32;
        (true, false, None, None, Some(eta))
    }

    pub(super) fn register_hotkeys(&self, app: &AppHandle, source: &str) -> Result<(), String> {
        let (primary_hotkey, fallback_hotkeys) = {
            let state = self.state.lock();
            (
                state.settings.hotkey.clone(),
                state.settings.fallback_hotkeys.clone(),
            )
        };

        match hotkeys::replace_registration_with_fallbacks(app, &primary_hotkey, &fallback_hotkeys)
        {
            Ok(registration) => {
                let mut state = self.state.lock();
                state.hotkey_status = match registration.mode {
                    RegistrationMode::Primary => HotkeyStatusDto::Ok,
                    RegistrationMode::Fallback => HotkeyStatusDto::Fallback,
                };
                if registration.mode == RegistrationMode::Fallback {
                    state.settings.hotkey = registration.selected_hotkey.clone();
                    state.last_error = Some(format!(
                        "{source}: primary hotkey unavailable, using fallback '{}'",
                        registration.selected_hotkey
                    ));
                    events::emit_hotkey_conflict(
                        app,
                        format!(
                            "Primary hotkey unavailable. Using fallback {}",
                            registration.selected_hotkey
                        ),
                        Some("Choose a different shortcut in settings if needed.".to_string()),
                    );
                }
                Ok(())
            }
            Err(err) => {
                let mut state = self.state.lock();
                state.hotkey_status = HotkeyStatusDto::Conflict;
                Err(err)
            }
        }
    }

    pub(super) fn select_replay_for_save(
        &self,
        replay_duration_secs: u16,
        anchor_time: SystemTime,
    ) -> Result<Option<ReplaySelection>, String> {
        let pipeline = self.pipeline.lock();
        match pipeline.as_ref() {
            Some(handles) => handles
                .capture
                .replay_selection_for_save_at(replay_duration_secs, anchor_time),
            None => Ok(None),
        }
    }

    pub(super) fn current_warmup_message(&self) -> String {
        let pipeline = self.pipeline.lock();
        match pipeline.as_ref() {
            Some(handles) => {
                format!(
                    "{} Will save automatically.",
                    handles.capture.warmup_reason()
                )
            }
            None => "Replay warming up, will save automatically.".to_string(),
        }
    }

    pub(super) fn capture_unavailable_message(&self) -> Option<String> {
        let (last_error, last_audio_mode_error, audio_fallback_policy, requested_audio_mode) = {
            let state = self.state.lock();
            (
                state.last_error.clone(),
                state.last_audio_mode_error.clone(),
                state.settings.audio_fallback_policy.clone(),
                state.settings.audio_mode.clone(),
            )
        };
        if audio_fallback_policy == "system_only_fallback" && requested_audio_mode != "video_only" {
            if let Some(audio_err) = last_audio_mode_error {
                return Some(format!(
                    "Required system-audio path unavailable: {audio_err}"
                ));
            }
        }
        let last_error = last_error?;
        let lower = last_error.to_ascii_lowercase();
        let likely_capture_failure = lower.contains("capture recovery retry failed")
            || lower.contains("failed to start capture")
            || lower.contains("capture strategy")
            || lower.contains("capture_start_timeout");
        if !likely_capture_failure {
            return None;
        }

        let log_path = self.capture_log_path();
        let ffmpeg_tail = fs::read_to_string(&log_path)
            .ok()
            .and_then(|content| {
                let lines: Vec<&str> = content
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect();
                if lines.is_empty() {
                    None
                } else {
                    let start = lines.len().saturating_sub(6);
                    Some(lines[start..].join(" | "))
                }
            })
            .unwrap_or_else(|| "no ffmpeg capture log tail available".to_string());

        Some(format!(
            "Capture pipeline is unavailable ({last_error}). Check Screen Recording permission and source availability. log: {} | tail: {}",
            log_path.display(),
            ffmpeg_tail
        ))
    }

    pub(super) fn capture_log_path(&self) -> PathBuf {
        let settings = self.state.lock().settings.clone();
        settings
            .output_dir_path()
            .join(".rewinder-live")
            .join("ffmpeg-capture.log")
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn perform_save_replay(
        &self,
        app: &AppHandle,
        settings: SettingsDto,
        selection: ReplaySelection,
        save_id: u64,
        source: &TriggerSourceDto,
        requested_replay_secs: u16,
        anchor_time: SystemTime,
    ) -> SaveReplayResultDto {
        self.append_capture_runtime_marker(&format!(
            "phase: save_started id={} source={}",
            save_id,
            trigger_source_label(source)
        ));
        {
            let mut state = self.state.lock();
            state.is_saving = true;
            state.lifecycle_state = LifecycleState::SavingReplay;
            state.capture_health = CaptureHealthDto::Running;
            state.last_error = None;
            state.save_ready = true;
            state.save_stage = SaveStageDto::SavingFast;
        }
        self.pending_save.lock().take();
        *self.last_save_started_at.lock() = Some(Instant::now());
        events::emit_engine_state(app, &self.get_engine_state());
        let (save_effective_video_resolution, save_effective_fps) = {
            let state = self.state.lock();
            (
                Some(state.effective_video_resolution),
                Some(state.effective_fps),
            )
        };
        let requested_duration_secs = Some(f32::from(requested_replay_secs.max(1)));
        let selected_duration_secs = Some(selection.target_trim_secs);
        let contiguous_duration_secs = Some(selection.contiguous_duration_secs);
        let partial_reason_code = selection.partial_reason_code.clone();
        let anchor_epoch_ms = Some(to_epoch_ms_i64(anchor_time));

        let (
            capture_speed_x,
            playback_realtime_x,
            effective_output_fps,
            dropped_frames,
            queue_overflows,
        ) = {
            let pipeline = self.pipeline.lock();
            if let Some(handles) = pipeline.as_ref() {
                (
                    handles.capture.capture_speed_x(),
                    handles.capture.playback_realtime_x(),
                    handles.capture.effective_output_fps(),
                    handles.capture.capture_dropped_frames(),
                    handles.capture.capture_queue_overflows(),
                )
            } else {
                (None, None, None, 0, 0)
            }
        };
        self.append_capture_runtime_marker(&format!(
            "phase: save_diagnostics id={} capture_speed_x={} playback_realtime_x={} output_fps={} dropped_frames={} queue_overflows={}",
            save_id,
            capture_speed_x
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "none".to_string()),
            playback_realtime_x
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "none".to_string()),
            effective_output_fps
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "none".to_string()),
            dropped_frames,
            queue_overflows
        ));

        {
            let pipeline = self.pipeline.lock();
            if let Some(handles) = pipeline.as_ref() {
                handles.capture.freeze_pruning();
            }
        }
        let mut result = match replay_writer::write_replay_from_segments(
            &selection.segments,
            &settings,
            selection.target_trim_secs,
            selection.available_secs,
        ) {
            Ok(outcome) => {
                let save_message = {
                    let mut messages: Vec<String> = Vec::new();
                    let state = self.state.lock();
                    if state.active_audio_mode != state.settings.audio_mode {
                        messages.push(format!(
                            "Capture is running in {} mode.",
                            state.active_audio_mode
                        ));
                    }
                    drop(state);
                    if selection.partial_history {
                        let partial_detail = selection
                            .partial_reason
                            .as_deref()
                            .unwrap_or("insufficient contiguous history at trigger time");
                        messages.push(format!(
                            "Saved {:.1}s partial replay due to {}.",
                            selection.target_trim_secs, partial_detail
                        ));
                    }
                    if messages.is_empty() {
                        None
                    } else {
                        Some(messages.join(" "))
                    }
                };
                {
                    let mut state = self.state.lock();
                    state.recent_clips.insert(0, outcome.clip.clone());
                    state.recent_clips.truncate(MAX_RECENT_CLIPS);
                    state.last_error = None;
                    state.last_contiguity_break_code = selection.partial_reason_code.clone();
                    state.save_stage = SaveStageDto::Ready;
                    state.video_smooth_state = if outcome.audio_repaired {
                        VideoSmoothStateDto::Complete
                    } else {
                        VideoSmoothStateDto::Idle
                    };
                }
                let actual_duration = outcome.clip.duration_secs;
                events::emit_clip_saved(app, &outcome.clip);
                if let Some(warning) = outcome.warning.as_ref() {
                    events::emit_save_warning(
                        app,
                        warning.code.clone(),
                        warning.message.clone(),
                        warning.action.clone(),
                    );
                    if warning.code.ends_with("_corrected") {
                        self.append_capture_runtime_marker(&format!(
                            "phase: save_integrity_corrected id={} code={} source={}",
                            save_id,
                            warning.code,
                            trigger_source_label(source)
                        ));
                    }
                }
                if selection.partial_history {
                    let partial_detail = selection.partial_reason.clone().unwrap_or_else(|| {
                        "insufficient contiguous history at trigger time".to_string()
                    });
                    let reason_code = selection
                        .partial_reason_code
                        .as_deref()
                        .unwrap_or("insufficient_contiguous_history");
                    events::emit_save_warning(
                        app,
                        "partial_history",
                        format!(
                            "Saved latest {:.1}s (requested {}s, partial due to {}).",
                            selection.target_trim_secs,
                            requested_replay_secs,
                            partial_detail
                        ),
                        Some(format!(
                            "reason_code={reason_code}; capture timeline was not fully contiguous near trigger time."
                        )),
                    );
                }
                SaveReplayResultDto {
                    ok: true,
                    queued: false,
                    clip: Some(outcome.clip),
                    error: None,
                    message: save_message,
                    actual_duration_secs: Some(actual_duration),
                    audio_repaired: outcome.audio_repaired,
                    save_audio_strategy: Some(outcome.save_audio_strategy),
                    smooth_pending: false,
                    smooth_applied: outcome.audio_repaired,
                    smooth_error: None,
                    effective_video_resolution: save_effective_video_resolution,
                    effective_fps: save_effective_fps,
                    requested_duration_secs,
                    selected_duration_secs,
                    contiguous_duration_secs,
                    partial_reason_code,
                    anchor_epoch_ms,
                }
            }
            Err(err) => {
                {
                    let mut state = self.state.lock();
                    state.last_error = Some(err.clone());
                    state.last_contiguity_break_code = selection.partial_reason_code.clone();
                    state.capture_health = CaptureHealthDto::Degraded;
                    state.audio_health = AudioHealthDto::Degraded;
                    state.save_stage = SaveStageDto::Idle;
                }
                events::emit_save_failed_code(
                    app,
                    "writer_error",
                    err.clone(),
                    Some("Check output folder access and ffmpeg availability.".to_string()),
                );
                SaveReplayResultDto {
                    ok: false,
                    queued: false,
                    clip: None,
                    error: Some(err),
                    message: None,
                    actual_duration_secs: None,
                    audio_repaired: false,
                    save_audio_strategy: None,
                    smooth_pending: false,
                    smooth_applied: false,
                    smooth_error: None,
                    effective_video_resolution: save_effective_video_resolution,
                    effective_fps: save_effective_fps,
                    requested_duration_secs,
                    selected_duration_secs,
                    contiguous_duration_secs,
                    partial_reason_code,
                    anchor_epoch_ms,
                }
            }
        };
        {
            let pipeline = self.pipeline.lock();
            if let Some(handles) = pipeline.as_ref() {
                handles.capture.unfreeze_pruning();
            }
        }

        if result.ok && Self::should_schedule_smooth_postprocess(&settings) {
            if let Some(clip) = result.clip.as_ref() {
                self.enqueue_smooth_postprocess(
                    save_id,
                    source.clone(),
                    PathBuf::from(&clip.path),
                    settings.clone(),
                );
                result.smooth_pending = true;
                result.smooth_applied = false;
                self.append_capture_runtime_marker(&format!(
                    "phase: smooth_postprocess_queued id={} source={} path={}",
                    save_id,
                    trigger_source_label(source),
                    clip.path
                ));
            }
        }

        if result.ok
            && Self::should_schedule_fast_integrity_check(result.save_audio_strategy.as_deref())
        {
            if let Some(clip) = result.clip.as_ref() {
                self.enqueue_fast_integrity_check(
                    save_id,
                    source.clone(),
                    PathBuf::from(&clip.path),
                    settings.clone(),
                    result
                        .save_audio_strategy
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                );
                self.append_capture_runtime_marker(&format!(
                    "phase: fast_save_verify_queued id={} source={} strategy={} path={}",
                    save_id,
                    trigger_source_label(source),
                    result.save_audio_strategy.as_deref().unwrap_or("unknown"),
                    clip.path
                ));
            }
        }

        {
            let mut state = self.state.lock();
            state.is_saving = false;
            state.lifecycle_state =
                lifecycle::idle_state(&state.permission, state.settings.replay_enabled);
            if matches!(state.save_stage, SaveStageDto::SavingFast) {
                state.save_stage = if result.ok {
                    SaveStageDto::Ready
                } else {
                    SaveStageDto::Idle
                };
            }
            if state.capture_health == CaptureHealthDto::Degraded && self.pipeline.lock().is_some()
            {
                state.capture_health = CaptureHealthDto::Running;
            }
        }
        events::emit_engine_state(app, &self.get_engine_state());

        let status = if result.ok { "ok" } else { "err" };
        self.append_capture_runtime_marker(&format!(
            "phase: save_finished id={} source={} status={}",
            save_id,
            trigger_source_label(source),
            status
        ));

        result
    }

    pub(super) fn process_pending_save(&self, app: &AppHandle) {
        let request = {
            let guard = self.pending_save.lock();
            guard.clone()
        };
        let Some(request) = request else {
            return;
        };

        let now = Instant::now();

        if now >= request.expires_at {
            self.pending_save.lock().take();
            if !self.state.lock().is_saving {
                self.state.lock().save_stage = SaveStageDto::Idle;
            }
            let source_label = match request.source {
                TriggerSourceDto::Manual => "manual",
                TriggerSourceDto::Hotkey => "hotkey",
            };
            events::emit_save_deferred(
                app,
                format!(
                    "Replay warmup timed out for {source_label} trigger. Press save again when capture is active."
                ),
            );
            events::emit_engine_state(app, &self.get_engine_state());
            return;
        }

        let _save_gate = match self.save_entry_gate.try_lock() {
            Some(guard) => guard,
            None => return,
        };

        let blocker = {
            let state = self.state.lock();
            self.save_blocker_with_runtime(&state)
        };
        if let Some(blocker) = blocker {
            if blocker.retryable {
                self.state.lock().save_stage = SaveStageDto::Queued;
                return;
            }
            self.pending_save.lock().take();
            if !self.state.lock().is_saving {
                self.state.lock().save_stage = SaveStageDto::Idle;
            }
            return;
        }

        let settings = self.state.lock().settings.clone();
        let requested_replay_secs = request.requested_replay_secs.max(1);

        match self.select_replay_for_save(requested_replay_secs, request.anchor_time) {
            Ok(Some(selection)) => {
                self.pending_save.lock().take();
                let save_id = self.save_operation_seq.fetch_add(1, Ordering::Relaxed) + 1;
                self.append_save_trigger_marker("accepted", trigger_source_label(&request.source));
                self.append_capture_runtime_marker(&format!(
                    "phase: save_anchor_time epoch_ms={}",
                    to_epoch_ms(request.anchor_time)
                ));
                if selection.partial_history {
                    self.append_capture_runtime_marker(&format!(
                        "phase: partial_save_immediate requested_secs={} selected_secs={:.2} reason_code={}",
                        requested_replay_secs,
                        selection.target_trim_secs,
                        selection
                            .partial_reason_code
                            .as_deref()
                            .unwrap_or("insufficient_contiguous_history")
                    ));
                    self.append_capture_runtime_marker(
                        "phase: full_window_wait_bypassed policy=immediate_partial",
                    );
                }
                self.append_capture_runtime_marker(&format!(
                    "phase: selected_window_start={} selected_window_end={} selected_duration_secs={:.2} contiguous_secs={:.2} partial_history={} boundary_count={} reason_code={}",
                    to_epoch_ms(selection.window_start),
                    to_epoch_ms(selection.window_end),
                    selection.target_trim_secs,
                    selection.contiguous_duration_secs,
                    selection.partial_history,
                    selection.session_boundary_count,
                    selection
                        .partial_reason_code
                        .as_deref()
                        .unwrap_or("none")
                ));
                if let Some(gap_ms) = selection.discontinuity_gap_ms {
                    self.append_capture_runtime_marker(&format!(
                        "phase: session_boundary_gap_ms={} partial_reason={}",
                        gap_ms,
                        selection
                            .partial_reason
                            .as_deref()
                            .unwrap_or("discontinuity")
                            .replace(' ', "_")
                    ));
                }
                let _ = self.perform_save_replay(
                    app,
                    settings,
                    selection,
                    save_id,
                    &request.source,
                    requested_replay_secs,
                    request.anchor_time,
                );
            }
            Ok(None) => {
                self.state.lock().save_stage = SaveStageDto::Queued;
            }
            Err(err) => {
                self.pending_save.lock().take();
                {
                    let mut state = self.state.lock();
                    state.last_error = Some(err.clone());
                    state.capture_health = CaptureHealthDto::Degraded;
                    state.audio_health = AudioHealthDto::Degraded;
                    state.save_stage = SaveStageDto::Idle;
                }
                let (code, action) = classify_capture_failure(&err);
                self.pause_for_capture_owner_conflict(app, code);
                events::emit_save_failed_code(app, code, err, action);
                events::emit_engine_state(app, &self.get_engine_state());
            }
        }
    }
}
