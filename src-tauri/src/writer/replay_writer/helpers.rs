use super::*;

pub(super) fn run_fast_save_pipeline(
    ffmpeg_bin: &str,
    _settings: &SettingsDto,
    list_file: &Path,
    concat_mp4: &Path,
    output_path: &Path,
    _target_trim_secs: f32,
) -> Result<(), String> {
    let concat_args = build_fast_concat_args(list_file, concat_mp4);
    run_ffmpeg(ffmpeg_bin, &concat_args).map_err(|err| {
        format!(
            "fast concat phase failed (list_file={}): {err}",
            path_to_string(list_file)
        )
    })?;

    finalize_concat_to_output(concat_mp4, output_path)
        .map_err(|err| format!("fast finalize phase failed: {err}"))
}

pub(super) fn run_smooth_save_pipeline(
    ffmpeg_bin: &str,
    settings: &SettingsDto,
    list_file: &Path,
    smooth_mp4: &Path,
    output_path: &Path,
    target_trim_secs: f32,
) -> Result<(), String> {
    let smooth_args = build_smooth_concat_args(list_file, smooth_mp4, settings);
    run_ffmpeg(ffmpeg_bin, &smooth_args).map_err(|err| {
        format!(
            "smooth concat phase failed (list_file={}): {err}",
            path_to_string(list_file)
        )
    })?;

    finalize_from_concat(
        ffmpeg_bin,
        settings,
        smooth_mp4,
        output_path,
        target_trim_secs,
        "smooth",
    )
}

pub(super) fn finalize_from_concat(
    ffmpeg_bin: &str,
    settings: &SettingsDto,
    concat_mp4: &Path,
    output_path: &Path,
    target_trim_secs: f32,
    stage: &str,
) -> Result<(), String> {
    let trim_copy_args = build_trim_copy_args(concat_mp4, output_path, target_trim_secs);
    let trim_reencode_args =
        build_trim_reencode_args(settings, concat_mp4, output_path, target_trim_secs);

    if let Err(copy_err) = run_ffmpeg(ffmpeg_bin, &trim_copy_args) {
        run_ffmpeg(ffmpeg_bin, &trim_reencode_args).map_err(|reencode_err| {
            format!(
                "{stage} trim phase failed: stream-copy failed: {copy_err}; reencode fallback failed: {reencode_err}"
            )
        })?;
    }
    Ok(())
}

pub(super) fn build_fast_concat_args(list_file: &Path, concat_mp4: &Path) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-fflags".to_string(),
        "+genpts".to_string(),
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        path_to_string(list_file),
        "-c".to_string(),
        "copy".to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(concat_mp4),
    ]
}

pub(super) fn build_smooth_concat_args(
    list_file: &Path,
    smooth_mp4: &Path,
    settings: &SettingsDto,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-fflags".to_string(),
        "+genpts".to_string(),
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        path_to_string(list_file),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a:0?".to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        format!("{}k", settings.audio_bitrate_kbps),
        "-af".to_string(),
        "aresample=async=1:min_hard_comp=0.100:first_pts=0".to_string(),
        "-ar".to_string(),
        settings.audio_sample_rate_hz.to_string(),
        "-ac".to_string(),
        settings.audio_channels.to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(smooth_mp4),
    ]
}

pub(super) fn build_trim_copy_args(
    concat_mp4: &Path,
    output_path: &Path,
    target_trim_secs: f32,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-fflags".to_string(),
        "+genpts".to_string(),
        "-sseof".to_string(),
        format!("-{target_trim_secs:.3}"),
        "-i".to_string(),
        path_to_string(concat_mp4),
        "-t".to_string(),
        format!("{target_trim_secs:.3}"),
        "-c".to_string(),
        "copy".to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(output_path),
    ]
}

pub(super) fn build_trim_reencode_args(
    settings: &SettingsDto,
    concat_mp4: &Path,
    output_path: &Path,
    target_trim_secs: f32,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-sseof".to_string(),
        format!("-{target_trim_secs:.3}"),
        "-i".to_string(),
        path_to_string(concat_mp4),
        "-t".to_string(),
        format!("{target_trim_secs:.3}"),
        "-c:v".to_string(),
        "h264_videotoolbox".to_string(),
        "-b:v".to_string(),
        format!("{}k", settings.video_bitrate_kbps),
        "-color_range".to_string(),
        "tv".to_string(),
        "-colorspace".to_string(),
        "bt709".to_string(),
        "-color_primaries".to_string(),
        "bt709".to_string(),
        "-color_trc".to_string(),
        "iec61966-2-1".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        format!("{}k", settings.audio_bitrate_kbps),
        "-af".to_string(),
        "aresample=async=1:min_hard_comp=0.100:first_pts=0".to_string(),
        "-ar".to_string(),
        settings.audio_sample_rate_hz.to_string(),
        "-ac".to_string(),
        settings.audio_channels.to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(output_path),
    ]
}

pub(super) fn build_strict_cap_reencode_args(
    settings: &SettingsDto,
    input_path: &Path,
    output_path: &Path,
    target_trim_secs: f32,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        path_to_string(input_path),
        "-t".to_string(),
        format!("{target_trim_secs:.3}"),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a:0?".to_string(),
        "-c:v".to_string(),
        "h264_videotoolbox".to_string(),
        "-b:v".to_string(),
        format!("{}k", settings.video_bitrate_kbps),
        "-color_range".to_string(),
        "tv".to_string(),
        "-colorspace".to_string(),
        "bt709".to_string(),
        "-color_primaries".to_string(),
        "bt709".to_string(),
        "-color_trc".to_string(),
        "iec61966-2-1".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        format!("{}k", settings.audio_bitrate_kbps),
        "-af".to_string(),
        "aresample=async=1:min_hard_comp=0.100:first_pts=0".to_string(),
        "-ar".to_string(),
        settings.audio_sample_rate_hz.to_string(),
        "-ac".to_string(),
        settings.audio_channels.to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(output_path),
    ]
}

pub(super) fn build_playback_timing_correction_args(
    settings: &SettingsDto,
    input_path: &Path,
    output_path: &Path,
    target_trim_secs: f32,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        path_to_string(input_path),
        "-t".to_string(),
        format!("{target_trim_secs:.3}"),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a:0?".to_string(),
        "-vf".to_string(),
        "setpts=PTS-STARTPTS".to_string(),
        "-af".to_string(),
        "aresample=async=1:first_pts=0".to_string(),
        "-r".to_string(),
        settings.fps.max(1).to_string(),
        "-fps_mode".to_string(),
        "cfr".to_string(),
        "-c:v".to_string(),
        "h264_videotoolbox".to_string(),
        "-b:v".to_string(),
        format!("{}k", settings.video_bitrate_kbps),
        "-color_range".to_string(),
        "tv".to_string(),
        "-colorspace".to_string(),
        "bt709".to_string(),
        "-color_primaries".to_string(),
        "bt709".to_string(),
        "-color_trc".to_string(),
        "iec61966-2-1".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        format!("{}k", settings.audio_bitrate_kbps),
        "-ar".to_string(),
        settings.audio_sample_rate_hz.to_string(),
        "-ac".to_string(),
        settings.audio_channels.to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(output_path),
    ]
}

pub(super) fn build_smooth_postprocess_args(
    settings: &SettingsDto,
    input_path: &Path,
    output_path: &Path,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        path_to_string(input_path),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a:0?".to_string(),
        "-vf".to_string(),
        "setpts=PTS-STARTPTS".to_string(),
        "-af".to_string(),
        "aresample=async=1:first_pts=0".to_string(),
        "-r".to_string(),
        settings.fps.max(1).to_string(),
        "-fps_mode".to_string(),
        "cfr".to_string(),
        "-c:v".to_string(),
        "h264_videotoolbox".to_string(),
        "-b:v".to_string(),
        format!("{}k", settings.video_bitrate_kbps),
        "-color_range".to_string(),
        "tv".to_string(),
        "-colorspace".to_string(),
        "bt709".to_string(),
        "-color_primaries".to_string(),
        "bt709".to_string(),
        "-color_trc".to_string(),
        "iec61966-2-1".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        format!("{}k", settings.audio_bitrate_kbps),
        "-ar".to_string(),
        settings.audio_sample_rate_hz.to_string(),
        "-ac".to_string(),
        settings.audio_channels.to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        path_to_string(output_path),
    ]
}

pub(super) fn prepare_segments_for_concat(
    segments: &[PathBuf],
    list_file: &Path,
) -> Result<Vec<PathBuf>, String> {
    let first_missing = segments.iter().find(|segment| !segment.exists());
    if let Some(missing) = first_missing {
        return Err(format!(
            "segment does not exist (list_file={}, missing={})",
            path_to_string(list_file),
            path_to_string(missing)
        ));
    }

    let mut prepared = Vec::with_capacity(segments.len());
    for segment in segments {
        let canonical = segment.canonicalize().map_err(|err| {
            format!(
                "failed to canonicalize segment (list_file={}, segment={}): {err}",
                path_to_string(list_file),
                path_to_string(segment)
            )
        })?;
        prepared.push(canonical);
    }

    Ok(prepared)
}

pub(super) fn write_concat_list(path: &Path, segments: &[PathBuf]) -> Result<(), String> {
    let mut content = String::new();
    for segment in segments {
        content.push_str(&concat_line_for_path(segment));
    }

    fs::write(path, content).map_err(|err| format!("failed to write concat list: {err}"))
}

pub(super) fn concat_line_for_path(path: &Path) -> String {
    let escaped = path_to_string(path).replace('\'', "'\\''");
    format!("file '{escaped}'\n")
}

pub(super) fn run_ffmpeg(ffmpeg_bin: &str, args: &[String]) -> Result<(), String> {
    let output = Command::new(ffmpeg_bin)
        .args(args)
        .output()
        .map_err(|err| format!("failed to launch ffmpeg: {err}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "ffmpeg failed (status {}): {}",
        output.status,
        stderr_tail(&stderr, 14)
    ))
}

pub(super) fn stderr_tail(stderr: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = stderr.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join(" | ")
}

pub(super) fn cleanup_temp_paths(list_file: &Path, temp_outputs: &[PathBuf], is_failure: bool) {
    let keep_list = is_failure && keep_concat_list_on_failure();

    if !keep_list {
        let _ = fs::remove_file(list_file);
    }
    for path in temp_outputs {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn keep_concat_list_on_failure() -> bool {
    let Ok(value) = env::var("REWINDER_KEEP_CONCAT_LIST") else {
        return false;
    };

    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub(super) fn finalize_concat_to_output(
    concat_mp4: &Path,
    output_path: &Path,
) -> Result<(), String> {
    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|err| format!("failed to remove previous output file: {err}"))?;
    }

    match fs::rename(concat_mp4, output_path) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            fs::copy(concat_mp4, output_path).map_err(|copy_err| {
                format!("rename failed: {rename_err}; copy fallback failed: {copy_err}")
            })?;
            fs::remove_file(concat_mp4)
                .map_err(|err| format!("failed to remove temporary concat file: {err}"))?;
            Ok(())
        }
    }
}

pub(super) fn working_temp_path(output_path: &Path, suffix: &str) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("rewinder");
    let dir = output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".rewinder-tmp");
    let _ = fs::create_dir_all(&dir);
    dir.join(format!("{stem}.{suffix}"))
}

pub(super) fn replace_output_preserving_original(
    new_output: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let backup_path = working_temp_path(output_path, "fast.backup.mp4");

    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .map_err(|err| format!("failed to remove previous smoothing backup: {err}"))?;
    }

    fs::rename(output_path, &backup_path)
        .map_err(|err| format!("failed to move original clip to backup before smoothing: {err}"))?;

    let replace_result = match fs::rename(new_output, output_path) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            fs::copy(new_output, output_path).map_err(|copy_err| {
                format!("rename failed: {rename_err}; copy fallback failed: {copy_err}")
            })?;
            fs::remove_file(new_output)
                .map_err(|err| format!("failed to remove temporary smoothed clip: {err}"))?;
            Ok(())
        }
    };

    match replace_result {
        Ok(()) => {
            let _ = fs::remove_file(&backup_path);
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(output_path);
            let _ = fs::rename(&backup_path, output_path);
            Err(err)
        }
    }
}

pub(super) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub(super) fn duration_corrected_warning(
    existing: Option<ReplayWriteWarning>,
    target_trim_secs: f32,
    measured_duration_secs: f32,
) -> ReplayWriteWarning {
    let correction = format!(
        "Replay duration exceeded target ({target_trim_secs:.1}s) and was corrected to {measured_duration_secs:.1}s."
    );
    let message = match existing {
        Some(previous) => format!("{} {}", previous.message, correction),
        None => correction,
    };

    ReplayWriteWarning {
        code: "duration_corrected".to_string(),
        message,
        action: Some("Duration was capped to requested replay length.".to_string()),
    }
}

pub(super) fn playback_timing_corrected_warning(
    existing: Option<ReplayWriteWarning>,
    video_duration_secs: f32,
    audio_duration_secs: f32,
) -> ReplayWriteWarning {
    let correction = format!(
        "Playback timing drift was corrected (video {video_duration_secs:.2}s vs audio {audio_duration_secs:.2}s)."
    );
    let message = match existing {
        Some(previous) => format!("{} {}", previous.message, correction),
        None => correction,
    };

    ReplayWriteWarning {
        code: "playback_timing_corrected".to_string(),
        message,
        action: Some("Clip was re-timed for 1x playback consistency.".to_string()),
    }
}

pub(super) fn generate_random_clip_id(output_dir: &Path) -> String {
    for _ in 0..8 {
        let Some(random) = random_hex(8) else {
            break;
        };
        let id = format!("rewinder-{random}");
        if !output_dir.join(format!("{id}.mp4")).exists() {
            return id;
        }
    }

    format!("rewinder-{}", fallback_random_suffix())
}

pub(super) fn random_hex(bytes_len: usize) -> Option<String> {
    let mut bytes = vec![0_u8; bytes_len];
    let mut file = File::open("/dev/urandom").ok()?;
    file.read_exact(&mut bytes).ok()?;

    let mut out = String::with_capacity(bytes_len * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    Some(out)
}

pub(super) fn fallback_random_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let counter = CLIP_ID_FALLBACK_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:032x}{counter:016x}")
}
