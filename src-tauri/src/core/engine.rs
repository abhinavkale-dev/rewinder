use std::collections::VecDeque;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};
use std::{fs, path::PathBuf};

use parking_lot::Mutex;
use crate::events::EngineHost;

use crate::capture::engine::{
    AudioStartupStrategy, CaptureEngine, LiveQueueProfile,
    MicAttachRuntimeState as CaptureMicAttachState, ReplaySelection,
};
use crate::capture::permissions::{self, MicrophonePermissionStatus};
use crate::core::lifecycle;
use crate::core::state::{
    AudioHealthDto, CaptureHealthDto, ClipMetadataDto, ClipperState, EngineStateDto,
    GrantMicrophoneAccessResultDto, GrantOutputDirAccessResultDto,
    GrantScreenRecordingAccessResultDto, HotkeyStatusDto, LifecycleState, MicAttachStateDto,
    PermissionStateDto, SaveReplayResultDto, SaveStageDto, TriggerSourceDto, VideoSmoothStateDto,
    MAX_RECENT_CLIPS,
};
use crate::events;
use crate::hotkeys::RegistrationMode;
use crate::settings::{ensure_buffer_for_replay, ResolutionPreset, SettingsDto, SettingsPatchDto};
use crate::writer::replay_writer;

struct PipelineHandles {
    capture: CaptureEngine,
}

const NO_SEGMENTS_MIN_STALL_MS: u64 = 2_500;
const NO_SEGMENTS_MAX_STALL_MS: u64 = 6_000;
const NO_SEGMENTS_MISS_REQUIRED: u8 = 2;
const POST_PIPELINE_START_GRACE_MS: u64 = 3_500;
const POST_SAVE_START_GRACE_MS: u64 = 2_500;
const DISPLAY_CHANGE_DEBOUNCE_MS: u64 = 800;
const RESTART_LOOP_WINDOW_SECS: u64 = 20;
const RESTART_LOOP_MAX_ATTEMPTS: usize = 3;
const RESTART_LOOP_COOLDOWN_SECS: u64 = 10;
const PLAYBACK_OVERLOAD_THRESHOLD_X: f32 = 0.95;
const PLAYBACK_EMERGENCY_OVERLOAD_THRESHOLD_X: f32 = 0.90;
const PLAYBACK_OVERLOAD_HOLD_SECS: u64 = 8;
const PLAYBACK_RECOVER_THRESHOLD_X: f32 = 0.97;
const PLAYBACK_RECOVER_HOLD_SECS: u64 = 8;
const PROFILE_CHANGE_COOLDOWN_SECS: u64 = 12;
const PROFILE_CHANGE_DWELL_SECS: u64 = 10;
const STARTUP_PROFILE_STABILIZATION_FREEZE_SECS: u64 = 30;
const NON_CRITICAL_SAVE_RESTART_SUPPRESSION_MS: u64 = POST_SAVE_START_GRACE_MS * 2;
const STARTUP_PERF_GUARD_SECS: u64 = 10;
const STARTUP_BOOTSTRAP_SECS: u64 = 12;
const STARTUP_BOOTSTRAP_PROFILE_INDEX: usize = 2;
const MAX_RUNTIME_PROFILE_INDEX: usize = 3;
const BATTERY_FLOOR_HOLD_SECS: u64 = 5;
const STARTUP_NO_SEGMENTS_EXTRA_THRESHOLD_MS: u64 = 4_500;
const MIC_RECOVERY_MIN_STABLE_SECS: u64 = 8;
const MIC_SIGNAL_SILENT_DBFS_THRESHOLD: f32 = -65.0;
const MIC_SIGNAL_SILENCE_HOLD_SECS: u64 = 5;
const MIC_OFFLINE_WATCHDOG_MULTIPLIER: u64 = 3;
const SYSTEM_AUDIO_STARTUP_GRACE_MS: u64 = 7_000;
const SYSTEM_AUDIO_DROPOUT_GRACE_MS: u64 = 2_500;
const SYSTEM_AUDIO_HARD_FAIL_AFTER_MS: u64 = 8_000;
const AUDIO_WARMUP_MIN_DEFER_TTL_MS: u64 = SYSTEM_AUDIO_HARD_FAIL_AFTER_MS + 1_500;
const STARTUP_INTERRUPT_MAX_RETRIES: u8 = 2;
const STARTUP_INTERRUPT_WINDOW_SECS: u64 = 12;
const PROCESS_DIAGNOSTICS_SAMPLE_INTERVAL_MS: u64 = 3_000;
const RESOURCE_SOFT_PRESSURE_HOLD_SECS: u64 = 8;
const RESOURCE_HARD_PRESSURE_HOLD_SECS: u64 = 4;
const RESOURCE_SOFT_TRIGGER_WINDOW_SECS: u64 = 90;
const RESOURCE_HARD_TRIGGER_REPEAT_COUNT: usize = 2;
const CAPTURE_STACK_CPU_SOFT_THRESHOLD_PCT: f32 = 120.0;
const CAPTURE_STACK_CPU_HARD_THRESHOLD_PCT: f32 = 180.0;
const OVERLOAD_DROP_DELTA_THRESHOLD: u64 = 20;
const OVERLOAD_DROP_EMERGENCY_DELTA_THRESHOLD: u64 = 40;
const OVERLOAD_OUTPUT_FPS_RATIO_THRESHOLD: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingSaveReason {
    Retryable,
    AudioWarmup,
}

#[derive(Debug, Clone)]
struct PendingSaveRequest {
    source: TriggerSourceDto,
    expires_at: Instant,
    anchor_time: SystemTime,
    reason: PendingSaveReason,
    requested_replay_secs: u16,
}

#[derive(Debug, Clone, Copy)]
enum PendingSaveEnqueueOutcome {
    QueuedNew,
    ReplacedExisting { previous_anchor_time: SystemTime },
}

#[derive(Debug, Clone)]
struct PendingSmoothJob {
    save_id: u64,
    source: TriggerSourceDto,
    clip_path: PathBuf,
    settings: SettingsDto,
}

#[derive(Debug, Clone)]
struct PendingFastVerifyJob {
    save_id: u64,
    source: TriggerSourceDto,
    clip_path: PathBuf,
    settings: SettingsDto,
    save_audio_strategy: String,
}

#[derive(Debug, Clone)]
struct ProcessDiagnosticsSnapshot {
    sampled_at: Instant,
    app_rss_mb: Option<u32>,
    app_cpu_percent: Option<f32>,
    capture_stack_rss_mb: Option<u32>,
    capture_stack_cpu_percent: Option<f32>,
    capture_stack_rss_delta_mb: Option<u32>,
    thermal_state: Option<String>,
    power_source: Option<String>,
}

#[derive(Debug, Clone)]
struct PerfGuardTransitionRecord {
    action: String,
    hard: bool,
    primary_reason_code: Option<String>,
    contributing_reason_codes: Vec<String>,
    suppressed_reason_code: Option<String>,
    from_profile: Option<String>,
    to_profile: Option<String>,
    sampled_at_epoch_ms: i64,
}

#[derive(Debug, Clone)]
struct SaveBlocker {
    code: &'static str,
    message: String,
    action: Option<String>,
    retryable: bool,
}

#[derive(Debug, Clone, Copy)]
enum CaptureRestartReason {
    MissingPipeline,
    CaptureStartInterrupted,
    NoSegments {
        segment_age_ms: Option<u64>,
        threshold_ms: u64,
        miss_count: u8,
    },
    Overloaded,
    ProfileRecovered,
    UserStoppedSharing,
    CaptureProcessExited,
    DisplayChanged,
    PowerSourceChanged,
}

impl CaptureRestartReason {
    fn as_code(self) -> &'static str {
        match self {
            Self::MissingPipeline => "missing_pipeline",
            Self::CaptureStartInterrupted => "capture_start_interrupted",
            Self::NoSegments { .. } => "no_segments",
            Self::Overloaded => "capture_overloaded",
            Self::ProfileRecovered => "capture_profile_recovered",
            Self::UserStoppedSharing => "user_stopped_sharing",
            Self::CaptureProcessExited => "capture_process_exited",
            Self::DisplayChanged => "display_changed",
            Self::PowerSourceChanged => "power_source_changed",
        }
    }

    fn as_message(self) -> &'static str {
        match self {
            Self::MissingPipeline => "Capture pipeline missing; attempting restart.",
            Self::CaptureStartInterrupted => "Capture startup interrupted; retrying.",
            Self::NoSegments { .. } => "Capture stalled (no recent segments); restarting.",
            Self::Overloaded => "Capture overloaded; reducing quality profile.",
            Self::ProfileRecovered => "Capture stable; restoring quality profile.",
            Self::UserStoppedSharing => {
                "Screen recording was interrupted. Click Restart Capture to resume."
            }
            Self::CaptureProcessExited => "Capture process exited; restarting.",
            Self::DisplayChanged => "Display source changed; restarting capture.",
            Self::PowerSourceChanged => "Power source changed; adjusting quality.",
        }
    }

    fn detail(self) -> Option<String> {
        match self {
            Self::NoSegments {
                segment_age_ms,
                threshold_ms,
                miss_count,
            } => Some(format!(
                "segment_age_ms={} threshold_ms={} miss_count={}",
                segment_age_ms
                    .map(|age| age.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                threshold_ms,
                miss_count
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EffectiveCaptureProfile {
    video_resolution: u16,
    fps: u16,
    video_bitrate_kbps: u32,
}

impl EffectiveCaptureProfile {
    fn label(self) -> String {
        format!(
            "{}p{}fps-{}k",
            self.video_resolution, self.fps, self.video_bitrate_kbps
        )
    }
}

pub struct Engine {
    state: Arc<Mutex<ClipperState>>,
    pipeline: Mutex<Option<PipelineHandles>>,
    pipeline_transition: Mutex<()>,
    runtime_mutation_gate: Mutex<()>,
    save_entry_gate: Mutex<()>,
    pending_save: Mutex<Option<PendingSaveRequest>>,
    pending_smooth_jobs: Mutex<VecDeque<PendingSmoothJob>>,
    pending_fast_verify_jobs: Mutex<VecDeque<PendingFastVerifyJob>>,
    fast_verify_inflight: AtomicBool,
    save_operation_seq: AtomicU64,
    restart_history: Mutex<Vec<Instant>>,
    crash_loop_cooldown_until: Mutex<Option<Instant>>,
    runtime_profile_index: Mutex<usize>,
    overload_since: Mutex<Option<Instant>>,
    recover_since: Mutex<Option<Instant>>,
    last_mic_retry_at: Mutex<Option<Instant>>,
    no_segments_miss_count: Mutex<u8>,
    last_pipeline_started_at: Mutex<Option<Instant>>,
    last_save_started_at: Mutex<Option<Instant>>,
    last_profile_change_at: Mutex<Option<Instant>>,
    next_queue_profile: Mutex<LiveQueueProfile>,
    startup_bootstrap_until: Mutex<Option<Instant>>,
    startup_bootstrap_pending: Mutex<bool>,
    startup_requested_profile_hold_logged: Mutex<bool>,
    startup_interrupt_window_started_at: Mutex<Option<Instant>>,
    startup_interrupt_retry_count: Mutex<u8>,
    last_drop_total: Mutex<u64>,
    last_overflow_total: Mutex<u64>,
    process_diagnostics_cache: Mutex<Option<ProcessDiagnosticsSnapshot>>,
    capture_stack_rss_baseline_mb: Mutex<Option<u32>>,
    resource_soft_pressure_since: Mutex<Option<Instant>>,
    resource_hard_pressure_since: Mutex<Option<Instant>>,
    resource_soft_trigger_timestamps: Mutex<Vec<Instant>>,
    resource_hard_stepdown_pending: Mutex<bool>,
    battery_floor_since: Mutex<Option<Instant>>,
    battery_floor_engaged: Mutex<bool>,
    pending_guard_transition: Mutex<Option<PerfGuardTransitionRecord>>,
    capture_paused_by_user: Mutex<bool>,
    capture_pause_reason: Mutex<Option<String>>,
    user_stop_disarmed_reason: Mutex<Option<String>>,
    system_audio_not_ready_since: Mutex<Option<Instant>>,
    last_system_audio_ready_at: Mutex<Option<Instant>>,
    system_audio_hard_unavailable_logged: Mutex<bool>,
    mic_signal_silence_since: Mutex<Option<Instant>>,
    mic_signal_warning_emitted: Mutex<bool>,
    mic_offline_since: Mutex<Option<Instant>>,
    mic_offline_watchdog_warned: Mutex<bool>,
    mic_device_not_found_warned: Mutex<bool>,
    recovery_stop: Arc<AtomicBool>,
    recovery_worker: Mutex<Option<JoinHandle<()>>>,
    restart_in_flight: AtomicBool,
    restart_pending: AtomicBool,
    restart_rollback_target: Mutex<Option<SettingsDto>>,
}

impl Engine {
    pub fn new(settings: SettingsDto, permission: PermissionStateDto) -> Arc<Self> {
        Arc::new(Self {
            state: Arc::new(Mutex::new(ClipperState::new(settings, permission))),
            pipeline: Mutex::new(None),
            pipeline_transition: Mutex::new(()),
            runtime_mutation_gate: Mutex::new(()),
            save_entry_gate: Mutex::new(()),
            pending_save: Mutex::new(None),
            pending_smooth_jobs: Mutex::new(VecDeque::new()),
            pending_fast_verify_jobs: Mutex::new(VecDeque::new()),
            fast_verify_inflight: AtomicBool::new(false),
            save_operation_seq: AtomicU64::new(0),
            restart_history: Mutex::new(Vec::new()),
            crash_loop_cooldown_until: Mutex::new(None),
            runtime_profile_index: Mutex::new(0),
            overload_since: Mutex::new(None),
            recover_since: Mutex::new(None),
            last_mic_retry_at: Mutex::new(None),
            no_segments_miss_count: Mutex::new(0),
            last_pipeline_started_at: Mutex::new(None),
            last_save_started_at: Mutex::new(None),
            last_profile_change_at: Mutex::new(None),
            next_queue_profile: Mutex::new(LiveQueueProfile::Small),
            startup_bootstrap_until: Mutex::new(None),
            startup_bootstrap_pending: Mutex::new(false),
            startup_requested_profile_hold_logged: Mutex::new(false),
            startup_interrupt_window_started_at: Mutex::new(None),
            startup_interrupt_retry_count: Mutex::new(0),
            last_drop_total: Mutex::new(0),
            last_overflow_total: Mutex::new(0),
            process_diagnostics_cache: Mutex::new(None),
            capture_stack_rss_baseline_mb: Mutex::new(None),
            resource_soft_pressure_since: Mutex::new(None),
            resource_hard_pressure_since: Mutex::new(None),
            resource_soft_trigger_timestamps: Mutex::new(Vec::new()),
            resource_hard_stepdown_pending: Mutex::new(false),
            battery_floor_since: Mutex::new(None),
            battery_floor_engaged: Mutex::new(false),
            pending_guard_transition: Mutex::new(None),
            capture_paused_by_user: Mutex::new(false),
            capture_pause_reason: Mutex::new(None),
            user_stop_disarmed_reason: Mutex::new(None),
            system_audio_not_ready_since: Mutex::new(None),
            last_system_audio_ready_at: Mutex::new(None),
            system_audio_hard_unavailable_logged: Mutex::new(false),
            mic_signal_silence_since: Mutex::new(None),
            mic_signal_warning_emitted: Mutex::new(false),
            mic_offline_since: Mutex::new(None),
            mic_offline_watchdog_warned: Mutex::new(false),
            mic_device_not_found_warned: Mutex::new(false),
            recovery_stop: Arc::new(AtomicBool::new(false)),
            recovery_worker: Mutex::new(None),
            restart_in_flight: AtomicBool::new(false),
            restart_pending: AtomicBool::new(false),
            restart_rollback_target: Mutex::new(None),
        })
    }

    pub fn initialize(self: &Arc<Self>, app: &Arc<dyn EngineHost>) -> Result<(), String> {
        let mic_probe = permissions::probe_microphone_permission(false);
        {
            let mut state = self.state.lock();
            state.lifecycle_state =
                lifecycle::boot_state(&state.permission, state.settings.replay_enabled);
            state.capture_health = if state.settings.replay_enabled {
                CaptureHealthDto::Starting
            } else {
                CaptureHealthDto::Stopped
            };
            state.audio_health = if state.settings.replay_enabled {
                AudioHealthDto::Degraded
            } else {
                AudioHealthDto::Unavailable
            };
            state.system_audio_path_ready = false;
            state.mic_path_ready = false;
            state.mic_frames_seen = false;
            state.mic_level_dbfs = None;
            state.mic_capture_session_running = false;
            state.mic_samples_per_sec = None;
            state.mic_attach_state = MicAttachStateDto::Inactive;
            state.mic_recovery_state = "ok".to_string();
            state.selected_microphone_name = None;
            state.last_mic_error_code = None;
            state.last_mic_error_message = None;
            state.concurrent_session_count = None;
            state.capture_owner_pid = None;
            state.audio_path_ready = false;
            state.first_audio_frame_seen = false;
            state.save_stage = SaveStageDto::Idle;
            state.video_smooth_state = VideoSmoothStateDto::Idle;
            state.capture_crash_loop = false;
            state.mic_permission_status = mic_probe.status.as_str().to_string();
            state.mic_permission_error = mic_probe.error.clone();
        }

        if let Err(err) = self.register_hotkeys(app, "startup") {
            let message = format!("failed to register hotkey during startup: {err}");
            {
                let mut state = self.state.lock();
                state.last_error = Some(message.clone());
                state.hotkey_status = HotkeyStatusDto::Conflict;
            }
            events::emit_hotkey_conflict(
                app,
                message.clone(),
                Some("Use tray Save Replay or choose a different shortcut.".to_string()),
            );
        }

        self.start_pipeline_recovery_worker(app.clone());

        let engine_state = self.get_engine_state();
        events::emit_engine_state(app, &engine_state);
        events::emit_mic_permission_changed(
            app,
            engine_state.mic_permission_status.clone(),
            engine_state.mic_permission_error.clone(),
        );
        if engine_state.lifecycle_state == LifecycleState::PermissionRequired {
            events::emit_permission_required(
                app,
                engine_state
                    .permission
                    .reason
                    .clone()
                    .unwrap_or_else(|| "Permission required".to_string()),
            );
        }

        Ok(())
    }

    pub fn get_settings(&self) -> SettingsDto {
        self.state.lock().settings.clone()
    }

    pub fn list_recent_clips(&self, limit: Option<usize>) -> Vec<ClipMetadataDto> {
        let state = self.state.lock();
        let limit = limit.unwrap_or(MAX_RECENT_CLIPS);
        state.recent_clips.iter().take(limit).cloned().collect()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.recovery_stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.recovery_worker.get_mut().take() {
            let _ = worker.join();
        }
        self.stop_pipeline_if_running();
    }
}

mod capture_errors;
mod helpers;
mod permissions_handler;
mod process_metrics;
mod pipeline;
mod profile;
mod recovery;
mod restart_policy;
mod runtime_basics;
mod save_pipeline;
mod save_trigger;
mod settings_patch;
mod state_projection;
use capture_errors::*;
use helpers::*;
use process_metrics::*;
#[cfg(test)]
mod tests;
