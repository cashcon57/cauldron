#!/bin/bash
# Build script for Cauldron.dmg
set -euo pipefail

# Configuration
APP_NAME="Cauldron"
VERSION="${1:-dev}"
BUILD_DIR="build"
DMG_DIR="${BUILD_DIR}/dmg"

echo "=== Building Cauldron v${VERSION} ==="

# Step 1: Build Rust workspace
echo "[1/5] Building Rust workspace..."
cargo build --release --workspace

# Step 2: Build Swift app
echo "[2/5] Building SwiftUI app..."
cd CauldronApp && swift build -c release && cd ..

# Step 3: Create app bundle structure
echo "[3/5] Creating app bundle..."
mkdir -p "${DMG_DIR}/${APP_NAME}.app/Contents/MacOS"
mkdir -p "${DMG_DIR}/${APP_NAME}.app/Contents/Resources"
mkdir -p "${DMG_DIR}/${APP_NAME}.app/Contents/Frameworks"

# Copy binaries
cp target/release/libcauldron_bridge.dylib "${DMG_DIR}/${APP_NAME}.app/Contents/Frameworks/" 2>/dev/null || true
cp CauldronApp/.build/release/CauldronApp "${DMG_DIR}/${APP_NAME}.app/Contents/MacOS/${APP_NAME}" 2>/dev/null || echo "Note: Swift binary not found, skipping"
cp target/release/cauldron "${DMG_DIR}/${APP_NAME}.app/Contents/MacOS/cauldron-cli" 2>/dev/null || echo "Note: CLI binary not found, skipping"

# Create Info.plist
cat > "${DMG_DIR}/${APP_NAME}.app/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.cauldron.app</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# Create entitlements
cat > "${BUILD_DIR}/entitlements.plist" << ENTITLEMENTS
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <key>com.apple.security.cs.allow-dyld-environment-variables</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
</dict>
</plist>
ENTITLEMENTS

# Step 4: Code sign (ad-hoc for development)
echo "[4/5] Code signing..."
codesign --force --deep --sign - --entitlements "${BUILD_DIR}/entitlements.plist" "${DMG_DIR}/${APP_NAME}.app" 2>/dev/null || echo "Note: Code signing skipped (no identity)"

# Step 5: Create DMG
echo "[5/5] Creating DMG..."
hdiutil create -volname "${APP_NAME}" -srcfolder "${DMG_DIR}" -ov -format UDZO "${BUILD_DIR}/${APP_NAME}-${VERSION}.dmg" 2>/dev/null || echo "Note: DMG creation requires macOS"

echo "=== Build complete: ${BUILD_DIR}/${APP_NAME}-${VERSION}.dmg ==="
