"use client";

import { useEffect, useState } from "react";
import { useCountUp, useInView } from "@/lib/hooks";

const REPO = "abhinavkale-dev/rewinder";
const FALLBACK_STARS = 14;

export function GithubStarMorph() {
  const [stars, setStars] = useState<number>(FALLBACK_STARS);
  const { ref, inView } = useInView<HTMLAnchorElement>(0.5);
  const display = useCountUp(stars, inView);

  useEffect(() => {
    let cancelled = false;
    fetch(`https://api.github.com/repos/${REPO}`)
      .then((res) => (res.ok ? res.json() : null))
      .then((data) => {
        if (!cancelled && data && typeof data.stargazers_count === "number") {
          setStars(data.stargazers_count);
        }
      })
      .catch(() => {
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <a ref={ref} className="star-pill" href={`https://github.com/${REPO}`}>
      <span className="star-glyph">&#9733;</span>
      <span className="star-count">{display.toLocaleString()}</span>
      <span className="star-label">on GitHub</span>
    </a>
  );
}
