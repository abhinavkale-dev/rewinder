use super::*;
impl SettingsDto {
    pub fn apply_patch(&mut self, patch: SettingsPatchDto) -> Result<(), String> {
        let requested_bitrate = patch.video_bitrate_kbps;
        let has_save_path_mode_patch = patch.save_path_mode.is_some();

        if let Some(value) = patch.replay_duration_secs {
            self.replay_duration_secs = value;
        }
        if let Some(value) = patch.buffer_duration_secs {
            self.buffer_duration_secs = value;
        }
        if let Some(value) = patch.fps {
            self.fps = value;
        }
        if let Some(value) = patch.video_resolution {
            self.video_resolution = value;
            if requested_bitrate.is_none() {
                self.video_bitrate_kbps = ResolutionPreset::from_height(value).bitrate_kbps();
            }
        }
        if let Some(value) = requested_bitrate {
            self.video_bitrate_kbps = value;
        }
        if let Some(value) = patch.audio_bitrate_kbps {
            self.audio_bitrate_kbps = value;
        }
        if let Some(value) = patch.output_dir {
            self.output_dir = value;
        }
        if let Some(value) = patch.hotkey {
            self.hotkey = value;
        }
        if let Some(value) = patch.fallback_hotkeys {
            self.fallback_hotkeys = value;
        }
        if let Some(value) = patch.replay_enabled {
            self.replay_enabled = value;
        }
        if let Some(value) = patch.audio_mode {
            self.audio_mode = value;
        }
        if let Some(value) = patch.mic_enabled {
            self.mic_enabled = value;
        }
        if let Some(value) = patch.audio_sample_rate_hz {
            self.audio_sample_rate_hz = value;
        }
        if let Some(value) = patch.audio_channels {
            self.audio_channels = value;
        }
        if let Some(value) = patch.segment_time_ms {
            self.segment_time_ms = value;
        }
        if let Some(value) = patch.warmup_defer_ttl_ms {
            self.warmup_defer_ttl_ms = value;
        }
        if let Some(value) = patch.quality_policy {
            self.quality_policy = value;
        }
        if let Some(value) = patch.quality_preference {
            self.quality_preference = value;
        }
        if let Some(value) = patch.audio_fallback_policy {
            self.audio_fallback_policy = value;
        }
        if let Some(value) = patch.mic_capture_backend {
            self.mic_capture_backend = normalize_mic_capture_backend(&value).to_string();
        }
        if let Some(value) = patch.selected_microphone_id {
            self.selected_microphone_id = if value.trim().is_empty() {
                None
