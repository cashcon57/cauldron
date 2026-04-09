#!/usr/bin/env bash
# build_patch_series.sh — Build the complete Cauldron Wine patch series
# Applies patches cumulatively in priority order, commits each, stops on conflict.
#
# Run on CI machine after cloning all sources to /tmp/cauldron-series/
set -euo pipefail

WINE="/tmp/cauldron-series/wine"
STAGING="/tmp/cauldron-series/staging"
OPENGLFREAK="/tmp/cauldron-series/openglfreak"
WINE_TKG="/tmp/cauldron-series/wine-tkg"
PROTON_GE="/tmp/cauldron-series/proton-ge"
VALVE="/tmp/cauldron-series/valve-wine"
ARM64EC="/tmp/cauldron-series/arm64ec"
LOG="/tmp/cauldron-series/build.log"
PASS=0; FAIL=0; SKIP=0

: > "$LOG"

apply_staging() {
    local name="$1"
    local pdir="$STAGING/patches/$name"
    [ -d "$pdir" ] || { echo "  SKIP $name (dir not found)"; SKIP=$((SKIP+1)); return 1; }
    local pfiles=$(find "$pdir" -name "*.patch" -type f | sort)
    [ -z "$pfiles" ] && { echo "  SKIP $name (no patches)"; SKIP=$((SKIP+1)); return 1; }
    local combined="/tmp/_staging_${name}.patch"
    cat $pfiles > "$combined"
    cd "$WINE"
    if git apply --check "$combined" 2>/dev/null; then
        git apply "$combined" && git add -A && \
        git commit -m "staging: $name" --author="wine-staging <staging@winehq.org>" --quiet && \
        echo "  ✓ staging: $name" && PASS=$((PASS+1))
        rm -f "$combined"; return 0
    elif git apply --check -C1 "$combined" 2>/dev/null; then
        git apply -C1 "$combined" && git add -A && \
        git commit -m "staging: $name (fuzzy)" --author="wine-staging <staging@winehq.org>" --quiet && \
        echo "  ~ staging: $name (fuzzy)" && PASS=$((PASS+1))
        rm -f "$combined"; return 0
    else
        echo "  ✗ staging: $name (CONFLICT after previous patches)" | tee -a "$LOG"
        FAIL=$((FAIL+1)); rm -f "$combined"; return 1
    fi
}

apply_file() {
    local src="$1" name="$2" pfile="$3"
    [ -s "$pfile" ] || { SKIP=$((SKIP+1)); return 1; }
    cd "$WINE"
    if git apply --check "$pfile" 2>/dev/null; then
        git apply "$pfile" && git add -A && \
        git commit -m "$src: $name" --author="$src <$src@cauldron>" --quiet && \
        echo "  ✓ $src: $name" && PASS=$((PASS+1)); return 0
    elif git apply --check -C1 "$pfile" 2>/dev/null; then
        git apply -C1 "$pfile" && git add -A && \
        git commit -m "$src: $name (fuzzy)" --author="$src <$src@cauldron>" --quiet && \
        echo "  ~ $src: $name (fuzzy)" && PASS=$((PASS+1)); return 0
    else
        echo "  ✗ $src: $name (CONFLICT)" | tee -a "$LOG"
        FAIL=$((FAIL+1)); return 1
    fi
}

apply_fork_commit() {
    local src="$1" repo="$2" hash="$3"
    local msg=$(cd "$repo" && git log -1 --format="%s" "$hash" 2>/dev/null | head -c 80)
    local f="/tmp/_fork_${hash:0:8}.patch"
    (cd "$repo" && git format-patch -1 --stdout "$hash" > "$f" 2>/dev/null) || { SKIP=$((SKIP+1)); return 1; }
    apply_file "$src" "$msg" "$f"
    rm -f "$f"
}

echo "================================================================"
echo "  CAULDRON WINE PATCH SERIES BUILD"
echo "  Base: Wine 11.6"
echo "  Date: $(date)"
echo "================================================================"
echo ""

########################################
echo "=== TIER 0: Cauldron Own Patches ==="
########################################
apply_file "cauldron" "VirtualProtect-COW-fix" "/Users/cashconway/cauldron/patches/cauldron/0001-ntdll-Preserve-private-pages-on-VirtualProtect.patch"

########################################
echo ""
echo "=== TIER 1: Gaming Critical (wine-staging) ==="
########################################
for name in \
    winemac.drv-no-flicker-patch \
    ntdll-APC_Performance \
    wined3d-zero-inf-shaders \
    wined3d-unset-flip-gdi \
    wined3d-rotate-WINED3D_SWAP_EFFECT_DISCARD \
    dxgi_getFrameStatistics \
    d3dx9_36-D3DXStubs \
    d3dx9-sprite-state \
    ddraw-GetPickRecords \
    ntdll-Hide_Wine_Exports \
; do apply_staging "$name"; done

########################################
echo ""
echo "=== TIER 2: Kernel/Stability (wine-staging) ==="
########################################
for name in \
    kernel32-CopyFileEx \
    kernel32-Debugger \
    kernel32-limit_heap_old_exe \
    ntdll-Exception \
    ntdll-RtlQueryPackageIdentity \
    ntdll-NtDevicePath \
    ntdll-Serial_Port_Detection \
    vcomp_for_dynamic_init_i8 \
    server-PeekMessage \
    server-Signal_Thread \
; do apply_staging "$name"; done

########################################
echo ""
echo "=== TIER 3: Compatibility (wine-staging) ==="
########################################
for name in \
    dbghelp-Debug_Symbols \
    shell32-IconCache \
    shell32-ACE_Viewer \
    mountmgr-DosDevices \
    winedbg-Process_Arguments \
    windowscodecs-GIF_Encoder \
    windowscodecs-TIFF_Support \
    wine.inf-Dummy_CA_Certificate \
    winecfg-Libraries \
    wintrust-WTHelperGetProvCertFromChain \
    oleaut32-CreateTypeLib \
    oleaut32-default-pic-size \
    oleaut32_VarAdd \
    oleaut32_typelib_dispatch \
    riched20-IText_Interface \
    richedit20-ImportDataObject \
    comctl32-rebar-capture \
    comdlg32-lpstrFileTitle \
    msxml3-whitespace \
    msxml3-write_out_doc \
    msxml3_embedded_cdata \
    msxml3_encode_gb2312 \
    sapi-ISpObjectToken-CreateInstance \
    stdole32.idl-Typelib \
    user32-DrawTextExW \
    user32-message-order \
    version-VerQueryValue \
    winmm-mciSendCommandA \
    explorer-Video_Registry_Key \
    inseng-Implementation \
    mshtml-TranslateAccelerator \
    msi-cabinet \
    winepulse-PulseAudio_Support \
    winepulse-aux_channels \
    winex11-Window_Style \
    winex11-ime-check-thread-data \
    dmime_segment_getaudiopath \
    dmscript_enum_routine \
; do apply_staging "$name"; done

########################################
echo ""
echo "=== TIER 4: Performance (openglfreak) ==="
########################################
for p in \
    0001-ntdll-Read-Qpc-frequency-from-user-shared-data \
    0002-ntdll-Use-rdtsc-p-for-RtlQueryPerformanceCounter-whe \
    0004-ntdll-Prefer-RtlQueryPerformanceCounter-over-NtQuery \
    0005-hal-Prefer-RtlQueryPerformanceCounter-over-NtQueryPe \
    0006-kernelbase-Prefer-RtlQueryPerformanceCounter-over-Nt \
; do
    f=$(find "$OPENGLFREAK" -name "${p}*" -type f | head -1)
    [ -n "$f" ] && apply_file "openglfreak" "$p" "$f"
done

########################################
echo ""
echo "=== TIER 5: Spec/API Fixes (openglfreak) ==="
########################################
for p in \
    0003-ntdll-tests-Add-tests-for-RtlWaitOnAddress-and-Keyed \
    ps0001-p0003-kernelbase-Fix-some-spec-file-entries \
    ps0001-p0004-shell32-Fix-some-spec-file-entries \
    ps0001-p0006-oleaut32-Fix-some-spec-file-entries \
    ps0002-include-Add-include-guard-in-devguid.h \
    ps0003-include-Add-ndisguid.h \
    ps0003-secur32-Disable-CHACHA20-POLY1305-ciphersuites \
    ps0005-crypt32-Hash-the-SubjectPublicKeyInfo-as-fallback- \
    ps0005-include-Add-new-fields-for-SYSTEM_PEFORMANCE_INFOR \
    ps0008-msvcp90-Add-manifest \
    ps0008-secur32-Initialize-SECBUFFER_ALERT-type-output-buf \
    ps0009-p0008-ncrypt-Create-ncrypt-storage-properties-defi \
    0005-user32-tests-Remove-SetForegroundWindow-success-chec \
; do
    f=$(find "$OPENGLFREAK" -name "${p}*" -type f | head -1)
    [ -n "$f" ] && apply_file "openglfreak" "$p" "$f"
done

########################################
echo ""
echo "=== TIER 6: Valve/Proton Cherry-picks ==="
########################################
cd "$VALVE"
git remote add upstream https://github.com/wine-mirror/wine.git 2>/dev/null || true
git fetch upstream wine-11.6 --depth=1 2>/dev/null
for h in $(git log --reverse FETCH_HEAD..HEAD --format="%H" 2>/dev/null | head -200); do
    f="/tmp/_valve_${h:0:8}.patch"
    git format-patch -1 --stdout "$h" > "$f" 2>/dev/null || continue
    [ -s "$f" ] || { rm -f "$f"; continue; }
    # Only try patches that passed individual test
    cd "$WINE"
    if git apply --check "$f" 2>/dev/null || git apply --check -C1 "$f" 2>/dev/null; then
        msg=$(cd "$VALVE" && git log -1 --format="%s" "$h" | head -c 80)
        apply_file "proton" "$msg" "$f"
    fi
    rm -f "$f"
    cd "$VALVE"
done

########################################
echo ""
echo "=== TIER 7: ARM64EC Portable Patches ==="
########################################
cd "$ARM64EC"
git remote add upstream https://github.com/wine-mirror/wine.git 2>/dev/null || true
git fetch upstream wine-11.6 --depth=1 2>/dev/null
for h in $(git log --reverse FETCH_HEAD..HEAD --format="%H" 2>/dev/null | head -100); do
    f="/tmp/_a64_${h:0:8}.patch"
    git format-patch -1 --stdout "$h" > "$f" 2>/dev/null || continue
    [ -s "$f" ] || { rm -f "$f"; continue; }
    cd "$WINE"
    if git apply --check "$f" 2>/dev/null || git apply --check -C1 "$f" 2>/dev/null; then
        msg=$(cd "$ARM64EC" && git log -1 --format="%s" "$h" | head -c 80)
        apply_file "arm64ec" "$msg" "$f"
    fi
    rm -f "$f"
    cd "$ARM64EC"
done

########################################
echo ""
echo "=== TIER 8: Proton-GE Cherry-picks ==="
########################################
for p in $(find "$PROTON_GE/patches" -name "*.patch" -type f 2>/dev/null | head -50); do
    name=$(basename "$p" .patch)
    cd "$WINE"
    if git apply --check "$p" 2>/dev/null || git apply --check -C1 "$p" 2>/dev/null; then
        apply_file "proton-ge" "$name" "$p"
    fi
done

########################################
echo ""
echo "=== TIER 9: wine-tkg ==="
########################################
for p in $(find "$WINE_TKG" -name "*.patch" -path "*/wine-tkg-patches/*" -type f 2>/dev/null | head -50); do
    name=$(basename "$p" .patch)
    cd "$WINE"
    if git apply --check "$p" 2>/dev/null || git apply --check -C1 "$p" 2>/dev/null; then
        apply_file "wine-tkg" "$name" "$p"
    fi
done

########################################
echo ""
echo "================================================================"
echo "  RESULTS"
echo "================================================================"
########################################
cd "$WINE"
TOTAL_COMMITS=$(git log --oneline wine-11.6..cauldron/main 2>/dev/null | wc -l | tr -d ' ')
echo "Patches applied: $PASS"
echo "Patches failed:  $FAIL"
echo "Patches skipped: $SKIP"
echo "Total commits:   $TOTAL_COMMITS"
echo ""
echo "=== COMMIT LOG ==="
git log --oneline wine-11.6..cauldron/main 2>/dev/null
echo ""
echo "=== FAILURES ==="
cat "$LOG"
