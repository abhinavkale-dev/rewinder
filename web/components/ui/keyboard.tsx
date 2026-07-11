"use client";

import { cx } from "@/lib/utils";
import type { KeyDef } from "../keyboardLayout";

export function Keycap({
  def,
  highlighted,
  pressed,
  onPress,
}: {
  def: KeyDef;
  highlighted?: boolean;
  pressed?: boolean;
  onPress?: () => void;
}) {
  return (
    <button
      type="button"
      tabIndex={-1}
      className={cx(
        "keycap",
        def.modifier && "keycap-modifier",
        highlighted && "keycap-highlight",
        pressed && "keycap-pressed"
      )}
      style={{ flexGrow: def.width ?? 1, flexBasis: 0 }}
      onPointerDown={onPress}
      aria-label={def.label || "space"}
    >
      {def.sub && <span className="keycap-sub">{def.sub}</span>}
      <span className="keycap-label">{def.label}</span>
    </button>
  );
}
