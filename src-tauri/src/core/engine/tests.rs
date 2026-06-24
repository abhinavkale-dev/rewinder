use super::profile::evaluate_profile_guard_signals;
use super::{
    battery_floor_index, capture_stack_rss_delta_hard_budget_mb,
    capture_stack_rss_delta_soft_budget_mb, classify_capture_failure, derive_capture_load_state,
    derive_mic_recovery_state, derive_operator_health_state, effective_profile_for_index,
    parse_power_source, parse_ps_rss_cpu_totals, required_overload_signal_count, save_blocker,
    segment_stall_threshold_ms, should_apply_startup_bootstrap, Engine, PendingSaveEnqueueOutcome,
    PendingSaveReason, AUDIO_WARMUP_MIN_DEFER_TTL_MS, STARTUP_INTERRUPT_MAX_RETRIES,
};
use crate::core::state::{
    CaptureHealthDto, ClipperState, LifecycleState, PermissionStateDto, TriggerSourceDto,
};
use crate::settings::SettingsDto;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

fn sample_permission() -> PermissionStateDto {
    PermissionStateDto {
        screen_recording_granted: true,
        system_audio_granted: true,
        output_dir_writable: true,
        output_dir_permission_error: None,
        reason: None,
    }
}

#[test]
fn save_blocker_none_when_armed() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Armed;
    state.capture_health = CaptureHealthDto::Running;
    state.system_audio_path_ready = true;
    assert!(save_blocker(&state).is_none());
}

#[test]
fn save_blocker_disabled() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Disabled;
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "disabled");
    assert_eq!(
        blocker.message,
        "Replay is disabled. Enable replay to save clips."
    );
}

#[test]
fn save_blocker_permission_required_uses_reason() {
    let mut permission = sample_permission();
    permission.reason = Some("Screen Recording permission missing".to_string());
    let mut state = ClipperState::new(SettingsDto::default(), permission);
    state.lifecycle_state = LifecycleState::PermissionRequired;
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "permission_required");
    assert_eq!(blocker.message, "Screen Recording permission missing");
}

#[test]
fn save_blocker_output_dir_permission_required() {
    let mut permission = sample_permission();
    permission.output_dir_writable = false;
    permission.output_dir_permission_error = Some(
        "Downloads folder access is denied for Rewinder (/Users/test/Downloads/Rewinder)."
            .to_string(),
    );
    let state = ClipperState::new(SettingsDto::default(), permission);
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "output_dir_permission_required");
    assert!(blocker
        .message
        .contains("Downloads folder access is denied"));
}

#[test]
fn save_blocker_busy_when_saving() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Armed;
    state.is_saving = true;
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "busy");
    assert!(blocker.retryable);
}

#[test]
fn save_blocker_system_audio_warming_when_starting() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Armed;
    state.capture_health = CaptureHealthDto::Starting;
    state.system_audio_path_ready = false;
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "audio_warming_up");
}

#[test]
fn save_blocker_system_audio_required_when_running_and_not_ready() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Armed;
    state.capture_health = CaptureHealthDto::Running;
    state.system_audio_path_ready = false;
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "system_audio_unavailable");
}

#[test]
fn save_blocker_mic_required_unavailable_when_running() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Armed;
    state.capture_health = CaptureHealthDto::Running;
    state.system_audio_path_ready = true;
    state.settings.audio_mode = "system_plus_mic".to_string();
    state.settings.mic_enabled = true;
    state.settings.mic_failure_policy = "required".to_string();
    state.mic_path_ready = false;
    let blocker = save_blocker(&state).expect("expected blocker");
    assert_eq!(blocker.code, "mic_required_unavailable");
}

#[test]
fn mic_recovery_state_reports_retrying_for_best_effort_mic_loss() {
    let mut settings = SettingsDto::default();
    settings.audio_mode = "system_plus_mic".to_string();
    settings.mic_enabled = true;
    settings.mic_failure_policy = "best_effort".to_string();
    assert_eq!(
        derive_mic_recovery_state(&settings, "system_plus_mic", false, Some("retrying")),
        "retrying"
    );
}

#[test]
fn mic_recovery_state_reports_blocked_for_required_mic_loss() {
    let mut settings = SettingsDto::default();
    settings.audio_mode = "system_plus_mic".to_string();
    settings.mic_enabled = true;
    settings.mic_failure_policy = "required".to_string();
    assert_eq!(
        derive_mic_recovery_state(&settings, "system_plus_mic", false, None),
        "blocked_required"
    );
}

#[test]
fn operator_health_uses_retrying_mic_message() {
    let (state, message) = derive_operator_health_state(
        LifecycleState::Armed,
        CaptureHealthDto::Running,
        crate::core::state::AudioHealthDto::Degraded,
        None,
        None,
        "monitoring",
        true,
        1080,
        1080,
        60,
        60,
        "stable",
        "retrying",
        Some("ignored"),
    );
    assert_eq!(state, "attention");
    assert_eq!(
        message,
        "Microphone unavailable; continuing replay and retrying mic."
    );
}

#[test]
fn save_blocker_none_when_system_audio_required_and_ready() {
    let mut state = ClipperState::new(SettingsDto::default(), sample_permission());
    state.lifecycle_state = LifecycleState::Armed;
    state.capture_health = CaptureHealthDto::Running;
    state.system_audio_path_ready = true;
    state.audio_health = crate::core::state::AudioHealthDto::Ok;
    state.active_audio_mode = "system_only".to_string();
    assert!(save_blocker(&state).is_none());
}

#[test]
fn save_blocker_with_runtime_returns_capture_paused() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        *engine.capture_paused_by_user.lock() = true;
        *engine.capture_pause_reason.lock() = Some(
            "Capture paused from macOS screen recording controls. Click Resume Capture."
                .to_string(),
        );
    }
    let mut state = engine.state.lock().clone();
    state.lifecycle_state = LifecycleState::Armed;
    state.capture_health = CaptureHealthDto::Running;
    state.system_audio_path_ready = true;
    let blocker = engine
        .save_blocker_with_runtime(&state)
        .expect("expected blocker");
    assert_eq!(blocker.code, "capture_paused");
    assert!(!blocker.retryable);
}

#[test]
fn save_blocker_with_runtime_returns_user_stopped_sharing_when_disarmed() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        *engine.user_stop_disarmed_reason.lock() =
            Some("Screen recording was interrupted. Click Restart Capture to resume.".to_string());
    }
    let mut state = engine.state.lock().clone();
    state.settings.replay_enabled = false;
    state.lifecycle_state = LifecycleState::Disabled;
    let blocker = engine
        .save_blocker_with_runtime(&state)
        .expect("expected blocker");
    assert_eq!(blocker.code, "user_stopped_sharing");
    assert!(!blocker.retryable);
}

#[test]
fn shutdown_for_app_exit_disables_replay_and_clears_capture_runtime_state() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        let mut state = engine.state.lock();
        state.settings.replay_enabled = true;
        state.lifecycle_state = LifecycleState::Armed;
        state.capture_health = CaptureHealthDto::Running;
        state.system_audio_path_ready = true;
        state.mic_path_ready = true;
        state.mic_frames_seen = true;
        state.capture_owner_pid = Some(4242);
        state.audio_path_ready = true;
        state.first_audio_frame_seen = true;
        state.save_ready = true;
    }

    engine.shutdown_for_app_exit("unit_test_shutdown");

    let state = engine.state.lock().clone();
    assert!(!state.settings.replay_enabled);
    assert_eq!(state.lifecycle_state, LifecycleState::Disabled);
    assert_eq!(state.capture_health, CaptureHealthDto::Stopped);
    assert!(!state.system_audio_path_ready);
    assert!(!state.mic_path_ready);
    assert!(!state.mic_frames_seen);
    assert!(state.capture_owner_pid.is_none());
    assert!(!state.audio_path_ready);
    assert!(!state.first_audio_frame_seen);
    assert!(!state.save_ready);
    assert_eq!(
        state.last_error.as_deref(),
        Some("App exiting (unit_test_shutdown).")
    );
}

#[test]
fn classify_capture_timeout_sets_specific_code() {
    let (code, action) = classify_capture_failure(
            "capture_start_timeout: ... guidance: no first video frame marker; helper startup likely stalled",
        );
    assert_eq!(code, "capture_start_timeout");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("No frames reached"));
}

#[test]
fn classify_mixed_mic_pipe_startup_stall_sets_specific_code() {
    let (code, action) = classify_capture_failure(
        "capture_start_timeout: reason_code=mic_pipe_startup_stalled ScreenCaptureKit pipeline produced no stable segments within 8s (mode=system_plus_mic). guidance: first video and system audio seen but no microphone path; mixed audio pipe startup likely stalled",
    );
    assert_eq!(code, "mic_pipe_startup_stalled");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("retry mixed capture"));
}

#[test]
fn classify_mixed_mic_first_frame_stall_sets_specific_code() {
    let (code, action) = classify_capture_failure(
        "capture_start_timeout: reason_code=mic_first_frame_startup_stalled ScreenCaptureKit pipeline produced no stable segments within 8s (mode=system_plus_mic). guidance: microphone backend initialized but no first microphone frame reached ffmpeg; mixed mic startup likely stalled",
    );
    assert_eq!(code, "mic_first_frame_startup_stalled");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("retry mixed capture"));
}

#[test]
fn classify_overload_sets_specific_code() {
    let (code, _) = classify_capture_failure("Capture overloaded; reducing quality profile.");
    assert_eq!(code, "capture_overloaded");
}

#[test]
fn classify_warmup_sets_not_ready_code() {
    let (code, _) = classify_capture_failure("Replay buffer warming up (500ms segments).");
    assert_eq!(code, "capture_not_ready");
}

#[test]
fn classify_user_stop_sets_user_stopped_sharing_code() {
    let (code, action) = classify_capture_failure(
            "user_stopped_sharing: ScreenCaptureKit capture was stopped by macOS screen-recording controls. log: stream started | first video frame delivered | ScreenCaptureKit stopped with error: Error Domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain Code=-3805",
        );
    assert_eq!(code, "user_stopped_sharing");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("Restart Capture"));
}

#[test]
fn classify_startup_interruption_sets_capture_start_interrupted_code() {
    let (code, action) = classify_capture_failure(
            "capture_start_interrupted: ScreenCaptureKit startup interrupted (status exit status: 70, mode=system_only). log: stream start requested | stream started | phase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3805",
        );
    assert_eq!(code, "capture_start_interrupted");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("retry automatically"));
}

#[test]
fn classify_userstopped_3817_maps_to_user_stopped_sharing() {
    let (code, action) = classify_capture_failure(
        "ScreenCaptureKit helper exited unexpectedly. log: stream started | phase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3817 | phase: stream_stop_user_intent code=-3817",
    );
    assert_eq!(code, "user_stopped_sharing");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("Restart Capture"));
}

#[test]
fn classify_exit_73_stream_stop_prefers_user_stopped_sharing_over_startup_retry() {
    let (code, action) = classify_capture_failure(
        "capture_start_timeout: stream start requested | stream started | phase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3805 | phase: stream_stop_classified interrupted=true exit_code=73",
    );
    assert_eq!(code, "user_stopped_sharing");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("Restart Capture"));
}

#[test]
fn classify_capture_owner_exists_sets_specific_code() {
    let (code, action) = classify_capture_failure(
        "capture_owner_exists: Another Rewinder instance is already capturing. owner_pid=1234",
    );
    assert_eq!(code, "capture_owner_exists");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("Another Rewinder instance is already capturing"));
}

#[test]
fn classify_permission_denied_wins_over_interruption_markers() {
    let (code, action) = classify_capture_failure(
            "permission_required: screen recording permission is not granted. phase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3805",
        );
    assert_eq!(code, "permission_required");
    assert!(action
        .as_deref()
        .unwrap_or_default()
        .contains("Screen Recording permission"));
}

#[test]
fn startup_interruption_retry_policy_is_bounded() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    let now = Instant::now();

    let first = engine.next_startup_interrupt_retry(now);
    assert_eq!(first, Some((1, Duration::from_secs(1))));

    let second = engine.next_startup_interrupt_retry(now);
    assert_eq!(second, Some((2, Duration::from_secs(2))));

    let exhausted = engine.next_startup_interrupt_retry(now);
    assert_eq!(exhausted, None);

    assert_eq!(STARTUP_INTERRUPT_MAX_RETRIES, 2);
}

#[test]
fn restart_reason_is_suppressed_while_capture_is_paused() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        let mut state = engine.state.lock();
        state.lifecycle_state = LifecycleState::Armed;
    }
    *engine.capture_paused_by_user.lock() = true;
    assert!(engine.restart_reason_if_needed().is_none());
}

#[test]
fn segment_stall_threshold_uses_balanced_clamp() {
    assert_eq!(segment_stall_threshold_ms(500), 2_500);
    assert_eq!(segment_stall_threshold_ms(250), 2_500);
    assert_eq!(segment_stall_threshold_ms(1_500), 6_000);
    assert_eq!(segment_stall_threshold_ms(2_000), 6_000);
}

#[test]
fn derive_capture_load_state_flags_stress_and_recovery() {
    assert_eq!(
        derive_capture_load_state(
            Some(0.80),
            Some(0.95),
            false,
            Some(58.0),
            "normal",
            1080,
            1080,
            60,
            60
        ),
        "stressed"
    );
    assert_eq!(
        derive_capture_load_state(
            Some(1.10),
            Some(1.01),
            false,
            Some(60.0),
            "normal",
            720,
            1080,
            30,
            60
        ),
        "recovering"
    );
    assert_eq!(
        derive_capture_load_state(
            Some(1.10),
            Some(1.01),
            false,
            Some(60.0),
            "recovering",
            1080,
            1080,
            60,
            60
        ),
        "normal"
    );
    assert_eq!(
        derive_capture_load_state(
            Some(1.20),
            Some(1.08),
            false,
            Some(40.0),
            "normal",
            1080,
            1080,
            60,
            60
        ),
        "stressed"
    );
}

#[test]
fn runtime_profile_step_down_order_is_locked() {
    let settings = SettingsDto {
        video_resolution: 1080,
        fps: 60,
        video_bitrate_kbps: 10_000,
        ..SettingsDto::default()
    };

    let p0 = effective_profile_for_index(&settings, 0);
    let p1 = effective_profile_for_index(&settings, 1);
    let p2 = effective_profile_for_index(&settings, 2);
    let p3 = effective_profile_for_index(&settings, 3);
    let p4 = effective_profile_for_index(&settings, 4);

    assert_eq!((p0.video_resolution, p0.fps), (1080, 60));
    assert_eq!((p1.video_resolution, p1.fps), (1080, 30));
    assert_eq!((p2.video_resolution, p2.fps), (1080, 30));
    assert_eq!((p3.video_resolution, p3.fps), (720, 30));
    assert_eq!((p4.video_resolution, p4.fps), (720, 30));
}

#[test]
fn startup_bootstrap_respects_quality_preference() {
    let mut settings = SettingsDto {
        quality_policy: "adaptive_recover".to_string(),
        performance_guard_enabled: true,
        performance_guard_level: "balanced".to_string(),
        video_resolution: 1080,
        fps: 60,
        ..SettingsDto::default()
    };

    settings.quality_preference = "prefer_quality".to_string();
    assert!(!should_apply_startup_bootstrap(&settings, 0));

    settings.quality_preference = "prefer_smoothness".to_string();
    assert!(should_apply_startup_bootstrap(&settings, 0));
}

#[test]
fn overload_signal_requirement_depends_on_quality_preference() {
    assert_eq!(required_overload_signal_count("prefer_quality"), 3);
    assert_eq!(required_overload_signal_count("prefer_smoothness"), 2);
    assert_eq!(required_overload_signal_count("unknown"), 2);
}

#[test]
fn operator_health_treats_audio_warmup_as_warming_up() {
    let (state, message) = derive_operator_health_state(
        LifecycleState::Armed,
        CaptureHealthDto::Starting,
        crate::core::state::AudioHealthDto::Degraded,
        Some("audio_warming_up"),
        Some("System audio path is starting. Try again in a moment."),
        "monitoring",
        false,
        1080,
        1080,
        60,
        60,
        "steady",
        "ok",
        Some("Capture pipeline missing; attempting restart."),
    );

    assert_eq!(state, "warming_up");
    assert_eq!(
        message,
        "System audio path is starting. Try again in a moment."
    );
}

#[test]
fn operator_health_treats_engine_starting_as_warming_up() {
    let (state, message) = derive_operator_health_state(
        LifecycleState::Booting,
        CaptureHealthDto::Starting,
        crate::core::state::AudioHealthDto::Unavailable,
        Some("engine_starting"),
        Some("Replay engine is starting. Try again in a moment."),
        "idle",
        false,
        1080,
        1080,
        60,
        60,
        "steady",
        "ok",
        None,
    );

    assert_eq!(state, "warming_up");
    assert_eq!(message, "Replay engine is starting. Try again in a moment.");
}

#[test]
fn operator_health_treats_capture_paused_as_blocked() {
    let (state, message) = derive_operator_health_state(
        LifecycleState::Armed,
        CaptureHealthDto::Stopped,
        crate::core::state::AudioHealthDto::Unavailable,
        Some("capture_paused"),
        Some("Capture paused. Rewinder is not recording in the background."),
        "idle",
        false,
        1080,
        1080,
        60,
        60,
        "steady",
        "ok",
        None,
    );

    assert_eq!(state, "blocked");
    assert_eq!(
        message,
        "Capture paused. Rewinder is not recording in the background."
    );
}

#[test]
fn operator_health_treats_user_stopped_sharing_as_blocked() {
    let (state, message) = derive_operator_health_state(
        LifecycleState::Disabled,
        CaptureHealthDto::Stopped,
        crate::core::state::AudioHealthDto::Unavailable,
        Some("user_stopped_sharing"),
        Some("Capture stopped from macOS controls. Rewinder is not recording."),
        "idle",
        false,
        1080,
        1080,
        60,
        60,
        "steady",
        "ok",
        None,
    );

    assert_eq!(state, "blocked");
    assert_eq!(
        message,
        "Capture stopped from macOS controls. Rewinder is not recording."
    );
}

#[test]
fn operator_health_treats_audio_device_failures_as_blocked() {
    for code in ["system_audio_unavailable", "mic_required_unavailable"] {
        let (state, message) = derive_operator_health_state(
            LifecycleState::Armed,
            CaptureHealthDto::Running,
            crate::core::state::AudioHealthDto::Unavailable,
            Some(code),
            Some("Audio device unavailable."),
            "monitoring",
            false,
            1080,
            1080,
            60,
            60,
            "steady",
            "ok",
            None,
        );

        assert_eq!(state, "blocked");
        assert_eq!(message, "Audio device unavailable.");
    }
}

#[test]
fn recovery_uses_current_effective_profile_fps_not_requested_fps() {
    let signals = evaluate_profile_guard_signals(
        30,
        Some(29.8),
        Some(1.01),
        Some(1.01),
        0,
        0,
        false,
        false,
        None,
        None,
        "prefer_quality",
    );

    assert!(!signals.output_fps_under_target);
    assert!(signals.recovery_signal);
}

#[test]
fn startup_initial_fallback_is_suppressed_for_default_profile_without_hard_pressure() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());

    assert!(engine.should_suppress_initial_startup_fallback(true, false, false));
}

#[test]
fn startup_initial_fallback_is_allowed_under_hard_pressure() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());

    assert!(!engine.should_suppress_initial_startup_fallback(true, false, true));
}

#[test]
fn startup_initial_fallback_is_not_suppressed_after_already_degraded_profile() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    *engine.runtime_profile_index.lock() = 1;

    assert!(!engine.should_suppress_initial_startup_fallback(true, false, false));
}

#[test]
fn single_overflow_packet_does_not_count_as_overload_signal() {
    let signals = evaluate_profile_guard_signals(
        60,
        Some(60.0),
        Some(1.0),
        Some(1.0),
        0,
        1,
        false,
        false,
        None,
        None,
        "prefer_quality",
    );

    assert!(!signals.overflow_trigger);
    assert_eq!(signals.overload_signal_count, 0);
}

#[test]
fn small_transient_noise_does_not_block_recovery() {
    let signals = evaluate_profile_guard_signals(
        30,
        Some(29.5),
        Some(0.98),
        Some(0.99),
        3,
        1,
        false,
        false,
        None,
        None,
        "prefer_quality",
    );

    assert!(signals.recovery_signal);
}

#[test]
fn prefer_quality_requires_three_sustained_signals() {
    let signals = evaluate_profile_guard_signals(
        60,
        Some(40.0),
        Some(0.94),
        Some(0.94),
        20,
        4,
        false,
        false,
        None,
        None,
        "prefer_quality",
    );

    assert_eq!(signals.overload_signal_count, 4);
    assert_eq!(signals.overload_signal_requirement, 3);
    assert_eq!(signals.sustained_overload_signal_requirement, 3);
}

#[test]
fn system_memory_pressure_warning_blocks_recovery_without_hard_stepdown() {
    let signals = evaluate_profile_guard_signals(
        60,
        Some(60.0),
        Some(1.0),
        Some(1.0),
        0,
        0,
        false,
        false,
        Some("warning"),
        None,
        "prefer_quality",
    );

    assert!(signals.system_memory_pressure_warning);
    assert!(!signals.system_memory_pressure_critical);
    assert!(!signals.recovery_signal);
    assert_eq!(signals.overload_signal_count, 0);
}

#[test]
fn thermal_critical_is_reported_as_hard_guard_reason() {
    let signals = evaluate_profile_guard_signals(
        60,
        Some(60.0),
        Some(1.0),
        Some(1.0),
        0,
        0,
        false,
        false,
        None,
        Some("critical"),
        "prefer_quality",
    );

    assert!(signals.thermal_critical);
    assert!(!signals.recovery_signal);
}

#[test]
fn current_save_modes_do_not_schedule_background_smooth_postprocess() {
    let mut settings = SettingsDto::default();
    for mode in ["instant_mp4", "fast", "smooth", "adaptive"] {
        settings.save_path_mode = mode.to_string();
        assert!(!Engine::should_schedule_smooth_postprocess(&settings));
    }
}

#[test]
fn fast_integrity_check_is_only_scheduled_for_fast_outputs() {
    assert!(Engine::should_schedule_fast_integrity_check(Some(
        "instant_mp4"
    )));
    assert!(Engine::should_schedule_fast_integrity_check(Some("fast")));
    assert!(Engine::should_schedule_fast_integrity_check(Some(
        "fallback_fast"
    )));
    assert!(!Engine::should_schedule_fast_integrity_check(Some(
        "smooth"
    )));
    assert!(!Engine::should_schedule_fast_integrity_check(Some(
        "adaptive"
    )));
    assert!(!Engine::should_schedule_fast_integrity_check(None));
}

#[test]
fn capture_stack_rss_budgets_follow_perf_profile_targets() {
    assert_eq!(capture_stack_rss_delta_soft_budget_mb(1080, 60), 300);
    assert_eq!(capture_stack_rss_delta_soft_budget_mb(1080, 30), 200);
    assert_eq!(capture_stack_rss_delta_hard_budget_mb(1080, 60), 440);
    assert_eq!(capture_stack_rss_delta_hard_budget_mb(1080, 30), 300);
}

#[test]
fn parse_ps_rss_cpu_totals_sums_multiple_rows() {
    let input = "123456 12.5\n654321 3.5\n";
    let (rss_mb, cpu_pct) = parse_ps_rss_cpu_totals(input);
    assert_eq!(rss_mb, Some((123_456_u64 + 654_321_u64) as u32 / 1024));
    assert_eq!(cpu_pct, Some(16.0));
}

#[test]
fn hard_stepdown_advances_multiple_profiles_in_one_transition() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        let mut state = engine.state.lock();
        state.settings.video_resolution = 1080;
        state.settings.fps = 60;
        state.settings.video_bitrate_kbps = 10_000;
        state.settings.quality_policy = "adaptive_recover".to_string();
    }
    let settings = engine.state.lock().settings.clone();
    let from = effective_profile_for_index(&settings, 0).label();
    let to = effective_profile_for_index(&settings, 2).label();
    let transition = engine
        .advance_runtime_profile_for_overload_steps(2)
        .expect("expected multi-step transition");
    assert_eq!(transition.0, from);
    assert_eq!(transition.1, to);
    assert_eq!(*engine.runtime_profile_index.lock(), 2);
}

#[test]
fn ac_plug_in_restores_battery_bound_profile_in_one_action() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        let mut state = engine.state.lock();
        state.settings.video_resolution = 1080;
        state.settings.fps = 60;
        state.settings.quality_policy = "adaptive_recover".to_string();
    }
    let raised = engine
        .set_runtime_profile_at_least_floor(1)
        .expect("battery floor should raise the profile");
    *engine.battery_floor_engaged.lock() = true;
    assert_eq!(*engine.runtime_profile_index.lock(), 1);

    let restored = engine
        .lower_runtime_profile_to_floor(0)
        .expect("AC plug-in should restore the requested profile");
    assert_eq!(*engine.runtime_profile_index.lock(), 0);
    assert_eq!(restored.0, raised.1);
    assert_eq!(restored.1, raised.0);
}

#[test]
fn lower_runtime_profile_to_floor_noop_at_or_below_floor() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    assert!(engine.lower_runtime_profile_to_floor(0).is_none());
    assert_eq!(*engine.runtime_profile_index.lock(), 0);
}

#[test]
fn lower_runtime_profile_to_floor_stops_at_battery_floor_not_zero() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    {
        let mut state = engine.state.lock();
        state.settings.video_resolution = 1080;
        state.settings.fps = 60;
        state.settings.video_bitrate_kbps = 10_000;
        state.settings.quality_policy = "adaptive_recover".to_string();
    }
    engine
        .advance_runtime_profile_for_overload_steps(3)
        .expect("overload should advance the profile");
    assert_eq!(*engine.runtime_profile_index.lock(), 3);
    engine
        .lower_runtime_profile_to_floor(1)
        .expect("profile should lower to the floor");
    assert_eq!(*engine.runtime_profile_index.lock(), 1);
}

#[test]
fn battery_floor_engaged_defaults_to_false() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    assert!(!*engine.battery_floor_engaged.lock());
}

#[test]
fn capture_stack_pid_collection_reads_runtime_pid_files_only() {
    let mut settings = SettingsDto::default();
    let base = std::env::current_dir()
        .expect("cwd should resolve")
        .join("research")
        .join(format!("rewinder-test-{}", std::process::id()));
    settings.output_dir = base.to_string_lossy().to_string();
    let engine = Engine::new(settings, sample_permission());
    let live_dir = base.join(".rewinder-live");
    fs::create_dir_all(&live_dir).expect("live dir should be created");
    fs::write(live_dir.join("ffmpeg-capture.pid"), "123\n").expect("ffmpeg pid write");
    fs::write(live_dir.join("sck-capture.pid"), "456\n").expect("helper pid write");

    let pids = engine.capture_stack_pids();
    assert!(pids.contains(&std::process::id()));
    assert!(pids.contains(&123));
    assert!(pids.contains(&456));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn enqueue_or_replace_pending_save_with_ttl_dedupes_to_single_slot() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    let anchor_time = SystemTime::now();
    let outcome = engine.enqueue_or_replace_pending_save_with_ttl(
        TriggerSourceDto::Hotkey,
        50,
        anchor_time,
        PendingSaveReason::Retryable,
        30,
    );
    assert!(matches!(outcome, PendingSaveEnqueueOutcome::QueuedNew));
    let outcome = engine.enqueue_or_replace_pending_save_with_ttl(
        TriggerSourceDto::Manual,
        50,
        SystemTime::now(),
        PendingSaveReason::Retryable,
        30,
    );
    assert!(matches!(
        outcome,
        PendingSaveEnqueueOutcome::ReplacedExisting { .. }
    ));
}

#[test]
fn enqueue_or_replace_pending_save_updates_expiry_and_anchor() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    let initial_anchor = SystemTime::now();
    let first = engine.enqueue_or_replace_pending_save_with_ttl(
        TriggerSourceDto::Hotkey,
        50,
        initial_anchor,
        PendingSaveReason::Retryable,
        30,
    );
    assert!(matches!(first, PendingSaveEnqueueOutcome::QueuedNew));
    let first_state = engine
        .pending_save
        .lock()
        .as_ref()
        .cloned()
        .expect("pending save should exist");
    std::thread::sleep(Duration::from_millis(2));
    let next_anchor = SystemTime::now();
    let second = engine.enqueue_or_replace_pending_save_with_ttl(
        TriggerSourceDto::Manual,
        AUDIO_WARMUP_MIN_DEFER_TTL_MS,
        next_anchor,
        PendingSaveReason::AudioWarmup,
        30,
    );
    assert!(matches!(
        second,
        PendingSaveEnqueueOutcome::ReplacedExisting { .. }
    ));
    let second_state = engine
        .pending_save
        .lock()
        .as_ref()
        .cloned()
        .expect("pending save should still exist");
    assert!(second_state.expires_at > first_state.expires_at);
    assert!(matches!(second_state.source, TriggerSourceDto::Manual));
    assert_eq!(second_state.anchor_time, next_anchor);
}

#[test]
fn smooth_postprocess_queue_is_capped_to_latest_single_job() {
    let engine = Engine::new(SettingsDto::default(), sample_permission());
    let settings = SettingsDto::default();
    engine.enqueue_smooth_postprocess(
        1,
        TriggerSourceDto::Hotkey,
        PathBuf::from("/tmp/a.mp4"),
        settings.clone(),
    );
    engine.enqueue_smooth_postprocess(
        2,
        TriggerSourceDto::Manual,
        PathBuf::from("/tmp/b.mp4"),
        settings,
    );

    let queue = engine.pending_smooth_jobs.lock();
    assert_eq!(queue.len(), 1);
    let latest = queue.front().expect("latest smooth job should exist");
    assert_eq!(latest.save_id, 2);
    assert_eq!(latest.clip_path, PathBuf::from("/tmp/b.mp4"));
}

fn settings_with_video(fps: u16, video_resolution: u16) -> SettingsDto {
    let mut settings = SettingsDto::default();
    settings.fps = fps;
    settings.video_resolution = video_resolution;
    settings.video_bitrate_kbps = 10_000;
    settings
}

#[test]
fn parse_power_source_reads_ac() {
    let stdout = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)\t100%; charged; 0:00 remaining present: true\n";
    assert_eq!(parse_power_source(stdout), Some("ac"));
}

#[test]
fn parse_power_source_reads_battery() {
    let stdout = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=123)\t98%; discharging; 3:21 remaining present: true\n";
    assert_eq!(parse_power_source(stdout), Some("battery"));
}

#[test]
fn parse_power_source_desktop_without_battery_reads_ac() {
    let stdout = "Now drawing from 'AC Power'\n";
    assert_eq!(parse_power_source(stdout), Some("ac"));
}

#[test]
fn parse_power_source_unknown_is_none() {
    assert_eq!(parse_power_source(""), None);
    assert_eq!(parse_power_source("unexpected pmset output"), None);
}

#[test]
fn battery_floor_index_zero_on_ac() {
    let settings = settings_with_video(60, 1080);
    assert_eq!(battery_floor_index(&settings, false, true, 30), 0);
}

#[test]
fn battery_floor_index_zero_when_guard_disabled() {
    let settings = settings_with_video(60, 1080);
    assert_eq!(battery_floor_index(&settings, true, false, 30), 0);
}

#[test]
fn battery_floor_index_caps_1080p60_to_index_one() {
    let settings = settings_with_video(60, 1080);
    assert_eq!(battery_floor_index(&settings, true, true, 30), 1);
    assert_eq!(effective_profile_for_index(&settings, 1).fps, 30);
}

#[test]
fn battery_floor_index_no_penalty_when_already_at_or_below_cap() {
    let settings = settings_with_video(30, 1080);
    assert_eq!(battery_floor_index(&settings, true, true, 30), 0);
}

#[test]
fn battery_floor_index_respects_higher_cap() {
    let settings = settings_with_video(60, 1080);
    assert_eq!(battery_floor_index(&settings, true, true, 60), 0);
}
