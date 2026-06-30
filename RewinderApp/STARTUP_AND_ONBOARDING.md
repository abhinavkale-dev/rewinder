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
