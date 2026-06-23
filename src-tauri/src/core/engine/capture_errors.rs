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
    if lower.contains("phase: stream_stop_user_intent")
        || lower.contains("code=-3817")
        || lower.contains("code=-3821")
    {
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
