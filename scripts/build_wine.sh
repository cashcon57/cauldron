#!/usr/bin/env bash
# Build Cauldron Wine for macOS (arm64).
#
# Usage: ./scripts/build_wine.sh [--clean] [--jobs N]
#
# Prerequisites: brew install bison flex mingw-w64 gettext pkg-config
# The fork must be initialized first: ./scripts/init_wine_fork.sh

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
WINE_SRC="$ROOT/wine"
BUILD_DIR="$ROOT/build/wine"
INSTALL_DIR="$ROOT/build/wine-dist"
JOBS="${JOBS:-$(sysctl -n hw.ncpu)}"

# --- Options ---
CLEAN=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean) CLEAN=true; shift ;;
        --jobs)  JOBS="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ ! -d "$WINE_SRC/.git" ]]; then
    echo "ERROR: Wine source not found. Run ./scripts/init_wine_fork.sh first."
    exit 1
fi

if $CLEAN; then
    echo "==> Cleaning build directory..."
    rm -rf "$BUILD_DIR" "$INSTALL_DIR"
fi

# --- Check dependencies ---
echo "==> Checking build dependencies..."
MISSING=""
for dep in bison flex x86_64-w64-mingw32-gcc; do
    if ! command -v "$dep" &>/dev/null; then
        MISSING="$MISSING $dep"
    fi
done
if [[ -n "$MISSING" ]]; then
    echo "ERROR: Missing dependencies:$MISSING"
    echo "Run: brew install bison flex mingw-w64"
    exit 1
fi

# Use Homebrew's bison/flex (macOS ships ancient versions)
export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/flex/bin:$PATH"

# --- Configure ---
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

if [[ ! -f Makefile ]]; then
    echo "==> Configuring Wine (arm64, WoW64)..."
    "$WINE_SRC/configure" \
        --prefix="$INSTALL_DIR" \
        --enable-archs=x86_64 \
        --enable-win64 \
        --without-x \
        --with-gnutls \
        --with-freetype \
        --disable-tests \
        BISON="/opt/homebrew/opt/bison/bin/bison" \
        2>&1 | tee "$ROOT/build/configure.log" | tail -5
    echo "    Configuration complete. See build/configure.log for details."
fi

# --- Build ---
echo "==> Building Wine (${JOBS} jobs)..."
make -j"$JOBS" 2>&1 | tee "$ROOT/build/build.log" | tail -20

# --- Install ---
echo "==> Installing to $INSTALL_DIR..."
make install 2>&1 | tail -5

# --- Verify ---
echo ""
echo "=== Build Complete ==="
WINE_BIN="$INSTALL_DIR/bin/wine64"
if [[ -x "$WINE_BIN" ]]; then
    echo "Wine binary: $WINE_BIN"
    "$WINE_BIN" --version 2>/dev/null || echo "(version check requires runtime)"
    echo ""
    echo "To use: export PATH=\"$INSTALL_DIR/bin:\$PATH\""
else
    echo "WARNING: wine64 binary not found at $WINE_BIN"
    echo "Check build/build.log for errors."
fi
