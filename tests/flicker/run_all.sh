#!/usr/bin/env bash
# End-to-end orchestrator for the flicker A/B test.
# Builds both winemac.drv variants, records a session under each, and runs
# the frame-diff analyzer.
#
# Usage:
#   ./tests/flicker/run_all.sh <bottle-id> <wine-exe-path> [duration]

set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT="$(pwd)"
TEST="$ROOT/tests/flicker"

BOTTLE="${1:-}"
EXE="${2:-}"
DURATION="${3:-30}"

if [[ -z "$BOTTLE" || -z "$EXE" ]]; then
    cat <<EOF >&2
Usage: $0 <bottle-id> <wine-exe-path> [duration]

  bottle-id       Cauldron bottle UUID (from 'cauldron-cli list-bottles')
  wine-exe-path   Full path to the .exe inside the bottle's drive_c
  duration        Seconds of video to record per variant (default: 30)

Example:
  $0 f2a752a2-... \\
     "/Users/cashconway/Library/Application Support/CrossOver/Bottles/Steam/drive_c/.../game.exe"
EOF
    exit 1
fi

OUT_DIR="/Users/Shared/localci-logs/wine-flicker/$(date +%Y%m%dT%H%M%S)"
export FLICKER_OUT_DIR="$OUT_DIR"
mkdir -p "$OUT_DIR"

echo "================================================================"
echo "Flicker A/B test"
echo "Bottle:   $BOTTLE"
echo "Exe:      $EXE"
echo "Duration: ${DURATION}s per variant"
echo "Output:   $OUT_DIR"
echo "================================================================"

# 1. Build both variants.
echo ""
echo "==> Step 1/4: building winemac.drv variants ..."
"$TEST/build_variants.sh"

# 2. Record baseline.
echo ""
echo "==> Step 2/4: recording BASELINE (no flicker patch) ..."
echo "    Game window will appear in ~8s; recording starts after that."
read -p "    Press ENTER to start the baseline run..."
"$TEST/record_session.sh" baseline "$BOTTLE" "$EXE" "$DURATION"

read -p "Close the game window, then press ENTER to start the patched run..."

# 3. Record patched.
echo ""
echo "==> Step 3/4: recording PATCHED (0003 applied) ..."
"$TEST/record_session.sh" patched "$BOTTLE" "$EXE" "$DURATION"

read -p "Close the game window, then press ENTER to run the frame-diff analyzer..."

# 4. Analyze.
echo ""
echo "==> Step 4/4: analyzing ..."
python3 "$TEST/analyze_frames.py" "$OUT_DIR/baseline.mov"
echo ""
python3 "$TEST/analyze_frames.py" "$OUT_DIR/patched.mov"

echo ""
echo "================================================================"
echo "Done. Compare videos and CSVs in:"
echo "  $OUT_DIR"
echo ""
echo "Visual inspection (your call):"
echo "  open '$OUT_DIR/baseline.mov' '$OUT_DIR/patched.mov'"
echo ""
echo "After review, restore the runtime to the baseline (or patched, as you prefer):"
echo "  $TEST/swap_variant.sh baseline"
echo "================================================================"
