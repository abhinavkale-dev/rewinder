use serde::{Deserialize, Serialize};

use crate::settings::SettingsDto;

pub const MAX_RECENT_CLIPS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Booting,
    PermissionRequired,
    Armed,
    SavingReplay,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureHealthDto {
    Starting,
    Running,
    Restarting,
    Degraded,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyStatusDto {
    Ok,
    Conflict,
    Fallback,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioHealthDto {
    Ok,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveStageDto {
    Idle,
    Queued,
    SavingFast,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoSmoothStateDto {
    Idle,
    Pending,
    Processing,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicAttachStateDto {
    Inactive,
    SilenceFiller,
    Live,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneDeviceDto {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStateDto {
    pub screen_recording_granted: bool,
    pub system_audio_granted: bool,
    pub output_dir_writable: bool,
    pub output_dir_permission_error: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStateDto {
    pub lifecycle_state: LifecycleState,
    pub capture_health: CaptureHealthDto,
    pub audio_health: AudioHealthDto,
    pub save_stage: SaveStageDto,
    pub system_audio_path_ready: bool,
    pub system_audio_ready: bool,
    pub mic_path_ready: bool,
    pub mic_ready: bool,
    pub mic_frames_seen: bool,
    pub mic_level_dbfs: Option<f32>,
    pub mic_permission_status: String,
    pub mic_permission_error: Option<String>,
    pub mic_capture_session_running: bool,
    pub mic_samples_per_sec: Option<u32>,
    pub mic_attach_state: MicAttachStateDto,
    pub mic_recovery_state: String,
    pub mic_signal_silent: bool,
    pub selected_microphone_id: Option<String>,
    pub selected_microphone_name: Option<String>,
    pub last_mic_error_code: Option<String>,
    pub last_mic_error_message: Option<String>,
    pub audio_path_ready: bool,
    pub first_audio_frame_seen: bool,
    pub capture_speed_x: Option<f32>,
    pub encoder_throughput_x: Option<f32>,
    pub playback_realtime_x: Option<f32>,
    pub playback_stability: String,
    pub capture_load_state: String,
    pub operator_health_state: String,
    pub operator_health_message: String,
    pub guard_state: String,
    pub guard_primary_reason_code: Option<String>,
    pub guard_contributing_reason_codes: Vec<String>,
    pub guard_suppressed_reason_code: Option<String>,
    pub guard_last_transition_at_epoch_ms: Option<i64>,
    pub live_queue_profile: String,
    pub save_ready: bool,
    pub hotkey_status: HotkeyStatusDto,
    pub active_audio_mode: String,
    pub effective_audio_mode: String,
    pub capture_backend: String,
    pub mic_backend_in_use: String,
    pub mic_mix_gain_db: f32,
    pub requested_video_resolution: u16,
    pub requested_fps: u16,
    pub requested_video_bitrate_kbps: u32,
    pub effective_video_resolution: u16,
    pub effective_fps: u16,
    pub effective_video_bitrate_kbps: u32,
    pub audio_fallback_policy: String,
    pub degrade_reason: Option<String>,
    pub audio_degrade_reason: Option<String>,
    pub last_audio_mode_error: Option<String>,
    pub capture_restart_count: u32,
    pub capture_interrupt_count: u32,
    pub video_smooth_state: VideoSmoothStateDto,
    pub capture_dropped_frames: u64,
    pub capture_queue_overflows: u64,
    pub effective_output_fps: Option<f32>,
    pub concurrent_session_count: Option<u8>,
    pub capture_owner_pid: Option<u32>,
    pub app_rss_mb: Option<u32>,
    pub app_cpu_percent: Option<f32>,
    pub capture_stack_rss_mb: Option<u32>,
    pub capture_stack_cpu_percent: Option<f32>,
    pub capture_stack_rss_delta_mb: Option<u32>,
    pub system_memory_pressure_level: Option<String>,
    pub thermal_state: Option<String>,
    pub power_source: Option<String>,
    pub capture_crash_loop: bool,
    pub is_armed: bool,
    pub is_saving: bool,
    pub arm_blocker: Option<String>,
    pub arm_blocker_code: Option<String>,
    pub arm_blocker_action: Option<String>,
    pub pending_save: bool,
    pub pending_full_window: bool,
    pub pending_full_window_deadline_epoch_ms: Option<i64>,
    pub full_window_wait_remaining_ms: Option<u32>,
    pub warmup_eta_ms: Option<u32>,
    pub audio_warmup_grace_ms: Option<u32>,
    pub buffer_fill_secs: f32,
    pub replay_fill_secs: f32,
    pub replay_target_secs: u16,
    pub rolling_fill_secs: f32,
    pub rolling_target_secs: u16,
    pub last_error: Option<String>,
    pub last_capture_log_tail: Option<String>,
    pub capture_start_phase: Option<String>,
    pub dropped_video_packets: u64,
    pub dropped_audio_packets: u64,
    pub last_contiguity_break_code: Option<String>,
    pub permission: PermissionStateDto,
    pub settings: SettingsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipMetadataDto {
    pub id: String,
    pub path: String,
    pub created_at_epoch_ms: i64,
    pub duration_secs: f32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSourceDto {
    Manual,
    Hotkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReplayResultDto {
    pub ok: bool,
    pub queued: bool,
    pub clip: Option<ClipMetadataDto>,
    pub error: Option<String>,
    pub message: Option<String>,
    pub actual_duration_secs: Option<f32>,
    pub audio_repaired: bool,
    pub save_audio_strategy: Option<String>,
    pub smooth_pending: bool,
    pub smooth_applied: bool,
    pub smooth_error: Option<String>,
    pub effective_video_resolution: Option<u16>,
    pub effective_fps: Option<u16>,
    pub requested_duration_secs: Option<f32>,
    pub selected_duration_secs: Option<f32>,
    pub contiguous_duration_secs: Option<f32>,
    pub partial_reason_code: Option<String>,
    pub anchor_epoch_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantOutputDirAccessResultDto {
    pub permission: PermissionStateDto,
    pub opened_settings: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantScreenRecordingAccessResultDto {
    pub permission: PermissionStateDto,
    pub opened_settings: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantMicrophoneAccessResultDto {
    pub permission: PermissionStateDto,
    pub mic_permission_status: String,
    pub mic_permission_error: Option<String>,
    pub opened_settings: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ClipperState {
    pub lifecycle_state: LifecycleState,
    pub capture_health: CaptureHealthDto,
    pub audio_health: AudioHealthDto,
    pub save_stage: SaveStageDto,
    pub system_audio_path_ready: bool,
    pub mic_path_ready: bool,
    pub mic_frames_seen: bool,
    pub mic_level_dbfs: Option<f32>,
    pub mic_permission_status: String,
    pub mic_permission_error: Option<String>,
    pub mic_capture_session_running: bool,
    pub mic_samples_per_sec: Option<u32>,
    pub mic_attach_state: MicAttachStateDto,
    pub mic_recovery_state: String,
    pub mic_signal_silent: bool,
    pub selected_microphone_name: Option<String>,
    pub last_mic_error_code: Option<String>,
    pub last_mic_error_message: Option<String>,
    pub audio_path_ready: bool,
    pub first_audio_frame_seen: bool,
    pub capture_speed_x: Option<f32>,
    pub capture_load_state: String,
    pub guard_state: String,
    pub guard_primary_reason_code: Option<String>,
    pub guard_contributing_reason_codes: Vec<String>,
    pub guard_suppressed_reason_code: Option<String>,
    pub guard_last_transition_at_epoch_ms: Option<i64>,
    pub live_queue_profile: String,
    pub save_ready: bool,
    pub hotkey_status: HotkeyStatusDto,
    pub active_audio_mode: String,
    pub effective_audio_mode: String,
    pub capture_backend: String,
    pub mic_backend_in_use: String,
    pub mic_mix_gain_db: f32,
    pub requested_video_resolution: u16,
    pub requested_fps: u16,
    pub requested_video_bitrate_kbps: u32,
    pub effective_video_resolution: u16,
    pub effective_fps: u16,
    pub effective_video_bitrate_kbps: u32,
    pub audio_fallback_policy: String,
    pub degrade_reason: Option<String>,
    pub audio_degrade_reason: Option<String>,
    pub last_audio_mode_error: Option<String>,
    pub capture_restart_count: u32,
    pub capture_interrupt_count: u32,
    pub video_smooth_state: VideoSmoothStateDto,
    pub capture_dropped_frames: u64,
    pub capture_queue_overflows: u64,
    pub effective_output_fps: Option<f32>,
    pub concurrent_session_count: Option<u8>,
    pub capture_owner_pid: Option<u32>,
    pub system_memory_pressure_level: Option<String>,
    pub capture_crash_loop: bool,
    pub permission: PermissionStateDto,
    pub settings: SettingsDto,
    pub is_saving: bool,
    pub arm_blocker: Option<String>,
    pub arm_blocker_code: Option<String>,
    pub arm_blocker_action: Option<String>,
    pub last_error: Option<String>,
    pub dropped_video_packets: u64,
    pub dropped_audio_packets: u64,
    pub last_contiguity_break_code: Option<String>,
    pub recent_clips: Vec<ClipMetadataDto>,
}

impl ClipperState {
    pub fn new(settings: SettingsDto, permission: PermissionStateDto) -> Self {
        Self {
            lifecycle_state: LifecycleState::Booting,
            capture_health: CaptureHealthDto::Starting,
            audio_health: AudioHealthDto::Unavailable,
            save_stage: SaveStageDto::Idle,
            system_audio_path_ready: false,
            mic_path_ready: false,
            mic_frames_seen: false,
            mic_level_dbfs: None,
            mic_permission_status: "unknown".to_string(),
            mic_permission_error: None,
            mic_capture_session_running: false,
            mic_samples_per_sec: None,
            mic_attach_state: MicAttachStateDto::Inactive,
            mic_recovery_state: "ok".to_string(),
            mic_signal_silent: false,
            selected_microphone_name: None,
            last_mic_error_code: None,
            last_mic_error_message: None,
            audio_path_ready: false,
            first_audio_frame_seen: false,
            capture_speed_x: None,
            capture_load_state: "normal".to_string(),
            guard_state: "idle".to_string(),
            guard_primary_reason_code: None,
            guard_contributing_reason_codes: Vec::new(),
            guard_suppressed_reason_code: None,
            guard_last_transition_at_epoch_ms: None,
            live_queue_profile: "small".to_string(),
            save_ready: false,
            hotkey_status: HotkeyStatusDto::Ok,
            active_audio_mode: settings.audio_mode.clone(),
            effective_audio_mode: settings.audio_mode.clone(),
            capture_backend: "screencapturekit-swift".to_string(),
            mic_backend_in_use: settings.mic_capture_backend.clone(),
            mic_mix_gain_db: settings.mic_mix_gain_db,
            requested_video_resolution: settings.video_resolution,
            requested_fps: settings.fps,
            requested_video_bitrate_kbps: settings.video_bitrate_kbps,
            effective_video_resolution: settings.video_resolution,
            effective_fps: settings.fps,
            effective_video_bitrate_kbps: settings.video_bitrate_kbps,
            audio_fallback_policy: settings.audio_fallback_policy.clone(),
            degrade_reason: None,
            audio_degrade_reason: None,
            last_audio_mode_error: None,
            capture_restart_count: 0,
            capture_interrupt_count: 0,
            video_smooth_state: VideoSmoothStateDto::Idle,
            capture_dropped_frames: 0,
            capture_queue_overflows: 0,
            effective_output_fps: None,
            concurrent_session_count: None,
            capture_owner_pid: None,
            system_memory_pressure_level: None,
            capture_crash_loop: false,
            permission,
            settings,
            is_saving: false,
            arm_blocker: None,
            arm_blocker_code: None,
            arm_blocker_action: None,
            last_error: None,
            dropped_video_packets: 0,
            dropped_audio_packets: 0,
            last_contiguity_break_code: None,
            recent_clips: Vec::new(),
        }
    }

    pub fn is_armed(&self) -> bool {
        self.lifecycle_state == LifecycleState::Armed
    }
}
