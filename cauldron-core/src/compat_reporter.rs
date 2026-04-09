use cauldron_db::CompatStatus;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Command;

/// Describes the user's hardware and OS environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_version: String,
    pub chip: String,
    pub ram_gb: u32,
    pub gpu: String,
}

/// A community-submitted compatibility report for a game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatReport {
    pub game_id: String,
    pub status: CompatStatus,
    pub backend_used: String,
    pub fps_avg: Option<f32>,
    pub fps_min: Option<f32>,
    pub os_version: String,
    pub chip: String,
    pub ram_gb: u32,
    pub wine_version: String,
    pub notes: String,
    pub timestamp: String,
    pub reporter_hash: String,
}

/// Run a shell command and return trimmed stdout, or a fallback string on failure.
fn run_cmd(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Collect system information from the current macOS machine.
pub fn collect_system_info() -> Result<SystemInfo, Box<dyn std::error::Error>> {
    tracing::debug!("Collecting system information");
    let os_version = run_cmd("sw_vers", &["-productVersion"]);
    let chip = run_cmd("sysctl", &["-n", "machdep.cpu.brand_string"]);

    let ram_bytes_str = run_cmd("sysctl", &["-n", "hw.memsize"]);
    let ram_gb = ram_bytes_str
        .parse::<u64>()
        .map(|b| (b / (1024 * 1024 * 1024)) as u32)
        .unwrap_or(0);

    // Parse GPU chipset from system_profiler output
    let gpu_output = run_cmd("system_profiler", &["SPDisplaysDataType"]);
    let gpu = gpu_output
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("Chipset Model:") || trimmed.starts_with("Chip Model:") {
                trimmed.split(':').nth(1).map(|v| v.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    tracing::debug!(os = %os_version, chip = %chip, ram_gb = ram_gb, gpu = %gpu, "System information collected");
    Ok(SystemInfo {
        os_version,
        chip,
        ram_gb,
        gpu,
    })
}

/// Create a new compatibility report, automatically filling in system info and timestamp.
pub fn create_report(
    game_id: &str,
    status: &str,
    backend: &str,
    notes: &str,
) -> Result<CompatReport, Box<dyn std::error::Error>> {
    tracing::info!(game_id = %game_id, status = %status, backend = %backend, "Creating compatibility report");
    let sys = collect_system_info()?;
    let parsed_status: CompatStatus = status
        .parse()
        .map_err(|e: String| -> Box<dyn std::error::Error> { e.into() })?;

    let timestamp = chrono::Utc::now().to_rfc3339();
    let reporter_hash = generate_reporter_hash();

    tracing::info!(game_id = %game_id, status = %status, "Compatibility report created");
    Ok(CompatReport {
        game_id: game_id.to_string(),
        status: parsed_status,
        backend_used: backend.to_string(),
        fps_avg: None,
        fps_min: None,
        os_version: sys.os_version,
        chip: sys.chip,
        ram_gb: sys.ram_gb,
        wine_version: String::new(),
        notes: notes.to_string(),
        timestamp,
        reporter_hash,
    })
}

/// Save a compatibility report to the local SQLite database.
pub fn save_report_locally(
    conn: &Connection,
    report: &CompatReport,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!(game_id = %report.game_id, "Saving compat report to local database");
    conn.execute(
        "INSERT INTO compatibility_reports (game_id, reporter_hash, status, backend, fps_avg, notes, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            report.game_id,
            report.reporter_hash,
            report.status.to_string(),
            report.backend_used,
            report.fps_avg,
            report.notes,
            report.timestamp,
        ],
    )?;
    Ok(())
}

/// Export all locally stored compatibility reports as a JSON string.
pub fn export_reports(conn: &Connection) -> Result<String, Box<dyn std::error::Error>> {
    tracing::info!("Exporting all compatibility reports");
    let mut stmt = conn.prepare(
        "SELECT game_id, reporter_hash, status, backend, fps_avg, notes, timestamp
         FROM compatibility_reports ORDER BY timestamp ASC",
    )?;

    let reports: Vec<CompatReport> = stmt
        .query_map([], |row| {
            let status_str: String = row.get(2)?;  // index 2 = status column
            Ok(CompatReport {
                game_id: row.get(0)?,
                reporter_hash: row.get(1)?,
                status: status_str.parse().unwrap_or(CompatStatus::Unknown),
                backend_used: row.get(3)?,  // index 3 = backend column
                fps_avg: row.get(4)?,
                fps_min: None,
                os_version: String::new(),
                chip: String::new(),
                ram_gb: 0,
                wine_version: String::new(),
                notes: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let json = serde_json::to_string_pretty(&reports)?;
    Ok(json)
}

/// Generate a stable anonymous hash from hardware identifiers so duplicate
/// reports from the same user can be detected without identifying them.
pub fn generate_reporter_hash() -> String {
    let chip = run_cmd("sysctl", &["-n", "machdep.cpu.brand_string"]);
    let serial = run_cmd("ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"]);

    // Extract the IOPlatformSerialNumber line if present, otherwise use full output
    let serial_fragment = serial
        .lines()
        .find(|l| l.contains("IOPlatformSerialNumber"))
        .unwrap_or("")
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(chip.as_bytes());
    hasher.update(serial_fragment.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}
