use super::*;

impl CaptureEngine {
    pub fn replay_selection_for_save(
        &self,
        replay_duration_secs: u16,
    ) -> Result<Option<ReplaySelection>, String> {
        self.replay_selection_for_save_at(replay_duration_secs, SystemTime::now())
    }

    pub fn replay_selection_for_save_at(
        &self,
        replay_duration_secs: u16,
        anchor_time: SystemTime,
    ) -> Result<Option<ReplaySelection>, String> {
        let mut entries = self.recent_segment_entries()?;
        if entries.is_empty() {
            return Ok(None);
        }

        entries.retain(|entry| entry.modified <= anchor_time);
        entries.retain(|entry| {
            entry
                .modified
                .elapsed()
                .map(|elapsed| elapsed >= Duration::from_millis(SEGMENT_STABLE_GRACE_MS))
                .unwrap_or(false)
        });

        if entries.is_empty() {
            return Ok(None);
        }

        let requested_secs = f32::from(replay_duration_secs);
        let active_session_id = self.session_id.as_str();
        let active_anchor_index = entries.iter().rposition(|entry| {
            entry.modified <= anchor_time && entry.session_id.as_deref() == Some(active_session_id)
        });
        let Some(anchor_index) = active_anchor_index.or_else(|| {
            entries
                .iter()
                .rposition(|entry| entry.modified <= anchor_time)
        }) else {
            return Ok(None);
        };
        let anchor_is_active = active_anchor_index == Some(anchor_index);
        let horizon_start = system_time_sub(
            anchor_time,
            Duration::from_secs_f32(requested_secs.max(self.segment_duration_secs)),
        );
        let mut foreign_sessions_in_horizon: HashSet<String> = HashSet::new();
        for entry in &entries {
            if entry.modified < horizon_start || entry.modified > anchor_time {
                continue;
            }
            if entry.session_id.as_deref() != Some(active_session_id) {
                if let Some(session_id) = &entry.session_id {
                    foreign_sessions_in_horizon.insert(session_id.clone());
                }
            }
        }
        let foreign_session_count_in_horizon =
            foreign_sessions_in_horizon.len().min(u8::MAX as usize) as u8;
        let continuity_threshold =
            replay_continuity_gap_threshold(&entries, anchor_index, self.segment_duration_secs);
        let anchor_end = entries[anchor_index].modified;
        let anchor_gap = anchor_time
            .duration_since(anchor_end)
            .unwrap_or(Duration::ZERO);

        let mut selected_rev: Vec<&SegmentFile> = Vec::new();
        let mut available_secs = self.segment_duration_secs;
        selected_rev.push(&entries[anchor_index]);
        let mut current_end = anchor_end;
        let mut current_session_id = entries[anchor_index].session_id.clone();
        let mut current_segment_index = entries[anchor_index].segment_index;
        let mut session_boundary_count = 0u8;
        let mut discontinuity_gap_ms: Option<u64> = None;
        let mut partial_reason: Option<String> = None;
        let mut partial_reason_code: Option<String> = None;
        // OSS-REF(replay_window): keep replay selection anchored to the hotkey moment
        // and stop exactly at the first continuity break (session boundary or media gap).
        for idx in (0..anchor_index).rev() {
            let candidate = &entries[idx];
            if anchor_is_active && candidate.session_id.as_deref() != Some(active_session_id) {
                continue;
            }
            let delta = current_end
                .duration_since(candidate.modified)
                .unwrap_or(Duration::ZERO);
            let crosses_session_boundary = candidate.session_id != current_session_id;
            if crosses_session_boundary {
                if anchor_is_active {
                    continue;
                }
                session_boundary_count = session_boundary_count.saturating_add(1);
                discontinuity_gap_ms = Some(duration_to_ms_u64(delta));
                partial_reason =
                    Some("capture restart/session boundary in rolling buffer".to_string());
                partial_reason_code = Some("session_boundary".to_string());
                break;
            }
            if delta > continuity_threshold {
                discontinuity_gap_ms = Some(duration_to_ms_u64(delta));
                partial_reason =
                    Some("capture restart/discontinuity in rolling buffer".to_string());
                partial_reason_code = Some("timeline_gap".to_string());
                break;
            }
            let step_secs = match (current_segment_index, candidate.segment_index) {
                (Some(current_idx), Some(candidate_idx)) => {
                    if current_idx <= candidate_idx {
                        discontinuity_gap_ms = Some(duration_to_ms_u64(delta));
                        partial_reason =
                            Some("non-monotonic segment index near hotkey anchor".to_string());
                        partial_reason_code = Some("segment_index_order".to_string());
                        break;
                    }
                    let index_gap = current_idx - candidate_idx;
                    if index_gap != 1 {
                        discontinuity_gap_ms = Some(duration_to_ms_u64(delta));
                        partial_reason = Some("segment index gap in rolling buffer".to_string());
                        partial_reason_code = Some("segment_index_gap".to_string());
                        break;
                    }
                    self.segment_duration_secs * index_gap as f32
                }
                _ => {
                    let estimated = delta.as_secs_f32();
                    estimated.min(self.segment_duration_secs).max(0.0)
                }
            };
            available_secs += step_secs.max(0.0);
            selected_rev.push(candidate);
            current_end = candidate.modified;
            current_session_id = candidate.session_id.clone();
            current_segment_index = candidate.segment_index;
            if available_secs >= requested_secs {
                break;
            }
        }

        selected_rev.reverse();
        let selected: Vec<PathBuf> = selected_rev
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        if selected.is_empty() {
            return Ok(None);
        }

        let selected_secs = available_secs.max(self.segment_duration_secs);
        let contiguous_duration_secs = selected_secs;
        let mut target_trim_secs = requested_secs
            .min(selected_secs)
            .max(self.segment_duration_secs);
        // Keep trim targets aligned to contiguous segment windows when we're already
        // near the boundary to avoid aggressive edge trims.
        let near_window_boundary =
            (selected_secs - target_trim_secs) <= (self.segment_duration_secs * 0.5);
        if near_window_boundary {
            target_trim_secs = selected_secs;
        }
        let window_start = system_time_sub(anchor_end, Duration::from_secs_f32(target_trim_secs));
        let partial_history = target_trim_secs + f32::EPSILON < requested_secs;
        if partial_history && partial_reason.is_none() {
            if partial_reason_code.is_none() {
                partial_reason_code = Some(if anchor_gap > continuity_threshold {
                    "anchor_lag".to_string()
                } else {
                    "insufficient_contiguous_history".to_string()
                });
            }
            partial_reason = Some(if anchor_gap > continuity_threshold {
                format!(
                    "latest segment lags trigger by {:.1}s",
                    anchor_gap.as_secs_f32()
                )
            } else {
                "insufficient contiguous history at trigger time".to_string()
            });
        }
        if partial_history
            && anchor_is_active
            && foreign_session_count_in_horizon > 0
            && matches!(
                partial_reason_code.as_deref(),
                None | Some("anchor_lag") | Some("insufficient_contiguous_history")
            )
        {
            partial_reason = Some("capture restart/session boundary in rolling buffer".to_string());
            partial_reason_code = Some("session_boundary".to_string());
            session_boundary_count = session_boundary_count.max(foreign_session_count_in_horizon);
        }

        Ok(Some(ReplaySelection {
            segments: selected,
            available_secs: selected_secs,
            contiguous_duration_secs,
            target_trim_secs,
            window_start,
            window_end: anchor_end,
            partial_history,
            partial_reason,
            partial_reason_code,
            discontinuity_gap_ms,
            session_boundary_count,
        }))
    }

    pub fn freeze_pruning(&self) {
        self.prune_frozen.store(true, Ordering::Relaxed);
    }

    pub fn unfreeze_pruning(&self) {
        self.prune_frozen.store(false, Ordering::Relaxed);
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().clone()
    }

    pub fn startup_fallback_error(&self) -> Option<String> {
        self.startup_fallback_error.clone()
    }

    pub fn active_audio_mode(&self) -> &'static str {
        self.active_audio_mode.as_str()
    }

    pub fn capture_owner_pid(&self) -> Option<u32> {
        Some(self.capture_owner_pid)
    }

    pub fn concurrent_session_count(&self, replay_duration_secs: u16) -> Option<u8> {
        let entries = self.recent_segment_entries().ok()?;
        let horizon_start = system_time_sub(
            SystemTime::now(),
            Duration::from_secs(u64::from(replay_duration_secs.max(1))),
        );
        let mut sessions: HashSet<String> = HashSet::new();
        for entry in entries.iter().rev() {
            if entry.modified < horizon_start {
                break;
            }
            if let Some(session_id) = &entry.session_id {
                sessions.insert(session_id.clone());
                if sessions.len() >= u8::MAX as usize {
                    return Some(u8::MAX);
                }
            }
        }
        if sessions.is_empty() {
            None
        } else {
            Some(sessions.len() as u8)
        }
    }

    pub fn latest_segment_age_ms(&self) -> Option<u64> {
        let entries = self.segment_entries().ok()?;
        let modified = entries
            .iter()
            .rev()
            .find(|entry| entry.session_id.as_deref() == Some(self.session_id.as_str()))
            .map(|entry| entry.modified)?;
        let elapsed = modified.elapsed().ok()?;
        Some(elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
    }

    pub fn save_ready(&self) -> bool {
        self.recent_segment_entries()
            .map(|entries| {
                entries.iter().any(|entry| {
                    entry.size_bytes >= STARTUP_MIN_SEGMENT_BYTES
                        && entry
                            .modified
                            .elapsed()
                            .map(|elapsed| {
                                elapsed >= Duration::from_millis(SEGMENT_STABLE_GRACE_MS)
                            })
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    pub fn current_session_has_stable_segment(&self) -> bool {
        self.segment_entries()
            .map(|entries| {
                entries.iter().any(|entry| {
                    entry.session_id.as_deref() == Some(self.session_id.as_str())
                        && entry.size_bytes >= STARTUP_MIN_SEGMENT_BYTES
                        && entry
                            .modified
                            .elapsed()
                            .map(|elapsed| {
                                elapsed >= Duration::from_millis(SEGMENT_STABLE_GRACE_MS)
                            })
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    pub fn first_audio_frame_seen(&self) -> bool {
        has_first_audio_frame_marker_in_log(&self.capture_log_path)
    }

    pub fn system_audio_path_ready(&self) -> bool {
        if !self.active_audio_mode.has_audio() {
            return false;
        }
        has_first_system_audio_frame_marker_in_log(&self.capture_log_path)
    }

    pub fn mic_path_ready(&self) -> bool {
        if !self.active_audio_mode.has_mic() {
            return false;
        }
        has_mic_path_ready_marker_in_log(&self.capture_log_path)
    }

    pub fn mic_frames_seen(&self) -> bool {
        if !self.active_audio_mode.has_mic() {
            return false;
        }
        has_first_mic_audio_frame_marker_in_log(&self.capture_log_path)
    }

    pub fn mic_level_dbfs(&self) -> Option<f32> {
        if !self.active_audio_mode.has_mic() {
            return None;
        }
        read_latest_mic_level_dbfs(&self.capture_log_path)
    }

    pub fn mic_capture_session_running(&self) -> bool {
        if !self.active_audio_mode.has_mic() {
            return false;
        }
        has_mic_capture_session_running_marker_in_log(&self.capture_log_path)
    }

    pub fn mic_samples_per_sec(&self) -> Option<u32> {
        if !self.active_audio_mode.has_mic() {
            return None;
        }
        read_latest_mic_samples_per_sec(&self.capture_log_path)
    }

    pub fn mic_attach_runtime_state(&self) -> Option<MicAttachRuntimeState> {
        if !self.active_audio_mode.has_mic() {
            return None;
        }
        read_latest_mic_attach_runtime_state(&self.capture_log_path)
    }

    pub fn mic_sustained_silence(&self) -> bool {
        if !self.active_audio_mode.has_mic() {
            return false;
        }
        has_mic_sustained_silence_marker_in_log(&self.capture_log_path)
    }

    pub fn audio_path_ready(&self) -> bool {
        self.system_audio_path_ready()
    }

    pub fn mic_backend_in_use(&self) -> &str {
        self.mic_backend_in_use.as_str()
    }

    pub fn capture_speed_x(&self) -> Option<f32> {
        read_capture_speed_x_since(&self.capture_log_path, self.capture_log_offset)
    }

    pub fn effective_output_fps(&self) -> Option<f32> {
        read_latest_video_output_fps(&self.capture_log_path)
    }

    pub fn capture_dropped_frames(&self) -> u64 {
        read_latest_video_frame_drop_total(&self.capture_log_path).unwrap_or(0)
    }

    pub fn capture_queue_overflows(&self) -> u64 {
        read_latest_video_queue_overflow_count(&self.capture_log_path).unwrap_or(0)
    }

    pub fn system_memory_pressure_level(&self) -> Option<String> {
        read_latest_system_memory_pressure_level(&self.capture_log_path)
    }

    pub fn helper_thermal_state(&self) -> Option<String> {
        read_latest_helper_thermal_state(&self.capture_log_path)
    }

    pub fn mic_recovery_state(&self) -> Option<String> {
        if !self.active_audio_mode.has_mic() {
            return None;
        }
        read_latest_mic_recovery_state(&self.capture_log_path)
    }

    pub fn selected_microphone_name(&self) -> Option<String> {
        if !self.active_audio_mode.has_mic() {
            return None;
        }
        read_latest_selected_microphone_name(&self.capture_log_path)
    }

    pub fn mic_selected_device_not_found(&self) -> bool {
        if !self.active_audio_mode.has_mic() {
            return false;
        }
        let Some(content) = read_capture_log_from_offset(&self.capture_log_path, 0) else {
            return false;
        };
        has_mic_selected_device_not_found_marker(&content)
    }

    pub fn last_mic_backend_error(&self) -> Option<(String, String)> {
        if !self.active_audio_mode.has_mic() {
            return None;
        }
        read_latest_mic_backend_error(&self.capture_log_path)
    }

    pub fn playback_realtime_x(&self) -> Option<f32> {
        let entries = self.recent_segment_entries().ok()?;
        let mut current_session_entries: Vec<&SegmentFile> = entries
            .iter()
            .filter(|entry| entry.session_id.as_deref() == Some(self.session_id.as_str()))
            .collect();
        if current_session_entries.len() < 2 {
            return None;
        }

        let window_size = 12usize;
        if current_session_entries.len() > window_size {
            current_session_entries =
                current_session_entries[current_session_entries.len() - window_size..].to_vec();
        }

        let mut delta_sum_secs = 0.0f32;
        let mut delta_count = 0usize;
        for window in current_session_entries.windows(2) {
            let prev = window[0];
            let next = window[1];
            let Ok(delta) = next.modified.duration_since(prev.modified) else {
                continue;
            };
            let delta_secs = delta.as_secs_f32();
            if delta_secs <= 0.0 {
                continue;
            }
            delta_sum_secs += delta_secs;
            delta_count += 1;
        }

        if delta_count == 0 {
            return None;
        }
        let mean_delta_secs = delta_sum_secs / delta_count as f32;
        if mean_delta_secs <= 0.0 {
            return None;
        }

        Some((self.segment_duration_secs / mean_delta_secs).clamp(0.05, 3.0))
    }

    pub fn playback_stability(&self) -> &'static str {
        if !self.current_session_has_stable_segment() {
            return "recovering";
        }
        match self.playback_realtime_x() {
            Some(value) if (0.97..=1.03).contains(&value) => "stable",
            Some(_) => "drifting",
            None => "recovering",
        }
    }

    pub fn queue_starvation_detected(&self) -> bool {
        has_ffmpeg_queue_starvation_marker_in_log_since(
            &self.capture_log_path,
            self.capture_log_offset,
        )
    }

    pub fn live_queue_profile(&self) -> LiveQueueProfile {
        self.queue_profile
    }

    pub fn append_runtime_marker(&self, marker: &str) {
        append_capture_log_line(&self.capture_log_path, marker);
    }

    pub fn has_display_changed(&self, debounce: Duration) -> bool {
        let current = current_display_signature();
        if current == 0 || self.display_signature == 0 {
            return false;
        }

        let mut seen = self.display_change_seen_at.lock();
        if current != self.display_signature {
            if let Some(first_seen) = *seen {
                return first_seen.elapsed() >= debounce;
            }
            *seen = Some(Instant::now());
            return false;
        }

        *seen = None;
        false
    }

    pub fn capture_log_tail(&self, lines: usize) -> Option<String> {
        read_capture_log_tail(&self.capture_log_path, lines)
    }

    pub fn warmup_reason(&self) -> String {
        format!(
            "Replay buffer warming up ({}ms segments).",
            (self.segment_duration_secs * 1_000.0) as u16
        )
    }

    pub(super) fn segment_entries(&self) -> Result<Vec<SegmentFile>, String> {
        let mut files = Vec::new();
        let entries = fs::read_dir(&self.segment_dir).map_err(|err| {
            map_segment_dir_io_error("failed to read live segment directory", err)
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some(OsStr::new(SEGMENT_EXTENSION)) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let session_id = parse_segment_session_id(&path);
            let segment_index = parse_segment_index(&path);
            files.push(SegmentFile {
                path,
                modified,
                size_bytes: metadata.len(),
                session_id,
                segment_index,
            });
        }

        files.sort_by_key(|entry| entry.modified);
        Ok(files)
    }

    pub(super) fn recent_segment_entries(&self) -> Result<Vec<SegmentFile>, String> {
        let max_age = max_replay_history_age(self.buffer_duration_secs);
        self.segment_entries().map(|entries| {
            entries
                .into_iter()
                .filter(|entry| {
                    entry
                        .modified
                        .elapsed()
                        .map(|age| age <= max_age)
                        .unwrap_or(false)
                })
                .collect()
        })
    }

    pub(super) fn recent_segment_files(&self) -> Result<Vec<PathBuf>, String> {
        self.recent_segment_entries()
            .map(|entries| entries.into_iter().map(|entry| entry.path).collect())
    }
}
