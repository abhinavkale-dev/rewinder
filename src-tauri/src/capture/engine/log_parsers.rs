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
