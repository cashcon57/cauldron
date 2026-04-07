-- Cauldron initial database schema
-- This migration is kept as a reference; cauldron-db also creates
-- these tables programmatically via its Rust migration code.

CREATE TABLE IF NOT EXISTS games (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    steam_app_id    INTEGER NOT NULL UNIQUE,
    title           TEXT    NOT NULL,
    backend         TEXT    NOT NULL,  -- e.g. 'D3DMetal', 'DXMT', 'DxvkMoltenVK'
    compat_status   TEXT    NOT NULL,  -- 'Platinum', 'Gold', 'Silver', 'Bronze', 'Borked'
    known_issues    TEXT,              -- free-text description of known problems
    notes           TEXT,              -- additional notes
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proton_commits (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    sha             TEXT    NOT NULL UNIQUE,
    author          TEXT,
    date            TEXT    NOT NULL,
    subject         TEXT    NOT NULL,
    classification  TEXT    NOT NULL,  -- 'High', 'Medium', 'Low'
    files_changed   INTEGER NOT NULL DEFAULT 0,
    reviewed        INTEGER NOT NULL DEFAULT 0,  -- boolean: has a human reviewed this
    notes           TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS backend_overrides (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    steam_app_id    INTEGER NOT NULL,
    backend         TEXT    NOT NULL,  -- which backend to force
    flags           TEXT,              -- space-separated flags, e.g. 'noopwr gamedrive'
    source          TEXT,              -- 'proton_config', 'manual', 'auto'
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (steam_app_id) REFERENCES games(steam_app_id)
);

CREATE TABLE IF NOT EXISTS compatibility_reports (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    steam_app_id    INTEGER NOT NULL,
    reporter        TEXT,
    mac_model       TEXT,              -- e.g. 'MacBook Pro M2 Max'
    macos_version   TEXT,              -- e.g. '15.2'
    backend         TEXT    NOT NULL,
    status          TEXT    NOT NULL,  -- 'Platinum', 'Gold', 'Silver', 'Bronze', 'Borked'
    fps_avg         REAL,
    notes           TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (steam_app_id) REFERENCES games(steam_app_id)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_games_steam_app_id ON games(steam_app_id);
CREATE INDEX IF NOT EXISTS idx_games_compat_status ON games(compat_status);
CREATE INDEX IF NOT EXISTS idx_games_backend ON games(backend);

CREATE INDEX IF NOT EXISTS idx_proton_commits_classification ON proton_commits(classification);
CREATE INDEX IF NOT EXISTS idx_proton_commits_date ON proton_commits(date);
CREATE INDEX IF NOT EXISTS idx_proton_commits_reviewed ON proton_commits(reviewed);

CREATE INDEX IF NOT EXISTS idx_backend_overrides_app_id ON backend_overrides(steam_app_id);

CREATE INDEX IF NOT EXISTS idx_compat_reports_app_id ON compatibility_reports(steam_app_id);
CREATE INDEX IF NOT EXISTS idx_compat_reports_status ON compatibility_reports(status);
