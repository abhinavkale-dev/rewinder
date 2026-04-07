use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::core::state::ClipMetadataDto;
use crate::settings::SettingsDto;

static CLIP_ID_FALLBACK_COUNTER: AtomicU64 = AtomicU64::new(0);
const STRICT_DURATION_TOLERANCE_SECS: f32 = 0.5;
const PLAYBACK_TIMING_DRIFT_THRESHOLD_RATIO: f32 = 0.03;

#[derive(Debug, Clone)]
pub struct ReplayWriteWarning {
    pub code: String,
    pub message: String,
    pub action: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReplayWriteOutcome {
    pub clip: ClipMetadataDto,
    pub audio_repaired: bool,
    pub save_audio_strategy: String,
    pub warning: Option<ReplayWriteWarning>,
}

#[derive(Debug, Clone)]
pub enum FastReplayVerificationOutcome {
    Verified,
    Corrected {
        warning: ReplayWriteWarning,
        duration_secs: f32,
    },
    RepairFailed {
        issue: ReplayWriteWarning,
        error: String,
    },
}

pub fn write_replay_from_segments(
    segments: &[PathBuf],
    settings: &SettingsDto,
    target_trim_secs: f32,
    _available_secs: f32,
) -> Result<ReplayWriteOutcome, String> {
    if segments.is_empty() {
        return Err("No segments available to write replay".to_string());
    }
    if target_trim_secs <= 0.0 {
        return Err("target trim duration must be > 0".to_string());
    }

    let output_dir = settings.output_dir_path();
    fs::create_dir_all(&output_dir).map_err(|err| format!("failed to create output dir: {err}"))?;

    let created_at_epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock error: {err}"))?
        .as_millis() as i64;

    let id = generate_random_clip_id(&output_dir);
    let temp_dir = output_dir.join(".rewinder-tmp");
    fs::create_dir_all(&temp_dir).map_err(|err| format!("failed to create temp dir: {err}"))?;

    let list_file = temp_dir.join(format!("{id}.txt"));
    let output_path = output_dir.join(format!("{id}.mp4"));
    let fast_concat_mp4 = temp_dir.join(format!("{id}-concat.mp4"));
    let smooth_concat_mp4 = temp_dir.join(format!("{id}-smooth.mp4"));
    let strict_corrected_mp4 = temp_dir.join(format!("{id}-strict.mp4"));
    let timing_corrected_mp4 = temp_dir.join(format!("{id}-timing.mp4"));
    let temp_outputs = vec![
        fast_concat_mp4.clone(),
        smooth_concat_mp4.clone(),
        strict_corrected_mp4.clone(),
        timing_corrected_mp4.clone(),
    ];

    let prepared_segments = prepare_segments_for_concat(segments, &list_file)?;
    write_concat_list(&list_file, &prepared_segments)?;

    let ffmpeg_bin = resolve_ffmpeg_binary();
    let ffprobe_bin = resolve_ffprobe_binary(&ffmpeg_bin);
    let save_mode = SavePathMode::from_settings(
        settings.save_path_mode.as_str(),
        settings.audio_save_mode.as_str(),
    );

    let mut warning = None;
    let (audio_repaired, save_audio_strategy, used_fast_path) = match save_mode {
        SavePathMode::InstantMp4 => {
            run_fast_save_pipeline(
                &ffmpeg_bin,
                settings,
                &list_file,
                &fast_concat_mp4,
                &output_path,
                target_trim_secs,
            )
            .map_err(|err| {
                cleanup_temp_paths(&list_file, &temp_outputs, true);
                err
            })?;
            (false, "instant_mp4".to_string(), true)
        }
        SavePathMode::Fast => {
            run_fast_save_pipeline(
                &ffmpeg_bin,
                settings,
                &list_file,
                &fast_concat_mp4,
                &output_path,
                target_trim_secs,
            )
            .map_err(|err| {
                cleanup_temp_paths(&list_file, &temp_outputs, true);
                err
            })?;
            (false, "fast".to_string(), true)
        }
        SavePathMode::Smooth | SavePathMode::Adaptive => {
            let smooth_result = run_smooth_save_pipeline(
                &ffmpeg_bin,
                settings,
                &list_file,
                &smooth_concat_mp4,
                &output_path,
                target_trim_secs,
            );
            match smooth_result {
                Ok(()) => (true, "smooth".to_string(), false),
                Err(smooth_err) => {
                    run_fast_save_pipeline(
                        &ffmpeg_bin,
                        settings,
                        &list_file,
                        &fast_concat_mp4,
                        &output_path,
                        target_trim_secs,
                    )
                    .map_err(|fast_err| {
                        cleanup_temp_paths(&list_file, &temp_outputs, true);
                        format!(
                            "smooth save failed and fast fallback failed (list_file={}): smooth={smooth_err}; fast={fast_err}",
                            path_to_string(&list_file)
                        )
                    })?;
                    warning = Some(ReplayWriteWarning {
                        code: "audio_smooth_fallback".to_string(),
                        message: "Smooth audio save failed; saved clip using fast fallback.".to_string(),
                        action: Some(
                            "Capture continued and clip was saved. If jitter persists, lower FPS or resolution."
                                .to_string(),
                        ),
                    });
                    (false, "fallback_fast".to_string(), true)
                }
            }
        }
    };
    // --- Post-concat processing: split by save mode ---
    //
    // instant_mp4 / fast: whole-window concat+remux is the final output.
    //   Only probe duration for metadata. No re-encode, no deep integrity probes.
    //   This is the gamer-optimised path — fastest possible save.
    //
    // smooth / adaptive: correction-heavy path with timing normalisation,
    //   A/V drift repair, and strict duration capping. Re-encode only when needed.

    let measured_duration_secs;

    if used_fast_path {
        // Fast path: single ffprobe for duration metadata, nothing else.
        measured_duration_secs =
            probe_media_duration_secs(&ffprobe_bin, &output_path).map_err(|err| {
                cleanup_temp_paths(&list_file, &temp_outputs, true);
                err
            })?;
    } else {
        // Smooth/adaptive path: full correction pipeline.
        measured_duration_secs = run_correction_pipeline(
            &ffmpeg_bin,
            &ffprobe_bin,
            settings,
            &output_path,
            &timing_corrected_mp4,
            &strict_corrected_mp4,
            &list_file,
            &temp_outputs,
            target_trim_secs,
            &mut warning,
        )?;
    }

    cleanup_temp_paths(&list_file, &temp_outputs, false);

    let size_bytes = fs::metadata(&output_path)
        .map_err(|err| format!("failed to read replay metadata: {err}"))?
        .len();

    Ok(ReplayWriteOutcome {
        clip: ClipMetadataDto {
            id,
            path: path_to_string(&output_path),
            created_at_epoch_ms,
            duration_secs: measured_duration_secs,
            size_bytes,
        },
        audio_repaired,
        save_audio_strategy,
        warning,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_correction_pipeline(
    ffmpeg_bin: &str,
    ffprobe_bin: &str,
    settings: &SettingsDto,
    output_path: &Path,
    timing_corrected_mp4: &Path,
    strict_corrected_mp4: &Path,
    list_file: &Path,
    temp_outputs: &[PathBuf],
    target_trim_secs: f32,
    warning: &mut Option<ReplayWriteWarning>,
) -> Result<f32, String> {
    let mut measured_duration_secs =
        probe_media_duration_secs(ffprobe_bin, output_path).map_err(|err| {
            cleanup_temp_paths(list_file, temp_outputs, true);
            err
        })?;
    let av_stream_durations =
        probe_av_stream_durations_secs(ffprobe_bin, output_path).map_err(|err| {
            cleanup_temp_paths(list_file, temp_outputs, true);
            err
        })?;

    // Timing correction for A/V drift (smooth/adaptive only).
    if let Some((video_duration_secs, audio_duration_secs)) = av_stream_durations {
        let max_duration = video_duration_secs.max(audio_duration_secs).max(0.001);
        let drift_ratio = (video_duration_secs - audio_duration_secs).abs() / max_duration;
        if drift_ratio > PLAYBACK_TIMING_DRIFT_THRESHOLD_RATIO {
            let timing_args = build_playback_timing_correction_args(
                settings,
                output_path,
                timing_corrected_mp4,
                target_trim_secs,
            );
            run_ffmpeg(ffmpeg_bin, &timing_args).map_err(|err| {
                cleanup_temp_paths(list_file, temp_outputs, true);
                format!("playback timing correction failed: {err}")
            })?;
            finalize_concat_to_output(timing_corrected_mp4, output_path).map_err(|err| {
                cleanup_temp_paths(list_file, temp_outputs, true);
                format!("playback timing correction finalize failed: {err}")
            })?;
            measured_duration_secs =
                probe_media_duration_secs(ffprobe_bin, output_path).map_err(|err| {
                    cleanup_temp_paths(list_file, temp_outputs, true);
                    err
                })?;
            *warning = Some(playback_timing_corrected_warning(
                warning.take(),
                video_duration_secs,
                audio_duration_secs,
            ));
        }
    }

    // Strict duration capping (smooth/adaptive only).
    let duration_cap = target_trim_secs + STRICT_DURATION_TOLERANCE_SECS;
    if measured_duration_secs > duration_cap {
        let strict_args = build_strict_cap_reencode_args(
            settings,
            output_path,
            strict_corrected_mp4,
            target_trim_secs,
        );
        run_ffmpeg(ffmpeg_bin, &strict_args).map_err(|err| {
            cleanup_temp_paths(list_file, temp_outputs, true);
            format!("strict duration correction failed: {err}")
        })?;
        finalize_concat_to_output(strict_corrected_mp4, output_path).map_err(|err| {
            cleanup_temp_paths(list_file, temp_outputs, true);
            format!("strict duration correction finalize failed: {err}")
        })?;
        measured_duration_secs =
            probe_media_duration_secs(ffprobe_bin, output_path).map_err(|err| {
                cleanup_temp_paths(list_file, temp_outputs, true);
                err
            })?;
        if measured_duration_secs > duration_cap {
            cleanup_temp_paths(list_file, temp_outputs, true);
            return Err(format!(
                "strict duration cap failed: measured {:.3}s exceeds cap {:.3}s",
                measured_duration_secs, duration_cap
            ));
        }
        *warning = Some(duration_corrected_warning(
            warning.take(),
            target_trim_secs,
            measured_duration_secs,
        ));
    }

    Ok(measured_duration_secs)
}

pub fn smooth_replay_in_place(output_path: &Path, settings: &SettingsDto) -> Result<(), String> {
    if !output_path.exists() {
        return Err(format!(
            "smooth postprocess input does not exist: {}",
            path_to_string(output_path)
        ));
    }

    let ffmpeg_bin = resolve_ffmpeg_binary();
    let temp_smoothed_path = output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "{}.smooth.tmp.mp4",
            output_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("rewinder")
        ));
    let smoothing_args = build_smooth_postprocess_args(settings, output_path, &temp_smoothed_path);

    run_ffmpeg(&ffmpeg_bin, &smoothing_args).map_err(|err| {
        let _ = fs::remove_file(&temp_smoothed_path);
        format!("smooth postprocess ffmpeg failed: {err}")
    })?;

    replace_output_preserving_original(&temp_smoothed_path, output_path).map_err(|err| {
        let _ = fs::remove_file(&temp_smoothed_path);
        format!("smooth postprocess replace failed: {err}")
    })?;

    Ok(())
}

pub fn verify_fast_replay_in_place(
    output_path: &Path,
    settings: &SettingsDto,
) -> Result<FastReplayVerificationOutcome, String> {
    if !output_path.exists() {
        return Err(format!(
            "fast verify input does not exist: {}",
            path_to_string(output_path)
        ));
    }

    let ffmpeg_bin = resolve_ffmpeg_binary();
    let ffprobe_bin = resolve_ffprobe_binary(&ffmpeg_bin);
    let snapshot = probe_fast_integrity_snapshot(&ffprobe_bin, output_path)?;
    let Some(issue) = detect_fast_integrity_issue(&snapshot, settings.fps) else {
        return Ok(FastReplayVerificationOutcome::Verified);
    };

    let temp_corrected_path = output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "{}.fastverify.tmp.mp4",
            output_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("rewinder")
        ));
    let timing_args = build_playback_timing_correction_args(
        settings,
        output_path,
        &temp_corrected_path,
        snapshot.repair_target_secs(),
    );

    if let Err(err) = run_ffmpeg(&ffmpeg_bin, &timing_args) {
        let _ = fs::remove_file(&temp_corrected_path);
        return Ok(FastReplayVerificationOutcome::RepairFailed {
            issue: ReplayWriteWarning {
                code: issue.detected_code.to_string(),
                message: issue.message,
                action: issue.action,
            },
            error: format!("playback timing correction failed: {err}"),
        });
    }

    if let Err(err) = replace_output_preserving_original(&temp_corrected_path, output_path) {
        let _ = fs::remove_file(&temp_corrected_path);
        return Ok(FastReplayVerificationOutcome::RepairFailed {
            issue: ReplayWriteWarning {
                code: issue.detected_code.to_string(),
                message: issue.message,
                action: issue.action,
            },
            error: format!("playback timing correction replace failed: {err}"),
        });
    }

    let duration_secs = probe_media_duration_secs(&ffprobe_bin, output_path)?;
    Ok(FastReplayVerificationOutcome::Corrected {
        warning: ReplayWriteWarning {
            code: issue.corrected_code.to_string(),
            message: format!(
                "{} Repaired in background for playback consistency.",
                issue.message
            ),
            action: Some("Repaired clip was written back in place.".to_string()),
        },
        duration_secs,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SavePathMode {
    InstantMp4,
    Smooth,
    Fast,
    Adaptive,
}

impl SavePathMode {
    fn from_settings(path_mode: &str, legacy_audio_mode: &str) -> Self {
        match path_mode {
            "instant_mp4" => Self::InstantMp4,
            "fast" => Self::Fast,
            "smooth" => Self::Smooth,
            "adaptive" => Self::Adaptive,
            _ => match legacy_audio_mode {
                "fast" => Self::InstantMp4,
                "adaptive" => Self::Adaptive,
                _ => Self::Smooth,
            },
        }
    }
}

mod helpers;
use helpers::*;

#[cfg(test)]
mod tests;
