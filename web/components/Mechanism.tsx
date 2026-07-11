const STEPS = [
  {
    step: "01",
    title: "Capture",
    body: "A ScreenCaptureKit helper streams encoded frames into a rolling in-memory buffer sized to your replay window.",
    detail: "sck_capture helper \u2192 encoded frames",
  },
  {
    step: "02",
    title: "Hold",
    body: "The buffer holds the most recent minutes only. Old frames fall off the back; nothing is ever written silently.",
    detail: "ring buffer \u2192 fixed memory budget",
  },
  {
    step: "03",
    title: "Save",
    body: "On your hotkey, the buffer snapshot is muxed into an MP4 with audio and dropped into your clips folder.",
    detail: "snapshot \u2192 H.264 MP4 + AAC",
  },
];

export function Mechanism() {
  return (
    <section className="mechanism" id="how-it-works">
      <h2>How it works</h2>
      <p className="section-lede">
        Three stages, all on-device. The engine is a Rust static library driven
        by a native SwiftUI shell — no web runtime in the app.
      </p>
      <div className="mechanism-steps">
        {STEPS.map((s, i) => (
          <div className="mechanism-step" key={s.step}>
            <div className="step-header">
              <span className="step-number">{s.step}</span>
              {i < STEPS.length - 1 && <span className="step-arrow">&rarr;</span>}
            </div>
            <h3>{s.title}</h3>
            <p>{s.body}</p>
            <code className="step-detail">{s.detail}</code>
          </div>
        ))}
      </div>
    </section>
  );
}
