# Cauldron — Architecture & Project Plan

> Bleeding-edge macOS game compatibility layer — Proton-synced, Rust-powered, Metal-native.
> v0.2 — April 2026

---

## 1. Executive Summary

Cauldron is an open-source macOS application that provides bleeding-edge Windows game compatibility, inspired by Proton-GE on Linux. The project forks the now-archived Whisky app as a starting point, replaces its core with a Rust engine, and builds an automated pipeline that continuously syncs fixes from Valve's Proton into a macOS-compatible format.

The core innovation is a **Proton Compatibility Mapping System**: an automated toolchain that monitors Proton commits, classifies them by subsystem (Wine API, graphics, kernel, game-specific), and either applies them directly or translates them to macOS equivalents. Combined with a hybrid graphics backend that auto-selects between five DirectX translation paths per game, Cauldron offers macOS gamers the most up-to-date compatibility layer available.

---

## 2. Problem Statement

macOS gaming compatibility suffers from three structural problems:

- **Staleness.** CrossOver updates lag weeks to months behind Proton. Whisky is archived entirely. By the time a fix lands on macOS, Linux users have had it for a long time.
- **Translation gap.** Proton's fixes assume a Linux environment with native Vulkan, futex-based synchronization, and specific driver behaviors. No one systematically maps these assumptions to macOS equivalents.
- **No community-driven bleeding edge.** Linux has Proton-GE, which cherry-picks community patches, experimental fixes, and game-specific hacks faster than official Proton. macOS has nothing equivalent.

---

## 3. System Architecture

### 3.1 High-Level Architecture

Cauldron is structured as four layers:

| Layer | Language | Responsibility |
|---|---|---|
| **UI Shell** | Swift / SwiftUI | Native macOS interface, bottle management, game library, settings |
| **Core Engine** | Rust | Wine process management, bottle lifecycle, environment setup, configuration |
| **Sync Pipeline** | Rust | Proton commit monitoring, classification, patch adaptation, auto-apply |
| **Compatibility Runtime** | C / C++ | Wine fork, DXVK-macOS, MoltenVK/KosmicKrisp, D3DMetal, DXMT — the actual translation layer at runtime |

### 3.2 Why Rust Core with Swift Shell

The language split plays to each language's strengths. Rust handles the core engine because it excels at systems-level work: managing child processes, file I/O, concurrent pipeline execution, and FFI to C libraries (Wine, DXVK). It's also cross-platform, meaning the core engine could theoretically support other targets in the future.

Swift handles the UI because SwiftUI is the only way to build a macOS app that feels truly native. The boundary between them is a clean FFI layer: the Rust core exposes a C-compatible API, and Swift calls into it via `cbindgen`-generated headers. This is a well-established pattern (Signal Desktop, 1Password).

> **Prior art:** [SkuldNorniern's Whisky fork](https://github.com/SkuldNorniern/Whisky) already experiments with a Swift-Rust bridge, PE parsing in Rust, and runtime tag management. This validates the architecture direction.

---

## 4. Graphics Backend: The Full Picture

The macOS DirectX translation landscape is more complex and more capable than most people realize. As of April 2026, there are five viable paths. Cauldron supports all of them with automatic per-game selection.

### 4.1 Path A: D3DMetal (Apple GPTK)

```
D3D11/12 → D3DMetal → Metal
```

Apple's proprietary DirectX-to-Metal layer. Ships with GPTK 3.0 (December 2025). Skips Vulkan entirely. Best for D3D12 titles. Supports DXR on M3+ (`D3DM_SUPPORT_DXR=1`). GPTK 3.0 adds DLSS-to-MetalFX translation and MetalFX Frame Interpolation.

**Does NOT support DirectX 9.** DX9 games must use another path.

### 4.2 Path B: DXMT (Direct Metal)

```
D3D10/11 → DXMT → Metal
```

[DXMT](https://github.com/3Shain/dxmt) by 3Shain. ~1,000 commits from a solo developer. Metal-native D3D11/D3D10 implementation built specifically for Wine on macOS. Now integrated into CrossOver 26 as v0.72. Outperforms both DXVK+MoltenVK and D3DMetal on lower-spec Macs. Supports MetalFX Spatial Upscaling via `DXMT_METALFX_SPATIAL_SWAPCHAIN`.

**The single most important community contribution to macOS gaming in the last two years.**

### 4.3 Path C: DXVK + MoltenVK

```
D3D9/10/11 → DXVK → Vulkan → MoltenVK → Metal
```

The Proton-compatible path. Benefits directly from Proton's DXVK fixes. On macOS, stuck at [DXVK 1.10.3](https://github.com/Gcenx/DXVK-macOS) because upstream DXVK 2.0+ requires `VK_EXT_graphics_pipeline_library`, which MoltenVK does not support ([issue #1711](https://github.com/KhronosGroup/MoltenVK/issues/1711)).

Async shader compilation works via `DXVK_ASYNC=1` in Gcenx's fork. **This is the only path that handles DX9.**

### 4.4 Path D: DXVK + KosmicKrisp (Future)

```
D3D9/10/11 → DXVK → Vulkan → KosmicKrisp → Metal
```

[KosmicKrisp](https://docs.mesa3d.org/drivers/kosmickrisp.html) is a fully Vulkan 1.3-conformant driver on Metal 4, built by LunarG (Google-sponsored), upstreamed to Mesa. Achieved MoltenVK feature parity in Mesa 26.0 (February 2026). Requires macOS 26+, Apple Silicon only.

**This is the biggest missing piece for getting the full Proton stack on macOS.** If KosmicKrisp supports `VK_EXT_graphics_pipeline_library`, it unlocks DXVK 2.x on macOS for the first time. This would be a game-changer.

### 4.5 Path E: vkd3d-proton + Vulkan (DX12 via Vulkan)

```
D3D12 → vkd3d-proton → Vulkan → MoltenVK/KosmicKrisp → Metal
```

Experimental. DX12 requires ~1,000,000 shader resource views; Metal caps at ~500,000. Works on a per-game basis. Chip Davis (CodeWeavers) has patches. Long-term viability depends on KosmicKrisp.

### 4.6 Auto-Select Logic

The Rust core maintains a SQLite game database mapping Steam App IDs and executable hashes to optimal backends:

1. Check local DB for known-good backend.
2. Default: D3DMetal for DX12, DXMT for DX11, DXVK for DX9.
3. One-click override in UI.
4. Community reports feed back via optional telemetry.

### 4.7 MoltenVK: The Extension Gap

DXVK 2.x requires Vulkan extensions MoltenVK still lacks:

| Extension | Status | Impact |
|---|---|---|
| `VK_EXT_graphics_pipeline_library` | [Not supported](https://github.com/KhronosGroup/MoltenVK/issues/1711) | **Blocks DXVK 2.0+.** Major refactoring needed. |
| `VK_EXT_transform_feedback` | [Not supported](https://github.com/KhronosGroup/MoltenVK/issues/1588) | UE4 and most modern 3D apps |
| Geometry shaders | [Not supported](https://github.com/KhronosGroup/MoltenVK/issues/1524) | MSL has no geometry stage. PR #1815 in progress. |
| Pipeline statistics queries | Not supported | Performance metrics |
| Sparse textures | Limited | DX12 and some DX11 titles |

**MoltenVK performance note:** Since v1.2.11, `MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS` defaults enabled, causing up to 50% regression in DXVK games. **Workaround:** `MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS=0`.

KosmicKrisp may solve all of this with full Vulkan 1.3 conformance on Metal 4.

---

## 5. Synchronization: The Performance Battleground

Synchronization is where macOS can gain the most performance. Wine's default NT synchronization goes through wineserver round-trips, which is slow.

### 5.1 The Linux Solutions (for context)

Linux solved this progressively:
- **esync** — eventfd-based, user-space
- **fsync** — futex-based, user-space, Valve's Proton
- **ntsync** — kernel driver, Linux 6.14+ (March 2025) — 20-30% FPS gains

macOS has none of these primitives natively.

### 5.2 MSync (Current Best)

[MSync](https://github.com/marzent/wine-msync) by marzent. Uses Mach semaphore pools + dedicated Mach message pump in wineserver. The macOS equivalent of fsync/ntsync.

**Benchmarks (M2 Max):**

| Test | MSync | ESync | Improvement |
|------|-------|-------|-------------|
| Contended wait (10M iter) | 3.79s | 7.42s | ~49% faster |
| Zigzag test | 401,605 iter | 222,675 iter | ~80% more throughput |
| FFXIV CPU-bound | 219 FPS | 145 FPS | ~51% faster |

When `__ulock_wait2` (Apple's private futex-like API) is available, MSync achieves "better-than-NT performance" on single-wait cases.

Enable: `WINEMSYNC=1`

### 5.3 os_sync_wait_on_address (macOS 14.4+)

Apple's first **public** futex-like API. Atomic compare-and-wait with `OS_SYNC_WAIT_ON_ADDRESS_SHARED` for cross-process operation via shared memory. Documentation is incomplete but the API is real and usable. This could be the foundation for a next-generation macOS sync mechanism that doesn't rely on private APIs.

### 5.4 Cauldron's Sync Strategy

1. **Ship MSync** as the default (proven, fast).
2. **Experiment with `os_sync_wait_on_address`** for a public-API-only fast path.
3. **Long-term:** Investigate IOKit UserClient for kernel-side sync objects matching ntsync semantics.

---

## 6. Proton Auto-Sync Pipeline

This is the core differentiator. Four stages:

### 6.1 Stage 1: Monitor

A scheduled Rust task (`git2` crate) polls the Proton repository and its submodules (Wine, DXVK, vkd3d-proton) for new commits. Each commit is fetched, parsed, and stored in a local SQLite database with its diff, message, author, and affected files.

### 6.2 Stage 2: Classify

Each commit is tagged by subsystem using file-path heuristics and commit message patterns:

| Classification | Signal | macOS Transferability |
|---|---|---|
| Wine API fix | Changes in `dlls/`, `server/`, `loader/` | **High** — usually direct apply |
| DXVK fix | Changes in `dxvk/` submodule | **High** — applies to Path C directly |
| vkd3d-proton fix | Changes in `vkd3d-proton/` submodule | **Medium** — needs Vulkan compat check |
| Game-specific config | Changes to `proton` script, app ID lists | **High** — config-only, parse and import |
| Kernel/driver workaround | futex, fsync, `/proc` references | **Low** — needs macOS equivalent mapping |
| Steam integration | `lsteamclient/`, `vrclient/` | **Low** — Linux-specific IPC |
| Build system | `Makefile`, `configure.sh`, container | **None** — skip |

### 6.3 Stage 3: Adapt

- **High transferability:** Queued for automatic application to Cauldron's Wine and DXVK forks.
- **Medium:** Flagged for manual review with a suggested adaptation.
- **Low:** Logged; requires a developer to write a macOS-specific equivalent.

Kernel-level mapping table (Linux → macOS):

| Linux Mechanism | macOS Equivalent | Notes |
|---|---|---|
| `futex` / `FUTEX_WAIT_MULTIPLE` (fsync) | `os_unfair_lock` / `dispatch_semaphore_t` / MSync | MSync proven; `os_sync_wait_on_address` emerging |
| `eventfd` (esync) | `kqueue` `EVFILT_USER` / Mach ports | kqueue not inherited by fork(); Mach ports preferred |
| `/proc/self/maps` | `mach_vm_region_recurse()` | Memory layout inspection |
| `clone()` with custom flags | `pthread_create()` + Mach threads | Less granular but sufficient |
| `io_uring` | Grand Central Dispatch | Different paradigm; dispatch_io provides async I/O |
| `SIGSEGV` for SEH emulation | Mach exception ports | More reliable on macOS than signal handling |
| `ptrace()` | Mach task/thread ports | `thread_get_state`/`thread_set_state` for register access |
| `epoll` | `kqueue` | Mature and performant |
| `__ulock_wait`/`__ulock_wake` (private) | `os_sync_wait_on_address` (public, 14.4+) | Future migration path |

### 6.4 Stage 4: Validate

Adapted patches are applied to a CI build and run through automated smoke tests: launch game, check for crashes within 60 seconds, verify renderer initialization. Passing patches merge into `cauldron-nightly`. Failing patches are quarantined for manual review.

### 6.5 Game Compatibility Database

Proton's game-specific fixes live in a Python script (`default_compat_config()`) mapping Steam App IDs to compatibility flags (`noopwr`, `gamedrive`, `heapdelayfree`, etc.). Cauldron parses this function automatically and imports these mappings, translating Linux-specific flags to macOS equivalents where applicable.

Database schema tracks: game identity (App ID, executable hash, title), optimal graphics backend, required compatibility flags, Wine configuration overrides, known issues, and community-reported status. Ships with the app and receives OTA updates.

---

## 7. Proton-GE Patches: What We're Syncing

GE-Proton carries ~530 custom patches on top of Valve's bleeding-edge Wine. Here's what matters for macOS:

### 7.1 High-Value Portable Patches

| Patch | What It Does | Why It Matters |
|---|---|---|
| **De-steamification** | Removes hardcoded `steamuser`, Steam-specific loader | Essential for standalone use |
| **FSR injection** | AMD FidelityFX via Vulkan compute shaders (SPIR-V) | Adaptable to Metal compute via MoltenVK |
| **`WINE_BLOCK_HOSTS`** | DNS-level hostname blocking in ws2_32 | Anti-cheat workaround, pure Win32 API |
| **Dynamic .exe relocation** | Forces relocation of .exe files | Helps modding (FFXIV plugins), portable |
| **`LARGE_ADDRESS_AWARE` override** | 32-bit games get full address space | Many 32-bit games need this |
| **`ntdll-Hide_Wine_Exports`** | Hides Wine exports from detection | Anti-detection, portable |
| **Unity crash hotfix** | DXGI debug interface fix | Portable, fixes many Unity games |
| **D2D1 crash fix** | ID2D1DeviceContext null target | Portable bug fix |
| **NCryptDecrypt** | Crypto implementation | Fixes PSN login (Ghost of Tsushima) |
| **`WINE_DISABLE_SFN`** | Disable short filenames | Fixes Yakuza 5 cutscenes |

### 7.2 Game-Specific Protonfixes (354+ Scripts)

The [umu-protonfixes](https://github.com/Open-Wine-Components/umu-protonfixes) system provides Python scripts for 354 Steam games plus GOG, Epic, Ubisoft, Amazon, Battle.net, itch.io, Humble. These are largely platform-agnostic:

- **DLL overrides:** `protontricks('vcrun2019')`, `protontricks('dotnet48')`
- **Launch arg injection:** `append_argument('-fullscreen -vulkan')`
- **Command replacement:** `replace_command('SkyrimSELauncher.exe', 'skse64_loader.exe')`
- **File creation:** Elden Ring dummy DLC files workaround
- **Automatic upscaler management:** Downloads and installs DLSS/XeSS/FSR DLLs with hash verification

### 7.3 DLSS/Upscaler Translation for macOS

Proton-GE has a `WINE_LOADDLL_REPLACE` system for force-loading upscaler DLLs. On macOS, the equivalent is:

- **DLSS → MetalFX:** Intercept nvngx_dlss.dll load, implement DLSS evaluation API against `MTLFXSpatialScaler`/`MTLFXTemporalScaler`. GPTK 3.0 already does this.
- **FSR 1.0 spatial:** Compute shaders, works on Metal.
- **FSR 3/4 frame gen:** Theoretically possible via Metal compute, untested independently.

### 7.4 Wine-Staging Patches Not in Proton (macOS-Critical)

| Patchset | Relevance |
|---|---|
| `winemac.drv-no-flicker-patch` | **Directly targets macOS** — reduces window flicker |
| `ntdll-WRITECOPY` | Proper WRITECOPY page protection |
| `gdiplus-Performance-Improvements` | GDI+ rendering (game menus/UI) |
| `user32-Mouse_Message_Hwnd` | Mouse message targeting |
| `mfplat-streaming-support` | Media Foundation video playback |
| `xactengine3_7-PrepareWave` | XACT audio (many XNA/DirectX games) |
| `dinput-scancode` | DirectInput keyboard/controller |

---

## 8. The Rosetta 2 Problem

This is an existential risk to the project.

**Timeline:**
- macOS 26 Tahoe: Full Rosetta 2. Deprecation warnings in 26.4 (Feb 2026).
- macOS 27: Rosetta 2 still available on Apple Silicon. Intel Macs dropped.
- macOS 28 (Fall 2027): Rosetta 2 removed. Possible limited subset for "older unmaintained gaming titles."

**Impact:** Wine runs as x86-64 under Rosetta 2 on Apple Silicon. The entire translation stack is: Rosetta 2 (x86→ARM64) + Wine (Win32→POSIX) + D3DMetal/DXVK (DX→Metal). Without Rosetta, x86 Windows games cannot run.

**Potential solutions being tracked:**

| Solution | Status | Notes |
|---|---|---|
| [Jpkovas/FEX_MacOs](https://github.com/Jpkovas/FEX_MacOs) | Very early (0 stars, 7,368+ inherited commits) | FEX-Emu ported to macOS |
| Wine ARM64EC support (Wine 10.0+) | In progress | For ARM Windows binaries, not x86 |
| Apple's retained Rosetta subset | Unknown scope | May cover "older unmaintained gaming titles" |
| Box64 | Primarily Linux | Has 16K page size support |

**Strategy:** Track and contribute to solutions. This is a multi-year problem — Rosetta removal is Fall 2027 at earliest — but Cauldron must have a migration path before then.

---

## 9. macOS Kernel Deep Dive

Understanding macOS kernel constraints is essential for Wine compatibility work.

### 9.1 Memory Model

- **16K pages on Apple Silicon.** Windows expects 4K. Wine 11 simulates different page sizes. This is non-trivial.
- **Low address space:** macOS maps first 4GB as `__PAGEZERO`. Wine's `preloader_mac.c` reserves ~8GB at `0x1000` with `MAP_FIXED` + `PROT_NONE`.
- **ARM64 ASLR is mandatory.** Signed pointers encode expected page zero size. No custom `pagezero_size`.
- **Shared memory max:** Only 4MB by default (vs. effectively infinite on Linux).

### 9.2 Code Signing / JIT / SIP

- **W^X on Apple Silicon:** Pages can't be simultaneously writable and executable. Must use `MAP_JIT` + `pthread_jit_write_protect_np()`.
- **Required entitlements:** `com.apple.security.cs.allow-jit`, `com.apple.security.cs.disable-library-validation`, `com.apple.security.cs.allow-dyld-environment-variables`.
- **Gatekeeper:** Install with `--no-quarantine` or `xattr -rd com.apple.quarantine`.
- **SIP:** Strips `DYLD_*` vars from protected binaries. Wine binaries are not SIP-protected, so this is manageable.

### 9.3 Exception Handling

Wine uses Mach exception ports (`task_set_exception_ports`) for SEH emulation rather than POSIX signals. Flow: hardware exception → `exception_triage()` → thread port → task port → host port → BSD signal fallback. More reliable than signal handling on macOS.

### 9.4 Threading

Wine uses pthreads for creation, drops to Mach level for: debug registers (`thread_get_state`/`thread_set_state`), suspension (`task_suspend`/`task_resume`), TLS setup (private `_thread_set_tsd_base()` for GSBASE).

### 9.5 Audio

`winecoreaudio.drv` maps WASAPI/mmdevapi to CoreAudio via AUHAL. Works but latency management is complex. Proton-GE's `winepulse-fast-polling.patch` concept (tighter polling for lower latency) should be adapted for CoreAudio.

---

## 10. Ecosystem: Projects We Build On

### 10.1 Critical Dependencies

| Project | Role | Status |
|---|---|---|
| [Gcenx/macOS_Wine_builds](https://github.com/Gcenx/macOS_Wine_builds) | Official WineHQ macOS packages | Active. Gcenx is the linchpin of free Wine-on-macOS. |
| [3Shain/dxmt](https://github.com/3Shain/dxmt) | Metal-native D3D11 | Active. v0.74. In CrossOver 26. |
| [marzent/wine-msync](https://github.com/marzent/wine-msync) | macOS sync primitives | Mature. In Whisky and CrossOver. |
| [Gcenx/DXVK-macOS](https://github.com/Gcenx/DXVK-macOS) | DXVK 1.10.3 for macOS | Maintenance. Ceiling until KosmicKrisp. |
| [KhronosGroup/MoltenVK](https://github.com/KhronosGroup/MoltenVK) | Vulkan on Metal | Active. v1.4. Missing key extensions. |
| KosmicKrisp (Mesa) | Vulkan 1.3 on Metal 4 | Alpha. Game-changer potential. |
| [italomandara/CXPatcher](https://github.com/italomandara/CXPatcher) | CrossOver component upgrader | Active. Bridges release gaps. |

### 10.2 Whisky Forks Worth Watching

| Fork | Why | Status |
|---|---|---|
| [frankea/Whisky](https://github.com/frankea/Whisky) | Most professional successor. Wine 11.0, 67 commits ahead, 83% test coverage, launcher compat system, CI/CD. | Active (Jan 2026) |
| [cyyever/Whisky](https://github.com/cyyever/Whisky) | Most bleeding-edge. Wine 11.5, DXMT submodule, **fixed wineboot hang on macOS 26 Tahoe**. | Active (Mar 2026) |
| [SkuldNorniern/Whisky](https://github.com/SkuldNorniern/Whisky) | Rust integration experiment. Swift-Rust bridge, PE parsing in Rust. Validates our architecture. | Active (Feb 2026) |
| [Zinedinarnaut/Whisky](https://github.com/Zinedinarnaut/Whisky) | "Vector" — heavy Steam optimization, macOS 26 fixes. | Active (Feb 2026) |
| [ThatOneTequilaDev/Tequila](https://github.com/ThatOneTequilaDev/Tequila) | Wine 11 integration, builds on Bourbon's DXMT work. | Active (Jan 2026) |

### 10.3 Diamond-in-the-Rough Projects

| Project | What | Why It Matters |
|---|---|---|
| [Gcenx/macports-wine](https://github.com/Gcenx/macports-wine) | 997 commits, 116 stars | One person maintaining the entire free Wine-on-macOS build infra |
| [Jpkovas/FEX_MacOs](https://github.com/Jpkovas/FEX_MacOs) | 0 stars, 7,368+ commits | FEX-Emu x86 emulator ported to macOS. If viable, solves Rosetta deprecation. |
| [MythicApp/Mythic](https://github.com/MythicApp/Mythic) | 1,229 stars, 1,619 commits | Full macOS game launcher with GPTK integration |
| [neo773/macgamingdb](https://github.com/neo773/macgamingdb) | 91 stars | Community game compat DB at macgamingdb.app |
| [philipturner/metal-benchmarks](https://github.com/philipturner/metal-benchmarks) | 592 stars, 418 commits | Apple GPU microarchitecture documentation. Foundational. |
| [kiku-jw/peak-crossover-mouse-fix](https://github.com/kiku-jw/peak-crossover-mouse-fix) | 11 stars | Fixes Unity pointer bug blocking many games. Tiny but critical. |
| [EnderIce2/rpc-bridge](https://github.com/EnderIce2/rpc-bridge) | 200 stars | Discord Rich Presence for Wine games |

### 10.4 CrossOver 26: Current State of the Art (February 2026)

CrossOver 26 represents the current benchmark:

- **Anti-cheat:** nProtect GameGuard, EAC, and BattlEye now work for 20+ AAA titles. CodeWeavers calls this "curing artificial incompatibility."
- **Components:** Wine 11.0, D3DMetal 3.0, DXMT v0.72, vkd3d 1.18, NTSync (Linux)
- **DLSS → MetalFX:** Intercepts NVIDIA DLSS/DLSS-FG calls and translates to MetalFX upscaling + frame interpolation
- **Tested titles:** Helldivers 2, Kingdom Come: Deliverance II, God of War Ragnarok, Starfield, Age of Empires IV

**What CrossOver has that upstream Wine doesn't:**

1. **wine32on64** — Custom LLVM compiler for 32-bit on 64-bit macOS (requires forked Clang-8 with `cdecl32`/`stdcall32`/`thiscall32`/`fastcall32` attributes)
2. **Patched MoltenVK** — Fakes unsupported Vulkan extensions
3. **Custom DXVK** — macOS-specific modifications
4. **D3DMetal integration** — Apple's proprietary layer
5. **DXMT integration** — 3Shain's Metal D3D11
6. **MSync** — Mach semaphore synchronization
7. **Anti-cheat patches** — Proprietary, not open-source
8. **DLSS via MetalFX** — Maps NVIDIA calls to Apple's upscaler

---

## 11. Technology Stack

| Component | Technology | Rationale |
|---|---|---|
| Core Engine | Rust (with tokio for async) | Systems-level performance, safety, excellent C FFI |
| UI Shell | Swift 6.2+ / SwiftUI | Native macOS look and feel, macOS 26+ |
| Rust-Swift Bridge | `cbindgen` + C FFI (`@_silgen_name`) | 40+ exported C functions, JSON serialization across boundary |
| Game Database | SQLite via `rusqlite` (WAL mode) | Embedded, no external dependencies, fast |
| Sync Pipeline | Rust (`git2` crate) | Git polling, commit classification, patch auto-adaptation |
| Wine Runtime | Custom Wine fork (C) | Wine 11.6 with 131 patches from 9 sources |
| D3D Translation (Path A) | D3DMetal (GPTK) | Apple's native D3D-to-Metal for DX11/12 |
| D3D Translation (Path B) | DXMT | Metal-native D3D10/11, best DX11 perf on Mac |
| D3D Translation (Path C) | DXVK-macOS + MoltenVK | Gcenx's macOS DXVK fork, D3D9/10/11 via Vulkan-to-Metal |
| D3D Translation (Path D) | DXVK + KosmicKrisp (future) | Full DXVK 2.x via Vulkan 1.3 on Metal 4 |
| D3D Translation (Path E) | vkd3d-proton + Vulkan (experimental) | DX12 via Vulkan, per-game viability |
| Graphics Detection | PE import table analysis | DX8-12, Vulkan, OpenGL auto-detection from exe imports |
| Game Binary Patching | Hex pattern matching + backup/restore | Reversible GPU check/driver version fixes |
| x87 Acceleration | RosettaX87 (WineAndAqua) | 4-10x faster x87 FP via patched Rosetta |
| Synchronization | MSync (default), os_sync_wait_on_address (experimental) | Fast Wine sync on macOS |
| Configuration | Three-tier profile system (Stable/Preview/Bleeding Edge) | Global presets + per-game overrides |
| Build System | Cargo + Swift Package Manager + Make | Cargo for Rust, SPM for Swift, Make for Wine |
| CI/CD | GitHub Actions (macOS runners) | Build checks, unit tests (256+), E2E tests |
| Package Distribution | DMG + Homebrew Cask + Sparkle | Standard macOS distribution channels |

---

## 12. Project Plan & Status

### Phase 0: Foundation — COMPLETE

- [x] Rust workspace: `cauldron-core`, `cauldron-sync`, `cauldron-db`, `cauldron-bridge`, `cauldron-cli`
- [x] Rust-to-Swift FFI bridge (40+ exported C functions)
- [x] CI pipeline (GitHub Actions: cargo test, cargo check, swift build, E2E)
- [x] Wine 11.6 fork with 131 patches from 9 sources
- [x] 256+ unit tests, 2 integration test suites

### Phase 1: Core Engine — COMPLETE

- [x] Bottle management (create, list, delete, import, discover from Whisky/CrossOver)
- [x] Wine downloading (Gcenx releases, local builds)
- [x] Wine process launching with full environment setup
- [x] Five graphics backends with auto-select (D3DMetal, DXMT, DXVK+MoltenVK, DXVK+KosmicKrisp, vkd3d-proton)
- [x] D3DMetal auto-detection from CrossOver/GPTK with one-click import
- [x] MSync integration (Mach semaphore sync)
- [x] Dependency auto-install via winetricks (vcrun, dotnet, d3dx9, etc.)
- [x] Steam installer (download + silent setup in bottle)
- [x] Activation system (14-day trial, Ed25519 JWT, Keychain storage)

### Phase 2: Intelligent Features — COMPLETE

- [x] Graphics API auto-detection via PE import table (DX8-12, Vulkan, OpenGL)
- [x] RosettaX87 integration (detection, settings toggle, env var injection)
- [x] Game binary patching system (reversible GPU check/driver version fixes)
- [x] Per-game profile seeding (20 pre-configured games with known-good backends)
- [x] Three optimization profiles: Stable, Preview, Bleeding Edge
- [x] Per-game settings overrides with profile mismatch warnings
- [x] Advanced settings tab for power users
- [x] Game library with Play buttons, API badges, per-game launch
- [x] Proton auto-sync pipeline (monitor → classify → adapt → apply → database)
- [x] Patch triage UI (apply, skip, reverse, inspect diffs)

### Phase 3: Polish — IN PROGRESS

- [ ] DXMT Wine patches (winemetal.drv)
- [ ] umu-protonfixes full integration (354 game scripts)
- [ ] umu-database cross-store title lookup
- [ ] Performance overlay / frame timing capture
- [ ] Community compatibility reporting
- [ ] Notarized DMG for Gatekeeper
- [ ] Stripe backend (Cloudflare Worker)

### Phase 4+: Future

- [ ] Extension system (custom patches, community registry)
- [ ] Anti-cheat patches (EAC/BattlEye) if available
- [ ] KosmicKrisp integration (Vulkan 1.3 on Metal 4)
- [ ] os_sync_wait_on_address (macOS 14.4+ public futex)
- [ ] ARM64EC native execution

---

## 13. Wine & Graphics Translation Technical Reference

Hard-won knowledge from integration testing. Read this before modifying Wine launch code, DLL override logic, or graphics backend switching.

### 13.1 Wine DLL Loading Architecture

Wine has three DLL types with strict semantics:

| Type | Where Wine looks | How to trigger | Example |
|---|---|---|---|
| **builtin** | `lib/wine/x86_64-windows/` then `drive_c/windows/system32/` | Default, or `=b` override | Wine's own d3d11.dll |
| **native** | Application directory, then Windows PATH dirs | `=n` override | A game's bundled DLL |
| **native,builtin** | Try native first, fall back to builtin | `=n,b` override | DXMT/DXVK DLLs in game dir |

Key behaviors:
- **`WINEDLLPATH`** only affects builtin search. If a DLL in WINEDLLPATH is not a Wine builtin (missing Wine PE signature), it is **silently ignored** with the message `"found in WINEDLLPATH but not a builtin, ignoring"`.
- **`WINEDLLOVERRIDES`** is read from the **Unix process environment** via `getenv()` in `ntdll/unix/loadorder.c`. It is NOT read from the Wine registry. The `Session Manager\Environment` registry key sets Windows env vars visible to `cmd /c echo %VAR%` but does NOT affect Wine's DLL loader.
- **AppDefaults registry overrides** (`HKCU\Software\Wine\AppDefaults\<exe>\DllOverrides`) DO take precedence over `WINEDLLOVERRIDES` env var. This is how we protect `steamwebhelper.exe` from DXMT overrides.
- Wine marks its own PE DLLs with a **"Wine builtin" signature** in the PE header. DLLs with this marker are treated as builtins regardless of where they're found.

### 13.2 DXMT Installation

DXMT (Direct3D 11 to Metal) consists of five files across two directories:

```
lib/wine/x86_64-windows/     (PE DLLs — Windows side)
  d3d11.dll                   DXMT's D3D11 implementation (5.2MB, has Wine builtin marker)
  d3d10core.dll               DXMT's D3D10 implementation
  dxgi.dll                    DXMT's DXGI implementation
  winemetal.dll               Windows-side Metal bridge stub (73KB)

lib/wine/x86_64-unix/        (Unix shared objects — native macOS side)
  winemetal.so                Actual Metal rendering engine (33MB)
```

DXMT's d3d11.dll imports `winemetal.dll`, which uses Wine's `__wine_unix_call()` to dispatch into `winemetal.so` (the actual Metal GPU code). Without `winemetal.dll` + `winemetal.so`, DXMT's d3d11.dll loads but falls back to `wined3d.dll` (Wine's OpenGL renderer), causing `GL_INVALID_FRAMEBUFFER_OPERATION` errors and crashes.

**Installation approaches (ranked by reliability):**

1. **Replace Wine builtins** — Copy DXMT DLLs into `lib/wine/x86_64-windows/`, replacing Wine's originals (back them up as `.wine-orig`). This makes DXMT the default for ALL processes. Most reliable.
2. **Game directory + override** — Copy DLLs to the game directory and set `WINEDLLOVERRIDES=d3d11=n,b`. Works when the env var propagates. Fails for Steam-launched games (see 13.3).
3. **System32 replacement** — Copy to `drive_c/windows/system32/`. Wine treats these as builtins, but its own `lib/wine/` copies take priority, so this doesn't work.
4. **WINEDLLPATH** — Does NOT work for DXMT. Wine rejects non-builtin DLLs from this path.

### 13.3 Environment Variable Propagation Through Steam

This is the hardest problem in the Wine/DXMT integration.

**The problem:** When you launch `wine steam.exe -applaunch <id>`, Steam creates a child process for the game. Wine's child process creation uses `fork()` + `exec_wineloader()` (in `ntdll/unix/process.c`). The `exec_wineloader` function calls `execv()` on the wineloader binary at `lib/wine/x86_64-unix/wine`.

**What propagates:**
- Unix env vars set before the initial `wine` call propagate to the wineserver and first process
- The wineserver inherits env from its creator
- `fork()` inherits parent env, so children of wine processes get the env

**What does NOT propagate:**
- Env vars from a second `wine` CLI call (e.g., `wine steam.exe -applaunch 377160`) do NOT affect existing wineserver children
- Shell script wrappers around the wine binary are NOT called by `exec_wineloader` — Wine resolves the wineloader path from `ntdll.so`'s directory and execs the binary directly
- `DYLD_INSERT_LIBRARIES` propagates via fork+exec but may be stripped by SIP for certain binaries

**Working solutions:**
- Set `WINEDLLOVERRIDES` in the Windows system registry (`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`). These are visible to `cmd /c echo %VAR%` but do NOT affect `loadorder.c`'s `getenv()`.
- Install DXMT as Wine builtin replacement (approach 1 above) — no overrides needed
- Patch Wine's `loadorder.c` to prefer native DLLs from the application directory for known graphics DLLs (d3d11, dxgi, d3d10core, d3d12, d3d9)

### 13.4 Steam CEF (steamwebhelper.exe) Compatibility

Steam's UI is Chromium Embedded Framework (CEF). It crashes on upstream Wine without fixes:

| Fix | Mechanism | Location |
|---|---|---|
| `--no-sandbox` | Disables CEF sandbox that hooks ntdll syscall thunks | Passed as `-no-cef-sandbox` flag to steam.exe |
| `--in-process-gpu --disable-gpu` | Disables GPU rendering in CEF | Injected by `hack_append_command_line` patch in `kernelbase/process.c` |
| `ShowCrashDialog=0` | Suppresses Wine crash dialog for non-fatal CEF subprocess crashes | Registry: `HKCU\Software\Wine\WineDbg` |
| `steamwebhelper.exe` → `builtin` d3d11 | Prevents DXMT native d3d11 from crashing CEF's rendering | Registry: `AppDefaults\steamwebhelper.exe\DllOverrides` |

### 13.5 Wine Process Lifecycle on macOS

**Zombie prevention:**
- `WINEBOOT=""` skips `wineboot --init` for existing bottles. Without this, rapid launch/kill cycles create `wineboot.exe` processes stuck in uninterruptible sleep (macOS kernel state `UEs`), which cannot be killed by any signal.
- `WINEDEBUGGER=""` disables Wine's auto crash debugger (`winedbg --auto`), which otherwise spawns for every crashed subprocess and accumulates as dock icon zombies.
- `winemenubuilder.exe=d` in `WINEDLLOVERRIDES` disables Wine's dock/menu integration.

**Clean shutdown:**
- Always use `wineserver -k` followed by `wineserver -w` (wait). This cleanly deregisters all windows from the macOS window server.
- Never use `pkill -9 -f wine` — this leaves orphaned dock icons and stale wineserver sockets.
- The Cauldron stop button iterates all bottle prefixes and calls `WINEPREFIX=<path> wineserver -k` for each.

**WINEPREFIX and symlinks:**
- `WINEPREFIX` MUST point to the resolved (real) path, not a symlink. Wine resolves the wineserver socket path from WINEPREFIX — a symlinked path creates a different socket name, causing Wine to exit immediately.
- Use `std::fs::canonicalize()` in Rust or `os.path.realpath()` in Python before setting WINEPREFIX.

### 13.6 D3DMetal vs DXMT vs DXVK

| Feature | D3DMetal (Apple) | DXMT (3Shain) | DXVK (via MoltenVK) |
|---|---|---|---|
| DX11 | Yes | Yes | Yes |
| DX12 | Yes | No | Via vkd3d-proton |
| DX9 | No | No | Yes |
| Translation path | D3D → Metal (direct) | D3D → Metal (direct) | D3D → Vulkan → Metal |
| Performance | Best (10-30% over DXVK) | Close to D3DMetal (within 5-15%) | Slowest (double translation) |
| Wine patches needed | Zero (external DLL/dylib) | Zero (Wine builtin replacement) | Zero (DLL override) |
| Open source | No (Apple proprietary) | Yes (MIT) | Yes (zlib) |
| Requires CrossOver | Effectively yes (hooks into winemac.drv) | No | No |

**D3DMetal requires zero Wine source patches.** It operates entirely at the DLL override / dylib loading level. The `.so` bridge files in `x86_64-unix/` replace Wine's builtin wined3d path by `dlopen("D3DMetal.framework/D3DMetal")` at runtime.

---

## 14. Current Codebase Metrics

| Metric | Count |
|---|---|
| Rust crates | 5 (core, db, sync, bridge, cli) |
| Swift modules | 1 (CauldronApp with 30+ views/models) |
| Rust tests | 256+ (unit + integration) |
| FFI functions | 40+ (C boundary between Rust and Swift) |
| SQLite tables | 6 (games, proton_commits, backend_overrides, compatibility_reports, patch_log, sync_status) |
| Wine patches | 131 from 9 sources |
| Game profiles | 20 pre-seeded |
| Binary patch definitions | 4 games (Elden Ring, Cyberpunk 2077, Hogwarts Legacy, Baldur's Gate 3) |

---

## 14. Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| MoltenVK missing Vulkan extensions | High | High | KosmicKrisp as alternative; DXMT for DX11; D3DMetal for DX12; contribute upstream |
| **Rosetta 2 deprecation (macOS 28)** | **Critical** | **High** | Track FEX_MacOs, Wine ARM64EC, Apple's retained subset; contribute to solutions |
| Apple breaks GPTK in macOS update | High | Medium | Pin known-good GPTK version; test on macOS betas early |
| Proton Wine fork diverges from macOS-buildable | Medium | Medium | Maintain rebase branch; don't track Proton HEAD blindly |
| Anti-cheat (EAC, BattlEye) blocks macOS | High | High | Out of scope for v1; document as known limitation; CrossOver 26 made progress here |
| Legal concerns (GPTK licensing, Wine LGPL) | High | Low | GPTK is redistributable per Apple; Wine LGPL allows linking; audit early |
| 16K page size issues | Medium | Medium | Wine 11 simulates 4K pages; test thoroughly on Apple Silicon |
| Code signing / JIT restrictions | Medium | Medium | Proper entitlements; `MAP_JIT` + `pthread_jit_write_protect_np`; document SIP interactions |
| Community adoption too slow | Medium | Medium | Seed with ProtonDB data; manual testing sprint at launch |
| Wine fork maintenance burden | High | Medium | Minimize delta from upstream; automate rebasing; focus patches on macOS-specific issues |

---

## 15. Repository Structure

```
cauldron/
├── cauldron-core/          # Rust: bottles, wine, graphics, game_scanner, game_patches,
│                           #       rosettax87, runtime_downloader, shader_cache, registry
├── cauldron-sync/          # Rust: monitor, classifier, adapter, applicator, pipeline,
│                           #       auto_adapter, config_importer, protonfixes, patch_analysis
├── cauldron-db/            # Rust: SQLite schema, queries, models (6 tables)
├── cauldron-bridge/        # Rust: C FFI layer (40+ functions), JSON serialization
├── cauldron-cli/           # Rust: CLI interface for testing/debugging
├── CauldronApp/            # Swift/SwiftUI macOS app
│   ├── Bridge/             #   CauldronFFI.swift, CauldronBridge.swift
│   ├── Models/             #   Bottle, GameRecord, GraphicsBackend, ConfigProfile,
│   │                       #   PerGameSettings, AppSettings, WineVersion, SyncStatus
│   ├── Views/              #   ContentView, BottleDetailView, GameLibraryView,
│   │                       #   SettingsView (Profile/General/Advanced/Sync/Performance/About),
│   │                       #   SyncStatusView, PatchInspectSheet, SteamInstallWizard,
│   │                       #   DiscoveredBottlesView, CreateBottleView, DependencyPickerSheet
│   ├── ViewModels/         #   BottleListViewModel
│   └── Licensing/          #   LicenseManager, JWTValidator, KeychainHelper, ActivationView
├── patches/cauldron/       # Wine patch series (131 patches)
├── scripts/                # Build scripts (init_wine_fork.sh, build_wine.sh, etc.)
├── deps/                   # Submodules (wine-msync, cxpatcher, umu-protonfixes)
├── dxvk/, dxmt/, moltenvk/ # Graphics runtime submodules
├── db/                     # SQLite schema, migrations, seed data
├── Cargo.toml              # Workspace root
├── Makefile                # Build targets (build, test, swift-build, wine-init, etc.)
└── README.md
```

---

## 16. Key Environment Variables Reference

### Wine/Sync

| Variable | Effect |
|---|---|
| `WINEMSYNC=1` | Enable MSync (Mach semaphore sync) |
| `WINEESYNC=1` | Enable ESync (fallback) |
| `WINEMSYNC_QLIMIT=50` | Server Mach port queue size |

### Graphics

| Variable | Effect |
|---|---|
| `DXVK_ASYNC=1` | Async shader compilation (DXVK 1.10.3) |
| `MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS=0` | Fix MoltenVK perf regression |
| `MTL_HUD_ENABLED=1` | Metal Performance HUD |
| `D3DM_SUPPORT_DXR=1` | Enable DXR in D3DMetal (M3+) |
| `DXMT_METALFX_SPATIAL_SWAPCHAIN=1` | MetalFX upscaling in DXMT |
| `ROSETTA_ADVERTISE_AVX=1` | Enable AVX/AVX2 in Rosetta (Sequoia+) |
| `ROSETTA_X87_PATH=/path` | RosettaX87 patched FP handler (4-10x x87 perf) |

### Game Fixes

| Variable | Effect |
|---|---|
| `WINE_FULLSCREEN_FSR=1` | Enable FSR upscaling |
| `WINE_FULLSCREEN_FSR_STRENGTH=2` | FSR sharpening (0-5) |
| `WINE_BLOCK_HOSTS=hostname` | DNS-level host blocking |
| `WINE_LARGE_ADDRESS_AWARE=1` | Override for 32-bit games |
| `WINE_DISABLE_SFN=1` | Disable short filenames |
| `WINE_NO_WM_DECORATION=1` | Disable window decorations |

---

## 17. Open Questions

- **Shader cache infrastructure:** Hosting pre-compiled shader caches requires a server. Self-hosted, or CDN? (Recommendation: Cloudflare R2 for cost efficiency.)
- **Telemetry:** Opt-in game compatibility reporting is essential, but privacy-sensitive. Define exactly what data is collected and publish the schema.
- **KosmicKrisp timeline:** When does it become stable enough to ship as a MoltenVK replacement? Monitor Mesa releases. MoltenVK feature parity reached in Mesa 26.0, but `VK_EXT_graphics_pipeline_library` status for DXVK 2.x unknown.
- **os_sync_wait_on_address:** Is the API complete enough to build a full sync primitive on? Needs experimentation.
- **Protonfixes runtime:** Ship Python runtime for protonfixes scripts, or rewrite critical ones in Rust? (Recommendation: embed Python initially, migrate high-value scripts over time.)
- **Extension system scope:** How much flexibility vs. security? Current design is declarative TOML-only (no script execution).

### Resolved Questions

- **Naming:** "Cauldron" confirmed as final name.
- **Steam dependency:** Supports both Steam and standalone EXE launching. Game library scans Steam ACF manifests and arbitrary executables.
- **Apple Silicon only:** ARM64 only for v1. macOS 26+ required.
- **Homebrew distribution:** Ship DMG + Homebrew Cask + Sparkle (auto-update for official builds only).
- **Graphics API detection:** Implemented via PE import table analysis — detects DX8-12, Vulkan, OpenGL automatically.
- **Profile system:** Three-tier (Stable/Preview/Bleeding Edge) with per-game overrides. Resolved the "how do users configure this" question.

---

*Cauldron is not affiliated with Valve, Apple, CodeWeavers, or any of the projects listed above.*
