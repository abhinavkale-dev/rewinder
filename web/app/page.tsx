import { Hero } from "@/components/Hero";
import { SocialProofBar } from "@/components/SocialProofBar";
import { Features } from "@/components/Features";
import { UseCases } from "@/components/UseCases";
import { Mechanism } from "@/components/Mechanism";
import { ComparisonTable } from "@/components/ComparisonTable";
import { AppleKeyboard } from "@/components/AppleKeyboard";
import { PerfPanel } from "@/components/PerfPanel";
import { MacOSDock } from "@/components/MacOSDock";
import { Testimonials } from "@/components/Testimonials";
import { Pricing } from "@/components/Pricing";
import { Roadmap } from "@/components/Roadmap";
import { Faq } from "@/components/Faq";
import { FooterCta } from "@/components/FooterCta";
import { SiteFooter } from "@/components/SiteFooter";
import { RevealCopy } from "@/components/RevealCopy";

export default function Home() {
  return (
    <main>
      <div className="container">
        <nav className="nav">
          <div className="wordmark">
            rewinder<span>.</span>
          </div>
          <div className="nav-links">
            <a href="#features">Features</a>
            <a href="#how-it-works">How it works</a>
            <a href="#hotkey">Hotkey</a>
            <a href="#pricing">Pricing</a>
            <a href="#faq">FAQ</a>
            <a href="https://github.com/abhinavkale-dev/rewinder">GitHub</a>
          </div>
        </nav>

        <Hero />

        <SocialProofBar />

        <RevealCopy>
          <Features />
        </RevealCopy>

        <RevealCopy>
          <UseCases />
        </RevealCopy>

        <RevealCopy>
          <Mechanism />
        </RevealCopy>

        <RevealCopy>
          <ComparisonTable />
        </RevealCopy>

        <RevealCopy>
          <AppleKeyboard />
        </RevealCopy>

        <RevealCopy>
          <PerfPanel />
        </RevealCopy>

        <RevealCopy>
          <section className="dock-section">
            <h2>At home on your Mac</h2>
            <p className="section-lede">
              Native SwiftUI, not a wrapped web page. It behaves like the rest
              of your system because it is built like the rest of your system.
            </p>
            <MacOSDock />
          </section>
        </RevealCopy>

        <RevealCopy>
          <Testimonials />
        </RevealCopy>

        <RevealCopy>
          <Pricing />
        </RevealCopy>

        <RevealCopy>
          <Roadmap />
        </RevealCopy>

        <RevealCopy>
          <Faq />
        </RevealCopy>

        <RevealCopy>
          <FooterCta />
        </RevealCopy>

        <SiteFooter />
      </div>
    </main>
  );
}
