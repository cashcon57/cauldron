# Cauldron

Windows-to-macOS game compatibility layer. Custom Wine fork, Rust core, SwiftUI frontend, DXMT/D3DMetal/DXVK graphics backends. Designed as an open-source spiritual successor to CrossOver.

> **Status:** Active development. Alpha. Individual games work (FO4 at 150fps, Skyrim SE with SKSE, Hogwarts Legacy with D3DMetal). UI is functional. Wine fork builds. Some systems (msync rewrite, GFXT adapter, Proton sync) are partial.

---

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│ SwiftUI frontend (CauldronApp/)                         │
│   Bottles · Game library · Settings · Patch triage      │
└────────────────────────┬────────────────────────────────┘
                         │ C FFI (40+ exported fns)
┌────────────────────────▼────────────────────────────────┐
│ Rust core                                               │
│   cauldron-bridge/  — FFI surface, launch orchestration │
│   cauldron-core/    — Wine/bottle mgmt, launch resolver │
│   cauldron-sync/    — Proton upstream sync pipeline     │
│   cauldron-db/      — SQLite schema + queries           │
│   cauldron-cli/     — CLI surface for the same engine   │
└────────────────────────┬────────────────────────────────┘
                         │ spawns wine process
┌────────────────────────▼────────────────────────────────┐
│ Cauldron Wine fork  (wine/, patched from Wine 11.6)     │
│   + DXMT (D3D11→Metal)                                  │
│   + D3DMetal (closed, bundled from CrossOver runtime)   │
│   + DXVK-macOS (DX9–11→Vulkan→MoltenVK)                 │
│   + vkd3d-proton (DX12→Vulkan, experimental)            │
│   + winemetal.dll (Cauldron Metal bridge stub)          │
└─────────────────────────────────────────────────────────┘
```

### Crate layout

| Crate | Purpose | Notable modules |
| --- | --- | --- |
| [`cauldron-bridge`](cauldron-bridge/src/lib.rs) | `#[no_mangle]` FFI surface called from Swift | `cauldron_launch_exe`, `detect_steam_app_id` (ACF parser), registry entry application, exe_override wiring |
| [`cauldron-core`](cauldron-core/src/) | Engine: Wine mgmt, bottle lifecycle, launch orchestration | [`launch_config_resolver`](cauldron-core/src/launch_config_resolver.rs), [`registry`](cauldron-core/src/registry.rs), [`graphics`](cauldron-core/src/graphics.rs), [`game_scanner`](cauldron-core/src/game_scanner.rs), [`game_patches`](cauldron-core/src/game_patches.rs), [`rosettax87`](cauldron-core/src/rosettax87.rs), [`wine_builder`](cauldron-core/src/wine_builder.rs) |
| [`cauldron-sync`](cauldron-sync/src/) | Proton/Wine upstream sync pipeline | [`monitor`](cauldron-sync/src/monitor.rs) (git polling), [`classifier`](cauldron-sync/src/classifier.rs), [`auto_adapter`](cauldron-sync/src/auto_adapter.rs), [`config_importer`](cauldron-sync/src/config_importer.rs), [`protonfixes`](cauldron-sync/src/protonfixes.rs) |
| [`cauldron-db`](cauldron-db/src/) | SQLite — schema, models, queries | 10 tables, idempotent migrations |
| [`cauldron-cli`](cauldron-cli/src/) | CLI wrapper around `cauldron-core` | — |
| [`CauldronApp`](CauldronApp/Sources/CauldronApp/) | SwiftUI macOS app (macOS 26+) | [`Bridge/`](CauldronApp/Sources/CauldronApp/Bridge/) (Swift↔Rust), [`Views/`](CauldronApp/Sources/CauldronApp/Views/), [`Models/`](CauldronApp/Sources/CauldronApp/Models/) |

---

## Launch flow

The core of the app. When the user clicks Play, everything in the launch pipeline runs from [`cauldron_launch_exe`](cauldron-bridge/src/lib.rs) in the bridge crate:

1. **Detect `steam_app_id`** from the exe path. Reads `steamapps/appmanifest_<id>.acf` and matches `installdir` against the game directory name. Falls back to a title-word heuristic for non-Steam layouts.
2. **Resolve launch config** via [`launch_config_resolver::resolve()`](cauldron-core/src/launch_config_resolver.rs) — merges three layers in priority order:
   - Layer 1: `games.wine_overrides` JSON (legacy seed data)
   - Layer 2: `game_recommended_settings` table (structured per-game config)
   - Layer 3: `UserLaunchOverrides` from the UI (highest priority)
3. **Apply env vars** — `WINEMSYNC`, `WINEESYNC`, `WINE_CPU_TOPOLOGY`, `STAGING_AUDIO_PERIOD`, `WINE_LARGE_ADDRESS_AWARE`, `DXVK_CUSTOM_VENDOR_ID`, graphics backend vars, DLL overrides into `WINEDLLOVERRIDES`
4. **Apply `exe_replacement`** — swap the user-launched exe for a different one (e.g. `SkyrimSELauncher.exe` → `skse64_loader.exe`, `Borderlands2.exe` → `Binaries/Win32/Borderlands2.exe`)
5. **Write `windows_version`** — per-game Wine version via a dropped `.cauldron_winver.reg` file
6. **Apply `registry_entries`** — arbitrary per-game registry keys (e.g. macdrv `RetinaMode`, per-app `Mac Driver` options, compatibility flags). Typed via `RegistryHive` + `RegValueType`. Uses [`registry::set_value`](cauldron-core/src/registry.rs).
7. **Stage DXMT/DXVK DLLs** — copies `d3d11.dll`, `d3d10core.dll`, `dxgi.dll`, `winemetal.dll` into the game directory; sets per-app native/builtin DLL overrides in `user.reg`
8. **Protect `steamwebhelper.exe`** — forces it back to builtin d3d11 so Steam's CEF process doesn't try to load DXMT
9. **HiDPI mode** — if enabled (global toggle or per-game DB flag), writes `HKCU\Software\Wine\Mac Driver\RetinaMode=y` so Wine's macdrv reports physical pixels instead of logical points on Retina displays
10. **Spawn Wine** — finds the Wine binary (prefers Cauldron's built binary at `~/Library/Cauldron/wine/bin/wine64`, falls back to `~/Library/Cauldron/wine/`, `/usr/local/bin`, `/opt/homebrew/bin`, and system Wine app bundles)

The Swift side passes settings to Rust as a JSON blob through the `backend` parameter — see [`LaunchSettings`](CauldronApp/Sources/CauldronApp/Bridge/CauldronBridge.swift) and its Rust parser in `cauldron_launch_exe`.

---

## Graphics backends

| Backend | Translates | Underlying | Status |
| --- | --- | --- | --- |
| DXMT | D3D11 | Metal (native) | Primary — works for most DX11 titles |
| D3DMetal | D3D11/12 | Metal (native) | Bundled from CrossOver's closed runtime; higher compat for AAA DX12 |
| DXVK-macOS | D3D9/10/11 | Vulkan → MoltenVK → Metal | Fallback for games DXMT can't handle |
| DXVK+KosmicKrisp | D3D9/10/11 | Vulkan 1.3 → Metal 4 (Mesa) | Experimental, tracks KosmicKrisp |
| vkd3d-proton | D3D12 | Vulkan → MoltenVK | Experimental, for DX12 via MoltenVK path |

Backend selection is automatic based on the game's PE import table ([`game_scanner`](cauldron-core/src/game_scanner.rs)) — badges in the UI show every detected graphics API. The auto-select logic picks the preferred backend per detected API; users can override globally or per-game.

DXMT and DXVK DLLs are staged into the game directory before launch. Per-app `HKCU\Software\Wine\AppDefaults\<exe>\DllOverrides` keys force them to load as `native,builtin`. `steamwebhelper.exe` gets the opposite override (`builtin`) to keep Steam's CEF sandbox happy.

---

## Wine fork

The Wine source lives at `wine/` (initialized via [`scripts/init_wine_fork.sh`](scripts/init_wine_fork.sh)), based on upstream Wine 11.6. Patches live in [`patches/`](patches/):

- [`patches/cauldron/`](patches/cauldron/) — Cauldron's own patches
  - `0001-ntdll-Preserve-private-pages-on-VirtualProtect.patch` — COW preservation fix; required for SKSE / F4SE / mod loaders that `VirtualProtect` their own code pages
  - `0003-winemac-drv-reduce-compositor-flicker.patch` — macdrv flicker reduction
  - `0004-ntdll-prefer-native-dlls-from-app-directory.patch` — changes Wine's loader search order so DXMT DLLs staged next to the game exe always win
- [`patches/rosetta/`](patches/rosetta/) — CrossOver hack series adapted for Rosetta
- [`patches/PATCH_AUDIT.md`](patches/PATCH_AUDIT.md) — per-patch provenance, conflict status, stability notes

Build with `make wine-build`. The fork targets `arch -x86_64` Rosetta builds (Apple Silicon native builds are tracked but not primary). See [`scripts/build_wine.sh`](scripts/build_wine.sh).

**Always compile-check C changes with `arch -x86_64 gcc` before starting a full Wine build** — a full build takes ~20 minutes, a broken patch wastes all of it.

---

## Per-game intelligence

`game_recommended_settings` stores structured config per `steam_app_id`. The resolver reads it at launch and applies everything automatically. Current seed data covers ~45 games with explicit settings; the wider game DB (`games` table) covers ~30 titles with backend/compat metadata.

Examples of what the resolver actually applies:

```sql
-- Skyrim SE: use SKSE loader, fix macdrv cursor trailing
INSERT INTO game_recommended_settings (steam_app_id, exe_override, windows_version, registry_entries, ...)
VALUES (489830, 'skse64_loader.exe', 'win10', '[{"hive":"HKCU","key":"Software\\\\Wine\\\\AppDefaults\\\\SkyrimSE.exe\\\\Mac Driver","name":"RetinaMode","reg_type":"REG_SZ","data":"n"}]', ...);

-- Far Cry series: limit CPU topology (engine crashes with high core counts)
INSERT INTO game_recommended_settings (steam_app_id, cpu_topology) VALUES (19900, '16:1');  -- Far Cry 2

-- Age of Empires 3: force WinXP mode
INSERT INTO game_recommended_settings (steam_app_id, windows_version) VALUES (105450, 'winxp');

-- HITMAN 3: spoof NVIDIA vendor ID + skip version check
INSERT INTO game_recommended_settings (steam_app_id, env_vars, launch_args)
VALUES (1659040, '{"DXVK_CUSTOM_VENDOR_ID": "10de"}', '--skip-version-check');

-- Borderlands 2: bypass broken launcher
UPDATE game_recommended_settings SET exe_override = 'Binaries/Win32/Borderlands2.exe' WHERE steam_app_id = 49520;
```

Structured fields in `game_recommended_settings`:

| Field | Purpose |
| --- | --- |
| `msync_enabled`, `esync_enabled` | Force-disable sync primitives for games that crash with them |
| `cpu_topology` | `WINE_CPU_TOPOLOGY` — limit exposed cores/threads |
| `windows_version` | Wine Windows version (winxp/win7/win10/win11) |
| `env_vars` | Arbitrary env vars as JSON |
| `wine_dll_overrides` | Per-game DLL overrides as JSON |
| `launch_args` | Extra args appended to the game command line |
| `required_dependencies` | JSON array of winetricks verbs to install on first launch |
| `exe_override` | Path relative to the game dir — replaces the launched exe (launcher bypasses, SKSE, Vulkan renderers) |
| `registry_entries` | JSON array of `{hive, key, name, reg_type, data}` objects applied before launch |
| `audio_latency_ms` | `STAGING_AUDIO_PERIOD` for games with crackling audio |
| `hidpi_mode` | Per-game override for Wine RetinaMode |
| `rosetta_x87` | Force-enable RosettaX87 for mod loaders |

User per-game overrides (`UserLaunchOverrides`) layer on top and always win.

---

## Database schema

10 tables in [`cauldron-db/src/schema.rs`](cauldron-db/src/schema.rs), with idempotent `ALTER TABLE … ADD COLUMN` migrations for forward compat:

| Table | Purpose |
| --- | --- |
| `games` | Game records: title, backend, compat status, DX version, popularity rank, known issues |
| `game_recommended_settings` | Structured per-game launch config (fields above) |
| `game_binary_patches` | Binary patch definitions (pattern + offset modes, SHA-256 verified) |
| `game_deps_installed` | Per-bottle per-game dependency tracking (prevents re-install) |
| `proton_commits` | Proton/Wine upstream commits being tracked |
| `proton_game_configs` | Imported Proton compat flags with macOS translations |
| `backend_overrides` | Per-game user backend preference |
| `compatibility_reports` | Community compat reports |
| `patch_log` | Wine patch application history |
| `sync_status` | Sync pipeline state |

---

## Building

```bash
make build          # cargo build --workspace
make swift-build    # build CauldronApp
make wine-init      # clone Wine fork + apply patches
make wine-build     # full Wine build (~20 min on M3 Max)
make               # default target
```

**Requirements:**

- Rust stable (check `rust-toolchain.toml` if present)
- Swift 6.2+ / Xcode 26+
- macOS 26+ (deployment target)
- Wine build deps: `brew install bison flex mingw-w64 gettext pkg-config gnutls freetype`

The workspace compiles clean (`cargo check --workspace`). Two existing warnings are dead_code for in-progress UI wiring — ignore.

**Running the Swift app from CLI:**

```bash
swift run --package-path CauldronApp
```

**Running the CLI:**

```bash
cargo run -p cauldron-cli -- <command>
```

---

## Reverse engineering

Some of Cauldron's interop work depends on understanding CrossOver's closed runtime (D3DMetal/GFXT). Notes live in [`docs/`](docs/) and (gitignored) `docs/re/`:

- [`docs/CROSSOVER_D3DMETAL_ARCHITECTURE.md`](docs/CROSSOVER_D3DMETAL_ARCHITECTURE.md) — high-level map of D3DMetal binaries, entry points, COM vtables
- [`docs/GFXT_ADAPTER_SPEC.md`](docs/GFXT_ADAPTER_SPEC.md) — GFXT adapter interface spec (derived from Ghidra analysis of CrossOver's `libGFXT.dylib`), PE stubs, `macdrv_functions` dispatch table

Everything in `docs/re/` is excluded from git (`.gitignore`) to keep decompiled artifacts out of the public repo.

---

## Development workflow

The project is built with heavy use of Claude Code and multi-agent worktrees. Concurrent agents implement isolated features (HiDPI mode, Proton importer, patch classifier) on separate branches via `git worktree`, and results are merged back into `main`.

Local CI runs in an isolated `localci` macOS user over SSH — scripts in [`scripts/`](scripts/). This keeps Wine build tests and the main dev environment separate.

---

## Current state

**Working:**

- Bottle management (create, list, delete, discover CrossOver bottles in-place)
- Game library with PE import scanning and auto-backend selection
- Launch config resolver (3-layer merge: DB → recommended settings → user overrides)
- Registry writes before launch (typed hive + value type)
- Exe overrides (SKSE loader, Borderlands binaries, Vulkan renderers)
- Appmanifest-based `steam_app_id` detection
- HiDPI / RetinaMode toggle (global + per-game)
- DXMT integration with staged DLLs + per-app registry overrides
- Skyrim SE launching via SKSE with per-game fixes
- Fallout 4 at 150fps (DXMT)
- Wine fork builds cleanly on Wine 11.6 with the current patch series
- Settings profiles (Stable / Preview / Bleeding Edge) with drift detection

**Partial:**

- msync rewrite — works for simple Wine ops; `steamwebhelper.exe` won't start with it enabled
- Proton sync pipeline — monitor and classifier work; auto-adapter is tier-1 only
- GFXT adapter — spec is recovered, implementation is stub-only
- Game binary patching — 4 verified patches, the rest are pattern-based

**Planned:**

- Full GFXT adapter implementation (replace dependency on CrossOver's closed runtime)
- vkd3d-proton integration for DX12
- KosmicKrisp (Mesa Vulkan 1.3 on Metal 4) as a DXVK backend
- Metal HUD wiring from Swift UI → Rust bridge → env var propagation
- SSEEngineFixesForWine bundling

---

## Upstream projects

| Project | Role |
| --- | --- |
| [Gcenx/DXVK-macOS](https://github.com/Gcenx/DXVK-macOS) | DXVK fork targeting macOS / MoltenVK |
| [3Shain/dxmt](https://github.com/3Shain/dxmt) | D3D11→Metal (native, no Vulkan layer) |
| [marzent/wine-msync](https://github.com/marzent/wine-msync) | Mach semaphore sync primitives |
| [WineAndAqua/rosettax87](https://github.com/WineAndAqua/rosettax87) | Patched Rosetta for faster x87 FP |
| [Open-Wine-Components/umu-protonfixes](https://github.com/Open-Wine-Components/umu-protonfixes) | Per-game fix scripts (354+ games) |
| [Open-Wine-Components/umu-database](https://github.com/Open-Wine-Components/umu-database) | Title → UMU ID lookup |
| [KhronosGroup/MoltenVK](https://github.com/KhronosGroup/MoltenVK) | Vulkan on Metal |
| [cbusillo/macos-game-patches](https://github.com/cbusillo/macos-game-patches) | Binary patch patterns |
| [timkurvers/macos-game-patches](https://github.com/timkurvers/macos-game-patches) | Offset-based game patches |

---

## License

- Wine fork patches: **LGPL-2.1** (required by Wine)
- Rust / Swift application code: see [`LICENSE`](LICENSE)
- Third-party component licenses: [`THIRD_PARTY_LICENSES`](THIRD_PARTY_LICENSES)
