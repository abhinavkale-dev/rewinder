export const theme = {
  bg: "#0a0a0c",
  surface: "#131318",
  border: "#26262e",
  text: "#ededf0",
  muted: "#8b8b96",
  accent: "#e8543f",
  accentSoft: "rgba(232, 84, 63, 0.14)",
} as const;

export type ThemeColor = keyof typeof theme;
