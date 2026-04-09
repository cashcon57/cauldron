mod completions;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

/// Cauldron — macOS game compatibility layer CLI
#[derive(Parser)]
#[command(name = "cauldron", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage Wine bottles (prefixes)
    Bottle {
        #[command(subcommand)]
        action: BottleAction,
    },
    /// Proton sync pipeline operations
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
    /// Game database operations
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
    /// Performance and compatibility reporting
    Perf {
        #[command(subcommand)]
        action: PerfAction,
    },
    /// Wine version management
    Wine {
        #[command(subcommand)]
        action: WineAction,
    },
    /// KosmicKrisp Vulkan driver management
    #[command(alias = "kosmickrisp")]
    Kk {
        #[command(subcommand)]
        action: KkAction,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Subcommand)]
enum BottleAction {
    /// Create a new Wine bottle
    Create {
        /// Name for the bottle
        name: String,
        /// Wine version to use
        #[arg(long, default_value = "wine-9.0")]
        wine_version: String,
    },
    /// List all bottles
    List,
    /// Delete a bottle by ID
    Delete {
        /// Bottle ID to delete
        id: String,
    },
    /// Launch a game in a bottle
    Launch {
        /// Bottle ID
        id: String,
        /// Path to the executable inside the bottle
        exe_path: String,
        /// Graphics backend override (d3dmetal, dxmt, dxvk-moltenvk, etc.)
        #[arg(long)]
        backend: Option<String>,
        /// Enable msync
        #[arg(long)]
        msync: bool,
        /// Enable esync
        #[arg(long)]
        esync: bool,
        /// Enable FidelityFX Super Resolution
        #[arg(long)]
        fsr: bool,
    },
    /// Export a bottle to a .tar.gz archive
    Export {
        /// Bottle ID to export
        id: String,
        /// Output archive path
        output_path: PathBuf,
    },
    /// Import a bottle from a .tar.gz archive
    Import {
        /// Path to the archive to import
        archive_path: PathBuf,
    },
    /// Scan a bottle for installed games
    Scan {
        /// Bottle ID to scan
        id: String,
    },
    /// Manage Wine registry overrides in a bottle
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    /// Discover existing Wine bottles from other apps (Whisky, CrossOver, etc.)
    Discover,
    /// Import a discovered bottle into Cauldron
    ImportDiscovered {
        /// Path to the discovered bottle
        path: PathBuf,
        /// Create a symlink instead of copying
        #[arg(long)]
        symlink: bool,
        /// Name override for the imported bottle
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum RegistryAction {
    /// List DLL overrides in a bottle
    ListOverrides {
        /// Bottle ID
        id: String,
    },
    /// Set a DLL override in a bottle
    SetOverride {
        /// Bottle ID
        id: String,
        /// DLL name (e.g., d3d11, dxgi)
        dll: String,
        /// Override mode (native, builtin, native,builtin, disabled)
        mode: String,
    },
}

#[derive(Debug, Subcommand)]
enum SyncAction {
    /// Poll the Proton repository for new commits
    Monitor {
        /// Path to local clone of the Proton repo
        #[arg(long, default_value = "./proton-repo")]
        repo_path: PathBuf,
        /// Remote URL to fetch from
        #[arg(long, default_value = "https://github.com/ValveSoftware/Proton.git")]
        remote_url: String,
    },
    /// Classify a commit by its hash (reads from the database)
    Classify {
        /// Commit hash to classify
        commit_hash: String,
    },
    /// Show sync pipeline status
    Status,
    /// Run one full sync cycle (poll + classify + adapt)
    Run {
        /// Path to local clone of the Proton repo
        #[arg(long, default_value = "./proton-repo")]
        repo_path: PathBuf,
        /// Remote URL to fetch from
        #[arg(long, default_value = "https://github.com/ValveSoftware/Proton.git")]
        remote_url: String,
    },
    /// Import Proton's default_compat_config() from a script
    ImportConfig {
        /// Path to the Proton Python script containing default_compat_config()
        proton_script_path: PathBuf,
    },
    /// Scan a protonfixes directory for game fix scripts
    ScanFixes {
        /// Path to the protonfixes scripts directory
        fixes_dir: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum DbAction {
    /// Initialize the game database
    Init {
        /// Path to the SQLite database file
        #[arg(long, default_value = "./cauldron.db")]
        path: PathBuf,
    },
    /// Load seed data from db/seed.sql
    Seed {
        /// Path to the SQLite database file
        #[arg(long, default_value = "./cauldron.db")]
        path: PathBuf,
    },
    /// Query a game by Steam App ID
    Query {
        /// Steam application ID
        app_id: u32,
    },
    /// Recommend a graphics backend for a game
    Recommend {
        /// Steam application ID
        app_id: u32,
    },
}

#[derive(Debug, Subcommand)]
enum PerfAction {
    /// Submit a compatibility report for a game
    Report {
        /// Steam App ID or game identifier
        app_id: String,
        /// Compatibility status (platinum, gold, silver, bronze, borked)
        status: String,
        /// Optional notes about the report
        #[arg(long, default_value = "")]
        notes: String,
    },
    /// Display current system information
    SystemInfo,
    /// List all shader caches
    CacheList,
    /// Clear a game's shader cache for a specific backend
    CacheClear {
        /// Game identifier
        game_id: String,
        /// Graphics backend (d3dmetal, dxvk, etc.)
        backend: String,
    },
}

#[derive(Debug, Subcommand)]
enum KkAction {
    /// Show KosmicKrisp detection status
    Status,
    /// Build KosmicKrisp from Mesa source
    Build {
        /// Mesa branch or tag to build (default: main)
        #[arg(long, default_value = "main")]
        mesa_version: String,
    },
    /// Check Vulkan extensions supported by KosmicKrisp
    CheckExtensions,
}

#[derive(Debug, Subcommand)]
enum WineAction {
    /// List available Wine versions for download
    List,
    /// Download and install a Wine version
    Install {
        /// Wine version to install (e.g. "10.0", "9.0", "gptk-2.0")
        version: String,
    },
    /// List installed Wine versions
    Installed,
    /// Validate a Wine installation
    Validate {
        /// Path to a Wine installation directory or binary
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_env("CAULDRON_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    tracing::info!("Cauldron CLI starting");

    let cli = Cli::parse();

    match cli.command {
        Commands::Bottle { ref action } => {
            tracing::info!("Executing bottle command");
            tracing::debug!(action = ?action, "Bottle subcommand");
        }
        Commands::Sync { ref action } => {
            tracing::info!("Executing sync command");
            tracing::debug!(action = ?action, "Sync subcommand");
        }
        Commands::Db { ref action } => {
            tracing::info!("Executing db command");
            tracing::debug!(action = ?action, "Db subcommand");
        }
        Commands::Perf { ref action } => {
            tracing::info!("Executing perf command");
            tracing::debug!(action = ?action, "Perf subcommand");
        }
        Commands::Wine { ref action } => {
            tracing::info!("Executing wine command");
            tracing::debug!(action = ?action, "Wine subcommand");
        }
        Commands::Kk { ref action } => {
            tracing::info!("Executing kk (KosmicKrisp) command");
            tracing::debug!(action = ?action, "Kk subcommand");
        }
        Commands::Completions { .. } => {
            tracing::info!("Generating shell completions");
        }
    }

    match cli.command {
        Commands::Bottle { action } => handle_bottle(action)?,
        Commands::Sync { action } => handle_sync(action).await?,
        Commands::Db { action } => handle_db(action)?,
        Commands::Perf { action } => handle_perf(action)?,
        Commands::Wine { action } => handle_wine(action)?,
        Commands::Kk { action } => handle_kk(action)?,
        Commands::Completions { shell } => {
            completions::generate_completions(shell);
        }
    }

    Ok(())
}

fn cauldron_base_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cauldron")
}

fn default_db_path() -> PathBuf {
    cauldron_base_dir().join("cauldron.db")
}

// ---------------------------------------------------------------------------
// Bottle commands
// ---------------------------------------------------------------------------

fn handle_bottle(action: BottleAction) -> Result<()> {
    let manager = cauldron_core::BottleManager::new(cauldron_base_dir());

    match action {
        BottleAction::Create { name, wine_version } => {
            let bottle = manager
                .create(&name, &wine_version)
                .context("Failed to create bottle")?;
            println!("Created bottle:");
            println!("  ID:            {}", bottle.id);
            println!("  Name:          {}", bottle.name);
            println!("  Wine version:  {}", bottle.wine_version);
            println!("  Path:          {}", bottle.path.display());
            println!("  Backend:       {}", bottle.graphics_backend);
            println!("  Created at:    {}", bottle.created_at);
        }
        BottleAction::List => {
            let bottles = manager.list().context("Failed to list bottles")?;
            if bottles.is_empty() {
                println!("No bottles found.");
            } else {
                println!("{:<38} {:<20} {:<12} {}", "ID", "NAME", "WINE", "BACKEND");
                println!("{}", "-".repeat(82));
                for b in &bottles {
                    println!(
                        "{:<38} {:<20} {:<12} {}",
                        b.id, b.name, b.wine_version, b.graphics_backend
                    );
                }
                println!("\n{} bottle(s) total.", bottles.len());
            }
        }
        BottleAction::Delete { id } => {
            manager
                .delete(&id)
                .context("Failed to delete bottle")?;
            println!("Deleted bottle {id}");
        }
        BottleAction::Launch {
            id,
            exe_path,
            backend,
            msync,
            esync,
            fsr,
        } => {
            let bottle = manager.get(&id).context("Failed to find bottle")?;
            println!("Launching in bottle '{}'...", bottle.name);
            println!("  Executable:  {exe_path}");
            if let Some(ref b) = backend {
                println!("  Backend:     {b}");
            }
            if msync {
                println!("  msync:       enabled");
            }
            if esync {
                println!("  esync:       enabled");
            }
            if fsr {
                println!("  FSR:         enabled");
            }
            // Build environment overrides
            let mut env = bottle.env_overrides.clone();
            if msync {
                env.insert("WINEMSYNC".to_string(), "1".to_string());
            }
            if esync {
                env.insert("WINEESYNC".to_string(), "1".to_string());
            }
            if fsr {
                env.insert("WINE_FULLSCREEN_FSR".to_string(), "1".to_string());
            }
            if let Some(ref b) = backend {
                env.insert("CAULDRON_BACKEND".to_string(), b.clone());
            }
            println!("  Environment: {} override(s) set", env.len());
            println!("\n(Launch execution not yet wired — environment prepared)");
        }
        BottleAction::Export { id, output_path } => {
            let bottle = manager.get(&id).context("Failed to find bottle")?;
            println!("Exporting bottle '{}' to {}...", bottle.name, output_path.display());

            let status = std::process::Command::new("tar")
                .arg("czf")
                .arg(&output_path)
                .arg("-C")
                .arg(bottle.path.parent().unwrap_or(&bottle.path))
                .arg(bottle.path.file_name().unwrap_or_default())
                .status()
                .context("Failed to run tar")?;

            if !status.success() {
                anyhow::bail!("tar exited with status: {status}");
            }
            println!("Exported bottle '{}' to {}", bottle.name, output_path.display());
        }
        BottleAction::Import { archive_path } => {
            println!("Importing bottle from {}...", archive_path.display());

            let bottles_dir = manager.bottles_dir.clone();
            std::fs::create_dir_all(&bottles_dir)
                .context("Failed to create bottles directory")?;

            let status = std::process::Command::new("tar")
                .arg("xzf")
                .arg(&archive_path)
                .arg("-C")
                .arg(&bottles_dir)
                .status()
                .context("Failed to run tar")?;

            if !status.success() {
                anyhow::bail!("tar extract exited with status: {status}");
            }
            println!("Imported bottle from {}", archive_path.display());
        }
        BottleAction::Scan { id } => {
            let bottle = manager.get(&id).context("Failed to find bottle")?;
            println!("Scanning bottle '{}' for installed games...\n", bottle.name);

            let games = cauldron_core::game_scanner::GameScanner::scan_bottle(&bottle.path, &id)
                .context("Failed to scan bottle")?;

            let steam_games = cauldron_core::game_scanner::GameScanner::detect_steam_apps(&bottle.path)
                .context("Failed to detect Steam apps")?;

            if games.is_empty() && steam_games.is_empty() {
                println!("No games found.");
            } else {
                if !games.is_empty() {
                    println!("Detected executables:");
                    println!("{:<30} {:<12} {:>10} {}", "TITLE", "DX", "SIZE", "PATH");
                    println!("{}", "-".repeat(80));
                    for g in &games {
                        let dx = g.dx_version.map_or("-".to_string(), |v| format!("DX{v}"));
                        let size_mb = g.size_bytes / (1024 * 1024);
                        println!(
                            "{:<30} {:<12} {:>7} MB  {}",
                            g.title, dx, size_mb,
                            g.exe_path.display()
                        );
                    }
                    println!("\n{} executable(s) found.", games.len());
                }
                if !steam_games.is_empty() {
                    println!("\nSteam games:");
                    for g in &steam_games {
                        println!(
                            "  [{}] {}",
                            g.steam_app_id.map_or("-".to_string(), |id| id.to_string()),
                            g.title
                        );
                    }
                    println!("{} Steam game(s) found.", steam_games.len());
                }
            }
        }
        BottleAction::Registry { action: reg_action } => {
            handle_registry(reg_action)?;
        }
        BottleAction::Discover => {
            use cauldron_core::bottle_discovery::BottleDiscovery;

            println!("Discovering existing Wine bottles...\n");

            let bottles = BottleDiscovery::discover_all();

            // Also discover Cauldron's own bottles
            let cauldron_bottles =
                BottleDiscovery::discover_cauldron(&manager.bottles_dir);

            let all: Vec<_> = bottles.into_iter().chain(cauldron_bottles).collect();

            if all.is_empty() {
                println!("No existing Wine bottles found.");
            } else {
                println!(
                    "{:<25} {:<15} {:<12} {:>10} {:>5} {}",
                    "NAME", "SOURCE", "WINE", "SIZE", "GAMES", "PATH"
                );
                println!("{}", "-".repeat(100));
                for b in &all {
                    let size_mb = b.size_bytes / (1024 * 1024);
                    let steam_indicator = if b.has_steam { " [Steam]" } else { "" };
                    println!(
                        "{:<25} {:<15} {:<12} {:>7} MB {:>5} {}{}",
                        truncate_str(&b.name, 24),
                        b.source,
                        b.wine_version,
                        size_mb,
                        b.game_count,
                        b.path.display(),
                        steam_indicator,
                    );
                }
                println!(
                    "\n{} bottle(s) discovered. Use 'cauldron bottle import-discovered <path>' to import.",
                    all.len()
                );
            }
        }
        BottleAction::ImportDiscovered {
            path,
            symlink,
            name,
        } => {
            use cauldron_core::bottle_discovery::{BottleDiscovery, BottleSource, DiscoveredBottle};

            if !path.is_dir() {
                anyhow::bail!("Path does not exist or is not a directory: {}", path.display());
            }

            let display_name = name.unwrap_or_else(|| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

            let bottle = DiscoveredBottle {
                name: display_name.clone(),
                path: path.clone(),
                source: BottleSource::Unknown,
                wine_version: "unknown".to_string(),
                size_bytes: 0,
                has_steam: false,
                game_count: 0,
                graphics_backend: "unknown".to_string(),
            };

            let mode = if symlink { "symlink" } else { "copy" };
            println!(
                "Importing bottle '{}' from {} (mode: {})...",
                display_name,
                path.display(),
                mode
            );

            let imported_path = BottleDiscovery::import_discovered(
                &bottle,
                &manager.bottles_dir,
                symlink,
            )
            .context("Failed to import discovered bottle")?;

            println!("Imported to: {}", imported_path.display());
        }
    }

    Ok(())
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn handle_registry(action: RegistryAction) -> Result<()> {
    let manager = cauldron_core::BottleManager::new(cauldron_base_dir());

    match action {
        RegistryAction::ListOverrides { id } => {
            let bottle = manager.get(&id).context("Failed to find bottle")?;
            let user_reg_path = bottle.path.join("user.reg");

            if !user_reg_path.exists() {
                println!("No user.reg found in bottle '{}'.", bottle.name);
                return Ok(());
            }

            let content = std::fs::read_to_string(&user_reg_path)
                .context("Failed to read user.reg")?;

            println!("DLL overrides in bottle '{}':\n", bottle.name);

            let mut in_section = false;
            let mut found = false;
            for line in content.lines() {
                if line.contains("[Software\\\\Wine\\\\DllOverrides]") {
                    in_section = true;
                    continue;
                }
                if in_section {
                    if line.starts_with('[') {
                        break;
                    }
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && trimmed.starts_with('"') {
                        println!("  {trimmed}");
                        found = true;
                    }
                }
            }

            if !found {
                println!("  (no DLL overrides set)");
            }
        }
        RegistryAction::SetOverride { id, dll, mode } => {
            let bottle = manager.get(&id).context("Failed to find bottle")?;
            let user_reg_path = bottle.path.join("user.reg");

            let section_header = "[Software\\\\Wine\\\\DllOverrides]";
            let entry = format!("\"*{}\"=\"{}\"", dll, mode);

            if user_reg_path.exists() {
                let content = std::fs::read_to_string(&user_reg_path)
                    .context("Failed to read user.reg")?;

                if content.contains(&entry) {
                    println!("Override already set: {dll}={mode}");
                    return Ok(());
                }

                let new_content = if content.contains(section_header) {
                    content.replacen(
                        section_header,
                        &format!("{}\n{}", section_header, entry),
                        1,
                    )
                } else {
                    format!("{}\n\n{}\n{}\n", content.trim_end(), section_header, entry)
                };

                std::fs::write(&user_reg_path, new_content)
                    .context("Failed to write user.reg")?;
            } else {
                let content = format!(
                    "WINE REGISTRY Version 2\n\n{}\n{}\n",
                    section_header, entry
                );
                std::fs::write(&user_reg_path, content)
                    .context("Failed to write user.reg")?;
            }

            println!("Set DLL override in bottle '{}': {dll}={mode}", bottle.name);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Sync commands
// ---------------------------------------------------------------------------

async fn handle_sync(action: SyncAction) -> Result<()> {
    match action {
        SyncAction::Monitor {
            repo_path,
            remote_url,
        } => {
            let monitor = cauldron_sync::ProtonMonitor::new(
                repo_path,
                remote_url,
                Duration::from_secs(300),
            );

            println!("Polling Proton repository...");
            let raw_commits = monitor
                .poll_once(None)
                .await
                .context("Failed to poll Proton repository")?;

            println!("Fetched {} commit(s). Classifying...\n", raw_commits.len());

            for raw in &raw_commits {
                let classified = cauldron_sync::classify(raw);
                println!("  {} {}", &classified.hash[..8], first_line(&classified.message));
                println!(
                    "    Classification: {}  Transferability: {}",
                    classified.classification, classified.transferability
                );
                println!("    Action: {}\n", classified.suggested_action);
            }

            println!("Poll complete. {} commit(s) processed.", raw_commits.len());
        }
        SyncAction::Classify { commit_hash } => {
            let db_path = default_db_path();
            let conn = cauldron_db::init_db(&db_path)
                .context("Failed to open database")?;

            // Look up the commit in the proton_commits table
            let mut stmt = conn
                .prepare(
                    "SELECT hash, message, author, timestamp, affected_files, classification, transferability, applied \
                     FROM proton_commits WHERE hash LIKE ?1 LIMIT 1",
                )
                .context("Failed to prepare query")?;

            let hash_pattern = format!("{}%", commit_hash);
            let commit: Option<cauldron_db::ProtonCommit> = stmt
                .query_map(rusqlite::params![hash_pattern], |row| {
                    Ok(cauldron_db::ProtonCommit {
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
                })
                .context("Failed to query commit")?
                .next()
                .transpose()
                .context("Failed to read commit row")?;

            match commit {
                Some(c) => {
                    println!("Commit: {}", c.hash);
                    println!("  Author:          {}", c.author);
                    println!("  Message:         {}", first_line(&c.message));
                    println!("  Timestamp:       {}", c.timestamp);
                    println!("  Classification:  {}", c.classification);
                    println!("  Transferability: {}", c.transferability);
                    println!("  Applied:         {}", c.applied);
                    println!("  Affected files:  {}", c.affected_files);
                }
                None => {
                    println!("No commit found matching hash prefix '{commit_hash}'.");
                }
            }
        }
        SyncAction::Status => {
            let db_path = default_db_path();
            let conn = cauldron_db::init_db(&db_path)
                .context("Failed to open database")?;

            let unapplied = cauldron_db::get_unapplied_commits(&conn)
                .context("Failed to query unapplied commits")?;

            let total: i64 = conn
                .query_row("SELECT COUNT(*) FROM proton_commits", [], |row| row.get(0))
                .unwrap_or(0);

            let last_sync: String = conn
                .query_row(
                    "SELECT COALESCE(MAX(timestamp), 'never') FROM proton_commits",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "never".to_string());

            println!("Sync Pipeline Status");
            println!("{}", "-".repeat(40));
            println!("  Total tracked commits: {total}");
            println!("  Pending (unapplied):   {}", unapplied.len());
            println!("  Applied:               {}", total as usize - unapplied.len());
            println!("  Last sync timestamp:   {last_sync}");

            if !unapplied.is_empty() {
                println!("\nPending commits:");
                for c in unapplied.iter().take(10) {
                    println!("  {} {} [{}]", &c.hash[..8.min(c.hash.len())], first_line(&c.message), c.classification);
                }
                if unapplied.len() > 10 {
                    println!("  ... and {} more", unapplied.len() - 10);
                }
            }
        }
        SyncAction::Run {
            repo_path,
            remote_url,
        } => {
            let db_path = default_db_path();
            let pipeline = cauldron_sync::SyncPipeline::new(
                repo_path,
                remote_url,
                db_path,
                Duration::from_secs(300),
            );

            println!("Running full sync cycle...\n");
            let result = pipeline.run_once().await.context("Sync cycle failed")?;

            println!("Sync Cycle Results");
            println!("{}", "-".repeat(40));
            println!("  Total commits:   {}", result.total_commits);
            println!("  Applied:         {}", result.applied);
            println!("  Pending review:  {}", result.pending_review);
            println!("  Skipped:         {}", result.skipped);
            println!("  Duration:        {:?}", result.duration);

            let cb = &result.classified;
            println!("\n  Classification breakdown:");
            println!("    Wine API fix:      {}", cb.wine_api_fix);
            println!("    DXVK fix:          {}", cb.dxvk_fix);
            println!("    VKD3D fix:         {}", cb.vkd3d_fix);
            println!("    Game config:       {}", cb.game_config);
            println!("    Kernel workaround: {}", cb.kernel_workaround);
            println!("    Steam integration: {}", cb.steam_integration);
            println!("    Build system:      {}", cb.build_system);
            println!("    Unknown:           {}", cb.unknown);

            if !result.errors.is_empty() {
                println!("\n  Errors:");
                for e in &result.errors {
                    println!("    - {e}");
                }
            }
        }
        SyncAction::ImportConfig { proton_script_path } => {
            println!(
                "Importing Proton compat config from {}...\n",
                proton_script_path.display()
            );

            let script_content = std::fs::read_to_string(&proton_script_path)
                .context("Failed to read Proton script")?;

            let configs = cauldron_sync::config_importer::parse_compat_config(&script_content)
                .context("Failed to parse compat config")?;

            println!("Parsed {} game configuration(s):\n", configs.len());
            for cfg in &configs {
                let flags: Vec<String> = cfg.flags.iter().map(|f| f.to_string()).collect();
                println!("  App ID {}: {}", cfg.app_id, flags.join(", "));
            }

            // Import into the database
            let db_path = default_db_path();
            let conn = cauldron_db::init_db(&db_path)
                .context("Failed to open database")?;

            let stats = cauldron_sync::config_importer::import_to_db(&conn, &configs)
                .context("Failed to import configs to database")?;

            println!("\nImport complete:");
            println!("  Inserted: {}", stats.inserted);
            println!("  Updated:  {}", stats.updated);
            println!("  Skipped:  {}", stats.skipped);
        }
        SyncAction::ScanFixes { fixes_dir } => {
            println!(
                "Scanning protonfixes directory: {}\n",
                fixes_dir.display()
            );

            let fixes = cauldron_sync::protonfixes::scan_fixes_directory(&fixes_dir)
                .context("Failed to scan fixes directory")?;

            if fixes.is_empty() {
                println!("No game fix scripts found.");
            } else {
                println!(
                    "{:<10} {:<35} {}",
                    "APP ID", "GAME", "ACTIONS"
                );
                println!("{}", "-".repeat(70));
                for fix in &fixes {
                    let actions_desc: Vec<String> = fix.actions.iter().map(|a| match a {
                        cauldron_sync::protonfixes::FixAction::InstallVerb(v) => format!("verb({v})"),
                        cauldron_sync::protonfixes::FixAction::SetEnvVar { key, .. } => format!("env({key})"),
                        cauldron_sync::protonfixes::FixAction::DllOverride { dll, .. } => format!("dll({dll})"),
                        cauldron_sync::protonfixes::FixAction::DisableNvapi => "no-nvapi".to_string(),
                        cauldron_sync::protonfixes::FixAction::AppendArgument(a) => format!("arg({a})"),
                        cauldron_sync::protonfixes::FixAction::ReplaceCommand { to, .. } => format!("replace->{to}"),
                        cauldron_sync::protonfixes::FixAction::CreateFile { path, .. } => format!("file({path})"),
                        cauldron_sync::protonfixes::FixAction::RenameFile { from, to } => format!("rename({from}->{to})"),
                        cauldron_sync::protonfixes::FixAction::DeleteFile { path } => format!("delete({path})"),
                        cauldron_sync::protonfixes::FixAction::CopyFile { from, to } => format!("copy({from}->{to})"),
                        cauldron_sync::protonfixes::FixAction::SetRegistry { key, name, .. } => format!("reg({key}\\{name})"),
                        cauldron_sync::protonfixes::FixAction::Unknown(_) => "unknown".to_string(),
                    }).collect();
                    println!(
                        "{:<10} {:<35} {}",
                        fix.app_id,
                        fix.game_name,
                        actions_desc.join(", ")
                    );
                }
                println!("\n{} fix script(s) found.", fixes.len());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DB commands
// ---------------------------------------------------------------------------

fn handle_db(action: DbAction) -> Result<()> {
    match action {
        DbAction::Init { path } => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create database directory")?;
            }
            let _conn = cauldron_db::init_db(&path)
                .context("Failed to initialize database")?;
            println!("Database initialized at {}", path.display());
        }
        DbAction::Seed { path } => {
            let conn = cauldron_db::init_db(&path)
                .context("Failed to open database")?;

            let seed_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("manifest dir has parent")
                .join("db/seed.sql");

            let sql = std::fs::read_to_string(&seed_path)
                .with_context(|| format!("Failed to read seed file at {}", seed_path.display()))?;

            conn.execute_batch(&sql)
                .context("Failed to execute seed SQL")?;

            println!("Seed data loaded from {}", seed_path.display());
        }
        DbAction::Query { app_id } => {
            let db_path = default_db_path();
            let conn = cauldron_db::init_db(&db_path)
                .context("Failed to open database")?;

            match cauldron_db::get_game_by_app_id(&conn, app_id)
                .context("Failed to query game")?
            {
                Some(game) => {
                    println!("Game: {}", game.title);
                    println!("  Steam App ID:    {:?}", game.steam_app_id);
                    println!("  Exe Hash:        {:?}", game.exe_hash);
                    println!("  Backend:         {}", game.backend);
                    println!("  Compat Status:   {}", game.compat_status);
                    println!("  Wine Overrides:  {}", game.wine_overrides);
                    println!("  Known Issues:    {}", game.known_issues);
                    println!("  Last Tested:     {}", game.last_tested);
                    println!("  Notes:           {}", game.notes);
                }
                None => {
                    println!("No game found with Steam App ID {app_id}.");
                }
            }
        }
        DbAction::Recommend { app_id } => {
            let db_path = default_db_path();
            let conn = cauldron_db::init_db(&db_path)
                .context("Failed to open database")?;

            let backend = cauldron_db::get_recommended_backend(&conn, Some(app_id), None)
                .context("Failed to get backend recommendation")?;

            println!("Recommended graphics backend for App ID {app_id}: {backend}");

            if let Some(game) = cauldron_db::get_game_by_app_id(&conn, app_id)
                .context("Failed to query game")?
            {
                println!("  Game:          {}", game.title);
                println!("  Compat Status: {}", game.compat_status);
                if !game.known_issues.is_empty() {
                    println!("  Known Issues:  {}", game.known_issues);
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Perf commands
// ---------------------------------------------------------------------------

fn handle_perf(action: PerfAction) -> Result<()> {
    match action {
        PerfAction::Report {
            app_id,
            status,
            notes,
        } => {
            println!("Submitting compatibility report for app {app_id}...\n");

            let report = cauldron_core::compat_reporter::create_report(
                &app_id,
                &status,
                "auto",
                &notes,
            )
            .map_err(|e| anyhow::anyhow!("Failed to create compatibility report: {e}"))?;

            println!("Report created:");
            println!("  Game ID:    {}", report.game_id);
            println!("  Status:     {}", report.status);
            println!("  Backend:    {}", report.backend_used);
            println!("  OS:         {}", report.os_version);
            println!("  Chip:       {}", report.chip);
            println!("  RAM:        {} GB", report.ram_gb);
            println!("  Timestamp:  {}", report.timestamp);
            if !report.notes.is_empty() {
                println!("  Notes:      {}", report.notes);
            }

            // Save to local DB
            let db_path = default_db_path();
            if let Ok(conn) = cauldron_db::init_db(&db_path) {
                match cauldron_core::compat_reporter::save_report_locally(&conn, &report) {
                    Ok(()) => println!("\nReport saved to local database."),
                    Err(e) => println!("\nWarning: could not save report locally: {e}"),
                }
            }
        }
        PerfAction::SystemInfo => {
            let info = cauldron_core::compat_reporter::collect_system_info()
                .map_err(|e| anyhow::anyhow!("Failed to collect system info: {e}"))?;

            println!("System Information");
            println!("{}", "-".repeat(40));
            println!("  macOS version:  {}", info.os_version);
            println!("  Chip:           {}", info.chip);
            println!("  RAM:            {} GB", info.ram_gb);
            println!("  GPU:            {}", info.gpu);
        }
        PerfAction::CacheList => {
            let cache_mgr = cauldron_core::ShaderCacheManager::new(cauldron_base_dir());
            let caches = cache_mgr
                .list_caches()
                .context("Failed to list shader caches")?;

            if caches.is_empty() {
                println!("No shader caches found.");
            } else {
                println!(
                    "{:<15} {:<12} {:>8} {:>10} {}",
                    "GAME ID", "BACKEND", "SHADERS", "SIZE", "UPDATED"
                );
                println!("{}", "-".repeat(65));
                for c in &caches {
                    let size_kb = c.size_bytes / 1024;
                    println!(
                        "{:<15} {:<12} {:>8} {:>7} KB  {}",
                        c.game_id, c.backend, c.shader_count, size_kb, c.last_updated
                    );
                }
                println!("\n{} cache(s) total.", caches.len());
            }

            let total_size = cache_mgr
                .total_cache_size()
                .unwrap_or(0);
            let total_mb = total_size / (1024 * 1024);
            println!("Total cache size: {} MB", total_mb);
        }
        PerfAction::CacheClear { game_id, backend } => {
            let cache_mgr = cauldron_core::ShaderCacheManager::new(cauldron_base_dir());
            cache_mgr
                .clear_cache(&game_id, &backend)
                .context("Failed to clear shader cache")?;
            println!("Cleared shader cache for {game_id}/{backend}");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Wine commands
// ---------------------------------------------------------------------------

fn handle_wine(action: WineAction) -> Result<()> {
    let wine_mgr = cauldron_core::WineManager::new(cauldron_base_dir());

    match action {
        WineAction::List => {
            let versions = wine_mgr.available_versions();
            println!("Available Wine versions:\n");
            println!(
                "{:<10} {:<10} {:<10} {}",
                "VERSION", "CATEGORY", "STATUS", "URL"
            );
            println!("{}", "-".repeat(100));
            for v in &versions {
                let status = if v.installed { "installed" } else { "available" };
                println!("{:<10} {:<10} {:<10} {}", v.version, v.category, status, v.url);
            }
            println!("\n{} version(s) listed.", versions.len());
            println!("\nTo install a version, run: cauldron wine install <VERSION>");
        }
        WineAction::Install { version } => {
            println!("Installing Wine version {version}...\n");
            match wine_mgr.download_version(&version) {
                Ok(wine_bin) => {
                    println!("\nWine {version} installed successfully.");
                    println!("  Binary: {}", wine_bin.display());

                    // Validate the installation
                    match cauldron_core::wine_downloader::validate_installation(&wine_bin) {
                        Ok(version_str) => {
                            println!("  Validated: {version_str}");
                        }
                        Err(e) => {
                            println!("  Warning: validation failed: {e}");
                            println!("  (The binary was downloaded but may not run on this system.)");
                        }
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to install Wine {version}: {e}");
                }
            }
        }
        WineAction::Installed => {
            let versions = wine_mgr.installed_versions();
            if versions.is_empty() {
                println!("No Wine versions installed.");
                println!("\nTo install a version, run: cauldron wine list");
            } else {
                println!("Installed Wine versions:\n");
                println!("{:<10} {:<50} {}", "VERSION", "PATH", "BINARY");
                println!("{}", "-".repeat(100));
                for v in &versions {
                    let bin_path = cauldron_core::wine_downloader::find_wine_binary(&v.path)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "(binary not found)".to_string());
                    println!("{:<10} {:<50} {}", v.version, v.path.display(), bin_path);
                }
                println!("\n{} version(s) installed.", versions.len());
            }
        }
        WineAction::Validate { path } => {
            println!("Validating Wine installation at {}...\n", path.display());

            // Try to find the wine binary
            let wine_bin = if path.is_file() {
                path.clone()
            } else {
                cauldron_core::wine_downloader::find_wine_binary(&path)
                    .context("Could not locate wine binary in the given path")?
            };

            match cauldron_core::wine_downloader::validate_installation(&wine_bin) {
                Ok(version_str) => {
                    println!("Valid Wine installation found.");
                    println!("  Binary:   {}", wine_bin.display());
                    println!("  Version:  {version_str}");
                }
                Err(e) => {
                    println!("Validation failed: {e}");
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// KosmicKrisp commands
// ---------------------------------------------------------------------------

fn handle_kk(action: KkAction) -> Result<()> {
    match action {
        KkAction::Status => {
            println!("KosmicKrisp Driver Status");
            println!("{}", "-".repeat(50));

            let status = cauldron_core::kosmickrisp::detect_kosmickrisp();

            if status.installed {
                let icd = status
                    .icd_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());
                println!("  Installed:    yes");
                println!("  ICD path:     {icd}");
                println!(
                    "  Vulkan ver:   {}",
                    status.vulkan_version.as_deref().unwrap_or("(unavailable)")
                );
                println!("  Extensions:   {}", status.supported_extensions.len());

                let gpl = if status.has_graphics_pipeline_library {
                    "YES"
                } else {
                    "no"
                };
                let tf = if status.has_transform_feedback {
                    "YES"
                } else {
                    "no"
                };
                println!(
                    "  graphics_pipeline_library: {}",
                    gpl
                );
                println!(
                    "  transform_feedback:        {}",
                    tf
                );

                let dxvk2 = cauldron_core::kosmickrisp::is_dxvk2_compatible(&status);
                println!();
                if dxvk2 {
                    println!("  DXVK 2.x compatible: YES");
                } else {
                    println!("  DXVK 2.x compatible: no (missing VK_EXT_graphics_pipeline_library)");
                }

                let driver = cauldron_core::kosmickrisp::VulkanDriver::KosmicKrisp(
                    status.icd_path.clone().unwrap(),
                );
                let env = cauldron_core::kosmickrisp::build_vulkan_env(&driver);
                println!();
                println!("  Environment variables to use this driver:");
                for (k, v) in &env {
                    println!("    {k}={v}");
                }
            } else {
                println!("  Installed:    no");
                println!();
                println!("  KosmicKrisp was not found on this system.");
                println!("  To build it from source, run:");
                println!("    cauldron kk build");
                println!("  or:");
                println!("    make build-kosmickrisp");
            }
        }
        KkAction::Build { mesa_version } => {
            println!("Building KosmicKrisp from Mesa source...\n");

            let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("manifest dir has parent")
                .join("scripts/build_kosmickrisp.sh");

            if !script.exists() {
                anyhow::bail!(
                    "Build script not found at {}. Make sure scripts/build_kosmickrisp.sh exists.",
                    script.display()
                );
            }

            let status = std::process::Command::new("bash")
                .arg(&script)
                .arg(&mesa_version)
                .status()
                .context("Failed to execute build script")?;

            if !status.success() {
                anyhow::bail!("KosmicKrisp build failed with exit code: {status}");
            }

            println!("\nBuild complete. Run 'cauldron kk status' to verify.");
        }
        KkAction::CheckExtensions => {
            let status = cauldron_core::kosmickrisp::detect_kosmickrisp();

            if !status.installed {
                println!("KosmicKrisp is not installed. Run 'cauldron kk build' first.");
                return Ok(());
            }

            let icd = status.icd_path.as_ref().unwrap();
            println!("Checking extensions for KosmicKrisp at {}...\n", icd.display());

            match cauldron_core::kosmickrisp::check_extensions(icd) {
                Ok(extensions) => {
                    if extensions.is_empty() {
                        println!("No extensions reported (vulkaninfo may have failed).");
                    } else {
                        println!("Supported Vulkan extensions ({}):\n", extensions.len());

                        // Highlight DXVK-critical extensions
                        let critical = [
                            "VK_EXT_graphics_pipeline_library",
                            "VK_EXT_transform_feedback",
                            "VK_EXT_extended_dynamic_state2",
                            "VK_EXT_extended_dynamic_state3",
                            "VK_KHR_dynamic_rendering",
                            "VK_KHR_synchronization2",
                            "VK_KHR_maintenance4",
                        ];

                        println!("DXVK-critical extensions:");
                        for ext in &critical {
                            let present = extensions.iter().any(|e| e == ext);
                            let marker = if present { "[x]" } else { "[ ]" };
                            println!("  {marker} {ext}");
                        }

                        println!("\nAll extensions:");
                        for ext in &extensions {
                            println!("  {ext}");
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to check extensions: {e}");
                    println!("Make sure vulkaninfo is installed (e.g. from the Vulkan SDK).");
                }
            }
        }
    }

    Ok(())
}

/// Return the first line of a string, for compact display.
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}
