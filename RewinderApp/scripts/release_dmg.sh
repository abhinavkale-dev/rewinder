#!/usr/bin/env bash
#
# One-command release: build Rewinder.app, Developer ID sign it, notarize +
# staple, wrap it in a .dmg, sign + notarize + staple the .dmg, and emit a
# SHA-256 checksum. The result in dist/ is ready to host for download.
#
# ── One-time setup (per machine) ─────────────────────────────────────────────
#   1. Join the Apple Developer Program and create a "Developer ID Application"
#      certificate; install it in your login keychain (Xcode > Settings >
#      Accounts > Manage Certificates, or download from developer.apple.com).
#   2. Store a notarytool credential profile (App Store Connect API key is best):
#        xcrun notarytool store-credentials rewinder-notary \
#          --key /path/AuthKey_XXXX.p8 --key-id XXXX --issuer <issuer-uuid>
#      (or: --apple-id you@example.com --team-id TEAMID --password <app-specific>)
#
# ── Usage ────────────────────────────────────────────────────────────────────
#   scripts/release_dmg.sh
#
# ── Configuration (env vars) ─────────────────────────────────────────────────
#   REWINDER_CODESIGN_IDENTITY  Developer ID identity. Auto-detected from the
#                               keychain when unset.
#   REWINDER_NOTARY_PROFILE     notarytool keychain profile. Default: rewinder-notary
#   REWINDER_SKIP_NOTARIZE=1    Build + sign + dmg only (no notarize/staple) —
#                               handy for a local dry run before you have creds.
#
set -euo pipefail

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"   # RewinderApp/
OUT="$APP_DIR/build/Rewinder.app"
DIST="$APP_DIR/dist"
PROFILE="${REWINDER_NOTARY_PROFILE:-rewinder-notary}"
SKIP_NOTARIZE="${REWINDER_SKIP_NOTARIZE:-0}"

# ── Resolve a Developer ID Application identity ───────────────────────────────
IDENTITY="${REWINDER_CODESIGN_IDENTITY:-}"
if [[ -z "$IDENTITY" ]]; then
    IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null \
        | awk -F'"' '/Developer ID Application/ {print $2; exit}')"
fi
if [[ -z "$IDENTITY" ]]; then
    cat >&2 <<'EOF'
ERROR: no "Developer ID Application" certificate found.

Distribution requires a Developer ID cert (Apple Developer Program). Either:
  - install one and re-run, or
  - set REWINDER_CODESIGN_IDENTITY="Developer ID Application: Name (TEAMID)", or
  - run a local dry run without notarization:
        REWINDER_SKIP_NOTARIZE=1 REWINDER_CODESIGN_IDENTITY="-" scripts/release_dmg.sh
EOF
    exit 1
fi
echo "==> Signing identity: $IDENTITY"

# ── 1. Build + sign the .app (delegates to package_app.sh) ────────────────────
echo "==> Building + signing Rewinder.app"
REWINDER_CODESIGN_IDENTITY="$IDENTITY" "$APP_DIR/scripts/package_app.sh"

# ── 2. Verify the signature is distribution-grade ─────────────────────────────
echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$OUT"
if [[ "$IDENTITY" != "-" ]]; then
    # Hardened runtime must be on, and get-task-allow (debug) must be off, or
    # notarization will reject the app. Capture codesign output before grepping:
    # `codesign | grep -q` under pipefail dies of SIGPIPE when grep exits early,
    # which made these checks report false failures/passes.
    SIGN_INFO="$(codesign -dvvv "$OUT" 2>&1)"
    grep -q "flags=.*runtime" <<< "$SIGN_INFO" \
        || { echo "ERROR: hardened runtime missing on app" >&2; exit 1; }
    APP_ENTITLEMENTS="$(codesign -d --entitlements - --xml "$OUT" 2>/dev/null || true)"
    if grep -q "get-task-allow" <<< "$APP_ENTITLEMENTS"; then
        echo "ERROR: com.apple.security.get-task-allow present (use a Developer ID" >&2
        echo "       cert, not Apple Development) — notarization would fail." >&2
        exit 1
    fi
fi

VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
    "$OUT/Contents/Info.plist" 2>/dev/null || echo 0.0.0)"
mkdir -p "$DIST"
DMG="$DIST/Rewinder-$VERSION.dmg"
rm -f "$DMG"

# ── 3. Notarize + staple the .app ─────────────────────────────────────────────
if [[ "$SKIP_NOTARIZE" == "1" || "$IDENTITY" == "-" ]]; then
    echo "==> Skipping notarization (REWINDER_SKIP_NOTARIZE/ad-hoc)"
else
    echo "==> Notarizing app (profile: $PROFILE)"
    ZIP="$DIST/Rewinder-$VERSION.zip"
    /usr/bin/ditto -c -k --keepParent "$OUT" "$ZIP"
    xcrun notarytool submit "$ZIP" --keychain-profile "$PROFILE" --wait
    rm -f "$ZIP"
    echo "==> Stapling app"
    xcrun stapler staple "$OUT"
fi

# ── 4. Build the .dmg (app + /Applications drop target) ───────────────────────
echo "==> Building $DMG"
if command -v create-dmg >/dev/null 2>&1; then
    BG_ARGS=()
    # Prefer the multi-resolution tiff (1x + 2x) so the background renders
    # sharp on Retina displays; fall back to the 1x png.
    if [[ -f "$APP_DIR/Resources/dmg-background.tiff" ]]; then
        BG_ARGS=(--background "$APP_DIR/Resources/dmg-background.tiff")
    elif [[ -f "$APP_DIR/Resources/dmg-background.png" ]]; then
        BG_ARGS=(--background "$APP_DIR/Resources/dmg-background.png")
    fi
    # Portrait Aside-style layout: app on top, Applications below, the
    # background's blue beam bridging the drag path between them. These
    # coordinates must match the slots painted in dmg-background.html.
    # Window height = 640px background + 28px title bar (create-dmg window
    # bounds include the title bar, which otherwise crops the bottom).
    create-dmg \
        --volname "Rewinder" \
        --window-size 460 668 \
        --icon-size 110 \
        --icon "Rewinder.app" 230 195 \
        --app-drop-link 230 450 \
        ${BG_ARGS[@]+"${BG_ARGS[@]}"} \
        --no-internet-enable \
        "$DMG" "$OUT"
else
    echo "    create-dmg not found; using hdiutil (brew install create-dmg for a"
    echo "    prettier window with a background + drop target)."
    STAGE="$(mktemp -d)"
    cp -R "$OUT" "$STAGE/Rewinder.app"
    ln -s /Applications "$STAGE/Applications"
    hdiutil create -volname "Rewinder" -srcfolder "$STAGE" -ov \
        -format UDZO "$DMG"
    rm -rf "$STAGE"
fi

# ── 5. Sign + notarize + staple the .dmg ──────────────────────────────────────
if [[ "$IDENTITY" != "-" ]]; then
    echo "==> Signing dmg"
    codesign --force --timestamp --sign "$IDENTITY" "$DMG"
fi
if [[ "$SKIP_NOTARIZE" != "1" && "$IDENTITY" != "-" ]]; then
    echo "==> Notarizing + stapling dmg"
    xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
    xcrun stapler staple "$DMG"
fi

# ── 6. Verify + checksum ──────────────────────────────────────────────────────
echo "==> Gatekeeper assessment"
spctl -a -vvv --type install "$DMG" 2>&1 || \
    echo "    (spctl reports not-notarized; expected for a dry run)"
echo "==> SHA-256"
shasum -a 256 "$DMG" | tee "$DMG.sha256"

echo "==> Done: $DMG"
