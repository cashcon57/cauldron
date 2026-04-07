#!/usr/bin/env bash
# Initialize Cauldron's Wine fork from upstream Wine + apply our patches.
#
# Usage: ./scripts/init_wine_fork.sh [--clean]
#
# This clones upstream Wine, creates the cauldron/main branch, and applies
# all patches from patches/cauldron/ in order.

set -euo pipefail

WINE_DIR="wine"
UPSTREAM_URL="https://github.com/wine-mirror/wine.git"
UPSTREAM_BRANCH="wine-10.0"  # Latest stable
CAULDRON_BRANCH="cauldron/main"
PATCHES_DIR="patches/cauldron"
MSYNC_DIR="deps/wine-msync"

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

# --- Options ---
CLEAN=false
if [[ "${1:-}" == "--clean" ]]; then
    CLEAN=true
fi

if $CLEAN && [[ -d "$WINE_DIR" ]]; then
    echo "==> Removing existing wine directory..."
    rm -rf "$WINE_DIR"
fi

# --- Step 1: Clone upstream Wine ---
if [[ ! -d "$WINE_DIR/.git" ]]; then
    echo "==> Cloning upstream Wine ($UPSTREAM_BRANCH)..."
    git clone --branch "$UPSTREAM_BRANCH" --single-branch --depth=1 "$UPSTREAM_URL" "$WINE_DIR"
    echo "    Cloned $(cd "$WINE_DIR" && git log --oneline -1)"
else
    echo "==> Wine directory exists, updating..."
    cd "$WINE_DIR"
    git fetch origin "$UPSTREAM_BRANCH" --depth=1
    git checkout "$UPSTREAM_BRANCH" 2>/dev/null || git checkout -b "$UPSTREAM_BRANCH" "origin/$UPSTREAM_BRANCH"
    git reset --hard "origin/$UPSTREAM_BRANCH"
    cd "$ROOT"
fi

# --- Step 2: Create cauldron branch ---
cd "$WINE_DIR"
if git rev-parse --verify "$CAULDRON_BRANCH" &>/dev/null; then
    echo "==> Resetting existing $CAULDRON_BRANCH branch..."
    git checkout "$CAULDRON_BRANCH"
    git reset --hard "$UPSTREAM_BRANCH"
else
    echo "==> Creating $CAULDRON_BRANCH branch..."
    git checkout -b "$CAULDRON_BRANCH"
fi
cd "$ROOT"

# --- Step 3: Apply MSync patches (if available) ---
if [[ -d "$MSYNC_DIR/.git" ]]; then
    echo "==> Applying wine-msync patches..."
    MSYNC_PATCHES=$(find "$MSYNC_DIR" -name "*.patch" -o -name "*.diff" | sort)
    MSYNC_COUNT=0
    for patch in $MSYNC_PATCHES; do
        if cd "$WINE_DIR" && git apply --check "$ROOT/$patch" 2>/dev/null; then
            git apply "$ROOT/$patch"
            git add -A
            git commit -m "msync: $(basename "$patch" .patch)" --author="wine-msync <msync@wine-msync>"
            MSYNC_COUNT=$((MSYNC_COUNT + 1))
        else
            echo "    WARN: Skipping $patch (does not apply cleanly)"
        fi
        cd "$ROOT"
    done
    echo "    Applied $MSYNC_COUNT msync patches"
else
    echo "==> No wine-msync submodule found, skipping"
fi

# --- Step 4: Apply Cauldron patches ---
if [[ -d "$PATCHES_DIR" ]]; then
    echo "==> Applying Cauldron patches..."
    CAULDRON_PATCHES=$(find "$PATCHES_DIR" -name "*.patch" | sort)
    PATCH_COUNT=0
    for patch in $CAULDRON_PATCHES; do
        PATCH_NAME=$(basename "$patch" .patch)
        echo "    Applying: $PATCH_NAME"
        cd "$WINE_DIR"
        # Use git apply with context reduction for flexibility
        if git apply --check "$ROOT/$patch" 2>/dev/null; then
            git apply "$ROOT/$patch"
            git add -A
            # Extract commit message from patch Subject line
            SUBJECT=$(grep -m1 "^Subject:" "$ROOT/$patch" | sed 's/^Subject: \[PATCH\] //' | sed 's/^Subject: //')
            git commit -m "cauldron: $SUBJECT" --author="Cauldron <cauldron@cauldron.app>"
            PATCH_COUNT=$((PATCH_COUNT + 1))
        else
            echo "    ERROR: Patch does not apply cleanly!"
            echo "    Trying with reduced context (-C1)..."
            if git apply --check -C1 "$ROOT/$patch" 2>/dev/null; then
                git apply -C1 "$ROOT/$patch"
                git add -A
                SUBJECT=$(grep -m1 "^Subject:" "$ROOT/$patch" | sed 's/^Subject: \[PATCH\] //' | sed 's/^Subject: //')
                git commit -m "cauldron: $SUBJECT (fuzzy)" --author="Cauldron <cauldron@cauldron.app>"
                PATCH_COUNT=$((PATCH_COUNT + 1))
            else
                echo "    FAILED: $PATCH_NAME — manual resolution needed"
                cd "$ROOT"
                continue
            fi
        fi
        cd "$ROOT"
    done
    echo "    Applied $PATCH_COUNT Cauldron patches"
else
    echo "==> No Cauldron patches found"
fi

# --- Step 5: Summary ---
echo ""
echo "=== Cauldron Wine Fork Initialized ==="
cd "$WINE_DIR"
echo "Branch: $CAULDRON_BRANCH"
echo "Base:   $UPSTREAM_BRANCH ($(git log "$UPSTREAM_BRANCH" --oneline -1 2>/dev/null || echo 'unknown'))"
echo "Commits on top:"
git log --oneline "$UPSTREAM_BRANCH..$CAULDRON_BRANCH" 2>/dev/null || git log --oneline -20
echo ""
echo "To build: make wine-build"
