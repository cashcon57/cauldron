.PHONY: build build-release test check lint clean run-cli test-unit test-e2e test-all build-kosmickrisp kk-status wine-init wine-build wine-clean

# Development build
build:
	cargo build --workspace

# Release build
build-release:
	cargo build --release --workspace

# Run all tests
test:
	cargo test --workspace

# Type check
check:
	cargo check --workspace

# Lint
lint:
	cargo clippy --workspace -- -D warnings
	cargo fmt --check

# Format code
fmt:
	cargo fmt --all

# Clean build artifacts
clean:
	cargo clean
	rm -rf build/
	cd CauldronApp && swift package clean 2>/dev/null || true

# Build CLI and run
run-cli:
	cargo run -p cauldron-cli -- $(ARGS)

# Build Swift app
swift-build:
	cargo build --release -p cauldron-bridge
	cd CauldronApp && swift build

# Build DMG
dmg:
	./scripts/build_dmg.sh $(VERSION)

# Initialize dev environment
setup:
	./scripts/install_deps.sh

# Database operations
db-init:
	cargo run -p cauldron-cli -- db init

db-seed:
	cargo run -p cauldron-cli -- db seed

# Run unit tests only
test-unit:
	cargo test --workspace

# Run end-to-end CLI tests
test-e2e:
	cargo build --release -p cauldron-cli
	python3 scripts/test_e2e.py --binary target/release/cauldron

# Run all tests (unit + e2e)
test-all: test-unit test-e2e

# KosmicKrisp (Mesa Vulkan on Metal)
build-kosmickrisp:
	./scripts/build_kosmickrisp.sh

kk-status:
	cargo run -p cauldron-cli -- kk status

# Cauldron Wine fork
wine-init:
	./scripts/init_wine_fork.sh

wine-init-clean:
	./scripts/init_wine_fork.sh --clean

wine-build:
	./scripts/build_wine.sh

wine-build-clean:
	./scripts/build_wine.sh --clean

wine-clean:
	rm -rf wine/ build/wine build/wine-dist

# Generate shell completions
completions:
	cargo run -p cauldron-cli -- completions zsh > _cauldron
