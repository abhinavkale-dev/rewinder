"use client";

import { useState } from "react";
import { cx } from "@/lib/utils";

const QUESTIONS = [
  {
    q: "Does Rewinder record all the time?",
    a: "It captures into a fixed-size memory buffer. Nothing is saved to disk until you press the hotkey, and the buffer is discarded when you quit. There is no library of silent recordings building up anywhere.",
  },
  {
    q: "How much does it cost?",
    a: "Rewinder is free while in development. The source is available on GitHub under the repository license.",
  },
  {
    q: "What Macs are supported?",
    a: "Apple Silicon Macs running macOS 26 or newer. Intel Macs are not supported — the engine leans on hardware encode paths that are only tuned for Apple Silicon.",
  },
  {
    q: "Does it capture audio?",
    a: "Yes. System audio is captured alongside video, and you can enable an optional microphone track with built-in noise removal.",
  },
  {
    q: "How much memory does the buffer use?",
    a: "It scales with your replay window and quality preset. A 2-minute balanced-quality window typically stays under a few hundred megabytes.",
  },
  {
    q: "Where do clips go?",
    a: "Into a folder you choose. Clips are plain MP4 files you can share anywhere — no proprietary format, no export step.",
  },
  {
    q: "Is anything uploaded?",
    a: "No. There are no accounts, no cloud, and no telemetry. The only network request the app makes is the optional update check.",
  },
];

export function Faq() {
  const [open, setOpen] = useState<number | null>(0);

  return (
    <section className="faq" id="faq">
      <h2>Questions, answered</h2>
      <div className="faq-list">
        {QUESTIONS.map((item, i) => {
          const isOpen = open === i;
          return (
            <div className={cx("faq-item", isOpen && "faq-item-open")} key={item.q}>
              <button
                type="button"
                className="faq-question"
                aria-expanded={isOpen}
                onClick={() => setOpen(isOpen ? null : i)}
              >
                {item.q}
                <span className="faq-chevron">{isOpen ? "\u2212" : "+"}</span>
              </button>
              <div className="faq-answer" style={{ gridTemplateRows: isOpen ? "1fr" : "0fr" }}>
                <div className="faq-answer-inner">
                  <p>{item.a}</p>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
