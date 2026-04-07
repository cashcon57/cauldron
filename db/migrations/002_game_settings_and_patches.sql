-- Migration 002: Game recommended settings and binary patches tables
-- Adds per-game optimal settings and a database-driven binary patch registry.

-- Recommended per-game settings (overrides for optimal play)
CREATE TABLE IF NOT EXISTS game_recommended_settings (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    steam_app_id        INTEGER NOT NULL,
    -- Graphics
    graphics_backend    TEXT,                    -- preferred backend override
    -- Sync primitives
    msync_enabled       INTEGER DEFAULT 1,       -- 0/1
    esync_enabled       INTEGER DEFAULT 1,       -- 0/1
    -- Translation features
    rosetta_x87         INTEGER DEFAULT 0,       -- 0/1 (x87 FPU emulation for 32-bit games)
    async_shader        INTEGER DEFAULT 1,       -- 0/1
    metalfx_upscaling   INTEGER DEFAULT 0,       -- 0/1
    dxr_ray_tracing     INTEGER DEFAULT 0,       -- 0/1
    moltenvk_arg_bufs   INTEGER DEFAULT 0,       -- 0/1
    fsr_enabled         INTEGER DEFAULT 0,       -- 0/1
    large_address_aware INTEGER DEFAULT 0,       -- 0/1
    -- Wine DLL overrides (JSON object: {"d3d11": "native", ...})
    wine_dll_overrides  TEXT NOT NULL DEFAULT '{}',
    -- Environment variables (JSON object: {"DXVK_ASYNC": "1", ...})
    env_vars            TEXT NOT NULL DEFAULT '{}',
    -- Wine Windows version override (e.g., "win10", "win7")
    windows_version     TEXT,
    -- Launch arguments appended to game exe
    launch_args         TEXT NOT NULL DEFAULT '',
    -- Whether to auto-apply binary patches for this game
    auto_apply_patches  INTEGER DEFAULT 1,
    -- Human-readable notes about why these settings
    settings_notes      TEXT NOT NULL DEFAULT '',
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    -- steam_app_id is a logical link to games table (not strict FK due to composite PK)
);

-- Binary patch definitions stored in DB (supplements built-in Rust patches)
CREATE TABLE IF NOT EXISTS game_binary_patches (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    steam_app_id    INTEGER NOT NULL,
    title           TEXT NOT NULL,
    exe_name        TEXT NOT NULL,              -- target executable filename
    description     TEXT NOT NULL,              -- what this patch fixes
    category        TEXT NOT NULL DEFAULT 'Other', -- GpuCapabilityCheck, DriverVersionCheck, DxFeatureCheck, AntiCheatBypass, MemoryFix, CrashFix, PerformanceFix, Other
    -- Hex-encoded patterns (with ?? wildcards)
    pattern         TEXT NOT NULL,
    replacement     TEXT NOT NULL,
    -- Versioning: which exe versions this patch applies to
    min_exe_size    INTEGER,                   -- optional: minimum exe size (bytes) for version targeting
    max_exe_size    INTEGER,                   -- optional: maximum exe size (bytes) for version targeting
    exe_hash        TEXT,                       -- optional: specific exe SHA-256 hash this targets
    -- Metadata
    enabled         INTEGER NOT NULL DEFAULT 1, -- 0/1: allows disabling patches without removing
    verified        INTEGER NOT NULL DEFAULT 0, -- 0/1: has this pattern been tested against an actual game binary
    priority        INTEGER NOT NULL DEFAULT 0, -- higher = apply first
    source          TEXT NOT NULL DEFAULT 'cauldron', -- where this patch originated
    notes           TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    -- steam_app_id is a logical link to games table (not strict FK due to composite PK)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_game_settings_app_id ON game_recommended_settings(steam_app_id);
CREATE INDEX IF NOT EXISTS idx_game_patches_app_id ON game_binary_patches(steam_app_id);
CREATE INDEX IF NOT EXISTS idx_game_patches_exe_name ON game_binary_patches(exe_name);
CREATE INDEX IF NOT EXISTS idx_game_patches_enabled ON game_binary_patches(enabled);

-- Add popularity_rank and dx_version columns to games table
-- popularity_rank: lower number = more popular (based on concurrent players / sales data)
-- dx_version: primary DirectX version used (9, 10, 11, 12)
ALTER TABLE games ADD COLUMN popularity_rank INTEGER;
ALTER TABLE games ADD COLUMN dx_version INTEGER;
ALTER TABLE games ADD COLUMN genre TEXT;
