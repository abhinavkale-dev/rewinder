import type { EngineStateDto, SettingsDto } from "../clipper/types";

type HeaderPanelProps = {
  status: string;
  showCapturePrivacyNote: boolean;
  showBackgroundRunningNote: boolean;
  profileFallbackActive: boolean;
  engineState: EngineStateDto | null;
  settings: SettingsDto;
  showResumeCaptureCta: boolean;
  showEnableReplayCta: boolean;
  showScreenGrantCta: boolean;
  showDownloadsGrantCta: boolean;
  showMicGrantCta: boolean;
  onResumeCapture: () => void;
  onEnableReplay: () => void;
  onGrantScreenRecordingAccess: () => void;
  onGrantDownloadsAccess: () => void;
  onGrantMicrophoneAccess: () => void;
};

export function HeaderPanel({
  status,
  showCapturePrivacyNote,
  showBackgroundRunningNote,
  profileFallbackActive,
  engineState,
  settings,
  showResumeCaptureCta,
  showEnableReplayCta,
  showScreenGrantCta,
  showDownloadsGrantCta,
  showMicGrantCta,
  onResumeCapture,
  onEnableReplay,
  onGrantScreenRecordingAccess,
  onGrantDownloadsAccess,
  onGrantMicrophoneAccess,
}: HeaderPanelProps) {
  return (
    <section className="panel header-panel">
      <h1>Rewinder</h1>
      <p className="subtitle">Instant replay engine, always armed</p>
      <p className="status">{status}</p>
      {showCapturePrivacyNote && (
        <p className="privacy-note">
          macOS may show a system screen recording indicator while replay is armed. This is expected and
          capture remains local unless you choose to share a saved clip.
        </p>
      )}
      {showBackgroundRunningNote && (
        <p className="privacy-note">
          Closing the window keeps Rewinder running in the background while replay is armed. Use tray
          Quit Rewinder to exit fully.
        </p>
      )}
      {profileFallbackActive && (
        <p className="privacy-note">
          Capture fallback active: {engineState?.effectiveVideoResolution ?? settings.videoResolution}p@
          {engineState?.effectiveFps ?? settings.fps} (target {engineState?.requestedVideoResolution ??
            settings.videoResolution}
          p@{engineState?.requestedFps ?? settings.fps}).
        </p>
      )}
      {showResumeCaptureCta && (
        <div className="status-actions">
          <button type="button" onClick={onResumeCapture}>
            Resume Capture
          </button>
        </div>
      )}
      {!showResumeCaptureCta && showEnableReplayCta && (
        <div className="status-actions">
          <button type="button" onClick={onEnableReplay}>
            Restart Capture
          </button>
        </div>
      )}
      {!showResumeCaptureCta && !showEnableReplayCta && showScreenGrantCta && (
        <div className="status-actions">
          <button type="button" onClick={onGrantScreenRecordingAccess}>
            Enable Screen Recording
          </button>
        </div>
      )}
      {!showResumeCaptureCta && !showEnableReplayCta && showDownloadsGrantCta && (
        <div className="status-actions">
          <button type="button" onClick={onGrantDownloadsAccess}>
            Enable Downloads Permission
          </button>
        </div>
      )}
      {!showResumeCaptureCta && !showEnableReplayCta && showMicGrantCta && (
        <div className="status-actions">
          <button type="button" onClick={onGrantMicrophoneAccess}>
            Enable Microphone Permission
          </button>
        </div>
      )}
    </section>
  );
}
