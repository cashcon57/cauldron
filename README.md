# Cauldron

Bleeding-edge Windows game compatibility for macOS. Wine 11.6 fork with 131 patches from 9 sources.

**[$30 — Buy](https://cauldron.app/buy)** | [Download](https://cauldron.app/download)

---

## What is this

Cauldron is a macOS app that runs Windows games. It's like CrossOver, but:

- **Newer Wine base** — Wine 11.6 (dev) vs CrossOver's 9.x/10.x
- **More patches** — 131 patches from wine-staging, Valve/Proton, proton-ge, CrossOver cherry-picks, and our own fixes
- **Per-game intelligence** — Auto-applies optimal settings, DLL overrides, sync primitives, CPU topology, and binary patches per game from a curated database of 110+ titles
- **Multiple graphics backends** — D3DMetal, DXMT, DXVK+MoltenVK, DXVK+KosmicKrisp, VKD3D-Proton
- **Proton compat flag import** — Translates Valve's ~200 per-game flags to macOS equivalents automatically
- **Automatic redistributable installation** — Auto-installs vcrun, d3dcompiler, media codecs per game on first launch
- **Game binary patching** — Reversible GPU check/driver version fixes (pattern + offset modes, 28 games)
- **RosettaX87 integration** — Optional 4-10x x87 FP acceleration for mod loaders and older games
- **Runs CrossOver bottles directly** — no migration, no conversion, same bottles
- **Open source** — LGPL (Wine patches) + source-available (app)

Cauldron complements CrossOver. We recommend buying CrossOver for stability and D3DMetal access, then using Cauldron for bleeding-edge fixes on the same bottles.

## How Cauldron compares

| Feature | CrossOver | Proton (Linux) | Cauldron |
|---------|-----------|----------------|----------|
| Per-game runtime config | Graphics backend only | 354+ protonfixes scripts | DB-driven auto-config + protonfixes |
| Proton compat flags | None | ~20 flags, ~200 app IDs | All flags translated to macOS |
| CPU topology limiting | None | Env var per game | Auto from DB (Far Cry, DoW2, etc.) |
| Auto-install redistributables | CrossTie install-time only | Per-game at runtime | Per-game at first launch |
| Binary game patching | None | None | 28 games (4 verified, pattern + offset) |
| Media codecs | Manual install | GStreamer integration | Auto-install quartz/lavfilters/wmp |
| Sync primitives | msync | fsync/NTsync | msync + per-game disable |
| Game database | Proprietary ratings | ProtonDB (community) | Open DB, 110+ games, NexusMods-ranked |
| Audio latency per-game | None | None | STAGING_AUDIO_PERIOD auto-set |
| Launcher bypasses | None | Exe redirects | Auto from DB (Borderlands, Bethesda, etc.) |
| Windows version per-game | Manual per-bottle | Per-game in protonfixes | Auto from DB (AoE3→WinXP, etc.) |

## Architecture

```
SwiftUI (macOS 26+)  →  Rust core (FFI)  →  Cauldron Wine 11.6  →  Metal
```

| Layer | Lang | What |
|-------|------|------|
| UI | Swift/SwiftUI | Bottles, game library, per-game settings, patch triage, profile system |
| Engine | Rust | Wine process management, launch config resolver, game binary patching, dependency tracking, RosettaX87 |
| Sync | Rust | Proton/CrossOver/staging patch monitoring, classification, auto-adaptation, protonfixes import, config importer |
| Database | Rust/SQLite | Game compatibility DB, recommended settings, binary patches, dependency tracking, Proton commit log |
| Runtime | C | Wine fork, DXVK-macOS, MoltenVK, DXMT, D3DMetal |

### Launch flow

```
User clicks Play
  → Swift UI calls cauldron_launch_exe (FFI bridge)
  → detect_steam_app_id() from exe path
  → launch_config_resolver::resolve() merges 3 layers:
      Layer 1: DB game_recommended_settings (per-game optimal config)
      Layer 2: Protonfixes actions (env vars, DLL overrides, launch args)
      Layer 3: User per-game overrides from UI (highest priority)
  → config.apply_to_env() sets:
      WINEMSYNC/WINEESYNC, WINE_CPU_TOPOLOGY, WINEDLLOVERRIDES,
      STAGING_AUDIO_PERIOD, WINE_LARGE_ADDRESS_AWARE, DXVK_CUSTOM_VENDOR_ID, etc.
  → apply exe_replacement if set (launcher bypasses)
  → write windows_version to Wine registry if set
  → auto-install required_dependencies if missing
  → apply binary patches if auto_apply_patches=true
  → spawn Wine process
```

## Features

### Per-Game Intelligence Engine

The launch config resolver automatically applies optimal settings when a game is detected. 110+ games are seeded with configurations covering:

- **Sync primitive control** — Disable msync/esync for games that crash with them (BioShock, Yakuza, Total War, Supreme Commander, Borderlands)
- **CPU topology limiting** — `WINE_CPU_TOPOLOGY` for games that crash with high core counts (Far Cry series→16 cores, Dawn of War II→8, The Forest→4, Little Nightmares→1)
- **Windows version override** — Per-game Wine version (Age of Empires 3→WinXP, Dark Souls PTDE→Win7)
- **Vendor ID spoofing** — `DXVK_CUSTOM_VENDOR_ID=10de` to bypass GPU vendor checks without binary patching (HITMAN 3)
- **DLL overrides** — Per-game native/builtin DLL selection
- **Environment variables** — Arbitrary per-game env vars (DXVK_ASYNC, WINE_HEAP_DELAY_FREE, etc.)
- **Launch arguments** — Per-game command-line args (-dx11, -fullscreen, --skip-version-check)
- **Audio latency** — Per-game STAGING_AUDIO_PERIOD for games with crackling audio (FF IX, Evil Within, Gothic)
- **Launcher bypasses** — Auto-substitute broken launchers with actual game exe (Borderlands 2, Conan Exiles, Evil Genius 2)
- **Required dependencies** — Auto-install vcrun, d3dcompiler, media codecs on first launch

User per-game overrides always take priority. If a user has configured a game manually, DB settings won't override them.

### Proton Compat Config Import

Imports Valve's `default_compat_config()` (~200 app IDs) and translates all flags to macOS equivalents:

| Proton Flag | macOS Translation |
|---|---|
| `gamedrive` | Wine drive letter symlink to game directory |
| `heapdelayfree` | `WINE_HEAP_DELAY_FREE=1` |
| `heapzeromemory` | `WINE_HEAP_ZERO_MEMORY=1` |
| `nofsync`/`noesync` | Disable msync/esync in game_recommended_settings |
| `forcelgadd` | `WINE_LARGE_ADDRESS_AWARE=1` |
| `hidenvgpu` | `WINE_HIDE_NVIDIA_GPU=1` |
| `disablenvapi` | `DXVK_ENABLE_NVAPI=0` + nvapi DLL overrides |
| `nomfdxgiman` | `WINE_DISABLE_MF_DXGI_MANAGER=1` |
| `cmdlineappend:arg` | Appended to game launch args |
| `noopwr` | Set for compat (primarily Wayland-specific) |
| `xalia` | Skipped (X11/Wayland-only) |

Imported configs are written into `game_recommended_settings` so the launch config resolver applies them automatically. Conditional updates never overwrite user customizations.

### Game Binary Patching

Reversible binary patches for games with GPU capability checks, driver version checks, or DirectX feature checks that fail under Wine/Metal. Two modes:

- **Pattern mode** — Hex pattern search with `??` wildcards, works across game versions
- **Offset mode** — Fixed byte offset writes with SHA-256 hash verification, for .NET DLLs and version-specific patches

28 games have built-in patches (4 verified against actual binaries, 24 based on common engine patterns). Inspired by [cbusillo/macos-game-patches](https://github.com/cbusillo/macos-game-patches) and [timkurvers/macos-game-patches](https://github.com/timkurvers/macos-game-patches). Patches are also stored in SQLite for OTA updates without app rebuilds.

### Optimization Profiles

Three global profiles configure all settings at once:

| | Stable | Preview | Bleeding Edge |
|---|---|---|---|
| RosettaX87 | Off | On | On |
| MetalFX Upscaling | Off | On | On |
| DXR Ray Tracing | Off | Off | On |
| Auto-Apply Game Patches | Off | Off | On |
| Nightly Patches | Hidden | Hidden | Shown |
| Sync Interval | 24h | 6h | 1h |
| Performance Monitoring | Off | Off | On |

Per-game overrides can customize any setting. The UI warns when a game's settings diverge from the active profile.

### Graphics API Auto-Detection

Games are scanned via PE import table analysis to detect all linked graphics APIs (DX8-12, Vulkan, OpenGL). Each detected API is shown as a color-coded badge in the game library. The auto-select backend logic uses the detected API to choose the optimal translation path.

### RosettaX87

Optional integration with [WineAndAqua/rosettax87](https://github.com/WineAndAqua/rosettax87) for 4-10x faster x87 floating-point operations via patched Rosetta. Benefits mod loaders (SKSE, F4SE, NVSE) and older DX9 games.

### Dependency Auto-Installation

19 winetricks verbs available for auto-installation:

| Category | Verbs |
|----------|-------|
| C++ Runtimes | vcrun2022, vcrun2019, vcrun2017 |
| .NET | dotnet48, dotnet40 |
| DirectX | d3dx9, d3dcompiler_47, dxvk |
| Media Codecs | quartz, lavfilters, wmp9, wmp11, wmv9vcm, devenum, amstream |
| Fonts | corefonts |
| Audio | xact, faudio |

Dependencies are tracked per-bottle per-game in SQLite to prevent re-installation.

### Extended Protonfixes

Parses 354+ umu-protonfixes Python scripts and supports 11 action types:

`InstallVerb`, `AppendArgument`, `ReplaceCommand`, `SetEnvVar`, `DllOverride`, `DisableNvapi`, `CreateFile`, `RenameFile`, `DeleteFile`, `CopyFile`, `SetRegistry`

## Wine Fork

`patches/` contains our Wine patch series. 131 patches on Wine 11.6, zero conflicts:

| Source | Patches | What |
|--------|---------|------|
| wine-staging | 58 | macOS flicker fix, wined3d, ntdll perf, D3DX9 stubs |
| Valve/Proton | 24 | API stubs, GPU detection, audio, media framework |
| openglfreak | 18 | QPC performance counters, TLS/crypto, spec fixes |
| arm64ec | 12 | dwmapi stubs, jscript, keyboard locale |
| Wine GitLab MRs | 7 | Mach COW write watches, surface optimization, GPU ID |
| proton-ge | 5 | D2D crash fix, ntoskrnl stubs |
| wine-tkg | 4 | CSMT toggle, Steam integration |
| CrossOver | 2 | Apple Silicon display mode, mach_continuous_time |
| Cauldron | 1 | VirtualProtect COW fix (SKSE/mod loader compat) |

See [`patches/PATCH_AUDIT.md`](patches/PATCH_AUDIT.md) for the full audit.

## Building from source

```
make build
make swift-build
make wine-init
make wine-build
```

Requires: Rust stable, Swift 6.2+, macOS 26+, Xcode CLI tools. Wine build additionally needs `brew install bison flex mingw-w64 gettext pkg-config gnutls freetype`.

Self-builds are fully functional with no restrictions. Auto-updates are disabled on self-builds because we can't push signed updates to unsigned binaries.

## Project structure

```
cauldron-core/               Rust — Core engine
  src/
    launch_config_resolver   Per-game config resolution (3-layer merge)
    game_patches             Binary patching (pattern + offset modes)
    game_scanner             PE import analysis, Steam manifest parsing
    dependency_installer     Winetricks verb runner (19 verbs)
    dependency_tracker       Per-bottle dependency tracking
    wine                     Wine process management
    graphics                 Backend selection, env var building
    rosettax87               RosettaX87 detection and integration
    bottle                   Bottle lifecycle management
    registry                 Wine registry read/write
    icon_processor           .exe icon extraction → .icns
    shader_cache             Per-backend shader cache management
    runtime_downloader       DXVK, DXMT, MoltenVK, vkd3d-proton downloads

cauldron-sync/               Rust — Proton sync pipeline
  src/
    config_importer          Proton compat config parser + macOS translator
    protonfixes              umu-protonfixes script parser (11 action types)
    monitor                  Git-based Proton commit polling
    classifier               Commit classification by subsystem
    auto_adapter             Linux→macOS code adaptation (3 tiers)
    applicator               Patch application with conflict detection

cauldron-db/                 Rust — SQLite database layer
  src/
    schema                   9 tables, idempotent migrations
    models                   GameRecord, GameRecommendedSettings, GameBinaryPatchRecord,
                             ProtonCommit, CompatReportRecord
    queries                  CRUD for all tables

cauldron-bridge/             Rust — C FFI layer (40+ exported functions)
cauldron-cli/                Rust — Command-line interface
CauldronApp/                 Swift — SwiftUI macOS app
  Sources/
    Models/                  ConfigProfile, PerGameSettings, AppSettings
    Views/                   BottleDetailView, GameLibraryView, SettingsView, etc.
    Bridge/                  CauldronFFI, CauldronBridge (Swift↔Rust)
    Licensing/               Activation, JWT validation

db/                          SQLite migrations and seed data (110+ games)
patches/                     Wine fork patch series (131 patches)
scripts/                     Build scripts, CI helpers
```

### Database tables

| Table | Purpose |
|-------|---------|
| `games` | 110+ game records: backend, compat status, DX version, popularity rank |
| `game_recommended_settings` | Per-game optimal config: sync, topology, env vars, DLL overrides, deps, registry, audio |
| `game_binary_patches` | Binary patch definitions (pattern + offset modes, verified flag) |
| `game_deps_installed` | Tracks installed dependencies per bottle per game |
| `proton_commits` | Proton repository commits being tracked |
| `proton_game_configs` | Imported Proton compat flags with macOS translations |
| `backend_overrides` | User backend preferences per game |
| `compatibility_reports` | Community compatibility reports |
| `patch_log` | Wine patch application history |
| `sync_status` | Proton sync pipeline state |

## Upstream Sources

Projects Cauldron builds on or tracks:

| Project | Role |
|---------|------|
| [Gcenx/DXVK-macOS](https://github.com/Gcenx/DXVK-macOS) | macOS DXVK fork (our DX9-11 Vulkan backend) |
| [3Shain/dxmt](https://github.com/3Shain/dxmt) | Metal-native DX11 (our DXMT backend) |
| [marzent/wine-msync](https://github.com/marzent/wine-msync) | Mach semaphore sync for Wine |
| [WineAndAqua/rosettax87](https://github.com/WineAndAqua/rosettax87) | x87 FP acceleration for Rosetta |
| [cbusillo/macos-game-patches](https://github.com/cbusillo/macos-game-patches) | Game binary patch patterns |
| [timkurvers/macos-game-patches](https://github.com/timkurvers/macos-game-patches) | Offset-based game patches |
| [Open-Wine-Components/umu-protonfixes](https://github.com/Open-Wine-Components/umu-protonfixes) | Per-game fix scripts (354+ games) |
| [Open-Wine-Components/umu-database](https://github.com/Open-Wine-Components/umu-database) | Title-to-UMU-ID lookup table |
| [KhronosGroup/MoltenVK](https://github.com/KhronosGroup/MoltenVK) | Vulkan on Metal |
| KosmicKrisp (Mesa) | Vulkan 1.3 on Metal 4 (future) |

## License

Wine patches: LGPL-2.1 (required by Wine's license).
Application code: Source-available. Official builds are $30.
