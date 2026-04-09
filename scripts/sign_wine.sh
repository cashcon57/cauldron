#!/usr/bin/env bash
# sign_wine.sh — Sign all Wine binaries with JIT entitlements for macOS
#
# Wine on ARM64 macOS needs these entitlements:
# - com.apple.security.cs.allow-jit — JIT compilation for PE→native translation
# - com.apple.security.cs.allow-unsigned-executable-memory — Wine's code generation
# - com.apple.security.cs.disable-library-validation — load unsigned .so modules
#
# Usage: ./scripts/sign_wine.sh <wine-install-dir>

set -euo pipefail

WINE_DIR="${1:?Usage: $0 <wine-install-dir>}"

if [ ! -d "$WINE_DIR/bin" ]; then
    echo "Error: $WINE_DIR/bin not found"
    exit 1
fi

# Create entitlements plist
ENT=$(mktemp /tmp/wine-entitlements.XXXXXX.plist)
cat > "$ENT" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <key>com.apple.security.cs.disable-executable-page-protection</key>
    <true/>
</dict>
</plist>
EOF

echo "Signing Wine binaries in $WINE_DIR..."

# Sign all Mach-O binaries and shared libraries
SIGNED=0
FAILED=0

sign_file() {
    local f="$1"
    # --options runtime is REQUIRED for entitlements to take effect on macOS
    if codesign --force --sign - --entitlements "$ENT" --options runtime "$f" 2>/dev/null; then
        SIGNED=$((SIGNED + 1))
    else
        # Some files aren't Mach-O — skip silently
        FAILED=$((FAILED + 1))
    fi
}

# Sign executables in bin/
for f in "$WINE_DIR/bin/"*; do
    [ -f "$f" ] && [ -x "$f" ] && sign_file "$f"
done

# Sign ALL .so files — critical for Wine PE execution
# Without proper entitlements on ntdll.so et al, wineboot gets SIGKILL'd
for f in "$WINE_DIR/lib/wine/"*-unix/*.so; do
    [ -f "$f" ] && sign_file "$f"
done

# Sign any .dylib files
for f in "$WINE_DIR/lib/"*.dylib "$WINE_DIR/lib/wine/"*-unix/*.dylib; do
    [ -f "$f" ] && sign_file "$f"
done

rm -f "$ENT"

echo "Signed $SIGNED files ($FAILED skipped)"

# Verify critical binaries
echo ""
echo "Verification:"
for bin in wine wineserver wineboot; do
    if [ -f "$WINE_DIR/bin/$bin" ]; then
        ent=$(codesign -d --entitlements - "$WINE_DIR/bin/$bin" 2>&1 | grep "allow-jit" | wc -l)
        if [ "$ent" -gt 0 ]; then
            echo "  ✓ $bin — JIT entitlement present"
        else
            echo "  ✗ $bin — JIT entitlement MISSING"
        fi
    fi
done
