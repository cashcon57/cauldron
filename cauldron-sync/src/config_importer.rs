use regex::Regex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigImportError {
    #[error("Could not locate default_compat_config() in script")]
    FunctionNotFound,
    #[error("Failed to parse app_id '{0}' as u32")]
    InvalidAppId(String),
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
}

type Result<T> = std::result::Result<T, ConfigImportError>;

// ---------------------------------------------------------------------------
// ProtonFlag
// ---------------------------------------------------------------------------

/// Individual compatibility flags used in Proton's `default_compat_config()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtonFlag {
    Gamedrive,
    Heapdelayfree,
    Heapzeromemory,
    Noopwr,
    Nofsync,
    Noesync,
    Forcelgadd,
    Noforcelgadd,
    Oldglstr,
    Hidenvgpu,
    Disablenvapi,
    Nomfdxgiman,
    Xalia,
    Nohardwarescheduling,
    Cmdlineappend(String),
    Unknown(String),
}

impl FromStr for ProtonFlag {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        // Handle cmdlineappend specially — it may appear as
        // `cmdlineappend:arg` or `cmdlineappend=arg`.
        if let Some(rest) = s
            .strip_prefix("cmdlineappend:")
            .or_else(|| s.strip_prefix("cmdlineappend="))
        {
            return Ok(ProtonFlag::Cmdlineappend(rest.to_string()));
        }

        let flag = match s.to_ascii_lowercase().as_str() {
            "gamedrive" => ProtonFlag::Gamedrive,
            "heapdelayfree" => ProtonFlag::Heapdelayfree,
            "heapzeromemory" => ProtonFlag::Heapzeromemory,
            "noopwr" => ProtonFlag::Noopwr,
            "nofsync" => ProtonFlag::Nofsync,
            "noesync" => ProtonFlag::Noesync,
            "forcelgadd" => ProtonFlag::Forcelgadd,
            "noforcelgadd" => ProtonFlag::Noforcelgadd,
            "oldglstr" => ProtonFlag::Oldglstr,
            "hidenvgpu" => ProtonFlag::Hidenvgpu,
            "disablenvapi" => ProtonFlag::Disablenvapi,
            "nomfdxgiman" => ProtonFlag::Nomfdxgiman,
            "xalia" => ProtonFlag::Xalia,
            "nohardwarescheduling" => ProtonFlag::Nohardwarescheduling,
            other if other == "cmdlineappend" => ProtonFlag::Cmdlineappend(String::new()),
            _ => ProtonFlag::Unknown(s.to_string()),
        };
        Ok(flag)
    }
}

impl fmt::Display for ProtonFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtonFlag::Gamedrive => write!(f, "gamedrive"),
            ProtonFlag::Heapdelayfree => write!(f, "heapdelayfree"),
            ProtonFlag::Heapzeromemory => write!(f, "heapzeromemory"),
            ProtonFlag::Noopwr => write!(f, "noopwr"),
            ProtonFlag::Nofsync => write!(f, "nofsync"),
            ProtonFlag::Noesync => write!(f, "noesync"),
            ProtonFlag::Forcelgadd => write!(f, "forcelgadd"),
            ProtonFlag::Noforcelgadd => write!(f, "noforcelgadd"),
            ProtonFlag::Oldglstr => write!(f, "oldglstr"),
            ProtonFlag::Hidenvgpu => write!(f, "hidenvgpu"),
            ProtonFlag::Disablenvapi => write!(f, "disablenvapi"),
            ProtonFlag::Nomfdxgiman => write!(f, "nomfdxgiman"),
            ProtonFlag::Xalia => write!(f, "xalia"),
            ProtonFlag::Nohardwarescheduling => write!(f, "nohardwarescheduling"),
            ProtonFlag::Cmdlineappend(arg) => write!(f, "cmdlineappend:{arg}"),
            ProtonFlag::Unknown(s) => write!(f, "{s}"),
        }
    }
}

// ---------------------------------------------------------------------------
// ProtonGameConfig
// ---------------------------------------------------------------------------

/// A single game's compatibility configuration parsed from the Proton script.
#[derive(Debug, Clone)]
pub struct ProtonGameConfig {
    pub app_id: u32,
    pub flags: Vec<ProtonFlag>,
    pub raw_line: String,
}

// ---------------------------------------------------------------------------
// MacOsEquivalent
// ---------------------------------------------------------------------------

/// A macOS-native translation of a single Proton compatibility flag.
#[derive(Debug, Clone)]
pub struct MacOsEquivalent {
    pub env_vars: HashMap<String, String>,
    pub wine_overrides: Vec<String>,
    pub notes: String,
}

// ---------------------------------------------------------------------------
// ImportStats
// ---------------------------------------------------------------------------

/// Statistics returned after a database import run.
#[derive(Debug, Clone, Default)]
pub struct ImportStats {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse the `default_compat_config()` function from a Proton script and
/// return all game compatibility entries it contains.
///
/// The function body is expected to contain lines like:
/// ```python
/// "1091500": "gamedrive heapdelayfree",  # Cyberpunk 2077
/// ```
pub fn parse_compat_config(script_content: &str) -> Result<Vec<ProtonGameConfig>> {
    // Step 1: locate the function body.  We look for the function definition
    // and then collect everything up to the closing `return` / next `def`.
    let func_re = Regex::new(
        r"(?s)def\s+default_compat_config\s*\(\s*\)\s*:.*?\{(.*?)\}",
    )?;

    let body = func_re
        .captures(script_content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())
        .ok_or(ConfigImportError::FunctionNotFound)?;

    // Step 2: parse individual entries.
    // Matches quoted app-id (single or double quotes), colon, quoted flags.
    let entry_re = Regex::new(
        r#"(?m)["'](\d+)["']\s*:\s*["']([^"']*)["']"#,
    )?;

    let mut configs = Vec::new();

    for cap in entry_re.captures_iter(body) {
        let id_str = &cap[1];
        let flags_str = &cap[2];

        let app_id: u32 = id_str
            .parse()
            .map_err(|_| ConfigImportError::InvalidAppId(id_str.to_string()))?;

        let flags: Vec<ProtonFlag> = flags_str
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| {
                // FromStr::from_str is infallible here.
                s.parse::<ProtonFlag>().unwrap()
            })
            .collect();

        let raw_line = cap[0].to_string();

        configs.push(ProtonGameConfig {
            app_id,
            flags,
            raw_line,
        });
    }

    tracing::info!(
        "Parsed {} game configs from default_compat_config()",
        configs.len()
    );

    Ok(configs)
}

// ---------------------------------------------------------------------------
// Flag translation
// ---------------------------------------------------------------------------

/// Translate a single Proton compatibility flag to its macOS equivalent.
///
/// Returns `None` for flags that have no meaningful macOS translation (yet).
pub fn translate_flag_to_macos(flag: &ProtonFlag) -> Option<MacOsEquivalent> {
    match flag {
        ProtonFlag::Gamedrive => {
            let mut env = HashMap::new();
            env.insert(
                "STEAM_COMPAT_INSTALL_PATH".to_string(),
                "<game_install_path>".to_string(),
            );
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Sets up a game drive symlink so the game can access its install directory \
                        as a Wine drive letter. Compatible with macOS."
                    .to_string(),
            })
        }
        ProtonFlag::Heapdelayfree => {
            let mut env = HashMap::new();
            env.insert("WINE_HEAP_DELAY_FREE".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Delays freeing heap allocations to work around use-after-free bugs. \
                        Requires the heap-delay-free Wine patch in our fork."
                    .to_string(),
            })
        }
        ProtonFlag::Heapzeromemory => {
            let mut env = HashMap::new();
            env.insert("WINE_HEAP_ZERO_MEMORY".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Zeros heap allocations to work around uninitialized memory bugs. \
                        Requires the heap-zero-memory Wine patch in our fork."
                    .to_string(),
            })
        }
        ProtonFlag::Noopwr => {
            // noopwr is primarily a Wayland/X11 presentation optimization.
            // On macOS with the winemac.drv compositor, the equivalent concern
            // is different. We log it but still set the env var in case future
            // Wine builds respect it.
            let mut env = HashMap::new();
            env.insert("WINE_DISABLE_OPWR".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Disables optimized presentation. Primarily Wayland-specific but \
                        set for compatibility."
                    .to_string(),
            })
        }
        ProtonFlag::Nofsync => Some(MacOsEquivalent {
            env_vars: HashMap::new(),
            wine_overrides: vec![],
            notes: "Disables fsync (Linux futex-based sync). On macOS, maps to disabling \
                    msync. Applied via msync_enabled=false in game_recommended_settings."
                .to_string(),
        }),
        ProtonFlag::Noesync => Some(MacOsEquivalent {
            env_vars: HashMap::new(),
            wine_overrides: vec![],
            notes: "Disables esync (eventfd-based sync). Applied via esync_enabled=false \
                    in game_recommended_settings."
                .to_string(),
        }),
        ProtonFlag::Forcelgadd => {
            let mut env = HashMap::new();
            env.insert("WINE_LARGE_ADDRESS_AWARE".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Forces the large-address-aware flag on 32-bit executables so they can \
                        use >2 GB of address space. Set via WINE_LARGE_ADDRESS_AWARE=1."
                    .to_string(),
            })
        }
        ProtonFlag::Hidenvgpu => {
            let mut env = HashMap::new();
            env.insert("WINE_HIDE_NVIDIA_GPU".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Hides NVIDIA GPU identity from the game to avoid vendor-specific code \
                        paths. Compatible with macOS (especially useful when running on Apple \
                        Silicon via MoltenVK)."
                    .to_string(),
            })
        }
        ProtonFlag::Disablenvapi => {
            let mut env = HashMap::new();
            env.insert("DXVK_ENABLE_NVAPI".to_string(), "0".to_string());
            env.insert("WINE_HIDE_NVIDIA_GPU".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec!["nvapi=d".to_string(), "nvapi64=d".to_string()],
                notes: "Disables DXVK-NVAPI emulation. Games on this list break when \
                        NVAPI is exposed. Sets DLL overrides to disable nvapi/nvapi64."
                    .to_string(),
            })
        }
        ProtonFlag::Nomfdxgiman => {
            let mut env = HashMap::new();
            env.insert("WINE_DISABLE_MF_DXGI_MANAGER".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Disables the Media Foundation DXGI device manager to work around \
                        crashes in video playback."
                    .to_string(),
            })
        }
        ProtonFlag::Nohardwarescheduling => {
            let mut env = HashMap::new();
            env.insert("WINE_DISABLE_HW_SCHEDULING".to_string(), "1".to_string());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: "Disables hardware GPU scheduling. Compatibility workaround."
                    .to_string(),
            })
        }
        ProtonFlag::Xalia => {
            // Xalia is an X11/Wayland launcher accessibility tool.
            // Not applicable on macOS — the winemac.drv handles this differently.
            tracing::debug!("Xalia flag skipped — not applicable on macOS");
            None
        }
        ProtonFlag::Noforcelgadd | ProtonFlag::Oldglstr => {
            // These are valid flags but have no direct macOS action.
            None
        }
        ProtonFlag::Cmdlineappend(arg) => {
            let mut env = HashMap::new();
            env.insert("PROTON_CMDLINE_APPEND".to_string(), arg.clone());
            Some(MacOsEquivalent {
                env_vars: env,
                wine_overrides: vec![],
                notes: format!(
                    "Appends '{}' to the game's command line. Review the argument for \
                     macOS compatibility before applying.",
                    arg
                ),
            })
        }
        ProtonFlag::Unknown(name) => {
            tracing::warn!("No macOS translation for unknown Proton flag: {}", name);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Database import
// ---------------------------------------------------------------------------

/// Import parsed game configs into a SQLite database.
///
/// Creates the `proton_game_configs` table if it does not exist, then
/// upserts each entry.  Returns statistics about how many rows were
/// inserted, updated, or skipped.
pub fn import_to_db(conn: &Connection, configs: &[ProtonGameConfig]) -> Result<ImportStats> {
    tracing::info!(config_count = configs.len(), "Starting import of Proton game configs to database");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS proton_game_configs (
            app_id      INTEGER PRIMARY KEY,
            flags       TEXT NOT NULL,
            raw_line    TEXT NOT NULL,
            macos_env   TEXT,
            macos_notes TEXT,
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    let mut stats = ImportStats::default();

    let mut insert_stmt = conn.prepare(
        "INSERT INTO proton_game_configs (app_id, flags, raw_line, macos_env, macos_notes, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
         ON CONFLICT(app_id) DO UPDATE SET
             flags       = excluded.flags,
             raw_line    = excluded.raw_line,
             macos_env   = excluded.macos_env,
             macos_notes = excluded.macos_notes,
             updated_at  = datetime('now')
         ",
    )?;

    for config in configs {
        let flags_str: String = config
            .flags
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Aggregate macOS translations for all flags on this game.
        let mut all_env: HashMap<String, String> = HashMap::new();
        let mut all_notes: Vec<String> = Vec::new();

        for flag in &config.flags {
            if let Some(equiv) = translate_flag_to_macos(flag) {
                all_env.extend(equiv.env_vars);
                if !equiv.notes.is_empty() {
                    all_notes.push(equiv.notes);
                }
            }
        }

        let macos_env = if all_env.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&all_env).unwrap_or_default())
        };
        let macos_notes = if all_notes.is_empty() {
            None
        } else {
            Some(all_notes.join("; "))
        };

        // Use `changes()` to distinguish insert vs update.
        // SQLite ON CONFLICT UPDATE always reports 1 change, so we check
        // whether the row existed before.
        let existed: bool = conn
            .query_row(
                "SELECT 1 FROM proton_game_configs WHERE app_id = ?1",
                [config.app_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        insert_stmt.execute(rusqlite::params![
            config.app_id,
            flags_str,
            config.raw_line,
            macos_env,
            macos_notes,
        ])?;

        if existed {
            stats.updated += 1;
        } else {
            stats.inserted += 1;
        }
    }

    // --- Phase 2B: Also upsert into game_recommended_settings ---
    // This makes the launch_config_resolver automatically apply these settings.
    let mut settings_upserted = 0;
    for config in configs {
        let mut env_vars: HashMap<String, String> = HashMap::new();
        let mut dll_overrides: HashMap<String, String> = HashMap::new();
        let mut msync_enabled: Option<bool> = None;
        let mut esync_enabled: Option<bool> = None;
        let mut large_address_aware: Option<bool> = None;
        let mut launch_args_parts: Vec<String> = Vec::new();

        for flag in &config.flags {
            match flag {
                ProtonFlag::Nofsync => msync_enabled = Some(false),
                ProtonFlag::Noesync => esync_enabled = Some(false),
                ProtonFlag::Forcelgadd => large_address_aware = Some(true),
                ProtonFlag::Noforcelgadd => large_address_aware = Some(false),
                ProtonFlag::Cmdlineappend(arg) => {
                    if !arg.is_empty() {
                        launch_args_parts.push(arg.clone());
                    }
                }
                ProtonFlag::Disablenvapi => {
                    env_vars.insert("DXVK_ENABLE_NVAPI".to_string(), "0".to_string());
                    env_vars.insert("WINE_HIDE_NVIDIA_GPU".to_string(), "1".to_string());
                    dll_overrides.insert("nvapi".to_string(), "d".to_string());
                    dll_overrides.insert("nvapi64".to_string(), "d".to_string());
                }
                other => {
                    if let Some(equiv) = translate_flag_to_macos(other) {
                        env_vars.extend(equiv.env_vars);
                        for ovr in &equiv.wine_overrides {
                            if let Some((dll, mode)) = ovr.split_once('=') {
                                dll_overrides.insert(dll.to_string(), mode.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Only upsert if we have something meaningful to set
        let has_settings = msync_enabled.is_some()
            || esync_enabled.is_some()
            || large_address_aware.is_some()
            || !env_vars.is_empty()
            || !dll_overrides.is_empty()
            || !launch_args_parts.is_empty();

        if has_settings {
            let env_json = if env_vars.is_empty() {
                "{}".to_string()
            } else {
                serde_json::to_string(&env_vars).unwrap_or_else(|_| "{}".to_string())
            };
            let dll_json = if dll_overrides.is_empty() {
                "{}".to_string()
            } else {
                serde_json::to_string(&dll_overrides).unwrap_or_else(|_| "{}".to_string())
            };
            let launch_args = if launch_args_parts.is_empty() {
                None
            } else {
                Some(launch_args_parts.join(" "))
            };

            // Use INSERT OR IGNORE + UPDATE to avoid overwriting user customizations.
            // Only set fields that come from Proton config; leave others as-is.
            conn.execute(
                "INSERT OR IGNORE INTO game_recommended_settings (steam_app_id) VALUES (?1)",
                [config.app_id],
            )?;

            if msync_enabled.is_some() {
                conn.execute(
                    "UPDATE game_recommended_settings SET msync_enabled = ?1 WHERE steam_app_id = ?2 AND msync_enabled IS NULL",
                    rusqlite::params![msync_enabled.map(|b| b as i32), config.app_id],
                )?;
            }
            if esync_enabled.is_some() {
                conn.execute(
                    "UPDATE game_recommended_settings SET esync_enabled = ?1 WHERE steam_app_id = ?2 AND esync_enabled IS NULL",
                    rusqlite::params![esync_enabled.map(|b| b as i32), config.app_id],
                )?;
            }
            if large_address_aware.is_some() {
                conn.execute(
                    "UPDATE game_recommended_settings SET large_address_aware = ?1 WHERE steam_app_id = ?2 AND large_address_aware IS NULL",
                    rusqlite::params![large_address_aware.map(|b| b as i32), config.app_id],
                )?;
            }
            if !env_vars.is_empty() {
                conn.execute(
                    "UPDATE game_recommended_settings SET env_vars = ?1 WHERE steam_app_id = ?2 AND env_vars = '{}'",
                    rusqlite::params![env_json, config.app_id],
                )?;
            }
            if !dll_overrides.is_empty() {
                conn.execute(
                    "UPDATE game_recommended_settings SET wine_dll_overrides = ?1 WHERE steam_app_id = ?2 AND wine_dll_overrides = '{}'",
                    rusqlite::params![dll_json, config.app_id],
                )?;
            }
            if let Some(ref args) = launch_args {
                conn.execute(
                    "UPDATE game_recommended_settings SET launch_args = ?1 WHERE steam_app_id = ?2 AND launch_args IS NULL",
                    rusqlite::params![args, config.app_id],
                )?;
            }

            settings_upserted += 1;
        }
    }

    tracing::info!(
        "Import complete: {} inserted, {} updated, {} skipped, {} game_recommended_settings upserted",
        stats.inserted,
        stats.updated,
        stats.skipped,
        settings_upserted,
    );

    Ok(stats)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SCRIPT: &str = r#"
#!/usr/bin/env python3
import os

class Proton:
    pass

def default_compat_config():
    ret = {
        "1091500": "gamedrive heapdelayfree",  # Cyberpunk 2077
        "1245620": "gamedrive heapdelayfree",  # Elden Ring
        "489830": "gamedrive",                  # Skyrim SE
        '275850': 'forcelgadd hidenvgpu',       # No Man's Sky
        "553850": "heapzeromemory noopwr",       # Helldivers 2
        "2630": "nofsync noesync",               # Call of Duty 2
        "1088850": "disablenvapi",               # GotG
        "1331440": "nomfdxgiman",                # FUSER
        "22300": "xalia gamedrive",              # Fallout 3
    }
    return ret

def other_func():
    pass
"#;

    #[test]
    fn parse_sample_script() {
        let configs = parse_compat_config(SAMPLE_SCRIPT).unwrap();
        assert_eq!(configs.len(), 9);

        assert_eq!(configs[0].app_id, 1091500);
        assert_eq!(configs[0].flags.len(), 2);
        assert_eq!(configs[0].flags[0], ProtonFlag::Gamedrive);
        assert_eq!(configs[0].flags[1], ProtonFlag::Heapdelayfree);

        assert_eq!(configs[2].app_id, 489830);
        assert_eq!(configs[2].flags, vec![ProtonFlag::Gamedrive]);

        // Single-quoted entry
        assert_eq!(configs[3].app_id, 275850);
        assert_eq!(
            configs[3].flags,
            vec![ProtonFlag::Forcelgadd, ProtonFlag::Hidenvgpu]
        );

        // New flags
        assert_eq!(configs[4].app_id, 553850); // Helldivers 2
        assert_eq!(configs[4].flags, vec![ProtonFlag::Heapzeromemory, ProtonFlag::Noopwr]);

        assert_eq!(configs[5].app_id, 2630); // CoD2
        assert_eq!(configs[5].flags, vec![ProtonFlag::Nofsync, ProtonFlag::Noesync]);

        assert_eq!(configs[6].app_id, 1088850); // GotG
        assert_eq!(configs[6].flags, vec![ProtonFlag::Disablenvapi]);

        assert_eq!(configs[7].app_id, 1331440); // FUSER
        assert_eq!(configs[7].flags, vec![ProtonFlag::Nomfdxgiman]);

        assert_eq!(configs[8].app_id, 22300); // Fallout 3
        assert!(configs[8].flags.contains(&ProtonFlag::Xalia));
        assert!(configs[8].flags.contains(&ProtonFlag::Gamedrive));
    }

    #[test]
    fn flag_display_roundtrip() {
        let flags = vec![
            ProtonFlag::Gamedrive,
            ProtonFlag::Heapdelayfree,
            ProtonFlag::Cmdlineappend("-dx11".to_string()),
            ProtonFlag::Unknown("customflag".to_string()),
        ];

        for flag in &flags {
            let s = flag.to_string();
            let parsed: ProtonFlag = s.parse().unwrap();
            // Unknown flags round-trip as Unknown
            match (flag, &parsed) {
                (ProtonFlag::Unknown(a), ProtonFlag::Unknown(b)) => assert_eq!(a, b),
                _ => assert_eq!(flag, &parsed),
            }
        }
    }

    #[test]
    fn translate_known_flags() {
        // Flags with macOS translations
        assert!(translate_flag_to_macos(&ProtonFlag::Gamedrive).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Heapdelayfree).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Heapzeromemory).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Forcelgadd).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Hidenvgpu).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Noopwr).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Nofsync).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Noesync).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Disablenvapi).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Nomfdxgiman).is_some());
        assert!(translate_flag_to_macos(&ProtonFlag::Nohardwarescheduling).is_some());

        // Flags without macOS equivalents
        assert!(translate_flag_to_macos(&ProtonFlag::Noforcelgadd).is_none());
        assert!(translate_flag_to_macos(&ProtonFlag::Oldglstr).is_none());
        assert!(translate_flag_to_macos(&ProtonFlag::Xalia).is_none());
        assert!(translate_flag_to_macos(&ProtonFlag::Unknown("foo".into())).is_none());
    }

    #[test]
    fn forcelgadd_sets_large_address() {
        let equiv = translate_flag_to_macos(&ProtonFlag::Forcelgadd).unwrap();
        assert_eq!(
            equiv.env_vars.get("WINE_LARGE_ADDRESS_AWARE"),
            Some(&"1".to_string())
        );
    }

    #[test]
    fn import_to_memory_db() {
        let conn = Connection::open_in_memory().unwrap();
        // Need game_recommended_settings table for the upsert
        cauldron_db::schema::run_migrations(&conn).unwrap();

        let configs = parse_compat_config(SAMPLE_SCRIPT).unwrap();

        let stats = import_to_db(&conn, &configs).unwrap();
        assert_eq!(stats.inserted, 9);
        assert_eq!(stats.updated, 0);

        // Import again — should update, not insert.
        let stats2 = import_to_db(&conn, &configs).unwrap();
        assert_eq!(stats2.inserted, 0);
        assert_eq!(stats2.updated, 9);
    }

    #[test]
    fn import_populates_game_recommended_settings() {
        let conn = Connection::open_in_memory().unwrap();
        cauldron_db::schema::run_migrations(&conn).unwrap();

        let configs = parse_compat_config(SAMPLE_SCRIPT).unwrap();
        import_to_db(&conn, &configs).unwrap();

        // CoD2 (2630) should have nofsync + noesync → msync_enabled=0, esync_enabled=0
        let settings = cauldron_db::get_game_settings(&conn, 2630).unwrap();
        assert!(settings.is_some());
        let s = settings.unwrap();
        assert_eq!(s.msync_enabled, Some(false));
        assert_eq!(s.esync_enabled, Some(false));

        // No Man's Sky (275850) should have large_address_aware=true
        let settings = cauldron_db::get_game_settings(&conn, 275850).unwrap();
        assert!(settings.is_some());
        assert_eq!(settings.unwrap().large_address_aware, Some(true));

        // GotG (1088850) should have disablenvapi env vars
        let settings = cauldron_db::get_game_settings(&conn, 1088850).unwrap();
        assert!(settings.is_some());
        let s = settings.unwrap();
        let env: HashMap<String, String> = serde_json::from_str(&s.env_vars).unwrap_or_default();
        assert_eq!(env.get("DXVK_ENABLE_NVAPI"), Some(&"0".to_string()));
    }

    #[test]
    fn disablenvapi_translation() {
        let equiv = translate_flag_to_macos(&ProtonFlag::Disablenvapi).unwrap();
        assert_eq!(equiv.env_vars.get("DXVK_ENABLE_NVAPI"), Some(&"0".to_string()));
        assert!(equiv.wine_overrides.contains(&"nvapi=d".to_string()));
        assert!(equiv.wine_overrides.contains(&"nvapi64=d".to_string()));
    }

    #[test]
    fn heapzeromemory_translation() {
        let equiv = translate_flag_to_macos(&ProtonFlag::Heapzeromemory).unwrap();
        assert_eq!(equiv.env_vars.get("WINE_HEAP_ZERO_MEMORY"), Some(&"1".to_string()));
    }

    #[test]
    fn missing_function_returns_error() {
        let result = parse_compat_config("def other(): pass");
        assert!(result.is_err());
    }
}
