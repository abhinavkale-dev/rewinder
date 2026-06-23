use std::path::Path;

use super::*;

impl Engine {
    pub fn trigger_save_replay(
        &self,
        app: &Arc<dyn EngineHost>,
        source: TriggerSourceDto,
    ) -> SaveReplayResultDto {
        self.refresh_runtime_readiness_from_pipeline();
        self.clear_expired_pending_save();
        let anchor_time = SystemTime::now();
        let (save_effective_video_resolution, save_effective_fps) = {
            let state = self.state.lock();
            (
                Some(state.effective_video_resolution),
                Some(state.effective_fps),
            )
        };
        if matches!(source, TriggerSourceDto::Hotkey) {
            let hotkey = self.state.lock().settings.hotkey.clone();
            events::emit_hotkey_triggered(app, hotkey);
        }
        let _save_gate = match self.save_entry_gate.try_lock() {
            Some(guard) => guard,
            None => {
                return self.queue_retryable_save(
                    app,
                    source,
                    &SaveBlocker {
                        code: "busy",
                        message: "Replay save already in progress.".to_string(),
                        action: Some("Wait for the current save to finish.".to_string()),
                        retryable: true,
                    },
                    anchor_time,
                )
            }
        };

        let blocker_result = {
            let mut state = self.state.lock();
            if let Some(blocker) = self.save_blocker_with_runtime(&state) {
                if blocker.retryable {
                    state.arm_blocker = None;
                    state.arm_blocker_code = None;
                    state.arm_blocker_action = None;
                    state.last_error = None;
                } else {
                    state.arm_blocker = Some(blocker.message.clone());
                    state.arm_blocker_code = Some(blocker.code.to_string());
                    state.arm_blocker_action = blocker.action.clone();
                    state.last_error = Some(blocker.message.clone());
                }
                Err(blocker)
            } else {
                state.arm_blocker = None;
                state.arm_blocker_code = None;
                state.arm_blocker_action = None;
                Ok(state.settings.clone())
            }
        };
        let (settings, warmup_override_active) = match blocker_result {
            Ok(settings) => (settings, false),
            Err(blocker) => {
                if blocker.retryable && blocker.code == "audio_warming_up" {
                    (self.state.lock().settings.clone(), true)
                } else if blocker.retryable {
                    return self.queue_retryable_save(app, source, &blocker, anchor_time);
                } else {
                    events::emit_save_failed_code(
                        app,
                        blocker.code,
                        blocker.message.clone(),
                        blocker.action.clone(),
                    );
                    if is_audio_path_blocker_code(blocker.code) {
                        events::emit_audio_path_failed(
                            app,
                            blocker.message.clone(),
                            blocker.action.clone(),
                        );
                    }
                    {
                        let mut state = self.state.lock();
                        state.save_stage = SaveStageDto::Idle;
                    }
                    events::emit_engine_state(app, &self.get_engine_state());
                    return SaveReplayResultDto {
                        ok: false,
                        queued: false,
                        clip: None,
                        error: Some(blocker.message),
                        message: None,
                        actual_duration_secs: None,
                        audio_repaired: false,
                        save_audio_strategy: None,
                        smooth_pending: false,
                        smooth_applied: false,
                        smooth_error: None,
                        effective_video_resolution: save_effective_video_resolution,
                        effective_fps: save_effective_fps,
                        requested_duration_secs: Some(f32::from(
                            self.state.lock().settings.replay_duration_secs.max(1),
                        )),
                        selected_duration_secs: None,
                        contiguous_duration_secs: None,
                        partial_reason_code: None,
                        anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
                    };
                }
            }
        };

        let selection = match self
            .select_replay_for_save(settings.replay_duration_secs, anchor_time)
        {
            Ok(Some(selection)) => selection,
            Ok(None) => {
                if warmup_override_active {
                    self.append_save_trigger_marker(
                        "deferred_warmup",
                        trigger_source_label(&source),
                    );
                    let ttl_ms = self
                        .default_pending_save_ttl_ms()
                        .max(AUDIO_WARMUP_MIN_DEFER_TTL_MS);
                    self.enqueue_pending_save(
                        source.clone(),
                        ttl_ms,
                        anchor_time,
                        PendingSaveReason::AudioWarmup,
                        settings.replay_duration_secs,
                    );
                    {
                        let mut state = self.state.lock();
                        state.last_error = None;
                        state.save_ready = false;
                        state.save_stage = SaveStageDto::Queued;
                    }
                    let message = self.current_warmup_message();
                    events::emit_save_deferred(app, message.clone());
                    events::emit_engine_state(app, &self.get_engine_state());
                    return SaveReplayResultDto {
                        ok: true,
                        queued: true,
                        clip: None,
                        error: None,
                        message: Some(message),
                        actual_duration_secs: None,
                        audio_repaired: false,
                        save_audio_strategy: None,
                        smooth_pending: false,
                        smooth_applied: false,
                        smooth_error: None,
                        effective_video_resolution: save_effective_video_resolution,
                        effective_fps: save_effective_fps,
                        requested_duration_secs: Some(f32::from(settings.replay_duration_secs)),
                        selected_duration_secs: None,
                        contiguous_duration_secs: None,
                        partial_reason_code: None,
                        anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
                    };
                }
                if let Some(message) = self.capture_unavailable_message() {
                    {
                        let mut state = self.state.lock();
                        state.last_error = Some(message.clone());
                        state.capture_health = CaptureHealthDto::Degraded;
                        state.audio_health = AudioHealthDto::Degraded;
                        state.save_stage = SaveStageDto::Idle;
                    }
                    let (code, action) = classify_capture_failure(&message);
                    events::emit_save_failed_code(app, code, message.clone(), action);
                    events::emit_engine_state(app, &self.get_engine_state());
                    return SaveReplayResultDto {
                        ok: false,
                        queued: false,
                        clip: None,
                        error: Some(message),
                        message: None,
                        actual_duration_secs: None,
                        audio_repaired: false,
                        save_audio_strategy: None,
                        smooth_pending: false,
                        smooth_applied: false,
                        smooth_error: None,
                        effective_video_resolution: save_effective_video_resolution,
                        effective_fps: save_effective_fps,
                        requested_duration_secs: Some(f32::from(settings.replay_duration_secs)),
                        selected_duration_secs: None,
                        contiguous_duration_secs: None,
                        partial_reason_code: None,
                        anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
                    };
                }
                self.append_save_trigger_marker("deferred_warmup", trigger_source_label(&source));
                let ttl_ms = self.default_pending_save_ttl_ms();
                self.enqueue_pending_save(
                    source.clone(),
                    ttl_ms,
                    anchor_time,
                    PendingSaveReason::Retryable,
                    settings.replay_duration_secs,
                );
                {
                    let mut state = self.state.lock();
                    state.last_error = None;
                    state.save_ready = false;
                    state.save_stage = SaveStageDto::Queued;
                }
                let message = self.current_warmup_message();
                events::emit_save_deferred(app, message.clone());
                events::emit_engine_state(app, &self.get_engine_state());
                return SaveReplayResultDto {
                    ok: true,
                    queued: true,
                    clip: None,
                    error: None,
                    message: Some(message),
                    actual_duration_secs: None,
                    audio_repaired: false,
                    save_audio_strategy: None,
                    smooth_pending: false,
                    smooth_applied: false,
                    smooth_error: None,
                    effective_video_resolution: save_effective_video_resolution,
                    effective_fps: save_effective_fps,
                    requested_duration_secs: Some(f32::from(settings.replay_duration_secs)),
                    selected_duration_secs: None,
                    contiguous_duration_secs: None,
                    partial_reason_code: None,
                    anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
                };
            }
            Err(err) => {
                {
                    let mut state = self.state.lock();
                    state.last_error = Some(err.clone());
                    state.capture_health = CaptureHealthDto::Degraded;
                    state.audio_health = AudioHealthDto::Degraded;
                    state.save_stage = SaveStageDto::Idle;
                    state.video_smooth_state = VideoSmoothStateDto::Idle;
                }
                let (code, action) = classify_capture_failure(&err);
                events::emit_save_failed_code(app, code, err.clone(), action);
                events::emit_engine_state(app, &self.get_engine_state());
                return SaveReplayResultDto {
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
                    requested_duration_secs: Some(f32::from(settings.replay_duration_secs)),
                    selected_duration_secs: None,
                    contiguous_duration_secs: None,
                    partial_reason_code: None,
                    anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
                };
            }
        };

        let save_id = self.save_operation_seq.fetch_add(1, Ordering::Relaxed) + 1;
        self.append_save_trigger_marker("accepted", trigger_source_label(&source));
        self.append_capture_runtime_marker(&format!(
            "phase: save_anchor_time epoch_ms={}",
            to_epoch_ms(anchor_time)
        ));
        if selection.partial_history {
            self.append_capture_runtime_marker(&format!(
                "phase: partial_save_immediate requested_secs={} selected_secs={:.2} reason_code={}",
                settings.replay_duration_secs,
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
        if warmup_override_active {
            self.append_capture_runtime_marker(
                "phase: save_gate_override_audio_warmup_has_segments",
            );
        }
        let requested_replay_secs = settings.replay_duration_secs;
        let result = self.perform_save_replay(
            app,
            settings,
            selection,
            save_id,
            &source,
            requested_replay_secs,
            anchor_time,
        );
        if warmup_override_active && result.ok {
            events::emit_save_warning(
                app,
                "audio_warmup_saved",
                "Saved replay while audio path was warming up; clip may include a brief audio ramp-in.",
                Some("Capture stayed armed and did not block replay save.".to_string()),
            );
        }
        result
    }

    pub(super) fn enqueue_pending_save(
        &self,
        source: TriggerSourceDto,
        ttl_ms: u64,
        anchor_time: SystemTime,
        reason: PendingSaveReason,
        requested_replay_secs: u16,
    ) {
        self.enqueue_pending_save_with_ttl(
            source,
            ttl_ms,
            anchor_time,
            reason,
            requested_replay_secs,
        );
    }

    pub(super) fn default_pending_save_ttl_ms(&self) -> u64 {
        u64::from(self.state.lock().settings.warmup_defer_ttl_ms)
    }

    pub(super) fn enqueue_pending_save_with_ttl(
        &self,
        source: TriggerSourceDto,
        ttl_ms: u64,
        anchor_time: SystemTime,
        reason: PendingSaveReason,
        requested_replay_secs: u16,
    ) {
        let now = Instant::now();
        let request = PendingSaveRequest {
            source,
            expires_at: now + Duration::from_millis(ttl_ms),
            anchor_time,
            reason,
            requested_replay_secs,
        };
        *self.pending_save.lock() = Some(request);
        self.state.lock().save_stage = SaveStageDto::Queued;
    }

    pub(super) fn enqueue_or_replace_pending_save_with_ttl(
        &self,
        source: TriggerSourceDto,
        ttl_ms: u64,
        anchor_time: SystemTime,
        reason: PendingSaveReason,
        requested_replay_secs: u16,
    ) -> PendingSaveEnqueueOutcome {
        let now = Instant::now();
        let mut pending = self.pending_save.lock();
        let outcome = if let Some(request) = pending.as_mut() {
            let previous_anchor_time = request.anchor_time;
            request.source = source;
            request.anchor_time = anchor_time;
            request.expires_at = now + Duration::from_millis(ttl_ms);
            request.reason = reason;
            request.requested_replay_secs = requested_replay_secs;
            PendingSaveEnqueueOutcome::ReplacedExisting {
                previous_anchor_time,
            }
        } else {
            *pending = Some(PendingSaveRequest {
                source,
                expires_at: now + Duration::from_millis(ttl_ms),
                anchor_time,
                reason,
                requested_replay_secs,
            });
            PendingSaveEnqueueOutcome::QueuedNew
        };
        drop(pending);
        self.state.lock().save_stage = SaveStageDto::Queued;
        outcome
    }

    pub(super) fn should_schedule_smooth_postprocess(settings: &SettingsDto) -> bool {
        let _ = settings;
        false
    }

    pub(super) fn should_schedule_fast_integrity_check(save_audio_strategy: Option<&str>) -> bool {
        matches!(
            save_audio_strategy,
            Some("instant_mp4" | "fast" | "fallback_fast")
        )
    }

    pub(super) fn enqueue_smooth_postprocess(
        &self,
        save_id: u64,
        source: TriggerSourceDto,
        clip_path: PathBuf,
        settings: SettingsDto,
    ) {
        let mut replaced_existing = false;
        {
            let mut jobs = self.pending_smooth_jobs.lock();
            if !jobs.is_empty() {
                jobs.clear();
                replaced_existing = true;
            }
            jobs.push_back(PendingSmoothJob {
                save_id,
                source,
                clip_path,
                settings,
            });
        }
        if replaced_existing {
            self.append_capture_runtime_marker("phase: smooth_queue_replaced latest_wins");
        }
        self.state.lock().video_smooth_state = VideoSmoothStateDto::Pending;
    }

    pub(super) fn process_pending_smooth_jobs(&self, app: &Arc<dyn EngineHost>) {
        let job = self.pending_smooth_jobs.lock().pop_front();
        let Some(job) = job else {
            return;
        };

        {
            let mut state = self.state.lock();
            state.video_smooth_state = VideoSmoothStateDto::Processing;
        }
        events::emit_engine_state(app, &self.get_engine_state());
        self.append_capture_runtime_marker(&format!(
            "phase: smooth_postprocess_started id={} source={} path={}",
            job.save_id,
            trigger_source_label(&job.source),
            job.clip_path.display()
        ));

        let started = Instant::now();
        let smooth_result = replay_writer::smooth_replay_in_place(&job.clip_path, &job.settings);
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        match smooth_result {
            Ok(()) => {
                {
                    let mut state = self.state.lock();
                    state.video_smooth_state = VideoSmoothStateDto::Complete;
                }
                self.append_capture_runtime_marker(&format!(
                    "phase: smooth_postprocess_finished id={} status=ok duration_ms={elapsed_ms}",
                    job.save_id
                ));
                events::emit_save_warning(
                    app,
                    "video_smooth_applied",
                    "Smooth replay postprocess applied to latest saved clip.",
                    None,
                );
            }
            Err(err) => {
                {
                    let mut state = self.state.lock();
                    state.video_smooth_state = VideoSmoothStateDto::Failed;
                }
                self.append_capture_runtime_marker(&format!(
                    "phase: smooth_postprocess_finished id={} status=err duration_ms={} detail={}",
                    job.save_id,
                    elapsed_ms,
                    err.replace(' ', "_")
                ));
                events::emit_save_warning(
                    app,
                    "video_smooth_failed",
                    format!(
                        "Smooth replay postprocess failed; kept instant clip as-is. detail={err}"
                    ),
                    Some(
                        "Clip was saved instantly; smoothing retry will occur on next save."
                            .to_string(),
                    ),
                );
            }
        }

        events::emit_engine_state(app, &self.get_engine_state());
    }

    pub(super) fn enqueue_fast_integrity_check(
        &self,
        save_id: u64,
        source: TriggerSourceDto,
        clip_path: PathBuf,
        settings: SettingsDto,
        save_audio_strategy: String,
    ) {
        self.pending_fast_verify_jobs
            .lock()
            .push_back(PendingFastVerifyJob {
                save_id,
                source,
                clip_path,
                settings,
                save_audio_strategy,
            });
    }

    pub(super) fn process_pending_fast_integrity_jobs(self: &Arc<Self>, app: &Arc<dyn EngineHost>) {
        if self.fast_verify_inflight.load(Ordering::Relaxed) {
            return;
        }

        let job = self.pending_fast_verify_jobs.lock().pop_front();
        let Some(job) = job else {
            return;
        };

        self.fast_verify_inflight.store(true, Ordering::Relaxed);
        let engine = Arc::clone(self);
        let app = app.clone();
        thread::spawn(move || {
            engine.append_capture_runtime_marker(&format!(
                "phase: fast_save_verify_started id={} source={} strategy={} path={}",
                job.save_id,
                trigger_source_label(&job.source),
                job.save_audio_strategy,
                job.clip_path.display()
            ));

            let started = Instant::now();
            let outcome = replay_writer::verify_fast_replay_in_place(&job.clip_path, &job.settings);
            let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

            match outcome {
                Ok(replay_writer::FastReplayVerificationOutcome::Verified) => {
                    engine.append_capture_runtime_marker(&format!(
                        "phase: fast_save_verify_finished id={} status=ok duration_ms={elapsed_ms}",
                        job.save_id
                    ));
                }
                Ok(replay_writer::FastReplayVerificationOutcome::Corrected {
                    warning,
                    duration_secs,
                }) => {
                    engine.append_capture_runtime_marker(&format!(
                        "phase: fast_save_verify_finished id={} status=corrected code={} duration_ms={elapsed_ms}",
                        job.save_id,
                        warning.code
                    ));
                    events::emit_save_warning(
                        &app,
                        warning.code.clone(),
                        warning.message.clone(),
                        warning.action.clone(),
                    );
                    if let Some(updated_clip) =
                        engine.refresh_recent_clip_metadata_for_path(&job.clip_path, duration_secs)
                    {
                        events::emit_clip_saved(&app, &updated_clip);
                    }
                }
                Ok(replay_writer::FastReplayVerificationOutcome::RepairFailed { issue, error }) => {
                    engine.append_capture_runtime_marker(&format!(
                        "phase: fast_save_verify_finished id={} status=repair_failed code={} duration_ms={} detail={}",
                        job.save_id,
                        issue.code,
                        elapsed_ms,
                        error.replace(' ', "_")
                    ));
                    events::emit_save_warning(
                        &app,
                        "fast_verify_repair_failed",
                        format!(
                            "{} Background correction failed; kept clip as-is. detail={error}",
                            issue.message
                        ),
                        Some(
                            issue
                                .action
                                .unwrap_or_else(|| {
                                    "Clip was saved immediately; if playback still looks wrong, try Smooth save mode."
                                        .to_string()
                                }),
                        ),
                    );
                }
                Err(err) => {
                    engine.append_capture_runtime_marker(&format!(
                        "phase: fast_save_verify_finished id={} status=err duration_ms={} detail={}",
                        job.save_id,
                        elapsed_ms,
                        err.replace(' ', "_")
                    ));
                    events::emit_save_warning(
                        &app,
                        "fast_verify_failed",
                        format!(
                            "Fast replay verification failed; clip kept as-is. detail={err}"
                        ),
                        Some(
                            "Clip was saved immediately; if playback looks wrong, try Smooth save mode."
                                .to_string(),
                        ),
                    );
                }
            }

            engine.fast_verify_inflight.store(false, Ordering::Relaxed);
        });
    }

    fn refresh_recent_clip_metadata_for_path(
        &self,
        clip_path: &Path,
        duration_secs: f32,
    ) -> Option<ClipMetadataDto> {
        let size_bytes = fs::metadata(clip_path).ok()?.len();
        let clip_path = clip_path.to_string_lossy().to_string();
        let mut state = self.state.lock();
        let clip = state
            .recent_clips
            .iter_mut()
            .find(|clip| clip.path == clip_path)?;
        clip.duration_secs = duration_secs;
        clip.size_bytes = size_bytes;
        Some(clip.clone())
    }

    pub(super) fn queue_retryable_save(
        &self,
        app: &Arc<dyn EngineHost>,
        source: TriggerSourceDto,
        blocker: &SaveBlocker,
        anchor_time: SystemTime,
    ) -> SaveReplayResultDto {
        let requested_replay_secs = self.state.lock().settings.replay_duration_secs.max(1);
        let (save_effective_video_resolution, save_effective_fps) = {
            let state = self.state.lock();
            (
                Some(state.effective_video_resolution),
                Some(state.effective_fps),
            )
        };
        if blocker.code == "audio_warming_up" {
            let ttl_ms = self
                .default_pending_save_ttl_ms()
                .max(AUDIO_WARMUP_MIN_DEFER_TTL_MS);
            let queue_outcome = self.enqueue_or_replace_pending_save_with_ttl(
                source.clone(),
                ttl_ms,
                anchor_time,
                PendingSaveReason::AudioWarmup,
                requested_replay_secs,
            );
            self.append_save_trigger_marker(
                match queue_outcome {
                    PendingSaveEnqueueOutcome::QueuedNew => "retryable_queued",
                    PendingSaveEnqueueOutcome::ReplacedExisting { .. } => {
                        "retryable_updated_queued_anchor"
                    }
                },
                trigger_source_label(&source),
            );
            let queued_message = match queue_outcome {
                PendingSaveEnqueueOutcome::QueuedNew => {
                    "Replay queued, waiting for capture warmup.".to_string()
                }
                PendingSaveEnqueueOutcome::ReplacedExisting {
                    previous_anchor_time,
                } => {
                    self.append_capture_runtime_marker(&format!(
                        "phase: pending_save_anchor_updated reason=audio_warming_up prev_epoch_ms={} next_epoch_ms={}",
                        to_epoch_ms(previous_anchor_time),
                        to_epoch_ms(anchor_time)
                    ));
                    "Replay queue updated to latest trigger while capture warms up.".to_string()
                }
            };
            events::emit_save_deferred(app, queued_message.clone());
            self.state.lock().save_stage = SaveStageDto::Queued;
            events::emit_engine_state(app, &self.get_engine_state());
            return SaveReplayResultDto {
                ok: true,
                queued: true,
                clip: None,
                error: None,
                message: Some(queued_message),
                actual_duration_secs: None,
                audio_repaired: false,
                save_audio_strategy: None,
                smooth_pending: false,
                smooth_applied: false,
                smooth_error: None,
                effective_video_resolution: save_effective_video_resolution,
                effective_fps: save_effective_fps,
                requested_duration_secs: Some(f32::from(requested_replay_secs)),
                selected_duration_secs: None,
                contiguous_duration_secs: None,
                partial_reason_code: None,
                anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
            };
        }

        let ttl_ms = self.default_pending_save_ttl_ms();
        let queue_outcome = self.enqueue_or_replace_pending_save_with_ttl(
            source.clone(),
            ttl_ms,
            anchor_time,
            PendingSaveReason::Retryable,
            requested_replay_secs,
        );
        let queued_message = match (blocker.code, queue_outcome) {
            ("busy", PendingSaveEnqueueOutcome::QueuedNew) => {
                "Save already in progress. Queued next replay.".to_string()
            }
            (
                "busy",
                PendingSaveEnqueueOutcome::ReplacedExisting {
                    previous_anchor_time,
                },
            ) => {
                self.append_capture_runtime_marker(&format!(
                    "phase: pending_save_anchor_updated reason=busy prev_epoch_ms={} next_epoch_ms={}",
                    to_epoch_ms(previous_anchor_time),
                    to_epoch_ms(anchor_time)
                ));
                "Save already in progress. Updated queued replay to latest trigger.".to_string()
            }
            (_, PendingSaveEnqueueOutcome::QueuedNew) => {
                format!(
                    "{} Replay queued; will save automatically.",
                    blocker.message
                )
            }
            (
                _,
                PendingSaveEnqueueOutcome::ReplacedExisting {
                    previous_anchor_time,
                },
            ) => {
                self.append_capture_runtime_marker(&format!(
                    "phase: pending_save_anchor_updated reason=retryable prev_epoch_ms={} next_epoch_ms={}",
                    to_epoch_ms(previous_anchor_time),
                    to_epoch_ms(anchor_time)
                ));
                "Replay queue updated to latest trigger while capture recovers.".to_string()
            }
        };
        self.append_save_trigger_marker(
            match queue_outcome {
                PendingSaveEnqueueOutcome::QueuedNew => "retryable_queued",
                PendingSaveEnqueueOutcome::ReplacedExisting { .. } => {
                    "retryable_updated_queued_anchor"
                }
            },
            trigger_source_label(&source),
        );
        events::emit_save_deferred(app, queued_message.clone());
        self.state.lock().save_stage = SaveStageDto::Queued;
        events::emit_engine_state(app, &self.get_engine_state());
        SaveReplayResultDto {
            ok: true,
            queued: true,
            clip: None,
            error: None,
            message: Some(queued_message),
            actual_duration_secs: None,
            audio_repaired: false,
            save_audio_strategy: None,
            smooth_pending: false,
            smooth_applied: false,
            smooth_error: None,
            effective_video_resolution: save_effective_video_resolution,
            effective_fps: save_effective_fps,
            requested_duration_secs: Some(f32::from(requested_replay_secs)),
            selected_duration_secs: None,
            contiguous_duration_secs: None,
            partial_reason_code: None,
            anchor_epoch_ms: Some(to_epoch_ms_i64(anchor_time)),
        }
    }

    pub(super) fn append_save_trigger_marker(&self, disposition: &str, source_label: &str) {
        self.append_capture_runtime_marker(&format!(
            "phase: save_trigger_received source={} disposition={}",
            source_label, disposition
        ));
    }

    pub(super) fn append_capture_runtime_marker(&self, marker: &str) {
        if let Some(handles) = self.pipeline.lock().as_ref() {
            handles.capture.append_runtime_marker(marker);
        }
    }
}
