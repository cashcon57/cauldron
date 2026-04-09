# Cauldron Wine Fork — Patch Audit Results

**Date:** 2026-04-06 (updated)
**Base:** Wine 11.6 (latest development — bleeding-edge)
**Tested on:** `localci` isolated macOS user via SSH

## Why 11.6 over 10.0

Wine 11.6 is the latest dev release. CrossOver 26 ships on Wine 9.x/10.x — using 11.6
gives Cauldron a genuine competitive edge (newer APIs, more upstreamed Proton fixes).
11.6 already contains ~60 Proton patches that we'd need to maintain on 10.0, reducing
our patch burden significantly.

## Summary (Wine 11.6)

| Source | Total Tested | Clean Apply | Fuzzy Apply | Conflict | Mergeable |
|--------|-------------|-------------|-------------|----------|-----------|
| wine-staging | 65 | 59 | 1 | 5 | **60** |
| Valve/Proton Wine | 200 | 12 | 10 | 178 | **22** |
| Cauldron (our own) | 1 | 1 | 0 | 0 | **1** |
| **Total** | **266** | **72** | **11** | **183** | **83** |

Note: Proton shows fewer mergeable on 11.6 because Wine upstream already absorbed ~60
Proton patches. This is ideal — fewer patches for us to maintain.

## Previous Audit (Wine 10.0, for reference)

| Source | Mergeable on 10.0 | Mergeable on 11.6 | Delta |
|--------|-------------------|-------------------|-------|
| wine-staging | 48 | 60 | +12 (more patches apply) |
| Valve/Proton | 82 | 22 | -60 (upstreamed into Wine) |
| Cauldron | 1 | 1 | same (rebased) |

## Priority Patches for Cauldron Wine Fork

### Tier 1 — Gaming Critical (apply first)

| # | Source | Patch | Files | Why |
|---|--------|-------|-------|-----|
| 0001 | cauldron | VirtualProtect COW fix | 1 | SKSE/F4SE mod loader compat |
| 0002 | staging | ntdll-APC_Performance | 2 | Performance: async procedure calls |
| 0003 | staging | wined3d-zero-inf-shaders | 3 | Shader compilation fix |
| 0004 | staging | wined3d-unset-flip-gdi | 3 | Display flip/GDI fix |
| 0005 | staging | wined3d-rotate-WINED3D_SWAP_EFFECT_DISCARD | 1 | Swap chain rotation fix |
| 0006 | staging | dxgi_getFrameStatistics | 1 | Frame timing for VSync/perf overlay |
| 0007 | staging | d3dx9_36-D3DXStubs | 21 | D3DX9 stubs for older games |
| 0008 | staging | d3dx9-sprite-state | 1 | Sprite rendering state fix |
| 0009 | staging | ddraw-GetPickRecords | 4 | DirectDraw pick records (older games) |

### Tier 2 — Kernel/Stability

| # | Source | Patch | Files | Why |
|---|--------|-------|-------|-----|
| 0010 | staging | kernel32-CopyFileEx | 2 | File copy progress callbacks |
| 0011 | staging | kernel32-Debugger | 1 | Debugger attachment fix |
| 0012 | staging | kernel32-limit_heap_old_exe | 1 | Heap limit for legacy 32-bit games |
| 0013 | staging | ntdll-Exception | 1 | Exception handling fix |
| 0014 | staging | ntdll-RtlQueryPackageIdentity | 2 | UWP package identity stubs |
| 0015 | staging | vcomp_for_dynamic_init_i8 | 7 | OpenMP parallel for (game engines) |

### Tier 3 — Compatibility/Polish

| # | Source | Patch | Files | Why |
|---|--------|-------|-------|-----|
| 0016 | staging | dbghelp-Debug_Symbols | 2 | Debug symbol loading |
| 0017 | staging | shell32-IconCache | 1 | Shell icon cache |
| 0018 | staging | mountmgr-DosDevices | 3 | Drive letter mounting |
| 0019 | staging | winedbg-Process_Arguments | 1 | Debugger process args |
| 0020 | staging | windowscodecs-GIF_Encoder | 1 | GIF image support |
| 0021 | staging | windowscodecs-TIFF_Support | 1 | TIFF image support |
| 0022 | staging | wine.inf-Dummy_CA_Certificate | 1 | Certificate authority stub |
| 0023 | staging | winecfg-Libraries | 1 | Library override UI |
| 0024 | staging | wintrust-WTHelperGetProvCertFromChain | 2 | Code signing chain helper |

### Tier 4 — Valve/Proton (cherry-pick candidates)

82 patches apply cleanly. Top candidates:

| Source | Patch | Files | Why |
|--------|-------|-------|-----|
| proton | ntdll: Avoid excessive committed range scan in NtProtectVirtualMemory() | 1 | Performance fix |
| proton | ntdll: Fill IOSB in NtUnlockFile() | 3 | File locking correctness |
| proton | ntdll: Set output frame to Rsp - 8 in epilogue on x64 | 2 | Stack unwinding fix |
| proton | ntdll: Fix handling jmp in epilogue unwind on x64 | 2 | Stack unwinding fix |
| proton | win32u: Fill some GPU info in HKLM\Software\Microsoft\DirectX | 1 | GPU detection |
| proton | win32u: Initialize surface with white colour on creation | 1 | Rendering init fix |
| proton | xaudio2: Free effect chain on error return | 1 | Audio memory leak |
| proton | xaudio2_8: Add XAudio2CreateWithVersionInfo() | 5 | Audio API compat |
| proton | windows.storage: Add stub dll | 7 | UWP storage stubs |
| proton | include: Add robuffer.idl | 2 | WinRT buffer API |
| proton | tdh: Add semi-stub for TdhEnumerateProviders() | 8 | ETW tracing stubs |
| proton | iphlpapi: Implement GetOwnerModuleFromTcpEntry() | 5 | Network API |
| proton | kernelbase: Add synchronization barrier stubs | 6 | Threading stubs |
| proton | ntdll: Add synchronization barrier stubs | 5 | Threading stubs |

## Graphics Component Wine Requirements

| Component | Wine Patches? | Details |
|-----------|--------------|---------|
| KosmicKrisp | **No** | Must build Wine against `libvulkan.dylib` (not `libMoltenVK`). Set `VK_DRIVER_FILES` at runtime. |
| DXVK-macOS | **No** | Drop-in DLL replacement into prefix |
| DXMT | **Yes (2 patches)** | 1) Expose `winemac.drv` hidden symbols. 2) Add `winemetal.dll`/`winemetal.so` module. |
| MoltenVK | **No** | Works unmodified; `winemac.drv/vulkan.c` already handles both surface extensions |
| D3DMetal | **No** (proprietary) | Apple's GPTK component, can't redistribute. Detect from CrossOver/GPTK install. |

**Build note:** `configure.ac` checks for `libvulkan` first, falls back to `libMoltenVK`. Cauldron Wine
must be built with the Vulkan SDK's loader available so KosmicKrisp/DXVK can work via the ICD mechanism.

## Not Mergeable (Need Work)

### wine-msync (all 4 conflict)
MSync patches target CrossOver's Wine fork, not upstream. They need rebasing
onto Wine 10.0. The `msync-devel.patch` is the most relevant (5K lines).

### winemac.drv-no-flicker-patch (staging, macOS-specific, conflicts)
Directly relevant to macOS rendering but conflicts with Wine 10.0.
Needs manual rebase.

### proton-slr macOS patches (37 conflict)
Most of these are large macOS-specific patches (Rosetta2 nop handlers,
ring0 emulation, DXMT integration) that were written against a different
Wine base. Very valuable content but needs significant rebasing work.

## Next Steps

1. Apply Tier 1-3 patches (24 total) — already tested cumulative
2. Cherry-pick top Proton patches — need cumulative test
3. Rebase wine-msync onto Wine 10.0 — significant effort
4. Rebase winemac.drv-no-flicker-patch — moderate effort
5. Evaluate proton-slr macOS hacks (Rosetta2, DXMT) — research needed
