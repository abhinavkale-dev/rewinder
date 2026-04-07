export type LifecycleState =
  | "booting"
  | "permission_required"
  | "armed"
  | "saving_replay"
  | "disabled";

export interface SettingsDto {
  replayDurationSecs: number;
  bufferDurationSecs: number;
  fps: number;
  videoResolution: number;
  videoBitrateKbps: number;
  audioBitrateKbps: number;
  outputDir: string;
  hotkey: string;
  fallbackHotkeys: string[];
  replayEnabled: boolean;
  audioMode: "system_only" | "system_plus_mic" | "video_only";
  micEnabled: boolean;
  audioSampleRateHz: number;
  audioChannels: number;
  segmentTimeMs: number;
  warmupDeferTtlMs: number;
  qualityPolicy: "adaptive_recover" | "strict";
  qualityPreference: "prefer_quality" | "prefer_smoothness";
  audioFallbackPolicy: "system_only_fallback" | "allow_video_only";
  micCaptureBackend: "auto" | "avcapture" | "sck_native";
  selectedMicrophoneId: string | null;
  micFailurePolicy: "best_effort" | "required";
  micStartupTimeoutMs: number;
  micRetryIntervalSecs: number;
  micMixGainDb: number;
  micAutoRequestPermission: boolean;
  audioStartupTimeoutMs: number;
  profileRecoverHoldSecs: number;
  excludeCurrentProcessAudio: boolean;
  savePathMode: "instant_mp4" | "smooth" | "adaptive" | "fast";
  audioSaveMode: "smooth" | "fast" | "adaptive";
  performanceGuardEnabled: boolean;
  performanceGuardLevel: "balanced" | "quality_first" | "performance_first";
}

export interface SettingsPatchDto {
  replayDurationSecs?: number;
  bufferDurationSecs?: number;
  fps?: number;
  videoResolution?: number;
  videoBitrateKbps?: number;
  audioBitrateKbps?: number;
  outputDir?: string;
  hotkey?: string;
  fallbackHotkeys?: string[];
  replayEnabled?: boolean;
  audioMode?: "system_only" | "system_plus_mic" | "video_only";
  micEnabled?: boolean;
  audioSampleRateHz?: number;
  audioChannels?: number;
  segmentTimeMs?: number;
  warmupDeferTtlMs?: number;
  qualityPolicy?: "adaptive_recover" | "strict";
  qualityPreference?: "prefer_quality" | "prefer_smoothness";
  audioFallbackPolicy?: "system_only_fallback" | "allow_video_only";
  micCaptureBackend?: "auto" | "avcapture" | "sck_native";
  selectedMicrophoneId?: string | null;
  micFailurePolicy?: "best_effort" | "required";
  micStartupTimeoutMs?: number;
  micRetryIntervalSecs?: number;
  micMixGainDb?: number;
  micAutoRequestPermission?: boolean;
  audioStartupTimeoutMs?: number;
  profileRecoverHoldSecs?: number;
  excludeCurrentProcessAudio?: boolean;
  savePathMode?: "instant_mp4" | "smooth" | "adaptive" | "fast";
  audioSaveMode?: "smooth" | "fast" | "adaptive";
  performanceGuardEnabled?: boolean;
  performanceGuardLevel?: "balanced" | "quality_first" | "performance_first";
}

export type CaptureHealth =
  | "starting"
  | "running"
  | "restarting"
  | "degraded"
  | "stopped";

export type AudioHealth = "ok" | "degraded" | "unavailable";
export type HotkeyStatus = "ok" | "conflict" | "fallback" | "invalid";
export type SaveStage = "idle" | "queued" | "saving_fast" | "ready";
export type VideoSmoothState =
  | "idle"
  | "pending"
  | "processing"
  | "complete"
  | "failed";

export interface PermissionStateDto {
  screenRecordingGranted: boolean;
  systemAudioGranted: boolean;
  outputDirWritable: boolean;
  outputDirPermissionError: string | null;
  reason: string | null;
}

export interface GrantOutputDirAccessResultDto {
  permission: PermissionStateDto;
  openedSettings: boolean;
  message: string;
}

export interface GrantScreenRecordingAccessResultDto {
  permission: PermissionStateDto;
  openedSettings: boolean;
  message: string;
}

export interface GrantMicrophoneAccessResultDto {
  permission: PermissionStateDto;
  micPermissionStatus: string;
  micPermissionError: string | null;
  openedSettings: boolean;
  message: string;
}

export interface EngineStateDto {
  lifecycleState: LifecycleState;
  captureHealth: CaptureHealth;
  audioHealth: AudioHealth;
  saveStage: SaveStage;
  systemAudioPathReady: boolean;
  systemAudioReady: boolean;
  micPathReady: boolean;
  micReady: boolean;
  micFramesSeen: boolean;
  micLevelDbfs: number | null;
  audioPathReady: boolean;
  firstAudioFrameSeen: boolean;
  micPermissionStatus: string;
  micPermissionError: string | null;
  micCaptureSessionRunning: boolean;
  micSamplesPerSec: number | null;
  micAttachState: "inactive" | "silence_filler" | "live" | "degraded";
  micRecoveryState: "ok" | "retrying" | "fallback_system_only" | "blocked_required";
  selectedMicrophoneId: string | null;
  selectedMicrophoneName: string | null;
  lastMicErrorCode: string | null;
  lastMicErrorMessage: string | null;
  captureSpeedX: number | null;
  encoderThroughputX: number | null;
  playbackRealtimeX: number | null;
  playbackStability: "stable" | "drifting" | "recovering";
  captureLoadState: "normal" | "stressed" | "recovering";
  operatorHealthState: string;
  operatorHealthMessage: string;
  guardState: string;
  guardPrimaryReasonCode?: string | null;
  guardContributingReasonCodes: string[];
  guardSuppressedReasonCode?: string | null;
  guardLastTransitionAtEpochMs?: number | null;
  liveQueueProfile: "small" | "elevated";
  saveReady: boolean;
  hotkeyStatus: HotkeyStatus;
  activeAudioMode: string;
  effectiveAudioMode: string;
  captureBackend: string;
  micBackendInUse: string;
  micMixGainDb: number;
  requestedVideoResolution: number;
  requestedFps: number;
  requestedVideoBitrateKbps: number;
  effectiveVideoResolution: number;
  effectiveFps: number;
  effectiveVideoBitrateKbps: number;
  audioFallbackPolicy: string;
  degradeReason: string | null;
  audioDegradeReason: string | null;
  lastAudioModeError: string | null;
  captureRestartCount: number;
  captureInterruptCount: number;
  videoSmoothState: VideoSmoothState;
  captureDroppedFrames: number;
  captureQueueOverflows: number;
  effectiveOutputFps: number | null;
  concurrentSessionCount: number | null;
  captureOwnerPid: number | null;
  appRssMb: number | null;
  appCpuPercent: number | null;
  captureStackRssMb: number | null;
  captureStackCpuPercent: number | null;
  captureStackRssDeltaMb: number | null;
  systemMemoryPressureLevel?: string | null;
  thermalState: string | null;
  captureCrashLoop: boolean;
  isArmed: boolean;
  isSaving: boolean;
  armBlocker: string | null;
  armBlockerCode?: string | null;
  armBlockerAction?: string | null;
  pendingSave: boolean;
  pendingFullWindow: boolean;
  pendingFullWindowDeadlineEpochMs: number | null;
  fullWindowWaitRemainingMs: number | null;
  warmupEtaMs: number | null;
  audioWarmupGraceMs?: number | null;
  bufferFillSecs: number;
  replayFillSecs: number;
  replayTargetSecs: number;
  rollingFillSecs: number;
  rollingTargetSecs: number;
  lastError: string | null;
  lastCaptureLogTail: string | null;
  captureStartPhase: string | null;
  droppedVideoPackets: number;
  droppedAudioPackets: number;
  lastContiguityBreakCode: string | null;
  permission: PermissionStateDto;
  settings: SettingsDto;
}

export interface MicrophoneDeviceDto {
  id: string;
  name: string;
  isDefault: boolean;
  isAvailable: boolean;
}

export interface ClipMetadataDto {
  id: string;
  path: string;
  createdAtEpochMs: number;
  durationSecs: number;
  sizeBytes: number;
}

export type TriggerSourceDto = "manual" | "hotkey";

export interface SaveReplayResultDto {
  ok: boolean;
  queued: boolean;
  clip: ClipMetadataDto | null;
  error: string | null;
  message: string | null;
  actualDurationSecs: number | null;
  audioRepaired: boolean;
  saveAudioStrategy: "instant_mp4" | "smooth" | "fast" | "fallback_fast" | null;
  smoothPending: boolean;
  smoothApplied: boolean;
  smoothError: string | null;
  effectiveVideoResolution: number | null;
  effectiveFps: number | null;
  requestedDurationSecs: number | null;
  selectedDurationSecs: number | null;
  contiguousDurationSecs: number | null;
  partialReasonCode: string | null;
  anchorEpochMs: number | null;
}

export interface ErrorPayload {
  message: string;
  code?: string;
  action?: string;
}

export interface HotkeyPayload {
  hotkey: string;
}

export interface SettingsUpdatedPayload {
  message: string;
}

export interface CaptureProfilePayload {
  from: string;
  to: string;
  reason: string;
}

export interface PerfGuardTransitionPayload {
  action: string;
  guardState: string;
  hard: boolean;
  primaryReasonCode?: string | null;
  contributingReasonCodes: string[];
  suppressedReasonCode?: string | null;
  fromProfile?: string | null;
  toProfile?: string | null;
  sampledAtEpochMs: number;
}
