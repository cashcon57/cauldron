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
