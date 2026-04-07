use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AudioAttemptPlan {
    pub mode: AudioMode,
    pub mic_backend: String,
}

pub(super) fn requested_audio_modes(
    settings: &SettingsDto,
    startup_strategy: AudioStartupStrategy,
) -> Vec<AudioMode> {
    let _ = startup_strategy;
    let mut modes = match settings.audio_mode.as_str() {
        "video_only" => vec![AudioMode::VideoOnly],
        "system_plus_mic"
            if settings.mic_enabled && settings.mic_failure_policy.as_str() == "required" =>
        {
            vec![AudioMode::SystemPlusMic]
        }
        // Keep mixed mode first for best-effort mic so mic can attach without restart.
        "system_plus_mic" if settings.mic_enabled => {
            vec![AudioMode::SystemPlusMic, AudioMode::SystemOnly]
        }
        "system_plus_mic" => vec![AudioMode::SystemOnly],
        "system_only" => vec![AudioMode::SystemOnly],
        _ => vec![AudioMode::SystemOnly],
    };

    if settings.audio_fallback_policy == "allow_video_only"
        && !modes
            .iter()
            .any(|mode| matches!(mode, AudioMode::VideoOnly))
    {
        modes.push(AudioMode::VideoOnly);
    }

    modes
}

pub(super) fn requested_audio_attempts(
    settings: &SettingsDto,
    startup_strategy: AudioStartupStrategy,
) -> Vec<AudioAttemptPlan> {
    let _ = startup_strategy;
    let normalized_backend = normalize_mic_backend_label(&settings.mic_capture_backend);
    let mut attempts: Vec<AudioAttemptPlan> = Vec::new();

    for mode in requested_audio_modes(settings, startup_strategy) {
        if mode == AudioMode::SystemPlusMic && settings.mic_enabled {
            match normalized_backend.as_str() {
                "auto" => {
                    attempts.push(AudioAttemptPlan {
                        mode,
                        mic_backend: "sck_native".to_string(),
                    });
                    attempts.push(AudioAttemptPlan {
                        mode,
                        mic_backend: "avcapture".to_string(),
                    });
                }
                backend => attempts.push(AudioAttemptPlan {
                    mode,
                    mic_backend: backend.to_string(),
                }),
            }
        } else {
            attempts.push(AudioAttemptPlan {
                mode,
                mic_backend: "none".to_string(),
            });
        }
    }

    attempts
}

pub(super) fn normalize_mic_backend_label(value: &str) -> String {
    match value {
        "sck_experimental" => "sck_native".to_string(),
        "auto" | "avcapture" | "sck_native" => value.to_string(),
        _ => "auto".to_string(),
    }
}

pub(super) fn resolve_ffmpeg_binary() -> String {
    if let Ok(bin) = std::env::var("REWINDER_FFMPEG_BIN") {
        if !bin.trim().is_empty() {
            return bin;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(contents_dir) = exe.parent().and_then(|p| p.parent()) {
            let bundled = contents_dir.join("Resources").join("bin").join("ffmpeg");
            if bundled.exists() {
                return bundled.to_string_lossy().to_string();
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let dev_bundled = cwd.join("src-tauri").join("bin").join("ffmpeg");
        if dev_bundled.exists() {
            return dev_bundled.to_string_lossy().to_string();
        }
    }

    if Path::new("/opt/homebrew/bin/ffmpeg").exists() {
        return "/opt/homebrew/bin/ffmpeg".to_string();
    }

    "ffmpeg".to_string()
}

pub(super) fn resolve_sck_helper_binary() -> Result<String, String> {
    if let Ok(bin) = std::env::var("REWINDER_SCK_HELPER_BIN") {
        if !bin.trim().is_empty() {
            if Path::new(&bin).exists() {
                return Ok(bin);
            }
            return Err(format!(
                "REWINDER_SCK_HELPER_BIN is set but missing: {}",
                bin
            ));
        }
    }

    if let Some(compiled) = option_env!("REWINDER_SCK_HELPER_PATH") {
        if !compiled.trim().is_empty() && Path::new(compiled).exists() {
            return Ok(compiled.to_string());
        }
    }

    Err("ScreenCaptureKit helper binary was not built. Rebuild the app and retry.".to_string())
}

pub(super) fn start_capture_via_sck_bridge(
    ffmpeg_bin: &str,
    helper_bin: &str,
    settings: &SettingsDto,
    segment_dir: &Path,
    capture_log_path: &Path,
    segment_duration_secs: f32,
    mode: AudioMode,
    mic_backend: &str,
    queue_profile: LiveQueueProfile,
    attempt_index: usize,
) -> Result<CaptureStartup, String> {
    let session_id = generate_capture_session_id();
    let (width, height) = capture_dimensions(settings.video_resolution);
    let fps = settings.fps.max(1);
    let display_index = std::env::var("REWINDER_DISPLAY_INDEX")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    append_capture_log_line(
        capture_log_path,
        &format!(
            "ScreenCaptureKit config: display_index={} size={}x{} fps={} mode={} mic_backend={}",
            display_index,
            width,
            height,
            fps,
            mode.as_str(),
            mic_backend
        ),
    );

    let pipes = create_capture_pipes(segment_dir, mode)?;
    let attempt_log_offset = capture_log_size(capture_log_path);

    let ffmpeg_args = build_capture_args_from_pipes(
        settings,
        segment_dir,
        &session_id,
        width,
        height,
        segment_duration_secs,
        &pipes,
        mode,
        queue_profile,
    );
    append_capture_log_line(
        capture_log_path,
        &format!("ffmpeg args: {}", ffmpeg_args.join(" ")),
    );
    if mode == AudioMode::SystemPlusMic {
        let mix_graph = build_system_plus_mic_mix_graph(settings.mic_mix_gain_db);
        append_capture_log_line(capture_log_path, &format!("mix graph: {mix_graph}"));
    }

    let mut ffmpeg_child = spawn_ffmpeg_encoder_child(ffmpeg_bin, &ffmpeg_args, capture_log_path)?;
    append_capture_log_line(capture_log_path, "phase: ffmpeg_spawned");

    let mut helper_child = match spawn_sck_helper_child(
        helper_bin,
        settings,
        width,
        height,
        fps,
        display_index,
        &pipes,
        mode,
        mic_backend,
        capture_log_path,
    ) {
        Ok(child) => child,
        Err(err) => {
            terminate_child_gracefully(&mut ffmpeg_child);
            let _ = ffmpeg_child.wait();
            cleanup_pipes(&pipes);
            return Err(err);
        }
    };
    append_capture_log_line(capture_log_path, "phase: helper_spawned");

    let startup_probe_timeout = startup_probe_timeout_for_mode(mode, settings, attempt_index);
    let require_system_audio_marker = settings.audio_fallback_policy == "system_only_fallback"
        && settings.audio_mode != "video_only"
        && mode.has_audio();
    match wait_for_first_stable_segment_or_exit(
        &mut ffmpeg_child,
        &mut helper_child,
        segment_dir,
        &session_id,
        startup_probe_timeout,
        capture_log_path,
        attempt_log_offset,
        require_system_audio_marker,
    ) {
        StartupProbe::Ready => Ok(CaptureStartup {
            ffmpeg_child,
            helper_child,
            active_audio_mode: mode,
            mic_backend_in_use: mic_backend.to_string(),
            pipes,
            session_id,
            attempt_log_offset,
        }),
        StartupProbe::FfmpegExited(status) => {
            terminate_child_gracefully(&mut helper_child);
            let _ = helper_child.wait();
            cleanup_pipes(&pipes);
            let log_tail = read_capture_log_tail_since(capture_log_path, attempt_log_offset, 40)
                .unwrap_or_default();
            Err(format!(
                "ffmpeg encoder exited during startup with status {status} (mode={}). log: {log_tail}",
                mode.as_str()
            ))
        }
        StartupProbe::HelperExited(status) => {
            terminate_child_gracefully(&mut ffmpeg_child);
            let _ = ffmpeg_child.wait();
            cleanup_pipes(&pipes);
            let log_tail = read_capture_log_tail_since(capture_log_path, attempt_log_offset, 40)
                .unwrap_or_default();
            if helper_exit_indicates_user_stopped_sharing(status.code(), &log_tail) {
                append_capture_log_line(
                    capture_log_path,
                    "phase: startup_user_stopped_sharing_sc3805",
                );
                return Err(format!(
                    "user_stopped_sharing: ScreenCaptureKit capture was stopped by macOS screen-recording controls during startup (status {status}, mode={}). log: {log_tail}",
                    mode.as_str()
                ));
            }
            if is_startup_interrupted_log(&log_tail) {
                append_capture_log_line(capture_log_path, "phase: startup_interrupted_sc3805");
                return Err(format!(
                    "capture_start_interrupted: ScreenCaptureKit startup interrupted (status {status}, mode={}). log: {log_tail}",
                    mode.as_str()
                ));
            }
            Err(format!(
                "ScreenCaptureKit helper exited during startup with status {status} (mode={}). log: {log_tail}",
                mode.as_str()
            ))
        }
        StartupProbe::TimeoutNoSegments => {
            let (helper_tail, ffmpeg_tail, combined_tail) =
                capture_startup_log_tails_since(capture_log_path, attempt_log_offset, 60);
            let reason_code = startup_timeout_reason_code(&combined_tail);
            append_capture_log_line(
                capture_log_path,
                &format!("phase: startup_timeout_reason={reason_code}"),
            );
            terminate_child_gracefully(&mut ffmpeg_child);
            let _ = ffmpeg_child.wait();
            terminate_child_gracefully(&mut helper_child);
            let _ = helper_child.wait();
            cleanup_pipes(&pipes);
            let guidance = startup_timeout_guidance(&combined_tail);
            Err(format!(
                "capture_start_timeout: reason_code={reason_code} ScreenCaptureKit pipeline produced no stable segments within {:?} (mode={}). guidance: {}. helper_tail: {}. ffmpeg_tail: {}",
                startup_probe_timeout,
                mode.as_str(),
                guidance,
                helper_tail,
                ffmpeg_tail
                ))
        }
        StartupProbe::TimeoutNoAudio => {
            append_capture_log_line(capture_log_path, "phase: startup_timeout_reason=no_audio");
            terminate_child_gracefully(&mut ffmpeg_child);
            let _ = ffmpeg_child.wait();
            terminate_child_gracefully(&mut helper_child);
            let _ = helper_child.wait();
            cleanup_pipes(&pipes);
            let (helper_tail, ffmpeg_tail, combined_tail) =
                capture_startup_log_tails_since(capture_log_path, attempt_log_offset, 60);
            let guidance = startup_timeout_guidance(&combined_tail);
            Err(format!(
                "audio_start_timeout: ScreenCaptureKit produced video segments but no first audio frame marker within {:?} (mode={}). guidance: {}. helper_tail: {}. ffmpeg_tail: {}",
                startup_probe_timeout,
                mode.as_str(),
                guidance,
                helper_tail,
                ffmpeg_tail
            ))
        }
    }
}

pub(super) fn helper_exit_indicates_user_stopped_sharing(
    status_code: Option<i32>,
    log_tail: &str,
) -> bool {
    if status_code != Some(HELPER_INTERRUPTED_EXIT_CODE) {
        return false;
    }

    let lower = log_tail.to_ascii_lowercase();
    let has_stream_stop_signature = lower.contains("phase: stream_stopped_error")
        || lower.contains("phase: stream_stop_details")
        || lower.contains("phase: stream_stop_classified")
        || lower.contains("phase: stream_inactive_watchdog_triggered")
        || lower.contains("application connection being interrupted")
        || lower.contains("scstreamerrordomain code=-3805");

    has_stream_stop_signature || is_user_stopped_sharing_log(log_tail)
}

pub(super) fn startup_probe_timeout_for_mode(
    mode: AudioMode,
    settings: &SettingsDto,
    attempt_index: usize,
) -> Duration {
    match mode {
        AudioMode::SystemPlusMic | AudioMode::SystemOnly => {
            let mut timeout_ms = u64::from(settings.audio_startup_timeout_ms.clamp(2_000, 20_000));
            if attempt_index == 0 {
                timeout_ms = (timeout_ms + STARTUP_FIRST_ATTEMPT_EXTRA_TIMEOUT_MS).min(24_000);
            }
            if mode == AudioMode::SystemPlusMic {
                timeout_ms = (timeout_ms + STARTUP_MIC_MIX_EXTRA_TIMEOUT_MS).min(30_000);
            }
            Duration::from_millis(timeout_ms)
        }
        AudioMode::VideoOnly => {
            let mut timeout_ms = 6_000_u64;
            if attempt_index == 0 {
                timeout_ms += STARTUP_FIRST_ATTEMPT_EXTRA_TIMEOUT_MS;
            }
            Duration::from_millis(timeout_ms)
        }
    }
}

pub(super) fn create_capture_pipes(
    segment_dir: &Path,
    mode: AudioMode,
) -> Result<CapturePipes, String> {
    let video_pipe = segment_dir.join("video.pipe");
    ensure_fifo(&video_pipe)?;

    let system_audio_pipe = if mode.has_audio() {
        let path = segment_dir.join("system_audio.pipe");
        ensure_fifo(&path)?;
        Some(path)
    } else {
        None
    };

    let mic_audio_pipe = if mode.has_mic() {
        let path = segment_dir.join("mic_audio.pipe");
        ensure_fifo(&path)?;
        Some(path)
    } else {
        None
    };

    Ok(CapturePipes {
        video_pipe,
        system_audio_pipe,
        mic_audio_pipe,
    })
}

pub(super) fn cleanup_pipes(pipes: &CapturePipes) {
    let _ = fs::remove_file(&pipes.video_pipe);
    if let Some(path) = &pipes.system_audio_pipe {
        let _ = fs::remove_file(path);
    }
    if let Some(path) = &pipes.mic_audio_pipe {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn ensure_fifo(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|err| {
            format!(
                "failed to remove existing pipe {}: {err}",
                path.to_string_lossy()
            )
        })?;
    }

    let status = Command::new("mkfifo")
        .arg(path)
        .status()
        .map_err(|err| format!("failed to run mkfifo for {}: {err}", path.to_string_lossy()))?;

    if !status.success() {
        return Err(format!(
            "mkfifo failed for {} with status {}",
            path.to_string_lossy(),
            status
        ));
    }

    Ok(())
}

pub(super) fn capture_dimensions(height: u16) -> (u16, u16) {
    match height {
        360 => (640, 360),
        480 => (854, 480),
        540 => (960, 540),
        720 => (1280, 720),
        _ => (1920, 1080),
    }
}

pub(super) fn spawn_sck_helper_child(
    helper_bin: &str,
    settings: &SettingsDto,
    width: u16,
    height: u16,
    fps: u16,
    display_index: usize,
    pipes: &CapturePipes,
    mode: AudioMode,
    mic_backend: &str,
    capture_log_path: &Path,
) -> Result<Child, String> {
    let stderr_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(capture_log_path)
        .map_err(|err| format!("failed to open capture log file: {err}"))?;

    let mut args = vec![
        "--width".to_string(),
        width.to_string(),
        "--height".to_string(),
        height.to_string(),
        "--fps".to_string(),
        fps.to_string(),
        "--display-index".to_string(),
        display_index.to_string(),
        "--video-pipe".to_string(),
        pipes.video_pipe.to_string_lossy().to_string(),
        "--enable-system-audio".to_string(),
        if mode.has_audio() { "1" } else { "0" }.to_string(),
        "--enable-mic".to_string(),
        if mode.has_mic() { "1" } else { "0" }.to_string(),
        "--audio-sample-rate".to_string(),
        settings.audio_sample_rate_hz.to_string(),
        "--audio-channels".to_string(),
        settings.audio_channels.to_string(),
        "--exclude-current-process-audio".to_string(),
        if settings.exclude_current_process_audio {
            "1"
        } else {
            "0"
        }
        .to_string(),
        "--mic-backend".to_string(),
        mic_backend.to_string(),
        "--mic-retry-interval-secs".to_string(),
        settings.mic_retry_interval_secs.to_string(),
    ];

    if let Some(path) = &pipes.system_audio_pipe {
        args.push("--audio-pipe".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    if let Some(path) = &pipes.mic_audio_pipe {
        args.push("--mic-pipe".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    if let Some(selected_microphone_id) = &settings.selected_microphone_id {
        if !selected_microphone_id.trim().is_empty() {
            args.push("--selected-microphone-id".to_string());
            args.push(selected_microphone_id.clone());
        }
    }

    if settings.mic_auto_boost_volume && mode.has_mic() {
        args.push("--boost-mic-volume".to_string());
        args.push("1".to_string());
    }

    append_capture_log_line(
        capture_log_path,
        &format!("sck helper args: {}", args.join(" ")),
    );

    Command::new(helper_bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|err| format!("failed to start ScreenCaptureKit helper: {err}"))
}

pub(super) fn build_capture_args_from_pipes(
    settings: &SettingsDto,
    segment_dir: &Path,
    session_id: &str,
    width: u16,
    height: u16,
    segment_duration_secs: f32,
    pipes: &CapturePipes,
    mode: AudioMode,
    queue_profile: LiveQueueProfile,
) -> Vec<String> {
    let segment_pattern = segment_dir.join(format!("seg_{session_id}_%08d.mp4"));
    let segment_delta = segment_time_delta_for_fps(settings.fps);
    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "info".to_string(),
        "-probesize".to_string(),
        "32".to_string(),
        "-analyzeduration".to_string(),
        "0".to_string(),
        "-thread_queue_size".to_string(),
        queue_profile.video_thread_queue_size().to_string(),
        "-f".to_string(),
        "rawvideo".to_string(),
        "-pix_fmt".to_string(),
        "nv12".to_string(),
        "-video_size".to_string(),
        format!("{width}x{height}"),
        "-framerate".to_string(),
        settings.fps.max(1).to_string(),
        "-i".to_string(),
        pipes.video_pipe.to_string_lossy().to_string(),
    ];

    if let Some(path) = &pipes.system_audio_pipe {
        args.extend([
            "-thread_queue_size".to_string(),
            queue_profile.audio_thread_queue_size().to_string(),
            "-f".to_string(),
            "f32le".to_string(),
            "-ar".to_string(),
            settings.audio_sample_rate_hz.to_string(),
            "-ac".to_string(),
            settings.audio_channels.to_string(),
            "-i".to_string(),
            path.to_string_lossy().to_string(),
        ]);
    }

    if let Some(path) = &pipes.mic_audio_pipe {
        args.extend([
            "-thread_queue_size".to_string(),
            queue_profile.audio_thread_queue_size().to_string(),
            "-f".to_string(),
            "f32le".to_string(),
            "-ar".to_string(),
            settings.audio_sample_rate_hz.to_string(),
            "-ac".to_string(),
            settings.audio_channels.to_string(),
            "-i".to_string(),
            path.to_string_lossy().to_string(),
        ]);
    }

    args.extend(VideoEncoder::ffmpeg_args(settings));
    args.extend(["-fps_mode".to_string(), "cfr".to_string()]);

    match mode {
        AudioMode::VideoOnly => {
            args.extend(AudioEncoder::ffmpeg_args(settings, false));
        }
        AudioMode::SystemOnly => {
            args.extend([
                "-map".to_string(),
                "0:v:0".to_string(),
                "-map".to_string(),
                "1:a:0".to_string(),
                "-af".to_string(),
                "aresample=async=1:first_pts=0".to_string(),
            ]);
            args.extend(AudioEncoder::ffmpeg_args(settings, true));
        }
        AudioMode::SystemPlusMic => {
            let mix_graph = build_system_plus_mic_mix_graph(settings.mic_mix_gain_db);
            args.extend([
                "-filter_complex".to_string(),
                mix_graph,
                "-map".to_string(),
                "0:v:0".to_string(),
                "-map".to_string(),
                "[aout]".to_string(),
            ]);
            args.extend(AudioEncoder::ffmpeg_args(settings, true));
        }
    }

    args.extend([
        "-f".to_string(),
        "segment".to_string(),
        "-segment_time".to_string(),
        format!("{segment_duration_secs}"),
        "-segment_time_delta".to_string(),
        format!("{segment_delta:.6}"),
        "-reset_timestamps".to_string(),
        "0".to_string(),
        "-segment_format".to_string(),
        "mp4".to_string(),
        segment_pattern.to_string_lossy().to_string(),
    ]);
    args
}

pub(super) fn spawn_ffmpeg_encoder_child(
    ffmpeg_bin: &str,
    args: &[String],
    capture_log_path: &Path,
) -> Result<Child, String> {
    let stderr_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(capture_log_path)
        .map_err(|err| format!("failed to create capture log file: {err}"))?;

    Command::new(ffmpeg_bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .map_err(|err| format!("failed to start ffmpeg encoder: {err}"))
}

pub(super) fn prune_old_segments(
    dir: &Path,
    buffer_duration_secs: u16,
    segment_duration_secs: f32,
) -> Result<(), String> {
    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();

    for entry in fs::read_dir(dir)
        .map_err(|err| map_segment_dir_io_error("failed to list segment dir", err))?
    {
        let Ok(entry) = entry else {
            continue;
        };
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

        files.push((path, modified));
    }

    files.sort_by_key(|(_, modified)| *modified);

    let keep_count = keep_segment_count_for_duration(buffer_duration_secs, segment_duration_secs)
        + RETENTION_MARGIN_SEGMENTS;
    if files.len() <= keep_count {
        return Ok(());
    }

    let remove_count = files.len() - keep_count;
    for (path, _) in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }

    Ok(())
}

pub(super) fn map_segment_dir_io_error(context: &str, err: std::io::Error) -> String {
    if matches!(err.kind(), std::io::ErrorKind::PermissionDenied) {
        return format!("output_dir_permission_required: {context}: {err}");
    }
    format!("{context}: {err}")
}

pub(super) fn is_output_dir_permission_error(err: &str) -> bool {
    err.starts_with("output_dir_permission_required:")
}

pub(super) fn strip_output_dir_permission_prefix(err: &str) -> String {
    err.strip_prefix("output_dir_permission_required:")
        .map(str::trim)
        .unwrap_or(err)
        .to_string()
}
