//! Dependency tracking for games.
//!
//! Tracks which runtime dependencies (vcrun2019, d3dcompiler_47, etc.) have
//! been installed for a given game in a given bottle, so they are only installed
//! once and not re-installed on every launch.

use rusqlite::Connection;

/// Check which required dependencies are missing for a game in a bottle.
///
/// Returns the list of dependency IDs that are required but not yet marked
/// as installed.
pub fn check_deps_installed(
    conn: &Connection,
    bottle_id: &str,
    app_id: u32,
    required: &[String],
) -> Vec<String> {
    let installed = cauldron_db::get_installed_deps(conn, bottle_id, app_id).unwrap_or_default();
    required
        .iter()
        .filter(|dep| !installed.contains(dep))
        .cloned()
        .collect()
}

/// Mark a single dependency as installed for a game in a bottle.
pub fn mark_dep_installed(
    conn: &Connection,
    bottle_id: &str,
    app_id: u32,
    dep_id: &str,
) -> Result<(), cauldron_db::DbError> {
    cauldron_db::mark_dep_installed(conn, bottle_id, app_id, dep_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cauldron_db::schema::run_migrations;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_check_deps_all_missing() {
        let conn = setup_db();
        let required = vec!["vcrun2022".to_string(), "d3dcompiler_47".to_string()];
        let missing = check_deps_installed(&conn, "bottle1", 1593500, &required);
        assert_eq!(missing, required);
    }

    #[test]
    fn test_check_deps_some_installed() {
        let conn = setup_db();
        mark_dep_installed(&conn, "bottle1", 1593500, "vcrun2022").unwrap();

        let required = vec!["vcrun2022".to_string(), "d3dcompiler_47".to_string()];
        let missing = check_deps_installed(&conn, "bottle1", 1593500, &required);
        assert_eq!(missing, vec!["d3dcompiler_47".to_string()]);
    }

    #[test]
    fn test_check_deps_all_installed() {
        let conn = setup_db();
        mark_dep_installed(&conn, "bottle1", 100, "vcrun2019").unwrap();

        let required = vec!["vcrun2019".to_string()];
        let missing = check_deps_installed(&conn, "bottle1", 100, &required);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_mark_dep_idempotent() {
        let conn = setup_db();
        mark_dep_installed(&conn, "bottle1", 100, "vcrun2019").unwrap();
        // Should not error on duplicate
        mark_dep_installed(&conn, "bottle1", 100, "vcrun2019").unwrap();

        let installed = cauldron_db::get_installed_deps(&conn, "bottle1", 100).unwrap();
        assert_eq!(installed.len(), 1);
    }
}
