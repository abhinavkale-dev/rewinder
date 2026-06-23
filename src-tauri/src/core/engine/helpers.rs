use super::*;

pub(super) fn profile_from_settings(settings: &SettingsDto) -> EffectiveCaptureProfile {
    EffectiveCaptureProfile {
        video_resolution: settings.video_resolution,
        fps: settings.fps,
        video_bitrate_kbps: settings.video_bitrate_kbps,
    }
}

pub(super) fn effective_profile_for_index(
    settings: &SettingsDto,
    index: usize,
) -> EffectiveCaptureProfile {
    let requested = profile_from_settings(settings);
    let requested_fps = requested.fps.max(1);
    let requested_res = requested.video_resolution.max(360);
    match index {
        0 => requested,
        1 => EffectiveCaptureProfile {
            video_resolution: requested_res,
            fps: requested_fps.min(30),
            video_bitrate_kbps: requested.video_bitrate_kbps.min(8_000),
        },
        2 => EffectiveCaptureProfile {
            video_resolution: requested_res,
            fps: requested_fps.min(30),
            video_bitrate_kbps: requested.video_bitrate_kbps.min(5_500),
        },
        _ => EffectiveCaptureProfile {
            video_resolution: requested_res.min(720),
            fps: requested_fps.min(30),
            video_bitrate_kbps: requested.video_bitrate_kbps.min(4_800),
        },
    }
}

pub(super) fn battery_floor_index(
    settings: &SettingsDto,
    on_battery: bool,
    battery_guard_enabled: bool,
    battery_max_fps: u16,
) -> usize {
    if !on_battery || !battery_guard_enabled {
        return 0;
    }
    for index in 0..=MAX_RUNTIME_PROFILE_INDEX {
        if effective_profile_for_index(settings, index).fps <= battery_max_fps {
            return index;
        }
    }
    MAX_RUNTIME_PROFILE_INDEX
}

pub(super) fn normalize_patch_for_runtime(current: &SettingsDto, patch: &mut SettingsPatchDto) {
    if let Some(replay) = patch.replay_duration_secs {
        let buffer_candidate = patch
            .buffer_duration_secs
            .unwrap_or(current.buffer_duration_secs);
        patch.buffer_duration_secs = Some(ensure_buffer_for_replay(replay, buffer_candidate));
    }

    if patch.mic_enabled == Some(true) {
        let target_audio_mode = patch
            .audio_mode
            .clone()
            .unwrap_or_else(|| current.audio_mode.clone());
        if target_audio_mode == "system_only" {
            patch.audio_mode = Some("system_plus_mic".to_string());
        }
    }
}

pub(super) fn build_settings_updated_message(source: &str, patch: &SettingsPatchDto) -> String {
    let from_tray = source == "tray";

    if patch.mic_enabled == Some(true) && patch.audio_mode.as_deref() == Some("system_plus_mic") {
        return if from_tray {
            "Mic enabled; switched audio mode to system+mic.".to_string()
        } else {
            format!("{source}: mic enabled; switched audio mode to system+mic")
        };
    }

    if let Some(seconds) = patch.replay_duration_secs {
        return if from_tray {
            format!("Tray updated replay length to {seconds}s")
        } else {
            format!("{source}: replay length set to {seconds}s")
        };
    }

    if let Some(height) = patch.video_resolution {
        let resolution = ResolutionPreset::from_height(height);
        return if from_tray {
            format!("Tray updated resolution to {resolution}")
        } else {
            format!("{source}: resolution set to {resolution}")
        };
    }

    if let Some(video_bitrate_kbps) = patch.video_bitrate_kbps {
        return if from_tray {
            format!("Tray updated bitrate to {video_bitrate_kbps} kbps")
        } else {
            format!("{source}: bitrate set to {video_bitrate_kbps} kbps")
        };
    }

    if let Some(fps) = patch.fps {
        return if from_tray {
            format!("Tray updated FPS to {fps}")
        } else {
            format!("{source}: fps set to {fps}")
        };
    }

    if let Some(audio_mode) = patch.audio_mode.as_deref() {
        return if from_tray {
            format!("Tray updated audio mode to {audio_mode}")
        } else {
            format!("{source}: audio mode set to {audio_mode}")
        };
    }

    if let Some(mic_enabled) = patch.mic_enabled {
        return if from_tray {
            if mic_enabled {
                "Tray enabled microphone capture".to_string()
            } else {
                "Tray disabled microphone capture".to_string()
            }
        } else if mic_enabled {
            format!("{source}: microphone capture enabled")
        } else {
            format!("{source}: microphone capture disabled")
        };
    }

    if let Some(enabled) = patch.replay_enabled {
        return if enabled {
            if from_tray {
                "Tray enabled replay".to_string()
            } else {
                format!("{source}: replay enabled")
            }
        } else if from_tray {
            "Tray disabled replay".to_string()
        } else {
            format!("{source}: replay disabled")
        };
    }

    format!("{source}: settings updated")
}

pub(super) fn trigger_source_label(source: &TriggerSourceDto) -> &'static str {
    match source {
        TriggerSourceDto::Manual => "manual",
        TriggerSourceDto::Hotkey => "hotkey",
    }
}

pub(super) fn to_epoch_ms(value: SystemTime) -> u128 {
    value
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(super) fn to_epoch_ms_i64(value: SystemTime) -> i64 {
    let millis = to_epoch_ms(value);
    i64::try_from(millis).unwrap_or(i64::MAX)
}

pub(super) fn derive_mic_attach_state(
    settings: &SettingsDto,
    active_audio_mode: &str,
    mic_path_ready: bool,
    mic_frames_seen: bool,
    runtime_state: Option<CaptureMicAttachState>,
    current_state: MicAttachStateDto,
) -> MicAttachStateDto {
    if !settings.mic_enabled || settings.audio_mode != "system_plus_mic" {
        return MicAttachStateDto::Inactive;
    }
    if active_audio_mode != "system_plus_mic" {
        return MicAttachStateDto::Degraded;
    }
    if let Some(state) = runtime_state {
        return match state {
            CaptureMicAttachState::SilenceFiller => MicAttachStateDto::SilenceFiller,
            CaptureMicAttachState::Live => MicAttachStateDto::Live,
            CaptureMicAttachState::Degraded => MicAttachStateDto::Degraded,
        };
    }
    if mic_path_ready && mic_frames_seen {
        return MicAttachStateDto::Live;
    }
    if mic_path_ready {
        return MicAttachStateDto::SilenceFiller;
    }
    if matches!(current_state, MicAttachStateDto::Live) {
        return MicAttachStateDto::Degraded;
    }
    MicAttachStateDto::SilenceFiller
}

pub(super) fn save_blocker(state: &ClipperState) -> Option<SaveBlocker> {
    if state.is_saving || state.lifecycle_state == LifecycleState::SavingReplay {
        return Some(SaveBlocker {
            code: "busy",
            message: "Replay save already in progress.".to_string(),
            action: Some("Wait for the current save to finish.".to_string()),
            retryable: true,
        });
    }

    if !state.permission.output_dir_writable {
        return Some(SaveBlocker {
            code: "output_dir_permission_required",
            message: state
                .permission
                .output_dir_permission_error
                .clone()
                .unwrap_or_else(|| {
                    "Downloads folder access is denied for Rewinder.".to_string()
                }),
            action: Some(
                "Enable Rewinder in System Settings > Privacy & Security > Files and Folders (Downloads)."
                    .to_string(),
            ),
            retryable: false,
        });
    }

    if state.lifecycle_state == LifecycleState::PermissionRequired {
        return Some(SaveBlocker {
            code: "permission_required",
            message: state.permission.reason.clone().unwrap_or_else(|| {
                "Screen Recording permission is denied. Enable it in System Settings.".to_string()
            }),
            action: Some(
                "Open System Settings > Privacy & Security and grant required permissions."
                    .to_string(),
            ),
            retryable: false,
        });
    }

    if state.lifecycle_state == LifecycleState::Disabled {
        return Some(SaveBlocker {
            code: "disabled",
            message: "Replay is disabled. Enable replay to save clips.".to_string(),
            action: Some("Enable replay from tray or settings.".to_string()),
            retryable: false,
        });
    }

    if state.lifecycle_state == LifecycleState::Booting {
        return Some(SaveBlocker {
            code: "engine_starting",
            message: "Replay engine is starting. Try again in a moment.".to_string(),
            action: None,
            retryable: true,
        });
    }

    let capture_warming = matches!(
        state.capture_health,
        CaptureHealthDto::Starting | CaptureHealthDto::Restarting | CaptureHealthDto::Stopped
    );
    let requires_system_audio = state.settings.audio_fallback_policy == "system_only_fallback"
        && state.settings.audio_mode != "video_only";
    if requires_system_audio && !state.system_audio_path_ready {
        if capture_warming {
            return Some(SaveBlocker {
                code: "audio_warming_up",
                message: "System audio path is starting. Try again in a moment.".to_string(),
                action: None,
                retryable: true,
            });
        }
        return Some(SaveBlocker {
            code: "system_audio_unavailable",
            message: "System audio path unavailable for current source.".to_string(),
            action: Some(
                "Check source availability or switch audio fallback policy to allow_video_only."
                    .to_string(),
            ),
            retryable: false,
        });
    }

    let mic_required = state.settings.audio_mode == "system_plus_mic"
        && state.settings.mic_enabled
        && state.settings.mic_failure_policy == "required";
    if mic_required && !state.mic_path_ready {
        if capture_warming {
            return Some(SaveBlocker {
                code: "audio_warming_up",
                message: "Microphone path is starting. Try again in a moment.".to_string(),
                action: None,
                retryable: true,
            });
        }
        return Some(SaveBlocker {
            code: "mic_required_unavailable",
            message: "Microphone required but unavailable.".to_string(),
            action: Some(
                "Grant microphone permission, choose a microphone device, or set mic policy to best_effort."
                    .to_string(),
            ),
            retryable: false,
        });
    }

    None
}

pub(super) fn is_audio_path_blocker_code(code: &str) -> bool {
    match code {
        "audio_warming_up" | "system_audio_unavailable" | "mic_required_unavailable" => true,
        _ => false,
    }
}

pub(super) fn segment_stall_threshold_ms(segment_time_ms: u16) -> u64 {
    (u64::from(segment_time_ms) * 5).clamp(NO_SEGMENTS_MIN_STALL_MS, NO_SEGMENTS_MAX_STALL_MS)
}

pub(super) fn should_apply_startup_bootstrap(
    settings: &SettingsDto,
    current_profile_index: usize,
) -> bool {
    settings.performance_guard_enabled
        && settings.performance_guard_level == "balanced"
        && settings.quality_policy == "adaptive_recover"
        && settings.quality_preference == "prefer_smoothness"
        && current_profile_index == 0
        && settings.video_resolution >= 1080
        && settings.fps > 30
}

pub(super) fn required_overload_signal_count(quality_preference: &str) -> u8 {
    if quality_preference == "prefer_quality" {
        3
    } else {
        2
    }
}

pub(super) fn is_retryable_operator_health_blocker_code(code: &str) -> bool {
    matches!(code, "engine_starting" | "audio_warming_up")
}

pub(super) fn derive_mic_recovery_state(
    settings: &SettingsDto,
    active_audio_mode: &str,
    mic_path_ready: bool,
    runtime_state: Option<&str>,
) -> String {
    if !settings.mic_enabled || settings.audio_mode != "system_plus_mic" {
        return "ok".to_string();
    }
    if settings.mic_failure_policy == "required" && !mic_path_ready {
        return "blocked_required".to_string();
    }
    if active_audio_mode == "system_only" {
        return "fallback_system_only".to_string();
    }
    if matches!(runtime_state, Some("retrying")) || !mic_path_ready {
        return "retrying".to_string();
    }
    "ok".to_string()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_operator_health_state(
    lifecycle_state: LifecycleState,
    capture_health: CaptureHealthDto,
    audio_health: AudioHealthDto,
    arm_blocker_code: Option<&str>,
    arm_blocker: Option<&str>,
    guard_state: &str,
    save_ready: bool,
    effective_video_resolution: u16,
    requested_video_resolution: u16,
    effective_fps: u16,
    requested_fps: u16,
    playback_stability: &str,
    mic_recovery_state: &str,
    last_error: Option<&str>,
) -> (String, String) {
    if let Some(code) = arm_blocker_code {
        if is_retryable_operator_health_blocker_code(code) {
            let message = match code {
                "engine_starting" => "Replay engine is starting. Try again in a moment.",
                "audio_warming_up" => {
                    arm_blocker.unwrap_or("System audio path is starting. Try again in a moment.")
                }
                _ => arm_blocker
                    .unwrap_or("Capture is starting and building a stable replay buffer."),
            };
            return ("warming_up".to_string(), message.to_string());
        }
        if code == "busy" {
            return (
                "normal".to_string(),
                "Replay capture is stable.".to_string(),
            );
        }
        let message = match code {
            "permission_required" => {
                "Screen Recording permission is required before capture can start.".to_string()
            }
            "output_dir_permission_required" => {
                "Downloads permission is required before clips can be saved.".to_string()
            }
            "capture_paused" => arm_blocker
                .unwrap_or("Capture paused. Rewinder is not recording in the background.")
                .to_string(),
            "user_stopped_sharing" => {
                "Capture stopped from macOS controls. Rewinder is not recording.".to_string()
            }
            _ => arm_blocker
                .unwrap_or("Replay is blocked until the current issue is resolved.")
                .to_string(),
        };
        return ("blocked".to_string(), message);
    }

    if matches!(
        lifecycle_state,
        LifecycleState::Booting | LifecycleState::PermissionRequired
    ) || matches!(
        capture_health,
        CaptureHealthDto::Starting | CaptureHealthDto::Restarting
    ) || !save_ready
    {
        return (
            "warming_up".to_string(),
            "Capture is starting and building a stable replay buffer.".to_string(),
        );
    }

    if guard_state == "protecting"
        || effective_video_resolution < requested_video_resolution
        || effective_fps < requested_fps
    {
        return (
            "protecting".to_string(),
            "Capture protection is active. Rewinder reduced quality to keep replay stable."
                .to_string(),
        );
    }

    if mic_recovery_state == "retrying" {
        return (
            "attention".to_string(),
            "Microphone unavailable; continuing replay and retrying mic.".to_string(),
        );
    }

    if mic_recovery_state == "fallback_system_only" {
        return (
            "attention".to_string(),
            "Microphone unavailable; continuing replay with system audio only.".to_string(),
        );
    }

    if playback_stability == "drifting"
        || matches!(capture_health, CaptureHealthDto::Degraded)
        || !matches!(audio_health, AudioHealthDto::Ok)
    {
        return (
            "attention".to_string(),
            last_error
                .unwrap_or("Capture is running, but some diagnostics need attention.")
                .to_string(),
        );
    }

    (
        "normal".to_string(),
        "Replay capture is stable.".to_string(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_capture_load_state(
    capture_speed_x: Option<f32>,
    playback_realtime_x: Option<f32>,
    queue_starvation_detected: bool,
    effective_output_fps: Option<f32>,
    previous: &str,
    effective_video_resolution: u16,
    requested_video_resolution: u16,
    effective_fps: u16,
    requested_fps: u16,
) -> String {
    if queue_starvation_detected {
        return "stressed".to_string();
    }
    if playback_realtime_x
        .map(|realtime| !(0.97..=1.03).contains(&realtime))
        .unwrap_or(false)
    {
        return "stressed".to_string();
    }
    if capture_speed_x.map(|speed| speed < 0.95).unwrap_or(false) {
        return "stressed".to_string();
    }
    if effective_output_fps
        .map(|fps| fps < f32::from(requested_fps.max(1)) * OVERLOAD_OUTPUT_FPS_RATIO_THRESHOLD)
        .unwrap_or(false)
    {
        return "stressed".to_string();
    }
    if effective_video_resolution < requested_video_resolution || effective_fps < requested_fps {
        return "recovering".to_string();
    }
    if previous == "recovering"
        && playback_realtime_x
            .map(|realtime| realtime < 1.0)
            .unwrap_or(capture_speed_x.map(|speed| speed < 1.0).unwrap_or(true))
    {
        return "recovering".to_string();
    }
    "normal".to_string()
}

pub(super) fn wait_for_stop(stop: &AtomicBool, duration: Duration) -> bool {
    let step = Duration::from_millis(200);
    let mut waited = Duration::ZERO;

    while waited < duration {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        let remaining = duration.saturating_sub(waited);
        let sleep_for = if remaining < step { remaining } else { step };
        thread::sleep(sleep_for);
        waited += sleep_for;
    }

    stop.load(Ordering::Relaxed)
}
