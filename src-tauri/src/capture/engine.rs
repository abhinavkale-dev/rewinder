use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use fs2::FileExt;
use parking_lot::Mutex;
use serde::Serialize;

use crate::encoding::audio_encoder::AudioEncoder;
use crate::encoding::video_encoder::VideoEncoder;
use crate::settings::SettingsDto;
use crate::sys::binaries;

const SEGMENT_EXTENSION: &str = "mp4";
const SEGMENT_STABLE_GRACE_MS: u64 = 500;
const STARTUP_MIN_SEGMENT_BYTES: u64 = 32 * 1024;
const RETENTION_MARGIN_SEGMENTS: usize = 12;
const PROCESS_TERM_GRACE_MS: u64 = 1_200;
const PROCESS_SWEEP_TERM_GRACE_MS: u64 = 300;
const SEGMENT_DIR_PERMISSION_FAILURE_THRESHOLD: u8 = 3;
const VIDEO_INPUT_THREAD_QUEUE_SMALL: usize = 8;
const AUDIO_INPUT_THREAD_QUEUE_SMALL: usize = 64;
const VIDEO_INPUT_THREAD_QUEUE_ELEVATED: usize = 32;
const AUDIO_INPUT_THREAD_QUEUE_ELEVATED: usize = 256;
const FIRST_SYSTEM_AUDIO_MARKER: &str = "first system audio frame delivered";
const FIRST_MIC_AUDIO_MARKER: &str = "first microphone audio frame delivered";
const FIRST_VIDEO_MARKER: &str = "first video frame delivered";
const MIC_CAPTURE_SESSION_RUNNING_MARKER: &str = "phase: mic_capture_session_running";
const MIC_SILENCE_FILLER_ACTIVE_MARKER: &str = "phase: mic_silence_filler_active";
const MIC_LIVE_FRAMES_DETECTED_MARKER: &str = "phase: mic_live_frames_detected";
const MIC_LIVE_FRAMES_LOST_MARKER: &str = "phase: mic_live_frames_lost";
const MIC_SUSTAINED_SILENCE_MARKER: &str = "phase: mic_sustained_silence_detected";
const MIC_SAMPLES_PER_SEC_PREFIX: &str = "mic_samples_per_sec=";
const VIDEO_OUTPUT_FPS_PREFIX: &str = "video_output_fps=";
const VIDEO_FRAME_DROP_TOTAL_PREFIX: &str = "video_frame_drop_total=";
const VIDEO_QUEUE_OVERFLOW_COUNT_PREFIX: &str = "video_queue_overflow_count=";
const SYSTEM_MEMORY_PRESSURE_PREFIX: &str = "system_memory_pressure=";
const THERMAL_STATE_PREFIX: &str = "thermal_state=";
const FFMPEG_QUEUE_STARVATION_MARKER: &str = "Thread message queue blocking";
const STARTUP_SOFT_READY_EXTENSION_MS: u64 = 2_000;
const STARTUP_FIRST_ATTEMPT_EXTRA_TIMEOUT_MS: u64 = 2_000;
const STARTUP_MIC_MIX_EXTRA_TIMEOUT_MS: u64 = 6_000;
const STARTUP_SEGMENT_PROGRESS_MIN_BYTES: u64 = 1;
const HELPER_INTERRUPTED_EXIT_CODE: i32 = 73;
const CAPTURE_LOCK_FILENAME: &str = "capture.lock";
const CAPTURE_LOCK_SESSION_PENDING: &str = "pending";
const LOG_METRICS_REFRESH_INTERVAL_MS: u64 = 500;
const CAPTURE_LOG_ROTATE_BYTES: u64 = 8 * 1024 * 1024;
const CAPTURE_LOG_ROTATE_KEEP_BYTES: u64 = 1024 * 1024;

static CAPTURE_SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveQueueProfile {
    Small,
    Elevated,
}

impl LiveQueueProfile {
    fn video_thread_queue_size(self) -> usize {
        match self {
            Self::Small => VIDEO_INPUT_THREAD_QUEUE_SMALL,
            Self::Elevated => VIDEO_INPUT_THREAD_QUEUE_ELEVATED,
        }
    }

    fn audio_thread_queue_size(self) -> usize {
        match self {
            Self::Small => AUDIO_INPUT_THREAD_QUEUE_SMALL,
            Self::Elevated => AUDIO_INPUT_THREAD_QUEUE_ELEVATED,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Elevated => "elevated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioMode {
    VideoOnly,
    SystemOnly,
    SystemPlusMic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStartupStrategy {
    SystemFirst,
    PreferMic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicAttachRuntimeState {
    SilenceFiller,
    Live,
    Degraded,
}

impl AudioMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::VideoOnly => "video_only",
            Self::SystemOnly => "system_only",
            Self::SystemPlusMic => "system_plus_mic",
        }
    }

    fn has_audio(self) -> bool {
        !matches!(self, Self::VideoOnly)
    }

    fn has_mic(self) -> bool {
        matches!(self, Self::SystemPlusMic)
    }
}

struct CapturePipes {
    video_pipe: PathBuf,
    system_audio_pipe: Option<PathBuf>,
    mic_audio_pipe: Option<PathBuf>,
}

struct CaptureStartup {
    ffmpeg_child: Child,
    helper_child: Child,
    active_audio_mode: AudioMode,
    mic_backend_in_use: String,
    pipes: CapturePipes,
    session_id: String,
    attempt_log_offset: u64,
}

struct SegmentFile {
    path: PathBuf,
    modified: SystemTime,
    size_bytes: u64,
    session_id: Option<String>,
    segment_index: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct CaptureLockPayload {
    owner_pid: Option<u32>,
    started_epoch_ms: Option<i64>,
    session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplaySelection {
    pub segments: Vec<PathBuf>,
    pub available_secs: f32,
    pub contiguous_duration_secs: f32,
    pub target_trim_secs: f32,
    pub window_start: SystemTime,
    pub window_end: SystemTime,
    pub partial_history: bool,
    pub partial_reason: Option<String>,
    pub partial_reason_code: Option<String>,
    pub discontinuity_gap_ms: Option<u64>,
    pub session_boundary_count: u8,
}

pub struct CaptureEngine {
    segment_dir: PathBuf,
    capture_log_path: PathBuf,
    capture_lock_path: PathBuf,
    capture_lock_file: Option<File>,
    capture_owner_pid: u32,
    ffmpeg_pid_file: PathBuf,
    helper_pid_file: PathBuf,
    video_pipe_path: PathBuf,
    system_audio_pipe_path: Option<PathBuf>,
    mic_audio_pipe_path: Option<PathBuf>,
    buffer_duration_secs: u16,
    segment_duration_secs: f32,
    stop: Arc<AtomicBool>,
    ffmpeg_child: Arc<Mutex<Option<Child>>>,
    helper_child: Arc<Mutex<Option<Child>>>,
    worker: Option<JoinHandle<()>>,
    last_error: Arc<Mutex<Option<String>>>,
    startup_fallback_error: Option<String>,
    active_audio_mode: AudioMode,
    session_id: String,
    capture_log_offset: Arc<AtomicU64>,
    mic_backend_in_use: String,
    queue_profile: LiveQueueProfile,
    display_signature: u64,
    display_change_seen_at: Arc<Mutex<Option<Instant>>>,
    prune_frozen: Arc<AtomicBool>,
    log_metrics: Mutex<LogMetricsCache>,
}

impl CaptureEngine {
    pub fn start(
        settings: SettingsDto,
        queue_profile: LiveQueueProfile,
        startup_strategy: AudioStartupStrategy,
    ) -> Result<Self, String> {
        let ffmpeg_bin = binaries::resolve_ffmpeg_binary();
        let helper_bin = binaries::resolve_sck_helper_binary(
            "ScreenCaptureKit helper binary was not built. Rebuild the app and retry.",
        )?;

        let segment_dir = settings.output_dir_path().join(".rewinder-live");
        fs::create_dir_all(&segment_dir)
            .map_err(|err| format!("failed to create live segment directory: {err}"))?;
        let capture_lock_path = segment_dir.join(CAPTURE_LOCK_FILENAME);
        let owner_pid = std::process::id();
        let capture_log_path = segment_dir.join("ffmpeg-capture.log");
        let _ = fs::remove_file(&capture_log_path);
        append_capture_log_line(
            &capture_log_path,
            "=== backend: ScreenCaptureKit bridge ===",
        );
        let mut startup_sweep_exclusions = HashSet::new();
        startup_sweep_exclusions.insert(owner_pid);
        let _ = sweep_orphan_capture_processes(
            &capture_log_path,
            "startup_preflight",
            &startup_sweep_exclusions,
        );
        let (mut capture_lock_file, stale_lock_owner) =
            acquire_capture_lock(&capture_lock_path, owner_pid, CAPTURE_LOCK_SESSION_PENDING)?;

        let ffmpeg_pid_file = segment_dir.join("ffmpeg-capture.pid");
        let helper_pid_file = segment_dir.join("sck-capture.pid");
        terminate_stale_capture_process(&ffmpeg_pid_file);
        terminate_stale_capture_process(&helper_pid_file);

        if let Some(stale_owner) = stale_lock_owner {
            append_capture_log_line(
                &capture_log_path,
                &format!(
                    "phase: stale_capture_lock_reclaimed owner_pid={} session_id={}",
                    stale_owner
                        .owner_pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    stale_owner
                        .session_id
                        .as_deref()
                        .unwrap_or("unknown_session"),
                ),
            );
        }

        let segment_duration_secs = segment_duration_secs(settings.segment_time_ms);
        append_capture_log_line(
            &capture_log_path,
            &format!(
                "phase: audio_startup_strategy={}",
                match startup_strategy {
                    AudioStartupStrategy::SystemFirst => "system_first",
                    AudioStartupStrategy::PreferMic => "prefer_mic",
                }
            ),
        );

        let output_echo_prone = probe_output_echo_prone(&helper_bin, &capture_log_path);
        let attempts = requested_audio_attempts(&settings, startup_strategy, output_echo_prone);

        let mut startup: Option<CaptureStartup> = None;
        let mut mode_errors: Vec<String> = Vec::new();

        for (attempt_index, attempt) in attempts.into_iter().enumerate() {
            append_capture_log_line(
                &capture_log_path,
                &format!(
                    "=== audio mode: {} mic_backend={} ===",
                    attempt.mode.as_str(),
                    attempt.mic_backend
                ),
            );
            match start_capture_via_sck_bridge(
                &ffmpeg_bin,
                &helper_bin,
                &settings,
                &segment_dir,
                &capture_log_path,
                segment_duration_secs,
                attempt.mode,
                &attempt.mic_backend,
                queue_profile,
                attempt_index,
            ) {
                Ok(active) => {
                    startup = Some(active);
                    break;
                }
                Err(err) => {
                    append_capture_log_line(
                        &capture_log_path,
                        &format!(
                            "audio mode {} mic_backend={} failed: {err}",
                            attempt.mode.as_str(),
                            attempt.mic_backend
                        ),
                    );
                    mode_errors.push(format!(
                        "{} [{}] => {}",
                        attempt.mode.as_str(),
                        attempt.mic_backend,
                        err
                    ));
                }
            }
        }

        let startup = startup.ok_or_else(|| {
            let tail = read_capture_log_tail(&capture_log_path, 24).unwrap_or_default();
            format!(
                "failed to start capture after trying all audio modes: {}. ffmpeg log: {tail}",
                mode_errors.join(" || ")
            )
        })?;
        let startup_fallback_error = if startup.active_audio_mode.as_str() != settings.audio_mode
            && !mode_errors.is_empty()
        {
            let specific = if mode_errors.iter().any(|err| {
                err.contains("mic_pipe_startup_stalled")
                    || err.contains("mic_first_frame_startup_stalled")
            }) {
                let code = if mode_errors
                    .iter()
                    .any(|err| err.contains("mic_first_frame_startup_stalled"))
                {
                    "mic_first_frame_startup_stalled"
                } else {
                    "mic_pipe_startup_stalled"
                };
                let message = if code == "mic_first_frame_startup_stalled" {
                    "mixed microphone backend started but no first usable mic frame reached ffmpeg; continuing with system audio only."
                } else {
                    "mixed microphone pipe startup stalled before stable segments; continuing with system audio only."
                };
                Some(format!("{code}: {message}"))
            } else {
                mode_errors.last().cloned()
            };
            if let Some(error) = specific.clone() {
                if error.contains("mic_pipe_startup_stalled")
                    || error.contains("mic_first_frame_startup_stalled")
                {
                    let code = if error.contains("mic_first_frame_startup_stalled") {
                        "mic_first_frame_startup_stalled"
                    } else {
                        "mic_pipe_startup_stalled"
                    };
                    append_capture_log_line(
                        &capture_log_path,
                        &format!(
                            "phase: startup_audio_fallback_cause code={} from={} to={}",
                            code,
                            settings.audio_mode,
                            startup.active_audio_mode.as_str()
                        ),
                    );
                }
            }
            specific
        } else {
            None
        };
        if let Err(err) = write_capture_lock_payload(
            &mut capture_lock_file,
            owner_pid,
            &startup.session_id,
            capture_lock_started_epoch_ms(),
        ) {
            append_capture_log_line(
                &capture_log_path,
                &format!("phase: capture_lock_write_failed detail={err}"),
            );
        } else {
            append_capture_log_line(
                &capture_log_path,
                &format!(
                    "phase: capture_lock_acquired owner_pid={} session_id={}",
                    owner_pid, startup.session_id
                ),
            );
        }

        let _ = fs::write(&ffmpeg_pid_file, startup.ffmpeg_child.id().to_string());
        let _ = fs::write(&helper_pid_file, startup.helper_child.id().to_string());

        let CaptureStartup {
            ffmpeg_child: startup_ffmpeg_child,
            helper_child: startup_helper_child,
            active_audio_mode,
            mic_backend_in_use,
            pipes,
            session_id,
            attempt_log_offset,
        } = startup;

        let stop = Arc::new(AtomicBool::new(false));
        let prune_frozen = Arc::new(AtomicBool::new(false));
        let ffmpeg_child = Arc::new(Mutex::new(Some(startup_ffmpeg_child)));
        let helper_child = Arc::new(Mutex::new(Some(startup_helper_child)));
        let stop_signal = Arc::clone(&stop);
        let prune_frozen_signal = Arc::clone(&prune_frozen);
        let ffmpeg_signal = Arc::clone(&ffmpeg_child);
        let helper_signal = Arc::clone(&helper_child);
        let segment_dir_signal = segment_dir.clone();
        let capture_log_signal = capture_log_path.clone();
        let capture_log_offset = Arc::new(AtomicU64::new(attempt_log_offset));
        let capture_log_offset_signal = Arc::clone(&capture_log_offset);
        let retention_secs = settings.buffer_duration_secs;
        let retention_segment_secs = segment_duration_secs;
        let last_error = Arc::new(Mutex::new(None));
        let error_signal = Arc::clone(&last_error);

        let worker = thread::spawn(move || {
            let mut segment_dir_permission_failures: u8 = 0;
            while !stop_signal.load(Ordering::Relaxed) {
                if prune_frozen_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                if let Err(err) =
                    prune_old_segments(&segment_dir_signal, retention_secs, retention_segment_secs)
                {
                    if is_output_dir_permission_error(&err) {
                        segment_dir_permission_failures =
                            segment_dir_permission_failures.saturating_add(1);
                        append_capture_log_line(
                            &capture_log_signal,
                            &format!(
                                "phase: segment_dir_access_error count={} code=permission_denied",
                                segment_dir_permission_failures
                            ),
                        );
                        if segment_dir_permission_failures
                            >= SEGMENT_DIR_PERMISSION_FAILURE_THRESHOLD
                        {
                            append_capture_log_line(
                                &capture_log_signal,
                                &format!(
                                    "phase: output_dir_permission_denied path={}",
                                    segment_dir_signal.display()
                                ),
                            );
                            *error_signal.lock() = Some(format!(
                                "output_dir_permission_required: {}",
                                strip_output_dir_permission_prefix(&err)
                            ));
                            break;
                        }
                    } else {
                        segment_dir_permission_failures = 0;
                        append_capture_log_line(
                            &capture_log_signal,
                            &format!("phase: segment_dir_access_error code=transient detail={err}"),
                        );
                    }
                } else if segment_dir_permission_failures > 0 {
                    segment_dir_permission_failures = 0;
                }

                {
                    let mut helper_guard = helper_signal.lock();
                    let Some(helper_child) = helper_guard.as_mut() else {
                        break;
                    };

                    match helper_child.try_wait() {
                        Ok(Some(status)) => {
                            let log_tail = read_capture_log_tail_since(
                                &capture_log_signal,
                                capture_log_offset_signal.load(Ordering::Relaxed),
                                48,
                            )
                            .unwrap_or_default();
                            if status.code() == Some(HELPER_INTERRUPTED_EXIT_CODE)
                                || is_user_stopped_sharing_log(&log_tail)
                            {
                                *error_signal.lock() = Some(format!(
                                    "user_stopped_sharing: ScreenCaptureKit capture was stopped by macOS screen-recording controls. status={status}. log: {log_tail}"
                                ));
                            } else {
                                *error_signal.lock() = Some(format!(
                                    "ScreenCaptureKit helper exited unexpectedly with status {status}"
                                ));
                            }
                            break;
                        }
                        Ok(None) => {}
                        Err(err) => {
                            *error_signal.lock() = Some(format!(
                                "ScreenCaptureKit helper status check failed: {err}"
                            ));
                            break;
                        }
                    }
                }

                {
                    let mut ffmpeg_guard = ffmpeg_signal.lock();
                    let Some(ffmpeg_child) = ffmpeg_guard.as_mut() else {
                        break;
                    };

                    match ffmpeg_child.try_wait() {
                        Ok(Some(status)) => {
                            *error_signal.lock() = Some(format!(
                                "ffmpeg encoder exited unexpectedly with status {status}"
                            ));
                            break;
                        }
                        Ok(None) => {}
                        Err(err) => {
                            *error_signal.lock() =
                                Some(format!("ffmpeg capture status check failed: {err}"));
                            break;
                        }
                    }
                }

                thread::sleep(Duration::from_millis(500));
            }
        });

        Ok(Self {
            segment_dir,
            capture_log_path,
            capture_lock_path,
            capture_lock_file: Some(capture_lock_file),
            capture_owner_pid: owner_pid,
            ffmpeg_pid_file,
            helper_pid_file,
            video_pipe_path: pipes.video_pipe,
            system_audio_pipe_path: pipes.system_audio_pipe,
            mic_audio_pipe_path: pipes.mic_audio_pipe,
            buffer_duration_secs: settings.buffer_duration_secs,
            segment_duration_secs,
            stop,
            ffmpeg_child,
            helper_child,
            worker: Some(worker),
            last_error,
            startup_fallback_error,
            active_audio_mode,
            session_id,
            capture_log_offset,
            mic_backend_in_use,
            queue_profile,
            display_signature: current_display_signature(),
            display_change_seen_at: Arc::new(Mutex::new(None)),
            prune_frozen,
            log_metrics: Mutex::new(LogMetricsCache::default()),
        })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        append_capture_log_line(&self.capture_log_path, "phase: capture_stop_requested");

        if let Some(mut child) = self.ffmpeg_child.lock().take() {
            let pid = child.id();
            let outcome = terminate_child_gracefully(&mut child);
            let reap_incomplete = outcome.status.is_none() || process_is_running(pid);
            append_capture_log_line(
                &self.capture_log_path,
                &format!(
                    "phase: process_reaped name=ffmpeg pid={} forced_kill={} status={}",
                    pid,
                    outcome.forced_kill,
                    outcome
                        .status
                        .map(|status| status.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            );
            if reap_incomplete {
                append_capture_log_line(
                    &self.capture_log_path,
                    &format!(
                        "phase: process_reap_incomplete name=ffmpeg pid={} forced_kill={} status_known={}",
                        pid,
                        outcome.forced_kill,
                        outcome.status.is_some()
                    ),
                );
            }
        }

        if let Some(mut child) = self.helper_child.lock().take() {
            let pid = child.id();
            let outcome = terminate_child_gracefully(&mut child);
            let reap_incomplete = outcome.status.is_none() || process_is_running(pid);
            append_capture_log_line(
                &self.capture_log_path,
                &format!(
                    "phase: process_reaped name=sck_helper pid={} forced_kill={} status={}",
                    pid,
                    outcome.forced_kill,
                    outcome
                        .status
                        .map(|status| status.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            );
            if reap_incomplete {
                append_capture_log_line(
                    &self.capture_log_path,
                    &format!(
                        "phase: process_reap_incomplete name=sck_helper pid={} forced_kill={} status_known={}",
                        pid,
                        outcome.forced_kill,
                        outcome.status.is_some()
                    ),
                );
            }
        }

        let _ = fs::remove_file(&self.ffmpeg_pid_file);
        let _ = fs::remove_file(&self.helper_pid_file);

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }

        self.capture_lock_file.take();
        let _ = fs::remove_file(&self.capture_lock_path);

        let _ = fs::remove_file(&self.video_pipe_path);
        if let Some(path) = &self.system_audio_pipe_path {
            let _ = fs::remove_file(path);
        }
        if let Some(path) = &self.mic_audio_pipe_path {
            let _ = fs::remove_file(path);
        }
        let mut sweep_exclusions = HashSet::new();
        sweep_exclusions.insert(self.capture_owner_pid);
        let _ = sweep_orphan_capture_processes(
            &self.capture_log_path,
            "post_stop_cleanup",
            &sweep_exclusions,
        );
        append_capture_log_line(&self.capture_log_path, "phase: capture_stop_completed");
    }

    pub fn buffer_fill_secs(&self) -> f32 {
        match self.recent_segment_files() {
            Ok(files) => {
                let fill = files.len() as f32 * self.segment_duration_secs;
                fill.min(f32::from(self.buffer_duration_secs))
            }
            Err(_) => 0.0,
        }
    }

    pub fn replay_fill_secs(&self, replay_duration_secs: u16) -> f32 {
        self.replay_selection_for_save_at(replay_duration_secs, SystemTime::now())
            .ok()
            .flatten()
            .map(|selection| selection.target_trim_secs)
            .unwrap_or(0.0)
            .min(f32::from(replay_duration_secs))
    }

    pub fn rolling_fill_secs(&self) -> f32 {
        self.buffer_fill_secs()
    }
}

impl Drop for CaptureEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

mod log_helpers;
mod log_parsers;
mod process_helpers;
mod replay_runtime;
mod startup_helpers;

use log_helpers::*;
use log_parsers::*;
use process_helpers::*;
use startup_helpers::*;

#[cfg(test)]
mod tests;
