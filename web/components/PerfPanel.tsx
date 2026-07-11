"use client";

import { useInView } from "@/lib/hooks";
import { cx } from "@/lib/utils";

interface Meter {
  label: string;
  value: string;
  fill: number;
  note: string;
}

const METERS: Meter[] = [
  {
    label: "CPU while idle-capturing",
    value: "~3%",
    fill: 6,
    note: "of one performance core, 60 fps balanced preset",
  },
  {
    label: "Memory for a 2-min buffer",
    value: "< 300 MB",
    fill: 22,
    note: "fixed budget, never grows past the window",
  },
  {
    label: "Time from hotkey to MP4",
    value: "< 2 s",
    fill: 10,
    note: "snapshot and mux, no re-encode of the buffer",
  },
];

export function PerfPanel() {
  const { ref, inView } = useInView<HTMLDivElement>(0.75);

  return (
    <section className="perf" id="performance">
      <h2>Light enough to forget</h2>
      <p className="section-lede">
        A replay buffer only earns its keep if you can leave it running all
        day. Numbers below are from the performance report in the repo.
      </p>
      <div ref={ref} className="perf-panel">
        {METERS.map((meter) => (
          <div className="perf-meter" key={meter.label}>
            <div className="perf-row">
              <span className="perf-label">{meter.label}</span>
              <span className="perf-value">{meter.value}</span>
            </div>
            <div className="perf-track">
              <div
                className={cx("perf-fill", inView && "perf-fill-in")}
                style={{ width: inView ? `${meter.fill}%` : "0%" }}
              />
            </div>
            <span className="perf-note">{meter.note}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
