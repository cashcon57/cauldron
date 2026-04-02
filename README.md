# Cauldron

**Bleeding-edge macOS game compatibility layer -- Proton-synced, Rust-powered, Metal-native.**

Cauldron is an open-source macOS application that provides bleeding-edge Windows game compatibility, inspired by Proton-GE on Linux. It forks the now-archived [Whisky](https://github.com/Whisky-App/Whisky) app as a starting point, replaces its core with a Rust engine, and builds an automated pipeline that continuously syncs fixes from Valve's Proton into a macOS-compatible format.

> **Status:** Pre-alpha. Architecture and research phase.

---

## Why Cauldron Exists

macOS gaming compatibility suffers from three structural problems:

1. **Staleness.** CrossOver updates lag weeks to months behind Proton. Whisky is archived. By the time a fix lands on macOS, Linux users have had it for a long time.
2. **Translation gap.** Proton's fixes assume Linux with native Vulkan, futex-based synchronization, and specific driver behaviors. No one systematically maps these to macOS equivalents.
3. **No community-driven bleeding edge.** Linux has Proton-GE, which cherry-picks community patches, experimental fixes, and game-specific hacks faster than official Proton. macOS has nothing equivalent.

Cauldron fills that gap.

---

## Architecture

Four layers:

| Layer | Language | Responsibility |
|---|---|---|
| **UI Shell** | Swift / SwiftUI | Native macOS interface, bottle management, game library |
| **Core Engine** | Rust | Wine process management, bottle lifecycle, environment setup |
| **Sync Pipeline** | Rust + Python | Proton commit monitoring, classification, patch adaptation |
| **Compatibility Runtime** | C / C++ | Wine fork, DXVK, MoltenVK/KosmicKrisp, D3DMetal, DXMT |

### Why Rust Core + Swift Shell

Rust handles systems-level work: child processes, file I/O, concurrent pipelines, FFI to C libraries. Swift handles the UI because SwiftUI is the only way to build a truly native macOS app. The boundary is a clean C FFI via `cbindgen`. This is the same pattern used by Signal Desktop and 1Password.

> **Prior art:** [SkuldNorniern's Whisky fork](https://github.com/SkuldNorniern/Whisky) already experiments with a Swift-Rust bridge, PE parsing in Rust, and runtime tag management. This validates the architecture direction.

---

## Graphics Backend: The Full Picture

The macOS DirectX translation landscape is more complex and more capable than most people realize. As of April 2026:

### Path A: D3DMetal (Apple GPTK)

```
D3D11/12 --> D3DMetal --> Metal
```

Apple's proprietary DirectX-to-Metal layer. Ships with GPTK 3.0 (December 2025). Skips Vulkan entirely. Best for D3D12 titles. Supports DXR on M3+ (`D3DM_SUPPORT_DXR=1`). GPTK 3.0 adds DLSS-to-MetalFX translation and MetalFX Frame Interpolation.

**Does NOT support DirectX 9.** DX9 games must use another path.

### Path B: DXMT (Direct Metal)

```
D3D10/11 --> DXMT --> Metal
```

[DXMT](https://github.com/3Shain/dxmt) by 3Shain. ~1,000 commits from a solo developer. Metal-native D3D11/D3D10 implementation built specifically for Wine on macOS. Now integrated into CrossOver 26 as v0.72. Outperforms both DXVK+MoltenVK and D3DMetal on lower-spec Macs. Supports MetalFX Spatial Upscaling via `DXMT_METALFX_SPATIAL_SWAPCHAIN`.

**The single most important community contribution to macOS gaming in the last two years.**

### Path C: DXVK + MoltenVK

```
D3D9/10/11 --> DXVK --> Vulkan --> MoltenVK --> Metal
```

The Proton-compatible path. Benefits directly from Proton's DXVK fixes. On macOS, stuck at [DXVK 1.10.3](https://github.com/Gcenx/DXVK-macOS) because upstream DXVK 2.0+ requires `VK_EXT_graphics_pipeline_library`, which MoltenVK does not support ([issue #1711](https://github.com/KhronosGroup/MoltenVK/issues/1711)).

Async shader compilation works via `DXVK_ASYNC=1` in Gcenx's fork. **This is the only path that handles DX9.**

### Path D: DXVK + KosmicKrisp (Future)

```
D3D9/10/11 --> DXVK --> Vulkan --> KosmicKrisp --> Metal
```

[KosmicKrisp](https://docs.mesa3d.org/drivers/kosmickrisp.html) is a fully Vulkan 1.3-conformant driver on Metal 4, built by LunarG (Google-sponsored), upstreamed to Mesa. Achieved MoltenVK feature parity in Mesa 26.0 (February 2026). Requires macOS 26+, Apple Silicon only.

**This is the biggest missing piece for getting the full Proton stack on macOS.** If KosmicKrisp supports `VK_EXT_graphics_pipeline_library`, it unlocks DXVK 2.x on macOS for the first time. This would be a game-changer.

### Path E: vkd3d-proton + Vulkan (DX12 via Vulkan)

```
D3D12 --> vkd3d-proton --> Vulkan --> MoltenVK/KosmicKrisp --> Metal
```

Experimental. DX12 requires ~1,000,000 shader resource views; Metal caps at ~500,000. Works on a per-game basis. Chip Davis (CodeWeavers) has patches. Long-term viability depends on KosmicKrisp.

### Auto-Select Logic

Cauldron's Rust core maintains a SQLite game database mapping Steam App IDs and executable hashes to optimal backends:

1. Check local DB for known-good backend.
2. Default: D3DMetal for DX12, DXMT for DX11, DXVK for DX9.
3. One-click override in UI.
4. Community reports feed back via optional telemetry.

---

## Synchronization: The Performance Battleground

Synchronization is where macOS can gain the most performance. The landscape:

### The Problem

Wine's default NT synchronization goes through wineserver round-trips. This is slow. Linux solved this with:
- **esync** (eventfd-based, user-space)
- **fsync** (futex-based, user-space, Valve's Proton)
- **ntsync** (kernel driver, Linux 6.14+, March 2025 -- 20-30% FPS gains)

macOS has none of these primitives natively.

### MSync (Current Best)

[MSync](https://github.com/marzent/wine-msync) by marzent. Uses Mach semaphore pools + dedicated Mach message pump in wineserver. The macOS equivalent of fsync/ntsync.

**Benchmarks (M2 Max):**

| Test | MSync | ESync | Improvement |
|------|-------|-------|-------------|
| Contended wait (10M iter) | 3.79s | 7.42s | ~49% faster |
| Zigzag test | 401,605 iter | 222,675 iter | ~80% more throughput |
| FFXIV CPU-bound | 219 FPS | 145 FPS | ~51% faster |

When `__ulock_wait2` (Apple's private futex-like API) is available, MSync achieves "better-than-NT performance" on single-wait cases.

Enable: `WINEMSYNC=1`

### os_sync_wait_on_address (macOS 14.4+)

Apple's first **public** futex-like API. Atomic compare-and-wait with `OS_SYNC_WAIT_ON_ADDRESS_SHARED` for cross-process operation via shared memory. Documentation is incomplete but the API is real and usable. This could be the foundation for a next-generation macOS sync mechanism that doesn't rely on private APIs.

### Cauldron's Sync Strategy

1. **Ship MSync** as the default (proven, fast).
2. **Experiment with `os_sync_wait_on_address`** for a public-API-only fast path.
3. **Long-term:** Investigate IOKit UserClient for kernel-side sync objects matching ntsync semantics.

---

## Proton Auto-Sync Pipeline

The core differentiator. Four stages:

### Stage 1: Monitor

Rust task (`git2` crate) polls Proton repo + submodules (Wine, DXVK, vkd3d-proton) for new commits. Each stored in SQLite with diff, message, author, affected files.

### Stage 2: Classify

| Classification | Signal | macOS Transferability |
|---|---|---|
| Wine API fix | `dlls/`, `server/`, `loader/` | **High** -- usually direct apply |
| DXVK fix | `dxvk/` submodule | **High** -- applies to Path C directly |
| vkd3d-proton fix | `vkd3d-proton/` submodule | **Medium** -- needs Vulkan compat check |
| Game-specific config | `proton` script, app ID lists | **High** -- config-only, import |
| Kernel/driver workaround | futex, fsync, `/proc` refs | **Low** -- needs macOS equivalent |
| Steam integration | `lsteamclient/`, `vrclient/` | **Low** -- Linux-specific IPC |
| Build system | `Makefile`, `configure.sh` | **None** -- skip |

### Stage 3: Adapt

Kernel-level mapping table:

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

### Stage 4: Validate

CI smoke tests: launch game, check for crashes within 60s, verify renderer init. Passing patches merge to `cauldron-nightly`. Failing patches quarantined.

---

## Proton-GE Patches: What We're Syncing

GE-Proton carries ~530 custom patches on top of Valve's bleeding-edge Wine. Here's what matters for macOS:

### High-Value Portable Patches

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

### Game-Specific Protonfixes (354+ Scripts)

The [umu-protonfixes](https://github.com/Open-Wine-Components/umu-protonfixes) system provides Python scripts for 354 Steam games plus GOG, Epic, Ubisoft, Amazon, Battle.net, itch.io, Humble. These are largely platform-agnostic:

- **DLL overrides:** `protontricks('vcrun2019')`, `protontricks('dotnet48')`
- **Launch arg injection:** `append_argument('-fullscreen -vulkan')`
- **Command replacement:** `replace_command('SkyrimSELauncher.exe', 'skse64_loader.exe')`
- **File creation:** Elden Ring dummy DLC files workaround
- **Automatic upscaler management:** Downloads and installs DLSS/XeSS/FSR DLLs with hash verification

### DLSS/Upscaler Translation for macOS

Proton-GE has a `WINE_LOADDLL_REPLACE` system for force-loading upscaler DLLs. On macOS, the equivalent is:

- **DLSS -> MetalFX:** Intercept nvngx_dlss.dll load, implement DLSS evaluation API against `MTLFXSpatialScaler`/`MTLFXTemporalScaler`. GPTK 3.0 already does this.
- **FSR 1.0 spatial:** Compute shaders, works on Metal.
- **FSR 3/4 frame gen:** Theoretically possible via Metal compute, untested independently.

### Wine-Staging Patches Not in Proton (macOS-Critical)

| Patchset | Relevance |
|---|---|
| `winemac.drv-no-flicker-patch` | **Directly targets macOS** -- reduces window flicker |
| `ntdll-WRITECOPY` | Proper WRITECOPY page protection |
| `gdiplus-Performance-Improvements` | GDI+ rendering (game menus/UI) |
| `user32-Mouse_Message_Hwnd` | Mouse message targeting |
| `mfplat-streaming-support` | Media Foundation video playback |
| `xactengine3_7-PrepareWave` | XACT audio (many XNA/DirectX games) |
| `dinput-scancode` | DirectInput keyboard/controller |

---

## The Rosetta 2 Problem

**Timeline:**
- macOS 26 Tahoe: Full Rosetta 2. Deprecation warnings in 26.4 (Feb 2026).
- macOS 27: Rosetta 2 still available on Apple Silicon. Intel Macs dropped.
- macOS 28 (Fall 2027): Rosetta 2 removed. Possible limited subset for "older unmaintained gaming titles."

**Impact:** Wine runs as x86-64 under Rosetta 2 on Apple Silicon. The entire translation stack is: Rosetta 2 (x86->ARM64) + Wine (Win32->POSIX) + D3DMetal (DX->Metal). Without Rosetta, x86 Windows games cannot run.

**Potential solutions being tracked:**
- [Jpkovas/FEX_MacOs](https://github.com/Jpkovas/FEX_MacOs) -- FEX-Emu ported to macOS (0 stars, 7,368+ inherited commits, very early)
- Wine ARM64EC support (Wine 10.0+) -- for ARM Windows binaries, not x86
- Apple's retained Rosetta subset -- scope unknown
- Box64 -- has 16K page size support, primarily Linux

**This is an existential risk.** We need to track and contribute to solutions.

---

## macOS Kernel Deep Dive

### Memory Model

- **16K pages on Apple Silicon.** Windows expects 4K. Wine 11 simulates different page sizes. This is non-trivial.
- **Low address space:** macOS maps first 4GB as `__PAGEZERO`. Wine's `preloader_mac.c` reserves ~8GB at `0x1000` with `MAP_FIXED` + `PROT_NONE`.
- **ARM64 ASLR is mandatory.** Signed pointers encode expected page zero size. No custom `pagezero_size`.
- **Shared memory max:** Only 4MB by default (vs. effectively infinite on Linux).

### Code Signing / JIT / SIP

- **W^X on Apple Silicon:** Pages can't be simultaneously writable and executable. Must use `MAP_JIT` + `pthread_jit_write_protect_np()`.
- **Required entitlements:** `com.apple.security.cs.allow-jit`, `com.apple.security.cs.disable-library-validation`, `com.apple.security.cs.allow-dyld-environment-variables`.
- **Gatekeeper:** Install with `--no-quarantine` or `xattr -rd com.apple.quarantine`.
- **SIP:** Strips `DYLD_*` vars from protected binaries. Wine binaries are not SIP-protected, so this is manageable.

### Exception Handling

Wine uses Mach exception ports (`task_set_exception_ports`) for SEH emulation rather than POSIX signals. Flow: hardware exception -> `exception_triage()` -> thread port -> task port -> host port -> BSD signal fallback. More reliable than signal handling on macOS.

### Threading

Wine uses pthreads for creation, drops to Mach level for: debug registers (`thread_get_state`/`thread_set_state`), suspension (`task_suspend`/`task_resume`), TLS setup (private `_thread_set_tsd_base()` for GSBASE).

### Audio

`winecoreaudio.drv` maps WASAPI/mmdevapi to CoreAudio via AUHAL. Works but latency management is complex. Proton-GE's `winepulse-fast-polling.patch` concept (tighter polling for lower latency) should be adapted for CoreAudio.

---

## Ecosystem: Projects We Build On

### Critical Dependencies

| Project | Role | Status |
|---|---|---|
| [Gcenx/macOS_Wine_builds](https://github.com/Gcenx/macOS_Wine_builds) | Official WineHQ macOS packages | Active. Gcenx is the linchpin of free Wine-on-macOS. |
| [3Shain/dxmt](https://github.com/3Shain/dxmt) | Metal-native D3D11 | Active. v0.74. In CrossOver 26. |
| [marzent/wine-msync](https://github.com/marzent/wine-msync) | macOS sync primitives | Mature. In Whisky and CrossOver. |
| [Gcenx/DXVK-macOS](https://github.com/Gcenx/DXVK-macOS) | DXVK 1.10.3 for macOS | Maintenance. Ceiling until KosmicKrisp. |
| [KhronosGroup/MoltenVK](https://github.com/KhronosGroup/MoltenVK) | Vulkan on Metal | Active. v1.4. Missing key extensions. |
| KosmicKrisp (Mesa) | Vulkan 1.3 on Metal 4 | Alpha. Game-changer potential. |
| [italomandara/CXPatcher](https://github.com/italomandara/CXPatcher) | CrossOver component upgrader | Active. Bridges release gaps. |

### Whisky Forks Worth Watching

| Fork | Why | Status |
|---|---|---|
| [frankea/Whisky](https://github.com/frankea/Whisky) | Most professional successor. Wine 11.0, 67 commits ahead, 83% test coverage, launcher compat system, CI/CD. | Active (Jan 2026) |
| [cyyever/Whisky](https://github.com/cyyever/Whisky) | Most bleeding-edge. Wine 11.5, DXMT submodule, **fixed wineboot hang on macOS 26 Tahoe**. | Active (Mar 2026) |
| [SkuldNorniern/Whisky](https://github.com/SkuldNorniern/Whisky) | Rust integration experiment. Swift-Rust bridge, PE parsing in Rust. Validates our architecture. | Active (Feb 2026) |
| [Zinedinarnaut/Whisky](https://github.com/Zinedinarnaut/Whisky) | "Vector" -- heavy Steam optimization, macOS 26 fixes. | Active (Feb 2026) |
| [ThatOneTequilaDev/Tequila](https://github.com/ThatOneTequilaDev/Tequila) | Wine 11 integration, builds on Bourbon's DXMT work. | Active (Jan 2026) |

### Diamond-in-the-Rough Projects

| Project | What | Why It Matters |
|---|---|---|
| [Gcenx/macports-wine](https://github.com/Gcenx/macports-wine) | 997 commits, 116 stars | One person maintaining the entire free Wine-on-macOS build infra |
| [Jpkovas/FEX_MacOs](https://github.com/Jpkovas/FEX_MacOs) | 0 stars, 7,368+ commits | FEX-Emu x86 emulator ported to macOS. If viable, solves Rosetta deprecation. |
| [MythicApp/Mythic](https://github.com/MythicApp/Mythic) | 1,229 stars, 1,619 commits | Full macOS game launcher with GPTK integration |
| [neo773/macgamingdb](https://github.com/neo773/macgamingdb) | 91 stars | Community game compat DB at macgamingdb.app |
| [philipturner/metal-benchmarks](https://github.com/philipturner/metal-benchmarks) | 592 stars, 418 commits | Apple GPU microarchitecture documentation. Foundational. |
| [kiku-jw/peak-crossover-mouse-fix](https://github.com/kiku-jw/peak-crossover-mouse-fix) | 11 stars | Fixes Unity pointer bug blocking many games. Tiny but critical. |
| [EnderIce2/rpc-bridge](https://github.com/EnderIce2/rpc-bridge) | 200 stars | Discord Rich Presence for Wine games |

---

## CrossOver 26: What They Solved (February 2026)

CrossOver 26 represents the current state of the art:

- **Anti-cheat:** nProtect GameGuard, EAC, and BattlEye now work for 20+ AAA titles. CodeWeavers calls this "curing artificial incompatibility."
- **Components:** Wine 11.0, D3DMetal 3.0, DXMT v0.72, vkd3d 1.18, NTSync (Linux)
- **DLSS -> MetalFX:** Intercepts NVIDIA DLSS/DLSS-FG calls and translates to MetalFX upscaling + frame interpolation
- **Tested titles:** Helldivers 2, Kingdom Come: Deliverance II, God of War Ragnarok, Starfield, Age of Empires IV

### What CrossOver Has That Upstream Wine Doesn't

1. **wine32on64** -- Custom LLVM compiler for 32-bit on 64-bit macOS (requires forked Clang-8 with `cdecl32`/`stdcall32`/`thiscall32`/`fastcall32` attributes)
2. **Patched MoltenVK** -- Fakes unsupported Vulkan extensions
3. **Custom DXVK** -- macOS-specific modifications
4. **D3DMetal integration** -- Apple's proprietary layer
5. **DXMT integration** -- 3Shain's Metal D3D11
6. **MSync** -- Mach semaphore synchronization
7. **Anti-cheat patches** -- Proprietary, not open-source
8. **DLSS via MetalFX** -- Maps NVIDIA calls to Apple's upscaler

---

## MoltenVK: The Extension Gap

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

## Repository Structure

```
cauldron/
+-- cauldron-core/          # Rust: bottle management, process spawning, environment setup
+-- cauldron-sync/          # Rust: Proton monitor, commit classifier, patch adapter
+-- cauldron-db/            # Rust: SQLite game database, compatibility records
+-- cauldron-bridge/        # Rust: C FFI exports for Swift, cbindgen headers
+-- CauldronApp/            # Xcode: SwiftUI shell
+-- wine/                   # Submodule: Cauldron's Wine fork (Proton-based)
+-- dxvk/                   # Submodule: DXVK fork
+-- dxmt/                   # Submodule: DXMT (3Shain's Metal D3D11)
+-- moltenvk/               # Submodule: MoltenVK
+-- scripts/                # Python: Proton config parser, patch analysis
+-- db/                     # Seed data: game compat DB, migrations
+-- ci/                     # GitHub Actions: nightly sync, build, test, release
+-- Cargo.toml              # Workspace root
```

---

## Project Phases

### Phase 0: Foundation (Weeks 1-4)

- Fork Whisky. Strip Swift UI to minimal shell.
- Init Rust workspace: `cauldron-core`, `cauldron-sync`, `cauldron-db` crates.
- Implement Rust-to-Swift FFI bridge.
- Fork Wine (from Proton's Wine) and verify macOS build.
- CI pipeline on GitHub Actions.

**Exit:** Rust core creates a Wine bottle and launches `notepad.exe` via Swift UI.

### Phase 1: Graphics Backend (Weeks 5-10)

- Integrate DXVK-macOS + MoltenVK for DX9/10/11.
- Integrate D3DMetal/GPTK for DX11/12.
- Integrate DXMT for DX10/11 (the best DX11 path).
- Build auto-select logic in Rust.
- Test 10+ games across paths.

**Exit:** D3D11 game via DXMT, D3D12 via D3DMetal, D3D9 via DXVK, auto-selected.

### Phase 2: Sync Pipeline (Weeks 11-18)

- Proton repo monitor via `git2`.
- Commit classifier with transferability scoring.
- Game config importer (parse `default_compat_config()`).
- Patch adapter with conflict detection.
- Kernel-mapping layer (MSync integration, macOS equivalents).
- Nightly CI: auto-sync, build, smoke test.

**Exit:** New Proton Wine API fix auto-applied within 24 hours.

### Phase 3: Community & Polish (Weeks 19-26)

- Community compatibility reporting (opt-in).
- Game library UI with compat status.
- Shader cache sharing.
- Proton-GE-style community patch integration.
- Performance profiling (Metal HUD, frame timing).
- Protonfixes integration (354+ game scripts).

**Exit:** Public beta. 50+ games tested.

---

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| MoltenVK extension gaps | High | KosmicKrisp as alternative; DXMT for DX11; D3DMetal for DX12 |
| Rosetta 2 deprecation (macOS 28) | Critical | Track FEX_MacOs, Wine ARM64EC, Apple's retained subset |
| Apple breaks GPTK in macOS update | High | Pin known-good versions; test on betas early |
| Wine fork diverges from macOS-buildable | Medium | Maintain rebase branch; don't track Proton HEAD blindly |
| Anti-cheat blocks macOS | High | Out of scope for v1; CrossOver 26 made progress here |
| 16K page size issues | Medium | Wine 11 simulates 4K pages; test thoroughly |
| Code signing / JIT restrictions | Medium | Proper entitlements; `MAP_JIT` + `pthread_jit_write_protect_np` |

---

## Key Environment Variables Reference

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

## Contributing

This project is in early stages. The most valuable contributions right now:

1. **Game testing and reporting** -- Try games, document what works and what doesn't.
2. **Patch classification** -- Help classify Proton commits by macOS transferability.
3. **Kernel mapping** -- Implement macOS equivalents of Linux-specific Wine features.
4. **KosmicKrisp testing** -- Test DXVK 2.x with KosmicKrisp on macOS 26+.
5. **Rosetta alternatives** -- Any work on x86 emulation for macOS ARM64.

---

## License

LGPL-2.1 (matching Wine's license).

---

## Acknowledgments

Cauldron builds on the work of many projects and individuals:

- **Whisky** (archived) -- The SwiftUI Wine wrapper that started it all
- **Gcenx** -- Maintaining the entire free Wine-on-macOS ecosystem single-handedly
- **3Shain** -- DXMT, the most impactful solo contribution to macOS gaming
- **marzent** -- MSync, making Wine fast on macOS
- **GloriousEggroll** -- Proton-GE and the community patching model we're adapting
- **CodeWeavers** -- Two thirds of Wine commits and the CrossOver ecosystem
- **LunarG** -- KosmicKrisp, potentially the biggest unlock for macOS gaming
- **Apple** -- D3DMetal, MetalFX, and (reluctantly) Rosetta 2
- **Valve** -- Proton, DXVK, and open-sourcing MoltenVK
- **The Frogging Family** -- wine-tkg and community patches
- **frankea, cyyever, SkuldNorniern** -- Whisky fork maintainers pushing forward
- **philipturner** -- Apple GPU microarchitecture documentation
- **italomandara** -- CXPatcher bridging the gap

---

*Cauldron is not affiliated with Valve, Apple, CodeWeavers, or any of the projects listed above.*
