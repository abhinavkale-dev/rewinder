use super::*;

pub(super) fn run_fast_save_pipeline(
    ffmpeg_bin: &str,
    _settings: &SettingsDto,
    list_file: &Path,
    concat_mp4: &Path,
    output_path: &Path,
    _target_trim_secs: f32,
) -> Result<(), String> {
    // OSS-REF(replay_buffer_whole_window): keep instant save as keyframe-aligned
    // whole-window remux to avoid trim-edge timestamp instability.
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

    // OSS-REF(stream_copy_first): only escalate to re-encode when copy trim fails.
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
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        path_to_string(list_file),
        "-c".to_string(),
        "copy".to_string(),
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

pub(super) fn probe_media_duration_secs(
    ffprobe_bin: &str,
    media_path: &Path,
) -> Result<f32, String> {
    let output = Command::new(ffprobe_bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
            &path_to_string(media_path),
        ])
        .output()
        .map_err(|err| format!("failed to launch ffprobe: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffprobe failed (status {}): {}",
            output.status,
            stderr_tail(&stderr, 10)
        ));
    }

    parse_ffprobe_duration_secs(&output.stdout)
}

pub(super) fn probe_av_stream_durations_secs(
    ffprobe_bin: &str,
    media_path: &Path,
) -> Result<Option<(f32, f32)>, String> {
    let output = Command::new(ffprobe_bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,duration",
            "-of",
            "json",
            &path_to_string(media_path),
        ])
        .output()
        .map_err(|err| format!("failed to launch ffprobe: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffprobe failed (status {}): {}",
            output.status,
            stderr_tail(&stderr, 10)
        ));
    }

    parse_ffprobe_av_stream_durations_secs(&output.stdout)
}

#[derive(Debug, Clone)]
pub(super) struct FastReplayIntegritySnapshot {
    pub format_duration_secs: Option<f32>,
    pub video_duration_secs: Option<f32>,
    pub audio_duration_secs: Option<f32>,
    pub video_avg_frame_rate: Option<f64>,
    pub video_packet_pts_secs: Vec<f64>,
    pub audio_packet_pts_secs: Vec<f64>,
}

#[derive(Debug, Clone)]
pub(super) struct FastReplayIntegrityIssue {
    pub detected_code: &'static str,
    pub corrected_code: &'static str,
    pub message: String,
    pub action: Option<String>,
}

impl FastReplayIntegritySnapshot {
    pub fn repair_target_secs(&self) -> f32 {
        self.format_duration_secs
            .into_iter()
            .chain(self.video_duration_secs)
            .chain(self.audio_duration_secs)
            .fold(0.0_f32, f32::max)
            .max(0.1)
    }
}

pub(super) fn probe_fast_integrity_snapshot(
    ffprobe_bin: &str,
    media_path: &Path,
) -> Result<FastReplayIntegritySnapshot, String> {
    let output = Command::new(ffprobe_bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=index,codec_type,duration,avg_frame_rate:packet=stream_index,pts_time",
            "-show_packets",
            "-of",
            "json",
            &path_to_string(media_path),
        ])
        .output()
        .map_err(|err| format!("failed to launch ffprobe: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffprobe failed (status {}): {}",
            output.status,
            stderr_tail(&stderr, 10)
        ));
    }

    parse_ffprobe_fast_integrity_snapshot(&output.stdout)
}

pub(super) fn parse_ffprobe_fast_integrity_snapshot(
    stdout: &[u8],
) -> Result<FastReplayIntegritySnapshot, String> {
    let value: Value = serde_json::from_slice(stdout)
        .map_err(|err| format!("failed to parse ffprobe json: {err}"))?;
    let format_duration_secs = value
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(Value::as_str)
        .and_then(|raw| raw.parse::<f32>().ok())
        .filter(|duration| duration.is_finite() && *duration >= 0.0);

    let mut video_stream_index = None;
    let mut audio_stream_index = None;
    let mut video_duration_secs = None;
    let mut audio_duration_secs = None;
    let mut video_avg_frame_rate = None;

    if let Some(streams) = value.get("streams").and_then(Value::as_array) {
        for stream in streams {
            let codec_type = stream
                .get("codec_type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let stream_index = stream
                .get("index")
                .and_then(Value::as_i64)
                .and_then(|index| u32::try_from(index).ok());
            let duration = stream
                .get("duration")
                .and_then(Value::as_str)
                .and_then(|raw| raw.parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0);
            match codec_type {
                "video" if video_stream_index.is_none() => {
                    video_stream_index = stream_index;
                    video_duration_secs = duration;
                    video_avg_frame_rate = stream
                        .get("avg_frame_rate")
                        .and_then(Value::as_str)
                        .and_then(parse_ffprobe_frame_rate);
                }
                "audio" if audio_stream_index.is_none() => {
                    audio_stream_index = stream_index;
                    audio_duration_secs = duration;
                }
                _ => {}
            }
        }
    }

    let mut video_packet_pts_secs = Vec::new();
    let mut audio_packet_pts_secs = Vec::new();
    if let Some(packets) = value.get("packets").and_then(Value::as_array) {
        for packet in packets {
            let Some(stream_index) = packet
                .get("stream_index")
                .and_then(Value::as_i64)
                .and_then(|index| u32::try_from(index).ok())
            else {
                continue;
            };
            let Some(pts_time) = packet
                .get("pts_time")
                .and_then(Value::as_str)
                .and_then(|raw| raw.parse::<f64>().ok())
                .filter(|value| value.is_finite())
            else {
                continue;
            };
            if Some(stream_index) == video_stream_index {
                video_packet_pts_secs.push(pts_time);
            } else if Some(stream_index) == audio_stream_index {
                audio_packet_pts_secs.push(pts_time);
            }
        }
    }

    Ok(FastReplayIntegritySnapshot {
        format_duration_secs,
        video_duration_secs,
        audio_duration_secs,
        video_avg_frame_rate,
        video_packet_pts_secs,
        audio_packet_pts_secs,
    })
}

pub(super) fn detect_fast_integrity_issue(
    snapshot: &FastReplayIntegritySnapshot,
    expected_fps: u16,
) -> Option<FastReplayIntegrityIssue> {
    if has_timestamp_regression(&snapshot.video_packet_pts_secs) {
        return Some(FastReplayIntegrityIssue {
            detected_code: "video_timestamp_warning",
            corrected_code: "video_timestamp_corrected",
            message: "Non-monotonic video timestamps were detected in the fast replay output."
                .to_string(),
            action: Some(
                "Clip was saved immediately; timing repair can restore playback consistency."
                    .to_string(),
            ),
        });
    }
    if has_timestamp_regression(&snapshot.audio_packet_pts_secs) {
        return Some(FastReplayIntegrityIssue {
            detected_code: "audio_timestamp_warning",
            corrected_code: "audio_timestamp_corrected",
            message: "Non-monotonic audio timestamps were detected in the fast replay output."
                .to_string(),
            action: Some(
                "Clip was saved immediately; timing repair can restore playback consistency."
                    .to_string(),
            ),
        });
    }

    if let (Some(video_duration_secs), Some(audio_duration_secs)) =
        (snapshot.video_duration_secs, snapshot.audio_duration_secs)
    {
        let max_duration = video_duration_secs.max(audio_duration_secs).max(0.001);
        let drift_ratio = (video_duration_secs - audio_duration_secs).abs() / max_duration;
        if drift_ratio > 0.03 {
            return Some(FastReplayIntegrityIssue {
                detected_code: "av_drift_warning",
                corrected_code: "av_drift_corrected",
                message: format!(
                    "Fast replay output drifted between video ({video_duration_secs:.2}s) and audio ({audio_duration_secs:.2}s)."
                ),
                action: Some(
                    "Clip was saved immediately; timing repair can restore playback consistency."
                        .to_string(),
                ),
            });
        }
    }

    let measured_fps = measured_packet_fps(&snapshot.video_packet_pts_secs)
        .or(snapshot.video_avg_frame_rate)
        .filter(|fps| fps.is_finite() && *fps > 0.0)?;
    let expected_fps = f64::from(expected_fps.max(1));
    let relative_error = ((measured_fps - expected_fps) / expected_fps).abs();
    if relative_error > 0.12 {
        return Some(FastReplayIntegrityIssue {
            detected_code: "fps_drift_warning",
            corrected_code: "fps_drift_corrected",
            message: format!(
                "Fast replay cadence drifted to {measured_fps:.2}fps (target {expected_fps:.2}fps)."
            ),
            action: Some(
                "Clip was saved immediately; timing repair can restore playback consistency."
                    .to_string(),
            ),
        });
    }

    None
}

fn parse_ffprobe_frame_rate(raw: &str) -> Option<f64> {
    let (numerator, denominator) = raw.split_once('/')?;
    let numerator = numerator.trim().parse::<f64>().ok()?;
    let denominator = denominator.trim().parse::<f64>().ok()?;
    if !numerator.is_finite() || !denominator.is_finite() || numerator <= 0.0 || denominator <= 0.0
    {
        return None;
    }
    Some(numerator / denominator)
}

fn has_timestamp_regression(values: &[f64]) -> bool {
    const REGRESSION_EPSILON_SECS: f64 = 0.0005;

    values
        .windows(2)
        .any(|pair| pair[1] + REGRESSION_EPSILON_SECS < pair[0])
}

fn measured_packet_fps(values: &[f64]) -> Option<f64> {
    let mut deltas: Vec<f64> = values
        .windows(2)
        .filter_map(|pair| {
            let delta = pair[1] - pair[0];
            (delta > 0.0 && delta.is_finite()).then_some(delta)
        })
        .collect();
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_by(|left, right| left.total_cmp(right));
    let median = deltas[deltas.len() / 2];
    if median <= 0.0 {
        return None;
    }
    Some(1.0 / median)
}

pub(super) fn parse_ffprobe_duration_secs(stdout: &[u8]) -> Result<f32, String> {
    let value: Value = serde_json::from_slice(stdout)
        .map_err(|err| format!("failed to parse ffprobe json: {err}"))?;
    let Some(duration_str) = value
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(Value::as_str)
    else {
        return Err("ffprobe json missing format.duration".to_string());
    };

    let duration = duration_str
        .parse::<f32>()
        .map_err(|err| format!("invalid ffprobe duration '{duration_str}': {err}"))?;
    if !duration.is_finite() || duration < 0.0 {
        return Err(format!("invalid ffprobe duration value: {duration}"));
    }

    Ok(duration)
}

pub(super) fn parse_ffprobe_av_stream_durations_secs(
    stdout: &[u8],
) -> Result<Option<(f32, f32)>, String> {
    let value: Value = serde_json::from_slice(stdout)
        .map_err(|err| format!("failed to parse ffprobe json: {err}"))?;
    let Some(streams) = value.get("streams").and_then(Value::as_array) else {
        return Ok(None);
    };

    let mut video_duration = None;
    let mut audio_duration = None;
    for stream in streams {
        let codec_type = stream
            .get("codec_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let duration = stream
            .get("duration")
            .and_then(Value::as_str)
            .and_then(|raw| raw.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0);
        match codec_type {
            "video" => {
                if video_duration.is_none() {
                    video_duration = duration;
                }
            }
            "audio" => {
                if audio_duration.is_none() {
                    audio_duration = duration;
                }
            }
            _ => {}
        }
    }

    Ok(match (video_duration, audio_duration) {
        (Some(video), Some(audio)) => Some((video, audio)),
        _ => None,
    })
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

pub(super) fn replace_output_preserving_original(
    new_output: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let backup_path = output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "{}.fast.backup.mp4",
            output_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("rewinder")
        ));

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

pub(super) fn resolve_ffmpeg_binary() -> String {
    if let Ok(bin) = env::var("REWINDER_FFMPEG_BIN") {
        if !bin.trim().is_empty() {
            return bin;
        }
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(contents_dir) = exe.parent().and_then(|p| p.parent()) {
            let bundled = contents_dir.join("Resources").join("bin").join("ffmpeg");
            if bundled.exists() {
                return bundled.to_string_lossy().to_string();
            }
        }
    }

    if let Ok(cwd) = env::current_dir() {
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

pub(super) fn resolve_ffprobe_binary(ffmpeg_bin: &str) -> String {
    if let Ok(bin) = env::var("REWINDER_FFPROBE_BIN") {
        if !bin.trim().is_empty() {
            return bin;
        }
    }

    if let Some(sibling) = sibling_binary(ffmpeg_bin, "ffprobe") {
        return sibling;
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(contents_dir) = exe.parent().and_then(|p| p.parent()) {
            let bundled = contents_dir.join("Resources").join("bin").join("ffprobe");
            if bundled.exists() {
                return bundled.to_string_lossy().to_string();
            }
        }
    }

    if let Ok(cwd) = env::current_dir() {
        let dev_bundled = cwd.join("src-tauri").join("bin").join("ffprobe");
        if dev_bundled.exists() {
            return dev_bundled.to_string_lossy().to_string();
        }
    }

    if Path::new("/opt/homebrew/bin/ffprobe").exists() {
        return "/opt/homebrew/bin/ffprobe".to_string();
    }

    "ffprobe".to_string()
}

pub(super) fn sibling_binary(bin_path: &str, sibling_name: &str) -> Option<String> {
    let input = Path::new(bin_path);
    let parent = input.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    let sibling = parent.join(sibling_name);
    if sibling.exists() {
        Some(path_to_string(&sibling))
    } else {
        None
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
