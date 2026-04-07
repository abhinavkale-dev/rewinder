import { invoke } from "@tauri-apps/api/core";
import type {
  ClipMetadataDto,
  EngineStateDto,
  GrantMicrophoneAccessResultDto,
  GrantOutputDirAccessResultDto,
  GrantScreenRecordingAccessResultDto,
  MicrophoneDeviceDto,
  PermissionStateDto,
  SaveReplayResultDto,
  SettingsDto,
  SettingsPatchDto,
  TriggerSourceDto,
} from "./types";

export function getEngineState(): Promise<EngineStateDto> {
  return invoke<EngineStateDto>("get_engine_state");
}

export function getSettings(): Promise<SettingsDto> {
  return invoke<SettingsDto>("get_settings");
}

export function updateSettings(patch: SettingsPatchDto): Promise<SettingsDto> {
  return invoke<SettingsDto>("update_settings", { patch });
}

export function setReplayEnabled(enabled: boolean): Promise<EngineStateDto> {
  return invoke<EngineStateDto>("set_replay_enabled", { enabled });
}

export function resumeCapture(): Promise<EngineStateDto> {
  return invoke<EngineStateDto>("resume_capture");
}

export function triggerSaveReplay(source: TriggerSourceDto = "manual"): Promise<SaveReplayResultDto> {
  return invoke<SaveReplayResultDto>("trigger_save_replay", { source });
}

export function listRecentClips(limit?: number): Promise<ClipMetadataDto[]> {
  return invoke<ClipMetadataDto[]>("list_recent_clips", { limit });
}

export function recheckPermissions(): Promise<PermissionStateDto> {
  return invoke<PermissionStateDto>("recheck_permissions");
}

export function requestMicrophonePermission(): Promise<PermissionStateDto> {
  return invoke<PermissionStateDto>("request_microphone_permission");
}

export function listMicrophones(): Promise<MicrophoneDeviceDto[]> {
  return invoke<MicrophoneDeviceDto[]>("list_microphones");
}

export function grantOutputDirAccess(): Promise<GrantOutputDirAccessResultDto> {
  return invoke<GrantOutputDirAccessResultDto>("grant_output_dir_access");
}

export function grantScreenRecordingAccess(): Promise<GrantScreenRecordingAccessResultDto> {
  return invoke<GrantScreenRecordingAccessResultDto>("grant_screen_recording_access");
}

export function grantMicrophoneAccess(
  openSettingsIfDenied = true,
): Promise<GrantMicrophoneAccessResultDto> {
  return invoke<GrantMicrophoneAccessResultDto>("grant_microphone_access", {
    openSettingsIfDenied,
  });
}
