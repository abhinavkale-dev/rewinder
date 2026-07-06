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
