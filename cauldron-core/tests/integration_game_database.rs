//! Integration test: game database workflow.
//!
//! Tests the full data flow: init DB -> seed -> query -> report compat -> aggregate.

use cauldron_db::models::{CompatReportRecord, CompatStatus, GameRecord, GraphicsBackend};
use cauldron_db::queries::{
    get_aggregate_status, get_game_by_app_id, get_recommended_backend, insert_compat_report,
    insert_game, list_all_games,
};
use cauldron_db::schema::run_migrations;
use rusqlite::Connection;

fn setup_seeded_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    // Seed with test games
    let games = vec![
        GameRecord {
            steam_app_id: Some(1245620),
            exe_hash: Some("hash_elden".to_string()),
            title: "Elden Ring".to_string(),
            backend: GraphicsBackend::D3DMetal,
            compat_status: CompatStatus::Platinum,
            wine_overrides: "{}".to_string(),
            known_issues: String::new(),
            last_tested: "2024-03-28".to_string(),
            notes: "Runs well with D3DMetal".to_string(),
        },
        GameRecord {
            steam_app_id: Some(1091500),
            exe_hash: Some("hash_cyber".to_string()),
            title: "Cyberpunk 2077".to_string(),
            backend: GraphicsBackend::DXMT,
            compat_status: CompatStatus::Gold,
            wine_overrides: "{}".to_string(),
            known_issues: "No ray tracing".to_string(),
            last_tested: "2024-03-20".to_string(),
            notes: "DXMT best for this".to_string(),
        },
        GameRecord {
            steam_app_id: Some(730),
            exe_hash: Some("hash_cs2".to_string()),
            title: "Counter-Strike 2".to_string(),
            backend: GraphicsBackend::DxvkMoltenVK,
            compat_status: CompatStatus::Bronze,
            wine_overrides: "{}".to_string(),
            known_issues: "Anti-cheat blocks launch".to_string(),
            last_tested: "2024-03-18".to_string(),
            notes: String::new(),
        },
    ];

    for game in &games {
        insert_game(&conn, game).unwrap();
    }

    conn
}

#[test]
fn test_full_database_workflow() {
    let conn = setup_seeded_db();

    // 1. Verify we can list all games
    let games = list_all_games(&conn).unwrap();
    assert_eq!(games.len(), 3);

    // 2. Query specific game by app ID
    let elden_ring = get_game_by_app_id(&conn, 1245620).unwrap().unwrap();
    assert_eq!(elden_ring.title, "Elden Ring");
    assert_eq!(elden_ring.compat_status, CompatStatus::Platinum);

    // 3. Get recommended backend
    let backend = get_recommended_backend(&conn, Some(1245620), None).unwrap();
    assert_eq!(backend, GraphicsBackend::D3DMetal);

    let backend = get_recommended_backend(&conn, None, Some("hash_cyber")).unwrap();
    assert_eq!(backend, GraphicsBackend::DXMT);

    // 4. Submit compatibility reports
    let reports = vec![
        CompatReportRecord {
            game_id: "1245620".to_string(),
            reporter_hash: "user1".to_string(),
            status: "Platinum".to_string(),
            backend: "D3DMetal".to_string(),
            fps_avg: Some(60.0),
            notes: "Perfect on M3 Max".to_string(),
            timestamp: "2024-03-28T10:00:00Z".to_string(),
        },
        CompatReportRecord {
            game_id: "1245620".to_string(),
            reporter_hash: "user2".to_string(),
            status: "Platinum".to_string(),
            backend: "D3DMetal".to_string(),
            fps_avg: Some(45.0),
            notes: "Great on M1 Pro".to_string(),
            timestamp: "2024-03-28T11:00:00Z".to_string(),
        },
        CompatReportRecord {
            game_id: "1245620".to_string(),
            reporter_hash: "user3".to_string(),
            status: "Gold".to_string(),
            backend: "DXMT".to_string(),
            fps_avg: Some(30.0),
            notes: "Minor stutter".to_string(),
            timestamp: "2024-03-28T12:00:00Z".to_string(),
        },
    ];

    for report in &reports {
        insert_compat_report(&conn, report).unwrap();
    }

    // 5. Aggregate status should be Platinum (2 votes vs 1 Gold)
    let aggregate = get_aggregate_status(&conn, "1245620").unwrap();
    assert_eq!(aggregate, Some("Platinum".to_string()));

    // 6. Unknown game returns Auto backend
    let unknown_backend = get_recommended_backend(&conn, Some(99999), None).unwrap();
    assert_eq!(unknown_backend, GraphicsBackend::Auto);
}

#[test]
fn test_upsert_game_updates() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let game = GameRecord {
        steam_app_id: Some(1245620),
        exe_hash: Some("hash1".to_string()),
        title: "Elden Ring".to_string(),
        backend: GraphicsBackend::D3DMetal,
        compat_status: CompatStatus::Gold,
        wine_overrides: "{}".to_string(),
        known_issues: "Some issue".to_string(),
        last_tested: "2024-01-01".to_string(),
        notes: String::new(),
    };
    insert_game(&conn, &game).unwrap();

    // Update with new info
    let updated = GameRecord {
        steam_app_id: Some(1245620),
        exe_hash: Some("hash1".to_string()),
        title: "Elden Ring".to_string(),
        backend: GraphicsBackend::D3DMetal,
        compat_status: CompatStatus::Platinum,
        wine_overrides: "{}".to_string(),
        known_issues: String::new(),
        last_tested: "2024-03-28".to_string(),
        notes: "Updated!".to_string(),
    };
    insert_game(&conn, &updated).unwrap();

    // Should still be 1 game, but with updated data
    let games = list_all_games(&conn).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].compat_status, CompatStatus::Platinum);
    assert_eq!(games[0].notes, "Updated!");
}
