#!/bin/bash
# Install development dependencies for Cauldron
set -euo pipefail

echo "=== Cauldron Development Setup ==="

# Check macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo "Error: Cauldron requires macOS"
    exit 1
fi

# Check/install Homebrew
if ! command -v brew &> /dev/null; then
    echo "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

# Check/install Rust
if ! command -v rustup &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Install build tools
echo "Installing build dependencies..."
brew install meson ninja cmake python3 pkg-config || true

# Install Rust components
echo "Installing Rust components..."
rustup component add clippy rustfmt

# Verify Xcode CLI tools
if ! xcode-select -p &> /dev/null; then
    echo "Installing Xcode command line tools..."
    xcode-select --install
fi

# Verify Swift
if ! command -v swift &> /dev/null; then
    echo "Error: Swift not found. Install Xcode or Swift toolchain."
    exit 1
fi

echo ""
echo "=== Setup complete ==="
echo "Run 'cargo build --workspace' to build the Rust workspace"
echo "Run 'cd CauldronApp && swift build' to build the SwiftUI app"
