use std::path::Path;

use super::{
    build_fast_concat_args, build_smooth_concat_args, build_smooth_postprocess_args,
    build_trim_copy_args, concat_line_for_path, detect_fast_integrity_issue,
    generate_random_clip_id, parse_ffprobe_duration_secs, parse_ffprobe_fast_integrity_snapshot,
    prepare_segments_for_concat, SavePathMode,
};
use crate::settings::SettingsDto;

#[test]
fn concat_line_uses_single_quote_and_escapes_single_quote() {
    let line = concat_line_for_path(Path::new("/tmp/rewinder/o'malley clip.mp4"));
    assert_eq!(line, "file '/tmp/rewinder/o'\\''malley clip.mp4'\n");
}

#[test]
fn missing_segment_is_reported_before_ffmpeg_run() {
    let missing = Path::new("/tmp/rewinder_missing_segment_does_not_exist.mp4").to_path_buf();
    let list_file = Path::new("/tmp/rewinder_list_file.txt");

    let err = prepare_segments_for_concat(&[missing.clone()], list_file)
        .expect_err("expected missing segment error");
    assert!(err.contains("segment does not exist"));
    assert!(err.contains(missing.to_string_lossy().as_ref()));
}

#[test]
fn clip_id_is_randomized_format() {
    let id = generate_random_clip_id(Path::new("/tmp"));
    assert!(id.starts_with("rewinder-"));
    assert!(id.len() >= "rewinder-".len() + 16);
}

#[test]
fn save_path_mode_parsing_supports_instant_and_legacy() {
    assert!(matches!(
        SavePathMode::from_settings("instant_mp4", "fast"),
        SavePathMode::InstantMp4
    ));
    assert!(matches!(
        SavePathMode::from_settings("fast", "fast"),
        SavePathMode::Fast
    ));
    assert!(matches!(
        SavePathMode::from_settings("adaptive", "adaptive"),
        SavePathMode::Adaptive
    ));
    assert!(matches!(
        SavePathMode::from_settings("unknown", "fast"),
        SavePathMode::InstantMp4
    ));
    assert!(matches!(
        SavePathMode::from_settings("unknown", "smooth"),
        SavePathMode::Smooth
    ));
}

#[test]
fn smooth_concat_args_include_audio_resample() {
    let settings = SettingsDto::default();
    let args = build_smooth_concat_args(
        Path::new("/tmp/list.txt"),
        Path::new("/tmp/smooth.mp4"),
        &settings,
    );
    let joined = args.join(" ");
    assert!(joined.contains("-c:v copy"));
    assert!(joined.contains("-c:a aac"));
    assert!(joined.contains("aresample=async=1:min_hard_comp=0.100:first_pts=0"));
    assert!(joined.contains("-avoid_negative_ts make_zero"));
}

#[test]
fn fast_concat_args_keep_stream_copy() {
    let args = build_fast_concat_args(Path::new("/tmp/list.txt"), Path::new("/tmp/concat.mp4"));
    let joined = args.join(" ");
    assert!(joined.contains("-c copy"));
    assert!(!joined.contains("aresample=async=1"));
    assert!(joined.contains("-fflags +genpts"));
    assert!(joined.contains("-avoid_negative_ts make_zero"));
}

#[test]
fn trim_copy_args_include_duration_cap() {
    let args = build_trim_copy_args(Path::new("/tmp/in.mp4"), Path::new("/tmp/out.mp4"), 30.0);
    let joined = args.join(" ");
    assert!(joined.contains("-sseof -30.000"));
    assert!(joined.contains("-t 30.000"));
}

#[test]
fn smooth_postprocess_args_force_cfr_and_retime() {
    let settings = SettingsDto::default();
    let args = build_smooth_postprocess_args(
        &settings,
        Path::new("/tmp/in.mp4"),
        Path::new("/tmp/out.mp4"),
    );
    let joined = args.join(" ");
    assert!(joined.contains("setpts=PTS-STARTPTS"));
    assert!(joined.contains("aresample=async=1:first_pts=0"));
    assert!(joined.contains("-fps_mode cfr"));
    assert!(joined.contains("-r 60"));
    assert!(joined.contains("-color_range tv"));
    assert!(joined.contains("-colorspace bt709"));
    assert!(joined.contains("-color_primaries bt709"));
    assert!(joined.contains("-color_trc iec61966-2-1"));
}

#[test]
fn parses_ffprobe_duration_from_json_output() {
    let json = r#"{"format":{"duration":"30.123456"}}"#;
    let duration = parse_ffprobe_duration_secs(json.as_bytes()).expect("duration should parse");
    assert!((duration - 30.123456).abs() < 0.0001);
}

#[test]
fn parses_fast_integrity_snapshot_from_combined_ffprobe_json() {
    let json = r#"{
        "format": {"duration": "30.0"},
        "streams": [
            {"index": 0, "codec_type": "video", "duration": "30.0", "avg_frame_rate": "60/1"},
            {"index": 1, "codec_type": "audio", "duration": "29.9", "avg_frame_rate": "0/0"}
        ],
        "packets": [
            {"stream_index": 0, "pts_time": "0.000"},
            {"stream_index": 0, "pts_time": "0.017"},
            {"stream_index": 1, "pts_time": "0.000"},
            {"stream_index": 1, "pts_time": "0.021"}
        ]
    }"#;
    let snapshot =
        parse_ffprobe_fast_integrity_snapshot(json.as_bytes()).expect("snapshot should parse");
    assert_eq!(snapshot.format_duration_secs, Some(30.0));
    assert_eq!(snapshot.video_duration_secs, Some(30.0));
    assert_eq!(snapshot.audio_duration_secs, Some(29.9));
    assert_eq!(snapshot.video_packet_pts_secs.len(), 2);
    assert_eq!(snapshot.audio_packet_pts_secs.len(), 2);
}

#[test]
fn fast_integrity_detection_flags_video_timestamp_regression() {
    let json = r#"{
        "format": {"duration": "10.0"},
        "streams": [
            {"index": 0, "codec_type": "video", "duration": "10.0", "avg_frame_rate": "60/1"},
            {"index": 1, "codec_type": "audio", "duration": "10.0", "avg_frame_rate": "0/0"}
        ],
        "packets": [
            {"stream_index": 0, "pts_time": "0.000"},
            {"stream_index": 0, "pts_time": "0.033"},
            {"stream_index": 0, "pts_time": "0.020"},
            {"stream_index": 1, "pts_time": "0.000"},
            {"stream_index": 1, "pts_time": "0.021"}
        ]
    }"#;
    let snapshot =
        parse_ffprobe_fast_integrity_snapshot(json.as_bytes()).expect("snapshot should parse");
    let issue = detect_fast_integrity_issue(&snapshot, 60).expect("expected issue");
    assert_eq!(issue.detected_code, "video_timestamp_warning");
}

#[test]
fn fast_integrity_detection_flags_audio_video_drift() {
    let json = r#"{
        "format": {"duration": "12.0"},
        "streams": [
            {"index": 0, "codec_type": "video", "duration": "12.0", "avg_frame_rate": "60/1"},
            {"index": 1, "codec_type": "audio", "duration": "11.2", "avg_frame_rate": "0/0"}
        ],
        "packets": [
            {"stream_index": 0, "pts_time": "0.000"},
            {"stream_index": 0, "pts_time": "0.017"},
            {"stream_index": 1, "pts_time": "0.000"},
            {"stream_index": 1, "pts_time": "0.021"}
        ]
    }"#;
    let snapshot =
        parse_ffprobe_fast_integrity_snapshot(json.as_bytes()).expect("snapshot should parse");
    let issue = detect_fast_integrity_issue(&snapshot, 60).expect("expected issue");
    assert_eq!(issue.detected_code, "av_drift_warning");
}

#[test]
fn fast_integrity_detection_flags_cadence_drift() {
    let json = r#"{
        "format": {"duration": "8.0"},
        "streams": [
            {"index": 0, "codec_type": "video", "duration": "8.0", "avg_frame_rate": "30/1"},
            {"index": 1, "codec_type": "audio", "duration": "8.0", "avg_frame_rate": "0/0"}
        ],
        "packets": [
            {"stream_index": 0, "pts_time": "0.000"},
            {"stream_index": 0, "pts_time": "0.033"},
            {"stream_index": 0, "pts_time": "0.066"},
            {"stream_index": 1, "pts_time": "0.000"},
            {"stream_index": 1, "pts_time": "0.021"}
        ]
    }"#;
    let snapshot =
        parse_ffprobe_fast_integrity_snapshot(json.as_bytes()).expect("snapshot should parse");
    let issue = detect_fast_integrity_issue(&snapshot, 60).expect("expected issue");
    assert_eq!(issue.detected_code, "fps_drift_warning");
}
