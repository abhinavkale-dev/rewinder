const TIERS = [
  {
    name: "Beta",
    price: "Free",
    cadence: "while in development",
    highlight: true,
    cta: "Download for Mac",
    href: "https://github.com/abhinavkale-dev/rewinder/releases",
    features: [
      "Full replay buffer, up to 5 minutes",
      "60 fps capture with system audio",
      "Mic track with noise removal",
      "All quality presets",
      "No account, no telemetry",
    ],
  },
  {
    name: "Pro",
    price: "TBD",
    cadence: "one-time purchase, planned",
    highlight: false,
    cta: "Watch the repo",
    href: "https://github.com/abhinavkale-dev/rewinder",
    features: [
      "Everything in Beta",
      "Longer replay windows",
      "Per-app capture filters",
      "Clip trimming before save",
      "Priority support",
    ],
  },
  {
    name: "Build it yourself",
    price: "Free",
    cadence: "forever, from source",
    highlight: false,
    cta: "Read the build guide",
    href: "https://github.com/abhinavkale-dev/rewinder#readme",
    features: [
      "Full source on GitHub",
      "Swift + Rust toolchain required",
      "Sign with your own certificate",
      "Same app, your build",
      "Contributions welcome",
    ],
  },
];

export function Pricing() {
  return (
    <section className="pricing" id="pricing">
      <h2>Free while it bakes</h2>
      <p className="section-lede">
        The beta is free and fully functional. A paid Pro tier is planned so
        the project can sustain itself &mdash; the core will stay free.
      </p>
      <div className="pricing-grid">
        {TIERS.map((tier) => (
          <div className={tier.highlight ? "tier tier-highlight" : "tier"} key={tier.name}>
            {tier.highlight && <span className="tier-badge">Available now</span>}
            <h3>{tier.name}</h3>
            <div className="tier-price">
              {tier.price}
              <span className="tier-cadence"> &middot; {tier.cadence}</span>
            </div>
            <ul className="tier-features">
              {tier.features.map((feature) => (
                <li key={feature}>{feature}</li>
              ))}
            </ul>
            <a className={tier.highlight ? "btn btn-primary" : "btn btn-ghost"} href={tier.href}>
              {tier.cta}
            </a>
          </div>
        ))}
      </div>
    </section>
  );
}
