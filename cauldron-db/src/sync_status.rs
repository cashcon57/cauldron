use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncStatusError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Represents the current state of the sync pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub id: i64,
    pub last_sync_timestamp: String,
    pub last_commit_hash: String,
    pub total_commits_processed: i64,
    pub commits_applied: i64,
    pub commits_pending: i64,
    pub commits_skipped: i64,
    pub last_error: Option<String>,
    pub sync_duration_ms: i64,
}

/// Retrieve the most recent sync status record, if any.
pub fn get_sync_status(conn: &Connection) -> Result<Option<SyncStatus>, SyncStatusError> {
    tracing::debug!("Fetching sync status");
    let mut stmt = conn.prepare(
        "SELECT id, last_sync_timestamp, last_commit_hash, total_commits_processed,
                commits_applied, commits_pending, commits_skipped, last_error, sync_duration_ms
         FROM sync_status ORDER BY id DESC LIMIT 1",
    )?;

    let mut rows = stmt.query_map([], |row| {
        Ok(SyncStatus {
            id: row.get(0)?,
            last_sync_timestamp: row.get(1)?,
            last_commit_hash: row.get(2)?,
            total_commits_processed: row.get(3)?,
            commits_applied: row.get(4)?,
            commits_pending: row.get(5)?,
            commits_skipped: row.get(6)?,
            last_error: row.get(7)?,
            sync_duration_ms: row.get(8)?,
        })
    })?;

    match rows.next() {
        Some(Ok(status)) => Ok(Some(status)),
        Some(Err(e)) => Err(SyncStatusError::Sqlite(e)),
        None => Ok(None),
    }
}

/// Insert or update a sync status record.
pub fn update_sync_status(conn: &Connection, status: &SyncStatus) -> Result<(), SyncStatusError> {
    tracing::info!(
        total = status.total_commits_processed,
        applied = status.commits_applied,
        pending = status.commits_pending,
        "Updating sync status"
    );
    conn.execute(
        "INSERT OR REPLACE INTO sync_status
         (id, last_sync_timestamp, last_commit_hash, total_commits_processed,
          commits_applied, commits_pending, commits_skipped, last_error, sync_duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            status.id,
            status.last_sync_timestamp,
            status.last_commit_hash,
            status.total_commits_processed,
            status.commits_applied,
            status.commits_pending,
            status.commits_skipped,
            status.last_error,
            status.sync_duration_ms,
        ],
    )?;
    Ok(())
}

/// Record the result of a single sync run, appending a new row to the status table.
pub fn record_sync_run(
    conn: &Connection,
    commits_found: usize,
    applied: usize,
    skipped: usize,
    duration_ms: u64,
    error: Option<&str>,
) -> Result<(), SyncStatusError> {
    tracing::info!(
        commits_found = commits_found,
        applied = applied,
        skipped = skipped,
        duration_ms = duration_ms,
        has_error = error.is_some(),
        "Recording sync run"
    );
    let pending = commits_found.saturating_sub(applied).saturating_sub(skipped);
    let now = chrono_now();

    // Get the last commit hash from proton_commits if available
    let last_hash: String = conn
        .query_row(
            "SELECT hash FROM proton_commits ORDER BY timestamp DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    conn.execute(
        "INSERT INTO sync_status
         (last_sync_timestamp, last_commit_hash, total_commits_processed,
          commits_applied, commits_pending, commits_skipped, last_error, sync_duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            now,
            last_hash,
            commits_found as i64,
            applied as i64,
            pending as i64,
            skipped as i64,
            error,
            duration_ms as i64,
        ],
    )?;
    Ok(())
}

/// Return the current UTC timestamp as an ISO 8601 string.
/// Uses a simple approach without pulling in the chrono crate.
fn chrono_now() -> String {
    // We format via the system command fallback; in production you would use chrono.
    // For now, use a fixed-format approach compatible with SQLite.
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    // Store as Unix timestamp string for simplicity and sortability.
    duration.as_secs().to_string()
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

    #[test]
    fn test_get_sync_status_empty() {
        let conn = setup_db();
        let status = get_sync_status(&conn).unwrap();
        assert!(status.is_none());
    }

    #[test]
    fn test_record_sync_run_and_get_status() {
        let conn = setup_db();
        record_sync_run(&conn, 10, 5, 3, 1500, None).unwrap();

        let status = get_sync_status(&conn).unwrap().unwrap();
        assert_eq!(status.total_commits_processed, 10);
        assert_eq!(status.commits_applied, 5);
        assert_eq!(status.commits_skipped, 3);
        assert_eq!(status.sync_duration_ms, 1500);
        assert!(status.last_error.is_none());
        assert!(!status.last_sync_timestamp.is_empty());
    }

    #[test]
    fn test_record_sync_run_with_error() {
        let conn = setup_db();
        record_sync_run(&conn, 5, 0, 0, 500, Some("connection failed")).unwrap();

        let status = get_sync_status(&conn).unwrap().unwrap();
        assert_eq!(status.last_error, Some("connection failed".to_string()));
    }

    #[test]
    fn test_multiple_sync_runs() {
        let conn = setup_db();
        record_sync_run(&conn, 10, 5, 3, 1000, None).unwrap();
        record_sync_run(&conn, 20, 15, 2, 2000, None).unwrap();

        // get_sync_status returns the most recent
        let status = get_sync_status(&conn).unwrap().unwrap();
        assert_eq!(status.total_commits_processed, 20);
        assert_eq!(status.sync_duration_ms, 2000);
    }

    #[test]
    fn test_update_sync_status() {
        let conn = setup_db();

        let status = SyncStatus {
            id: 1,
            last_sync_timestamp: "12345".to_string(),
            last_commit_hash: "abc".to_string(),
            total_commits_processed: 50,
            commits_applied: 40,
            commits_pending: 5,
            commits_skipped: 5,
            last_error: None,
            sync_duration_ms: 3000,
        };

        update_sync_status(&conn, &status).unwrap();

        let fetched = get_sync_status(&conn).unwrap().unwrap();
        assert_eq!(fetched.total_commits_processed, 50);
        assert_eq!(fetched.last_commit_hash, "abc");
    }
}
