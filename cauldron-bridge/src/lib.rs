use cauldron_core::bottle::BottleManager;
use cauldron_core::wine_downloader::WineManager;
use cauldron_db::{self, GameRecord, SyncStatus};
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Top-level engine struct wrapping all Cauldron subsystems.
struct CauldronEngine {
    bottle_manager: BottleManager,
    wine_manager: WineManager,
    db_path: PathBuf,
    base_dir: PathBuf,
    /// Tokio runtime for async operations (sync pipeline, Steam install).
    tokio_rt: tokio::runtime::Runtime,
    /// Path to the Wine source tree for patch application.
    wine_source_dir: PathBuf,
    /// Shared progress state for Steam installation polling.
    steam_install_progress: Arc<Mutex<Option<SteamInstallProgress>>>,
}

/// Progress state for Steam installation, polled by the Swift UI.
#[derive(Debug, Clone, serde::Serialize)]
struct SteamInstallProgress {
    step: String,
    message: String,
    progress: f32,
    complete: bool,
    error: Option<String>,
}

/// Convert a C string pointer to a Rust `&str`, returning `None` if null or invalid UTF-8.
fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Helper to return a JSON string as a C string pointer. Caller must free with `cauldron_free_string`.
fn to_c_json<T: serde::Serialize>(value: &T) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(json) => CString::new(json).map(|cs| cs.into_raw()).unwrap_or(std::ptr::null_mut()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Initialize a CauldronEngine rooted at the given base directory.
/// Returns an opaque pointer to the engine, or null on failure.
#[no_mangle]
pub extern "C" fn cauldron_init(base_dir: *const c_char) -> *mut c_void {
    let dir = match cstr_to_str(base_dir) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_init called with null base_dir pointer");
            return std::ptr::null_mut();
        }
    };

    tracing::info!(base_dir = %dir, "Initializing Cauldron engine");
    let base = PathBuf::from(dir);

    let tokio_rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("Failed to create tokio runtime: {e}");
            return std::ptr::null_mut();
        }
    };

    let engine = CauldronEngine {
        bottle_manager: BottleManager::new(base.clone()),
        wine_manager: WineManager::new(base.clone()),
        db_path: base.join("cauldron.db"),
        wine_source_dir: base.join("wine-source"),
        tokio_rt,
        steam_install_progress: Arc::new(Mutex::new(None)),
        base_dir: base,
    };
    let boxed = Box::new(engine);
    tracing::debug!("Cauldron engine initialized successfully");
    Box::into_raw(boxed) as *mut c_void
}

/// Free a CauldronEngine previously created by `cauldron_init`.
#[no_mangle]
pub extern "C" fn cauldron_free(ptr: *mut c_void) {
    if ptr.is_null() {
        tracing::debug!("cauldron_free called with null pointer, ignoring");
        return;
    }
    tracing::info!("Freeing Cauldron engine");
    unsafe {
        drop(Box::from_raw(ptr as *mut CauldronEngine));
    }
}

/// Initialize the game database at the given path. Returns 0 on success, -1 on failure.
#[no_mangle]
pub extern "C" fn cauldron_init_db(path: *const c_char) -> i32 {
    let db_path = match cstr_to_str(path) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_init_db called with null path pointer");
            return -1;
        }
    };

    match cauldron_db::init_db(std::path::Path::new(db_path)) {
        Ok(_conn) => {
            tracing::info!("Database initialized at {db_path}");
            0
        }
        Err(e) => {
            tracing::error!("Failed to initialize database: {e}");
            -1
        }
    }
}

/// Query a single game by Steam app ID. Returns a JSON string or null.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_query_game(mgr: *mut c_void, app_id: u32) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_query_game called with null engine pointer");
        return std::ptr::null_mut();
    }
    tracing::debug!(app_id = app_id, "FFI: querying game");
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open database: {e}");
            return std::ptr::null_mut();
        }
    };

    match cauldron_db::get_game_by_app_id(&conn, app_id) {
        Ok(Some(record)) => to_c_json(&record),
        Ok(None) => {
            tracing::info!("No game found for app_id={app_id}");
            std::ptr::null_mut()
        }
        Err(e) => {
            tracing::error!("Failed to query game: {e}");
            std::ptr::null_mut()
        }
    }
}

/// List all games in the database as a JSON array.
/// Returns seed data as a placeholder if the database is empty.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_list_games(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_list_games called with null engine pointer");
        return std::ptr::null_mut();
    }
    tracing::debug!("FFI: listing games");
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open database: {e}");
            return std::ptr::null_mut();
        }
    };

    // Return only real games from the database — no placeholder data
    let games: Vec<GameRecord> = match cauldron_db::list_all_games(&conn) {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("Failed to list games from DB: {e}");
            Vec::new()
        }
    };

    to_c_json(&games)
}

/// Get the current sync pipeline status as JSON.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_get_sync_status(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_get_sync_status called with null engine pointer");
        return std::ptr::null_mut();
    }
    tracing::debug!("FFI: getting sync status");
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open database: {e}");
            return std::ptr::null_mut();
        }
    };

    match cauldron_db::get_sync_status(&conn) {
        Ok(Some(status)) => to_c_json(&status),
        Ok(None) => {
            // Return a default/empty status
            let default_status = SyncStatus {
                id: 0,
                last_sync_timestamp: String::new(),
                last_commit_hash: String::new(),
                total_commits_processed: 0,
                commits_applied: 0,
                commits_pending: 0,
                commits_skipped: 0,
                last_error: None,
                sync_duration_ms: 0,
            };
            to_c_json(&default_status)
        }
        Err(e) => {
            tracing::error!("Failed to get sync status: {e}");
            std::ptr::null_mut()
        }
    }
}

/// Get available Wine versions as a JSON array of WineVersion objects.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_get_wine_versions(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_get_wine_versions called with null engine pointer");
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let versions = engine.wine_manager.available_versions();
    to_c_json(&versions)
}

/// Download and install a Wine version. Returns a JSON object with
/// `{"success": true, "path": "..."}` or `{"success": false, "error": "..."}`.
/// This is a blocking call. The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_download_wine(mgr: *mut c_void, version: *const c_char) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_download_wine called with null engine pointer");
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let ver = match cstr_to_str(version) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_download_wine: null version pointer");
            return std::ptr::null_mut();
        }
    };
    tracing::info!(version = %ver, "FFI: downloading Wine version");

    match engine.wine_manager.download_version(ver) {
        Ok(wine_bin) => {
            let result = serde_json::json!({
                "success": true,
                "path": wine_bin.to_string_lossy(),
                "version": ver,
            });
            to_c_json(&result)
        }
        Err(e) => {
            let result = serde_json::json!({
                "success": false,
                "error": e.to_string(),
            });
            to_c_json(&result)
        }
    }
}

/// Get all installed Wine versions as a JSON array with paths.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_get_installed_wine(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_get_installed_wine called with null engine pointer");
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let installed = engine.wine_manager.installed_versions();
    to_c_json(&installed)
}

/// Scan a bottle for detected games. Returns a JSON array of game records.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_scan_bottle_games(mgr: *mut c_void, bottle_id: *const c_char) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_scan_bottle_games called with null engine pointer");
        return std::ptr::null_mut();
    }
    tracing::debug!("FFI: scanning bottle games");
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let bid = match cstr_to_str(bottle_id) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_scan_bottle_games: bottle_id is None");
            return std::ptr::null_mut();
        }
    };
    tracing::debug!("scan called for bid={}", bid);

    // Look up the bottle to get its path
    let bottle = match engine.bottle_manager.get(bid) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("get bottle failed: {e}");
            return to_c_json(&Vec::<GameRecord>::new());
        }
    };

    // First, detect real Steam games via ACF manifests
    let bottle_path = &bottle.path;
    let steam_games = match cauldron_core::game_scanner::GameScanner::detect_steam_apps(bottle_path) {
        Ok(games) => games,
        Err(e) => {
            tracing::error!("detect_steam_apps failed: {e}");
            Vec::new()
        }
    };
    let steam_titles: std::collections::HashSet<String> = steam_games.iter().map(|g| g.title.clone()).collect();

    // Then scan for other executables
    let other_exes = match cauldron_core::game_scanner::GameScanner::scan_bottle(bottle_path, bid) {
        Ok(exes) => {
            tracing::info!(count = exes.len(), "Found other executables");
            exes
        }
        Err(e) => {
            tracing::error!("scan_bottle failed: {e}");
            Vec::new()
        }
    };

    let mut results: Vec<GameRecord> = Vec::new();

    // Steam games first (tagged as "game" in known_issues field for sorting)
    // Filter out non-game Steam entries
    const STEAM_NON_GAMES: &[&str] = &[
        "Steamworks Common Redistributables",
        "Steam Linux Runtime",
        "Steam Linux Runtime - Soldier",
        "Steam Linux Runtime - Sniper",
        "Proton Experimental",
        "Proton EasyAntiCheat Runtime",
        "Proton BattlEye Runtime",
    ];
    for g in &steam_games {
        if STEAM_NON_GAMES.iter().any(|&name| g.title == name) {
            continue;
        }
        let dx_label = g.dx_version.map(|v| format!("DX{}", v)).unwrap_or_default();
        results.push(GameRecord {
            steam_app_id: g.steam_app_id,
            exe_hash: g.exe_hash.clone(),
            title: g.title.clone(),
            backend: cauldron_db::GraphicsBackend::Auto,
            compat_status: cauldron_db::CompatStatus::Unknown,
            wine_overrides: "{}".to_string(),
            known_issues: "game".to_string(),
            last_tested: String::new(),
            notes: if dx_label.is_empty() {
                g.exe_path.display().to_string()
            } else {
                format!("{} | {}", dx_label, g.exe_path.display())
            },
        });
    }

    // Other executables (tagged as "utility")
    for g in &other_exes {
        if steam_titles.contains(&g.title) {
            continue; // Already listed as a Steam game
        }
        let dx_label = g.dx_version.map(|v| format!("DX{}", v)).unwrap_or_default();
        results.push(GameRecord {
            steam_app_id: g.steam_app_id,
            exe_hash: g.exe_hash.clone(),
            title: g.title.clone(),
            backend: cauldron_db::GraphicsBackend::Auto,
            compat_status: cauldron_db::CompatStatus::Unknown,
            wine_overrides: "{}".to_string(),
            known_issues: "utility".to_string(),
            last_tested: String::new(),
            notes: if dx_label.is_empty() {
                g.exe_path.display().to_string()
            } else {
                format!("{} | {}", dx_label, g.exe_path.display())
            },
        });
    }

    to_c_json(&results)
}

/// Create a new bottle. Returns a JSON string describing the bottle, or null on failure.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_create_bottle(
    mgr: *mut c_void,
    name: *const c_char,
    wine_version: *const c_char,
) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_create_bottle called with null engine pointer");
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let name = match cstr_to_str(name) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_create_bottle: null name pointer");
            return std::ptr::null_mut();
        }
    };
    let wine_ver = match cstr_to_str(wine_version) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_create_bottle: null wine_version pointer");
            return std::ptr::null_mut();
        }
    };
    tracing::debug!(name = %name, wine_version = %wine_ver, "FFI: creating bottle");

    match engine.bottle_manager.create(name, wine_ver) {
        Ok(bottle) => to_c_json(&bottle),
        Err(e) => {
            tracing::error!("Failed to create bottle: {e}");
            std::ptr::null_mut()
        }
    }
}

/// List all bottles as a JSON array string.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_list_bottles(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_list_bottles called with null engine pointer");
        return std::ptr::null_mut();
    }
    tracing::debug!("FFI: listing bottles");
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    match engine.bottle_manager.list() {
        Ok(bottles) => to_c_json(&bottles),
        Err(e) => {
            tracing::error!("Failed to list bottles: {e}");
            std::ptr::null_mut()
        }
    }
}

/// Delete a bottle by its ID. Returns 0 on success, -1 on failure.
#[no_mangle]
pub extern "C" fn cauldron_delete_bottle(mgr: *mut c_void, id: *const c_char) -> i32 {
    if mgr.is_null() {
        tracing::error!("cauldron_delete_bottle called with null engine pointer");
        return -1;
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let id = match cstr_to_str(id) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_delete_bottle: null id pointer");
            return -1;
        }
    };
    tracing::debug!(bottle_id = %id, "FFI: deleting bottle");

    match engine.bottle_manager.delete(id) {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!("Failed to delete bottle: {e}");
            -1
        }
    }
}

/// Discover existing Wine bottles from other applications (Whisky, CrossOver, etc.).
/// Returns a JSON array of discovered bottles, or null on failure.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_discover_bottles(_mgr: *mut c_void) -> *mut c_char {
    tracing::debug!("FFI: discovering bottles from other applications");
    let bottles = cauldron_core::bottle_discovery::BottleDiscovery::discover_all();
    to_c_json(&bottles)
}

/// Import a discovered bottle by its source path. Creates a symlink in Cauldron's
/// bottles directory and writes a proper bottle.toml so it appears in the bottle list.
/// Returns the imported Bottle as JSON, or null on failure.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_import_bottle(
    mgr: *mut c_void,
    source_path: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_import_bottle called with null engine pointer");
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let src = match cstr_to_str(source_path) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_import_bottle: null source_path");
            return std::ptr::null_mut();
        }
    };
    let bottle_name = match cstr_to_str(name) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_import_bottle: null name");
            return std::ptr::null_mut();
        }
    };

    tracing::info!(source = %src, name = %bottle_name, "FFI: importing discovered bottle");

    // Find the matching discovered bottle to get full metadata (wine version, etc.)
    let discovered = cauldron_core::bottle_discovery::BottleDiscovery::discover_all();
    let matching = discovered.iter().find(|b| b.path.to_string_lossy() == src);

    let bottle_to_import = if let Some(found) = matching {
        found.clone()
    } else {
        // Construct a minimal DiscoveredBottle from the path
        tracing::warn!("No matching discovered bottle for {src}, constructing from path");
        cauldron_core::bottle_discovery::DiscoveredBottle {
            name: bottle_name.to_string(),
            path: PathBuf::from(src),
            source: cauldron_core::bottle_discovery::BottleSource::StandaloneWine,
            wine_version: "unknown".to_string(),
            size_bytes: 0,
            has_steam: false,
            game_count: 0,
            graphics_backend: "unknown".to_string(),
        }
    };

    // Import using symlink (fast, no disk copy)
    let bottles_dir = &engine.bottle_manager.bottles_dir;
    match cauldron_core::bottle_discovery::BottleDiscovery::import_discovered(
        &bottle_to_import,
        bottles_dir,
        true, // symlink
    ) {
        Ok(dest) => {
            // Read the bottle.toml — for symlinked bottles it's inside the source dir
            // (since the symlink points there, reading via dest also works)
            let config_path = dest.join("bottle.toml");
            let content = match std::fs::read_to_string(&config_path) {
                Ok(c) => c,
                Err(_) => {
                    // Fallback: try the source path directly
                    let fallback = bottle_to_import.path.join("bottle.toml");
                    match std::fs::read_to_string(&fallback) {
                        Ok(c) => c,
                        Err(e) => {
                            let err = format!(
                                "Cannot read bottle.toml from {} or {}: {e}",
                                config_path.display(),
                                fallback.display()
                            );
                            eprintln!("[cauldron] {err}");
                            return to_c_json(&serde_json::json!({"error": err}));
                        }
                    }
                }
            };

            match toml::from_str::<cauldron_core::bottle::Bottle>(&content) {
                Ok(bottle) => {
                    eprintln!(
                        "[cauldron] Bottle imported: id={} name={} wine={}",
                        bottle.id, bottle.name, bottle.wine_version
                    );
                    to_c_json(&bottle)
                }
                Err(e) => {
                    let err = format!("bottle.toml parse error: {e}\nContent: {content}");
                    eprintln!("[cauldron] {err}");
                    return to_c_json(&serde_json::json!({"error": err}));
                }
            }
        }
        Err(e) => {
            let err = format!("import_discovered failed: {e}");
            eprintln!("[cauldron] {err}");
            to_c_json(&serde_json::json!({"error": err}))
        }
    }
}

/// Run a full sync cycle: fetch Proton commits, classify, adapt, apply patches, and
/// record results in the database. Returns updated SyncStatus JSON.
/// This is a blocking call — the Swift side should call it from Task.detached.
/// The returned string must be freed with `cauldron_free_string`.
#[no_mangle]
pub extern "C" fn cauldron_run_sync(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        tracing::error!("cauldron_run_sync called with null engine pointer");
        return std::ptr::null_mut();
    }
    tracing::info!("FFI: running real sync pipeline");
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    // Define sync sources: Proton + CodeWeavers Wine fork
    let sources: Vec<(&str, &str, &str, &[&str])> = vec![
        ("proton", "proton-repo", "https://github.com/ValveSoftware/Proton.git",
         &["proton_10.0", "experimental_10.0", "bleeding-edge"]),
        ("crossover", "crossover-wine-repo", "https://github.com/AhmedSaidTuncworx/wine.git",
         &["master"]),
    ];

    for (source_name, repo_dir, remote_url, _branches) in &sources {
        let repo_path = engine.base_dir.join(repo_dir);
        let url = remote_url.to_string();

        let pipeline = if engine.wine_source_dir.join(".git").exists() {
            cauldron_sync::SyncPipeline::with_applicator(
                repo_path,
                url,
                engine.db_path.clone(),
                Duration::from_secs(300),
                engine.wine_source_dir.clone(),
            ).with_source(source_name)
        } else {
            cauldron_sync::SyncPipeline::new(
                repo_path,
                url,
                engine.db_path.clone(),
                Duration::from_secs(300),
            ).with_source(source_name)
        };

        match engine.tokio_rt.block_on(pipeline.run_once()) {
            Ok(result) => {
                tracing::info!(
                    "Sync [{source_name}] complete: {} total, {} applied, {} pending, {} skipped",
                    result.total_commits, result.applied, result.pending_review, result.skipped,
                );
            }
            Err(e) => {
                tracing::error!("Sync [{source_name}] failed: {e}");
            }
        }
    }

    // Return the updated status from the database
    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open database after sync: {e}");
            return std::ptr::null_mut();
        }
    };

    match cauldron_db::get_sync_status(&conn) {
        Ok(Some(status)) => to_c_json(&status),
        Ok(None) => {
            let default_status = SyncStatus {
                id: 0,
                last_sync_timestamp: String::new(),
                last_commit_hash: String::new(),
                total_commits_processed: 0,
                commits_applied: 0,
                commits_pending: 0,
                commits_skipped: 0,
                last_error: None,
                sync_duration_ms: 0,
            };
            to_c_json(&default_status)
        }
        Err(e) => {
            tracing::error!("Failed to get sync status: {e}");
            std::ptr::null_mut()
        }
    }
}

/// Launch an executable inside a bottle using Wine.
/// Returns 0 on success (process spawned), -1 on failure.
#[no_mangle]
pub extern "C" fn cauldron_launch_exe(
    mgr: *mut c_void,
    bottle_id: *const c_char,
    exe_path: *const c_char,
    backend: *const c_char,
) -> i32 {
    if mgr.is_null() {
        tracing::error!("cauldron_launch_exe called with null engine pointer");
        return -1;
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    let bid = match cstr_to_str(bottle_id) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_launch_exe: null bottle_id");
            return -1;
        }
    };
    let exe = match cstr_to_str(exe_path) {
        Some(s) => s,
        None => {
            tracing::error!("cauldron_launch_exe: null exe_path");
            return -1;
        }
    };
    let backend_str = cstr_to_str(backend).unwrap_or("auto");

    tracing::info!(bottle_id = %bid, exe = %exe, backend = %backend_str, "FFI: launching exe");

    // Find the bottle
    let bottles = match engine.bottle_manager.list() {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to list bottles: {e}");
            return -1;
        }
    };

    let bottle = match bottles.into_iter().find(|b| b.id == bid) {
        Some(b) => b,
        None => {
            tracing::error!("Bottle not found: {bid}");
            return -1;
        }
    };

    // Look for wine binary: first check WineManager installed versions,
    // then fall back to standard system paths
    let wine_bin = {
        let installed = engine.wine_manager.installed_versions();
        let from_manager = installed.iter().find_map(|v| {
            cauldron_core::wine_downloader::find_wine_binary(&v.path).ok()
        });

        if let Some(bin) = from_manager {
            bin
        } else {
            let home = std::env::var("HOME").unwrap_or_default();
            let wine_search_paths = vec![
                PathBuf::from("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"),
                PathBuf::from(&home).join("Library/Cauldron/wine/bin/wine64"),
                PathBuf::from(&home).join("Library/Cauldron/wine/bin/wine"),
                PathBuf::from("/usr/local/bin/wine64"),
                PathBuf::from("/usr/local/bin/wine"),
                PathBuf::from("/opt/homebrew/bin/wine64"),
                PathBuf::from("/opt/homebrew/bin/wine"),
                PathBuf::from("/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine64"),
                PathBuf::from("/Applications/Wine Devel.app/Contents/Resources/wine/bin/wine64"),
            ];

            match wine_search_paths.into_iter().find(|p| p.exists()) {
                Some(p) => p,
                None => {
                    tracing::error!("No Wine binary found in any search path");
                    return -1;
                }
            }
        }
    };

    // Build environment variables
    let mut env_vars = std::collections::HashMap::new();
    env_vars.insert(
        "WINEPREFIX".to_string(),
        bottle.path.to_string_lossy().to_string(),
    );

    // Parse graphics backend
    let gfx_backend = match backend_str {
        "d3d_metal" => cauldron_db::GraphicsBackend::D3DMetal,
        "dxmt" => cauldron_db::GraphicsBackend::DXMT,
        "dxvk_moltenvk" => cauldron_db::GraphicsBackend::DxvkMoltenVK,
        "dxvk_kosmic_krisp" => cauldron_db::GraphicsBackend::DxvkKosmicKrisp,
        "vkd3d_proton" => cauldron_db::GraphicsBackend::Vkd3dProton,
        _ => cauldron_db::GraphicsBackend::Auto,
    };

    let gfx_config = cauldron_core::graphics::GraphicsConfig {
        backend: gfx_backend,
        dxvk_async: true,
        metalfx_spatial: false,
        metalfx_upscale_factor: 2.0,
        dlss_metalfx: false,
        metal_hud: false,
        dxr_enabled: false,
        mvk_argument_buffers: true,
    };
    let gfx_env = cauldron_core::graphics::build_env_vars(&gfx_config);
    env_vars.extend(gfx_env);

    // Add bottle env overrides
    env_vars.extend(bottle.env_overrides.clone());

    let mut exe_path = PathBuf::from(exe);

    // Resolve game-specific launch config from the database.
    // Try to detect steam_app_id from the exe path (e.g. steamapps/common/<game>/...)
    // and merge recommended settings into the launch environment.
    if let Ok(conn) = cauldron_db::init_db(&engine.db_path) {
        let steam_app_id = detect_steam_app_id(&exe_path, &conn);
        if let Some(app_id) = steam_app_id {
            let exe_name = exe_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let config = cauldron_core::launch_config_resolver::resolve(
                &conn, app_id, exe_name, None,
            );
            tracing::info!(app_id = app_id, "Resolved launch config from DB");

            // Apply resolved env vars and DLL overrides
            config.apply_to_env(&mut env_vars);

            // Apply exe_replacement if set (swap to a different exe within the game dir)
            if let Some(ref replacement) = config.exe_replacement {
                if let Some(parent) = exe_path.parent() {
                    let new_exe = parent.join(replacement);
                    if new_exe.exists() {
                        tracing::info!(
                            original = %exe_path.display(),
                            replacement = %new_exe.display(),
                            "Applying exe replacement from launch config"
                        );
                        exe_path = new_exe;
                    } else {
                        tracing::warn!(
                            replacement = %new_exe.display(),
                            "Exe replacement path does not exist, using original"
                        );
                    }
                }
            }

            // Apply windows_version via registry if set
            if let Some(ref win_ver) = config.windows_version {
                let version_reg = bottle.path.join(".cauldron_winver.reg");
                let (major, minor, build) = windows_version_to_nt(win_ver);
                let reg_content = format!(
                    "Windows Registry Editor Version 5.00\n\n\
                     [HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows NT\\CurrentVersion]\n\
                     \"CurrentVersion\"=\"{}.{}\"\n\
                     \"CurrentBuild\"=\"{}\"\n\
                     \"CurrentBuildNumber\"=\"{}\"\n",
                    major, minor, build, build
                );
                if let Err(e) = std::fs::write(&version_reg, &reg_content) {
                    tracing::warn!("Failed to write windows version reg file: {e}");
                }
            }

            // Log required dependencies (actual installation is handled separately)
            if !config.required_dependencies.is_empty() {
                tracing::info!(
                    deps = ?config.required_dependencies,
                    "Game requires dependencies"
                );
            }
        }
    }

    // Spawn Wine process synchronously (non-async) using std::process::Command
    match std::process::Command::new(&wine_bin)
        .arg(&exe_path)
        .envs(&env_vars)
        .spawn()
    {
        Ok(_child) => {
            tracing::info!(exe = %exe_path.display(), "Wine process spawned successfully");
            0
        }
        Err(e) => {
            tracing::error!(exe = %exe_path.display(), error = %e, "Failed to spawn Wine process");
            -1
        }
    }
}

/// Try to detect the Steam app ID from an exe path by checking if it matches
/// a known game in the database. Looks at the path for patterns like
/// `steamapps/common/<game>/` and cross-references the DB.
fn detect_steam_app_id(exe_path: &std::path::Path, conn: &cauldron_db::Connection) -> Option<u32> {
    // Try to find a Steam app ID by scanning all known games and matching exe path components
    let path_str = exe_path.to_string_lossy().to_lowercase();

    // Quick check: if path contains "steamapps" we can try to extract info
    if !path_str.contains("steamapps") {
        return None;
    }

    // List all games and see if any title matches a path component
    let games = cauldron_db::list_all_games(conn).ok()?;
    for game in &games {
        if let Some(app_id) = game.steam_app_id {
            // Check if the settings table has an entry for this app_id
            if let Ok(Some(_)) = cauldron_db::get_game_settings(conn, app_id) {
                // Simple heuristic: game title words appear in the path
                let title_lower = game.title.to_lowercase();
                let title_words: Vec<&str> = title_lower.split_whitespace().collect();
                if title_words.len() >= 2 && title_words.iter().all(|w| path_str.contains(w)) {
                    return Some(app_id);
                }
            }
        }
    }

    None
}

/// Map a Wine windows version string to NT version numbers.
fn windows_version_to_nt(version: &str) -> (&str, &str, &str) {
    match version {
        "winxp" => ("5", "1", "2600"),
        "win7" => ("6", "1", "7601"),
        "win8" => ("6", "2", "9200"),
        "win81" => ("6", "3", "9600"),
        "win10" => ("10", "0", "19041"),
        "win11" => ("10", "0", "22000"),
        _ => ("10", "0", "19041"), // Default to win10
    }
}

/// Free a string previously returned by any `cauldron_*` function.
#[no_mangle]
pub extern "C" fn cauldron_free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

/// Collect PE imports (DLL names) for all games across all bottles.
fn collect_game_imports(engine: &CauldronEngine) -> Vec<(String, Vec<String>)> {
    let mut results = Vec::new();
    let bottles = engine.bottle_manager.list().unwrap_or_default();
    for bottle in &bottles {
        let detected = cauldron_core::game_scanner::GameScanner::detect_steam_apps(&bottle.path)
            .unwrap_or_default();
        for game in &detected {
            if game.exe_path.exists() && game.exe_path.is_file() {
                let imports = cauldron_core::game_scanner::GameScanner::read_pe_import_names(&game.exe_path);
                if !imports.is_empty() {
                    results.push((game.title.clone(), imports));
                }
            }
        }
    }
    results
}

/// Analyze all pending patches: dry-run, impact score, affected games, ProtonDB.
/// Returns JSON array of PatchAnalysis objects.
#[no_mangle]
pub extern "C" fn cauldron_analyze_patches(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(_) => return to_c_json(&Vec::<cauldron_sync::PatchAnalysis>::new()),
    };

    let db_commits = cauldron_db::get_proton_commits(&conn, None, 200).unwrap_or_default();
    if db_commits.is_empty() {
        return to_c_json(&Vec::<cauldron_sync::PatchAnalysis>::new());
    }

    // Open both source repos for diff lookups
    let proton_repo = git2::Repository::open(engine.base_dir.join("proton-repo")).ok();
    let crossover_repo = git2::Repository::open(engine.base_dir.join("crossover-wine-repo")).ok();

    let mut classified_commits = Vec::new();
    for c in &db_commits {
        let diff = match c.source.as_str() {
            "crossover" => crossover_repo.as_ref().and_then(|r| diff_for_commit(r, &c.hash)),
            _ => proton_repo.as_ref().and_then(|r| diff_for_commit(r, &c.hash)),
        }.unwrap_or_default();
        let affected_files: Vec<String> = serde_json::from_str(&c.affected_files).unwrap_or_default();
        classified_commits.push(cauldron_sync::ClassifiedCommit {
            hash: c.hash.clone(),
            message: c.message.clone(),
            author: c.author.clone(),
            timestamp: c.timestamp.clone(),
            diff,
            affected_files,
            classification: cauldron_sync::Classification::from_str(&c.classification),
            transferability: cauldron_sync::Transferability::from_str(&c.transferability),
            suggested_action: String::new(),
        });
    }

    // Collect game PE imports for cross-referencing
    let game_imports = collect_game_imports(engine);

    let analyses = cauldron_sync::analyze_patches(
        &classified_commits,
        &engine.wine_source_dir,
        &game_imports,
    );

    to_c_json(&analyses)
}

/// Trigger a Wine build verification in the background.
/// Returns JSON with success/error. The build runs `make -j` in wine_source_dir.
#[no_mangle]
pub extern "C" fn cauldron_verify_build(mgr: *mut c_void) -> *mut c_char {
    if mgr.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };

    // Auto-clone Wine source if not present
    if !engine.wine_source_dir.join(".git").exists() {
        let clone = std::process::Command::new("git")
            .args(["clone", "--depth=1", "https://github.com/wine-mirror/wine.git",
                   engine.wine_source_dir.to_str().unwrap_or("")])
            .output();
        match clone {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return to_c_json(&serde_json::json!({
                    "success": false,
                    "error": format!("Failed to clone Wine source: {}", stderr.chars().take(300).collect::<String>())
                }));
            }
            Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("git clone failed: {e}")})),
        }
    }

    // Ensure homebrew tools (bison 3.0+, llvm for PE cross-compilation) are on PATH
    let homebrew_path = if cfg!(target_arch = "aarch64") {
        "/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/bison/bin:/opt/homebrew/bin"
    } else {
        "/usr/local/opt/llvm/bin:/usr/local/opt/bison/bin:/usr/local/bin"
    };
    let path = format!("{}:{}", homebrew_path, std::env::var("PATH").unwrap_or_default());

    // Check if configure/Makefile exists
    let has_makefile = engine.wine_source_dir.join("Makefile").exists();
    if !has_makefile {
        let configure = engine.wine_source_dir.join("configure");
        if configure.exists() {
            let conf_result = std::process::Command::new("./configure")
                .args(["--enable-win64"])
                .env("PATH", &path)
                .current_dir(&engine.wine_source_dir)
                .output();
            if let Ok(out) = conf_result {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return to_c_json(&serde_json::json!({
                        "success": false,
                        "error": format!("Configure failed: {}", stderr.chars().take(500).collect::<String>())
                    }));
                }
            }
        } else {
            return to_c_json(&serde_json::json!({
                "success": false,
                "error": "No Makefile or configure script found in Wine source."
            }));
        }
    }

    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let build = std::process::Command::new("make")
        .args(["-j", &cpus.to_string()])
        .env("PATH", &path)
        .current_dir(&engine.wine_source_dir)
        .output();

    match build {
        Ok(out) if out.status.success() => {
            to_c_json(&serde_json::json!({
                "success": true,
                "message": "Wine build succeeded."
            }))
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Extract the last 10 lines of stderr for the error
            let last_lines: String = stderr.lines().rev().take(10).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            to_c_json(&serde_json::json!({
                "success": false,
                "error": format!("Build failed:\n{last_lines}")
            }))
        }
        Err(e) => to_c_json(&serde_json::json!({
            "success": false,
            "error": format!("Failed to run make: {e}")
        })),
    }
}

/// Retrieve the diff for a commit from a pre-opened git repo.
fn diff_for_commit(repo: &git2::Repository, hash: &str) -> Option<String> {
    let oid = git2::Oid::from_str(hash).ok()?;
    let commit = repo.find_commit(oid).ok()?;
    let tree = commit.tree().ok()?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None).ok()?;
    let mut buf = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_delta: git2::DiffDelta<'_>, _hunk: Option<git2::DiffHunk<'_>>, line: git2::DiffLine<'_>| {
        buf.extend_from_slice(line.content());
        true
    }).ok()?;
    String::from_utf8(buf).ok()
}

/// Open the correct source repo and get a diff for a single commit.
/// Tries the commit's source repo first, then falls back to both repos.
fn get_diff_from_repo(engine: &CauldronEngine, hash: &str, source: &str) -> Option<String> {
    let primary = match source {
        "crossover" => "crossover-wine-repo",
        _ => "proton-repo",
    };
    let fallback = match source {
        "crossover" => "proton-repo",
        _ => "crossover-wine-repo",
    };

    for dir in [primary, fallback] {
        let repo_path = engine.base_dir.join(dir);
        if let Ok(repo) = git2::Repository::open(&repo_path) {
            if let Some(d) = diff_for_commit(&repo, hash) {
                return Some(d);
            }
        }
    }
    None
}

/// Query proton commits with optional filter. Returns JSON array.
#[no_mangle]
pub extern "C" fn cauldron_get_proton_commits(
    mgr: *mut c_void,
    filter: *const c_char,
    limit: u32,
) -> *mut c_char {
    if mgr.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let filter_opt = cstr_to_str(filter);

    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(_) => return to_c_json(&Vec::<cauldron_db::ProtonCommit>::new()),
    };

    match cauldron_db::get_proton_commits(&conn, filter_opt, limit as usize) {
        Ok(commits) => to_c_json(&commits),
        Err(_) => to_c_json(&Vec::<cauldron_db::ProtonCommit>::new()),
    }
}

/// Apply a single patch by commit hash. Returns JSON with success/error.
#[no_mangle]
pub extern "C" fn cauldron_apply_patch(mgr: *mut c_void, hash: *const c_char) -> *mut c_char {
    if mgr.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let hash_str = match cstr_to_str(hash) {
        Some(s) => s,
        None => return to_c_json(&serde_json::json!({"success": false, "error": "null hash"})),
    };

    // Auto-clone Wine source if not present
    if !engine.wine_source_dir.join(".git").exists() {
        let clone = std::process::Command::new("git")
            .args(["clone", "--depth=1", "https://github.com/wine-mirror/wine.git",
                   engine.wine_source_dir.to_str().unwrap_or("")])
            .output();
        match clone {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return to_c_json(&serde_json::json!({
                    "success": false,
                    "error": format!("Failed to clone Wine source: {}", stderr.chars().take(300).collect::<String>())
                }));
            }
            Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("git clone failed: {e}")})),
            _ => {}
        }
    }

    // Get commit from DB
    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("DB error: {e}")})),
    };

    let commit = match cauldron_db::get_commit_by_hash(&conn, hash_str) {
        Ok(Some(c)) => c,
        Ok(None) => return to_c_json(&serde_json::json!({"success": false, "error": "Commit not found in database"})),
        Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("DB error: {e}")})),
    };

    // Get diff from the correct source repo
    let diff = match get_diff_from_repo(engine, hash_str, &commit.source) {
        Some(d) => d,
        None => return to_c_json(&serde_json::json!({
            "success": false,
            "error": format!("Could not retrieve diff from {} repo. Run sync first.", commit.source)
        })),
    };

    // Auto-adapt Linux-specific patterns for macOS
    let adaptation_report = cauldron_sync::auto_adapt(&diff);
    let final_diff = if adaptation_report.was_adapted {
        adaptation_report.adapted_diff.clone()
    } else {
        diff.clone()
    };

    // Reconstruct ClassifiedCommit with (potentially adapted) diff
    let affected_files: Vec<String> = serde_json::from_str(&commit.affected_files).unwrap_or_default();
    let classified = cauldron_sync::ClassifiedCommit {
        hash: commit.hash.clone(),
        message: commit.message.clone(),
        author: commit.author.clone(),
        timestamp: commit.timestamp.clone(),
        diff: final_diff,
        affected_files,
        classification: cauldron_sync::Classification::from_str(&commit.classification),
        transferability: cauldron_sync::Transferability::from_str(&commit.transferability),
        suggested_action: String::new(),
    };

    let applicator = cauldron_sync::PatchApplicator::new(engine.wine_source_dir.clone());

    // User explicitly clicked Apply — bypass triage
    match applicator.force_apply_one(&classified) {
        Ok(outcome) => {
            match &outcome {
                cauldron_sync::PatchOutcome::Applied { hash, files_changed } => {
                    let _ = cauldron_db::mark_commit_applied(&conn, hash);
                    let _ = cauldron_db::insert_patch_log(&conn, hash, "applied", *files_changed, &[]);
                    to_c_json(&serde_json::json!({
                        "success": true,
                        "outcome": "applied",
                        "filesChanged": files_changed,
                        "adapted": adaptation_report.was_adapted,
                        "adaptationConfidence": adaptation_report.confidence,
                        "transformsApplied": adaptation_report.transforms_applied,
                        "adaptationWarnings": adaptation_report.warnings,
                    }))
                }
                cauldron_sync::PatchOutcome::Conflicted { hash, conflicts } => {
                    let _ = cauldron_db::insert_patch_log(&conn, hash, "conflicted", 0, conflicts);
                    to_c_json(&serde_json::json!({
                        "success": false,
                        "outcome": "conflicted",
                        "error": format!("Patch conflicts: {}", conflicts.join(", ")),
                    }))
                }
                cauldron_sync::PatchOutcome::Skipped { hash: _, reason } => {
                    to_c_json(&serde_json::json!({
                        "success": false,
                        "outcome": "skipped",
                        "error": format!("Triage decided to skip: {reason}"),
                    }))
                }
                cauldron_sync::PatchOutcome::Deferred { hash: _, reason } => {
                    to_c_json(&serde_json::json!({
                        "success": false,
                        "outcome": "deferred",
                        "error": format!("Needs manual review: {reason}"),
                    }))
                }
            }
        }
        Err(e) => to_c_json(&serde_json::json!({"success": false, "error": format!("Apply failed: {e}")})),
    }
}

/// Skip a patch by commit hash. Returns JSON with success/error.
#[no_mangle]
pub extern "C" fn cauldron_skip_patch(mgr: *mut c_void, hash: *const c_char) -> *mut c_char {
    if mgr.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let hash_str = match cstr_to_str(hash) {
        Some(s) => s,
        None => return to_c_json(&serde_json::json!({"success": false, "error": "null hash"})),
    };

    let conn = match cauldron_db::init_db(&engine.db_path) {
        Ok(c) => c,
        Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("DB error: {e}")})),
    };

    match cauldron_db::insert_patch_log(&conn, hash_str, "skipped", 0, &[]) {
        Ok(()) => to_c_json(&serde_json::json!({"success": true})),
        Err(e) => to_c_json(&serde_json::json!({"success": false, "error": format!("DB error: {e}")})),
    }
}

/// Reverse an applied patch via git revert. Returns JSON with success/error.
#[no_mangle]
pub extern "C" fn cauldron_reverse_patch(mgr: *mut c_void, hash: *const c_char) -> *mut c_char {
    if mgr.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &*(mgr as *const CauldronEngine) };
    let hash_str = match cstr_to_str(hash) {
        Some(s) => s,
        None => return to_c_json(&serde_json::json!({"success": false, "error": "null hash"})),
    };

    if !engine.wine_source_dir.join(".git").exists() {
        return to_c_json(&serde_json::json!({
            "success": false,
            "error": "Wine source tree not initialized."
        }));
    }

    // Find the wine source commit that applied this patch
    let output = std::process::Command::new("git")
        .args(["log", "--grep", &format!("Original commit: {}", hash_str), "--format=%H", "-1"])
        .current_dir(&engine.wine_source_dir)
        .output();

    let wine_commit = match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                return to_c_json(&serde_json::json!({
                    "success": false,
                    "error": "Could not find the corresponding commit in Wine source tree."
                }));
            }
            s
        }
        Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("Git error: {e}")})),
    };

    // Git revert
    let revert = std::process::Command::new("git")
        .args(["revert", "--no-edit", &wine_commit])
        .current_dir(&engine.wine_source_dir)
        .output();

    match revert {
        Ok(out) if out.status.success() => {
            // Update DB
            let conn = match cauldron_db::init_db(&engine.db_path) {
                Ok(c) => c,
                Err(e) => return to_c_json(&serde_json::json!({"success": false, "error": format!("DB error: {e}")})),
            };
            let _ = cauldron_db::mark_patch_reverted(&conn, hash_str);
            to_c_json(&serde_json::json!({"success": true}))
        }
        Ok(out) => {
            // Revert failed — abort and report
            let _ = std::process::Command::new("git")
                .args(["revert", "--abort"])
                .current_dir(&engine.wine_source_dir)
                .output();
            let stderr = String::from_utf8_lossy(&out.stderr);
            to_c_json(&serde_json::json!({
                "success": false,
                "error": format!("Revert conflicts — manual resolution needed. {stderr}")
            }))
        }
        Err(e) => to_c_json(&serde_json::json!({"success": false, "error": format!("Git error: {e}")})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_cauldron_init_and_free() {
        let tmp = tempfile::tempdir().unwrap();
        let path = CString::new(tmp.path().to_str().unwrap()).unwrap();

        let engine = cauldron_init(path.as_ptr());
        assert!(!engine.is_null());

        // Free should not crash
        cauldron_free(engine);
    }

    #[test]
    fn test_cauldron_init_null() {
        let engine = cauldron_init(std::ptr::null());
        assert!(engine.is_null());
    }

    #[test]
    fn test_cauldron_free_null() {
        // Should not crash
        cauldron_free(std::ptr::null_mut());
    }

    #[test]
    fn test_cauldron_free_string_null() {
        // Should not crash
        cauldron_free_string(std::ptr::null_mut());
    }

    #[test]
    fn test_cauldron_get_wine_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let path = CString::new(tmp.path().to_str().unwrap()).unwrap();
        let engine = cauldron_init(path.as_ptr());
        assert!(!engine.is_null());

        let ptr = cauldron_get_wine_versions(engine);
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert!(s.contains("9.0"));
        assert!(s.contains("10.0"));
        cauldron_free_string(ptr);
        cauldron_free(engine);
    }

    #[test]
    fn test_cauldron_get_wine_versions_null() {
        let ptr = cauldron_get_wine_versions(std::ptr::null_mut());
        assert!(ptr.is_null());
    }

    #[test]
    fn test_cauldron_get_installed_wine_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = CString::new(tmp.path().to_str().unwrap()).unwrap();
        let engine = cauldron_init(path.as_ptr());
        assert!(!engine.is_null());

        let ptr = cauldron_get_installed_wine(engine);
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(s, "[]");
        cauldron_free_string(ptr);
        cauldron_free(engine);
    }
}

