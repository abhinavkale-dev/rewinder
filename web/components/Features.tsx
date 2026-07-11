import { BufferTimeline } from "./BufferTimeline";
import { SimAppWindows } from "./SimAppWindows";
import { TrayMenu } from "./TrayMenu";

const CARDS = [
  {
    title: "Mic with noise removal",
    body: "Optional microphone track with built-in RNNoise denoising, mixed straight into your saved clips.",
  },
  {
    title: "Stays out of the way",
    body: "Lives in the menu bar with a tiny footprint. Start it once and forget it is running until you need it.",
  },
  {
    title: "Private by design",
    body: "Everything happens on-device. No accounts, no uploads, no telemetry. Your clips belong to you.",
  },
  {
    title: "Plain MP4 output",
    body: "Clips land in a folder you pick as standard H.264 MP4s. Drag them into iMessage, Slack, or an editor.",
  },
  {
    title: "Wired for Apple Silicon",
    body: "Hardware-accelerated encode via VideoToolbox keeps CPU usage low even at 60 fps capture.",
  },
  {
    title: "Open source",
    body: "The whole app — SwiftUI shell and Rust engine — is on GitHub. Read it, build it, fork it.",
  },
];

export function Features() {
  return (
    <section className="features" id="features">
      <div className="feature-row">
        <div className="feature-copy">
          <span className="eyebrow">The problem</span>
          <h2>The best moments never announce themselves</h2>
          <p>
            The clutch play, the one-in-a-hundred bug, the exact sentence a
            client said — by the time you reach for a recorder, it already
            happened. Recording everything all day is the wrong answer:
            gigabytes of footage you will never scrub through.
          </p>
        </div>
        <div className="feature-demo">
          <SimAppWindows />
        </div>
      </div>

      <div className="feature-row feature-row-flip">
        <div className="feature-copy">
          <span className="eyebrow">The fix</span>
          <h2>A replay buffer, not a recorder</h2>
          <p>
            Rewinder holds the last few minutes in memory and continuously
            forgets everything older. Press the hotkey and only that window is
            written to disk — a small, shareable MP4 of exactly the part that
            mattered.
          </p>
        </div>
        <div className="feature-demo">
          <BufferTimeline />
        </div>
      </div>

      <div className="feature-row">
        <div className="feature-copy">
          <span className="eyebrow">Always at hand</span>
          <h2>Runs from the menu bar</h2>
          <p>
            No dock icon, no window to manage. A quiet menu-bar item shows
            capture status, and everything — saving, settings, quitting — is
            two clicks or one hotkey away.
          </p>
        </div>
        <div className="feature-demo feature-demo-center">
          <TrayMenu />
        </div>
      </div>

      <div className="grid">
        {CARDS.map((card) => (
          <div className="card" key={card.title}>
            <h3>{card.title}</h3>
            <p>{card.body}</p>
          </div>
        ))}
      </div>
    </section>
  );
}
