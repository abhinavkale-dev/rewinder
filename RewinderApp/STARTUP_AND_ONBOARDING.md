# Startup & Onboarding Changes

Summary of the current first-launch and boot UI work in the native Swift app.

## What changed

Three related pieces shipped together:

1. **Boot splash** — ring-buffer assembly while the engine starts
2. **Three-step onboarding** — owl wake-up → Get Started → permissions
3. **Light mode support** — splash and onboarding follow system appearance

No engine / capture pipeline changes. UI only.

---

## Launch flow

```
App open
  │
  ├─ LoadingView (boot splash)
  │     ring-buffer sweep + R mark + "Rewinder"
  │     held ~1.15s, then crossfades to Home when engine is ready
  │
  └─ OnboardingView (first launch only)
        Step 1  Owl wake-up intro (~2.9s, tap to skip)
        Step 2  "Welcome to Rewinder" + Get Started
        Step 3  Permissions (Screen Recording required, Mic optional)
              → Continue → Home
```

Onboarding is gated by `hasCompletedOnboarding` (`@AppStorage`). After Continue, it never shows again unless that flag is reset.

Replay onboarding locally:

```bash
defaults write com.rewinder.app hasCompletedOnboarding -bool false
open RewinderApp/build/Rewinder.app
```

---

## 1. Boot splash (`LoadingView`)

**Files**

- [`Sources/RewinderApp/Views/LoadingView.swift`](Sources/RewinderApp/Views/LoadingView.swift)
- [`Sources/RewinderApp/Components/BufferRing.swift`](Sources/RewinderApp/Components/BufferRing.swift)
- [`Sources/RewinderApp/Views/ContentView.swift`](Sources/RewinderApp/Views/ContentView.swift)

**Behavior**

- 16 accent arc segments land clockwise on a circular track (same geometry as the Home `PowerButton` ring: 156pt, 7pt stroke).
- Older segments dim to 35% as the write head advances — the ring buffer overwriting itself.
- R mark fades in at center; "Rewinder" + status line fade in below.
- If the engine is still booting after the storyboard, the head keeps cycling calmly (no spinner).
- `ContentView` keeps the splash mounted until **both** engine state is ready **and** a ~1.15s minimum hold has elapsed, then crossfades to Home (250ms).
- Reduce Motion / boot error: skip the hold, reveal statically, no sweep loop.

**Light mode**

- Uses adaptive `Theme.appBackground` (not a fixed dark splash).
- Wordmark / status use `.primary` / `.secondary`.

---

## 2. Three-step onboarding (`OnboardingView`)

**Files**

- [`Sources/RewinderApp/Views/OnboardingView.swift`](Sources/RewinderApp/Views/OnboardingView.swift)
- [`Sources/RewinderApp/Branding/RewinderOwlLogo.swift`](Sources/RewinderApp/Branding/RewinderOwlLogo.swift)

### Step 1 — Owl wake-up intro

Replaces the earlier generic ring / warp burst around the owl.

Storyboard (~2.9s):

| Time   | Beat |
|--------|------|
| 0ms    | Owl fades in asleep (eyes closed, slight slouch) |
| 600ms  | Sleepy half-blink |
| 1050ms | Eyes spring open, head lifts |
| 1450ms | Pupils glance left |
| 1900ms | Pupils glance right |
| 2350ms | Pupils settle; two accent ripples converge *into* the owl |
| 2900ms | Auto-advance to Get Started |

- Tap anywhere during Step 1 skips to Get Started.
- Reduce Motion: static awake owl, single fade, shorter hold (~1.5s), no blinks / glances / ripples.

Owl animation knobs (defaults keep the normal logo look elsewhere):

- `eyeOpenness` — vertical squash of each eye group (cartoon blink)
- `pupilShift` — horizontal iris + catch-light offset (glance)

### Step 2 — Get Started

- Owl stays large and centered
- Title: **Welcome to Rewinder** (no personalized name)
- Subtitle: "Your screen's last moments, always ready to save."
- **Get Started** button → Step 3

### Step 3 — Permissions (unchanged flow)

- Owl shrinks to the top
- Screen Recording (required) + Microphone (optional)
- Privacy note + Continue
- Continue blocked (shake) until Screen Recording is granted
- If mic is skipped, settings patch to `system_only` / mic off

---

## 3. Light mode compatibility

| Surface | Change |
|---------|--------|
| Boot splash | Adaptive `Theme.appBackground`; semantic text colors |
| Onboarding | Already used semantic colors + `Theme.appBackground` |
| Theme | Removed unused fixed `splashBackground` |
| Owl sticker | Works on light and dark (white face + shadow) |

---

## Key files touched

| File | Role |
|------|------|
| `Views/LoadingView.swift` | Boot splash storyboard |
| `Views/ContentView.swift` | Splash hold + crossfade handoff |
| `Views/OnboardingView.swift` | 3-step flow + owl wake-up storyboard |
| `Branding/RewinderOwlLogo.swift` | `eyeOpenness` / `pupilShift` |
| `Components/BufferRing.swift` | Shared `BufferRingSweep` for splash |
| `Components/Theme.swift` | Adaptive app background; splash constant removed |

---

## Accessibility

- **Reduce Motion**: splash and intro collapse to short static fades; no loops, blinks, glances, or ripples.
- Boot errors never wait on animation timing.
- Tap-to-skip on the intro cancels the storyboard task cleanly.

---

## How to verify

```bash
cd RewinderApp
swift build
./scripts/package_app.sh
defaults write com.rewinder.app hasCompletedOnboarding -bool false
open build/Rewinder.app
```

Check:

1. Boot splash ring → Home (or onboarding overlay on first run)
2. Owl asleep → wake → glance → Get Started → permissions
3. System light / dark appearance: backgrounds and text stay legible
4. Reduce Motion: no sweep / blink choreography
