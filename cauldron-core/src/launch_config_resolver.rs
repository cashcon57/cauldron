//! Launch configuration resolver.
//!
//! Merges game-specific settings from multiple sources (DB game record,
//! game_recommended_settings table, and user overrides) into a single
//! [`LaunchConfig`] that the bridge uses to build the Wine launch environment.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A registry entry to apply to the Wine prefix before launch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryEntry {
    pub hive: String,
    pub key: String,
    pub name: String,
    pub reg_type: String,
    pub data: String,
}

/// A file operation to perform in the bottle before launch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileOperation {
    Rename { from: String, to: String },
    Delete { path: String },
    Copy { from: String, to: String },
}

/// User-provided overrides that take highest priority.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserLaunchOverrides {
    pub env_vars: HashMap<String, String>,
    pub dll_overrides: HashMap<String, String>,
    pub launch_args: Vec<String>,
    pub windows_version: Option<String>,
}

/// Fully resolved launch configuration for a game.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub env_vars: HashMap<String, String>,
    pub dll_overrides: HashMap<String, String>,
    pub launch_args: Vec<String>,
    pub exe_replacement: Option<String>,
    pub windows_version: Option<String>,
    pub cpu_topology: Option<String>,
    pub required_dependencies: Vec<String>,
    pub registry_entries: Vec<RegistryEntry>,
    pub file_operations: Vec<FileOperation>,
    pub auto_apply_patches: Option<bool>,
    pub audio_latency_ms: Option<i32>,
}

impl LaunchConfig {
    /// Apply the resolved environment variables (including DLL overrides) into
    /// an existing env map, as used by the bridge launch code.
    pub fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        // Apply env_vars
        for (k, v) in &self.env_vars {
            env.insert(k.clone(), v.clone());
        }

        // Apply DLL overrides into WINEDLLOVERRIDES
        if !self.dll_overrides.is_empty() {
            let dll_str: String = self
                .dll_overrides
                .iter()
                .map(|(dll, mode)| format!("{}={}", dll, mode))
                .collect::<Vec<_>>()
                .join(";");

            let existing = env.entry("WINEDLLOVERRIDES".to_string()).or_default();
            if !existing.is_empty() {
                existing.push(';');
            }
            existing.push_str(&dll_str);
        }

        // Apply cpu_topology
        if let Some(ref topo) = self.cpu_topology {
            env.insert("WINE_CPU_TOPOLOGY".to_string(), topo.clone());
        }

        // Apply audio latency
        if let Some(latency) = self.audio_latency_ms {
            env.insert("STAGING_AUDIO_PERIOD".to_string(), latency.to_string());
        }
    }
}

/// Resolve the launch configuration for a game by merging multiple data sources.
///
/// Priority (lowest to highest):
/// 1. `games.wine_overrides` JSON field (legacy)
/// 2. `game_recommended_settings` table (structured)
/// 3. User overrides
pub fn resolve(
    conn: &Connection,
    app_id: u32,
    _exe_name: &str,
    user_overrides: Option<&UserLaunchOverrides>,
) -> LaunchConfig {
    let mut config = LaunchConfig::default();

    // Layer 1: games table wine_overrides JSON
    if let Ok(Some(game)) = cauldron_db::get_game_by_app_id(conn, app_id) {
        if let Ok(overrides) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&game.wine_overrides) {
            // Extract dll_overrides from the JSON
            if let Some(dll_obj) = overrides.get("dll_overrides").and_then(|v| v.as_object()) {
                for (dll, mode) in dll_obj {
                    if let Some(m) = mode.as_str() {
                        config.dll_overrides.insert(dll.clone(), m.to_string());
                    }
                }
            }
            // Extract env_vars from the JSON
            if let Some(env_obj) = overrides.get("env_vars").and_then(|v| v.as_object()) {
                for (k, v) in env_obj {
                    if let Some(val) = v.as_str() {
                        config.env_vars.insert(k.clone(), val.to_string());
                    }
                }
            }
            // Extract windows_version
            if let Some(wv) = overrides.get("windows_version").and_then(|v| v.as_str()) {
                config.windows_version = Some(wv.to_string());
            }
            // Extract launch_args
            if let Some(args) = overrides.get("launch_args").and_then(|v| v.as_array()) {
                for arg in args {
                    if let Some(a) = arg.as_str() {
                        config.launch_args.push(a.to_string());
                    }
                }
            }
        }
    }

    // Layer 2: game_recommended_settings table (overrides layer 1)
    if let Ok(Some(settings)) = cauldron_db::get_game_settings(conn, app_id) {
        // msync_enabled
        if let Some(enabled) = settings.msync_enabled {
            if enabled {
                config.env_vars.insert("WINEMSYNC".to_string(), "1".to_string());
            } else {
                config.env_vars.insert("WINEMSYNC".to_string(), "0".to_string());
            }
        }

        // esync_enabled
        if let Some(enabled) = settings.esync_enabled {
            if enabled {
                config.env_vars.insert("WINEESYNC".to_string(), "1".to_string());
            } else {
                config.env_vars.insert("WINEESYNC".to_string(), "0".to_string());
            }
        }

        // rosetta_x87
        if let Some(enabled) = settings.rosetta_x87 {
            if enabled {
                config.env_vars.insert("ROSETTA_X87".to_string(), "1".to_string());
            }
        }

        // async_shader
        if let Some(enabled) = settings.async_shader {
            config.env_vars.insert(
                "DXVK_ASYNC".to_string(),
                if enabled { "1" } else { "0" }.to_string(),
            );
        }

        // metalfx_upscaling
        if let Some(enabled) = settings.metalfx_upscaling {
            if enabled {
                config.env_vars.insert("WINE_METALFX_UPSCALING".to_string(), "1".to_string());
            }
        }

        // dxr_ray_tracing
        if let Some(enabled) = settings.dxr_ray_tracing {
            config.env_vars.insert(
                "DXVK_ENABLE_DXR".to_string(),
                if enabled { "1" } else { "0" }.to_string(),
            );
        }

        // fsr_enabled
        if let Some(enabled) = settings.fsr_enabled {
            if enabled {
                config.env_vars.insert("WINE_FULLSCREEN_FSR".to_string(), "1".to_string());
            }
        }

        // large_address_aware
        if let Some(enabled) = settings.large_address_aware {
            if enabled {
                config.env_vars.insert("WINE_LARGE_ADDRESS_AWARE".to_string(), "1".to_string());
            }
        }

        // wine_dll_overrides JSON
        if let Ok(dll_map) = serde_json::from_str::<HashMap<String, String>>(&settings.wine_dll_overrides) {
            for (dll, mode) in dll_map {
                config.dll_overrides.insert(dll, mode);
            }
        }

        // env_vars JSON
        if let Ok(env_map) = serde_json::from_str::<HashMap<String, String>>(&settings.env_vars) {
            for (k, v) in env_map {
                config.env_vars.insert(k, v);
            }
        }

        // windows_version
        if let Some(wv) = settings.windows_version {
            config.windows_version = Some(wv);
        }

        // launch_args
        if let Some(args_str) = settings.launch_args {
            config.launch_args = args_str.split_whitespace().map(|s| s.to_string()).collect();
        }

        // auto_apply_patches
        if let Some(aap) = settings.auto_apply_patches {
            config.auto_apply_patches = Some(aap);
        }

        // cpu_topology
        if settings.cpu_topology.is_some() {
            config.cpu_topology = settings.cpu_topology;
        }

        // required_dependencies
        if let Ok(deps) = serde_json::from_str::<Vec<String>>(&settings.required_dependencies) {
            if !deps.is_empty() {
                config.required_dependencies = deps;
            }
        }

        // registry_entries
        if let Ok(entries) = serde_json::from_str::<Vec<RegistryEntry>>(&settings.registry_entries) {
            if !entries.is_empty() {
                config.registry_entries = entries;
            }
        }

        // exe_override
        if settings.exe_override.is_some() {
            config.exe_replacement = settings.exe_override;
        }

        // audio_latency_ms
        if settings.audio_latency_ms.is_some() {
            config.audio_latency_ms = settings.audio_latency_ms;
        }
    }

    // Layer 3: User overrides (highest priority)
    if let Some(overrides) = user_overrides {
        for (k, v) in &overrides.env_vars {
            config.env_vars.insert(k.clone(), v.clone());
        }
        for (dll, mode) in &overrides.dll_overrides {
            config.dll_overrides.insert(dll.clone(), mode.clone());
        }
        if !overrides.launch_args.is_empty() {
            config.launch_args = overrides.launch_args.clone();
        }
        if let Some(ref wv) = overrides.windows_version {
            config.windows_version = Some(wv.clone());
        }
    }

    config
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
    fn test_resolve_empty_db() {
        let conn = setup_db();
        let config = resolve(&conn, 99999, "game.exe", None);
        assert!(config.env_vars.is_empty());
        assert!(config.dll_overrides.is_empty());
        assert!(config.launch_args.is_empty());
        assert!(config.windows_version.is_none());
    }

    #[test]
    fn test_resolve_from_games_table() {
        let conn = setup_db();
        let game = cauldron_db::GameRecord {
            steam_app_id: Some(12345),
            exe_hash: Some("abc".to_string()),
            title: "Test Game".to_string(),
            backend: cauldron_db::GraphicsBackend::Auto,
            compat_status: cauldron_db::CompatStatus::Unknown,
            wine_overrides: r#"{"dll_overrides":{"d3d11":"n,b"},"env_vars":{"FOO":"bar"},"windows_version":"win10"}"#.to_string(),
            known_issues: String::new(),
            last_tested: String::new(),
            notes: String::new(),
        };
        cauldron_db::insert_game(&conn, &game).unwrap();

        let config = resolve(&conn, 12345, "game.exe", None);
        assert_eq!(config.dll_overrides.get("d3d11"), Some(&"n,b".to_string()));
        assert_eq!(config.env_vars.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(config.windows_version, Some("win10".to_string()));
    }

    #[test]
    fn test_resolve_from_recommended_settings() {
        let conn = setup_db();
        let settings = cauldron_db::GameRecommendedSettings {
            steam_app_id: 7670,
            msync_enabled: Some(false),
            esync_enabled: Some(false),
            rosetta_x87: None,
            async_shader: None,
            metalfx_upscaling: None,
            dxr_ray_tracing: None,
            fsr_enabled: None,
            large_address_aware: None,
            wine_dll_overrides: "{}".to_string(),
            env_vars: "{}".to_string(),
            windows_version: None,
            launch_args: None,
            auto_apply_patches: None,
            cpu_topology: None,
            required_dependencies: "[]".to_string(),
            registry_entries: "[]".to_string(),
            exe_override: None,
            audio_latency_ms: None,
        };
        cauldron_db::upsert_game_settings(&conn, &settings).unwrap();

        let config = resolve(&conn, 7670, "bioshock.exe", None);
        assert_eq!(config.env_vars.get("WINEMSYNC"), Some(&"0".to_string()));
        assert_eq!(config.env_vars.get("WINEESYNC"), Some(&"0".to_string()));
    }

    #[test]
    fn test_resolve_settings_override_games_table() {
        let conn = setup_db();

        // Layer 1: games table says win10
        let game = cauldron_db::GameRecord {
            steam_app_id: Some(100),
            exe_hash: Some("hash".to_string()),
            title: "Test".to_string(),
            backend: cauldron_db::GraphicsBackend::Auto,
            compat_status: cauldron_db::CompatStatus::Unknown,
            wine_overrides: r#"{"windows_version":"win10"}"#.to_string(),
            known_issues: String::new(),
            last_tested: String::new(),
            notes: String::new(),
        };
        cauldron_db::insert_game(&conn, &game).unwrap();

        // Layer 2: recommended settings say win7
        let settings = cauldron_db::GameRecommendedSettings {
            steam_app_id: 100,
            msync_enabled: None,
            esync_enabled: None,
            rosetta_x87: None,
            async_shader: None,
            metalfx_upscaling: None,
            dxr_ray_tracing: None,
            fsr_enabled: None,
            large_address_aware: None,
            wine_dll_overrides: "{}".to_string(),
            env_vars: "{}".to_string(),
            windows_version: Some("win7".to_string()),
            launch_args: None,
            auto_apply_patches: None,
            cpu_topology: None,
            required_dependencies: "[]".to_string(),
            registry_entries: "[]".to_string(),
            exe_override: None,
            audio_latency_ms: None,
        };
        cauldron_db::upsert_game_settings(&conn, &settings).unwrap();

        let config = resolve(&conn, 100, "game.exe", None);
        // Layer 2 should override layer 1
        assert_eq!(config.windows_version, Some("win7".to_string()));
    }

    #[test]
    fn test_user_overrides_highest_priority() {
        let conn = setup_db();

        let settings = cauldron_db::GameRecommendedSettings {
            steam_app_id: 200,
            msync_enabled: Some(false),
            esync_enabled: None,
            rosetta_x87: None,
            async_shader: None,
            metalfx_upscaling: None,
            dxr_ray_tracing: None,
            fsr_enabled: None,
            large_address_aware: None,
            wine_dll_overrides: "{}".to_string(),
            env_vars: "{}".to_string(),
            windows_version: Some("win7".to_string()),
            launch_args: None,
            auto_apply_patches: None,
            cpu_topology: None,
            required_dependencies: "[]".to_string(),
            registry_entries: "[]".to_string(),
            exe_override: None,
            audio_latency_ms: None,
        };
        cauldron_db::upsert_game_settings(&conn, &settings).unwrap();

        let user = UserLaunchOverrides {
            env_vars: [("WINEMSYNC".to_string(), "1".to_string())].into_iter().collect(),
            dll_overrides: HashMap::new(),
            launch_args: vec![],
            windows_version: Some("win10".to_string()),
        };

        let config = resolve(&conn, 200, "game.exe", Some(&user));
        assert_eq!(config.env_vars.get("WINEMSYNC"), Some(&"1".to_string()));
        assert_eq!(config.windows_version, Some("win10".to_string()));
    }

    #[test]
    fn test_resolve_cpu_topology() {
        let conn = setup_db();

        let settings = cauldron_db::GameRecommendedSettings {
            steam_app_id: 19900,
            msync_enabled: None,
            esync_enabled: None,
            rosetta_x87: None,
            async_shader: None,
            metalfx_upscaling: None,
            dxr_ray_tracing: None,
            fsr_enabled: None,
            large_address_aware: None,
            wine_dll_overrides: "{}".to_string(),
            env_vars: "{}".to_string(),
            windows_version: None,
            launch_args: None,
            auto_apply_patches: None,
            cpu_topology: Some("16:1".to_string()),
            required_dependencies: "[]".to_string(),
            registry_entries: "[]".to_string(),
            exe_override: None,
            audio_latency_ms: None,
        };
        cauldron_db::upsert_game_settings(&conn, &settings).unwrap();

        let config = resolve(&conn, 19900, "farcry2.exe", None);
        assert_eq!(config.cpu_topology, Some("16:1".to_string()));

        let mut env = HashMap::new();
        config.apply_to_env(&mut env);
        assert_eq!(env.get("WINE_CPU_TOPOLOGY"), Some(&"16:1".to_string()));
    }

    #[test]
    fn test_resolve_required_deps() {
        let conn = setup_db();

        let settings = cauldron_db::GameRecommendedSettings {
            steam_app_id: 1593500,
            msync_enabled: None,
            esync_enabled: None,
            rosetta_x87: None,
            async_shader: None,
            metalfx_upscaling: None,
            dxr_ray_tracing: None,
            fsr_enabled: None,
            large_address_aware: None,
            wine_dll_overrides: "{}".to_string(),
            env_vars: "{}".to_string(),
            windows_version: None,
            launch_args: None,
            auto_apply_patches: None,
            cpu_topology: None,
            required_dependencies: r#"["vcrun2022","d3dcompiler_47"]"#.to_string(),
            registry_entries: "[]".to_string(),
            exe_override: None,
            audio_latency_ms: None,
        };
        cauldron_db::upsert_game_settings(&conn, &settings).unwrap();

        let config = resolve(&conn, 1593500, "GoW.exe", None);
        assert_eq!(config.required_dependencies, vec!["vcrun2022", "d3dcompiler_47"]);
    }

    #[test]
    fn test_resolve_exe_override_and_audio() {
        let conn = setup_db();

        let settings = cauldron_db::GameRecommendedSettings {
            steam_app_id: 377840,
            msync_enabled: None,
            esync_enabled: None,
            rosetta_x87: None,
            async_shader: None,
            metalfx_upscaling: None,
            dxr_ray_tracing: None,
            fsr_enabled: None,
            large_address_aware: None,
            wine_dll_overrides: "{}".to_string(),
            env_vars: "{}".to_string(),
            windows_version: None,
            launch_args: None,
            auto_apply_patches: None,
            cpu_topology: None,
            required_dependencies: "[]".to_string(),
            registry_entries: "[]".to_string(),
            exe_override: Some("FF9_Launcher.exe".to_string()),
            audio_latency_ms: Some(60),
        };
        cauldron_db::upsert_game_settings(&conn, &settings).unwrap();

        let config = resolve(&conn, 377840, "ff9.exe", None);
        assert_eq!(config.exe_replacement, Some("FF9_Launcher.exe".to_string()));
        assert_eq!(config.audio_latency_ms, Some(60));

        let mut env = HashMap::new();
        config.apply_to_env(&mut env);
        assert_eq!(env.get("STAGING_AUDIO_PERIOD"), Some(&"60".to_string()));
    }

    #[test]
    fn test_apply_to_env_dll_overrides() {
        let mut config = LaunchConfig::default();
        config.dll_overrides.insert("d3d11".to_string(), "n,b".to_string());
        config.dll_overrides.insert("xaudio2_7".to_string(), "n".to_string());
        config.env_vars.insert("WINEMSYNC".to_string(), "0".to_string());

        let mut env = HashMap::new();
        config.apply_to_env(&mut env);

        assert_eq!(env.get("WINEMSYNC"), Some(&"0".to_string()));
        let overrides = env.get("WINEDLLOVERRIDES").unwrap();
        assert!(overrides.contains("d3d11=n,b"));
        assert!(overrides.contains("xaudio2_7=n"));
    }
}
