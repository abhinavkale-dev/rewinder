#!/usr/bin/env bash
#
# Verifies that the ffmpeg/ffprobe in a given directory are safe to ship:
#   1. arm64 architecture
#   2. fully portable — link ONLY against /usr/lib + /System (no Homebrew or
#      other non-system dylibs that won't exist on a user's Mac)
#   3. contain the encoders/filters Rewinder actually uses
#
# Usage: verify_ffmpeg.sh <dir-containing-ffmpeg-and-ffprobe>
# Exits non-zero (with a clear message) on any failure so callers can gate on it.
#
set -euo pipefail

DIR="${1:?usage: verify_ffmpeg.sh <dir>}"
FAIL=0

for tool in ffmpeg ffprobe; do
    BIN="$DIR/$tool"
    if [[ ! -x "$BIN" ]]; then
        echo "FAIL: $tool not found/executable at $BIN" >&2
        FAIL=1
        continue
    fi

    # 1. architecture
    if ! lipo -archs "$BIN" 2>/dev/null | tr ' ' '\n' | grep -qx "arm64"; then
        echo "FAIL: $tool is not arm64 (got: $(lipo -archs "$BIN" 2>/dev/null))" >&2
        FAIL=1
    fi

    # 2. portability — any linked dylib/framework outside /usr/lib + /System is fatal
    BAD="$(otool -L "$BIN" | tail -n +2 \
        | awk '{print $1}' \
        | grep -Ev '^/usr/lib/|^/System/' || true)"
    if [[ -n "$BAD" ]]; then
        echo "FAIL: $tool links non-system libraries (won't run on users' Macs):" >&2
        echo "$BAD" | sed 's/^/    /' >&2
        FAIL=1
    fi
done

# 3. required encoders + filters (check ffmpeg only)
if [[ -x "$DIR/ffmpeg" ]]; then
    ENC="$("$DIR/ffmpeg" -hide_banner -encoders 2>/dev/null)"
    for e in h264_videotoolbox aac; do
        grep -qE "[[:space:]]$e([[:space:]]|\$)" <<<"$ENC" \
            || { echo "FAIL: ffmpeg missing encoder: $e" >&2; FAIL=1; }
    done
    FLT="$("$DIR/ffmpeg" -hide_banner -filters 2>/dev/null)"
    for f in arnndn afftdn amix volume aresample setpts; do
        grep -qE "[[:space:]]$f([[:space:]]|\$)" <<<"$FLT" \
            || { echo "FAIL: ffmpeg missing filter: $f" >&2; FAIL=1; }
    done
fi

if [[ "$FAIL" -ne 0 ]]; then
    echo "ffmpeg verification FAILED for $DIR" >&2
    exit 1
fi

echo "ffmpeg verification OK ($DIR): arm64, portable, required codecs/filters present"
