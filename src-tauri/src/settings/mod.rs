use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

mod patch;
mod validation;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub mic_noise_suppression: bool,
    pub audio_startup_timeout_ms: u16,
    pub profile_recover_hold_secs: u16,
    pub exclude_current_process_audio: bool,
    pub save_path_mode: String,
    pub audio_save_mode: String,
    pub performance_guard_enabled: bool,
    pub performance_guard_level: String,
    pub battery_guard_enabled: bool,
    pub battery_max_fps: u16,
    #[serde(default = "default_system_volume_percent")]
    pub system_volume_percent: u8,
    #[serde(default)]
    pub selected_display_id: Option<String>,
}

fn default_system_volume_percent() -> u8 {
    100
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
    pub mic_noise_suppression: Option<bool>,
    pub audio_startup_timeout_ms: Option<u16>,
    pub profile_recover_hold_secs: Option<u16>,
    pub exclude_current_process_audio: Option<bool>,
    pub save_path_mode: Option<String>,
    pub audio_save_mode: Option<String>,
    pub performance_guard_enabled: Option<bool>,
    pub performance_guard_level: Option<String>,
    pub battery_guard_enabled: Option<bool>,
    pub battery_max_fps: Option<u16>,
    pub system_volume_percent: Option<u8>,
    pub selected_display_id: Option<String>,
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
            mic_mix_gain_db: 6.0,
            mic_auto_request_permission: true,
            mic_auto_boost_volume: false,
            mic_noise_suppression: true,
            audio_startup_timeout_ms: 6_000,
            profile_recover_hold_secs: 8,
            exclude_current_process_audio: true,
            save_path_mode: "instant_mp4".to_string(),
            audio_save_mode: "fast".to_string(),
            performance_guard_enabled: true,
            performance_guard_level: "balanced".to_string(),
            battery_guard_enabled: true,
            battery_max_fps: 30,
            system_volume_percent: 100,
            selected_display_id: None,
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
        "auto" | "avcapture" | "sck_native" | "voice_isolation" => value,
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
        assert_eq!(settings.mic_mix_gain_db, 6.0);
        assert!(settings.mic_auto_request_permission);
        assert!(!settings.mic_auto_boost_volume);
        assert!(settings.mic_noise_suppression);
        assert_eq!(settings.audio_startup_timeout_ms, 6_000);
        assert_eq!(settings.profile_recover_hold_secs, 8);
        assert!(settings.exclude_current_process_audio);
        assert_eq!(settings.save_path_mode, "instant_mp4");
        assert_eq!(settings.audio_save_mode, "fast");
        assert!(settings.performance_guard_enabled);
        assert_eq!(settings.performance_guard_level, "balanced");
        assert!(settings.battery_guard_enabled);
        assert_eq!(settings.battery_max_fps, 30);
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
    fn rejects_invalid_battery_max_fps() {
        let mut settings = SettingsDto::default();
        settings.battery_max_fps = 5;
        assert!(settings.validate().is_err());

        settings.battery_max_fps = 200;
        assert!(settings.validate().is_err());

        settings.battery_max_fps = 30;
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn apply_patch_updates_battery_guard_fields() {
        let mut settings = SettingsDto::default();
        let patch = SettingsPatchDto {
            battery_guard_enabled: Some(false),
            battery_max_fps: Some(24),
            ..SettingsPatchDto::default()
        };
        settings.apply_patch(patch).expect("patch should apply");
        assert!(!settings.battery_guard_enabled);
        assert_eq!(settings.battery_max_fps, 24);
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
