#!/usr/bin/env bash
# Copies the chosen winemac.drv variant into the Cauldron Wine runtime so the
# next game launch picks it up.
#
# Usage: ./tests/flicker/swap_variant.sh <baseline|patched>

set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT="$(pwd)"
RUNTIME="$HOME/Library/Cauldron/wine"
VARIANTS="$ROOT/tests/flicker/variants"

VARIANT="${1:-}"
if [[ "$VARIANT" != "baseline" && "$VARIANT" != "patched" ]]; then
    echo "Usage: $0 <baseline|patched>" >&2
    exit 1
fi

SRC_SO="$VARIANTS/$VARIANT/winemac.so"
SRC_DRV="$VARIANTS/$VARIANT/winemac.drv"

if [[ ! -f "$SRC_SO" || ! -f "$SRC_DRV" ]]; then
    echo "ERROR: variant '$VARIANT' not built. Run ./tests/flicker/build_variants.sh first." >&2
    exit 1
fi

DST_SO="$RUNTIME/lib/wine/x86_64-unix/winemac.so"
DST_DRV="$RUNTIME/lib/wine/x86_64-windows/winemac.drv"

if [[ ! -d "$RUNTIME" ]]; then
    echo "ERROR: Cauldron Wine runtime not found at $RUNTIME" >&2
    exit 1
fi

# Kill any running wineserver to ensure the new winemac.drv gets picked up.
pkill -9 -f "$RUNTIME/bin/wineserver" 2>/dev/null || true
pkill -9 -f wine64-preloader 2>/dev/null || true
sleep 1

cp "$SRC_SO" "$DST_SO"
cp "$SRC_DRV" "$DST_DRV"

echo "Runtime winemac.drv now: $VARIANT"
echo "  unix: $DST_SO"
echo "  pe:   $DST_DRV"
