"use client";

const STEPS = [
  {
    title: "Grant screen recording",
    body: "Rewinder needs the macOS screen-recording permission to keep its replay buffer.",
  },
  {
    title: "Pick your window",
    body: "Choose how far back the replay reaches: 30 seconds up to 5 minutes.",
  },
  {
    title: "Learn the hotkey",
    body: "Press the save hotkey any time and the buffer is written out as a clip.",
  },
];

export function OnboardingOverlay({
  step,
  onNext,
  onSkip,
}: {
  step: number;
  onNext: () => void;
  onSkip: () => void;
}) {
  const current = STEPS[Math.min(step, STEPS.length - 1)];
  const isLast = step >= STEPS.length - 1;

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-card">
        <div className="onboarding-progress">
          {STEPS.map((_, i) => (
            <span key={i} className={i <= step ? "dot dot-on" : "dot"} />
          ))}
        </div>
        <h4>{current.title}</h4>
        <p>{current.body}</p>
        <div className="onboarding-actions">
          <button type="button" className="link-btn" onClick={onSkip}>
            Skip
          </button>
          <button type="button" className="btn btn-primary btn-sm" onClick={onNext}>
            {isLast ? "Done" : "Next"}
          </button>
        </div>
      </div>
    </div>
  );
}
