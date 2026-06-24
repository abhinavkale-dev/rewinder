use super::*;
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
