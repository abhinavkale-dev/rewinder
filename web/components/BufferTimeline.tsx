"use client";

import { useEffect, useState } from "react";
import { useInView } from "@/lib/hooks";
import { cx } from "@/lib/utils";

const SEGMENTS = 24;
const TICK_MS = 240;

export function BufferTimeline() {
  const { ref, inView } = useInView<HTMLDivElement>(0.4);
  const [head, setHead] = useState(0);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!inView) return;
    const interval = setInterval(() => {
      setHead((h) => {
        const next = h + 1;
        if (next % (SEGMENTS * 2) === 0) {
          setSaving(true);
          setTimeout(() => setSaving(false), 900);
        }
        return next;
      });
    }, TICK_MS);
    return () => clearInterval(interval);
  }, [inView]);

  return (
    <div ref={ref} className={cx("buffer-timeline", saving && "buffer-saving")}>
      <div className="buffer-track">
        {Array.from({ length: SEGMENTS }, (_, i) => {
          const age = (head - i + SEGMENTS) % SEGMENTS;
          const filled = age < SEGMENTS - 4;
          const fresh = age < 3;
          return (
            <span
              key={i}
              className={cx("buffer-cell", filled && "buffer-cell-filled", fresh && "buffer-cell-fresh")}
            />
          );
        })}
      </div>
      <div className="buffer-caption">
        <span>&minus;2:00</span>
        <span className={saving ? "buffer-flash buffer-flash-on" : "buffer-flash"}>
          {saving ? "clip saved" : "rolling buffer"}
        </span>
        <span>now</span>
      </div>
    </div>
  );
}
