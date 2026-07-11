"use client";

import { useCallback, useEffect, useState } from "react";
import { KEYBOARD_ROWS, HOTKEY_CODES } from "./keyboardLayout";
import { Keycap } from "./ui/keyboard";
import { playSaveChime } from "./rewinder-window/sounds";

const HOTKEY_SEQUENCE = ["MetaLeft", "ShiftLeft", "KeyS"];
const STEP_MS = 420;
const RESET_MS = 1600;

export function AppleKeyboard() {
  const [pressed, setPressed] = useState<Set<string>>(new Set());
  const [looping, setLooping] = useState(true);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (!looping) return;
    let step = 0;
    let cancelled = false;
    const timers: ReturnType<typeof setTimeout>[] = [];

    const advance = () => {
      if (cancelled) return;
      if (step < HOTKEY_SEQUENCE.length) {
        const code = HOTKEY_SEQUENCE[step];
        setPressed((prev) => new Set(prev).add(code));
        step += 1;
        timers.push(setTimeout(advance, STEP_MS));
      } else {
        setSaved(true);
        timers.push(
          setTimeout(() => {
            if (cancelled) return;
            setPressed(new Set());
            setSaved(false);
            step = 0;
            timers.push(setTimeout(advance, STEP_MS));
          }, RESET_MS)
        );
      }
    };

    timers.push(setTimeout(advance, STEP_MS));
    return () => {
      cancelled = true;
      timers.forEach(clearTimeout);
    };
  }, [looping]);

  const pressKey = useCallback((code?: string) => {
    if (!code) return;
    setLooping(false);
    setPressed((prev) => {
      const next = new Set(prev);
      if (next.has(code)) {
        next.delete(code);
      } else {
        next.add(code);
      }
      return next;
    });
  }, []);

  useEffect(() => {
    if (looping) return;
    const hasCmd = pressed.has("MetaLeft") || pressed.has("MetaRight");
    const hasShift = pressed.has("ShiftLeft") || pressed.has("ShiftRight");
    if (hasCmd && hasShift && pressed.has("KeyS")) {
      setSaved(true);
      playSaveChime();
      const t = setTimeout(() => {
        setPressed(new Set());
        setSaved(false);
        setLooping(true);
      }, RESET_MS);
      return () => clearTimeout(t);
    }
  }, [pressed, looping]);

  return (
    <section className="keyboard-section" id="hotkey">
      <h2>One combo. That&apos;s the whole workflow.</h2>
      <p className="section-lede">
        No timelines to scrub, no record button to remember. Press the hotkey
        and the clip is already on disk. Try it below.
      </p>

      <div className="keyboard-stage">
        <div className={saved ? "save-toast save-toast-visible" : "save-toast"}>
          Clip saved — last 2 minutes
        </div>
        <div className="apple-keyboard" role="presentation">
          {KEYBOARD_ROWS.map((row, i) => (
            <div className="keyboard-row" key={i}>
              {row.map((def, j) => (
                <Keycap
                  key={`${i}-${j}`}
                  def={def}
                  highlighted={def.code ? HOTKEY_CODES.has(def.code) : false}
                  pressed={def.code ? pressed.has(def.code) : false}
                  onPress={() => pressKey(def.code)}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
