import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ClipMetadataDto,
  EngineStateDto,
  ErrorPayload,
  HotkeyPayload,
  CaptureProfilePayload,
  PerfGuardTransitionPayload,
  SettingsUpdatedPayload,
} from "./types";

export const EVENT_ENGINE_STATE_CHANGED = "rewinder://engine-state-changed";
export const EVENT_CLIP_SAVED = "rewinder://clip-saved";
export const EVENT_SAVE_FAILED = "rewinder://save-failed";
export const EVENT_SAVE_DEFERRED = "rewinder://save-deferred";
export const EVENT_SAVE_WARNING = "rewinder://save-warning";
export const EVENT_PERMISSION_REQUIRED = "rewinder://permission-required";
export const EVENT_HOTKEY_TRIGGERED = "rewinder://hotkey-triggered";
export const EVENT_SETTINGS_UPDATED = "rewinder://settings-updated";
export const EVENT_CAPTURE_HEALTH_CHANGED = "rewinder://capture-health-changed";
export const EVENT_HOTKEY_CONFLICT = "rewinder://hotkey-conflict";
export const EVENT_CAPTURE_RESTARTED = "rewinder://capture-restarted";
export const EVENT_AUDIO_MODE_CHANGED = "rewinder://audio-mode-changed";
export const EVENT_CAPTURE_DEGRADED = "rewinder://capture-degraded";
export const EVENT_CAPTURE_PROFILE_CHANGED = "rewinder://capture-profile-changed";
export const EVENT_CAPTURE_PROFILE_RECOVERED = "rewinder://capture-profile-recovered";
export const EVENT_CAPTURE_PAUSED = "rewinder://capture-paused";
export const EVENT_CAPTURE_RESUMED = "rewinder://capture-resumed";
export const EVENT_AUDIO_PATH_FAILED = "rewinder://audio-path-failed";
export const EVENT_AUDIO_PATH_READY = "rewinder://audio-path-ready";
export const EVENT_MIC_PATH_DEGRADED = "rewinder://mic-path-degraded";
export const EVENT_MIC_PATH_RECOVERED = "rewinder://mic-path-recovered";
export const EVENT_MIC_PERMISSION_CHANGED = "rewinder://mic-permission-changed";
export const EVENT_PERF_GUARD_TRANSITION = "rewinder://perf-guard-transition";

export function onEngineStateChanged(handler: (state: EngineStateDto) => void): Promise<UnlistenFn> {
  return listen<EngineStateDto>(EVENT_ENGINE_STATE_CHANGED, (event) => handler(event.payload));
}

export function onClipSaved(handler: (clip: ClipMetadataDto) => void): Promise<UnlistenFn> {
  return listen<ClipMetadataDto>(EVENT_CLIP_SAVED, (event) => handler(event.payload));
}

export function onSaveFailed(handler: (error: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_SAVE_FAILED, (event) => handler(event.payload));
}

export function onSaveDeferred(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_SAVE_DEFERRED, (event) => handler(event.payload));
}

export function onSaveWarning(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_SAVE_WARNING, (event) => handler(event.payload));
}

export function onPermissionRequired(handler: (error: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_PERMISSION_REQUIRED, (event) => handler(event.payload));
}

export function onHotkeyTriggered(handler: (hotkey: HotkeyPayload) => void): Promise<UnlistenFn> {
  return listen<HotkeyPayload>(EVENT_HOTKEY_TRIGGERED, (event) => handler(event.payload));
}

export function onSettingsUpdated(
  handler: (payload: SettingsUpdatedPayload) => void,
): Promise<UnlistenFn> {
  return listen<SettingsUpdatedPayload>(EVENT_SETTINGS_UPDATED, (event) => handler(event.payload));
}

export function onCaptureHealthChanged(handler: (payload: { health: string; reason?: string }) => void): Promise<UnlistenFn> {
  return listen<{ health: string; reason?: string }>(EVENT_CAPTURE_HEALTH_CHANGED, (event) =>
    handler(event.payload),
  );
}

export function onHotkeyConflict(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_HOTKEY_CONFLICT, (event) => handler(event.payload));
}

export function onCaptureRestarted(handler: (payload: { reason: string }) => void): Promise<UnlistenFn> {
  return listen<{ reason: string }>(EVENT_CAPTURE_RESTARTED, (event) => handler(event.payload));
}

export function onAudioModeChanged(
  handler: (payload: { mode: string; reason?: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ mode: string; reason?: string }>(EVENT_AUDIO_MODE_CHANGED, (event) =>
    handler(event.payload),
  );
}

export function onCaptureDegraded(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_CAPTURE_DEGRADED, (event) => handler(event.payload));
}

export function onCaptureProfileChanged(
  handler: (payload: CaptureProfilePayload) => void,
): Promise<UnlistenFn> {
  return listen<CaptureProfilePayload>(EVENT_CAPTURE_PROFILE_CHANGED, (event) =>
    handler(event.payload),
  );
}

export function onCaptureProfileRecovered(
  handler: (payload: CaptureProfilePayload) => void,
): Promise<UnlistenFn> {
  return listen<CaptureProfilePayload>(EVENT_CAPTURE_PROFILE_RECOVERED, (event) =>
    handler(event.payload),
  );
}

export function onCapturePaused(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_CAPTURE_PAUSED, (event) => handler(event.payload));
}

export function onCaptureResumed(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_CAPTURE_RESUMED, (event) => handler(event.payload));
}

export function onAudioPathFailed(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_AUDIO_PATH_FAILED, (event) => handler(event.payload));
}

export function onAudioPathReady(
  handler: (payload: { mode: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ mode: string }>(EVENT_AUDIO_PATH_READY, (event) => handler(event.payload));
}

export function onMicPathDegraded(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_MIC_PATH_DEGRADED, (event) => handler(event.payload));
}

export function onMicPathRecovered(handler: (payload: ErrorPayload) => void): Promise<UnlistenFn> {
  return listen<ErrorPayload>(EVENT_MIC_PATH_RECOVERED, (event) => handler(event.payload));
}

export function onMicPermissionChanged(
  handler: (payload: { status: string; message?: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ status: string; message?: string }>(EVENT_MIC_PERMISSION_CHANGED, (event) =>
    handler(event.payload),
  );
}

export function onPerfGuardTransition(
  handler: (payload: PerfGuardTransitionPayload) => void,
): Promise<UnlistenFn> {
  return listen<PerfGuardTransitionPayload>(EVENT_PERF_GUARD_TRANSITION, (event) =>
    handler(event.payload),
  );
}
