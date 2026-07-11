"use client";

import { cx } from "@/lib/utils";

export function PowerRing({
  active,
  onToggle,
}: {
  active: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={cx("power-ring", active && "power-ring-active")}
      onClick={onToggle}
      aria-pressed={active}
      aria-label={active ? "Stop capturing" : "Start capturing"}
    >
      <span className="power-ring-core" />
    </button>
  );
}
