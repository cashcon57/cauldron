use crate::models::{CompatReportRecord, CompatStatus, GameRecord, GraphicsBackend, ProtonCommit};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const GAME_COLUMNS: &str = "steam_app_id, exe_hash, title, backend, compat_status, wine_overrides, known_issues, last_tested, notes";
const COMMIT_COLUMNS: &str = "hash, message, author, timestamp, affected_files, classification, transferability, applied, source";

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Record not found: {0}")]
    NotFound(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Insert a game record into the database.
pub fn insert_game(conn: &Connection, game: &GameRecord) -> Result<(), DbError> {
    tracing::debug!(title = %game.title, app_id = ?game.steam_app_id, backend = %game.backend, "Inserting game record");
    conn.execute(
        "INSERT OR REPLACE INTO games (steam_app_id, exe_hash, title, backend, compat_status, wine_overrides, known_issues, last_tested, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            game.steam_app_id,
            game.exe_hash,
            game.title,
            game.backend.to_string(),
            game.compat_status.to_string(),
            game.wine_overrides,
            game.known_issues,
            game.last_tested,
            game.notes,
        ],
    )?;
    Ok(())
}

/// Look up a game by its Steam application ID.
/// Map a database row to a GameRecord.
fn game_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GameRecord> {
    Ok(GameRecord {
        steam_app_id: row.get(0)?,
        exe_hash: row.get(1)?,
        title: row.get(2)?,
        backend: row.get::<_, String>(3)?.parse().unwrap_or(GraphicsBackend::Auto),
        compat_status: row.get::<_, String>(4)?.parse().unwrap_or(CompatStatus::Unknown),
        wine_overrides: row.get(5)?,
        known_issues: row.get(6)?,
        last_tested: row.get(7)?,
        notes: row.get(8)?,
    })
}

/// Map a database row to a ProtonCommit.
fn commit_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProtonCommit> {
    Ok(ProtonCommit {
        hash: row.get(0)?,
        message: row.get(1)?,
        author: row.get(2)?,
        timestamp: row.get(3)?,
        affected_files: row.get(4)?,
        classification: row.get(5)?,
        transferability: row.get(6)?,
        applied: row.get::<_, i32>(7)? != 0,
        source: row.get::<_, String>(8).unwrap_or_else(|_| "proton".to_string()),
    })
}

pub fn get_game_by_app_id(conn: &Connection, app_id: u32) -> Result<Option<GameRecord>, DbError> {
    tracing::debug!(app_id = app_id, "Looking up game by app_id");
    let mut stmt = conn.prepare(
        &format!("SELECT {GAME_COLUMNS} FROM games WHERE steam_app_id = ?1 LIMIT 1"),
    )?;

    let mut rows = stmt.query_map(params![app_id], game_from_row)?;

    match rows.next() {
        Some(Ok(record)) => Ok(Some(record)),
        Some(Err(e)) => Err(DbError::Sqlite(e)),
        None => Ok(None),
    }
}

/// Look up a game by its executable hash.
pub fn get_game_by_hash(conn: &Connection, hash: &str) -> Result<Option<GameRecord>, DbError> {
    tracing::debug!(exe_hash = %hash, "Looking up game by exe hash");
    let mut stmt = conn.prepare(
        &format!("SELECT {GAME_COLUMNS} FROM games WHERE exe_hash = ?1 LIMIT 1"),
    )?;

    let mut rows = stmt.query_map(params![hash], game_from_row)?;

    match rows.next() {
        Some(Ok(record)) => Ok(Some(record)),
        Some(Err(e)) => Err(DbError::Sqlite(e)),
        None => Ok(None),
    }
}

/// Determine the recommended graphics backend for a game.
/// Checks by app ID first, then by hash. Returns `Auto` if no match is found.
pub fn get_recommended_backend(
    conn: &Connection,
    app_id: Option<u32>,
    hash: Option<&str>,
) -> Result<GraphicsBackend, DbError> {
    tracing::debug!(app_id = ?app_id, hash = ?hash, "Looking up recommended backend");
    if let Some(id) = app_id {
        if let Some(record) = get_game_by_app_id(conn, id)? {
            tracing::info!(app_id = id, backend = %record.backend, "Found backend recommendation by app_id");
            return Ok(record.backend);
        }
    }
    if let Some(h) = hash {
        if let Some(record) = get_game_by_hash(conn, h)? {
            tracing::info!(hash = %h, backend = %record.backend, "Found backend recommendation by hash");
            return Ok(record.backend);
        }
    }
    tracing::debug!("No backend recommendation found, defaulting to Auto");
    Ok(GraphicsBackend::Auto)
}

/// List all game records in the database.
pub fn list_all_games(conn: &Connection) -> Result<Vec<GameRecord>, DbError> {
    tracing::debug!("Listing all games");
    let mut stmt = conn.prepare(
        &format!("SELECT {GAME_COLUMNS} FROM games ORDER BY title ASC"),
    )?;

    let games = stmt
        .query_map([], game_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    tracing::debug!(count = games.len(), "Listed all games");
    Ok(games)
}

/// Insert a Proton commit record into the database.
pub fn insert_commit(conn: &Connection, commit: &ProtonCommit) -> Result<(), DbError> {
    tracing::debug!(hash = %commit.hash, classification = %commit.classification, "Inserting Proton commit");
    conn.execute(
        &format!("INSERT OR REPLACE INTO proton_commits ({COMMIT_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
        params![
            commit.hash,
            commit.message,
            commit.author,
            commit.timestamp,
            commit.affected_files,
            commit.classification,
            commit.transferability,
            commit.applied as i32,
            commit.source,
        ],
    )?;
    Ok(())
}

/// Retrieve all commits that have not yet been applied.
pub fn get_unapplied_commits(conn: &Connection) -> Result<Vec<ProtonCommit>, DbError> {
    tracing::debug!("Fetching unapplied commits");
    let mut stmt = conn.prepare(
&format!("SELECT {COMMIT_COLUMNS} FROM proton_commits WHERE applied = 0 ORDER BY timestamp ASC"),
    )?;

    let commits = stmt
        .query_map([], commit_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    tracing::debug!(count = commits.len(), "Found unapplied commits");
    Ok(commits)
}

/// Mark a commit as applied by its hash.
pub fn mark_commit_applied(conn: &Connection, hash: &str) -> Result<(), DbError> {
    tracing::info!(hash = %hash, "Marking commit as applied");
    let updated = conn.execute(
        "UPDATE proton_commits SET applied = 1 WHERE hash = ?1",
        params![hash],
    )?;
    if updated == 0 {
        return Err(DbError::NotFound(format!("commit {hash} not found")));
    }
    Ok(())
}

/// Insert a community compatibility report into the database.
pub fn insert_compat_report(
    conn: &Connection,
    report: &CompatReportRecord,
) -> Result<(), DbError> {
    tracing::debug!(game_id = %report.game_id, status = %report.status, "Inserting compat report");
    conn.execute(
        "INSERT INTO compatibility_reports (game_id, reporter_hash, status, backend, fps_avg, notes, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            report.game_id,
            report.reporter_hash,
            report.status,
            report.backend,
            report.fps_avg,
            report.notes,
            report.timestamp,
        ],
    )?;
    Ok(())
}

/// Retrieve all compatibility reports for a given game.
pub fn get_reports_for_game(
    conn: &Connection,
    game_id: &str,
) -> Result<Vec<CompatReportRecord>, DbError> {
    tracing::debug!(game_id = %game_id, "Fetching compat reports");
    let mut stmt = conn.prepare(
        "SELECT game_id, reporter_hash, status, backend, fps_avg, notes, timestamp
         FROM compatibility_reports WHERE game_id = ?1 ORDER BY timestamp DESC",
    )?;

    let reports = stmt
        .query_map(params![game_id], |row| {
            Ok(CompatReportRecord {
                game_id: row.get(0)?,
                reporter_hash: row.get(1)?,
                status: row.get(2)?,
                backend: row.get(3)?,
                fps_avg: row.get(4)?,
                notes: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    tracing::debug!(game_id = %game_id, count = reports.len(), "Fetched compat reports");
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::run_migrations;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    fn sample_game(title: &str, app_id: u32) -> GameRecord {
        GameRecord {
            steam_app_id: Some(app_id),
            exe_hash: Some(format!("hash_{}", app_id)),
            title: title.to_string(),
            backend: GraphicsBackend::D3DMetal,
            compat_status: CompatStatus::Gold,
            wine_overrides: "{}".to_string(),
            known_issues: String::new(),
            last_tested: "2024-01-01".to_string(),
            notes: String::new(),
        }
    }

    #[test]
    fn test_insert_and_get_game_by_app_id() {
        let conn = setup_db();
        let game = sample_game("Elden Ring", 1245620);
        insert_game(&conn, &game).unwrap();

        let result = get_game_by_app_id(&conn, 1245620).unwrap();
        assert!(result.is_some());
        let fetched = result.unwrap();
        assert_eq!(fetched.title, "Elden Ring");
        assert_eq!(fetched.backend, GraphicsBackend::D3DMetal);
    }

    #[test]
    fn test_get_game_by_app_id_not_found() {
        let conn = setup_db();
        let result = get_game_by_app_id(&conn, 99999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_game_by_hash() {
        let conn = setup_db();
        let game = sample_game("Test Game", 100);
        insert_game(&conn, &game).unwrap();

        let result = get_game_by_hash(&conn, "hash_100").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().title, "Test Game");
    }

    #[test]
    fn test_list_all_games() {
        let conn = setup_db();
        insert_game(&conn, &sample_game("Game A", 1)).unwrap();
        insert_game(&conn, &sample_game("Game B", 2)).unwrap();
        insert_game(&conn, &sample_game("Game C", 3)).unwrap();

        let games = list_all_games(&conn).unwrap();
        assert_eq!(games.len(), 3);
        // Should be sorted by title
        assert_eq!(games[0].title, "Game A");
        assert_eq!(games[1].title, "Game B");
        assert_eq!(games[2].title, "Game C");
    }

    #[test]
    fn test_recommended_backend_by_app_id() {
        let conn = setup_db();
        let mut game = sample_game("Test", 500);
        game.backend = GraphicsBackend::DXMT;
        insert_game(&conn, &game).unwrap();

        let backend = get_recommended_backend(&conn, Some(500), None).unwrap();
        assert_eq!(backend, GraphicsBackend::DXMT);
    }

    #[test]
    fn test_recommended_backend_by_hash() {
        let conn = setup_db();
        let mut game = sample_game("Test", 600);
        game.backend = GraphicsBackend::DxvkMoltenVK;
        insert_game(&conn, &game).unwrap();

        let backend = get_recommended_backend(&conn, None, Some("hash_600")).unwrap();
        assert_eq!(backend, GraphicsBackend::DxvkMoltenVK);
    }

    #[test]
    fn test_recommended_backend_default_auto() {
        let conn = setup_db();
        let backend = get_recommended_backend(&conn, Some(99999), None).unwrap();
        assert_eq!(backend, GraphicsBackend::Auto);
    }

    #[test]
    fn test_insert_and_get_commit() {
        let conn = setup_db();
        let commit = ProtonCommit {
            hash: "abc123".to_string(),
            message: "Fix Wine API".to_string(),
            author: "dev".to_string(),
            timestamp: "2024-01-01".to_string(),
            affected_files: "[]".to_string(),
            classification: "WineApiFix".to_string(),
            transferability: "High".to_string(),
            applied: false,
            source: "proton".to_string(),
        };
        insert_commit(&conn, &commit).unwrap();

        let unapplied = get_unapplied_commits(&conn).unwrap();
        assert_eq!(unapplied.len(), 1);
        assert_eq!(unapplied[0].hash, "abc123");
    }

    #[test]
    fn test_mark_commit_applied() {
        let conn = setup_db();
        let commit = ProtonCommit {
            hash: "def456".to_string(),
            message: "Fix something".to_string(),
            author: "dev".to_string(),
            timestamp: "2024-01-01".to_string(),
            affected_files: "[]".to_string(),
            classification: "Unknown".to_string(),
            transferability: "Medium".to_string(),
            applied: false,
            source: "proton".to_string(),
        };
        insert_commit(&conn, &commit).unwrap();
        mark_commit_applied(&conn, "def456").unwrap();

        let unapplied = get_unapplied_commits(&conn).unwrap();
        assert!(unapplied.is_empty());
    }

    #[test]
    fn test_insert_and_get_compat_report() {
        let conn = setup_db();
        let report = CompatReportRecord {
            game_id: "game1".to_string(),
            reporter_hash: "reporter1".to_string(),
            status: "Gold".to_string(),
            backend: "D3DMetal".to_string(),
            fps_avg: Some(60.0),
            notes: "Works great".to_string(),
            timestamp: "2024-01-01".to_string(),
        };
        insert_compat_report(&conn, &report).unwrap();

        let reports = get_reports_for_game(&conn, "game1").unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].status, "Gold");
    }

    #[test]
    fn test_aggregate_status() {
        let conn = setup_db();

        // Insert 3 Gold and 1 Silver report
        for _ in 0..3 {
            insert_compat_report(&conn, &CompatReportRecord {
                game_id: "game2".to_string(),
                reporter_hash: "r".to_string(),
                status: "Gold".to_string(),
                backend: "D3DMetal".to_string(),
                fps_avg: None,
                notes: String::new(),
                timestamp: "2024-01-01".to_string(),
            }).unwrap();
        }
        insert_compat_report(&conn, &CompatReportRecord {
            game_id: "game2".to_string(),
            reporter_hash: "r2".to_string(),
            status: "Silver".to_string(),
            backend: "DXMT".to_string(),
            fps_avg: None,
            notes: String::new(),
            timestamp: "2024-01-02".to_string(),
        }).unwrap();

        let status = get_aggregate_status(&conn, "game2").unwrap();
        assert_eq!(status, Some("Gold".to_string()));
    }

    #[test]
    fn test_aggregate_status_no_reports() {
        let conn = setup_db();
        let status = get_aggregate_status(&conn, "nonexistent").unwrap();
        assert!(status.is_none());
    }
}

/// Record a patch application outcome in the patch log.
pub fn insert_patch_log(
    conn: &Connection,
    commit_hash: &str,
    outcome: &str,
    files_changed: usize,
    conflicts: &[String],
) -> Result<(), DbError> {
    let conflicts_json = serde_json::to_string(conflicts)
        .unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO patch_log (commit_hash, outcome, files_changed, conflicts)
         VALUES (?1, ?2, ?3, ?4)",
        params![commit_hash, outcome, files_changed as i64, conflicts_json],
    )?;
    Ok(())
}

/// Get the full patch application history, newest first.
pub fn get_patch_log(conn: &Connection, limit: usize) -> Result<Vec<PatchLogEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, commit_hash, outcome, files_changed, conflicts, applied_at, reverted_at
         FROM patch_log ORDER BY applied_at DESC LIMIT ?1",
    )?;

    let entries = stmt
        .query_map(params![limit as i64], |row| {
            Ok(PatchLogEntry {
                id: row.get(0)?,
                commit_hash: row.get(1)?,
                outcome: row.get(2)?,
                files_changed: row.get::<_, i64>(3)? as usize,
                conflicts: row.get(4)?,
                applied_at: row.get(5)?,
                reverted_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(entries)
}

/// A row from the patch_log table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchLogEntry {
    pub id: i64,
    pub commit_hash: String,
    pub outcome: String,
    pub files_changed: usize,
    pub conflicts: String,
    pub applied_at: String,
    pub reverted_at: Option<String>,
}

/// Query proton commits with optional filter: "applied", "pending", "skipped", or None for all.
pub fn get_proton_commits(
    conn: &Connection,
    filter: Option<&str>,
    limit: usize,
) -> Result<Vec<ProtonCommit>, DbError> {
    let where_clause = match filter {
        Some("applied") => "WHERE applied = 1",
        Some("pending") => "WHERE applied = 0 AND hash NOT IN (SELECT commit_hash FROM patch_log WHERE outcome = 'skipped')",
        Some("skipped") => "WHERE hash IN (SELECT commit_hash FROM patch_log WHERE outcome = 'skipped')",
        _ => "",
    };
    let sql = format!("SELECT {COMMIT_COLUMNS} FROM proton_commits {where_clause} ORDER BY timestamp DESC LIMIT ?1");

    let mut stmt = conn.prepare(&sql)?;
    let commits = stmt
        .query_map(params![limit as i64], commit_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(commits)
}

/// Get a single proton commit by hash.
pub fn get_commit_by_hash(conn: &Connection, hash: &str) -> Result<Option<ProtonCommit>, DbError> {
    let mut stmt = conn.prepare(
        &format!("SELECT {COMMIT_COLUMNS} FROM proton_commits WHERE hash = ?1"),
    )?;

    let mut rows = stmt.query_map(params![hash], commit_from_row)?;

    match rows.next() {
        Some(Ok(commit)) => Ok(Some(commit)),
        Some(Err(e)) => Err(DbError::Sqlite(e)),
        None => Ok(None),
    }
}

/// Mark an applied patch as reverted: updates patch_log.reverted_at and sets applied=0.
pub fn mark_patch_reverted(conn: &Connection, hash: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE patch_log SET reverted_at = datetime('now')
         WHERE commit_hash = ?1 AND outcome = 'applied' AND reverted_at IS NULL",
        params![hash],
    )?;
    conn.execute(
        "UPDATE proton_commits SET applied = 0 WHERE hash = ?1",
        params![hash],
    )?;
    Ok(())
}

/// Return the most-reported compatibility status for a game (simple majority vote).
/// Returns `None` if there are no reports for the game.
pub fn get_aggregate_status(
    conn: &Connection,
    game_id: &str,
) -> Result<Option<String>, DbError> {
    tracing::debug!(game_id = %game_id, "Computing aggregate compat status");
    let mut stmt = conn.prepare(
        "SELECT status, COUNT(*) as cnt
         FROM compatibility_reports
         WHERE game_id = ?1
         GROUP BY status
         ORDER BY cnt DESC
         LIMIT 1",
    )?;

    let mut rows = stmt.query_map(params![game_id], |row| row.get::<_, String>(0))?;

    match rows.next() {
        Some(Ok(status)) => Ok(Some(status)),
        Some(Err(e)) => Err(DbError::Sqlite(e)),
        None => Ok(None),
    }
}
