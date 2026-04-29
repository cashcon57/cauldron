#!/usr/bin/env bash
# Build Cauldron Wine for macOS (arm64).
#
# Usage: ./scripts/build_wine.sh [--clean] [--jobs N] [--no-deploy]
#
# Prerequisites: brew install bison flex mingw-w64 gettext pkg-config gstreamer gst-plugins-base gst-plugins-good
# The fork must be initialized first: ./scripts/init_wine_fork.sh

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
WINE_SRC="$ROOT/wine"
BUILD_DIR="$ROOT/build/wine"
INSTALL_DIR="$ROOT/build/wine-dist"
RUNTIME_DIR="$HOME/Library/Cauldron/wine"
JOBS="${JOBS:-$(sysctl -n hw.ncpu)}"

# DLLs that get replaced by DXMT/DXVK — preserve them during make install
OVERLAY_DLLS="d3d11.dll dxgi.dll d3d10core.dll winemetal.dll"

# --- Options ---
CLEAN=false
DEPLOY=true
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean) CLEAN=true; shift ;;
        --jobs)  JOBS="$2"; shift 2 ;;
        --no-deploy) DEPLOY=false; shift ;;
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
# Prepend ccache to PATH if available (5-10x faster rebuilds)
if [[ -x "$HOME/.local/bin/ccache" ]]; then
    export PATH="$HOME/.local/bin:$PATH"
    echo "Using ccache ($(ccache -s 2>/dev/null | grep 'Cache size' || echo 'available'))"
fi
export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/flex/bin:$PATH"

# --- Configure ---
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

if [[ ! -f Makefile ]]; then
    echo "==> Configuring Wine (x86_64 via Rosetta)..."
    # Wine must be x86_64 to load x86_64 PE DLLs and .exe files.
    # On Apple Silicon, we build under Rosetta (arch -x86_64) so the
    # host wine binary, wineserver, and all .so modules are x86_64.
    # Homebrew x86_64 libs are in /usr/local/, arm64 in /opt/homebrew/.
    arch -x86_64 "$WINE_SRC/configure" \
        --prefix="$INSTALL_DIR" \
        --enable-win64 \
        --without-x \
        --with-gnutls \
        --with-freetype \
        --with-gstreamer \
        --disable-tests \
        BISON="/opt/homebrew/opt/bison/bin/bison" \
        PKG_CONFIG_PATH="/usr/local/lib/pkgconfig:/usr/local/opt/gstreamer/lib/pkgconfig:/usr/local/opt/gnutls/lib/pkgconfig:/usr/local/opt/freetype/lib/pkgconfig" \
        CFLAGS="-arch x86_64" \
        LDFLAGS="-arch x86_64" \
        2>&1 | tee "$ROOT/build/configure.log" | tail -5
    echo "    Configuration complete. See build/configure.log for details."
fi

# --- Build ---
echo "==> Building Wine (${JOBS} jobs, x86_64)..."
arch -x86_64 make -j"$JOBS" 2>&1 | tee "$ROOT/build/build.log" | tail -20

# --- Preserve DXMT/DXVK overlay DLLs ---
# make install overwrites these with Wine builtins. Back up from the RUNTIME dir
# (where DXMT is actually installed) so we can restore after deploy.
OVERLAY_BAK="$ROOT/build/.overlay-dlls"
mkdir -p "$OVERLAY_BAK"
if [[ -d "$RUNTIME_DIR" ]]; then
    for dll in $OVERLAY_DLLS; do
        src="$RUNTIME_DIR/lib/wine/x86_64-windows/$dll"
        if [[ -f "$src" && -f "$src.wine-orig" ]]; then
            cp "$src" "$OVERLAY_BAK/$dll"
        fi
    done
fi

# --- Install ---
echo "==> Installing to $INSTALL_DIR..."
make install 2>&1 | tail -5

# --- Restore DXMT/DXVK overlay DLLs ---
for dll in $OVERLAY_DLLS; do
    if [[ -f "$OVERLAY_BAK/$dll" ]]; then
        cp "$OVERLAY_BAK/$dll" "$INSTALL_DIR/lib/wine/x86_64-windows/$dll"
        echo "    Preserved overlay DLL: $dll"
    fi
done

# --- Fix rpaths ---
# On macOS, x86_64 Homebrew libs are in /usr/local/. Wine's .so modules need
# rpaths to find them at runtime (gnutls, freetype, gstreamer).
add_rpaths() {
    local dir="$1"
    for f in "$dir/"*.so; do
        [[ -f "$f" ]] || continue
        install_name_tool -add_rpath /usr/local/lib "$f" 2>/dev/null || true
        install_name_tool -add_rpath /usr/local/opt/gnutls/lib "$f" 2>/dev/null || true
        install_name_tool -add_rpath /usr/local/opt/freetype/lib "$f" 2>/dev/null || true
        install_name_tool -add_rpath /usr/local/opt/gstreamer/lib "$f" 2>/dev/null || true
    done
}

echo "==> Fixing rpaths for x86_64 libraries..."
add_rpaths "$INSTALL_DIR/lib/wine/x86_64-unix"
echo "    rpaths added to $(ls "$INSTALL_DIR/lib/wine/x86_64-unix/"*.so 2>/dev/null | wc -l | tr -d ' ') .so files"

# --- Deploy to runtime prefix ---
if $DEPLOY && [[ -d "$RUNTIME_DIR" ]]; then
    echo "==> Deploying to runtime prefix ($RUNTIME_DIR)..."

    # Copy wineserver and wine binaries
    cp "$INSTALL_DIR/bin/wineserver" "$RUNTIME_DIR/bin/wineserver"
    for bin in wine wine64 wineboot winecfg winedbg; do
        [[ -f "$INSTALL_DIR/bin/$bin" ]] && cp "$INSTALL_DIR/bin/$bin" "$RUNTIME_DIR/bin/$bin"
    done

    # Copy unix .so modules (includes ntdll.so)
    cp "$INSTALL_DIR/lib/wine/x86_64-unix/"*.so "$RUNTIME_DIR/lib/wine/x86_64-unix/"

    # Copy PE DLLs, but preserve DXMT/DXVK overlays already in runtime
    for f in "$INSTALL_DIR/lib/wine/x86_64-windows/"*; do
        fname="$(basename "$f")"
        dest="$RUNTIME_DIR/lib/wine/x86_64-windows/$fname"

        # Skip overlay DLLs if the runtime already has a non-builtin version
        skip=false
        for dll in $OVERLAY_DLLS; do
            if [[ "$fname" == "$dll" && -f "$dest" && -f "$dest.wine-orig" ]]; then
                skip=true
                break
            fi
        done

        if ! $skip; then
            cp "$f" "$dest"
        fi
    done

    # Restore backed-up DXMT/DXVK overlay DLLs into runtime
    for dll in $OVERLAY_DLLS; do
        if [[ -f "$OVERLAY_BAK/$dll" ]]; then
            cp "$OVERLAY_BAK/$dll" "$RUNTIME_DIR/lib/wine/x86_64-windows/$dll"
            echo "    Restored overlay DLL to runtime: $dll"
        fi
    done

    # Fix rpaths on the runtime .so files too
    add_rpaths "$RUNTIME_DIR/lib/wine/x86_64-unix"

    echo "    Deployed to $RUNTIME_DIR"
elif $DEPLOY; then
    echo "    Skipping deploy: $RUNTIME_DIR does not exist"
fi

# --- Verify ---
echo ""
echo "=== Build Complete ==="
WINE_BIN="$INSTALL_DIR/bin/wine64"
if [[ -x "$WINE_BIN" ]]; then
    echo "Wine binary: $WINE_BIN"
    "$WINE_BIN" --version 2>/dev/null || echo "(version check requires runtime)"
    if $DEPLOY && [[ -d "$RUNTIME_DIR" ]]; then
        echo "Deployed to: $RUNTIME_DIR"
    else
        echo "To use: export PATH=\"$INSTALL_DIR/bin:\$PATH\""
    fi
else
    echo "WARNING: wine64 binary not found at $WINE_BIN"
    echo "Check build/build.log for errors."
fi
