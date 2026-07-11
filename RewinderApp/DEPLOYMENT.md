# Deploying Rewinder (build a distributable .dmg)

This is the whole release process. Once the one-time setup is done, shipping a
new build is a single command.

## What you get

`scripts/release_dmg.sh` builds `Rewinder.app`, Developer ID-signs it, notarizes
+ staples it, wraps it in a `.dmg`, signs + notarizes + staples the dmg, and
writes `dist/Rewinder-<version>.dmg` (+ a `.sha256`). That dmg runs on any
Apple-Silicon Mac on macOS 26+ with no Gatekeeper warning.

## Prerequisites

- An Apple-Silicon Mac, macOS 26 (Tahoe), Xcode 26 toolchain (Swift 6.2+).
- Rust toolchain (`cargo`).
- A paid **Apple Developer Program** membership ($99/yr).
- `create-dmg` — **required for the production dmg**:
  ```bash
  brew install create-dmg
  ```
  This is what gives the dmg its designed installer window: the Rewinder icon,
  the drag-to-Applications art (Retina-sharp), and the correct window size.
  Without it the script silently falls back to a plain `hdiutil` dmg — it still
  installs and notarizes fine, but users get a bare white Finder window instead
  of the designed one. (This is exactly what happened to the first published
  build, so don't skip it.)

## One-time setup

1. **Developer ID certificate.** In Xcode → Settings → Accounts, add the Apple
   ID, then "Manage Certificates…" → + → **Developer ID Application**. (Or create
   it at developer.apple.com and download into your login keychain.) Verify:
   ```bash
   security find-identity -v -p codesigning   # should list "Developer ID Application: … (TEAMID)"
   ```
2. **Notarization credentials.** Recommended: an App Store Connect API key
   (App Store Connect → Users and Access → Integrations → Keys, role
   "Developer"). Store it as a reusable profile named `rewinder-notary`:
   ```bash
   xcrun notarytool store-credentials rewinder-notary \
     --key /path/to/AuthKey_XXXXXXXX.p8 --key-id XXXXXXXX --issuer <issuer-uuid>
   ```
   (Alternatively: `--apple-id you@example.com --team-id TEAMID --password <app-specific-password>`.)

## Release

```bash
cd RewinderApp
scripts/release_dmg.sh
```

That's it. The script:
1. Fetches a self-contained static `ffmpeg`/`ffprobe` if missing
   (`scripts/fetch_ffmpeg.sh`) and **refuses to ship** one that links non-system
   libraries (`scripts/verify_ffmpeg.sh`).
2. Builds + signs the app (`scripts/package_app.sh`) with hardened runtime +
   secure timestamp, and checks `get-task-allow` is absent.
3. Notarizes + staples the app, then builds the dmg with the designed installer
   window (`create-dmg` + `Resources/dmg-background.tiff`, a 1x + 2x
   multi-resolution image so the art is Retina-sharp), and signs + notarizes +
   staples the dmg too.
4. Prints the Gatekeeper assessment and SHA-256.

Before handing the dmg out, sanity-check it once: mount it and confirm the
window shows the background art with the full "© 2026 REWINDER" line visible at
the bottom, then:
```bash
spctl -a -vv -t install dist/Rewinder-*.dmg   # must say "accepted · Notarized Developer ID"
```

Useful overrides:

| Variable | Purpose |
| --- | --- |
| `REWINDER_CODESIGN_IDENTITY` | Pick a specific identity (auto-detected otherwise). |
| `REWINDER_NOTARY_PROFILE` | notarytool profile name (default `rewinder-notary`). |
| `REWINDER_SKIP_NOTARIZE=1` | Local dry run: build + sign + dmg, no notarize/staple. |
| `REWINDER_FFMPEG_DIR` | Use your own static ffmpeg/ffprobe instead of the vendored one. |
| `REWINDER_FFMPEG_URL` / `REWINDER_FFPROBE_URL` | Override the download source. |

Dry run without a cert (sanity-check the pipeline):
```bash
REWINDER_SKIP_NOTARIZE=1 REWINDER_CODESIGN_IDENTITY="-" scripts/release_dmg.sh
```

## Publish

1. Create a GitHub release on `abhinavkale-dev/rewinder` and upload
   `dist/Rewinder-<version>.dmg` (+ the `.sha256`).
2. The site's "Download for macOS" button points at the latest release
   (`DOWNLOAD_URL` in `web/components/Hero.tsx` and `web/components/FooterCta.tsx`).
   For a one-click direct download, name the asset `Rewinder.dmg` and switch
   `DOWNLOAD_URL` to
   `https://github.com/abhinavkale-dev/rewinder/releases/latest/download/Rewinder.dmg`.

## Notes

- **Architecture:** arm64 only (matches the current Rust/Swift build and
  macOS 26's Apple-Silicon focus). A universal build would also need x86_64
  builds of the Rust lib, the Swift app, and ffmpeg.
- **ffmpeg licensing:** the default vendored build (eugeneware/ffmpeg-static) is
  **GPL**. If you redistribute it you must offer the corresponding source — link
  the upstream `b6.1.1` tag, or point `REWINDER_FFMPEG_*` at an LGPL build
  (`--disable-gpl`, no x264/x265). Rewinder only needs `h264_videotoolbox`,
  `aac`, and core filters (`arnndn`/`afftdn`/`amix`/`volume`/`aresample`/
  `setpts`), so an LGPL build is sufficient.
- **Auto-updates:** not included. Sparkle (with a signed appcast) is the usual
  next step.
