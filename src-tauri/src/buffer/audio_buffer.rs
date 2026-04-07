use std::collections::VecDeque;

use crate::encoding::packet::{EncodedPacket, StreamKind};

#[derive(Debug)]
pub struct AudioBuffer {
    packets: VecDeque<EncodedPacket>,
    window_us: i64,
}

impl AudioBuffer {
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
        if packet.stream != StreamKind::Audio {
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
