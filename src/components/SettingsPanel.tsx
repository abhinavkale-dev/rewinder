import type { FormEvent } from "react";
import type { MicrophoneDeviceDto } from "../clipper/types";

type SettingsPanelProps = {
  isSubmittingSettings: boolean;
  formReplayDuration: number;
  formBufferDuration: number;
  formFps: number;
  formVideoResolution: number;
  formAudioBitrate: number;
  formAudioMode: "system_only" | "system_plus_mic" | "video_only";
  formMicEnabled: boolean;
  formQualityPolicy: "adaptive_recover" | "strict";
  formQualityPreference: "prefer_quality" | "prefer_smoothness";
  formAudioFallbackPolicy: "system_only_fallback" | "allow_video_only";
  formMicCaptureBackend: "auto" | "avcapture" | "sck_native";
  formSelectedMicrophoneId: string;
  microphoneDevices: MicrophoneDeviceDto[];
  formExcludeCurrentProcessAudio: boolean;
  formSavePathMode: "instant_mp4" | "smooth" | "adaptive" | "fast";
  formAudioSaveMode: "smooth" | "fast" | "adaptive";
  formOutputDir: string;
  formHotkey: string;
  onSubmit: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  setFormReplayDuration: (value: number) => void;
  setFormBufferDuration: (value: number) => void;
  setFormFps: (value: number) => void;
  setFormVideoResolution: (value: number) => void;
  setFormAudioBitrate: (value: number) => void;
  setFormAudioMode: (value: "system_only" | "system_plus_mic" | "video_only") => void;
  setFormMicEnabled: (value: boolean) => void;
  setFormQualityPolicy: (value: "adaptive_recover" | "strict") => void;
  setFormQualityPreference: (value: "prefer_quality" | "prefer_smoothness") => void;
  setFormAudioFallbackPolicy: (value: "system_only_fallback" | "allow_video_only") => void;
  setFormMicCaptureBackend: (value: "auto" | "avcapture" | "sck_native") => void;
  setFormSelectedMicrophoneId: (value: string) => void;
  setFormExcludeCurrentProcessAudio: (value: boolean) => void;
  setFormSavePathMode: (value: "instant_mp4" | "smooth" | "adaptive" | "fast") => void;
  setFormAudioSaveMode: (value: "smooth" | "fast" | "adaptive") => void;
  setFormOutputDir: (value: string) => void;
  setFormHotkey: (value: string) => void;
};

export function SettingsPanel({
  isSubmittingSettings,
  formReplayDuration,
  formBufferDuration,
  formFps,
  formVideoResolution,
  formAudioBitrate,
  formAudioMode,
  formMicEnabled,
  formQualityPolicy,
  formQualityPreference,
  formAudioFallbackPolicy,
  formMicCaptureBackend,
  formSelectedMicrophoneId,
  microphoneDevices,
  formExcludeCurrentProcessAudio,
  formSavePathMode,
  formAudioSaveMode,
  formOutputDir,
  formHotkey,
  onSubmit,
  setFormReplayDuration,
  setFormBufferDuration,
  setFormFps,
  setFormVideoResolution,
  setFormAudioBitrate,
  setFormAudioMode,
  setFormMicEnabled,
  setFormQualityPolicy,
  setFormQualityPreference,
  setFormAudioFallbackPolicy,
  setFormMicCaptureBackend,
  setFormSelectedMicrophoneId,
  setFormExcludeCurrentProcessAudio,
  setFormSavePathMode,
  setFormAudioSaveMode,
  setFormOutputDir,
  setFormHotkey,
}: SettingsPanelProps) {
  return (
    <section className="panel">
      <h2>Settings</h2>
      <form className="settings-form" onSubmit={(event) => void onSubmit(event)}>
        <label>
          Replay Duration (s)
          <input
            type="number"
            min={1}
            max={300}
            value={formReplayDuration}
            onChange={(event) => setFormReplayDuration(Number(event.currentTarget.value))}
          />
        </label>

        <label>
          Buffer Duration (s)
          <input
            type="number"
            min={1}
            max={600}
            value={formBufferDuration}
            onChange={(event) => setFormBufferDuration(Number(event.currentTarget.value))}
          />
        </label>

        <label>
          FPS
          <input
            type="number"
            min={10}
            max={120}
            value={formFps}
            onChange={(event) => setFormFps(Number(event.currentTarget.value))}
          />
        </label>

        <label>
          Video Resolution
          <select
            value={formVideoResolution}
            onChange={(event) => setFormVideoResolution(Number(event.currentTarget.value))}
          >
            <option value={1080}>1080p</option>
            <option value={720}>720p</option>
            <option value={480}>480p</option>
            <option value={360}>360p</option>
          </select>
        </label>

        <label>
          Audio Bitrate (kbps)
          <input
            type="number"
            min={64}
            value={formAudioBitrate}
            onChange={(event) => setFormAudioBitrate(Number(event.currentTarget.value))}
          />
        </label>

        <label>
          Audio Mode
          <select
            value={formAudioMode}
            onChange={(event) =>
              setFormAudioMode(event.currentTarget.value as "system_only" | "system_plus_mic" | "video_only")
            }
          >
            <option value="system_only">System Only</option>
            <option value="system_plus_mic">System + Mic</option>
            <option value="video_only">Video Only</option>
          </select>
        </label>

        <label>
          Mic Enabled
          <input
            type="checkbox"
            checked={formMicEnabled}
            onChange={(event) => {
              const enabled = event.currentTarget.checked;
              setFormMicEnabled(enabled);
              setFormAudioMode(enabled ? "system_plus_mic" : "system_only");
            }}
          />
        </label>

        <label>
          Microphone Device
          <select
            value={formSelectedMicrophoneId}
            onChange={(event) => setFormSelectedMicrophoneId(event.currentTarget.value)}
            disabled={!formMicEnabled || formAudioMode !== "system_plus_mic"}
          >
            <option value="">System Default Microphone</option>
            {microphoneDevices.map((device) => (
              <option key={device.id} value={device.id} disabled={!device.isAvailable}>
                {device.name}
                {device.isDefault ? " (Default)" : ""}
                {!device.isAvailable ? " (Unavailable)" : ""}
              </option>
            ))}
          </select>
        </label>

        <label>
          Mic Backend
          <select
            value={formMicCaptureBackend}
            onChange={(event) =>
              setFormMicCaptureBackend(
                event.currentTarget.value as "auto" | "avcapture" | "sck_native",
              )
            }
            disabled={!formMicEnabled || formAudioMode !== "system_plus_mic"}
          >
            <option value="auto">Auto (ScreenCaptureKit first)</option>
            <option value="sck_native">ScreenCaptureKit Native</option>
            <option value="avcapture">AVCapture Fallback</option>
          </select>
        </label>

        <label>
          Quality Policy
          <select
            value={formQualityPolicy}
            onChange={(event) =>
              setFormQualityPolicy(event.currentTarget.value as "adaptive_recover" | "strict")
            }
          >
            <option value="adaptive_recover">Adaptive + Recover</option>
            <option value="strict">Strict (No Fallback)</option>
          </select>
        </label>

        <label>
          Quality Preference
          <select
            value={formQualityPreference}
            onChange={(event) =>
              setFormQualityPreference(event.currentTarget.value as "prefer_quality" | "prefer_smoothness")
            }
          >
            <option value="prefer_quality">Prefer quality (1080-first)</option>
            <option value="prefer_smoothness">Prefer smoothness</option>
          </select>
        </label>

        <label>
          Audio Fallback Policy
          <select
            value={formAudioFallbackPolicy}
            onChange={(event) =>
              setFormAudioFallbackPolicy(event.currentTarget.value as "system_only_fallback" | "allow_video_only")
            }
          >
            <option value="system_only_fallback">System-Only Fallback</option>
            <option value="allow_video_only">Allow Video-Only</option>
          </select>
        </label>

        <label>
          Exclude App Audio
          <input
            type="checkbox"
            checked={formExcludeCurrentProcessAudio}
            onChange={(event) => setFormExcludeCurrentProcessAudio(event.currentTarget.checked)}
          />
        </label>

        <label>
          Save Path Mode
          <select
            value={formSavePathMode}
            onChange={(event) => {
              const nextMode = event.currentTarget.value as "instant_mp4" | "smooth" | "adaptive" | "fast";
              setFormSavePathMode(nextMode);
              const legacyMode = nextMode === "smooth" ? "smooth" : nextMode === "adaptive" ? "adaptive" : "fast";
              setFormAudioSaveMode(legacyMode);
            }}
          >
            <option value="instant_mp4">Instant MP4 (recommended)</option>
            <option value="smooth">Smooth (audio repair)</option>
            <option value="adaptive">Adaptive</option>
            <option value="fast">Fast (legacy)</option>
          </select>
        </label>

        <label>
          Audio Save Mode
          <select
            value={formAudioSaveMode}
            onChange={(event) =>
              setFormAudioSaveMode(event.currentTarget.value as "smooth" | "fast" | "adaptive")
            }
          >
            <option value="smooth">Smooth (audio-first)</option>
            <option value="fast">Fast (copy)</option>
            <option value="adaptive">Adaptive</option>
          </select>
        </label>

        <label>
          Output Directory
          <input
            type="text"
            value={formOutputDir}
            onChange={(event) => setFormOutputDir(event.currentTarget.value)}
          />
        </label>

        <label>
          Hotkey
          <input type="text" value={formHotkey} onChange={(event) => setFormHotkey(event.currentTarget.value)} />
        </label>

        <div className="settings-actions">
          <button type="submit" disabled={isSubmittingSettings}>
            {isSubmittingSettings ? "Saving..." : "Save Settings"}
          </button>
        </div>
      </form>
    </section>
  );
}
