#!/usr/bin/env bash
# audit_all_11.6.sh — Test all patch sources against Wine 11.6
set -euo pipefail

W="/tmp/cauldron-audit2/wine-base"
RESULTS="/tmp/cauldron-audit2/all-results.txt"
: > "$RESULTS"

test_patch() {
    local src="$1" name="$2" pfile="$3"
    [ -s "$pfile" ] || return
    cd "$W"
    local result="CONFLICT"
    if git apply --check "$pfile" 2>/dev/null; then result="CLEAN"
    elif git apply --check -C1 "$pfile" 2>/dev/null; then result="FUZZY"; fi
    local macos="portable"
    if grep -qi "darwin\|__APPLE__\|macdrv\|msync\|CoreAudio\|Metal\|mach_\|kqueue\|rosetta\|Rosetta" "$pfile" 2>/dev/null; then macos="macOS"
    elif grep -qi "futex\|eventfd\|epoll\|__linux__\|timerfd\|signalfd" "$pfile" 2>/dev/null; then macos="linux"; fi
    local files=$(grep -c "^diff --git" "$pfile" 2>/dev/null || echo 0)
    echo "$result|$macos|$src|$name|$files" >> "$RESULTS"
    [ "$result" != "CONFLICT" ] && echo "  ✓ [$macos] $name ($result, $files files)" || true
}

########################################
echo "=== 1. CROSSOVER (Gcenx/winecx) ==="
########################################
if [ -d /tmp/cauldron-audit2/winecx/.git ]; then
    cd /tmp/cauldron-audit2/winecx
    git remote add upstream https://github.com/wine-mirror/wine.git 2>/dev/null || true
    git fetch upstream wine-11.6 --depth=1 2>&1 | tail -1
    CX=$(git log --oneline FETCH_HEAD..HEAD 2>/dev/null | wc -l | tr -d ' ')
    echo "CrossOver-only commits: $CX"
    git log --oneline --reverse FETCH_HEAD..HEAD --format="%H" 2>/dev/null | head -200 | while read hash; do
        msg=$(git log -1 --format="%s" "$hash" 2>/dev/null | head -c 100)
        f="/tmp/cx-${hash:0:8}.patch"
        git format-patch -1 --stdout "$hash" > "$f" 2>/dev/null || continue
        test_patch "crossover" "$msg" "$f"
        rm -f "$f"
    done
else echo "  SKIP: winecx not cloned"; fi

########################################
echo ""
echo "=== 2. ZAKK4223 macOS PATCHES ==="
########################################
if [ -d /tmp/cauldron-audit2/zakk-patches ]; then
    for p in /tmp/cauldron-audit2/zakk-patches/*.patch /tmp/cauldron-audit2/zakk-patches/**/*.patch; do
        [ -f "$p" ] || continue
        name=$(basename "$p" .patch)
        test_patch "zakk4223" "$name" "$p"
    done
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 3. WINEROSETTA ==="
########################################
if [ -d /tmp/cauldron-audit2/winerosetta ]; then
    for p in /tmp/cauldron-audit2/winerosetta/*.patch /tmp/cauldron-audit2/winerosetta/**/*.patch; do
        [ -f "$p" ] || continue
        name=$(basename "$p" .patch)
        test_patch "winerosetta" "$name" "$p"
    done
    # Also check if it's a Wine fork with commits
    if [ -d /tmp/cauldron-audit2/winerosetta/.git ]; then
        cd /tmp/cauldron-audit2/winerosetta
        # Check for C source files that indicate Wine patches
        ls *.c *.h 2>/dev/null | head -3 && echo "  (library, not patches)"
    fi
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 4. WINEROSETTA2 ==="
########################################
if [ -d /tmp/cauldron-audit2/winerosetta2 ]; then
    cd /tmp/cauldron-audit2/winerosetta2
    echo "  Structure: $(ls | head -10 | tr '\n' ' ')"
    for p in $(find /tmp/cauldron-audit2/winerosetta2 -name "*.patch" -type f 2>/dev/null); do
        [ -f "$p" ] || continue
        name=$(basename "$p" .patch)
        test_patch "winerosetta2" "$name" "$p"
    done
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 5. ARM64EC (fathonix) ==="
########################################
if [ -d /tmp/cauldron-audit2/arm64ec/.git ]; then
    cd /tmp/cauldron-audit2/arm64ec
    git remote add upstream https://github.com/wine-mirror/wine.git 2>/dev/null || true
    git fetch upstream wine-11.6 --depth=1 2>&1 | tail -1
    A64=$(git log --oneline FETCH_HEAD..HEAD 2>/dev/null | wc -l | tr -d ' ')
    echo "ARM64EC-only commits: $A64"
    git log --oneline --reverse FETCH_HEAD..HEAD --format="%H" 2>/dev/null | head -100 | while read hash; do
        msg=$(git log -1 --format="%s" "$hash" 2>/dev/null | head -c 100)
        f="/tmp/a64-${hash:0:8}.patch"
        git format-patch -1 --stdout "$hash" > "$f" 2>/dev/null || continue
        test_patch "arm64ec" "$msg" "$f"
        rm -f "$f"
    done
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 6. WINE-TKG PATCHES ==="
########################################
if [ -d /tmp/cauldron-audit2/wine-tkg ]; then
    # wine-tkg stores patches in wine-tkg-git/wine-tkg-patches/
    TPDIR="/tmp/cauldron-audit2/wine-tkg/wine-tkg-git/wine-tkg-patches"
    if [ -d "$TPDIR" ]; then
        for p in "$TPDIR"/*.patch "$TPDIR"/**/*.patch; do
            [ -f "$p" ] || continue
            name=$(basename "$p" .patch)
            test_patch "wine-tkg" "$name" "$p"
        done
    else
        echo "  Patches dir not found, checking structure..."
        find /tmp/cauldron-audit2/wine-tkg -name "*.patch" -type f 2>/dev/null | head -5
    fi
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 7. OPENGLFREAK USERPATCHES ==="
########################################
if [ -d /tmp/cauldron-audit2/openglfreak ]; then
    for p in /tmp/cauldron-audit2/openglfreak/*.patch /tmp/cauldron-audit2/openglfreak/**/*.patch; do
        [ -f "$p" ] || continue
        name=$(basename "$p" .patch)
        test_patch "openglfreak" "$name" "$p"
    done
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 8. WINE-NSPA ==="
########################################
if [ -d /tmp/cauldron-audit2/wine-nspa/.git ]; then
    cd /tmp/cauldron-audit2/wine-nspa
    git remote add upstream https://github.com/wine-mirror/wine.git 2>/dev/null || true
    git fetch upstream wine-11.6 --depth=1 2>&1 | tail -1
    NSPA=$(git log --oneline FETCH_HEAD..HEAD 2>/dev/null | wc -l | tr -d ' ')
    echo "NSPA-only commits: $NSPA"
    git log --oneline --reverse FETCH_HEAD..HEAD --format="%H" 2>/dev/null | head -100 | while read hash; do
        msg=$(git log -1 --format="%s" "$hash" 2>/dev/null | head -c 100)
        f="/tmp/nspa-${hash:0:8}.patch"
        git format-patch -1 --stdout "$hash" > "$f" 2>/dev/null || continue
        test_patch "wine-nspa" "$msg" "$f"
        rm -f "$f"
    done
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "=== 9. PROTON-GE-CUSTOM ==="
########################################
if [ -d /tmp/cauldron-audit2/proton-ge ]; then
    # proton-ge stores Wine patches in patches/wine/
    GEDIR="/tmp/cauldron-audit2/proton-ge/patches/wine"
    if [ -d "$GEDIR" ]; then
        for p in "$GEDIR"/*.patch; do
            [ -f "$p" ] || continue
            name=$(basename "$p" .patch)
            test_patch "proton-ge" "$name" "$p"
        done
    else
        echo "  Checking structure..."
        find /tmp/cauldron-audit2/proton-ge -name "*.patch" -type f 2>/dev/null | head -10
    fi
else echo "  SKIP: not cloned"; fi

########################################
echo ""
echo "========================================="
echo "=== COMPREHENSIVE RESULTS ==="
echo "========================================="
########################################
total=$(wc -l < "$RESULTS" | tr -d ' ')
clean=$(grep -c "^CLEAN" "$RESULTS" || echo 0)
fuzzy=$(grep -c "^FUZZY" "$RESULTS" || echo 0)
conflict=$(grep -c "^CONFLICT" "$RESULTS" || echo 0)
echo "Total patches tested: $total"
echo "Clean: $clean  Fuzzy: $fuzzy  Conflict: $conflict"
echo ""
echo "--- BY SOURCE ---"
for src in crossover zakk4223 winerosetta winerosetta2 arm64ec wine-tkg openglfreak wine-nspa proton-ge; do
    t=$(grep "|$src|" "$RESULTS" | wc -l | tr -d ' ')
    c=$(grep "^CLEAN.*|$src|" "$RESULTS" | wc -l | tr -d ' ')
    f=$(grep "^FUZZY.*|$src|" "$RESULTS" | wc -l | tr -d ' ')
    [ "$t" -gt 0 ] && echo "  $src: $t tested, $c clean, $f fuzzy"
done
echo ""
echo "--- ALL MERGEABLE (clean + fuzzy, not linux-only) ---"
grep -E "^(CLEAN|FUZZY)" "$RESULTS" | grep -v "|linux|" | sort -t'|' -k3,3 -k4,4
echo ""
echo "--- macOS DIRECT ---"
grep "|macOS|" "$RESULTS" | grep -E "^(CLEAN|FUZZY)" | sort -t'|' -k3,3
