use tauri::{AppHandle, State};

use crate::capture::permissions;
use crate::core::state::{
    ClipMetadataDto, EngineStateDto, GrantMicrophoneAccessResultDto, GrantOutputDirAccessResultDto,
    GrantScreenRecordingAccessResultDto, MicrophoneDeviceDto, PermissionStateDto,
    SaveReplayResultDto, TriggerSourceDto,
};
use crate::settings::{SettingsDto, SettingsPatchDto};
use crate::AppState;

#[tauri::command]
pub fn get_engine_state(state: State<'_, AppState>) -> EngineStateDto {
    state.engine.get_engine_state()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsDto {
    state.engine.get_settings()
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatchDto,
) -> Result<SettingsDto, String> {
    state.engine.update_settings(&app, patch)
}

#[tauri::command]
pub fn set_replay_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<EngineStateDto, String> {
    state.engine.set_replay_enabled(&app, enabled)
}

#[tauri::command]
pub fn resume_capture(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EngineStateDto, String> {
    state.engine.resume_capture(&app)
}

#[tauri::command]
pub fn trigger_save_replay(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Option<TriggerSourceDto>,
) -> SaveReplayResultDto {
    state
        .engine
        .trigger_save_replay(&app, source.unwrap_or(TriggerSourceDto::Manual))
}

#[tauri::command]
pub fn list_recent_clips(state: State<'_, AppState>, limit: Option<usize>) -> Vec<ClipMetadataDto> {
    state.engine.list_recent_clips(limit)
}

#[tauri::command]
pub fn recheck_permissions(app: AppHandle, state: State<'_, AppState>) -> PermissionStateDto {
    state.engine.recheck_permissions(&app)
}

#[tauri::command]
pub fn request_microphone_permission(
    app: AppHandle,
    state: State<'_, AppState>,
) -> PermissionStateDto {
    state.engine.request_microphone_permission(&app)
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<MicrophoneDeviceDto>, String> {
    permissions::list_microphones()
}

#[tauri::command]
pub fn grant_output_dir_access(
    app: AppHandle,
    state: State<'_, AppState>,
) -> GrantOutputDirAccessResultDto {
    state.engine.grant_output_dir_access(&app)
}

#[tauri::command]
pub fn grant_screen_recording_access(
    app: AppHandle,
    state: State<'_, AppState>,
) -> GrantScreenRecordingAccessResultDto {
    state.engine.grant_screen_recording_access(&app)
}

#[tauri::command]
pub fn grant_microphone_access(
    app: AppHandle,
    state: State<'_, AppState>,
    open_settings_if_denied: Option<bool>,
) -> GrantMicrophoneAccessResultDto {
    state
        .engine
        .grant_microphone_access(&app, open_settings_if_denied.unwrap_or(true))
}
