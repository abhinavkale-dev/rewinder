import { AppPreview } from "./AppPreview";

export function Hero() {
  return (
    <section className="hero">
      <div className="badge">macOS 26+ · Apple Silicon</div>
      <h1>
        Never miss <em>the moment</em>
      </h1>
      <p>
        Rewinder keeps a rolling replay of your Mac. When something worth
        keeping happens, press a hotkey and save the last few minutes as a
        clip.
      </p>
      <div className="cta-row">
        <a
          className="btn btn-primary"
          href="https://github.com/abhinavkale-dev/rewinder/releases"
        >
          Download for Mac
        </a>
        <a
          className="btn btn-ghost"
          href="https://github.com/abhinavkale-dev/rewinder"
        >
          View source
        </a>
      </div>
      <div className="hint">
        Default hotkey: <kbd>&#8984;</kbd> <kbd>&#8679;</kbd> <kbd>S</kbd>
      </div>
      <AppPreview />
    </section>
  );
}
