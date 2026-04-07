use crate::settings::SettingsDto;

pub struct AudioEncoder;

impl AudioEncoder {
    pub fn ffmpeg_args(settings: &SettingsDto, has_audio_input: bool) -> Vec<String> {
        if !has_audio_input {
            return vec!["-an".to_string()];
        }

        vec![
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            format!("{}k", settings.audio_bitrate_kbps),
            "-ar".to_string(),
            settings.audio_sample_rate_hz.to_string(),
            "-ac".to_string(),
            settings.audio_channels.to_string(),
        ]
    }
}
