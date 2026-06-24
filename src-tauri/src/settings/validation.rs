use super::*;
impl SettingsDto {
    pub fn validate(&self) -> Result<(), String> {
        if self.replay_duration_secs == 0 {
            return Err("replay_duration_secs must be > 0".to_string());
        }
        if self.buffer_duration_secs < self.replay_duration_secs {
            return Err("buffer_duration_secs must be >= replay_duration_secs".to_string());
        }
        if !(10..=120).contains(&self.fps) {
            return Err("fps must be between 10 and 120".to_string());
        }
        if !matches!(self.video_resolution, 360 | 480 | 720 | 1080) {
            return Err("video_resolution must be one of: 360, 480, 720, 1080".to_string());
        }
        if self.video_bitrate_kbps < 500 {
            return Err("video_bitrate_kbps must be >= 500".to_string());
        }
        if self.audio_bitrate_kbps < 64 {
            return Err("audio_bitrate_kbps must be >= 64".to_string());
        }
        if self.output_dir.trim().is_empty() {
            return Err("output_dir cannot be empty".to_string());
        }
        if self.hotkey.trim().is_empty() {
            return Err("hotkey cannot be empty".to_string());
        }
        if self.fallback_hotkeys.is_empty() {
            return Err("fallback_hotkeys cannot be empty".to_string());
        }
        if self
            .fallback_hotkeys
            .iter()
            .any(|shortcut| shortcut.trim().is_empty())
        {
            return Err("fallback_hotkeys cannot contain empty values".to_string());
        }
        if !(250..=2_000).contains(&self.segment_time_ms) {
            return Err("segment_time_ms must be between 250 and 2000".to_string());
        }
        if !(500..=10_000).contains(&self.warmup_defer_ttl_ms) {
            return Err("warmup_defer_ttl_ms must be between 500 and 10000".to_string());
        }
        if !matches!(
            self.audio_mode.as_str(),
            "system_only" | "system_plus_mic" | "video_only"
        ) {
            return Err(
                "audio_mode must be one of: system_only, system_plus_mic, video_only".to_string(),
            );
        }
        if !(8_000..=192_000).contains(&self.audio_sample_rate_hz) {
            return Err("audio_sample_rate_hz must be between 8000 and 192000".to_string());
        }
        if !matches!(self.audio_channels, 1 | 2) {
            return Err("audio_channels must be 1 or 2".to_string());
        }
        if !matches!(self.quality_policy.as_str(), "adaptive_recover" | "strict") {
            return Err("quality_policy must be one of: adaptive_recover, strict".to_string());
        }
        if !matches!(
            self.quality_preference.as_str(),
            "prefer_quality" | "prefer_smoothness"
        ) {
            return Err(
                "quality_preference must be one of: prefer_quality, prefer_smoothness".to_string(),
            );
