use bytes::Bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Video,
    Audio,
}

#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub stream: StreamKind,
    pub data: Bytes,
    pub pts: i64,
    pub dts: i64,
    pub duration: i64,
    pub is_keyframe: bool,
    pub seq: u64,
}
