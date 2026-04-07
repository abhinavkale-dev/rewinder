use crate::encoding::packet::EncodedPacket;

#[derive(Debug, Clone)]
pub struct ReplaySnapshot {
    pub video_packets: Vec<EncodedPacket>,
    pub audio_packets: Vec<EncodedPacket>,
    pub duration_us: i64,
}

pub fn build_replay_snapshot(
    mut video_packets: Vec<EncodedPacket>,
    audio_packets: Vec<EncodedPacket>,
    replay_duration_us: i64,
) -> Result<ReplaySnapshot, String> {
    if video_packets.is_empty() {
        return Err("Buffer has no video packets yet".to_string());
    }

    let latest_pts = video_packets
        .last()
        .map(|packet| packet.pts)
        .ok_or_else(|| "Buffer has no video packets yet".to_string())?;

    let cutoff = latest_pts - replay_duration_us.max(0);
    video_packets.retain(|packet| packet.pts >= cutoff);

    if video_packets.is_empty() {
        return Err("No packets available in replay window".to_string());
    }

    let first_keyframe_index = video_packets
        .iter()
        .position(|packet| packet.is_keyframe)
        .ok_or_else(|| {
            "No keyframe found in replay window. Increase buffer duration or reduce GOP".to_string()
        })?;

    if first_keyframe_index > 0 {
        video_packets.drain(0..first_keyframe_index);
    }

    let first_pts = video_packets
        .first()
        .map(|packet| packet.pts)
        .ok_or_else(|| "Snapshot is empty after keyframe trim".to_string())?;

    let mut normalized_video = video_packets;
    for packet in &mut normalized_video {
        packet.pts -= first_pts;
        packet.dts -= first_pts;
    }

    let mut normalized_audio: Vec<EncodedPacket> = audio_packets
        .into_iter()
        .filter(|packet| packet.pts >= first_pts)
        .map(|mut packet| {
            packet.pts -= first_pts;
            packet.dts -= first_pts;
            packet
        })
        .collect();

    normalized_audio.sort_by_key(|packet| packet.pts);

    let video_end = normalized_video
        .last()
        .map(|packet| packet.pts + packet.duration)
        .unwrap_or(0);
    let audio_end = normalized_audio
        .last()
        .map(|packet| packet.pts + packet.duration)
        .unwrap_or(0);

    Ok(ReplaySnapshot {
        video_packets: normalized_video,
        audio_packets: normalized_audio,
        duration_us: video_end.max(audio_end),
    })
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::encoding::packet::{EncodedPacket, StreamKind};

    fn make_packet(stream: StreamKind, pts: i64, key: bool) -> EncodedPacket {
        EncodedPacket {
            stream,
            data: Bytes::from_static(&[1]),
            pts,
            dts: pts,
            duration: 1_000_000,
            is_keyframe: key,
            seq: pts as u64,
        }
    }

    #[test]
    fn trims_before_first_keyframe_and_rebases() {
        let video = vec![
            make_packet(StreamKind::Video, 1_000_000, false),
            make_packet(StreamKind::Video, 2_000_000, true),
            make_packet(StreamKind::Video, 3_000_000, false),
        ];
        let audio = vec![
            make_packet(StreamKind::Audio, 1_500_000, true),
            make_packet(StreamKind::Audio, 2_500_000, true),
        ];

        let snapshot = build_replay_snapshot(video, audio, 3_000_000).expect("snapshot");
        assert_eq!(snapshot.video_packets[0].pts, 0);
        assert!(snapshot.audio_packets[0].pts >= 0);
    }
}
