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
        }
        if !matches!(
            self.audio_fallback_policy.as_str(),
            "system_only_fallback" | "allow_video_only"
        ) {
            return Err(
                "audio_fallback_policy must be one of: system_only_fallback, allow_video_only"
                    .to_string(),
            );
        }
        let normalized_mic_backend = normalize_mic_capture_backend(&self.mic_capture_backend);
        if !matches!(
            normalized_mic_backend,
            "auto" | "avcapture" | "sck_native" | "voice_isolation"
        ) {
            return Err(
                "mic_capture_backend must be one of: auto, avcapture, sck_native, voice_isolation"
                    .to_string(),
            );
        }
        if !matches!(self.mic_failure_policy.as_str(), "best_effort" | "required") {
            return Err("mic_failure_policy must be one of: best_effort, required".to_string());
        }
        if !(1_000..=10_000).contains(&self.mic_startup_timeout_ms) {
            return Err("mic_startup_timeout_ms must be between 1000 and 10000".to_string());
        }
        if !(5..=120).contains(&self.mic_retry_interval_secs) {
            return Err("mic_retry_interval_secs must be between 5 and 120".to_string());
        }
        if !(0.0..=18.0).contains(&self.mic_mix_gain_db) {
            return Err("mic_mix_gain_db must be between 0.0 and 18.0".to_string());
        }
        if !(2_000..=20_000).contains(&self.audio_startup_timeout_ms) {
            return Err("audio_startup_timeout_ms must be between 2000 and 20000".to_string());
        }
        if !(5..=120).contains(&self.profile_recover_hold_secs) {
            return Err("profile_recover_hold_secs must be between 5 and 120".to_string());
        }
        if !matches!(
            self.save_path_mode.as_str(),
            "instant_mp4" | "smooth" | "adaptive" | "fast"
        ) {
            return Err(
                "save_path_mode must be one of: instant_mp4, smooth, adaptive, fast".to_string(),
            );
        }
        if !matches!(
            self.audio_save_mode.as_str(),
            "smooth" | "fast" | "adaptive"
        ) {
            return Err("audio_save_mode must be one of: smooth, fast, adaptive".to_string());
        }
        if !matches!(
            self.performance_guard_level.as_str(),
            "balanced" | "quality_first" | "performance_first"
        ) {
            return Err(
                "performance_guard_level must be one of: balanced, quality_first, performance_first"
                    .to_string(),
            );
        }
        if !(10..=120).contains(&self.battery_max_fps) {
            return Err("battery_max_fps must be between 10 and 120".to_string());
        }

        Ok(())
    }
}
