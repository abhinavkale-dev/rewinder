use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionPreset {
    P360,
    P480,
    P720,
    P1080,
}

impl ResolutionPreset {
    pub const fn height(self) -> u16 {
        match self {
            Self::P360 => 360,
            Self::P480 => 480,
            Self::P720 => 720,
            Self::P1080 => 1080,
        }
    }

    pub const fn bitrate_kbps(self) -> u32 {
        match self {
            Self::P360 => 1_800,
            Self::P480 => 2_800,
            Self::P720 => 5_500,
            Self::P1080 => 10_000,
        }
    }

    pub fn from_height(height: u16) -> Self {
        match height {
            360 => Self::P360,
            480 => Self::P480,
            720 => Self::P720,
            _ => Self::P1080,
        }
    }
}

impl fmt::Display for ResolutionPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::P360 => write!(f, "360p"),
            Self::P480 => write!(f, "480p"),
            Self::P720 => write!(f, "720p"),
            Self::P1080 => write!(f, "1080p"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioModePreset {
    SystemOnly,
    SystemPlusMic,
    VideoOnly,
}

impl AudioModePreset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SystemOnly => "system_only",
            Self::SystemPlusMic => "system_plus_mic",
            Self::VideoOnly => "video_only",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "system_plus_mic" => Self::SystemPlusMic,
            "video_only" => Self::VideoOnly,
            _ => Self::SystemOnly,
        }
    }
}

pub fn ensure_buffer_for_replay(replay_duration_secs: u16, current_buffer_secs: u16) -> u16 {
    if current_buffer_secs >= replay_duration_secs {
        return current_buffer_secs;
    }

    replay_duration_secs.saturating_add(30).min(600)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub replay_duration_secs: u16,
    pub buffer_duration_secs: u16,
    pub fps: u16,
    pub video_resolution: u16,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
    pub output_dir: String,
    pub hotkey: String,
    pub fallback_hotkeys: Vec<String>,
    pub replay_enabled: bool,
    pub audio_mode: String,
    pub mic_enabled: bool,
    pub audio_sample_rate_hz: u32,
    pub audio_channels: u8,
    pub segment_time_ms: u16,
    pub warmup_defer_ttl_ms: u16,
    pub quality_policy: String,
    pub quality_preference: String,
    pub audio_fallback_policy: String,
    pub mic_capture_backend: String,
    pub selected_microphone_id: Option<String>,
    pub mic_failure_policy: String,
    pub mic_startup_timeout_ms: u16,
    pub mic_retry_interval_secs: u16,
    pub mic_mix_gain_db: f32,
    pub mic_auto_request_permission: bool,
    pub mic_auto_boost_volume: bool,
    pub audio_startup_timeout_ms: u16,
    pub profile_recover_hold_secs: u16,
    pub exclude_current_process_audio: bool,
    pub save_path_mode: String,
    pub audio_save_mode: String,
    pub performance_guard_enabled: bool,
    pub performance_guard_level: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatchDto {
    pub replay_duration_secs: Option<u16>,
    pub buffer_duration_secs: Option<u16>,
    pub fps: Option<u16>,
    pub video_resolution: Option<u16>,
    pub video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub output_dir: Option<String>,
    pub hotkey: Option<String>,
    pub fallback_hotkeys: Option<Vec<String>>,
    pub replay_enabled: Option<bool>,
    pub audio_mode: Option<String>,
    pub mic_enabled: Option<bool>,
    pub audio_sample_rate_hz: Option<u32>,
    pub audio_channels: Option<u8>,
    pub segment_time_ms: Option<u16>,
    pub warmup_defer_ttl_ms: Option<u16>,
    pub quality_policy: Option<String>,
    pub quality_preference: Option<String>,
    pub audio_fallback_policy: Option<String>,
    pub mic_capture_backend: Option<String>,
    pub selected_microphone_id: Option<String>,
    pub mic_failure_policy: Option<String>,
    pub mic_startup_timeout_ms: Option<u16>,
    pub mic_retry_interval_secs: Option<u16>,
    pub mic_mix_gain_db: Option<f32>,
    pub mic_auto_request_permission: Option<bool>,
    pub mic_auto_boost_volume: Option<bool>,
    pub audio_startup_timeout_ms: Option<u16>,
    pub profile_recover_hold_secs: Option<u16>,
    pub exclude_current_process_audio: Option<bool>,
    pub save_path_mode: Option<String>,
    pub audio_save_mode: Option<String>,
    pub performance_guard_enabled: Option<bool>,
    pub performance_guard_level: Option<String>,
}

fn system_ram_gb() -> u64 {
    std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .map(|bytes| bytes / (1024 * 1024 * 1024))
        .unwrap_or(16)
}

impl Default for SettingsDto {
    fn default() -> Self {
        let output = default_output_dir();
        let low_ram = system_ram_gb() <= 8;
        let default_res: u16 = if low_ram { 720 } else { 1080 };
        let default_fps: u16 = if low_ram { 30 } else { 60 };

        Self {
            replay_duration_secs: 30,
            buffer_duration_secs: 120,
            fps: default_fps,
            video_resolution: default_res,
            video_bitrate_kbps: ResolutionPreset::from_height(default_res).bitrate_kbps(),
            audio_bitrate_kbps: 160,
            output_dir: output.to_string_lossy().to_string(),
            hotkey: "Ctrl+Option+R".to_string(),
            fallback_hotkeys: vec![
                "Ctrl+Option+R".to_string(),
                "Ctrl+Option+Shift+R".to_string(),
                "Cmd+Option+R".to_string(),
            ],
            replay_enabled: true,
            audio_mode: AudioModePreset::SystemPlusMic.as_str().to_string(),
            mic_enabled: true,
            audio_sample_rate_hz: 48_000,
            audio_channels: 2,
            segment_time_ms: 500,
            warmup_defer_ttl_ms: 3_000,
            quality_policy: "adaptive_recover".to_string(),
            quality_preference: if low_ram { "prefer_smoothness" } else { "prefer_quality" }.to_string(),
            audio_fallback_policy: "system_only_fallback".to_string(),
            mic_capture_backend: "auto".to_string(),
            selected_microphone_id: None,
            mic_failure_policy: "best_effort".to_string(),
            mic_startup_timeout_ms: 2_500,
            mic_retry_interval_secs: 15,
            mic_mix_gain_db: 10.0,
            mic_auto_request_permission: true,
            mic_auto_boost_volume: false,
            audio_startup_timeout_ms: 6_000,
            profile_recover_hold_secs: 20,
            exclude_current_process_audio: true,
            save_path_mode: "instant_mp4".to_string(),
            audio_save_mode: "fast".to_string(),
            performance_guard_enabled: true,
            performance_guard_level: "balanced".to_string(),
        }
    }
}

fn default_output_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let mut output = PathBuf::from(home);
        output.push("Downloads");
        output.push("Rewinder");
        return output;
    }

    let mut output = std::env::temp_dir();
    output.push("rewinder-clips");
    output
}

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
            } else {
                Some(value)
            };
        }
        if let Some(value) = patch.mic_failure_policy {
            self.mic_failure_policy = value;
        }
        if let Some(value) = patch.mic_startup_timeout_ms {
            self.mic_startup_timeout_ms = value;
        }
        if let Some(value) = patch.mic_retry_interval_secs {
            self.mic_retry_interval_secs = value;
        }
        if let Some(value) = patch.mic_mix_gain_db {
            self.mic_mix_gain_db = value;
        }
        if let Some(value) = patch.mic_auto_request_permission {
            self.mic_auto_request_permission = value;
        }
        if let Some(value) = patch.mic_auto_boost_volume {
            self.mic_auto_boost_volume = value;
        }
        if let Some(value) = patch.audio_startup_timeout_ms {
            self.audio_startup_timeout_ms = value;
        }
        if let Some(value) = patch.profile_recover_hold_secs {
            self.profile_recover_hold_secs = value;
        }
        if let Some(value) = patch.exclude_current_process_audio {
            self.exclude_current_process_audio = value;
        }
        if let Some(value) = patch.save_path_mode {
            self.save_path_mode = value;
        }
        if let Some(value) = patch.audio_save_mode {
            if !has_save_path_mode_patch {
                self.save_path_mode = match value.as_str() {
                    "smooth" => "smooth".to_string(),
                    "adaptive" => "adaptive".to_string(),
                    _ => "instant_mp4".to_string(),
                };
            }
            self.audio_save_mode = value;
        }
        if let Some(value) = patch.performance_guard_enabled {
            self.performance_guard_enabled = value;
        }
        if let Some(value) = patch.performance_guard_level {
            self.performance_guard_level = value;
        }

        self.validate()
    }

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
        if !matches!(normalized_mic_backend, "auto" | "avcapture" | "sck_native") {
            return Err(
                "mic_capture_backend must be one of: auto, avcapture, sck_native".to_string(),
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

        Ok(())
    }

    pub fn output_dir_path(&self) -> PathBuf {
        let configured = PathBuf::from(self.output_dir.clone());
        if configured.as_os_str().is_empty() {
            return default_output_dir();
        }

        let temp_root = std::env::temp_dir();
        if configured.starts_with(&temp_root) {
            return default_output_dir();
        }

        configured
    }

    pub fn replay_duration_us(&self) -> i64 {
        i64::from(self.replay_duration_secs) * 1_000_000
    }

    pub fn buffer_duration_us(&self) -> i64 {
        i64::from(self.buffer_duration_secs) * 1_000_000
    }
}

fn normalize_mic_capture_backend(value: &str) -> &str {
    match value {
        "sck_experimental" => "sck_native",
        "auto" | "avcapture" | "sck_native" => value,
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_valid() {
        assert!(SettingsDto::default().validate().is_ok());
    }

    #[test]
    fn rejects_invalid_ranges() {
        let mut settings = SettingsDto::default();
        settings.buffer_duration_secs = 10;
        settings.replay_duration_secs = 20;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn resolution_mapping_is_stable() {
        assert_eq!(ResolutionPreset::P360.height(), 360);
        assert_eq!(ResolutionPreset::P480.height(), 480);
        assert_eq!(ResolutionPreset::P720.height(), 720);
        assert_eq!(ResolutionPreset::P1080.height(), 1080);
        assert_eq!(ResolutionPreset::P360.bitrate_kbps(), 1_800);
        assert_eq!(ResolutionPreset::P480.bitrate_kbps(), 2_800);
        assert_eq!(ResolutionPreset::P720.bitrate_kbps(), 5_500);
        assert_eq!(ResolutionPreset::P1080.bitrate_kbps(), 10_000);
        assert_eq!(ResolutionPreset::from_height(360), ResolutionPreset::P360);
        assert_eq!(ResolutionPreset::from_height(480), ResolutionPreset::P480);
        assert_eq!(ResolutionPreset::from_height(720), ResolutionPreset::P720);
        assert_eq!(ResolutionPreset::from_height(1080), ResolutionPreset::P1080);
    }

    #[test]
    fn replay_buffer_floor_adjustment() {
        assert_eq!(ensure_buffer_for_replay(60, 30), 90);
        assert_eq!(ensure_buffer_for_replay(60, 90), 90);
        assert_eq!(ensure_buffer_for_replay(590, 100), 600);
    }

    #[test]
    fn defaults_include_hotkey_fallbacks_and_runtime_tuning() {
        let settings = SettingsDto::default();
        assert_eq!(settings.segment_time_ms, 500);
        assert_eq!(settings.warmup_defer_ttl_ms, 3_000);
        assert_eq!(settings.audio_mode, "system_plus_mic");
        assert!(settings.mic_enabled);
        assert_eq!(settings.audio_sample_rate_hz, 48_000);
        assert_eq!(settings.audio_channels, 2);
        assert_eq!(settings.quality_policy, "adaptive_recover");
        assert_eq!(settings.quality_preference, "prefer_quality");
        assert_eq!(settings.audio_fallback_policy, "system_only_fallback");
        assert_eq!(settings.mic_capture_backend, "auto");
        assert_eq!(settings.selected_microphone_id, None);
        assert_eq!(settings.mic_failure_policy, "best_effort");
        assert_eq!(settings.mic_startup_timeout_ms, 2_500);
        assert_eq!(settings.mic_retry_interval_secs, 15);
        assert_eq!(settings.mic_mix_gain_db, 10.0);
        assert!(settings.mic_auto_request_permission);
        assert!(!settings.mic_auto_boost_volume);
        assert_eq!(settings.audio_startup_timeout_ms, 6_000);
        assert_eq!(settings.profile_recover_hold_secs, 20);
        assert!(settings.exclude_current_process_audio);
        assert_eq!(settings.save_path_mode, "instant_mp4");
        assert_eq!(settings.audio_save_mode, "fast");
        assert!(settings.performance_guard_enabled);
        assert_eq!(settings.performance_guard_level, "balanced");
        assert_eq!(
            settings.fallback_hotkeys,
            vec![
                "Ctrl+Option+R".to_string(),
                "Ctrl+Option+Shift+R".to_string(),
                "Cmd+Option+R".to_string()
            ]
        );
    }

    #[test]
    fn rejects_invalid_runtime_tuning_ranges() {
        let mut settings = SettingsDto::default();
        settings.segment_time_ms = 200;
        assert!(settings.validate().is_err());

        settings.segment_time_ms = 500;
        settings.warmup_defer_ttl_ms = 200;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn rejects_invalid_audio_settings() {
        let mut settings = SettingsDto::default();
        settings.audio_mode = "invalid".to_string();
        assert!(settings.validate().is_err());

        settings.audio_mode = "system_only".to_string();
        settings.audio_sample_rate_hz = 4_000;
        assert!(settings.validate().is_err());

        settings.audio_sample_rate_hz = 48_000;
        settings.audio_channels = 3;
        assert!(settings.validate().is_err());

        settings.audio_channels = 2;
        settings.quality_policy = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.quality_policy = "adaptive_recover".to_string();
        settings.quality_preference = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.quality_preference = "prefer_quality".to_string();
        settings.audio_fallback_policy = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.audio_fallback_policy = "system_only_fallback".to_string();
        settings.mic_capture_backend = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.mic_capture_backend = "auto".to_string();
        settings.mic_failure_policy = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.mic_failure_policy = "best_effort".to_string();
        settings.mic_startup_timeout_ms = 100;
        assert!(settings.validate().is_err());

        settings.mic_startup_timeout_ms = 2_500;
        settings.mic_retry_interval_secs = 2;
        assert!(settings.validate().is_err());

        settings.mic_retry_interval_secs = 15;
        settings.mic_mix_gain_db = -1.0;
        assert!(settings.validate().is_err());

        settings.mic_mix_gain_db = 6.0;
        settings.audio_startup_timeout_ms = 1_000;
        assert!(settings.validate().is_err());

        settings.audio_startup_timeout_ms = 6_000;
        settings.profile_recover_hold_secs = 2;
        assert!(settings.validate().is_err());

        settings.profile_recover_hold_secs = 20;
        settings.save_path_mode = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.save_path_mode = "instant_mp4".to_string();
        settings.audio_save_mode = "unknown".to_string();
        assert!(settings.validate().is_err());

        settings.audio_save_mode = "fast".to_string();
        settings.performance_guard_level = "invalid".to_string();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn migrates_legacy_mic_backend_values() {
        let mut settings = SettingsDto::default();
        settings.mic_capture_backend = "sck_experimental".to_string();
        assert!(settings.validate().is_ok());
        assert_eq!(
            normalize_mic_capture_backend(&settings.mic_capture_backend),
            "sck_native"
        );
    }
}
