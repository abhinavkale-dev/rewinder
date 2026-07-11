"use client";

import { useCountUp, useInView } from "@/lib/hooks";
import { GithubStarMorph } from "./GithubStarMorph";

const STATS = [
  { value: 60, suffix: " fps", label: "capture, always" },
  { value: 5, suffix: " min", label: "max replay window" },
  { value: 0, suffix: "", label: "accounts required" },
];

function Stat({ value, suffix, label }: { value: number; suffix: string; label: string }) {
  const { ref, inView } = useInView<HTMLDivElement>(0.5);
  const display = useCountUp(value, inView, 900);
  return (
    <div ref={ref} className="proof-stat">
      <span className="proof-value">
        {display}
        {suffix}
      </span>
      <span className="proof-label">{label}</span>
    </div>
  );
}

export function SocialProofBar() {
  return (
    <section className="proof-bar">
      <GithubStarMorph />
      <div className="proof-divider" />
      {STATS.map((stat) => (
        <Stat key={stat.label} {...stat} />
      ))}
    </section>
  );
}
