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

C## Graphics Backend: The Full Picture

The macOS DirectX translation landscape is more complex and more capable than most people realize. As of April 2026:

### Path A: D3DMetal (Apple GPTK)

```
D3D11/12 --> D3DMetal --> Metal
```

Apple's proprietary DirectX-to-Metal layer. Ships with GPTK 3.0 (WWDC 2025). Skips Vulkan entirely. Best for D3D12 titles. Supports DXR on M3+ (`D3DM_SUPPORT_DXR=1`). GPTK 3.0 adds DLSS-to-MetalFX translation, MetalFX Frame Interpolation, sparse buffers/textures, performance insights, and function constants.

**Does NOT support DirectX 9.** DX9 games must use another path.

**D3DMetal internals:** D3DMetal intercepts D3D11/12 API calls and translates them to Metal at runtime. HLSL shaders compiled to DXBC (D3D11) or DXIL (D3D12) bytecode are translated to Metal Shading Language (MSL) via Apple's [Metal Shader Converter](https://developer.apple.com/metal/shader-converter/), then compiled by the Metal shader compiler. D3D resources, pipeline states, and command lists are mapped to Metal equivalents. Available via [MacPorts](https://ports.macports.org/port/d3dmetal/) (`port install d3dmetal`). **Closed-source** -- no significant RE efforts exist; the community uses it as a black box.

### Path B: DXMT (Direct Metal)

```
D3D10/11 --> DXMT --> Metal
```

[DXMT](https://github.com/3Shain/dxmt) by 3Shain. ~1,000 commits from a solo developer. Metal-native D3D11/D3D10 implementation built specifically for Wine on macOS. Now integrated into CrossOver 26 as v0.72. Outperforms both DXVK+MoltenVK and D3DMetal on lower-spec Macs. Supports MetalFX Spatial Upscaling via `DXMT_METALFX_SPATIAL_SWAPCHAIN`, shared resources via D3DKMT (Wine 10.18+), experimental Intel Mac support, emulated fullscreen, and HDR.

Architecture has four layers: API implementation, command encoding, shader compilation, and Metal backend integration.

**The single most important community contribution to macOS gaming in the last two years.**

### Path C: DXVK + MoltenVK

```
D3D9/10/11 --> DXVK --> Vulkan --> MoltenVK --> Metal
```

The Proton-compatible path. Benefits directly from Proton's DXVK fixes. On macOS, stuck at [DXVK 1.10.3](https://github.com/Gcenx/DXVK-macOS) because upstream DXVK 2.0+ requires `VK_EXT_graphics_pipeline_library`, which MoltenVK does not support ([issue #1711](https://github.com/KhronosGroup/MoltenVK/issues/1711)).

Async shader compilation works via `DXVK_ASYNC=1` in Gcenx's fork. **This is the only path that handles DX9.** [DXVK-GPLAsync](https://gitlab.com/Ph42oN/dxvk-gplasync) (164 stars, GitLab) provides additional async pipeline compilation patches to reduce shader stutter -- should be integrated into our DXVK fork.

### Path D: DXVK + KosmicKrisp (Future)

```
D3D9/10/11 --> DXVK --> Vulkan --> KosmicKrisp --> Metal
```

[KosmicKrisp](https://docs.mesa3d.org/drivers/kosmickrisp.html) is a fully Vulkan 1.3-conformant driver on Metal 4, built by LunarG (Google-sponsored), upstreamed to [Mesa](https://gitlab.freedesktop.org/mesa/mesa) 26.0 (February 2026). Achieved MoltenVK feature parity by February 2026. Requires macOS 26+, Apple Silicon only. Roadmap includes Vulkan 1.4, tessellation, geometry shaders, mesh shaders, and iOS support.

**This is the biggest missing piece for getting the full Proton stack on macOS.** If KosmicKrisp supports `VK_EXT_graphics_pipeline_library`, it unlocks DXVK 2.x on macOS for the first time. This would be a game-changer.

### Path E: vkd3d-proton + Vulkan (DX12 via Vulkan)

```
D3D12 --> vkd3d-proton --> Vulkan --> MoltenVK/KosmicKrisp --> Metal
```

Experimental. DX12 requires ~1,000,000 shader resource views; Metal caps at ~500,000. Metal also lacks Virtual Addresses (VAs) and Buffer Device Addresses (BDAs) -- Apple insists argument buffers suffice, but DX12 expects these features. Works on a per-game basis. Chip Davis (CodeWeavers) has patches. VKD3D-Proton 3.0 (November 2025) shipped a rewritten DXBC shader backend shared with DXVK, plus AMD FSR4 via Vulkan cooperative matrix extensions.

Long-term viability depends on KosmicKrisp and Metal 4's new capabilities (bindless rendering, tensor operations, mesh shaders, ray tracing refinements).

The canonical VKD3D development lives at [gitlab.winehq.org/wine/vkd3d](https://gitlab.winehq.org/wine/vkd3d) with very active developer forks (Elizabeth Figura, Giovanni Mascellani, Francisco Casas, Henri Verbeet).

### Path F: Asahi Linux Full Stack (Reference Implementation)

```
Windows Game --> FEX (x86) --> Wine --> DXVK/vkd3d-proton --> Honeykrisp (native Vulkan) --> Apple GPU
```

Not a macOS path, but the proof that full Proton gaming works on Apple Silicon hardware. Asahi Linux runs the complete Proton stack with [FEX-Emu](https://github.com/FEX-Emu/FEX) for x86 emulation, Wine, DXVK/vkd3d-proton, and [Honeykrisp](https://gitlab.freedesktop.org/mesa/mesa) -- the first and only fully conformant Vulkan 1.4 implementation for Apple GPU hardware, without portability waivers. No Metal middleman.

Confirmed playable: Cyberpunk 2077, The Witcher 3, Control, Fallout 4, Portal 2, Ghostrunner. Uses [muvm](https://github.com/nickvdp/muvm) for lightweight device passthrough.

This validates that Apple's GPU hardware is capable. The macOS bottleneck is Apple's Metal-only GPU access policy.

### Auto-Select Logic

Cauldron's Rust core maintains a SQLite game database mapping Steam App IDs and executable hashes to optimal backends:

1. Check local DB for known-good backend.
2. Default: D3DMetal for DX12, DXMT for DX11, DXVK for DX9.
3. One-click override in UI.
4. Community reports feed back via optional telemetry.

### Graphics Path Comparison

| Backend | Translation Path | DX Support | Best For | Notes |
|---------|-----------------|------------|----------|-------|
| **D3DMetal** | D3D11/12 -> Metal (direct) | DX11-12 | Modern DX12 games | Apple proprietary, best DX12 coverage |
| **DXMT** | D3D10/11 -> Metal (direct) | DX10-11 | Modern DX11 games | Open-source, single-hop, great perf |
| **DXVK+MoltenVK** | D3D9-11 -> Vulkan -> Metal | DX9-11 | Pre-2012 games, DX9 | Double-hop, only DX9 path |
| **DXVK+KosmicKrisp** | D3D9-11 -> Vulkan -> Metal | DX9-11 | Future DX9-11 (conformant) | Requires macOS 26+, Metal 4 |
| **vkd3d+Vulkan** | D3D12 -> Vulkan -> Metal | DX12 | Experimental DX12 alt | Metal binding limits, triple-hop |
| **WineD3D** | D3D -> OpenGL -> (ANGLE) Metal | DX1-11 | Very old games, last resort | Triple-hop, lowest performance |

---

## Shader Translation: The Critical Pipeline

Every graphics path depends on shader translation. The ecosystem:

### The Translation Chain

```
HLSL Source
  |
  v
DXBC (SM 4/5) or DXIL (SM 6.x)     <-- What Windows games ship
  |
  v  (dxil-spirv / vkd3d / DXVK)
SPIR-V                                <-- Vulkan's shader format
  |
  v  (SPIRV-Cross / MoltenVK / KosmicKrisp)
MSL (Metal Shading Language)          <-- What Apple GPUs consume
  |
  v  (Metal compiler)
AIR (Apple Intermediate Representation) --> GPU machine code
```

### Key Shader Tools

| Project | What It Does | Role in Pipeline |
|---------|-------------|------------------|
| [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross) | SPIR-V -> MSL/GLSL/HLSL | Core of MoltenVK's shader translation |
| [dxil-spirv](https://github.com/HansKristian-Work/dxil-spirv) | DXIL/DXBC -> SPIR-V | Used by vkd3d-proton. MIT licensed. |
| [Apple Metal Shader Converter](https://developer.apple.com/metal/shader-converter/) | DXIL -> Metal AIR (official) | Used by D3DMetal/GPTK |
| [SDL_shadercross](https://github.com/libsdl-org/SDL_shadercross) | HLSL -> DXBC/DXIL/SPIR-V/MSL unified | SDL's all-in-one pipeline |
| [Slang](https://github.com/shader-slang/slang) (5,176 stars) | Universal shader compiler -> Metal/Vulkan/D3D12 | Could simplify cross-platform shader story |
| [Naga](https://github.com/gfx-rs/naga) (1,568 stars) | SPIR-V/WGSL -> MSL (Rust) | Powers wgpu. 80x faster than Tint |
| [ShaderConductor](https://github.com/microsoft/ShaderConductor) (1,832 stars) | HLSL -> MSL via DXC + SPIRV-Cross | Microsoft's cross-compiler |
| [HLSLcc](https://github.com/Unity-Technologies/HLSLcc) (906 stars) | DX bytecode -> Metal/GL/Vulkan | Used internally by Unity |
| [CrossShader](https://github.com/alaingalvan/CrossShader) (306 stars) | GLSL/HLSL/MSL cross-compilation | Wraps DXC, glslang, Naga, SPIRV-Cross |
| [ShaderTranspiler](https://github.com/RavEngine/ShaderTranspiler) (98 stars) | GLSL -> HLSL/Metal/Vulkan/WebGPU | Clean C++ library |

### Metal AIR Internals (Reverse-Engineered)

Understanding Apple's shader intermediate representation enables custom compilation pipelines that bypass Apple's closed tools:

| Project | What It Does | Stars |
|---------|-------------|-------|
| [metal-air-docs](https://github.com/SamoZ256/metal-air-docs) | **Reverse-engineered AIR format documentation.** AIR is LLVM 4.0 bitcode with Metal-specific `!air.` metadata prefixes. Includes example shaders and compilation scripts. | New |
| [LLAIR](https://github.com/gzorin/LLAIR) | C++ library for **runtime generation and manipulation of Metal AIR**. Can produce `.metallib` files programmatically. Depends on a specialized LLVM fork. | 57 |
| [MetalLibraryArchive](https://github.com/YuAo/MetalLibraryArchive) | Extract Metal functions from `.metallib` files | 179 |
| [MetalLibraryExplorer](https://github.com/YuAo/MetalLibraryExplorer) | Parse and disassemble `.metallib` in browser | 52 |
| [MetalShaderTools](https://github.com/zhuowei/MetalShaderTools) | **Proves `.air` files are valid LLVM bitcode.** Includes `unmetallib.py` to extract `.metallib` archives into `.air` files and recompile to x86_64/ARM64 assembly with standard LLVM. [Blog post](https://worthdoingbadly.com/metalbitcode/). | 87 |
| [metal-ir-pipeline](https://github.com/imperatormk/metal-ir-pipeline) | LLVM IR -> Metal AIR -> metallib pipeline for GPU compute | 0 |
| [MetallibSupportPkg](https://github.com/dortania/MetallibSupportPkg) | Metal Library patching utilities (Hackintosh community) | 69 |
| [SamoZ256's AIR breakdown](https://medium.com/@samuliak/breaking-down-metals-intermediate-representation-format-41827022489c) | Detailed walkthrough of AIR internals including `air.` standard library functions and address space attributes | Article |

**Why this matters for Cauldron:** A custom SPIR-V -> AIR compiler could bypass SPIRV-Cross -> MSL -> Metal compiler entirely, reducing shader compilation latency and enabling optimizations Apple's toolchain doesn't. LLAIR + metal-air-docs make this tractable.

### Industry Shift: Microsoft Adopting SPIR-V

Microsoft announced that **Shader Model 7.0 will adopt SPIR-V as its interchange format**, with official SPIR-V<->DXIL translation tools. Long-term, this simplifies the entire cross-platform shader pipeline -- games will ship SPIR-V natively, eliminating the DXBC/DXIL -> SPIR-V translation step. Track this closely.

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

### NTSync on macOS (CrossOver 26)

CrossOver 26 ships NTSync support, suggesting CodeWeavers has ported equivalent functionality to macOS using macOS-native primitives. This validates the approach.

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

GE-Proton carries ~530 custom patches on top of Valve's bleeding-edge Wine. GE-Proton is active through GE-Proton10-34 (March 2026) but Linux-only -- no macOS variants exist. wine-ge-custom was archived July 2025 as developers began collaborating on ULWGL (Universal Linux Wine Glue Layer). Here's what matters for macOS:

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

- **DLSS -> MetalFX:** Intercept nvngx_dlss.dll load, implement DLSS evaluation API against `MTLFXSpatialScaler`/`MTLFXTemporalScaler`. GPTK 3.0 already does this. [crossover-gptk3-patcher](https://github.com/pleasenotagain/crossover-gptk3-patcher) patches CrossOver with GPTK 3.0 to enable DLSS-via-MetalFX.
- **FSR 1.0 spatial:** Compute shaders, works on Metal.
- **FSR 3/4 frame gen:** Theoretically possible via Metal compute, untested independently.

### Wine-Staging Patches Not in Proton (macOS-Critical)

Wine-Staging ([gitlab.winehq.org/wine/wine-staging](https://gitlab.winehq.org/wine/wine-staging), active through Wine Staging 11.5, March 2026) carries experimental patches:

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

## Wine on macOS: Active Development

Wine's macOS support is actively maintained. Wine 11.0 (January 2026) and Wine 11.5 (March 2026) ship with full macOS support. Key developments on [gitlab.winehq.org/wine/wine](https://gitlab.winehq.org/wine/wine):

### Critical Merge Requests to Track

| MR | Author | What | Status | Impact |
|---|---|---|---|---|
| [!6755](https://gitlab.winehq.org/wine/wine/-/merge_requests/6755) | Isaac Marovitz | **wined3d: Metal renderer** -- native Metal rendering backend for wined3d | Draft | Would give Wine direct Metal rendering without translation layers |
| [!7938](https://gitlab.winehq.org/wine/wine/-/merge_requests/7938) | Marc-Aurel Zent | **winemac: IOSurface for window layers** -- replaces CGImage with IOSurface | Draft | **50ms -> 1.5ms** window update latency. Critical for Metal rendering path. |
| [!10523](https://gitlab.winehq.org/wine/wine/-/merge_requests/10523) | — | **winemac.drv: GPU VendorId/DeviceId from Vulkan/MoltenVK** | Open | Fixes inconsistent GPU device ID reporting on Apple Silicon |
| [!9579](https://gitlab.winehq.org/wine/wine/-/merge_requests/9579) | — | **ntdll: Transform main thread into Wine thread on macOS** -- runs CFRunLoop | Draft | macOS main thread integration |
| [!10463](https://gitlab.winehq.org/wine/wine/-/merge_requests/10463) | — | **winemac: Replace OSAtomic functions** | Open | Modernizes atomic operations for macOS |

Plus recently merged MRs for macOS retina display handling, EDID support, HDR status reporting, OpenGL context fixes, and clipboard support.

### 32-bit Support (Solved)

macOS dropped all 32-bit support in Catalina (2019). CodeWeavers solved this with a custom Clang compiler understanding both 32-bit and 64-bit pointers simultaneously -- "WoW64" mode. This is now upstream in Wine 11. Wine no longer requires 32-bit host libraries. **This blocker is resolved.**

### ARM64EC Support

Wine 10.0 added full ARM64EC support for ARM Windows binaries. This is separate from x86 emulation but relevant to the post-Rosetta transition.

---

## The Rosetta 2 Problem

**Timeline:**
- macOS 26 Tahoe: Full Rosetta 2. Deprecation warnings in 26.4 (Feb 2026).
- macOS 27: Rosetta 2 still available on Apple Silicon. Intel Macs dropped.
- macOS 28 (Fall 2027): Rosetta 2 removed. Possible limited subset for "older unmaintained gaming titles."

**Impact:** Wine runs as x86-64 under Rosetta 2 on Apple Silicon. The entire translation stack is: Rosetta 2 (x86->ARM64) + Wine (Win32->POSIX) + D3DMetal (DX->Metal). Without Rosetta, x86 Windows games cannot run.

**Rosetta performance note:** Overhead is generally minimal; some benchmarks show x86_64 programs performing better under Rosetta 2 on M1 than native x86_64 on Intel. Initial load times are slower (JIT compilation), but in-game performance is stable. This makes the deprecation especially painful -- it works well.

**Potential solutions being tracked:**

| Project | What | Status |
|---|---|---|
| [Jpkovas/FEX_MacOs](https://github.com/Jpkovas/FEX_MacOs) | FEX-Emu x86 emulator ported to macOS | 0 stars, 7,368+ commits, very early |
| [FEX-Emu/FEX](https://github.com/FEX-Emu/FEX) (7,140 stars) | Fast x86 emulator for ARM64 Linux | Linux only. Leverages Apple Silicon's TSO hardware mode. Valve integrating into Proton for ARM64 SteamOS. CodeWeavers integrated into CrossOver ARM64 Linux preview (Nov 2025). [macOS discussion](https://github.com/FEX-Emu/FEX/discussions/3267) shows community interest but no active port. |
| [ptitSeb/box64](https://github.com/ptitSeb/box64) (5,319 stars) | x86_64 emulator for ARM64 | Linux only. v0.2.8 added 16K page support for Asahi Linux. No macOS port. |
| [Inokinoki/attesor](https://github.com/Inokinoki/attesor) (59 stars) | AI-powered Rosetta 2 reverse engineering for Linux | Research project. Could inform alternative x86 translation. |
| Wine ARM64EC | ARM Windows binary support | Wine 10.0+. For ARM binaries, not x86 emulation. |
| Apple's retained Rosetta subset | Unknown scope | May keep limited support for games |

**Valve's FEX+Proton integration** for ARM64 SteamOS devices (Steam Frame VR headset) and CodeWeavers' FEX integration into CrossOver validate FEX as the post-Rosetta template. A macOS port is the critical missing piece.

**This is an existential risk.** We need to track and contribute to solutions.

---

## Apple GPU: What We Know

### Reverse-Engineered Hardware Documentation

The Asahi Linux project has produced the most comprehensive public documentation of Apple's GPU architecture through pure reverse engineering:

| Resource | What It Covers |
|---|---|
| [dougallj/applegpu](https://github.com/dougallj/applegpu) (648 stars) | Apple G13 GPU ISA: disassembler, emulator, assembler. Register files (r0-r127, 32-bit/thread), instruction encodings, execution characteristics. The canonical reference. |
| [Asahi Linux GPU docs](https://asahilinux.org/docs/hw/soc/agx/) | AGX architecture, command stream, firmware interface, memory management, tiling architecture |
| [AsahiLinux/gpu](https://github.com/AsahiLinux/gpu) | IOKit GPU access demos (`demo/iokit.c`), `agxdecode` command stream decoder, `wrap.dylib` for intercepting `IOConnectCallMethod` GPU submissions |
| [philipturner/metal-benchmarks](https://github.com/philipturner/metal-benchmarks) (592 stars) | Apple GPU microarchitecture documentation and benchmarks |
| [philipturner/applegpuinfo](https://github.com/philipturner/applegpuinfo) (97 stars) | Print all known GPU hardware info from command line |
| [corsix/amx](https://github.com/corsix/amx) | Apple AMX (Matrix Extensions) reverse engineering, M1 through M4 |
| Alyssa Rosenzweig's blog series ([parts I-VI](https://alyssarosenzweig.ca/blog/asahi-gpu-part-1.html)) | Deepest public documentation of GPU command stream, macOS UABI for AGX, driver internals |
| [2026 arXiv paper (2603.28793)](https://arxiv.org/html/2603.28793) | First cross-vendor GPU ISA analysis spanning NVIDIA, AMD, Intel, and Apple across 16 microarchitectures |

### Metal 4 (WWDC 2025)

Metal 4 introduces capabilities critical for DX12 feature parity:
- **Bindless rendering** -- closes the gap with DX12's resource binding model
- **Tensor operations in shaders** -- enables ML workloads on GPU
- **MetalFX frame interpolation and denoising** -- DLSS equivalent
- **Mesh shaders** -- DX12 mesh shader support
- **Ray tracing refinements** -- improved DXR compatibility
- **Function constants** -- shader specialization

### IOKit GPU Access

- **AGXAccelerator** -- IOKit service for Apple's GPU. Accessible via `IOServiceNameMatching("AGXAccelerator")`
- **IOGPU** -- kernel extension managing graphics state
- **IOAccelerator** -- kernel class for GPU acceleration
- macOS provides **no mechanism for alternate GPU drivers**. Apple's Metal is the only sanctioned path. Direct GPU access (as Asahi does on Linux) is blocked by macOS's security model.

### GPU Virtualization

- Apple's **Hypervisor.framework** provides hardware virtualization but **no GPU passthrough**
- **libkrun + virtio-gpu + Venus protocol** enables Vulkan API forwarding from Linux containers to macOS Metal via MoltenVK. Used by Podman 5.x. Achieved 40x AI inference speedup. [Blog post](https://sinrega.org/2024-03-06-enabling-containers-gpu-macos/). This is an alternative GPU acceleration path worth watching.

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

## Vulkan on macOS: Three Paths

| Approach | Platform | How It Works | Status |
|---|---|---|---|
| **MoltenVK** | macOS/iOS | Translates Vulkan API -> Metal. SPIR-V -> MSL via SPIRV-Cross. | Vulkan 1.4 (portability waivers). Mature but non-conformant. |
| **KosmicKrisp** | macOS 26+ | Mesa Vulkan driver layered on Metal 4. | Vulkan 1.3 conformant (Oct 2025). MoltenVK parity (Feb 2026). Merged Mesa 26.0. |
| **Honeykrisp** | Linux (Asahi) | Direct GPU access via reverse-engineered kernel driver + Mesa. No Metal. | Vulkan 1.4 conformant. Fully upstream Mesa. |

### OpenGL on macOS

Apple deprecated OpenGL (stuck at non-conformant 4.1 since 2013). Alternatives:

| Approach | Path | Notes |
|---|---|---|
| **ANGLE** (Google) | OpenGL ES 3.0 -> Metal | Official Metal backend, used by Chrome/Godot. Active co-development with Apple. |
| **Zink** (Mesa) | OpenGL -> Vulkan -> MoltenVK -> Metal | Experimental on macOS. [Someone got Minecraft at 70fps](https://gist.github.com/lucamignatti/5312f5e937de2ba44256ecba6de54cc2) with OpenGL 4.6 via Zink+MoltenVK. |
| [MetalANGLE](https://github.com/kakashidinho/metalangle) (498 stars) | OpenGL ES -> Metal | Independent ANGLE fork. No longer actively developed (author joined Google). |

Asahi Linux ships the **only conformant** OpenGL 4.6, OpenGL ES 3.2, and OpenCL 3.0 implementations for Apple Silicon -- all via Mesa, all on Linux.

---

## Ecosystem: Projects We Build On

### Critical Dependencies

| Project | Role | Status |
|---|---|---|
| [Gcenx/macOS_Wine_builds](https://github.com/Gcenx/macOS_Wine_builds) | Official WineHQ macOS packages | Active. Gcenx is the linchpin of free Wine-on-macOS. |
| [3Shain/dxmt](https://github.com/3Shain/dxmt) | Metal-native D3D11 | Active. v0.74. In CrossOver 26. |
| [marzent/wine-msync](https://github.com/marzent/wine-msync) | macOS sync primitives | Mature. In Whisky and CrossOver. |
| [Gcenx/DXVK-macOS](https://github.com/Gcenx/DXVK-macOS) | DXVK 1.10.3 for macOS | Maintenance. Ceiling until KosmicKrisp. |
| [KhronosGroup/MoltenVK](https://github.com/KhronosGroup/MoltenVK) | Vulkan on Metal | Active. v1.4.1. Nearly-conformant Vulkan 1.4. Missing key extensions. |
| KosmicKrisp ([Mesa](https://gitlab.freedesktop.org/mesa/mesa)) | Vulkan 1.3 on Metal 4 | Alpha. Game-changer potential. |
| [italomandara/CXPatcher](https://github.com/italomandara/CXPatcher) | CrossOver component upgrader | Active. Bridges release gaps. |
| [Wine](https://gitlab.winehq.org/wine/wine) | The foundation | Active (updated hourly). macOS is first-class. |
| [VKD3D](https://gitlab.winehq.org/wine/vkd3d) | D3D12 -> Vulkan | Very active. Multiple developer forks on WineHQ GitLab. |
| [Wine-Staging](https://gitlab.winehq.org/wine/wine-staging) | Experimental patches | Active (Wine Staging 11.5, March 2026). |

### Whisky Forks Worth Watching

| Fork | Why | Status |
|---|---|---|
| [frankea/Whisky](https://github.com/frankea/Whisky) | Most professional successor. Wine 11.0, 67 commits ahead, 83% test coverage, launcher compat system, CI/CD. | Active (Jan 2026) |
| [cyyever/Whisky](https://github.com/cyyever/Whisky) | Most bleeding-edge. Wine 11.5, DXMT submodule, **fixed wineboot hang on macOS 26 Tahoe**. | Active (Mar 2026) |
| [SkuldNorniern/Whisky](https://github.com/SkuldNorniern/Whisky) | Rust integration experiment. Swift-Rust bridge, PE parsing in Rust. Validates our architecture. | Active (Feb 2026) |
| [Zinedinarnaut/Whisky](https://github.com/Zinedinarnaut/Whisky) | "Vector" -- heavy Steam optimization, macOS 26 fixes. | Active (Feb 2026) |
| [ThatOneTequilaDev/Tequila](https://github.com/ThatOneTequilaDev/Tequila) | Wine 11 integration, builds on Bourbon's DXMT work. | Active (Jan 2026) |
| [leonewt0n/Bourbon](https://github.com/leonewt0n/Bourbon) (153 stars) | Direct Whisky fork, multiple re-forks. | Active (Mar 2026) |

### macOS Game Launchers & Wrappers

| Project | Stars | What | Status |
|---|---|---|---|
| [Sikarugir](https://github.com/Sikarugir-App/Sikarugir) | 2,440 | Free Wine wrapper (successor to Wineskin/Kegworks). D3DMetal/DXVK/DXMT toggles. ARM + Intel builds. | Active (Mar 2026) |
| [MythicApp/Mythic](https://github.com/MythicApp/Mythic) | 1,229 | SwiftUI game launcher with custom GPTK engine. Steam/Epic/GOG. | Active (Apr 2026) |
| [Heroic Games Launcher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) | 11,112 | Cross-platform launcher for GOG/Amazon/Epic. Native DXMT support on Mac (v2.19.1+). | Active (Apr 2026) |
| [The Wineskin Project](https://github.com/The-Wineskin-Project/WineskinServer) | 2,464 | Classic Wineskin .app wrapper. Still maintained. | Active (Mar 2026) |
| [installaware/AGPT](https://github.com/installaware/AGPT) | 526 | GUI installer for Apple's Game Porting Toolkit | Active (Mar 2026) |
| [WinSteamOnMac](https://github.com/domschl/WinSteamOnMac) | 208 | Guides and tools for Windows Steam on macOS with GPTK | Active (Mar 2026) |
| [Harbor](https://github.com/ohaiibuzzle/Harbor) | 129 | macOS GPTK game porting GUI | Active (Mar 2026) |
| [Orion](https://github.com/andrewmd5/orion) | 134 | CLI game launcher for GPTK | Last update Nov 2025 |
| [macos-wine-steam](https://github.com/ByMedion/macos-wine-steam) | 80 | One-click Wine+DXMT Steam on Mac | Active (Mar 2026) |
| [ybmeng/moonshine](https://github.com/ybmeng/moonshine) | 8 | Maintained Whisky fork with one-step DMG install. Auto-downloads Wine Staging 11.2. | Active (Apr 2026) |
| [bomberfish/Converge](https://github.com/bomberfish/Converge) | 2 | Lightweight SwiftUI Wine wrapper | Last update Nov 2025 |

### Game-Specific Projects

| Project | Stars | What |
|---|---|---|
| [marzent/XIV-on-Mac](https://github.com/marzent/XIV-on-Mac) | 378 | Wine wrapper for FFXIV. Better perf than native Mac client. |
| [wmarti/xenia-mac](https://github.com/wmarti/xenia-mac) | 80 | Xbox 360 emulator with **native Metal backend**. Translates Xbox 360 shader microcode -> Metal Shader Converter -> Metal. |
| [SamoZ256/hydra](https://github.com/SamoZ256/hydra) | New | Nintendo Switch emulator, native Metal. Same dev writing Metal backends for Cemu ([PR #1287](https://github.com/cemu-project/Cemu/pull/1287)) and Panda3DS. |
| [natbro/kaon](https://github.com/natbro/kaon) | Small | Tools to launch Windows games in the **macOS Steam client** directly. Hacky "Steam Play for macOS" prototype. |
| [Coulin9/YuanShen_launcher_mac_porting](https://github.com/Coulin9/YuanShen_launcher_mac_porting) | 89 | Genshin Impact/ZZZ PC launcher port to macOS via WineSkin. |
| [Mac Source Ports](https://www.macsourceports.com/) | — | 156+ open-source game source ports compiled as Universal Binaries for macOS (Doom, Quake, Duke 3D, Fallout 1/2, etc.) |
| [nonoche2/the-macOS-game-workaround-repo](https://github.com/nonoche2/the-macOS-game-workaround-repo) | — | Centralized list of every known way to make games run on macOS |

### Diamond-in-the-Rough Projects

| Project | What | Why It Matters |
|---|---|---|
| [Gcenx/macports-wine](https://github.com/Gcenx/macports-wine) | 997 commits, 116 stars | One person maintaining the entire free Wine-on-macOS build infra |
| [Jpkovas/FEX_MacOs](https://github.com/Jpkovas/FEX_MacOs) | 0 stars, 7,368+ commits | FEX-Emu x86 emulator ported to macOS. If viable, solves Rosetta deprecation. |
| [neo773/macgamingdb](https://github.com/neo773/macgamingdb) | 91 stars | Community game compat DB at macgamingdb.app |
| [kiku-jw/peak-crossover-mouse-fix](https://github.com/kiku-jw/peak-crossover-mouse-fix) | 11 stars | Fixes Unity pointer bug blocking many games. Tiny but critical. |
| [EnderIce2/rpc-bridge](https://github.com/EnderIce2/rpc-bridge) | 200 stars | Discord Rich Presence for Wine games |
| [marzent/macOS-wine-bridge](https://github.com/marzent/macOS-wine-bridge) | 1 star | Enables Discord Rich Presence for Wine on macOS specifically |
| [Searchstars/proton-slr-wine-macos](https://github.com/Searchstars/proton-slr-wine-macos) | 5 stars | macOS-adapted Wine 10.0 build overlaying CachyOS's wine-10.0-proton-slr. Very early. |
| [Splendide-Imaginarius/caselinker](https://github.com/Splendide-Imaginarius/caselinker) | 1 star | Simulates case-insensitive FS with symlinks. Solves real Wine-on-APFS issue. |
| [tbraun96/awdl-symphonizer](https://github.com/tbraun96/awdl-symphonizer) | 10 stars | Fixes WiFi gaming lag by syncing AWDL channels. Quality-of-life. |
| [marzent/reshade-on-unix](https://github.com/marzent/reshade-on-unix) | 6 stars | ReShade post-processing injection under Wine on macOS |
| [oliwonders/MetalHUDHelper](https://github.com/oliwonders/MetalHUDHelper) | 13 stars | Menu bar app for toggling Metal Performance HUD globally |
| [caiovicentino/apple-silicon-internals](https://github.com/caiovicentino/apple-silicon-internals) | — | Apple Silicon private API RE toolkit. 55+ models mapped, Metal 4 ML pipeline, 1009 IOReport channels. |
| [hack-different/apple-knowledge](https://github.com/hack-different/apple-knowledge) | — | Machine-readable database of Apple hardware reverse engineering |

### Additional Compatibility Tools

| Project | What | Platform |
|---|---|---|
| [cnc-ddraw](https://gitlab.com/ShizCalev/cnc-ddraw) (GitLab) | DirectDraw reimplementation for classic 2D games. Wine-compatible. | Cross-platform |
| [wine-vulkanizer](https://gitlab.com/es20490446e/wine-vulkanizer) (GitLab) | Automates DXVK/vkd3d-proton installation in Wine prefixes | Cross-platform |
| [DXVK-GPLAsync](https://gitlab.com/Ph42oN/dxvk-gplasync) (164 stars, GitLab) | Async pipeline compilation patch for DXVK. Reduces shader stutter. | Cross-platform |
| [AreWeAntiCheatYet](https://github.com/AreWeAntiCheatYet/AreWeAntiCheatYet) (499 stars) | Crowd-sourced anti-cheat compatibility database | Reference |

### Homebrew & Package Management

| Resource | What |
|---|---|
| [Gcenx/homebrew-wine](https://github.com/Gcenx/homebrew-wine) | Key Homebrew tap for macOS gaming. `brew tap gcenx/wine`. Wine stable/devel/staging with D3DMetal/GPTK support. |
| [Gcenx/macOS_Wine_builds](https://github.com/Gcenx/macOS_Wine_builds) | Official WineHQ macOS packages with GPTK support |
| [Gcenx/wine-on-mac](https://github.com/Gcenx/wine-on-mac) | Comprehensive installation guide |
| apple/homebrew-apple tap | `game-porting-toolkit` formula |
| [MacPorts d3dmetal](https://ports.macports.org/port/d3dmetal/) | D3DMetal via MacPorts |

---

## CrossOver 26: What They Solved (February 2026)

CrossOver 26 represents the current state of the art:

- **Anti-cheat:** nProtect GameGuard, EAC, and BattlEye now work for 20+ AAA titles. CodeWeavers calls this "curing artificial incompatibility." (Kernel-level anti-cheat like Vanguard remains blocked.)
- **Components:** Wine 11.0, D3DMetal 3.0, DXMT v0.72, vkd3d 1.18, Wine Mono 10.4.1, NTSync
- **DLSS -> MetalFX:** Intercepts NVIDIA DLSS/DLSS-FG calls and translates to MetalFX upscaling + frame interpolation
- **Auto-backend selection:** Automatically picks WineD3D, DXVK, DXMT, or D3DMetal per game
- **Tested titles:** Helldivers 2, Kingdom Come: Deliverance II, God of War Ragnarok, Starfield, Age of Empires IV, FF7 Rebirth

### What CrossOver Has That Upstream Wine Doesn't

1. **wine32on64** -- Custom LLVM compiler for 32-bit on 64-bit macOS (requires forked Clang-8 with `cdecl32`/`stdcall32`/`thiscall32`/`fastcall32` attributes)
2. **Patched MoltenVK** -- Fakes unsupported Vulkan extensions
3. **Custom DXVK** -- macOS-specific modifications
4. **D3DMetal integration** -- Apple's proprietary layer
5. **DXMT integration** -- 3Shain's Metal D3D11
6. **MSync** -- Mach semaphore synchronization
7. **Anti-cheat patches** -- Proprietary, not open-source
8. **DLSS via MetalFX** -- Maps NVIDIA calls to Apple's upscaler
9. **FEX integration** (ARM64 Linux preview, Nov 2025) -- x86 emulation via FEX-Emu for non-Rosetta platforms

### Competitors

**GameHub by GameSir/Mist Studio** -- Closed-source, announced February 2026. Unified Mac gaming app integrating GPTK, Proton, and CrossOver. Connects to Steam, Epic, and GOG. Promises one-click install with AI upscaling. Expected full 1.0 release later in 2026.

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

**Notable forks:** [nastys/MoltenVK](https://github.com/nastys/MoltenVK) (84 stars) carries additional custom patches beyond upstream. [marzent/dxvk](https://github.com/marzent/dxvk) (110 stars) is a macOS-compatible DXVK fork originally for XIV-on-Mac.

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
+-- dxvk/                   # Submodule: DXVK fork (with GPLAsync patches)
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

- Integrate DXVK-macOS + MoltenVK for DX9/10/11. Include DXVK-GPLAsync patches.
- Integrate D3DMetal/GPTK for DX11/12.
- Integrate DXMT for DX10/11 (the best DX11 path).
- Build auto-select logic in Rust.
- Integrate [caselinker](https://github.com/Splendide-Imaginarius/caselinker) for case-insensitive FS compatibility.
- Test 10+ games across paths.

**Exit:** D3D11 game via DXMT, D3D12 via D3DMetal, D3D9 via DXVK, auto-selected.

### Phase 2: Sync Pipeline (Weeks 11-18)

- Proton repo monitor via `git2`.
- Commit classifier with transferability scoring.
- Game config importer (parse `default_compat_config()`).
- Patch adapter with conflict detection.
- Kernel-mapping layer (MSync integration, macOS equivalents).
- Track and cherry-pick relevant Wine MRs (!6755 Metal renderer, !7938 IOSurface optimization).
- Nightly CI: auto-sync, build, smoke test.

**Exit:** New Proton Wine API fix auto-applied within 24 hours.

### Phase 3: Community & Polish (Weeks 19-26)

- Community compatibility reporting (opt-in).
- Game library UI with compat status (integrate [macgamingdb](https://github.com/neo773/macgamingdb) data).
- Shader cache sharing.
- Proton-GE-style community patch integration.
- Performance profiling (Metal HUD, frame timing). Bundle [MetalHUDHelper](https://github.com/oliwonders/MetalHUDHelper) or equivalent.
- Protonfixes integration (354+ game scripts).
- Discord Rich Presence via [rpc-bridge](https://github.com/EnderIce2/rpc-bridge) / [macOS-wine-bridge](https://github.com/marzent/macOS-wine-bridge).
- [ReShade support](https://github.com/marzent/reshade-on-unix) for post-processing.
- WiFi gaming optimization (document [awdl-symphonizer](https://github.com/tbraun96/awdl-symphonizer)).

**Exit:** Public beta. 50+ games tested.

### Phase 4: Post-Rosetta Preparation (Ongoing)

- Track FEX-Emu macOS port progress.
- Test Wine ARM64EC with ARM Windows binaries.
- Monitor Apple's Rosetta deprecation timeline and retained subset scope.
- Investigate custom x86 translation integration points.
- Evaluate libkrun + virtio-gpu Venus as alternative GPU path.

**Exit:** Viable x86 translation strategy for macOS 28+.

---

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| MoltenVK extension gaps | High | KosmicKrisp as alternative; DXMT for DX11; D3DMetal for DX12 |
| Rosetta 2 deprecation (macOS 28) | Critical | Track FEX_MacOs, Wine ARM64EC, Apple's retained subset, Valve's FEX+Proton work |
| Apple breaks GPTK in macOS update | High | Pin known-good versions; test on betas early |
| Wine fork diverges from macOS-buildable | Medium | Maintain rebase branch; don't track Proton HEAD blindly |
| Anti-cheat blocks macOS | High | Out of scope for v1; CrossOver 26 made progress (EAC, BattlEye, GameGuard) |
| 16K page size issues | Medium | Wine 11 simulates 4K pages; test thoroughly |
| Code signing / JIT restrictions | Medium | Proper entitlements; `MAP_JIT` + `pthread_jit_write_protect_np` |
| Metal resource binding ceiling (~500K vs DX12's 1M SRVs) | Medium | Metal 4 bindless rendering may help; D3DMetal handles this internally |
| Gcenx single-point-of-failure | Medium | Contribute to and mirror Gcenx's build infrastructure |
| KosmicKrisp requires macOS 26+ | Medium | MoltenVK remains fallback for older macOS versions |
| GameHub closed-source competitor | Low | Open-source community advantage; move faster on bleeding-edge |

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
6. **Shader pipeline** -- Explore SPIR-V -> AIR direct compilation using LLAIR and metal-air-docs.
7. **Wine MR testing** -- Test and provide feedback on macOS-relevant Wine merge requests.

---

## License

LGPL-2.1 (matching Wine's license).

---

## Acknowledgments

Cauldron builds on the work of many projects and individuals:

- **Whisky** (archived) -- The SwiftUI Wine wrapper that started it all
- **Gcenx** -- Maintaining the entire free Wine-on-macOS ecosystem single-handedly
- **3Shain** -- DXMT, the most impactful solo contribution to macOS gaming
- **marzent** -- MSync, XIV-on-Mac, reshade-on-unix, macOS-wine-bridge
- **GloriousEggroll** -- Proton-GE and the community patching model we're adapting
- **CodeWeavers** -- Two thirds of Wine commits and the CrossOver ecosystem
- **LunarG** -- KosmicKrisp, potentially the biggest unlock for macOS gaming
- **Alyssa Rosenzweig** -- Honeykrisp, proving Apple GPUs can run conformant Vulkan 1.4
- **Dougall Johnson** -- Apple G13 GPU ISA reverse engineering
- **SamoZ256** -- Metal AIR documentation, hydra, Cemu Metal backend
- **Apple** -- D3DMetal, MetalFX, Metal Shader Converter, and (reluctantly) Rosetta 2
- **Valve** -- Proton, DXVK, and open-sourcing MoltenVK
- **The Frogging Family** -- wine-tkg and community patches
- **Isaac Marovitz** -- Whisky's creator, now contributing Wine Metal renderer MR
- **Marc-Aurel Zent** -- Wine IOSurface optimization MR
- **frankea, cyyever, SkuldNorniern** -- Whisky fork maintainers pushing forward
- **philipturner** -- Apple GPU microarchitecture documentation
- **italomandara** -- CXPatcher bridging the gap
- **The Asahi Linux team** -- Proving what's possible on Apple Silicon

---

*Cauldron is not affiliated with Valve, Apple, CodeWeavers, or any of the projects listed above.*
