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
    // Degradation ladder: preserve resolution as long as possible (like OBS).
    // Drop FPS and bitrate first; resolution is the last resort.
    //   0 → full requested profile (e.g. 1080p@60fps 10Mbps)
    //   1 → keep resolution, cap FPS to 30, reduce bitrate
    //   2 → keep resolution, cap FPS to 30, reduce bitrate further
    //   3 → drop resolution to 720p, cap FPS to 30, lowest bitrate
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
    // Require sustained multi-signal overload for non-emergency degradation.
    // resource_soft_triggered alone no longer counts as a standalone step-down cause.
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

pub(super) fn is_transient_capture_error(message: &str) -> bool {
    message.starts_with("capture initialization failed:")
        || message.starts_with("capture recovery retry failed:")
}

pub(super) fn is_user_stopped_sharing_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    if lower.contains("user_stopped_sharing") {
        return true;
    }
    if lower.contains("phase: stream_inactive_watchdog_triggered") {
        return true;
    }
    let has_stream_stop_marker = lower.contains("phase: stream_stopped_error")
        || lower.contains("phase: stream_stop_details")
        || lower.contains("phase: stream_stop_classified")
        || lower.contains("screencapturekit stopped with error");
    let has_exit_73 = lower.contains("status=exit status: 73")
        || lower.contains("status exit status: 73")
        || lower.contains("exit status: 73")
        || lower.contains("exit_code=73");
    if has_stream_stop_marker && has_exit_73 {
        return true;
    }
    let has_post_start_marker = lower.contains("first video frame delivered")
        || lower.contains("phase: first_segment_closed")
        || lower.contains("phase: first_stable_segment");
    (lower.contains("scstreamerrordomain code=-3805")
        || lower.contains("application connection being interrupted"))
        && has_post_start_marker
}

pub(super) fn is_capture_start_interrupted_error(message: &str) -> bool {
    if is_user_stopped_sharing_error(message) {
        return false;
    }
    let lower = message.to_ascii_lowercase();
    if lower.contains("capture_start_interrupted") {
        return true;
    }
    let has_stream_stop_marker = lower.contains("phase: stream_stopped_error");
    let has_interruption_code = lower.contains("code=-3805")
        || lower.contains("scstreamerrordomain code=-3805")
        || lower.contains("application connection being interrupted");
    has_stream_stop_marker
        && has_interruption_code
        && !lower.contains("first video frame delivered")
}

pub(super) fn detect_capture_start_phase(log_tail: Option<&str>) -> Option<String> {
    let tail = log_tail?;
    if tail.contains("phase: first_segment_closed") || tail.contains("phase: first_stable_segment")
    {
        return Some("first_segment".to_string());
    }
    if tail.contains("first system audio frame delivered")
        || tail.contains("first microphone audio frame delivered")
        || tail.contains("phase: first_audio_path_ready")
    {
        return Some("first_audio_frame".to_string());
    }
    if tail.contains("first video frame delivered") {
        return Some("first_video_frame".to_string());
    }
    if tail.contains("stream started") {
        return Some("stream_started".to_string());
    }
    if tail.contains("stream start requested") {
        return Some("stream_start_requested".to_string());
    }
    if tail.contains("phase: helper_spawned") {
        return Some("helper_spawned".to_string());
    }
    None
}

pub(super) fn sample_process_ps_metrics(pid: u32) -> (Option<u32>, Option<f32>) {
    let output = Command::new("ps")
        .arg("-o")
        .arg("rss=,%cpu=")
        .arg("-p")
        .arg(pid.to_string())
        .output();
    let Ok(output) = output else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|value| !value.is_empty());
    let Some(line) = line else {
        return (None, None);
    };
    let mut parts = line.split_whitespace();
    let rss_mb = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|kb| kb / 1024);
    let cpu_percent = parts.next().and_then(|value| value.parse::<f32>().ok());
    (rss_mb, cpu_percent)
}

pub(super) fn sample_process_ps_metrics_for_pids(pids: &[u32]) -> (Option<u32>, Option<f32>) {
    if pids.is_empty() {
        return (None, None);
    }

    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let output = Command::new("ps")
        .arg("-o")
        .arg("rss=,%cpu=")
        .arg("-p")
        .arg(pid_list)
        .output();
    let Ok(output) = output else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ps_rss_cpu_totals(&stdout)
}

pub(super) fn parse_ps_rss_cpu_totals(stdout: &str) -> (Option<u32>, Option<f32>) {
    let mut total_rss_kb: u64 = 0;
    let mut total_cpu: f32 = 0.0;
    let mut rss_seen = false;
    let mut cpu_seen = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        if let Some(rss_part) = parts.next() {
            if let Ok(rss_kb) = rss_part.parse::<u64>() {
                total_rss_kb = total_rss_kb.saturating_add(rss_kb);
                rss_seen = true;
            }
        }
        if let Some(cpu_part) = parts.next() {
            if let Ok(cpu_pct) = cpu_part.parse::<f32>() {
                total_cpu += cpu_pct;
                cpu_seen = true;
            }
        }
    }

    (
        if rss_seen {
            Some((total_rss_kb / 1024).min(u64::from(u32::MAX)) as u32)
        } else {
            None
        },
        if cpu_seen { Some(total_cpu) } else { None },
    )
}

pub(super) fn capture_stack_rss_delta_soft_budget_mb(
    effective_video_resolution: u16,
    effective_fps: u16,
) -> u32 {
    if effective_video_resolution >= 1080 && effective_fps >= 60 {
        300
    } else if effective_video_resolution >= 1080 && effective_fps >= 30 {
        200
    } else if effective_fps >= 60 {
        240
    } else {
        180
    }
}

pub(super) fn capture_stack_rss_delta_hard_budget_mb(
    effective_video_resolution: u16,
    effective_fps: u16,
) -> u32 {
    let soft = capture_stack_rss_delta_soft_budget_mb(effective_video_resolution, effective_fps);
    soft.saturating_add(if effective_fps >= 60 { 140 } else { 100 })
}

pub(super) fn sample_thermal_state() -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("pmset").arg("-g").arg("therm").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.to_ascii_lowercase().contains("thermallevel") {
                continue;
            }
            let level = trimmed
                .split('=')
                .nth(1)
                .map(str::trim)
                .and_then(|value| value.parse::<i32>().ok())?;
            let label = match level {
                i32::MIN..=0 => "nominal",
                1 => "fair",
                2 => "serious",
                _ => "critical",
            };
            return Some(label.to_string());
        }
        if stdout
            .to_ascii_lowercase()
            .contains("no thermal warning level")
        {
            return Some("nominal".to_string());
        }
        None
    }
}

pub(super) fn classify_capture_failure(message: &str) -> (&'static str, Option<String>) {
    let lower = message.to_ascii_lowercase();
    if lower.contains("output_dir_permission_required")
        || lower.contains("failed to list segment dir: operation not permitted")
        || lower.contains("failed to read live segment directory: operation not permitted")
        || lower.contains("downloads folder access is denied for rewinder")
    {
        return (
            "output_dir_permission_required",
            Some(
                "Grant Downloads access in System Settings > Privacy & Security > Files and Folders, then retry."
                    .to_string(),
            ),
        );
    }
    if lower.contains("permission_required")
        || lower.contains("screen recording permission is not granted")
        || lower.contains("screen recording access is denied")
    {
        return (
            "permission_required",
            Some("Grant Screen Recording permission in System Settings, then retry.".to_string()),
        );
    }
    if lower.contains("capture_owner_exists")
        || lower.contains("another rewinder instance is already capturing")
    {
        return (
            "capture_owner_exists",
            Some(
                "Another Rewinder instance is already capturing. Close duplicate launches, then click Resume Capture."
                    .to_string(),
            ),
        );
    }
    if is_user_stopped_sharing_error(message) {
        return (
            "user_stopped_sharing",
            Some("Screen recording was interrupted. Click Restart Capture to resume.".to_string()),
        );
    }
    if is_capture_start_interrupted_error(message) {
        return (
            "capture_start_interrupted",
            Some(
                "ScreenCaptureKit service was interrupted during startup; Rewinder will retry automatically."
                    .to_string(),
            ),
        );
    }
    if lower.contains("mic_required_unavailable")
        || lower.contains("required microphone path")
        || lower.contains("microphone path is not ready")
    {
        return (
            "mic_start_timeout",
            Some(
                "Microphone path is unavailable. Rewinder can continue in best-effort mode or you can make mic required."
                    .to_string(),
            ),
        );
    }
    if lower.contains("audio_start_timeout") {
        return (
            "audio_start_timeout",
            Some(
                "Audio path did not start in time. Rewinder will retry with configured audio fallback."
                    .to_string(),
            ),
        );
    }
    if lower.contains("mic_pipe_startup_stalled")
        || lower.contains("mic_first_frame_startup_stalled")
    {
        return (
            if lower.contains("mic_first_frame_startup_stalled") {
                "mic_first_frame_startup_stalled"
            } else {
                "mic_pipe_startup_stalled"
            },
            Some(
                "Mixed microphone startup stalled before audio segments were sealed. Rewinder will retry mixed capture before falling back."
                    .to_string(),
            ),
        );
    }
    if lower.contains("required audio path unavailable")
        || lower.contains("audio_required_unavailable")
        || lower.contains("microphone permission denied")
        || (lower.contains("failed to start capture after trying all audio modes")
            && !lower.contains("video_only =>"))
    {
        return (
            "system_audio_unavailable",
            Some(
                "Audio is required by your fallback policy. Fix mic/system-audio permissions or choose allow_video_only."
                    .to_string(),
            ),
        );
    }
    if lower.contains("auto-fallback applied")
        || lower.contains("runtime fallback active")
        || lower.contains("profile degraded")
    {
        return (
            "profile_degraded",
            Some("Capture quality was reduced to maintain realtime.".to_string()),
        );
    }
    if lower.contains("capture overloaded") || lower.contains("capture_overloaded") {
        return (
            "capture_overloaded",
            Some(
                "Capture is overloaded; Rewinder will step down profile automatically.".to_string(),
            ),
        );
    }
    if lower.contains("warming up") || lower.contains("no stable segments") {
        return (
            "capture_not_ready",
            Some("Replay buffer is warming up. Try again in a moment.".to_string()),
        );
    }
    if lower.contains("capture_start_timeout") {
        if lower.contains("mic_pipe_startup_stalled") {
            return (
                "mic_pipe_startup_stalled",
                Some(
                    "Mixed microphone startup stalled before audio segments were sealed. Rewinder will retry mixed capture before falling back."
                        .to_string(),
                ),
            );
        }
        if lower.contains("mic_first_frame_startup_stalled") {
            return (
                "mic_first_frame_startup_stalled",
                Some(
                    "Microphone backend started but no first usable mic frame reached ffmpeg. Rewinder will retry mixed capture before falling back."
                        .to_string(),
                ),
            );
        }
        if lower.contains("mode=system_plus_mic") || lower.contains("mode=system_only") {
            return (
                "audio_start_timeout",
                Some(
                    "Audio path did not start in time. Rewinder will retry with configured audio fallback."
                        .to_string(),
                ),
            );
        }
        let action = if lower.contains("no first video frame marker")
            || lower.contains("helper startup likely stalled")
        {
            "No frames reached the pipeline. Verify Screen Recording permission and active display."
        } else if lower.contains("first video frame seen but no stable segment")
            || lower.contains("ffmpeg pipe/mux path likely stalled")
        {
            "Frames arrived but ffmpeg could not seal segments. Check ffmpeg binary and pipe startup."
        } else {
            "Capture startup timed out. Rewinder will retry automatically."
        };
        return ("capture_start_timeout", Some(action.to_string()));
    }

    (
        "capture_unavailable",
        Some("Capture failed; Rewinder will retry automatically.".to_string()),
    )
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
