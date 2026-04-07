import { useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import {
  getEngineState,
  getSettings,
  grantMicrophoneAccess,
  grantOutputDirAccess,
  grantScreenRecordingAccess,
  listMicrophones,
  listRecentClips,
  recheckPermissions,
  resumeCapture,
  requestMicrophonePermission,
  setReplayEnabled,
  triggerSaveReplay,
  updateSettings,
} from "./clipper/api";
import {
  onAudioModeChanged,
  onCaptureDegraded,
  onCaptureHealthChanged,
  onCaptureProfileChanged,
  onCaptureProfileRecovered,
  onCapturePaused,
  onCaptureRestarted,
  onCaptureResumed,
  onClipSaved,
  onEngineStateChanged,
  onAudioPathReady,
  onAudioPathFailed,
  onMicPathDegraded,
  onMicPermissionChanged,
  onMicPathRecovered,
  onHotkeyTriggered,
  onHotkeyConflict,
  onPermissionRequired,
  onSaveFailed,
  onSaveDeferred,
  onSaveWarning,
  onSettingsUpdated,
} from "./clipper/events";
import type {
  ClipMetadataDto,
  EngineStateDto,
  MicrophoneDeviceDto,
  SettingsDto,
} from "./clipper/types";
import { EngineStatusPanel } from "./components/EngineStatusPanel";
import { HeaderPanel } from "./components/HeaderPanel";
import { RecentClipsPanel } from "./components/RecentClipsPanel";
import { SettingsPanel } from "./components/SettingsPanel";

const fallbackSettings: SettingsDto = {
  replayDurationSecs: 30,
  bufferDurationSecs: 120,
  fps: 60,
  videoResolution: 1080,
  videoBitrateKbps: 10000,
  audioBitrateKbps: 160,
  outputDir: "",
  hotkey: "Ctrl+Option+R",
  fallbackHotkeys: ["Ctrl+Option+R", "Ctrl+Option+Shift+R", "Cmd+Option+R"],
  replayEnabled: true,
  audioMode: "system_only",
  micEnabled: false,
  audioSampleRateHz: 48000,
  audioChannels: 2,
  segmentTimeMs: 500,
  warmupDeferTtlMs: 3000,
  qualityPolicy: "adaptive_recover",
  qualityPreference: "prefer_quality",
  audioFallbackPolicy: "system_only_fallback",
  micCaptureBackend: "auto",
  selectedMicrophoneId: null,
  micFailurePolicy: "best_effort",
  micStartupTimeoutMs: 2500,
  micRetryIntervalSecs: 15,
  micMixGainDb: 6,
  micAutoRequestPermission: true,
  audioStartupTimeoutMs: 6000,
  profileRecoverHoldSecs: 20,
  excludeCurrentProcessAudio: true,
  savePathMode: "instant_mp4",
  audioSaveMode: "fast",
  performanceGuardEnabled: true,
  performanceGuardLevel: "balanced",
};

function App() {
  const [engineState, setEngineState] = useState<EngineStateDto | null>(null);
  const [settings, setSettings] = useState<SettingsDto>(fallbackSettings);
  const [recentClips, setRecentClips] = useState<ClipMetadataDto[]>([]);
  const [microphoneDevices, setMicrophoneDevices] = useState<MicrophoneDeviceDto[]>([]);
  const [status, setStatus] = useState<string>("Loading engine state...");
  const [isSubmittingSettings, setIsSubmittingSettings] = useState(false);
  const transientStatusTimerRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const transientStatusUntilRef = useRef<number>(0);
  const lastTransientMessageRef = useRef<string>("");
  const lastTransientAtRef = useRef<number>(0);
  const engineStateRef = useRef<EngineStateDto | null>(null);
  const outputDirPollTimerRef = useRef<ReturnType<typeof window.setInterval> | null>(null);
  const outputDirPollDeadlineRef = useRef<number>(0);
  const outputDirPollInFlightRef = useRef(false);
  const screenPollTimerRef = useRef<ReturnType<typeof window.setInterval> | null>(null);
  const screenPollDeadlineRef = useRef<number>(0);
  const screenPollInFlightRef = useRef(false);
  const micPollTimerRef = useRef<ReturnType<typeof window.setInterval> | null>(null);
  const micPollDeadlineRef = useRef<number>(0);
  const micPollInFlightRef = useRef(false);

  const [formReplayDuration, setFormReplayDuration] = useState(30);
  const [formBufferDuration, setFormBufferDuration] = useState(120);
  const [formFps, setFormFps] = useState(60);
  const [formVideoResolution, setFormVideoResolution] = useState(1080);
  const [formAudioBitrate, setFormAudioBitrate] = useState(160);
  const [formAudioMode, setFormAudioMode] = useState<"system_only" | "system_plus_mic" | "video_only">(
    "system_only",
  );
  const [formMicEnabled, setFormMicEnabled] = useState(false);
  const [formQualityPolicy, setFormQualityPolicy] = useState<"adaptive_recover" | "strict">(
    "adaptive_recover",
  );
  const [formQualityPreference, setFormQualityPreference] = useState<
    "prefer_quality" | "prefer_smoothness"
  >("prefer_quality");
  const [formAudioFallbackPolicy, setFormAudioFallbackPolicy] = useState<
    "system_only_fallback" | "allow_video_only"
  >("system_only_fallback");
  const [formMicCaptureBackend, setFormMicCaptureBackend] = useState<
    "auto" | "avcapture" | "sck_native"
  >("auto");
  const [formSelectedMicrophoneId, setFormSelectedMicrophoneId] = useState("");
  const [formOutputDir, setFormOutputDir] = useState("");
  const [formHotkey, setFormHotkey] = useState("Ctrl+Option+R");
  const [formExcludeCurrentProcessAudio, setFormExcludeCurrentProcessAudio] = useState(true);
  const [formSavePathMode, setFormSavePathMode] = useState<
    "instant_mp4" | "smooth" | "adaptive" | "fast"
  >("instant_mp4");
  const [formAudioSaveMode, setFormAudioSaveMode] = useState<"smooth" | "fast" | "adaptive">(
    "fast",
  );

  const permissionText = useMemo(() => {
    if (!engineState) {
      return "Checking";
    }
    const permission = engineState.permission;
    if (
      permission.screenRecordingGranted &&
      permission.systemAudioGranted &&
      permission.outputDirWritable
    ) {
      return "Granted";
    }
    if (!permission.outputDirWritable) {
      return "Downloads permission required";
    }
    return permission.reason ?? "Permission required";
  }, [engineState]);

  const showScreenGrantCta = useMemo(
    () =>
      Boolean(
        engineState &&
          (engineState.armBlockerCode === "permission_required" ||
            engineState.permission.screenRecordingGranted === false),
      ),
    [engineState],
  );
  const showResumeCaptureCta = useMemo(
    () => Boolean(engineState && engineState.armBlockerCode === "capture_paused"),
    [engineState],
  );
  const showEnableReplayCta = useMemo(
    () =>
      Boolean(
        engineState &&
          engineState.armBlockerCode === "user_stopped_sharing" &&
          !settings.replayEnabled,
      ),
    [engineState, settings.replayEnabled],
  );
  const showDownloadsGrantCta = useMemo(
    () => Boolean(engineState && isOutputDirBlocked(engineState)),
    [engineState],
  );
  const showMicGrantCta = useMemo(
    () =>
      Boolean(
        engineState &&
          !isCapturePaused(engineState) &&
          !isScreenPermissionBlocked(engineState) &&
          !isOutputDirBlocked(engineState) &&
          isMicPermissionAssistNeeded(engineState),
      ),
    [engineState],
  );

  function isRetryableBlockerCode(code: string | null | undefined): boolean {
    return code === "audio_warming_up" || code === "busy" || code === "engine_starting";
  }

  function isScreenPermissionBlocked(next: EngineStateDto | null | undefined): boolean {
    if (!next) {
      return false;
    }
    return (
      next.armBlockerCode === "permission_required" ||
      next.permission.screenRecordingGranted === false
    );
  }

  function isCapturePaused(next: EngineStateDto | null | undefined): boolean {
    if (!next) {
      return false;
    }
    return (
      next.armBlockerCode === "capture_paused" ||
      next.armBlockerCode === "user_stopped_sharing"
    );
  }

  function isOutputDirBlocked(next: EngineStateDto | null | undefined): boolean {
    if (!next) {
      return false;
    }
    return (
      next.armBlockerCode === "output_dir_permission_required" ||
      next.permission.outputDirWritable === false
    );
  }

  function isMicPermissionAssistNeeded(next: EngineStateDto | null | undefined): boolean {
    if (!next) {
      return false;
    }
    if (!next.settings.micEnabled) {
      return false;
    }
    return (
      next.micPermissionStatus === "denied" ||
      next.micPermissionStatus === "restricted" ||
      next.micPermissionStatus === "not_determined"
    );
  }

  function describeEngineState(next: EngineStateDto): string {
    if (next.isSaving) {
      return "Saving replay...";
    }
    if (!isOutputDirBlocked(next) && isMicPermissionAssistNeeded(next)) {
      return "Microphone permission required for mic capture. Use Enable Microphone Permission.";
    }
    if (next.operatorHealthMessage) {
      return next.operatorHealthMessage;
    }
    if (next.armBlocker && !isRetryableBlockerCode(next.armBlockerCode)) {
      switch (next.armBlockerCode) {
        case "output_dir_permission_required":
          return "Downloads permission required to save clips.";
        case "permission_required":
          return "Replay blocked: Screen Recording permission is denied. Enable it in System Settings.";
        case "system_audio_unavailable":
          return "Replay blocked: System audio path unavailable for current source.";
        case "mic_required_unavailable":
          return "Replay blocked: Microphone required but unavailable.";
        case "capture_paused":
          return next.armBlocker ?? "Capture paused. Click Resume Capture to continue.";
        case "user_stopped_sharing":
          return next.operatorHealthMessage || "Capture stopped from macOS controls. Rewinder is not recording.";
        default:
          return `Replay blocked: ${next.armBlocker}`;
      }
    }
    if (next.captureHealth === "restarting") {
      return "Capture restarting...";
    }
    if (next.captureHealth === "degraded" && next.lastError) {
      return `Capture degraded: ${next.lastError}`;
    }
    return `Engine ${next.lifecycleState}`;
  }

  function hydrateForm(next: SettingsDto): void {
    setFormReplayDuration(next.replayDurationSecs);
    setFormBufferDuration(next.bufferDurationSecs);
    setFormFps(next.fps);
    setFormVideoResolution(next.videoResolution);
    setFormAudioBitrate(next.audioBitrateKbps);
    setFormAudioMode(next.audioMode);
    setFormMicEnabled(next.micEnabled);
    setFormQualityPolicy(next.qualityPolicy);
    setFormQualityPreference(next.qualityPreference);
    setFormAudioFallbackPolicy(next.audioFallbackPolicy);
    setFormMicCaptureBackend(next.micCaptureBackend);
    setFormSelectedMicrophoneId(next.selectedMicrophoneId ?? "");
    setFormOutputDir(next.outputDir);
    setFormHotkey(next.hotkey);
    setFormExcludeCurrentProcessAudio(next.excludeCurrentProcessAudio);
    setFormSavePathMode(next.savePathMode);
    setFormAudioSaveMode(next.audioSaveMode);
  }

  function setTransientStatus(message: string, durationMs = 1200): void {
    if (isOutputDirBlocked(engineStateRef.current)) {
      const lower = message.toLowerCase();
      const allowed =
        lower.includes("downloads") ||
        lower.includes("permission") ||
        lower.includes("access granted");
      if (!allowed) {
        return;
      }
    }
    const now = Date.now();
    if (
      lastTransientMessageRef.current === message &&
      now - lastTransientAtRef.current < 1400
    ) {
      return;
    }
    lastTransientMessageRef.current = message;
    lastTransientAtRef.current = now;
    transientStatusUntilRef.current = now + durationMs;
    setStatus(message);
    if (transientStatusTimerRef.current) {
      window.clearTimeout(transientStatusTimerRef.current);
    }
    transientStatusTimerRef.current = window.setTimeout(() => {
      transientStatusTimerRef.current = null;
      const current = engineStateRef.current;
      if (current) {
        setStatus(describeEngineState(current));
      }
    }, durationMs);
  }

  function stopOutputDirAccessPolling(): void {
    if (outputDirPollTimerRef.current) {
      window.clearInterval(outputDirPollTimerRef.current);
      outputDirPollTimerRef.current = null;
    }
    outputDirPollDeadlineRef.current = 0;
    outputDirPollInFlightRef.current = false;
  }

  function stopScreenAccessPolling(): void {
    if (screenPollTimerRef.current) {
      window.clearInterval(screenPollTimerRef.current);
      screenPollTimerRef.current = null;
    }
    screenPollDeadlineRef.current = 0;
    screenPollInFlightRef.current = false;
  }

  function stopMicAccessPolling(): void {
    if (micPollTimerRef.current) {
      window.clearInterval(micPollTimerRef.current);
      micPollTimerRef.current = null;
    }
    micPollDeadlineRef.current = 0;
    micPollInFlightRef.current = false;
  }

  async function pollOutputDirAccessOnce(): Promise<void> {
    if (outputDirPollInFlightRef.current) {
      return;
    }
    outputDirPollInFlightRef.current = true;
    try {
      const permission = await recheckPermissions();
      if (permission.outputDirWritable) {
        stopOutputDirAccessPolling();
        setTransientStatus("Access granted, capture resumed.", 1800);
        return;
      }
      if (Date.now() >= outputDirPollDeadlineRef.current) {
        stopOutputDirAccessPolling();
        setStatus(
          "Downloads access still required. Enable Rewinder (or Terminal in dev) in Files and Folders > Downloads.",
        );
      }
    } catch {
      if (Date.now() >= outputDirPollDeadlineRef.current) {
        stopOutputDirAccessPolling();
      }
    } finally {
      outputDirPollInFlightRef.current = false;
    }
  }

  async function pollScreenAccessOnce(): Promise<void> {
    if (screenPollInFlightRef.current) {
      return;
    }
    screenPollInFlightRef.current = true;
    try {
      const permission = await recheckPermissions();
      if (permission.screenRecordingGranted) {
        stopScreenAccessPolling();
        setTransientStatus("Screen Recording access granted, capture resumed.", 1800);
        return;
      }
      if (Date.now() >= screenPollDeadlineRef.current) {
        stopScreenAccessPolling();
        setStatus(
          "Screen Recording access still required. Enable Rewinder (or Terminal in dev) in Privacy & Security > Screen Recording.",
        );
      }
    } catch {
      if (Date.now() >= screenPollDeadlineRef.current) {
        stopScreenAccessPolling();
      }
    } finally {
      screenPollInFlightRef.current = false;
    }
  }

  async function pollMicAccessOnce(): Promise<void> {
    if (micPollInFlightRef.current) {
      return;
    }
    micPollInFlightRef.current = true;
    try {
      const result = await grantMicrophoneAccess(false);
      const next = await getEngineState();
      setEngineState(next);
      engineStateRef.current = next;
      setSettings(next.settings);

      if (result.micPermissionStatus === "granted") {
        stopMicAccessPolling();
        setTransientStatus("Microphone access granted, capture resumed.", 1800);
        return;
      }

      if (Date.now() >= micPollDeadlineRef.current) {
        stopMicAccessPolling();
        setStatus(
          "Microphone access still required. Enable Rewinder (or Terminal in dev) in Privacy & Security > Microphone.",
        );
      }
    } catch {
      if (Date.now() >= micPollDeadlineRef.current) {
        stopMicAccessPolling();
      }
    } finally {
      micPollInFlightRef.current = false;
    }
  }

  function startOutputDirAccessPolling(timeoutMs = 30000): void {
    stopOutputDirAccessPolling();
    outputDirPollDeadlineRef.current = Date.now() + timeoutMs;
    outputDirPollTimerRef.current = window.setInterval(() => {
      void pollOutputDirAccessOnce();
    }, 1500);
    void pollOutputDirAccessOnce();
  }

  function startScreenAccessPolling(timeoutMs = 30000): void {
    stopScreenAccessPolling();
    screenPollDeadlineRef.current = Date.now() + timeoutMs;
    screenPollTimerRef.current = window.setInterval(() => {
      void pollScreenAccessOnce();
    }, 1500);
    void pollScreenAccessOnce();
  }

  function startMicAccessPolling(timeoutMs = 30000): void {
    stopMicAccessPolling();
    micPollDeadlineRef.current = Date.now() + timeoutMs;
    micPollTimerRef.current = window.setInterval(() => {
      void pollMicAccessOnce();
    }, 1500);
    void pollMicAccessOnce();
  }

  async function refreshAll(): Promise<void> {
    const [nextState, nextSettings, nextClips, nextMicrophones] = await Promise.all([
      getEngineState(),
      getSettings(),
      listRecentClips(20),
      listMicrophones().catch(() => []),
    ]);
    setEngineState(nextState);
    engineStateRef.current = nextState;
    setSettings(nextSettings);
    setRecentClips(nextClips);
    setMicrophoneDevices(nextMicrophones);
    hydrateForm(nextSettings);
    setStatus(describeEngineState(nextState));
  }

  useEffect(() => {
    let mounted = true;
    const unlisten: Array<() => void> = [];

    refreshAll().catch((error: unknown) => {
      if (mounted) {
        setStatus(`Failed to initialize: ${String(error)}`);
      }
    });

    Promise.all([
      onEngineStateChanged((next) => {
        if (!mounted) {
          return;
        }
        setEngineState(next);
        engineStateRef.current = next;
        setSettings(next.settings);
        const now = Date.now();
        const hardBlocker = Boolean(next.armBlocker && !isRetryableBlockerCode(next.armBlockerCode));
        if (!hardBlocker && next.captureHealth !== "degraded" && now < transientStatusUntilRef.current) {
          return;
        }
        setStatus(describeEngineState(next));
      }),
      onClipSaved((clip) => {
        if (!mounted) {
          return;
        }
        setRecentClips((prev) => [clip, ...prev.filter((item) => item.id !== clip.id)].slice(0, 50));
        setStatus(`Saved replay: ${clip.id}`);
      }),
      onSaveFailed((error) => {
        if (!mounted) {
          return;
        }
        if (error.code === "output_dir_permission_required") {
          setStatus("Downloads permission required to save clips.");
          return;
        }
        if (error.code === "user_stopped_sharing") {
          setStatus(
            error.message ||
              "Capture stopped from macOS controls. Rewinder is not recording.",
          );
          return;
        }
        const guidance = error.action ? ` (${error.action})` : "";
        setStatus(`Save failed: ${error.message}${guidance}`);
      }),
      onSaveDeferred((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(payload.message);
      }),
      onSaveWarning((payload) => {
        if (!mounted) {
          return;
        }
        if (payload.code === "duration_corrected") {
          setTransientStatus(payload.message, 1800);
          return;
        }
        if (payload.code === "partial_history") {
          const action = payload.action ? ` (${payload.action})` : "";
          setTransientStatus(`${payload.message}${action}`, 2600);
          return;
        }
        if (payload.code === "mic_signal_missing") {
          const action = payload.action ? ` (${payload.action})` : "";
          setTransientStatus(`Mic signal warning: ${payload.message}${action}`, 2200);
          return;
        }
        const action = payload.action ? ` (${payload.action})` : "";
        setTransientStatus(`Save warning: ${payload.message}${action}`, 2000);
      }),
      onCaptureHealthChanged((payload) => {
        if (!mounted) {
          return;
        }
        if (payload.reason) {
          setTransientStatus(`Capture ${payload.health}: ${payload.reason}`, 1400);
        } else {
          setTransientStatus(`Capture ${payload.health}`, 1200);
        }
      }),
      onCaptureRestarted((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(`Capture restarted (${payload.reason})`, 1200);
      }),
      onCaptureProfileChanged((payload) => {
        if (!mounted) {
          return;
        }
        const text = payload.reason.toLowerCase().includes("overload")
          ? "Performance guard adjusted quality to keep capture stable."
          : "Capture profile adjusted.";
        setTransientStatus(text, 1600);
      }),
      onCaptureProfileRecovered((payload) => {
        if (!mounted) {
          return;
        }
        const text = payload.reason.toLowerCase().includes("recover")
          ? "Capture load recovering; quality will step back up automatically."
          : "Capture profile recovered.";
        setTransientStatus(text, 1600);
      }),
      onCapturePaused((payload) => {
        if (!mounted) {
          return;
        }
        setStatus(
          payload.message || "Capture paused.",
        );
      }),
      onCaptureResumed((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(payload.message || "Capture resumed.", 1600);
      }),
      onAudioModeChanged((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(
          payload.reason
            ? `Audio mode: ${payload.mode} (${payload.reason})`
            : `Audio mode: ${payload.mode}`,
          1500,
        );
      }),
      onCaptureDegraded((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(`Capture degraded: ${payload.message}`, 1800);
      }),
      onAudioPathFailed((payload) => {
        if (!mounted) {
          return;
        }
        const action = payload.action ? ` (${payload.action})` : "";
        setTransientStatus(`Audio path failed: ${payload.message}${action}`, 2200);
      }),
      onAudioPathReady((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(`Audio path ready (${payload.mode})`, 1400);
      }),
      onMicPathDegraded((payload) => {
        if (!mounted) {
          return;
        }
        const action = payload.action ? ` (${payload.action})` : "";
        setTransientStatus(`Mic degraded: ${payload.message}${action}`, 1800);
      }),
      onMicPathRecovered((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(payload.message || "Mic path recovered", 1400);
      }),
      onMicPermissionChanged((payload) => {
        if (!mounted) {
          return;
        }
        const suffix = payload.message ? ` (${payload.message})` : "";
        setTransientStatus(`Mic permission: ${payload.status}${suffix}`, 1800);
        if (payload.status === "granted") {
          stopMicAccessPolling();
        }
      }),
      onHotkeyConflict((payload) => {
        if (!mounted) {
          return;
        }
        setTransientStatus(`Hotkey conflict: ${payload.message}`, 1800);
      }),
      onPermissionRequired((error) => {
        if (!mounted) {
          return;
        }
        const lower = error.message.toLowerCase();
        if (lower.includes("screen recording")) {
          setStatus("Screen Recording permission required. Use the button below.");
          return;
        }
        if (lower.includes("downloads") || lower.includes("files and folders")) {
          setStatus("Downloads permission required. Use the button below.");
          return;
        }
        setStatus(`Permission required: ${error.message}`);
      }),
      onHotkeyTriggered((payload) => {
        if (!mounted) {
          return;
        }
        setStatus(`Hotkey triggered: ${payload.hotkey}`);
      }),
      onSettingsUpdated((payload) => {
        if (!mounted) {
          return;
        }
        setStatus(payload.message);
      }),
    ])
      .then((listeners) => {
        listeners.forEach((stop) => unlisten.push(stop));
      })
      .catch((error: unknown) => {
        if (mounted) {
          setStatus(`Failed to subscribe: ${String(error)}`);
        }
      });

    return () => {
      mounted = false;
      if (transientStatusTimerRef.current) {
        window.clearTimeout(transientStatusTimerRef.current);
      }
      stopScreenAccessPolling();
      stopOutputDirAccessPolling();
      stopMicAccessPolling();
      unlisten.forEach((stop) => stop());
    };
  }, []);

  async function handleManualSave(): Promise<void> {
    setStatus("Saving replay...");
    const result = await triggerSaveReplay("manual");
    if (result.queued) {
      setTransientStatus(result.message ?? "Replay warming up, will save automatically.");
      return;
    }
    if (!result.ok) {
      if (
        (result.error ?? "").toLowerCase().includes("downloads folder access is denied") ||
        (result.error ?? "").toLowerCase().includes("output_dir_permission_required")
      ) {
        setStatus("Downloads permission required to save clips.");
        return;
      }
      setStatus(`Save failed: ${result.error ?? "unknown error"}`);
      return;
    }
    if (result.clip) {
      const duration = result.actualDurationSecs ?? result.clip.durationSecs;
      const strategy = result.saveAudioStrategy ? `, audio=${result.saveAudioStrategy}` : "";
      const repaired = result.audioRepaired ? ", smoothed" : "";
      const smoothPending = result.smoothPending ? ", smooth pass pending" : "";
      const smoothError = result.smoothError ? `, smooth_error=${result.smoothError}` : "";
      const window =
        result.selectedDurationSecs != null && result.requestedDurationSecs != null
          ? `, window=${result.selectedDurationSecs.toFixed(2)}s/${result.requestedDurationSecs.toFixed(2)}s`
          : "";
      const partialReason = result.partialReasonCode
        ? `, partial_reason=${result.partialReasonCode}`
        : "";
      const anchor = result.anchorEpochMs != null ? `, anchor=${result.anchorEpochMs}` : "";
      const effectiveProfile =
        result.effectiveVideoResolution != null && result.effectiveFps != null
          ? `, profile=${result.effectiveVideoResolution}p${result.effectiveFps}`
          : "";
      const detail = result.message ? ` (${result.message})` : "";
      setStatus(
        `Replay saved (${duration.toFixed(1)}s${repaired}${smoothPending}${strategy}${smoothError}${effectiveProfile}${window}${partialReason}${anchor}): ${result.clip.path}${detail}`,
      );
    }
  }

  async function handleToggleReplay(enabled: boolean): Promise<void> {
    const next = await setReplayEnabled(enabled);
    setEngineState(next);
    setSettings(next.settings);
    setStatus(enabled ? "Replay armed" : "Replay disabled");
  }

  async function handlePermissionRecheck(): Promise<void> {
    const permission = await recheckPermissions();
    setStatus(
      permission.screenRecordingGranted &&
        permission.systemAudioGranted &&
        permission.outputDirWritable
        ? "Permissions granted"
        : `Permission required: ${
            permission.outputDirPermissionError ??
            permission.reason ??
            "missing permissions"
          }`,
    );
    const next = await getEngineState();
    setEngineState(next);
    setSettings(next.settings);
  }

  async function handleRequestMicPermission(): Promise<void> {
    const permission = await requestMicrophonePermission();
    setStatus(
      permission.screenRecordingGranted &&
        permission.systemAudioGranted &&
        permission.outputDirWritable
        ? "Mic permission checked"
        : `Permission required: ${
            permission.outputDirPermissionError ??
            permission.reason ??
            "missing permissions"
          }`,
    );
    const next = await getEngineState();
    setEngineState(next);
    setSettings(next.settings);
  }

  async function handleGrantDownloadsAccess(): Promise<void> {
    const result = await grantOutputDirAccess();
    setStatus(result.message);
    const next = await getEngineState();
    setEngineState(next);
    setSettings(next.settings);
    if (!result.permission.outputDirWritable) {
      startOutputDirAccessPolling(30000);
    } else {
      stopOutputDirAccessPolling();
    }
  }

  async function handleGrantScreenRecordingAccess(): Promise<void> {
    const result = await grantScreenRecordingAccess();
    setStatus(result.message);
    const next = await getEngineState();
    setEngineState(next);
    engineStateRef.current = next;
    setSettings(next.settings);
    if (!result.permission.screenRecordingGranted) {
      startScreenAccessPolling(30000);
    } else {
      stopScreenAccessPolling();
    }
  }

  async function handleResumeCapture(): Promise<void> {
    setStatus("Resuming capture...");
    try {
      const next = await resumeCapture();
      setEngineState(next);
      engineStateRef.current = next;
      setSettings(next.settings);
      setStatus("Capture resumed.");
    } catch (error: unknown) {
      setStatus(`Resume failed: ${String(error)}`);
    }
  }

  async function handleGrantMicrophoneAccess(): Promise<void> {
    const result = await grantMicrophoneAccess(true);
    setStatus(result.message);
    const next = await getEngineState();
    setEngineState(next);
    engineStateRef.current = next;
    setSettings(next.settings);
    if (result.micPermissionStatus !== "granted") {
      startMicAccessPolling(30000);
    } else {
      stopMicAccessPolling();
    }
  }

  async function handleSettingsSubmit(event: React.FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setIsSubmittingSettings(true);
    try {
      const next = await updateSettings({
        replayDurationSecs: formReplayDuration,
        bufferDurationSecs: formBufferDuration,
        fps: formFps,
        videoResolution: formVideoResolution,
        audioBitrateKbps: formAudioBitrate,
        audioMode: formAudioMode,
        micEnabled: formMicEnabled,
        qualityPolicy: formQualityPolicy,
        qualityPreference: formQualityPreference,
        audioFallbackPolicy: formAudioFallbackPolicy,
        micCaptureBackend: formMicCaptureBackend,
        selectedMicrophoneId: formSelectedMicrophoneId,
        excludeCurrentProcessAudio: formExcludeCurrentProcessAudio,
        savePathMode: formSavePathMode,
        audioSaveMode: formAudioSaveMode,
        outputDir: formOutputDir,
        hotkey: formHotkey,
      });
      setSettings(next);
      hydrateForm(next);
      setStatus("Settings updated");
      const nextState = await getEngineState();
      setEngineState(nextState);
    } catch (error: unknown) {
      setStatus(`Settings update failed: ${String(error)}`);
    } finally {
      setIsSubmittingSettings(false);
    }
  }

  const disableSaveButton = Boolean(engineState?.isSaving || engineState?.pendingSave);
  const showCapturePrivacyNote = Boolean(
    settings.replayEnabled && engineState?.permission.screenRecordingGranted,
  );
  const showBackgroundRunningNote = Boolean(!import.meta.env.DEV && settings.replayEnabled);
  const profileFallbackActive = Boolean(
    engineState &&
      engineState.guardState === "protecting" &&
      engineState.captureHealth === "running" &&
      engineState.saveReady &&
      engineState.captureStartPhase === "first_segment" &&
      (engineState.effectiveVideoResolution < engineState.requestedVideoResolution ||
        engineState.effectiveFps < engineState.requestedFps),
  );

  return (
    <main className="app">
      <HeaderPanel
        status={status}
        showCapturePrivacyNote={showCapturePrivacyNote}
        showBackgroundRunningNote={showBackgroundRunningNote}
        profileFallbackActive={profileFallbackActive}
        engineState={engineState}
        settings={settings}
        showResumeCaptureCta={showResumeCaptureCta}
        showEnableReplayCta={showEnableReplayCta}
        showScreenGrantCta={showScreenGrantCta}
        showDownloadsGrantCta={showDownloadsGrantCta}
        showMicGrantCta={showMicGrantCta}
        onResumeCapture={handleResumeCapture}
        onEnableReplay={() => void handleToggleReplay(true)}
        onGrantScreenRecordingAccess={handleGrantScreenRecordingAccess}
        onGrantDownloadsAccess={handleGrantDownloadsAccess}
        onGrantMicrophoneAccess={handleGrantMicrophoneAccess}
      />
      <EngineStatusPanel
        engineState={engineState}
        settings={settings}
        permissionText={permissionText}
        disableSaveButton={disableSaveButton}
        onManualSave={handleManualSave}
        onToggleReplay={handleToggleReplay}
        onPermissionRecheck={handlePermissionRecheck}
        onRequestMicPermission={handleRequestMicPermission}
      />
      <SettingsPanel
        isSubmittingSettings={isSubmittingSettings}
        formReplayDuration={formReplayDuration}
        formBufferDuration={formBufferDuration}
        formFps={formFps}
        formVideoResolution={formVideoResolution}
        formAudioBitrate={formAudioBitrate}
        formAudioMode={formAudioMode}
        formMicEnabled={formMicEnabled}
        formQualityPolicy={formQualityPolicy}
        formQualityPreference={formQualityPreference}
        formAudioFallbackPolicy={formAudioFallbackPolicy}
        formMicCaptureBackend={formMicCaptureBackend}
        formSelectedMicrophoneId={formSelectedMicrophoneId}
        microphoneDevices={microphoneDevices}
        formExcludeCurrentProcessAudio={formExcludeCurrentProcessAudio}
        formSavePathMode={formSavePathMode}
        formAudioSaveMode={formAudioSaveMode}
        formOutputDir={formOutputDir}
        formHotkey={formHotkey}
        onSubmit={handleSettingsSubmit}
        setFormReplayDuration={setFormReplayDuration}
        setFormBufferDuration={setFormBufferDuration}
        setFormFps={setFormFps}
        setFormVideoResolution={setFormVideoResolution}
        setFormAudioBitrate={setFormAudioBitrate}
        setFormAudioMode={setFormAudioMode}
        setFormMicEnabled={setFormMicEnabled}
        setFormQualityPolicy={setFormQualityPolicy}
        setFormQualityPreference={setFormQualityPreference}
        setFormAudioFallbackPolicy={setFormAudioFallbackPolicy}
        setFormMicCaptureBackend={setFormMicCaptureBackend}
        setFormSelectedMicrophoneId={setFormSelectedMicrophoneId}
        setFormExcludeCurrentProcessAudio={setFormExcludeCurrentProcessAudio}
        setFormSavePathMode={setFormSavePathMode}
        setFormAudioSaveMode={setFormAudioSaveMode}
        setFormOutputDir={setFormOutputDir}
        setFormHotkey={setFormHotkey}
      />
      <RecentClipsPanel recentClips={recentClips} />
    </main>
  );
}

export default App;
