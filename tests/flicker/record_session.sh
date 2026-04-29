#!/usr/bin/env bash
# Records a screen capture of a game already running under the chosen
# winemac.drv variant. The Cauldron-CLI launch path is currently a stub
# (it only prints the env it would set), so YOU must launch the game via
# the Cauldron app UI before this script starts recording.
#
# Usage:
#   ./tests/flicker/record_session.sh <variant> [duration]
#
# The script:
#   1. Swaps the runtime to the chosen variant.
#   2. Prompts you to launch FO4 (or whatever game) via Cauldron.app.
#   3. Records DURATION seconds of screen.
#   4. Saves output under $FLICKER_OUT_DIR (or a fresh timestamped dir).
#
# Requires: macOS Screen Recording permission for the host (Terminal /
# VS Code / Ghostty — whichever launches this script).

set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT="$(pwd)"

VARIANT="${1:-}"
DURATION="${2:-30}"

if [[ -z "$VARIANT" ]]; then
    cat <<EOF >&2
Usage: $0 <variant> [duration]
  variant   baseline | patched
  duration  seconds of video (default: 30)
EOF
    exit 1
fi

if [[ "$VARIANT" != "baseline" && "$VARIANT" != "patched" ]]; then
    echo "ERROR: variant must be 'baseline' or 'patched'" >&2
    exit 1
fi

# Ensure the runtime is on the chosen variant.
"$ROOT/tests/flicker/swap_variant.sh" "$VARIANT"

OUT_DIR="${FLICKER_OUT_DIR:-/Users/Shared/localci-logs/wine-flicker/$(date +%Y%m%dT%H%M%S)}"
mkdir -p "$OUT_DIR"

VIDEO="$OUT_DIR/$VARIANT.mov"
META="$OUT_DIR/$VARIANT.meta.txt"

WINE_BIN="$HOME/Library/Cauldron/wine/bin/wine64"
WINE_VER="(unknown)"
[[ -x "$WINE_BIN" ]] && WINE_VER="$("$WINE_BIN" --version 2>/dev/null | head -1)"

cat > "$META" <<EOF
variant:       $VARIANT
duration_s:    $DURATION
date:          $(date -u +%Y-%m-%dT%H:%M:%SZ)
wine_version:  $WINE_VER
macos:         $(sw_vers -productVersion)
host:          $(hostname -s)
patch:         patches/cauldron/0003-winemac-drv-reduce-compositor-flicker.patch
applied:       $([[ "$VARIANT" == "patched" ]] && echo "yes" || echo "no")
EOF

cat <<EOF

================================================================
Variant: $VARIANT
Output:  $VIDEO
Duration: ${DURATION}s

Now launch the game via the Cauldron app UI:
  1. Open /Applications/Cauldron.app
  2. Find FO4 (or your test game) in the bottle list
  3. Click Play
  4. Wait for the game window to render its main menu

The script will record for ${DURATION} seconds AS SOON AS YOU PRESS ENTER.
Make sure the game window has focus during recording — the flicker is
most visible when the game is the frontmost app.

================================================================
EOF

read -p "Press ENTER when the game is rendering and ready to record..."

echo ""
echo "==> Recording $DURATION seconds to $VIDEO ..."
screencapture -V "$DURATION" -v "$VIDEO" 2>&1 | tail -5 || true

if [[ -f "$VIDEO" ]]; then
    SIZE=$(stat -f%z "$VIDEO")
    echo ""
    echo "Recording saved: $VIDEO ($((SIZE / 1024 / 1024)) MB)"
    echo "Metadata:        $META"
else
    echo "ERROR: recording failed (no $VIDEO produced)." >&2
    exit 1
fi

echo ""
echo "==> Close the game manually before the next variant run."
