export const REPLAY_WINDOWS = [30, 60, 120, 300] as const;

export const QUALITY_PRESETS = [
  { id: "smooth", label: "Smooth", fps: 30, bitrate: "8 Mbps" },
  { id: "balanced", label: "Balanced", fps: 60, bitrate: "12 Mbps" },
  { id: "crisp", label: "Crisp", fps: 60, bitrate: "20 Mbps" },
] as const;

export type QualityPresetId = (typeof QUALITY_PRESETS)[number]["id"];

export const DEFAULT_HOTKEY = ["\u2318", "\u21e7", "S"];

export const WINDOW_SIZE = { width: 420, height: 560 };
