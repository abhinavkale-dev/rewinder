import type { EngineStateDto, SettingsDto } from "../clipper/types";

type EngineStatusPanelProps = {
  engineState: EngineStateDto | null;
  settings: SettingsDto;
  permissionText: string;
  disableSaveButton: boolean;
  onManualSave: () => void;
  onToggleReplay: (enabled: boolean) => void;
  onPermissionRecheck: () => void;
  onRequestMicPermission: () => void;
};

export function EngineStatusPanel({
  engineState,
  settings,
  permissionText,
  disableSaveButton,
  onManualSave,
  onToggleReplay,
  onPermissionRecheck,
  onRequestMicPermission,
}: EngineStatusPanelProps) {
  const replayFill = (engineState?.replayFillSecs ?? 0).toFixed(2);
  const replayTarget = engineState?.replayTargetSecs ?? settings.replayDurationSecs;
  const rollingFill = (engineState?.rollingFillSecs ?? engineState?.bufferFillSecs ?? 0).toFixed(2);
  const rollingTarget = engineState?.rollingTargetSecs ?? settings.bufferDurationSecs;
  const lastError = engineState?.lastError ?? engineState?.armBlocker ?? "None";
  const micAttachState = engineState?.micAttachState ?? "inactive";
  const micStatusLabel =
    micAttachState === "live"
      ? "Recording"
      : micAttachState === "silence_filler"
        ? "Silence (no signal)"
        : micAttachState === "degraded"
          ? "Disconnected"
          : "Off";
  const micDbfs = engineState?.micLevelDbfs ?? null;
  const micVuPercent =
    micDbfs != null ? Math.max(0, Math.min(100, ((micDbfs + 60) / 60) * 100)) : 0;
  const micVuColor =
    micDbfs == null || micDbfs < -50
      ? "#888"
      : micDbfs > -1
        ? "#e74c3c"
        : micDbfs > -6
          ? "#f1c40f"
          : "#2ecc71";

  return (
    <section className="panel status-grid">
      <div>
        <h2>Engine</h2>
        <p>Lifecycle: {engineState?.lifecycleState ?? "unknown"}</p>
        <p>Operator health: {engineState?.operatorHealthState ?? "unknown"}</p>
        <p>Capture health: {engineState?.captureHealth ?? "unknown"}</p>
        <p>Save ready: {engineState?.saveReady ? "Yes" : "No"}</p>
        <p>Save stage: {engineState?.saveStage ?? "idle"}</p>
        <p>
          Replay fill: {replayFill}s / {replayTarget}s
        </p>
        <p>
          Rolling fill: {rollingFill}s / {rollingTarget}s
        </p>
        <p>
          Effective profile: {engineState?.effectiveVideoResolution ?? settings.videoResolution}p @{" "}
          {engineState?.effectiveFps ?? settings.fps}fps (
          {engineState?.effectiveVideoBitrateKbps ?? settings.videoBitrateKbps} kbps)
        </p>
        <p>Audio health: {engineState?.audioHealth ?? "unknown"}</p>
        <p>System audio ready: {engineState?.systemAudioPathReady ? "Yes" : "No"}</p>
        <p>
          Mic: {micStatusLabel}
          {micDbfs != null && <> | Level: {micDbfs.toFixed(1)} dBFS</>}
        </p>
        {engineState?.settings.audioMode === "system_plus_mic" && (
          <p>
            Mic recovery: {engineState?.micRecoveryState ?? "ok"}
            {engineState?.selectedMicrophoneName
              ? ` (${engineState.selectedMicrophoneName})`
              : engineState?.selectedMicrophoneId
                ? ` (${engineState.selectedMicrophoneId})`
                : ""}
          </p>
        )}
        {micDbfs != null && (
          <div
            style={{
              height: 6,
              background: "#333",
              borderRadius: 3,
              margin: "2px 0 4px",
              overflow: "hidden",
            }}
          >
            <div
              style={{
                width: `${micVuPercent}%`,
                height: "100%",
                background: micVuColor,
                borderRadius: 3,
                transition: "width 0.15s ease-out",
              }}
            />
          </div>
        )}
        {engineState?.micPermissionStatus === "denied" && (
          <p style={{ color: "#e74c3c" }}>
            Mic permission denied. Replays will only include system audio.{" "}
            <button type="button" onClick={onRequestMicPermission}>
              Grant Access
            </button>
          </p>
        )}
        <p>Last error: {lastError}</p>

        <details>
          <summary>Advanced diagnostics</summary>
          <p>Armed: {engineState?.isArmed ? "Yes" : "No"}</p>
          <p>Saving: {engineState?.isSaving ? "Yes" : "No"}</p>
          <p>Pending save: {engineState?.pendingSave ? "Yes" : "No"}</p>
          <p>Pending full window: {engineState?.pendingFullWindow ? "Yes" : "No"}</p>
          <p>
            Full-window wait remaining: {" "}
            {engineState?.fullWindowWaitRemainingMs != null
              ? `${(engineState.fullWindowWaitRemainingMs / 1000).toFixed(1)}s`
              : "n/a"}
          </p>
          <p>
            Full-window deadline: {" "}
            {engineState?.pendingFullWindowDeadlineEpochMs != null
              ? String(engineState.pendingFullWindowDeadlineEpochMs)
              : "n/a"}
          </p>
          <p>Blocker: {engineState?.armBlocker ?? "None"}</p>
          <p>Blocker code: {engineState?.armBlockerCode ?? "None"}</p>
          <p>Blocker action: {engineState?.armBlockerAction ?? "None"}</p>
          <p>Capture start phase: {engineState?.captureStartPhase ?? "n/a"}</p>
          <p>
            Playback realtime (pipeline health): {engineState?.playbackRealtimeX?.toFixed(2) ?? "n/a"}x
          </p>
          <p>Playback stability: {engineState?.playbackStability ?? "recovering"}</p>
          <p>
            Encoder throughput: {" "}
            {engineState?.encoderThroughputX?.toFixed(2) ??
              engineState?.captureSpeedX?.toFixed(2) ??
              "n/a"}
            x
          </p>
          <p>Capture load: {engineState?.captureLoadState ?? "normal"}</p>
          <p>Guard state: {engineState?.guardState ?? "idle"}</p>
          <p>Guard primary reason: {engineState?.guardPrimaryReasonCode ?? "None"}</p>
          <p>
            Guard contributing reasons: {engineState?.guardContributingReasonCodes?.length
              ? engineState.guardContributingReasonCodes.join(", ")
              : "None"}
          </p>
          <p>Guard suppressed reason: {engineState?.guardSuppressedReasonCode ?? "None"}</p>
          <p>Live queue profile: {engineState?.liveQueueProfile ?? "small"}</p>
          <p>Video smooth state: {engineState?.videoSmoothState ?? "idle"}</p>
          <p>Effective output fps: {engineState?.effectiveOutputFps?.toFixed(2) ?? "n/a"}</p>
          <p>Capture dropped frames: {engineState?.captureDroppedFrames ?? 0}</p>
          <p>Capture queue overflows: {engineState?.captureQueueOverflows ?? 0}</p>
          <p>Concurrent sessions detected: {engineState?.concurrentSessionCount ?? "n/a"}</p>
          <p>Capture owner PID: {engineState?.captureOwnerPid ?? "n/a"}</p>
          <p>
            App RSS (rewinder only): {engineState?.appRssMb != null ? `${engineState.appRssMb} MB` : "n/a"}
          </p>
          <p>
            App CPU (rewinder only): {" "}
            {engineState?.appCpuPercent != null ? `${engineState.appCpuPercent.toFixed(1)}%` : "n/a"}
          </p>
          <p>
            Capture stack RSS (rewinder + helper + ffmpeg): {" "}
            {engineState?.captureStackRssMb != null ? `${engineState.captureStackRssMb} MB` : "n/a"}
          </p>
          <p>
            Capture stack CPU (rewinder + helper + ffmpeg): {" "}
            {engineState?.captureStackCpuPercent != null
              ? `${engineState.captureStackCpuPercent.toFixed(1)}%`
              : "n/a"}
          </p>
          <p>
            Capture stack RSS delta: {" "}
            {engineState?.captureStackRssDeltaMb != null
              ? `${engineState.captureStackRssDeltaMb} MB`
              : "n/a"}
          </p>
          <p>System memory pressure: {engineState?.systemMemoryPressureLevel ?? "n/a"}</p>
          <p>Thermal state: {engineState?.thermalState ?? "n/a"}</p>
          <p>
            Requested profile: {engineState?.requestedVideoResolution ?? settings.videoResolution}p @{" "}
            {engineState?.requestedFps ?? settings.fps}fps (
            {engineState?.requestedVideoBitrateKbps ?? settings.videoBitrateKbps} kbps)
          </p>
          <p>Degrade reason: {engineState?.degradeReason ?? "None"}</p>
          <p>Mic path ready: {engineState?.micPathReady ? "Yes" : "No"}</p>
          <p>Mic frames seen: {engineState?.micFramesSeen ? "Yes" : "No"}</p>
          <p>
            Mic level: {" "}
            {engineState?.micLevelDbfs != null ? `${engineState.micLevelDbfs.toFixed(1)} dBFS` : "n/a"}
          </p>
          <p>Mic attach state: {engineState?.micAttachState ?? "inactive"}</p>
          <p>Audio path ready: {engineState?.audioPathReady ? "Yes" : "No"}</p>
          <p>First audio frame seen: {engineState?.firstAudioFrameSeen ? "Yes" : "No"}</p>
          <p>
            Audio warmup grace left: {" "}
            {engineState?.audioWarmupGraceMs != null
              ? `${(engineState.audioWarmupGraceMs / 1000).toFixed(1)}s`
              : "n/a"}
          </p>
          <p>Active audio mode: {engineState?.activeAudioMode ?? "unknown"}</p>
          <p>Effective audio mode: {engineState?.effectiveAudioMode ?? "unknown"}</p>
          <p>Selected microphone ID: {engineState?.selectedMicrophoneId ?? "system_default"}</p>
          <p>Selected microphone name: {engineState?.selectedMicrophoneName ?? "n/a"}</p>
          <p>Mic backend in use: {engineState?.micBackendInUse ?? "unknown"}</p>
          <p>Mic recovery state: {engineState?.micRecoveryState ?? "ok"}</p>
          <p>Last mic error code: {engineState?.lastMicErrorCode ?? "None"}</p>
          <p>Last mic error: {engineState?.lastMicErrorMessage ?? "None"}</p>
          <p>Mic permission status: {engineState?.micPermissionStatus ?? "unknown"}</p>
          <p>Mic permission error: {engineState?.micPermissionError ?? "None"}</p>
          <p>Mic capture session running: {engineState?.micCaptureSessionRunning ? "Yes" : "No"}</p>
          <p>
            Mic samples/sec: {" "}
            {engineState?.micSamplesPerSec != null ? engineState.micSamplesPerSec : "n/a"}
          </p>
          <p>Mic mix gain: {(engineState?.micMixGainDb ?? settings.micMixGainDb).toFixed(1)} dB</p>
          <p>Audio fallback policy: {engineState?.audioFallbackPolicy ?? settings.audioFallbackPolicy}</p>
          <p>Quality preference: {settings.qualityPreference}</p>
          <p>Mic failure policy: {settings.micFailurePolicy}</p>
          <p>Save path mode: {settings.savePathMode}</p>
          <p>Audio save mode: {settings.audioSaveMode}</p>
          <p>Audio degrade reason: {engineState?.audioDegradeReason ?? "None"}</p>
          <p>Last contiguity break: {engineState?.lastContiguityBreakCode ?? "None"}</p>
          <p>Last audio mode error: {engineState?.lastAudioModeError ?? "None"}</p>
          <p>Hotkey status: {engineState?.hotkeyStatus ?? "unknown"}</p>
          <p>Permission: {permissionText}</p>
          <p>
            Rolling buffer: {settings.outputDir}/.rewinder-live. Final clips appear in{" "}
            {settings.outputDir} after Save/Hotkey.
          </p>
        </details>
      </div>
      <div className="actions">
        <button type="button" onClick={onManualSave} disabled={disableSaveButton}>
          Save Replay Now
        </button>
        <button
          type="button"
          className="secondary"
          onClick={() => onToggleReplay(!(settings.replayEnabled ?? true))}
        >
          {settings.replayEnabled ? "Disable Replay" : "Enable Replay"}
        </button>
        <button type="button" className="secondary" onClick={onPermissionRecheck}>
          Recheck Permissions
        </button>
        <button type="button" className="secondary" onClick={onRequestMicPermission}>
          Request Mic Access
        </button>
      </div>
    </section>
  );
}
