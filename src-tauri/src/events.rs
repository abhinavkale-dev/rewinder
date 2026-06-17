use std::sync::Arc;

use serde::Serialize;

use crate::core::state::{ClipMetadataDto, EngineStateDto};
use crate::hotkeys::HotkeyRegistration;

pub trait EngineHost: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
    fn replace_shortcuts(
        &self,
        primary: &str,
        fallbacks: &[String],
    ) -> Result<HotkeyRegistration, String>;
}

fn emit_payload<T: Serialize>(app: &Arc<dyn EngineHost>, event: &str, payload: T) {
    app.emit(
        event,
        serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
    );
}

pub const ENGINE_STATE_CHANGED: &str = "rewinder://engine-state-changed";
pub const CLIP_SAVED: &str = "rewinder://clip-saved";
pub const SAVE_FAILED: &str = "rewinder://save-failed";
pub const SAVE_DEFERRED: &str = "rewinder://save-deferred";
pub const SAVE_WARNING: &str = "rewinder://save-warning";
pub const PERMISSION_REQUIRED: &str = "rewinder://permission-required";
pub const HOTKEY_TRIGGERED: &str = "rewinder://hotkey-triggered";
pub const SETTINGS_UPDATED: &str = "rewinder://settings-updated";
pub const CAPTURE_HEALTH_CHANGED: &str = "rewinder://capture-health-changed";
pub const HOTKEY_CONFLICT: &str = "rewinder://hotkey-conflict";
pub const CAPTURE_RESTARTED: &str = "rewinder://capture-restarted";
pub const AUDIO_MODE_CHANGED: &str = "rewinder://audio-mode-changed";
pub const CAPTURE_DEGRADED: &str = "rewinder://capture-degraded";
pub const CAPTURE_PROFILE_CHANGED: &str = "rewinder://capture-profile-changed";
pub const CAPTURE_PROFILE_RECOVERED: &str = "rewinder://capture-profile-recovered";
pub const CAPTURE_PAUSED: &str = "rewinder://capture-paused";
pub const CAPTURE_RESUMED: &str = "rewinder://capture-resumed";
pub const AUDIO_PATH_FAILED: &str = "rewinder://audio-path-failed";
pub const AUDIO_PATH_READY: &str = "rewinder://audio-path-ready";
pub const MIC_PATH_DEGRADED: &str = "rewinder://mic-path-degraded";
pub const MIC_PATH_RECOVERED: &str = "rewinder://mic-path-recovered";
pub const MIC_PERMISSION_CHANGED: &str = "rewinder://mic-permission-changed";
pub const PERF_GUARD_TRANSITION: &str = "rewinder://perf-guard-transition";

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotkeyPayload {
    pub hotkey: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsUpdatedPayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureHealthPayload {
    pub health: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureRestartedPayload {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioModePayload {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioPathReadyPayload {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MicPermissionPayload {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureProfilePayload {
    pub from: String,
    pub to: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfGuardTransitionPayload {
    pub action: String,
    pub guard_state: String,
    pub hard: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_reason_code: Option<String>,
    pub contributing_reason_codes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressed_reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_profile: Option<String>,
    pub sampled_at_epoch_ms: i64,
}

pub fn emit_engine_state(app: &Arc<dyn EngineHost>, state: &EngineStateDto) {
    emit_payload(app, ENGINE_STATE_CHANGED, state);
}

pub fn emit_clip_saved(app: &Arc<dyn EngineHost>, clip: &ClipMetadataDto) {
    emit_payload(app, CLIP_SAVED, clip);
}

pub fn emit_save_failed(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    emit_save_failed_code(app, "unknown", message, Option::<String>::None);
}

pub fn emit_save_failed_code(
    app: &Arc<dyn EngineHost>,
    code: impl Into<String>,
    message: impl Into<String>,
    action: Option<String>,
) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some(code.into()),
        action,
    };
    emit_payload(app, SAVE_FAILED, payload);
}

pub fn emit_save_deferred(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("deferred".to_string()),
        action: None,
    };
    emit_payload(app, SAVE_DEFERRED, payload);
}

pub fn emit_save_warning(
    app: &Arc<dyn EngineHost>,
    code: impl Into<String>,
    message: impl Into<String>,
    action: Option<String>,
) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some(code.into()),
        action,
    };
    emit_payload(app, SAVE_WARNING, payload);
}

pub fn emit_permission_required(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("permission_required".to_string()),
        action: Some(
            "Enable required permissions in System Settings (Screen Recording and/or Files and Folders > Downloads). In dev, allow Terminal too."
                .to_string(),
        ),
    };
    emit_payload(app, PERMISSION_REQUIRED, payload);
}

pub fn emit_hotkey_triggered(app: &Arc<dyn EngineHost>, hotkey: impl Into<String>) {
    let payload = HotkeyPayload {
        hotkey: hotkey.into(),
    };
    emit_payload(app, HOTKEY_TRIGGERED, payload);
}

pub fn emit_settings_updated(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = SettingsUpdatedPayload {
        message: message.into(),
    };
    emit_payload(app, SETTINGS_UPDATED, payload);
}

pub fn emit_capture_health_changed(
    app: &Arc<dyn EngineHost>,
    health: impl Into<String>,
    reason: Option<String>,
) {
    let payload = CaptureHealthPayload {
        health: health.into(),
        reason,
    };
    emit_payload(app, CAPTURE_HEALTH_CHANGED, payload);
}

pub fn emit_hotkey_conflict(app: &Arc<dyn EngineHost>, message: impl Into<String>, action: Option<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("hotkey_conflict".to_string()),
        action,
    };
    emit_payload(app, HOTKEY_CONFLICT, payload);
}

pub fn emit_capture_restarted(app: &Arc<dyn EngineHost>, reason: impl Into<String>) {
    let payload = CaptureRestartedPayload {
        reason: reason.into(),
    };
    emit_payload(app, CAPTURE_RESTARTED, payload);
}

pub fn emit_audio_mode_changed(app: &Arc<dyn EngineHost>, mode: impl Into<String>, reason: Option<String>) {
    let payload = AudioModePayload {
        mode: mode.into(),
        reason,
    };
    emit_payload(app, AUDIO_MODE_CHANGED, payload);
}

pub fn emit_capture_degraded(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("capture_degraded".to_string()),
        action: Some("Capture auto-degraded to keep replay active.".to_string()),
    };
    emit_payload(app, CAPTURE_DEGRADED, payload);
}

pub fn emit_capture_profile_changed(
    app: &Arc<dyn EngineHost>,
    from: impl Into<String>,
    to: impl Into<String>,
    reason: impl Into<String>,
) {
    let payload = CaptureProfilePayload {
        from: from.into(),
        to: to.into(),
        reason: reason.into(),
    };
    emit_payload(app, CAPTURE_PROFILE_CHANGED, payload);
}

pub fn emit_capture_profile_recovered(
    app: &Arc<dyn EngineHost>,
    from: impl Into<String>,
    to: impl Into<String>,
    reason: impl Into<String>,
) {
    let payload = CaptureProfilePayload {
        from: from.into(),
        to: to.into(),
        reason: reason.into(),
    };
    emit_payload(app, CAPTURE_PROFILE_RECOVERED, payload);
}

pub fn emit_capture_paused(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("capture_paused".to_string()),
        action: Some("Click Resume Capture to continue.".to_string()),
    };
    emit_payload(app, CAPTURE_PAUSED, payload);
}

pub fn emit_capture_resumed(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("capture_resumed".to_string()),
        action: None,
    };
    emit_payload(app, CAPTURE_RESUMED, payload);
}

pub fn emit_audio_path_failed(app: &Arc<dyn EngineHost>, message: impl Into<String>, action: Option<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("audio_required_unavailable".to_string()),
        action,
    };
    emit_payload(app, AUDIO_PATH_FAILED, payload);
}

pub fn emit_audio_path_ready(app: &Arc<dyn EngineHost>, mode: impl Into<String>) {
    let payload = AudioPathReadyPayload { mode: mode.into() };
    emit_payload(app, AUDIO_PATH_READY, payload);
}

pub fn emit_mic_path_degraded(
    app: &Arc<dyn EngineHost>,
    code: impl Into<String>,
    message: impl Into<String>,
    action: Option<String>,
) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some(code.into()),
        action,
    };
    emit_payload(app, MIC_PATH_DEGRADED, payload);
}

pub fn emit_mic_path_recovered(app: &Arc<dyn EngineHost>, message: impl Into<String>) {
    let payload = ErrorPayload {
        message: message.into(),
        code: Some("mic_recovered".to_string()),
        action: None,
    };
    emit_payload(app, MIC_PATH_RECOVERED, payload);
}

pub fn emit_mic_permission_changed(
    app: &Arc<dyn EngineHost>,
    status: impl Into<String>,
    message: Option<String>,
) {
    let payload = MicPermissionPayload {
        status: status.into(),
        message,
    };
    emit_payload(app, MIC_PERMISSION_CHANGED, payload);
}

pub fn emit_perf_guard_transition(app: &Arc<dyn EngineHost>, payload: PerfGuardTransitionPayload) {
    emit_payload(app, PERF_GUARD_TRANSITION, payload);
}
