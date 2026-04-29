#!/usr/bin/env bash
# Builds two winemac.drv variants — baseline (no flicker patch) and patched
# (0003 applied) — by incrementally rebuilding only dlls/winemac.drv from the
# existing Wine build tree. Outputs land in tests/flicker/variants/{baseline,patched}/.
#
# The flicker patch is committed to the Cauldron Wine fork (commit 641f702 at
# the time of writing), so the source-on-disk normally represents the PATCHED
# state. This script detects the current state and inverts as needed:
#   - if patch IS present  -> build patched first, revert + build baseline,
#                             re-apply patch to restore source.
#   - if patch is NOT present -> build baseline first, apply + build patched,
#                                revert patch to restore source.
#
# Either way, source is left in its original state when the script exits.
#
# Usage: ./tests/flicker/build_variants.sh

set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT="$(pwd)"
WINE_SRC="$ROOT/wine"
BUILD_DIR="$ROOT/build/wine"
TEST_DIR="$ROOT/tests/flicker"
VARIANTS="$TEST_DIR/variants"
PATCH="$ROOT/patches/cauldron/0003-winemac-drv-reduce-compositor-flicker.patch"
JOBS="${JOBS:-$(sysctl -n hw.ncpu)}"

if [[ ! -d "$WINE_SRC/.git" ]]; then
    echo "ERROR: Wine source missing at $WINE_SRC. Run scripts/init_wine_fork.sh first." >&2
    exit 1
fi
if [[ ! -d "$BUILD_DIR/dlls/winemac.drv" ]]; then
    echo "ERROR: Wine build dir missing at $BUILD_DIR. Run scripts/build_wine.sh first." >&2
    exit 1
fi
if [[ ! -f "$PATCH" ]]; then
    echo "ERROR: Patch not found: $PATCH" >&2
    exit 1
fi

mkdir -p "$VARIANTS/baseline" "$VARIANTS/patched"

# Detect current state of the source: does the forward patch apply, or only
# the reverse? The check returns 0 on apply-success, non-zero on conflict.
PATCH_PRESENT=false
pushd "$WINE_SRC" >/dev/null
if git apply --reverse --check "$PATCH" >/dev/null 2>&1; then
    PATCH_PRESENT=true
elif git apply --check "$PATCH" >/dev/null 2>&1; then
    PATCH_PRESENT=false
else
    echo "ERROR: source state is ambiguous — the patch neither applies nor reverses" >&2
    echo "       cleanly. Source may have drifted; rebase the patch first." >&2
    popd >/dev/null
    exit 1
fi
popd >/dev/null

echo "==> Detected source state: patch is $([[ $PATCH_PRESENT == true ]] && echo PRESENT || echo ABSENT)"

# Track what we modified so cleanup can reverse it.
DID_REVERT=false
DID_APPLY=false

cleanup() {
    set +e
    pushd "$WINE_SRC" >/dev/null
    if $DID_REVERT && ! $DID_APPLY; then
        # Started patched, reverted, never re-applied -> re-apply now.
        echo "==> Restoring patched state in source..."
        git apply "$PATCH"
    fi
    if $DID_APPLY && ! $DID_REVERT; then
        # Started baseline, applied, never reverted -> revert now.
        echo "==> Restoring baseline state in source..."
        git apply --reverse "$PATCH"
    fi
    popd >/dev/null
    set -e
}
trap cleanup EXIT

build_variant() {
    local label="$1"
    echo "==> Building winemac.drv ($label) ..."
    touch "$WINE_SRC/dlls/winemac.drv/cocoa_window.m"
    # Build from the top-level Makefile, targeting BOTH the unix .so and the
    # PE driver. Wine is built x86_64 via Rosetta (see scripts/build_wine.sh),
    # so the same arch wrapper is required here or arm64 .o files will fail
    # to link against the existing x86_64 ntdll.so / win32u.so.
    rm -f "$BUILD_DIR/dlls/winemac.drv/cocoa_window.o" \
          "$BUILD_DIR/dlls/winemac.drv/winemac.so"
    arch -x86_64 make -C "$BUILD_DIR" -j"$JOBS" \
        dlls/winemac.drv/winemac.so \
        dlls/winemac.drv/x86_64-windows/winemac.drv 2>&1 | tail -10
    cp "$BUILD_DIR/dlls/winemac.drv/winemac.so" "$VARIANTS/$label/winemac.so"
    cp "$BUILD_DIR/dlls/winemac.drv/x86_64-windows/winemac.drv" "$VARIANTS/$label/winemac.drv"
    {
        echo "label: $label"
        echo "patch_applied: $([[ $label == patched ]] && echo yes || echo no)"
        echo "wine_commit: $(cd "$WINE_SRC" && git rev-parse HEAD)"
        echo "built_at: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "winemac.so size: $(stat -f%z "$VARIANTS/$label/winemac.so") bytes"
        echo "winemac.drv size: $(stat -f%z "$VARIANTS/$label/winemac.drv") bytes"
    } > "$VARIANTS/$label/META.txt"
    echo "    saved: $VARIANTS/$label/"
}

if $PATCH_PRESENT; then
    # Source starts patched. Build patched, then revert + build baseline.
    build_variant patched

    echo "==> Reverting patch to build baseline..."
    pushd "$WINE_SRC" >/dev/null
    git apply --reverse "$PATCH"
    DID_REVERT=true
    popd >/dev/null

    build_variant baseline
else
    # Source starts baseline. Build baseline, then apply + build patched.
    build_variant baseline

    echo "==> Applying patch to build patched..."
    pushd "$WINE_SRC" >/dev/null
    git apply "$PATCH"
    DID_APPLY=true
    popd >/dev/null

    build_variant patched
fi

echo ""
echo "===================="
echo "Variants ready:"
echo "===================="
for v in baseline patched; do
    echo ""
    echo "[$v]"
    cat "$VARIANTS/$v/META.txt"
done
