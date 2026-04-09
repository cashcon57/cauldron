use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Migration failed: {0}")]
    Migration(String),
}

const CREATE_GAMES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS games (
    steam_app_id  INTEGER,
    exe_hash      TEXT,
    title         TEXT NOT NULL,
    backend       TEXT NOT NULL DEFAULT 'Auto',
    compat_status TEXT NOT NULL DEFAULT 'Unknown',
    wine_overrides TEXT NOT NULL DEFAULT '{}',
    known_issues  TEXT NOT NULL DEFAULT '',
    last_tested   TEXT NOT NULL DEFAULT '',
    notes         TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (steam_app_id, exe_hash)
);
"#;

const CREATE_PROTON_COMMITS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS proton_commits (
    hash            TEXT PRIMARY KEY,
    message         TEXT NOT NULL,
    author          TEXT NOT NULL,
    timestamp       TEXT NOT NULL,
    affected_files  TEXT NOT NULL DEFAULT '[]',
    classification  TEXT NOT NULL DEFAULT 'Unknown',
    transferability TEXT NOT NULL DEFAULT 'Medium',
    applied         INTEGER NOT NULL DEFAULT 0
);
"#;

const CREATE_BACKEND_OVERRIDES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS backend_overrides (
    game_id       TEXT NOT NULL,
    user_backend  TEXT NOT NULL,
    reason        TEXT NOT NULL DEFAULT '',
    timestamp     TEXT NOT NULL DEFAULT ''
);
"#;

const CREATE_COMPATIBILITY_REPORTS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS compatibility_reports (
    game_id       TEXT NOT NULL,
    reporter_hash TEXT NOT NULL,
    status        TEXT NOT NULL,
    backend       TEXT NOT NULL,
    fps_avg       REAL,
    notes         TEXT NOT NULL DEFAULT '',
    timestamp     TEXT NOT NULL DEFAULT ''
);
"#;

const CREATE_PATCH_LOG_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS patch_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    commit_hash     TEXT NOT NULL,
    outcome         TEXT NOT NULL,
    files_changed   INTEGER NOT NULL DEFAULT 0,
    conflicts       TEXT NOT NULL DEFAULT '[]',
    applied_at      TEXT NOT NULL DEFAULT (datetime('now')),
    reverted_at     TEXT,
    FOREIGN KEY (commit_hash) REFERENCES proton_commits(hash)
);
"#;

const CREATE_SYNC_STATUS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS sync_status (
    id                      INTEGER PRIMARY KEY,
    last_sync_timestamp     TEXT NOT NULL DEFAULT '',
    last_commit_hash        TEXT NOT NULL DEFAULT '',
    total_commits_processed INTEGER NOT NULL DEFAULT 0,
    commits_applied         INTEGER NOT NULL DEFAULT 0,
    commits_pending         INTEGER NOT NULL DEFAULT 0,
    commits_skipped         INTEGER NOT NULL DEFAULT 0,
    last_error              TEXT,
    sync_duration_ms        INTEGER NOT NULL DEFAULT 0
);
"#;

/// Open or create the SQLite database at the given path and run migrations.
pub fn init_db(path: &Path) -> Result<Connection, SchemaError> {
    tracing::info!("Initializing database at {}", path.display());
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Create all required tables if they do not already exist.
pub fn run_migrations(conn: &Connection) -> Result<(), SchemaError> {
    tracing::info!("Running database migrations");

    // Schema version tracking
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);"
    )?;
    let current_version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| row.get(0))
        .unwrap_or(0);
    tracing::info!(current_version, "Current schema version");

    conn.execute_batch(CREATE_GAMES_TABLE)?;
    conn.execute_batch(CREATE_PROTON_COMMITS_TABLE)?;
    conn.execute_batch(CREATE_BACKEND_OVERRIDES_TABLE)?;
    conn.execute_batch(CREATE_COMPATIBILITY_REPORTS_TABLE)?;
    conn.execute_batch(CREATE_PATCH_LOG_TABLE)?;
    conn.execute_batch(CREATE_SYNC_STATUS_TABLE)?;

    // Migration: add source column to proton_commits (Proton vs CrossOver)
    let _ = conn.execute_batch(
        "ALTER TABLE proton_commits ADD COLUMN source TEXT NOT NULL DEFAULT 'proton';"
    );

    // Game recommended settings table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS game_recommended_settings (
            steam_app_id       INTEGER PRIMARY KEY,
            msync_enabled      INTEGER,
            esync_enabled      INTEGER,
            rosetta_x87        INTEGER,
            async_shader       INTEGER,
            metalfx_upscaling  INTEGER,
            dxr_ray_tracing    INTEGER,
            fsr_enabled        INTEGER,
            large_address_aware INTEGER,
            wine_dll_overrides TEXT NOT NULL DEFAULT '{}',
            env_vars           TEXT NOT NULL DEFAULT '{}',
            windows_version    TEXT,
            launch_args        TEXT,
            auto_apply_patches INTEGER
        );"
    )?;

    // Migration: add cpu_topology to game_recommended_settings (Phase 2A)
    let _ = conn.execute_batch(
        "ALTER TABLE game_recommended_settings ADD COLUMN cpu_topology TEXT;"
    );

    // Migration: add required_dependencies to game_recommended_settings (Phase 3)
    let _ = conn.execute_batch(
        "ALTER TABLE game_recommended_settings ADD COLUMN required_dependencies TEXT NOT NULL DEFAULT '[]';"
    );

    // Migration: add registry_entries, exe_override, audio_latency_ms (Phase 4B)
    let _ = conn.execute_batch(
        "ALTER TABLE game_recommended_settings ADD COLUMN registry_entries TEXT NOT NULL DEFAULT '[]';"
    );
    let _ = conn.execute_batch(
        "ALTER TABLE game_recommended_settings ADD COLUMN exe_override TEXT;"
    );
    let _ = conn.execute_batch(
        "ALTER TABLE game_recommended_settings ADD COLUMN audio_latency_ms INTEGER;"
    );

    // Game binary patches table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS game_binary_patches (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            steam_app_id    INTEGER NOT NULL,
            exe_name        TEXT NOT NULL,
            exe_hash        TEXT NOT NULL,
            description     TEXT NOT NULL DEFAULT '',
            search_pattern  BLOB NOT NULL,
            replace_pattern BLOB NOT NULL,
            enabled         INTEGER NOT NULL DEFAULT 1,
            patch_mode      TEXT NOT NULL DEFAULT 'pattern',
            file_offset     INTEGER
        );"
    )?;

    // Migration: add patch_mode and file_offset if table existed before this migration
    let _ = conn.execute_batch(
        "ALTER TABLE game_binary_patches ADD COLUMN patch_mode TEXT NOT NULL DEFAULT 'pattern';"
    );
    let _ = conn.execute_batch(
        "ALTER TABLE game_binary_patches ADD COLUMN file_offset INTEGER;"
    );

    // Game dependencies installed tracking table (Phase 3)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS game_deps_installed (
            bottle_id      TEXT NOT NULL,
            steam_app_id   INTEGER NOT NULL,
            dependency_id  TEXT NOT NULL,
            installed_at   TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY(bottle_id, steam_app_id, dependency_id)
        );"
    )?;

    // Performance indexes for frequently queried columns
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_compat_reports_game_id ON compatibility_reports(game_id);
         CREATE INDEX IF NOT EXISTS idx_patch_log_commit_hash ON patch_log(commit_hash);
         CREATE INDEX IF NOT EXISTS idx_proton_commits_classification ON proton_commits(classification);
         CREATE INDEX IF NOT EXISTS idx_game_deps_steam_app ON game_deps_installed(steam_app_id);"
    );

    // Update schema version to current
    let new_version: i64 = 1;
    if current_version < new_version {
        let _ = conn.execute("INSERT OR REPLACE INTO schema_version (version) VALUES (?1)", [new_version]);
        tracing::info!(version = new_version, "Schema version updated");
    }

    tracing::info!("Migrations complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_init_db_creates_tables() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let conn = init_db(&db_path).unwrap();

        // Verify all tables exist by querying sqlite_master
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"games".to_string()));
        assert!(tables.contains(&"proton_commits".to_string()));
        assert!(tables.contains(&"backend_overrides".to_string()));
        assert!(tables.contains(&"compatibility_reports".to_string()));
        assert!(tables.contains(&"sync_status".to_string()));
    }

    #[test]
    fn test_run_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // Running again should not fail
        run_migrations(&conn).unwrap();

        // Tables should still exist and work
        conn.execute(
            "INSERT INTO games (steam_app_id, exe_hash, title) VALUES (1, 'abc', 'Test')",
            [],
        ).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM games", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_init_db_file_created() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let _conn = init_db(&db_path).unwrap();
        assert!(db_path.exists());
    }
}
