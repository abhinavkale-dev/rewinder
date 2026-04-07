use std::collections::VecDeque;

use crate::encoding::packet::{EncodedPacket, StreamKind};

#[derive(Debug)]
pub struct VideoBuffer {
    packets: VecDeque<EncodedPacket>,
    window_us: i64,
}

impl VideoBuffer {
    pub fn new(window_us: i64) -> Self {
        Self {
            packets: VecDeque::new(),
            window_us,
        }
    }

    pub fn set_window_us(&mut self, window_us: i64) {
        self.window_us = window_us;
        self.evict_old();
    }

    pub fn push(&mut self, packet: EncodedPacket) {
        if packet.stream != StreamKind::Video {
            return;
        }
        self.packets.push_back(packet);
        self.evict_old();
    }

    pub fn snapshot_last_us(&self, duration_us: i64) -> Vec<EncodedPacket> {
        let Some(last) = self.packets.back() else {
            return Vec::new();
        };

        let cutoff = last.pts - duration_us.max(0);
        self.packets
            .iter()
            .filter(|packet| packet.pts >= cutoff)
            .cloned()
            .collect()
    }

    pub fn fill_secs(&self) -> f32 {
        let Some(first) = self.packets.front() else {
            return 0.0;
        };
        let Some(last) = self.packets.back() else {
            return 0.0;
        };

        let span = (last.pts + last.duration - first.pts).max(0) as f32;
        span / 1_000_000.0
    }

    fn evict_old(&mut self) {
        let Some(last) = self.packets.back() else {
            return;
        };
        let cutoff = last.pts - self.window_us;

        while let Some(front) = self.packets.front() {
            if front.pts < cutoff {
                let _ = self.packets.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::encoding::packet::EncodedPacket;

    #[test]
    fn evicts_by_window() {
        let mut buffer = VideoBuffer::new(2_000_000);
        for i in 0..5 {
            buffer.push(EncodedPacket {
                stream: StreamKind::Video,
                data: Bytes::from_static(&[1]),
                pts: i * 1_000_000,
                dts: i * 1_000_000,
                duration: 1_000_000,
                is_keyframe: i % 2 == 0,
                seq: i as u64,
            });
        }

        let packets = buffer.snapshot_last_us(10_000_000);
        assert!(packets.len() <= 3);
    }
}
