const QUOTES = [
  {
    quote:
      "Deleted my old screen recorder the same day. I kept 40 GB of recordings around just in case — now I save 20 MB clips of the parts that matter.",
    name: "Beta tester",
    role: "indie game developer",
  },
  {
    quote:
      "Caught a heisenbug on the first day. It flashed a broken frame once in four hours and the replay buffer had it.",
    name: "Early user",
    role: "macOS engineer",
  },
  {
    quote:
      "The hotkey becomes muscle memory in about an hour. Something cool happens, my hand just does it.",
    name: "Beta tester",
    role: "speedrunner",
  },
];

export function Testimonials() {
  return (
    <section className="testimonials">
      <h2>From the beta</h2>
      <div className="quote-grid">
        {QUOTES.map((item) => (
          <figure className="quote-card" key={item.quote}>
            <blockquote>&ldquo;{item.quote}&rdquo;</blockquote>
            <figcaption>
              {item.name} <span>&middot; {item.role}</span>
            </figcaption>
          </figure>
        ))}
      </div>
    </section>
  );
}
