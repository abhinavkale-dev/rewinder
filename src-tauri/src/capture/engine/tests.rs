use super::{
    acquire_capture_lock, build_capture_args_from_pipes, build_system_plus_mic_mix_graph,
    capture_dimensions,
    has_first_audio_frame_marker, has_first_system_audio_frame_marker, has_mic_path_ready_marker,
    has_startup_soft_ready_markers, helper_exit_indicates_user_stopped_sharing,
    is_rewinder_capture_process_command, is_startup_interrupted_log, is_user_stopped_sharing_log,
    parse_capture_lock_payload, parse_capture_process_candidates,
    parse_latest_helper_thermal_state, parse_latest_mic_attach_runtime_state,
    parse_latest_mic_backend_error, parse_latest_mic_level_dbfs, parse_latest_mic_recovery_state,
    parse_latest_mic_samples_per_sec, parse_latest_selected_microphone_name,
    parse_latest_system_memory_pressure_level, parse_latest_video_frame_drop_total,
    parse_latest_video_output_fps, parse_latest_video_queue_overflow_count, process_is_running,
    replay_continuity_gap_threshold, requested_audio_modes, segment_time_delta_for_fps,
    select_capture_process_sweep_candidates, startup_timeout_guidance, startup_timeout_reason_code,
    terminate_stale_capture_process, AudioMode, AudioStartupStrategy, CaptureEngine, CapturePipes,
    LiveQueueProfile, MicAttachRuntimeState, SegmentFile,
};
use crate::settings::SettingsDto;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{atomic::AtomicBool, Arc};
use std::thread;
use std::time::{Duration, SystemTime};

#[test]
fn detects_first_audio_marker_from_system_audio_line() {
    assert!(has_first_audio_frame_marker(
        "stream started\nfirst system audio frame delivered\n"
    ));
}

#[test]
fn detects_first_audio_marker_from_mic_line() {
    assert!(has_first_audio_frame_marker(
        "stream started\nfirst microphone audio frame delivered\n"
    ));
}

#[test]
fn detects_system_audio_ready_from_pipe_connected_marker() {
    assert!(has_first_system_audio_frame_marker(
        "stream started\nphase: system_audio_pipe_connected\n"
    ));
}

#[test]
fn does_not_detect_audio_marker_when_missing() {
    assert!(!has_first_audio_frame_marker(
        "stream started\nfirst video frame delivered\n"
    ));
}

#[test]
fn startup_soft_ready_detects_mux_open_with_video_and_audio_markers() {
    let log = "stream started\nfirst video frame delivered\nphase: system_audio_pipe_connected\n[segment @ 0x111] Opening '/tmp/seg_00000000.mp4' for writing\n";
    assert!(has_startup_soft_ready_markers(log, true));
}

#[test]
fn startup_soft_ready_requires_mux_open_marker() {
    let log = "stream started\nfirst video frame delivered\nphase: system_audio_pipe_connected\n";
    assert!(!has_startup_soft_ready_markers(log, true));
}

#[test]
fn detects_startup_interrupted_from_scstream_error_before_first_video() {
    let log = "stream start requested\nstream started\nphase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3805\nScreenCaptureKit stopped with error: Error Domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain Code=-3805 \"Failed during stream due to application connection being interrupted\"\n";
    assert!(is_startup_interrupted_log(log));
    assert!(!is_user_stopped_sharing_log(log));
}

#[test]
fn detects_user_stopped_sharing_from_scstream_interrupted_log() {
    let log = "stream started\nfirst video frame delivered\nScreenCaptureKit stopped with error: Error Domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain Code=-3805 \"Failed during stream due to application connection being interrupted\"\n";
    assert!(is_user_stopped_sharing_log(log));
    assert!(!is_startup_interrupted_log(log));
}

#[test]
fn helper_exit_73_with_stop_markers_maps_to_user_stopped_sharing() {
    let log = "stream started\nphase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3805\nphase: stream_stop_classified interrupted=true exit_code=73\n";
    assert!(helper_exit_indicates_user_stopped_sharing(Some(73), log));
}

#[test]
fn detects_user_stopped_sharing_from_userstopped_code_even_during_warmup() {
    let log = "stream start requested\nstream started\nphase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3817\n";
    assert!(is_user_stopped_sharing_log(log));
    assert!(!is_startup_interrupted_log(log));
}

#[test]
fn detects_user_stopped_sharing_from_user_intent_marker() {
    let log = "stream started\nphase: stream_stop_user_intent code=-3817\nphase: stream_stop_classified interrupted=true exit_code=73\n";
    assert!(is_user_stopped_sharing_log(log));
}

#[test]
fn detects_user_stopped_sharing_from_system_stopped_code() {
    let log = "stream started\nfirst video frame delivered\nphase: stream_stopped_error domain=com.apple.ScreenCaptureKit.SCStreamErrorDomain code=-3821\n";
    assert!(is_user_stopped_sharing_log(log));
    assert!(!is_startup_interrupted_log(log));
}

#[test]
fn helper_exit_without_stop_signature_does_not_map_to_user_stopped_sharing() {
    let log = "stream started\nsome unrelated log line\n";
    assert!(!helper_exit_indicates_user_stopped_sharing(Some(73), log));
}

#[test]
fn capture_process_classifier_matches_only_rewinder_workers() {
    assert!(is_rewinder_capture_process_command(
        "/Users/apple/Desktop/rewinder/src-tauri/bin/rewinder-sck-capture --output /tmp/video.pipe"
    ));
    assert!(is_rewinder_capture_process_command(
        "ffmpeg -i /tmp/video.pipe -i /tmp/system_audio.pipe /Users/apple/Downloads/.rewinder-live/seg_abc_%08d.mp4"
    ));
    assert!(!is_rewinder_capture_process_command(
        "ffmpeg -f avfoundation -i 1:none /Users/apple/Desktop/test.mp4"
    ));
}

#[test]
fn parse_capture_process_candidates_ignores_unrelated_processes() {
    let ps_output = " 101 /Applications/Cursor.app/Contents/MacOS/Cursor\n 202 /Users/apple/Desktop/rewinder/src-tauri/bin/rewinder-sck-capture --video /tmp/video.pipe\n 303 /opt/homebrew/bin/ffmpeg -f avfoundation -i 1:none /tmp/plain-recording.mp4\n 404 /opt/homebrew/bin/ffmpeg -i /tmp/video.pipe -i /tmp/system_audio.pipe /Users/apple/Downloads/.rewinder-live/seg_capture_%08d.mp4\n";
    let candidates = parse_capture_process_candidates(ps_output);
    assert_eq!(candidates, vec![202, 404]);
}

#[test]
fn process_sweep_candidate_selection_respects_exclusions() {
    let self_pid = std::process::id();
    let ps_output = format!(
        " {self_pid} /Users/apple/Desktop/rewinder/src-tauri/bin/rewinder-sck-capture --video /tmp/video.pipe\n 510 /Users/apple/Desktop/rewinder/src-tauri/bin/rewinder-sck-capture --video /tmp/video.pipe\n 520 /opt/homebrew/bin/ffmpeg -i /tmp/video.pipe /Users/apple/Downloads/.rewinder-live/seg_a_%08d.mp4\n"
    );
    let mut excluded = HashSet::new();
    excluded.insert(520);
    let selected = select_capture_process_sweep_candidates(&ps_output, &excluded);
    assert_eq!(selected, vec![510]);
}

#[test]
fn terminate_stale_capture_process_waits_until_process_is_gone() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|delta| delta.as_nanos())
        .unwrap_or(0);
    let pid_file = std::env::temp_dir().join(format!("rewinder-stale-helper-{unique}.pid"));
    let output = Command::new("sh")
        .args([
            "-c",
            "setsid sh -c 'trap \"\" TERM; while :; do sleep 1; done' >/dev/null 2>&1 & echo $!",
        ])
        .output()
        .expect("spawn detached stubborn process");
    let pid = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .expect("parse detached pid");
    fs::write(&pid_file, pid.to_string()).expect("write pid file");

    terminate_stale_capture_process(&pid_file);

    assert!(
        !process_is_running(pid),
        "expected stale process to be terminated"
    );
    assert!(
        !pid_file.exists(),
        "expected pid file to be removed after process exit"
    );
}

#[test]
fn system_only_fallback_policy_does_not_append_video_only_mode() {
    let mut settings = SettingsDto::default();
    settings.audio_mode = "system_only".to_string();
    settings.audio_fallback_policy = "system_only_fallback".to_string();
    let modes = requested_audio_modes(&settings, AudioStartupStrategy::SystemFirst);
    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].as_str(), "system_only");
}

#[test]
fn required_mic_policy_keeps_only_mic_mode() {
    let mut settings = SettingsDto::default();
    settings.audio_mode = "system_plus_mic".to_string();
    settings.mic_enabled = true;
    settings.mic_failure_policy = "required".to_string();
    let modes = requested_audio_modes(&settings, AudioStartupStrategy::SystemFirst);
    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].as_str(), "system_plus_mic");
}

#[test]
fn best_effort_mic_starts_with_mixed_mode_on_system_first_strategy() {
    let mut settings = SettingsDto::default();
    settings.audio_mode = "system_plus_mic".to_string();
    settings.mic_enabled = true;
    settings.mic_failure_policy = "best_effort".to_string();
    let modes = requested_audio_modes(&settings, AudioStartupStrategy::SystemFirst);
    assert_eq!(modes.len(), 2);
    assert_eq!(modes[0].as_str(), "system_plus_mic");
    assert_eq!(modes[1].as_str(), "system_only");
}

#[test]
fn best_effort_mic_prefers_mic_when_upgrade_strategy_requested() {
    let mut settings = SettingsDto::default();
    settings.audio_mode = "system_plus_mic".to_string();
    settings.mic_enabled = true;
    settings.mic_failure_policy = "best_effort".to_string();
    let modes = requested_audio_modes(&settings, AudioStartupStrategy::PreferMic);
    assert_eq!(modes.len(), 2);
    assert_eq!(modes[0].as_str(), "system_plus_mic");
    assert_eq!(modes[1].as_str(), "system_only");
}

#[test]
fn parses_latest_mic_level_marker() {
    let log = "mic_level_dbfs=-48.2\nnoise\nmic_level_dbfs=-22.7\n";
    assert_eq!(parse_latest_mic_level_dbfs(log), Some(-22.7));
}

#[test]
fn mic_level_parser_ignores_missing_or_invalid_values() {
    assert_eq!(parse_latest_mic_level_dbfs("no level"), None);
    assert_eq!(parse_latest_mic_level_dbfs("mic_level_dbfs=bad"), None);
}

#[test]
fn parses_latest_mic_samples_per_sec_marker() {
    let log = "mic_samples_per_sec=48000\nnoise\nmic_samples_per_sec=47912\n";
    assert_eq!(parse_latest_mic_samples_per_sec(log), Some(47_912));
}

#[test]
fn parses_latest_mic_attach_runtime_state_marker() {
    let log = "phase: mic_silence_filler_active\nphase: mic_live_frames_detected\n";
    assert_eq!(
        parse_latest_mic_attach_runtime_state(log),
        Some(MicAttachRuntimeState::Live)
    );

    let degraded_log = "phase: mic_live_frames_detected\nphase: mic_live_frames_lost\n";
    assert_eq!(
        parse_latest_mic_attach_runtime_state(degraded_log),
        Some(MicAttachRuntimeState::Degraded)
    );
}

#[test]
fn mic_path_ready_marker_includes_silence_filler_phase() {
    assert!(has_mic_path_ready_marker(
        "stream started\nphase: mic_silence_filler_active\n"
    ));
}

#[test]
fn parses_latest_video_metrics_markers() {
    let log = "video_output_fps=58.3\nvideo_frame_drop_total=4 reason=status_1\nvideo_queue_overflow_count=2\nvideo_output_fps=59.8\nvideo_frame_drop_total=7 reason=queue_overflow\nvideo_queue_overflow_count=3\n";
    assert_eq!(parse_latest_video_output_fps(log), Some(59.8));
    assert_eq!(parse_latest_video_frame_drop_total(log), Some(7));
    assert_eq!(parse_latest_video_queue_overflow_count(log), Some(3));
}

#[test]
fn startup_timeout_diagnosis_detects_mixed_mic_pipe_stall() {
    let log = "=== audio mode: system_plus_mic mic_backend=sck_native ===\nstream started\nfirst video frame delivered\nfirst system audio frame delivered\n";
    assert_eq!(startup_timeout_reason_code(log), "mic_pipe_startup_stalled");
    assert_eq!(
        startup_timeout_guidance(log),
        "first video and system audio seen but no microphone path; mixed audio pipe startup likely stalled"
    );
}

#[test]
fn startup_timeout_diagnosis_detects_mixed_mic_first_frame_stall() {
    let log = "=== audio mode: system_plus_mic mic_backend=sck_native ===\nstream started\nfirst video frame delivered\nfirst system audio frame delivered\nphase: mic_audio_pipe_connected\nphase: mic_backend_ready backend=sck_native device_id=default device_name=External Microphone\nmic source format: sample_rate=48000 channels=1 float=true interleaved=true bits=32\nmic converter configured: 48000hz-1ch -> 48000hz-2ch\n";
    assert_eq!(
        startup_timeout_reason_code(log),
        "mic_first_frame_startup_stalled"
    );
    assert_eq!(
        startup_timeout_guidance(log),
        "microphone backend initialized but no first microphone frame reached ffmpeg; mixed mic startup likely stalled"
    );
}

#[test]
fn parses_latest_system_pressure_markers() {
    let log = "system_memory_pressure=normal\nthermal_state=fair\nsystem_memory_pressure=warning\nthermal_state=serious\n";
    assert_eq!(
        parse_latest_system_memory_pressure_level(log),
        Some("warning".to_string())
    );
    assert_eq!(
        parse_latest_helper_thermal_state(log),
        Some("serious".to_string())
    );
}

#[test]
fn parses_latest_mic_backend_retry_and_error_markers() {
    let log = "phase: mic_backend_error backend=avcapture code=mic_device_missing reason=selected microphone disconnected\nphase: mic_backend_retry_scheduled backend=avcapture reason=mic_device_missing delay_ms=15000\n";
    assert_eq!(
        parse_latest_mic_recovery_state(log),
        Some("retrying".to_string())
    );
    assert_eq!(
        parse_latest_mic_backend_error(log),
        Some((
            "mic_device_missing".to_string(),
            "selected microphone disconnected".to_string()
        ))
    );
}

#[test]
fn parses_latest_mic_backend_ready_markers() {
    let log = "phase: mic_backend_attempt backend=avcapture device_id=123 device_name=USB Mic\nphase: mic_backend_ready backend=avcapture device_id=123 device_name=USB Mic\n";
    assert_eq!(parse_latest_mic_recovery_state(log), Some("ok".to_string()));
    assert_eq!(
        parse_latest_selected_microphone_name(log),
        Some("USB Mic".to_string())
    );
}

#[test]
fn segment_time_delta_tracks_frame_rate() {
    let delta_60 = segment_time_delta_for_fps(60);
    let delta_30 = segment_time_delta_for_fps(30);
    assert!(delta_60 > 0.0);
    assert!(delta_30 > delta_60);
}

#[test]
fn capture_args_use_small_queues_and_skip_live_faststart() {
    let settings = SettingsDto::default();
    let pipes = CapturePipes {
        video_pipe: PathBuf::from("/tmp/video.pipe"),
        system_audio_pipe: Some(PathBuf::from("/tmp/system_audio.pipe")),
        mic_audio_pipe: None,
    };
    let args = build_capture_args_from_pipes(
        &settings,
        Path::new("/tmp/rewinder-live"),
        "sessiontest",
        1920,
        1080,
        0.5,
        &pipes,
        AudioMode::SystemOnly,
        "automatic",
        LiveQueueProfile::Small,
    );
    let joined = args.join(" ");
    assert!(joined.contains("-thread_queue_size 8"));
    assert!(joined.contains("-thread_queue_size 64"));
    assert!(!joined.contains("-use_wallclock_as_timestamps"));
    assert!(joined.contains("-segment_time_delta"));
    assert!(!joined.contains("-vsync"));
    assert!(!joined.contains("+faststart"));
}

#[test]
fn capture_args_use_elevated_queues_when_requested() {
    let settings = SettingsDto::default();
    let pipes = CapturePipes {
        video_pipe: PathBuf::from("/tmp/video.pipe"),
        system_audio_pipe: Some(PathBuf::from("/tmp/system_audio.pipe")),
        mic_audio_pipe: Some(PathBuf::from("/tmp/mic_audio.pipe")),
    };
    let args = build_capture_args_from_pipes(
        &settings,
        Path::new("/tmp/rewinder-live"),
        "sessiontest",
        1920,
        1080,
        0.5,
        &pipes,
        AudioMode::SystemPlusMic,
        "automatic",
        LiveQueueProfile::Elevated,
    );
    let joined = args.join(" ");
    assert!(joined.contains("-thread_queue_size 32"));
    assert!(joined.contains("-thread_queue_size 256"));
}

#[test]
fn capture_args_skip_rnnoise_for_voice_isolation_backend() {
    let mut settings = SettingsDto::default();
    settings.mic_noise_suppression = true;
    let pipes = CapturePipes {
        video_pipe: PathBuf::from("/tmp/video.pipe"),
        system_audio_pipe: Some(PathBuf::from("/tmp/system_audio.pipe")),
        mic_audio_pipe: Some(PathBuf::from("/tmp/mic_audio.pipe")),
    };
    let args = build_capture_args_from_pipes(
        &settings,
        Path::new("/tmp/rewinder-live"),
        "sessiontest",
        1920,
        1080,
        0.5,
        &pipes,
        AudioMode::SystemPlusMic,
        "voice_isolation",
        LiveQueueProfile::Elevated,
    );
    let joined = args.join(" ");
    assert!(!joined.contains("arnndn"));
    assert!(!joined.contains("afftdn"));
}

#[test]
fn capture_dimensions_supports_runtime_540p_profile() {
    assert_eq!(capture_dimensions(540), (960, 540));
}

#[test]
fn continuity_threshold_adapts_to_recent_segment_cadence() {
    let base = SystemTime::UNIX_EPOCH;
    let entries = vec![
        SegmentFile {
            path: PathBuf::from("/tmp/seg_a_00000000.mp4"),
            modified: base + Duration::from_millis(0),
            size_bytes: 1000,
            session_id: Some("a".to_string()),
            segment_index: Some(0),
        },
        SegmentFile {
            path: PathBuf::from("/tmp/seg_a_00000001.mp4"),
            modified: base + Duration::from_millis(500),
            size_bytes: 1000,
            session_id: Some("a".to_string()),
            segment_index: Some(1),
        },
        SegmentFile {
            path: PathBuf::from("/tmp/seg_a_00000002.mp4"),
            modified: base + Duration::from_millis(1000),
            size_bytes: 1000,
            session_id: Some("a".to_string()),
            segment_index: Some(2),
        },
        SegmentFile {
            path: PathBuf::from("/tmp/seg_a_00000003.mp4"),
            modified: base + Duration::from_millis(1500),
            size_bytes: 1000,
            session_id: Some("a".to_string()),
            segment_index: Some(3),
        },
    ];
    let threshold = replay_continuity_gap_threshold(&entries, entries.len() - 1, 0.5);
    let secs = threshold.as_secs_f32();
    assert!(
        secs > 1.6 && secs < 1.9,
        "expected adaptive threshold around 1.75s, got {secs}"
    );
}

#[test]
fn continuity_threshold_clamps_for_large_segment_gaps() {
    let base = SystemTime::UNIX_EPOCH;
    let entries = vec![
        SegmentFile {
            path: PathBuf::from("/tmp/seg_b_00000000.mp4"),
            modified: base + Duration::from_secs(0),
            size_bytes: 1000,
            session_id: Some("b".to_string()),
            segment_index: Some(0),
        },
        SegmentFile {
            path: PathBuf::from("/tmp/seg_b_00000001.mp4"),
            modified: base + Duration::from_millis(1500),
            size_bytes: 1000,
            session_id: Some("b".to_string()),
            segment_index: Some(1),
        },
        SegmentFile {
            path: PathBuf::from("/tmp/seg_b_00000002.mp4"),
            modified: base + Duration::from_millis(3000),
            size_bytes: 1000,
            session_id: Some("b".to_string()),
            segment_index: Some(2),
        },
    ];
    let threshold = replay_continuity_gap_threshold(&entries, entries.len() - 1, 0.5);
    assert_eq!(threshold, Duration::from_secs(4));
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("rewinder-{label}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path).expect("temp dir should be creatable");
    path
}

fn write_segment_file(dir: &Path, session_id: &str, index: u64) {
    let path = dir.join(format!("seg_{session_id}_{index:08}.mp4"));
    fs::write(path, b"segment").expect("segment should be writable");
}

fn build_test_capture_engine(segment_dir: PathBuf, session_id: &str) -> CaptureEngine {
    CaptureEngine {
        segment_dir: segment_dir.clone(),
        capture_log_path: segment_dir.join("ffmpeg-capture.log"),
        capture_lock_path: segment_dir.join("capture.lock"),
        capture_lock_file: None,
        capture_owner_pid: std::process::id(),
        ffmpeg_pid_file: segment_dir.join("ffmpeg-capture.pid"),
        helper_pid_file: segment_dir.join("sck-capture.pid"),
        video_pipe_path: segment_dir.join("video.pipe"),
        system_audio_pipe_path: None,
        mic_audio_pipe_path: None,
        buffer_duration_secs: 120,
        segment_duration_secs: 0.5,
        stop: Arc::new(AtomicBool::new(false)),
        ffmpeg_child: Arc::new(Mutex::new(None)),
        helper_child: Arc::new(Mutex::new(None)),
        worker: None,
        last_error: Arc::new(Mutex::new(None)),
        startup_fallback_error: None,
        active_audio_mode: AudioMode::SystemOnly,
        session_id: session_id.to_string(),
        capture_log_offset: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        mic_backend_in_use: "avcapture".to_string(),
        queue_profile: LiveQueueProfile::Small,
        display_signature: 0,
        display_change_seen_at: Arc::new(Mutex::new(None)),
        prune_frozen: Arc::new(AtomicBool::new(false)),
        log_metrics: Mutex::new(super::LogMetricsCache::default()),
    }
}

#[test]
fn replay_selection_prefers_active_session_when_sessions_are_interleaved() {
    let temp_dir = unique_temp_dir("replay-active-session");
    for idx in 0..8_u64 {
        write_segment_file(&temp_dir, "active", idx);
        thread::sleep(Duration::from_millis(12));
        write_segment_file(&temp_dir, "other", idx);
        thread::sleep(Duration::from_millis(12));
    }
    thread::sleep(Duration::from_millis(600));

    let engine = build_test_capture_engine(temp_dir.clone(), "active");
    let selection = engine
        .replay_selection_for_save_at(3, SystemTime::now())
        .expect("selection should succeed")
        .expect("selection should exist");

    assert!(!selection.partial_history);
    for segment in &selection.segments {
        let name = segment
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_string();
        assert!(
            name.contains("seg_active_"),
            "selection should keep only active session segments, got {name}"
        );
    }

    drop(engine);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn replay_selection_marks_session_boundary_when_active_history_is_short() {
    let temp_dir = unique_temp_dir("replay-session-boundary");
    for idx in 0..8_u64 {
        write_segment_file(&temp_dir, "older", idx);
        thread::sleep(Duration::from_millis(10));
    }
    for idx in 0..2_u64 {
        write_segment_file(&temp_dir, "active", idx);
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(600));

    let engine = build_test_capture_engine(temp_dir.clone(), "active");
    let selection = engine
        .replay_selection_for_save_at(5, SystemTime::now())
        .expect("selection should succeed")
        .expect("selection should exist");

    assert!(selection.partial_history);
    assert_eq!(
        selection.partial_reason_code.as_deref(),
        Some("session_boundary")
    );

    drop(engine);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn capture_lock_payload_parsing_and_stale_reclaim_work() {
    let payload =
        parse_capture_lock_payload("owner_pid=123\nstarted_epoch_ms=456\nsession_id=abc\n");
    assert_eq!(payload.owner_pid, Some(123));
    assert_eq!(payload.started_epoch_ms, Some(456));
    assert_eq!(payload.session_id.as_deref(), Some("abc"));

    let temp_dir = unique_temp_dir("capture-lock");
    let lock_path = temp_dir.join("capture.lock");
    fs::write(
        &lock_path,
        "owner_pid=4294967295\nstarted_epoch_ms=1\nsession_id=old\n",
    )
    .expect("lock payload should be writable");

    let (lock_file, stale_owner) = acquire_capture_lock(&lock_path, std::process::id(), "active")
        .expect("lock acquisition should succeed");
    assert!(
        stale_owner.is_some(),
        "stale owner payload should be returned when owner pid is dead"
    );

    drop(lock_file);
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn mix_graph_uses_rnnoise_when_model_available() {
    let graph = build_system_plus_mic_mix_graph(10.0, 100, true, Some("/bundle/models/bd.rnnn"));
    assert!(graph.starts_with(
        "[1:a]volume=1.000[sys];[2:a]arnndn=m='/bundle/models/bd.rnnn',volume=10.0dB[mic]"
    ));
    assert!(!graph.contains("afftdn"));
    assert!(graph.contains("weights=1 1"));
    assert!(graph.contains("aresample=async=1:min_hard_comp=0.100:first_pts=0"));
}

#[test]
fn mix_graph_falls_back_to_afftdn_without_model() {
    let graph = build_system_plus_mic_mix_graph(10.0, 100, true, None);
    assert!(graph.contains("[2:a]afftdn="));
    assert!(!graph.contains("arnndn"));
}

#[test]
fn mix_graph_has_no_denoiser_when_suppression_off() {
    let graph = build_system_plus_mic_mix_graph(10.0, 100, false, Some("/bundle/models/bd.rnnn"));
    assert!(graph.contains("[2:a]volume=10.0dB[mic]"));
    assert!(!graph.contains("arnndn"));
    assert!(!graph.contains("afftdn"));
}
