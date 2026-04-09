#!/usr/bin/env bash
# test_all_patches.sh — Systematically test all available patches against Wine 10.0
#
# Sources tested:
# 1. Proton (ValveSoftware/Proton) — Wine submodule patches vs upstream
# 2. CrossOver (CodeWeavers Wine fork) — macOS-specific patches
# 3. wine-staging — curated community patches
# 4. wine-msync — macOS synchronization primitives
# 5. Cauldron's own patches
#
# Output: JSON report + human-readable summary

set -euo pipefail

WORKDIR="/tmp/cauldron-patch-test"
WINE_BASE="$WORKDIR/wine-base"
REPORT="$WORKDIR/patch-report.json"
SUMMARY="$WORKDIR/patch-summary.txt"
PATCH_DIR="$WORKDIR/extracted-patches"

mkdir -p "$PATCH_DIR"

echo "[]" > "$REPORT"
: > "$SUMMARY"

log() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$SUMMARY"; }

# Test a single patch file against wine-base
# Usage: test_patch <source> <name> <patch_file> <category>
test_patch() {
    local source="$1" name="$2" patch_file="$3" category="$4"
    local result="skip" reason="" files_changed=0 lines_added=0 lines_removed=0

    if [[ ! -f "$patch_file" ]]; then
        result="error"
        reason="Patch file not found"
    elif [[ ! -s "$patch_file" ]]; then
        result="skip"
        reason="Empty patch"
    else
        lines_added=$(grep -c '^+' "$patch_file" 2>/dev/null || echo 0)
        lines_removed=$(grep -c '^-' "$patch_file" 2>/dev/null || echo 0)
        files_changed=$(grep -c '^diff --git' "$patch_file" 2>/dev/null || echo 0)

        cd "$WINE_BASE"
        if git apply --check "$patch_file" 2>/dev/null; then
            result="clean"
            reason="Applies cleanly"
        elif git apply --check -C1 "$patch_file" 2>/dev/null; then
            result="fuzzy"
            reason="Applies with reduced context"
        elif git apply --check --3way "$patch_file" 2>/dev/null; then
            result="3way"
            reason="Applies with 3-way merge"
        else
            result="conflict"
            reason=$(git apply --check "$patch_file" 2>&1 | head -3 | tr '\n' ' ')
        fi
    fi

    # Classify relevance to macOS
    local macos_relevant="unknown"
    local patch_content=""
    if [[ -f "$patch_file" ]]; then
        patch_content=$(cat "$patch_file" 2>/dev/null || echo "")
    fi

    if echo "$patch_content" | grep -qi "darwin\|macos\|__APPLE__\|CoreAudio\|Metal\|macdrv\|msync\|MSync\|mach_\|kqueue\|IOKit"; then
        macos_relevant="direct"
    elif echo "$patch_content" | grep -qi "ntdll\|kernel32\|d3d\|dxgi\|wined3d\|vulkan\|opengl\|server/"; then
        macos_relevant="portable"
    elif echo "$patch_content" | grep -qi "futex\|eventfd\|epoll\|__linux__\|/proc/\|timerfd\|signalfd"; then
        macos_relevant="linux_only"
    elif echo "$patch_content" | grep -qi "configure\|Makefile\|\.gitlab\|AUTHORS\|MAINTAINERS"; then
        macos_relevant="build_infra"
    else
        macos_relevant="portable"
    fi

    # Append to JSON report
    python3 -c "
import json, sys
with open('$REPORT', 'r') as f:
    data = json.load(f)
data.append({
    'source': '$source',
    'name': '''${name//\'/\\\'}''',
    'category': '$category',
    'result': '$result',
    'reason': '''${reason//\'/\\\'}''',
    'macos_relevant': '$macos_relevant',
    'files_changed': $files_changed,
    'lines_added': $lines_added,
    'lines_removed': $lines_removed,
})
with open('$REPORT', 'w') as f:
    json.dump(data, f, indent=2)
" 2>/dev/null || true

    local status_icon="?"
    case "$result" in
        clean) status_icon="✓" ;;
        fuzzy) status_icon="~" ;;
        3way)  status_icon="≈" ;;
        conflict) status_icon="✗" ;;
        skip) status_icon="-" ;;
        error) status_icon="!" ;;
    esac

    echo "  $status_icon [$macos_relevant] $name ($result)" | tee -a "$SUMMARY"
}

###############################################################################
# 1. PROTON — Extract Wine patches from Proton's wine submodule
###############################################################################
log "=== PROTON PATCHES ==="

if [[ -d "$WORKDIR/proton" ]]; then
    cd "$WORKDIR/proton"

    # Proton stores Wine as a submodule. The interesting patches are commits
    # in the proton branch that touch Wine files. Extract them as diffs.
    # Since we cloned shallow, look at recent commits that modify wine-related paths.

    PROTON_PATCH_DIR="$PATCH_DIR/proton"
    mkdir -p "$PROTON_PATCH_DIR"

    # Get commits from proton that have Wine-related changes
    # Proton's structure: the wine/ submodule has its own history
    # But the proton repo itself has config files, protonfixes, etc.
    # The real Wine patches are in Proton's wine submodule.

    # Check if wine submodule exists
    if [[ -f ".gitmodules" ]] && grep -q "wine" .gitmodules; then
        log "Proton has Wine submodule — extracting proton-specific configs..."

        # Extract Proton's game config patches (proton_10.0 branch)
        PROTON_COUNT=0
        for f in $(git log --oneline --name-only -200 2>/dev/null | grep -E '\.py$|\.json$|compatibilitytool|proton$|toolmanifest' | sort -u | head -50); do
            if [[ -f "$f" ]]; then
                PROTON_COUNT=$((PROTON_COUNT + 1))
            fi
        done
        log "  Found $PROTON_COUNT Proton config files (not Wine patches)"

        # Extract actual Wine-level commits from Proton's log
        git log --oneline -200 --format="%H %s" 2>/dev/null | while read hash msg; do
            # Generate diff for each commit
            diff_file="$PROTON_PATCH_DIR/${hash:0:8}.patch"
            git format-patch -1 --stdout "$hash" > "$diff_file" 2>/dev/null || continue
            # Only keep if it touches Wine-related files
            if grep -q 'dlls/\|server/\|loader/\|programs/' "$diff_file" 2>/dev/null; then
                short_msg=$(echo "$msg" | head -c 80)
                test_patch "proton" "$short_msg" "$diff_file" "wine_patch"
            else
                rm -f "$diff_file"
            fi
        done
    else
        log "  No Wine submodule found in Proton repo"
        # Proton repo might have direct wine patches in its tree
        git log --oneline -200 --format="%H %s" 2>/dev/null | head -100 | while read hash msg; do
            diff_file="$PROTON_PATCH_DIR/${hash:0:8}.patch"
            git format-patch -1 --stdout "$hash" > "$diff_file" 2>/dev/null || continue
            if [[ -s "$diff_file" ]]; then
                short_msg=$(echo "$msg" | head -c 80)
                test_patch "proton" "$short_msg" "$diff_file" "proton_config"
            fi
        done
    fi
else
    log "  Proton repo not available"
fi

###############################################################################
# 2. CROSSOVER — Extract macOS-specific patches from CodeWeavers' Wine fork
###############################################################################
log ""
log "=== CROSSOVER PATCHES ==="

if [[ -d "$WORKDIR/crossover-wine" ]]; then
    cd "$WORKDIR/crossover-wine"

    CROSSOVER_PATCH_DIR="$PATCH_DIR/crossover"
    mkdir -p "$CROSSOVER_PATCH_DIR"

    # CrossOver's Wine fork diverges from upstream. We want the delta.
    # Since we cloned shallow, extract recent commits.
    CX_COUNT=0
    git log --oneline -200 --format="%H %s" 2>/dev/null | while read hash msg; do
        diff_file="$CROSSOVER_PATCH_DIR/${hash:0:8}.patch"
        git format-patch -1 --stdout "$hash" > "$diff_file" 2>/dev/null || continue
        if [[ -s "$diff_file" ]]; then
            short_msg=$(echo "$msg" | head -c 80)
            test_patch "crossover" "$short_msg" "$diff_file" "crossover_wine"
            CX_COUNT=$((CX_COUNT + 1))
        fi
    done
    log "  Tested CrossOver patches"
else
    log "  CrossOver Wine repo not available"
fi

###############################################################################
# 3. WINE-STAGING — Curated community patches
###############################################################################
log ""
log "=== WINE-STAGING PATCHES ==="

if [[ -d "$WORKDIR/wine-staging" ]]; then
    cd "$WORKDIR/wine-staging"

    STAGING_PATCH_DIR="$PATCH_DIR/staging"
    mkdir -p "$STAGING_PATCH_DIR"

    # wine-staging organizes patches in directories under patches/
    # Each directory is a patchset with numbered .patch files
    STAGING_TESTED=0
    for patchset_dir in patches/*/; do
        patchset_name=$(basename "$patchset_dir")

        # Skip disabled/meta patchsets
        [[ "$patchset_name" == "Staging" ]] && continue
        [[ "$patchset_name" == "Compiler_Warnings" ]] && continue

        # Combine all patches in the patchset into one
        combined="$STAGING_PATCH_DIR/${patchset_name}.patch"
        cat "$patchset_dir"/*.patch > "$combined" 2>/dev/null || continue

        if [[ -s "$combined" ]]; then
            test_patch "staging" "$patchset_name" "$combined" "staging_patchset"
            STAGING_TESTED=$((STAGING_TESTED + 1))
        fi
    done
    log "  Tested $STAGING_TESTED staging patchsets"
else
    log "  wine-staging repo not available"
fi

###############################################################################
# 4. WINE-MSYNC — macOS synchronization patches
###############################################################################
log ""
log "=== WINE-MSYNC PATCHES ==="

MSYNC_DIR="$HOME/cauldron/deps/wine-msync"
if [[ ! -d "$MSYNC_DIR" ]]; then
    MSYNC_DIR="/Users/cashconway/cauldron/deps/wine-msync"
fi

if [[ -d "$MSYNC_DIR" ]]; then
    for patch in "$MSYNC_DIR"/*.patch; do
        name=$(basename "$patch" .patch)
        test_patch "msync" "$name" "$patch" "sync_primitive"
    done
else
    log "  wine-msync not available"
fi

###############################################################################
# 5. CAULDRON OWN PATCHES — Already in our series
###############################################################################
log ""
log "=== CAULDRON PATCHES ==="

CAULDRON_PATCHES="$HOME/cauldron/patches/cauldron"
if [[ ! -d "$CAULDRON_PATCHES" ]]; then
    CAULDRON_PATCHES="/Users/cashconway/cauldron/patches/cauldron"
fi

if [[ -d "$CAULDRON_PATCHES" ]]; then
    for patch in "$CAULDRON_PATCHES"/*.patch; do
        name=$(basename "$patch" .patch)
        test_patch "cauldron" "$name" "$patch" "cauldron_fix"
    done
else
    log "  Cauldron patches not found"
fi

###############################################################################
# SUMMARY
###############################################################################
log ""
log "=== SUMMARY ==="

python3 << 'PYEOF'
import json

with open("/tmp/cauldron-patch-test/patch-report.json") as f:
    data = json.load(f)

total = len(data)
by_result = {}
by_source = {}
by_relevance = {}
mergeable = []

for p in data:
    r = p["result"]
    s = p["source"]
    rel = p["macos_relevant"]

    by_result[r] = by_result.get(r, 0) + 1
    by_source[s] = by_source.get(s, 0) + 1
    by_relevance[rel] = by_relevance.get(rel, 0) + 1

    # Mergeable = applies cleanly/fuzzy AND is macOS-relevant or portable
    if r in ("clean", "fuzzy", "3way") and rel in ("direct", "portable"):
        mergeable.append(p)

print(f"Total patches tested: {total}")
print(f"")
print("By result:")
for k, v in sorted(by_result.items(), key=lambda x: -x[1]):
    print(f"  {k}: {v}")
print(f"")
print("By source:")
for k, v in sorted(by_source.items(), key=lambda x: -x[1]):
    print(f"  {k}: {v}")
print(f"")
print("By macOS relevance:")
for k, v in sorted(by_relevance.items(), key=lambda x: -x[1]):
    print(f"  {k}: {v}")
print(f"")
print(f"=== MERGEABLE PATCHES: {len(mergeable)} ===")
print(f"(Applies cleanly + macOS relevant or portable)")
print(f"")

# Group mergeable by source
for source in sorted(set(p["source"] for p in mergeable)):
    patches = [p for p in mergeable if p["source"] == source]
    print(f"--- {source} ({len(patches)}) ---")
    for p in patches[:30]:  # limit output
        rel_tag = "macOS" if p["macos_relevant"] == "direct" else "portable"
        print(f"  [{rel_tag}] {p['name']} ({p['result']}, +{p['lines_added']}/-{p['lines_removed']})")
    if len(patches) > 30:
        print(f"  ... and {len(patches) - 30} more")
    print()

# Save mergeable list
with open("/tmp/cauldron-patch-test/mergeable.json", "w") as f:
    json.dump(mergeable, f, indent=2)

print(f"Full report: /tmp/cauldron-patch-test/patch-report.json")
print(f"Mergeable list: /tmp/cauldron-patch-test/mergeable.json")
PYEOF
