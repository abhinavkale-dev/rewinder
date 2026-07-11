"use client";

import { useState } from "react";
import { FinderMenuBar } from "./FinderMenuBar";
import { PowerRing } from "./PowerRing";
import { QualitySegment } from "./QualitySegment";
import { REPLAY_WINDOWS, DEFAULT_HOTKEY, type QualityPresetId } from "./constants";
import { formatDuration } from "@/lib/utils";

export function RewinderWindow() {
  const [capturing, setCapturing] = useState(true);
  const [quality, setQuality] = useState<QualityPresetId>("balanced");
  const [windowSecs, setWindowSecs] = useState<number>(120);

  return (
    <div className="rewinder-window">
      <FinderMenuBar />
      <div className="window-chrome">
        <div className="traffic-lights">
          <span className="light light-close" />
          <span className="light light-min" />
          <span className="light light-max" />
        </div>
        <span className="window-title">Rewinder</span>
      </div>

      <div className="window-body">
        <PowerRing active={capturing} onToggle={() => setCapturing((c) => !c)} />
        <div className="capture-status">
          {capturing ? "Capturing — replay ready" : "Paused"}
        </div>

        <div className="control-row">
          <span className="control-label">Replay window</span>
          <div className="window-picker">
            {REPLAY_WINDOWS.map((secs) => (
              <button
                key={secs}
                type="button"
                className={secs === windowSecs ? "pick pick-active" : "pick"}
                onClick={() => setWindowSecs(secs)}
              >
                {formatDuration(secs)}
              </button>
            ))}
          </div>
        </div>

        <div className="control-row">
          <span className="control-label">Quality</span>
          <QualitySegment value={quality} onChange={setQuality} />
        </div>

        <div className="hotkey-row">
          Save clip:
          {DEFAULT_HOTKEY.map((key) => (
            <kbd key={key}>{key}</kbd>
          ))}
        </div>
      </div>
    </div>
  );
}
