"use client";

import { QUALITY_PRESETS, type QualityPresetId } from "./constants";
import { cx } from "@/lib/utils";

export function QualitySegment({
  value,
  onChange,
}: {
  value: QualityPresetId;
  onChange: (id: QualityPresetId) => void;
}) {
  return (
    <div className="quality-segment" role="radiogroup" aria-label="Quality">
      {QUALITY_PRESETS.map((preset) => (
        <button
          key={preset.id}
          type="button"
          role="radio"
          aria-checked={value === preset.id}
          className={cx("quality-option", value === preset.id && "quality-option-active")}
          onClick={() => onChange(preset.id)}
        >
          {preset.label}
        </button>
      ))}
    </div>
  );
}
