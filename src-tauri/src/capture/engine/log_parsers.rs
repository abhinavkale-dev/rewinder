use super::*;
pub(super) fn parse_latest_mic_level_dbfs(log: &str) -> Option<f32> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix("mic_level_dbfs=") else {
            continue;
        };
        if let Ok(parsed) = value.trim().parse::<f32>() {
            if parsed.is_finite() {
                return Some(parsed);
            }
        }
    }
    None
}

pub(super) fn parse_latest_mic_samples_per_sec(log: &str) -> Option<u32> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(MIC_SAMPLES_PER_SEC_PREFIX) else {
            continue;
        };
        if let Ok(parsed) = value.trim().parse::<u32>() {
            if parsed > 0 {
                return Some(parsed);
            }
        }
    }
    None
}

pub(super) fn parse_latest_mic_attach_runtime_state(log: &str) -> Option<MicAttachRuntimeState> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        if trimmed.contains(MIC_LIVE_FRAMES_DETECTED_MARKER)
            || trimmed.contains(FIRST_MIC_AUDIO_MARKER)
        {
            return Some(MicAttachRuntimeState::Live);
        }
        if trimmed.contains(MIC_LIVE_FRAMES_LOST_MARKER) {
            return Some(MicAttachRuntimeState::Degraded);
        }
        if trimmed.contains(MIC_SILENCE_FILLER_ACTIVE_MARKER) {
            return Some(MicAttachRuntimeState::SilenceFiller);
        }
    }
    None
}

pub(super) fn parse_latest_video_output_fps(log: &str) -> Option<f32> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(VIDEO_OUTPUT_FPS_PREFIX) else {
            continue;
        };
        if let Ok(parsed) = value.trim().parse::<f32>() {
            if parsed.is_finite() && parsed > 0.0 {
                return Some(parsed);
            }
        }
    }
    None
}

pub(super) fn parse_latest_video_frame_drop_total(log: &str) -> Option<u64> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(VIDEO_FRAME_DROP_TOTAL_PREFIX) else {
            continue;
        };
        let token = value.split_whitespace().next().unwrap_or_default().trim();
        if let Ok(parsed) = token.parse::<u64>() {
            return Some(parsed);
        }
    }
    None
}

pub(super) fn parse_latest_video_queue_overflow_count(log: &str) -> Option<u64> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(VIDEO_QUEUE_OVERFLOW_COUNT_PREFIX) else {
            continue;
        };
        if let Ok(parsed) = value.trim().parse::<u64>() {
            return Some(parsed);
        }
    }
    None
}

pub(super) fn parse_latest_system_memory_pressure_level(log: &str) -> Option<String> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(SYSTEM_MEMORY_PRESSURE_PREFIX) else {
            continue;
        };
        let level = value.trim().to_ascii_lowercase();
        match level.as_str() {
            "normal" | "warning" | "critical" => return Some(level),
            _ => {}
        }
    }
    None
}

pub(super) fn parse_latest_helper_thermal_state(log: &str) -> Option<String> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(THERMAL_STATE_PREFIX) else {
            continue;
        };
        let level = value.trim().to_ascii_lowercase();
        match level.as_str() {
            "nominal" | "fair" | "serious" | "critical" => return Some(level),
            _ => {}
        }
    }
    None
}

pub(super) fn parse_latest_mic_recovery_state(log: &str) -> Option<String> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        if trimmed.contains("phase: mic_backend_recovered")
            || trimmed.contains("phase: mic_backend_ready")
        {
            return Some("ok".to_string());
        }
        if trimmed.contains("phase: mic_backend_retry_scheduled")
            || trimmed.contains("phase: mic_backend_error")
        {
            return Some("retrying".to_string());
        }
    }
    None
}

pub(super) fn parse_latest_selected_microphone_name(log: &str) -> Option<String> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        if !(trimmed.contains("phase: mic_backend_attempt")
            || trimmed.contains("phase: mic_backend_ready")
            || trimmed.contains("phase: mic_backend_recovered"))
        {
            continue;
        }
        let Some(index) = trimmed.find("device_name=") else {
            continue;
        };
        let value = trimmed[index + "device_name=".len()..].trim();
        if value.is_empty() || value == "system_default" {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

pub(super) fn has_mic_selected_device_not_found_marker(log: &str) -> bool {
    log.contains("phase: mic_selected_device_not_found")
}

pub(super) fn parse_latest_mic_backend_error(log: &str) -> Option<(String, String)> {
    for line in log.lines().rev() {
        let trimmed = line.trim();
        if !trimmed.contains("phase: mic_backend_error") {
            continue;
        }
        let code = parse_marker_token(trimmed, "code=")?;
        let reason_index = trimmed.find("reason=")?;
        let reason = trimmed[reason_index + "reason=".len()..].trim();
        if reason.is_empty() {
            return Some((code.to_string(), code.to_string()));
        }
        return Some((code.to_string(), reason.to_string()));
    }
    None
}

fn parse_marker_token<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(key)?;
    let value = &line[start + key.len()..];
    let token = value.split_whitespace().next()?.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}
