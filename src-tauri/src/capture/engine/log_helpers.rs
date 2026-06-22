use super::*;

pub(super) fn read_capture_log_tail(path: &Path, max_lines: usize) -> Option<String> {
    let content = read_capture_log_from_offset(path, 0)?;
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }

    let start = lines.len().saturating_sub(max_lines);
    Some(lines[start..].join(" | "))
}

pub(super) fn read_capture_log_tail_since(
    path: &Path,
    offset: u64,
    max_lines: usize,
) -> Option<String> {
    let content = read_capture_log_from_offset(path, offset)?;
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }

    Some(lines[lines.len().saturating_sub(max_lines)..].join(" | "))
}

pub(super) enum StartupProbe {
    Ready,
    FfmpegExited(ExitStatus),
    HelperExited(ExitStatus),
    TimeoutNoSegments,
    TimeoutNoAudio,
}

pub(super) fn wait_for_first_stable_segment_or_exit(
    ffmpeg_child: &mut Child,
    helper_child: &mut Child,
    dir: &Path,
    session_id: &str,
    timeout: Duration,
    capture_log_path: &Path,
    attempt_log_offset: u64,
    require_system_audio_marker: bool,
) -> StartupProbe {
    let start = Instant::now();
    let mut timeout_limit = timeout;
    let mut extension_applied = false;
    let mut mic_extension_applied = false;

    loop {
        let stable_segment_ready = has_stable_segment_file_for_session(dir, Some(session_id));
        let segment_progress_ready = has_segment_progress_for_session(
            dir,
            Some(session_id),
            STARTUP_SEGMENT_PROGRESS_MIN_BYTES,
        );
        let system_audio_marker_ready =
            has_first_system_audio_frame_marker_in_log_since(capture_log_path, attempt_log_offset);
        let soft_ready_markers_present = has_startup_soft_ready_markers_in_log_since(
            capture_log_path,
            attempt_log_offset,
            require_system_audio_marker,
        );
        let soft_ready_from_progress =
            segment_progress_ready && (!require_system_audio_marker || system_audio_marker_ready);
        let soft_ready = soft_ready_markers_present || soft_ready_from_progress;
        if stable_segment_ready && (!require_system_audio_marker || system_audio_marker_ready) {
            if require_system_audio_marker {
                append_capture_log_line(capture_log_path, "phase: first_audio_path_ready");
            }
            append_capture_log_line(capture_log_path, "phase: first_stable_segment");
            append_capture_log_line(capture_log_path, "phase: first_segment_closed");
            return StartupProbe::Ready;
        }
        if soft_ready {
            if require_system_audio_marker {
                append_capture_log_line(capture_log_path, "phase: first_audio_path_ready");
            }
            append_capture_log_line(
                capture_log_path,
                "phase: startup_soft_ready_no_stable_segment_yet",
            );
            append_capture_log_line(capture_log_path, "phase: first_segment_closed");
            return StartupProbe::Ready;
        }

        match ffmpeg_child.try_wait() {
            Ok(Some(status)) => return StartupProbe::FfmpegExited(status),
            Ok(None) => {}
            Err(_) => return StartupProbe::FfmpegExited(exit_status_unknown_failure()),
        }

        match helper_child.try_wait() {
            Ok(Some(status)) => return StartupProbe::HelperExited(status),
            Ok(None) => {}
            Err(_) => return StartupProbe::HelperExited(exit_status_unknown_failure()),
        }

        if start.elapsed() >= timeout_limit {
            if !extension_applied && (soft_ready_markers_present || segment_progress_ready) {
                extension_applied = true;
                timeout_limit += Duration::from_millis(STARTUP_SOFT_READY_EXTENSION_MS);
                append_capture_log_line(
                    capture_log_path,
                    &format!(
                        "phase: startup_soft_ready_extension_applied extra_ms={STARTUP_SOFT_READY_EXTENSION_MS}"
                    ),
                );
                continue;
            }
            if !mic_extension_applied
                && has_first_mic_audio_frame_marker_in_log_since(
                    capture_log_path,
                    attempt_log_offset,
                )
            {
                mic_extension_applied = true;
                let extra = STARTUP_SOFT_READY_EXTENSION_MS;
                timeout_limit += Duration::from_millis(extra);
                append_capture_log_line(
                    capture_log_path,
                    &format!(
                        "phase: startup_mic_frames_flowing_extension_applied extra_ms={extra}"
                    ),
                );
                continue;
            }
            if require_system_audio_marker
                && (stable_segment_ready || segment_progress_ready)
                && !system_audio_marker_ready
            {
                return StartupProbe::TimeoutNoAudio;
            }
            if soft_ready_markers_present || soft_ready_from_progress {
                if require_system_audio_marker {
                    append_capture_log_line(capture_log_path, "phase: first_audio_path_ready");
                }
                append_capture_log_line(
                    capture_log_path,
                    "phase: startup_soft_ready_no_stable_segment_yet",
                );
                append_capture_log_line(capture_log_path, "phase: first_segment_closed");
                return StartupProbe::Ready;
            }
            return StartupProbe::TimeoutNoSegments;
        }

        thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn capture_startup_log_tails_since(
    path: &Path,
    offset: u64,
    max_lines: usize,
) -> (String, String, String) {
    let content = read_capture_log_from_offset(path, offset).unwrap_or_default();
    let lines: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if lines.is_empty() {
        return (
            "no helper log lines".to_string(),
            "no ffmpeg log lines".to_string(),
            String::new(),
        );
    }

    let helper_markers = [
        "sck helper args:",
        "ScreenCaptureKit",
        "stream start requested",
        "stream started",
        "first video frame delivered",
        "first system audio frame delivered",
        "first microphone audio frame delivered",
    ];
    let housekeeping_markers = [
        "=== backend:",
        "=== audio mode:",
        "ScreenCaptureKit config:",
        "phase:",
        "ffmpeg args:",
    ];

    let helper_lines: Vec<String> = lines
        .iter()
        .filter(|line| helper_markers.iter().any(|marker| line.contains(marker)))
        .cloned()
        .collect();
    let ffmpeg_lines: Vec<String> = lines
        .iter()
        .filter(|line| {
            !helper_markers.iter().any(|marker| line.contains(marker))
                && !housekeeping_markers
                    .iter()
                    .any(|marker| line.contains(marker))
        })
        .cloned()
        .collect();

    let helper_tail = tail_join(&helper_lines, max_lines);
    let ffmpeg_tail = tail_join(&ffmpeg_lines, max_lines);
    let combined_tail = tail_join(&lines, max_lines);

    (helper_tail, ffmpeg_tail, combined_tail)
}

pub(super) fn startup_timeout_reason_code(log_tail: &str) -> &'static str {
    if !log_tail.contains("stream started") && !log_tail.contains("first video frame delivered") {
        "helper_startup_stalled"
    } else if log_tail.contains("first video frame delivered")
        && !has_first_system_audio_frame_marker(log_tail)
        && (log_tail.contains("mode=system_only") || log_tail.contains("mode=system_plus_mic"))
    {
        "system_audio_startup_stalled"
    } else if (log_tail.contains("mode=system_plus_mic")
        || log_tail.contains("audio mode: system_plus_mic"))
        && log_tail.contains("first video frame delivered")
        && has_first_system_audio_frame_marker(log_tail)
        && !has_mic_path_ready_marker(log_tail)
    {
        "mic_pipe_startup_stalled"
    } else if (log_tail.contains("mode=system_plus_mic")
        || log_tail.contains("audio mode: system_plus_mic"))
        && log_tail.contains("first video frame delivered")
        && has_first_system_audio_frame_marker(log_tail)
        && !has_first_mic_audio_frame_marker(log_tail)
        && (log_tail.contains("phase: mic_backend_ready")
            || log_tail.contains("mic source format:")
            || log_tail.contains("mic converter configured:"))
    {
        "mic_first_frame_startup_stalled"
    } else if log_tail.contains("first video frame delivered")
        && !log_tail.contains("phase: first_stable_segment")
    {
        "ffmpeg_pipe_mux_stalled"
    } else {
        "startup_timeout"
    }
}

pub(super) fn startup_timeout_guidance(log_tail: &str) -> &'static str {
    match startup_timeout_reason_code(log_tail) {
        "helper_startup_stalled" => "no first video frame marker; helper startup likely stalled",
        "system_audio_startup_stalled" => {
            "first video frame seen but no system-audio marker; audio path likely stalled"
        }
        "mic_pipe_startup_stalled" => {
            "first video and system audio seen but no microphone path; mixed audio pipe startup likely stalled"
        }
        "mic_first_frame_startup_stalled" => {
            "microphone backend initialized but no first microphone frame reached ffmpeg; mixed mic startup likely stalled"
        }
        "ffmpeg_pipe_mux_stalled" => {
            "first video frame seen but no stable segment; ffmpeg pipe/mux path likely stalled"
        }
        _ => "startup timed out before first stable segment",
    }
}

pub(super) fn tail_join(lines: &[String], max_lines: usize) -> String {
    if lines.is_empty() {
        return "none".to_string();
    }
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join(" | ")
}

pub(super) fn read_capture_speed_x_since(path: &Path, offset: u64) -> Option<f32> {
    let content = read_capture_log_from_offset(path, offset)?;
    parse_latest_capture_speed_x(&content)
}

pub(super) fn parse_latest_capture_speed_x(content: &str) -> Option<f32> {
    let normalized = content.replace('\r', "\n");
    for token in normalized.split_whitespace().rev() {
        if let Some(value) = token.strip_prefix("speed=") {
            let parsed = value.trim_end_matches('x').trim();
            if let Ok(speed) = parsed.parse::<f32>() {
                return Some(speed);
            }
        }
    }
    None
}

pub(super) fn read_latest_mic_level_dbfs(path: &Path) -> Option<f32> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_mic_level_dbfs(&content)
}

pub(super) fn read_latest_mic_samples_per_sec(path: &Path) -> Option<u32> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_mic_samples_per_sec(&content)
}

pub(super) fn read_latest_mic_attach_runtime_state(path: &Path) -> Option<MicAttachRuntimeState> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_mic_attach_runtime_state(&content)
}

pub(super) fn read_latest_video_output_fps(path: &Path) -> Option<f32> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_video_output_fps(&content)
}

pub(super) fn read_latest_video_frame_drop_total(path: &Path) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_video_frame_drop_total(&content)
}

pub(super) fn read_latest_video_queue_overflow_count(path: &Path) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_video_queue_overflow_count(&content)
}

pub(super) fn read_latest_system_memory_pressure_level(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_system_memory_pressure_level(&content)
}

pub(super) fn read_latest_helper_thermal_state(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_helper_thermal_state(&content)
}

pub(super) fn read_latest_mic_recovery_state(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_mic_recovery_state(&content)
}

pub(super) fn read_latest_selected_microphone_name(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_selected_microphone_name(&content)
}

pub(super) fn read_latest_mic_backend_error(path: &Path) -> Option<(String, String)> {
    let content = fs::read_to_string(path).ok()?;
    parse_latest_mic_backend_error(&content)
}

pub(super) fn has_stable_segment_file_for_session(dir: &Path, session_id: Option<&str>) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.extension() != Some(OsStr::new(SEGMENT_EXTENSION)) {
            return false;
        }
        if let Some(expected_session) = session_id {
            if parse_segment_session_id(&path).as_deref() != Some(expected_session) {
                return false;
            }
        }

        let Ok(metadata) = entry.metadata() else {
            return false;
        };

        if metadata.len() < STARTUP_MIN_SEGMENT_BYTES {
            return false;
        }

        let Ok(modified) = metadata.modified() else {
            return false;
        };

        modified
            .elapsed()
            .map(|elapsed| elapsed >= Duration::from_millis(SEGMENT_STABLE_GRACE_MS))
            .unwrap_or(false)
    })
}

pub(super) fn has_segment_progress_for_session(
    dir: &Path,
    session_id: Option<&str>,
    min_size_bytes: u64,
) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.extension() != Some(OsStr::new(SEGMENT_EXTENSION)) {
            return false;
        }
        if let Some(expected_session) = session_id {
            if parse_segment_session_id(&path).as_deref() != Some(expected_session) {
                return false;
            }
        }
        entry
            .metadata()
            .map(|metadata| metadata.len() >= min_size_bytes)
            .unwrap_or(false)
    })
}

pub(super) fn has_first_audio_frame_marker(log: &str) -> bool {
    log.contains(FIRST_SYSTEM_AUDIO_MARKER)
        || log.contains(FIRST_MIC_AUDIO_MARKER)
        || log.contains("phase: system_audio_pipe_connected")
        || log.contains("phase: first_audio_path_ready")
}

pub(super) fn has_first_system_audio_frame_marker(log: &str) -> bool {
    log.contains(FIRST_SYSTEM_AUDIO_MARKER)
        || log.contains("phase: system_audio_pipe_connected")
        || log.contains("phase: first_audio_path_ready")
}

pub(super) fn has_first_video_frame_marker(log: &str) -> bool {
    log.contains(FIRST_VIDEO_MARKER)
}

pub(super) fn has_ffmpeg_segment_open_marker(log: &str) -> bool {
    log.lines()
        .any(|line| line.contains("Opening '") && line.contains(".mp4' for writing"))
}

pub(super) fn is_startup_interrupted_log(log: &str) -> bool {
    let lower = log.to_ascii_lowercase();
    let has_stop_marker = lower.contains("phase: stream_stopped_error");
    let has_interruption_code = lower.contains("code=-3805")
        || lower.contains("scstreamerrordomain code=-3805")
        || lower.contains("application connection being interrupted");
    has_stop_marker && has_interruption_code && !has_first_video_frame_marker(log)
}

pub(super) fn is_user_stopped_sharing_log(log: &str) -> bool {
    let lower = log.to_ascii_lowercase();
    if lower.contains("phase: stream_stop_user_intent")
        || lower.contains("code=-3817")
        || lower.contains("code=-3821")
    {
        return true;
    }
    let has_post_start_marker = has_first_video_frame_marker(log)
        || lower.contains("phase: first_segment_closed")
        || lower.contains("phase: first_stable_segment");
    (lower.contains("scstreamerrordomain code=-3805")
        || lower.contains("application connection being interrupted"))
        && has_post_start_marker
}

pub(super) fn has_startup_soft_ready_markers(log: &str, require_system_audio_marker: bool) -> bool {
    let system_audio_ready =
        !require_system_audio_marker || has_first_system_audio_frame_marker(log);
    has_first_video_frame_marker(log) && has_ffmpeg_segment_open_marker(log) && system_audio_ready
}

pub(super) fn has_startup_soft_ready_markers_in_log_since(
    path: &Path,
    offset: u64,
    require_system_audio_marker: bool,
) -> bool {
    let Some(content) = read_capture_log_from_offset(path, offset) else {
        return false;
    };
    has_startup_soft_ready_markers(&content, require_system_audio_marker)
}

pub(super) fn has_first_mic_audio_frame_marker(log: &str) -> bool {
    log.contains(FIRST_MIC_AUDIO_MARKER)
}

pub(super) fn has_mic_path_ready_marker(log: &str) -> bool {
    has_first_mic_audio_frame_marker(log)
        || log.contains(MIC_SILENCE_FILLER_ACTIVE_MARKER)
        || log.contains("phase: mic_audio_pipe_connected")
}

pub(super) fn has_mic_capture_session_running_marker(log: &str) -> bool {
    log.contains(MIC_CAPTURE_SESSION_RUNNING_MARKER)
}

pub(super) fn parse_segment_identity(path: &Path) -> Option<(String, u64)> {
    let name = path.file_name()?.to_str()?;
    if !name.starts_with("seg_") || !name.ends_with(".mp4") {
        return None;
    }
    let stem = name.strip_suffix(".mp4")?;
    let remainder = stem.strip_prefix("seg_")?;
    let (session_id, index) = remainder.rsplit_once('_')?;
    if session_id.is_empty() || index.is_empty() || !index.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((session_id.to_string(), index.parse().ok()?))
}

pub(super) fn parse_segment_session_id(path: &Path) -> Option<String> {
    parse_segment_identity(path).map(|(session_id, _)| session_id)
}

pub(super) fn parse_segment_index(path: &Path) -> Option<u64> {
    parse_segment_identity(path).map(|(_, index)| index)
}

pub(super) fn has_first_audio_frame_marker_in_log(path: &Path) -> bool {
    let Some(content) = read_capture_log_from_offset(path, 0) else {
        return false;
    };
    has_first_audio_frame_marker(&content)
}

pub(super) fn has_first_system_audio_frame_marker_in_log(path: &Path) -> bool {
    let Some(content) = read_capture_log_from_offset(path, 0) else {
        return false;
    };
    has_first_system_audio_frame_marker(&content)
}

pub(super) fn has_first_system_audio_frame_marker_in_log_since(path: &Path, offset: u64) -> bool {
    let Some(content) = read_capture_log_from_offset(path, offset) else {
        return false;
    };
    has_first_system_audio_frame_marker(&content)
}

pub(super) fn has_first_mic_audio_frame_marker_in_log_since(path: &Path, offset: u64) -> bool {
    let Some(content) = read_capture_log_from_offset(path, offset) else {
        return false;
    };
    has_first_mic_audio_frame_marker(&content)
}

pub(super) fn has_ffmpeg_queue_starvation_marker_in_log_since(path: &Path, offset: u64) -> bool {
    let Some(content) = read_capture_log_from_offset(path, offset) else {
        return false;
    };
    content.contains(FFMPEG_QUEUE_STARVATION_MARKER)
}

pub(super) fn has_first_mic_audio_frame_marker_in_log(path: &Path) -> bool {
    let Some(content) = read_capture_log_from_offset(path, 0) else {
        return false;
    };
    has_first_mic_audio_frame_marker(&content)
}

pub(super) fn has_mic_path_ready_marker_in_log(path: &Path) -> bool {
    let Some(content) = read_capture_log_from_offset(path, 0) else {
        return false;
    };
    has_mic_path_ready_marker(&content)
}

pub(super) fn has_mic_capture_session_running_marker_in_log(path: &Path) -> bool {
    let Some(content) = read_capture_log_from_offset(path, 0) else {
        return false;
    };
    has_mic_capture_session_running_marker(&content)
}

pub(super) fn has_mic_sustained_silence_marker_in_log(path: &Path) -> bool {
    let Some(content) = read_capture_log_from_offset(path, 0) else {
        return false;
    };
    content.contains(MIC_SUSTAINED_SILENCE_MARKER)
}

pub(super) fn build_system_plus_mic_mix_graph(
    mic_mix_gain_db: f32,
    system_volume_percent: u8,
    mic_noise_suppression: bool,
    rnnoise_model: Option<&str>,
) -> String {
    let gain = mic_mix_gain_db.clamp(0.0, 18.0);
    let denoise = if mic_noise_suppression {
        match rnnoise_model {
            Some(model) => format!("arnndn=m='{model}',"),
            None => "afftdn=nr=12:nf=-40,".to_string(),
        }
    } else {
        String::new()
    };
    let system_volume = system_volume_factor(system_volume_percent);
    format!(
        "[1:a]volume={system_volume:.3}[sys];[2:a]{denoise}volume={gain:.1}dB[mic];[sys][mic]amix=inputs=2:weights=1 1:duration=longest:dropout_transition=0:normalize=0,aresample=async=1:min_hard_comp=0.100:first_pts=0[aout]"
    )
}

pub(super) fn system_volume_factor(system_volume_percent: u8) -> f32 {
    f32::from(system_volume_percent.min(100)) / 100.0
}

pub(super) fn max_replay_history_age(buffer_duration_secs: u16) -> Duration {
    Duration::from_secs(u64::from(buffer_duration_secs).saturating_add(15))
}

pub(super) fn capture_log_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

pub(super) fn read_capture_log_from_offset(path: &Path, offset: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let start = offset.min(file_len);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    Some(content)
}

const LOG_TAIL_WINDOW_BYTES: u64 = 64 * 1024;

pub(super) fn read_capture_log_tail_window(path: &Path, min_offset: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let window_start = file_len.saturating_sub(LOG_TAIL_WINDOW_BYTES);
    let start = window_start.max(min_offset).min(file_len);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let mut content = String::from_utf8_lossy(&bytes).into_owned();
    if start > min_offset {
        if let Some(newline) = content.find('\n') {
            content.drain(..=newline);
        }
    }
    Some(content)
}

pub(super) fn read_capture_log_tail_bytes(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let start = file_len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let mut content = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0 {
        if let Some(newline) = content.find('\n') {
            content.drain(..=newline);
        }
    }
    Some(content)
}

#[derive(Default)]
pub(super) struct LogMetricsCache {
    pub last_refresh_at: Option<Instant>,
    pub capture_speed_x: Option<f32>,
    pub video_output_fps: Option<f32>,
    pub video_frame_drop_total: Option<u64>,
    pub video_queue_overflow_count: Option<u64>,
    pub mic_level_dbfs: Option<f32>,
    pub mic_samples_per_sec: Option<u32>,
    pub mic_attach_runtime_state: Option<MicAttachRuntimeState>,
    pub system_memory_pressure_level: Option<String>,
    pub helper_thermal_state: Option<String>,
    pub mic_recovery_state: Option<String>,
    pub selected_microphone_name: Option<String>,
    pub mic_backend_error: Option<(String, String)>,
    pub first_audio_frame_seen: bool,
    pub system_audio_path_ready: bool,
    pub mic_path_ready: bool,
    pub mic_frames_seen: bool,
    pub mic_capture_session_running: bool,
    pub mic_sustained_silence: bool,
    pub mic_selected_device_not_found: bool,
    pub queue_starvation_detected: bool,
}

pub(super) fn refresh_log_metrics_from_window(cache: &mut LogMetricsCache, log: &str) {
    if let Some(value) = parse_latest_capture_speed_x(log) {
        cache.capture_speed_x = Some(value);
    }
    if let Some(value) = parse_latest_video_output_fps(log) {
        cache.video_output_fps = Some(value);
    }
    if let Some(value) = parse_latest_video_frame_drop_total(log) {
        cache.video_frame_drop_total = Some(value);
    }
    if let Some(value) = parse_latest_video_queue_overflow_count(log) {
        cache.video_queue_overflow_count = Some(value);
    }
    if let Some(value) = parse_latest_mic_level_dbfs(log) {
        cache.mic_level_dbfs = Some(value);
    }
    if let Some(value) = parse_latest_mic_samples_per_sec(log) {
        cache.mic_samples_per_sec = Some(value);
    }
    if let Some(value) = parse_latest_mic_attach_runtime_state(log) {
        cache.mic_attach_runtime_state = Some(value);
    }
    if let Some(value) = parse_latest_system_memory_pressure_level(log) {
        cache.system_memory_pressure_level = Some(value);
    }
    if let Some(value) = parse_latest_helper_thermal_state(log) {
        cache.helper_thermal_state = Some(value);
    }
    if let Some(value) = parse_latest_mic_recovery_state(log) {
        cache.mic_recovery_state = Some(value);
    }
    if let Some(value) = parse_latest_mic_backend_error(log) {
        cache.mic_backend_error = Some(value);
    }
    if log.contains("device_name=") {
        cache.selected_microphone_name = parse_latest_selected_microphone_name(log);
    }
    cache.first_audio_frame_seen |= has_first_audio_frame_marker(log);
    cache.system_audio_path_ready |= has_first_system_audio_frame_marker(log);
    cache.mic_path_ready |= has_mic_path_ready_marker(log);
    cache.mic_frames_seen |= has_first_mic_audio_frame_marker(log);
    cache.mic_capture_session_running |= has_mic_capture_session_running_marker(log);
    cache.mic_sustained_silence |= log.contains(MIC_SUSTAINED_SILENCE_MARKER);
    cache.mic_selected_device_not_found |= has_mic_selected_device_not_found_marker(log);
    cache.queue_starvation_detected |= log.contains(FFMPEG_QUEUE_STARVATION_MARKER);
}
