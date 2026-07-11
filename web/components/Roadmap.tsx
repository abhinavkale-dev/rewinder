const MILESTONES = [
  {
    tag: "shipped",
    title: "Core replay engine",
    body: "Rolling buffer, hotkey save, SwiftUI shell over the Rust engine.",
  },
  {
    tag: "shipped",
    title: "Mic + noise removal",
    body: "Optional microphone track denoised with RNNoise, mixed into clips.",
  },
  {
    tag: "in progress",
    title: "Signed, notarized releases",
    body: "One-command DMG pipeline so every release installs without warnings.",
  },
  {
    tag: "planned",
    title: "Per-app capture filters",
    body: "Exclude password managers and private windows from the buffer.",
  },
  {
    tag: "planned",
    title: "Trim before save",
    body: "Quick in-buffer trim so the clip starts exactly where you want.",
  },
];

export function Roadmap() {
  return (
    <section className="roadmap" id="roadmap">
      <h2>Where this is going</h2>
      <div className="roadmap-list">
        {MILESTONES.map((m) => (
          <div className="roadmap-item" key={m.title}>
            <span className={`roadmap-tag roadmap-tag-${m.tag.replace(" ", "-")}`}>{m.tag}</span>
            <div>
              <h3>{m.title}</h3>
              <p>{m.body}</p>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
