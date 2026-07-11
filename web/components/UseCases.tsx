"use client";

import { useState } from "react";
import { cx } from "@/lib/utils";

const CASES = [
  {
    id: "gamers",
    tab: "Gamers",
    title: "Clip the play, skip the VOD",
    body: "Ranked comeback, one-tap ace, physics glitch of the century. Save the highlight the second it lands instead of recording whole sessions and scrubbing for the good 40 seconds.",
    points: ["60 fps capture keeps fast motion readable", "System audio catches the comms", "Small MP4s drop straight into Discord"],
  },
  {
    id: "devs",
    tab: "Developers",
    title: "Catch the bug you cannot reproduce",
    body: "Flaky test, once-a-day render glitch, race condition that vanishes under a debugger. If it flashed on screen in the last few minutes, you have the repro on disk.",
    points: ["Attach clips to issues instead of prose", "Buffer survives across app restarts", "Menu-bar status shows it is armed"],
  },
  {
    id: "creators",
    tab: "Creators",
    title: "B-roll from moments, not sessions",
    body: "The unscripted reaction, the accidental perfect take. Keep working normally and harvest the moments afterward — no red dot changing how you behave.",
    points: ["No recording anxiety, no staging", "Clean MP4s ready for the editor", "Mic track with noise removal built in"],
  },
  {
    id: "teams",
    tab: "Remote workers",
    title: "Keep the decision, not the meeting",
    body: "The 30 seconds where the client approved the design, the exact repro a teammate demoed on a call. Save just that, share just that.",
    points: ["Clips beat rewatching hour-long recordings", "Nothing uploads anywhere by itself", "One hotkey mid-call, no interruption"],
  },
];

export function UseCases() {
  const [active, setActive] = useState(0);
  const current = CASES[active];

  return (
    <section className="use-cases" id="use-cases">
      <h2>Made for people who cannot ask the moment to wait</h2>
      <div className="case-tabs" role="tablist">
        {CASES.map((c, i) => (
          <button
            key={c.id}
            type="button"
            role="tab"
            aria-selected={i === active}
            className={cx("case-tab", i === active && "case-tab-on")}
            onClick={() => setActive(i)}
          >
            {c.tab}
          </button>
        ))}
      </div>
      <div className="case-panel" role="tabpanel">
        <h3>{current.title}</h3>
        <p>{current.body}</p>
        <ul>
          {current.points.map((point) => (
            <li key={point}>{point}</li>
          ))}
        </ul>
      </div>
    </section>
  );
}
