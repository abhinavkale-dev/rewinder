"use client";

import { useRef, useState } from "react";
import { cx } from "@/lib/utils";

interface DockApp {
  id: string;
  name: string;
  glyph: string;
  tint: string;
  running?: boolean;
}

const APPS: DockApp[] = [
  { id: "finder", name: "Finder", glyph: "F", tint: "#3b82f6" },
  { id: "safari", name: "Safari", glyph: "S", tint: "#0ea5e9" },
  { id: "terminal", name: "Terminal", glyph: ">_", tint: "#27272a" },
  { id: "rewinder", name: "Rewinder", glyph: "R", tint: "#e8543f", running: true },
  { id: "music", name: "Music", glyph: "M", tint: "#ec4899" },
  { id: "notes", name: "Notes", glyph: "N", tint: "#eab308" },
];

const BASE = 48;
const MAX_SCALE = 1.7;
const RADIUS = 110;

export function MacOSDock() {
  const trackRef = useRef<HTMLDivElement>(null);
  const [cursorX, setCursorX] = useState<number | null>(null);
  const [active, setActive] = useState<string | null>("rewinder");

  const scaleFor = (index: number) => {
    if (cursorX === null || !trackRef.current) return 1;
    const slotCenter = index * (BASE + 14) + BASE / 2 + 10;
    const distance = Math.abs(cursorX - slotCenter);
    if (distance > RADIUS) return 1;
    const t = 1 - distance / RADIUS;
    return 1 + (MAX_SCALE - 1) * t * t;
  };

  return (
    <div className="dock-wrap">
      <div
        ref={trackRef}
        className="dock"
        onPointerMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          setCursorX(e.clientX - rect.left);
        }}
        onPointerLeave={() => setCursorX(null)}
      >
        {APPS.map((app, i) => {
          const scale = scaleFor(i);
          return (
            <button
              key={app.id}
              type="button"
              className="dock-item"
              onClick={() => setActive(app.id)}
              aria-label={app.name}
            >
              <span className="dock-tooltip" style={{ opacity: scale > 1.3 ? 1 : 0 }}>
                {app.name}
              </span>
              <span
                className={cx("dock-icon", app.id === "rewinder" && "dock-icon-rewinder")}
                style={{
                  width: BASE * scale,
                  height: BASE * scale,
                  background: app.tint,
                }}
              >
                {app.glyph}
              </span>
              <span className={cx("dock-dot", (app.running || active === app.id) && "dock-dot-on")} />
            </button>
          );
        })}
      </div>
    </div>
  );
}
