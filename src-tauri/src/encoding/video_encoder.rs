use crate::settings::SettingsDto;

pub struct VideoEncoder;

impl VideoEncoder {
    pub fn ffmpeg_args(settings: &SettingsDto) -> Vec<String> {
        let fps = u32::from(settings.fps.max(1));
        let keyframe_interval_secs =
            f32::from(settings.segment_time_ms.clamp(250, 2_000)) / 1_000.0;
        let gop = ((fps as f32 * keyframe_interval_secs).round() as u32).max(1);

        vec![
            "-r".to_string(),
            settings.fps.to_string(),
            "-c:v".to_string(),
            "h264_videotoolbox".to_string(),
            "-realtime".to_string(),
            "1".to_string(),
            "-b:v".to_string(),
            format!("{}k", settings.video_bitrate_kbps),
            "-maxrate".to_string(),
            format!("{}k", settings.video_bitrate_kbps),
            "-bufsize".to_string(),
            format!("{}k", settings.video_bitrate_kbps),
            "-flags".to_string(),
            "+cgop".to_string(),
            "-force_key_frames".to_string(),
            format!("expr:gte(t,n_forced*{keyframe_interval_secs:.3})"),
            "-g".to_string(),
            gop.to_string(),
            "-keyint_min".to_string(),
            gop.to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::VideoEncoder;
    use crate::settings::SettingsDto;

    #[test]
    fn keyframe_interval_tracks_segment_duration() {
        let mut settings = SettingsDto::default();
        settings.fps = 60;
        settings.segment_time_ms = 500;

        let args = VideoEncoder::ffmpeg_args(&settings).join(" ");
        assert!(args.contains("expr:gte(t,n_forced*0.500)"));
        assert!(args.contains("-g 30"));
        assert!(args.contains("-keyint_min 30"));
    }
}
