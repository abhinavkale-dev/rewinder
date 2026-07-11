"use client";

import { useEffect, useState } from "react";
import { useInView } from "@/lib/hooks";
import { cx } from "@/lib/utils";

interface Scenario {
  id: string;
  app: string;
  line1: string;
  line2: string;
  accent: string;
}

const SCENARIOS: Scenario[] = [
  {
    id: "game",
    app: "Game — ranked final",
    line1: "Triple elimination with 12 HP left.",
    line2: "You were never going to press record in time.",
    accent: "#8b5cf6",
  },
  {
    id: "bug",
    app: "Terminal — flaky test",
    line1: "The race condition that only happens on Tuesdays.",
    line2: "Save the repro the moment it flashes past.",
    accent: "#22c55e",
  },
  {
    id: "call",
    app: "Video call — demo",
    line1: "Client said yes to the redesign on the spot.",
    line2: "Keep the exact quote without recording an hour.",
    accent: "#3b82f6",
  },
];

const ROTATE_MS = 3600;

export function SimAppWindows() {
  const { ref, inView } = useInView<HTMLDivElement>(0.4);
  const [active, setActive] = useState(0);

  useEffect(() => {
    if (!inView) return;
    const interval = setInterval(() => {
      setActive((a) => (a + 1) % SCENARIOS.length);
    }, ROTATE_MS);
    return () => clearInterval(interval);
  }, [inView]);

  return (
    <div ref={ref} className="sim-windows">
      {SCENARIOS.map((scenario, i) => {
        const offset = (i - active + SCENARIOS.length) % SCENARIOS.length;
        return (
          <div
            key={scenario.id}
            className={cx("sim-window", offset === 0 && "sim-window-front")}
            style={{
              transform: `translateY(${offset * 14}px) scale(${1 - offset * 0.05})`,
              zIndex: SCENARIOS.length - offset,
              opacity: offset === 2 ? 0.35 : 1,
            }}
          >
            <div className="sim-titlebar">
              <span className="sim-dot" style={{ background: scenario.accent }} />
              {scenario.app}
            </div>
            <div className="sim-body">
              <p>{scenario.line1}</p>
              <p className="sim-muted">{scenario.line2}</p>
            </div>
            <div className="sim-hotkey-hint">
              <kbd>&#8984;</kbd>
              <kbd>&#8679;</kbd>
              <kbd>S</kbd>
              <span>saved it</span>
            </div>
          </div>
        );
      })}
      <div className="sim-dots">
        {SCENARIOS.map((s, i) => (
          <button
            key={s.id}
            type="button"
            className={i === active ? "sim-pager sim-pager-on" : "sim-pager"}
            onClick={() => setActive(i)}
            aria-label={s.app}
          />
        ))}
      </div>
    </div>
  );
}
