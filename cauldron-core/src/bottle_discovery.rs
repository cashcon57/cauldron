use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Import error: {0}")]
    Import(String),
    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredBottle {
    pub name: String,
    pub path: PathBuf,
    pub source: BottleSource,
    pub wine_version: String,
    pub size_bytes: u64,
    pub has_steam: bool,
    pub game_count: usize,
    pub graphics_backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BottleSource {
    Whisky,
    CrossOver,
    Wineskin,
    StandaloneWine,
    Cauldron,
    Unknown,
}

impl fmt::Display for BottleSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BottleSource::Whisky => write!(f, "Whisky"),
            BottleSource::CrossOver => write!(f, "CrossOver"),
            BottleSource::Wineskin => write!(f, "Wineskin"),
            BottleSource::StandaloneWine => write!(f, "Standalone Wine"),
            BottleSource::Cauldron => write!(f, "Cauldron"),
            BottleSource::Unknown => write!(f, "Unknown"),
        }
    }
}

pub struct BottleDiscovery;

impl BottleDiscovery {
    /// Scan all known locations and return every discovered bottle.
    pub fn discover_all() -> Vec<DiscoveredBottle> {
        let mut bottles = Vec::new();
        bottles.extend(Self::discover_whisky());
        bottles.extend(Self::discover_crossover());
        bottles.extend(Self::discover_wineskin());
        bottles.extend(Self::discover_standalone());
        bottles
    }

    /// Discover Whisky bottles from known container and Application Support paths.
    pub fn discover_whisky() -> Vec<DiscoveredBottle> {
        let mut bottles = Vec::new();
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return bottles,
        };

        // Whisky stores bottles in various locations depending on version/fork.
        let whisky_container_ids = [
            "com.isaacmarovitz.Whisky",
            "com.isaacmarovitz.Whisky.WhiskyTester",
            "com.starward.Whisky",
        ];

        for container_id in &whisky_container_ids {
            let container_path = home
                .join("Library/Containers")
                .join(container_id)
                .join("Bottles");
            Self::scan_whisky_dir(&container_path, &mut bottles);
        }

        // Also check Application Support paths (varies by Whisky version).
        let app_support_paths = [
            home.join("Library/Application Support/Whisky/Bottles"),
            home.join("Library/Application Support/com.isaacmarovitz.Whisky/Bottles"),
        ];
        for path in &app_support_paths {
            Self::scan_whisky_dir(path, &mut bottles);
        }

        bottles
    }

    fn scan_whisky_dir(dir: &Path, bottles: &mut Vec<DiscoveredBottle>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Determine bottle name from config files or folder name.
            let name = Self::read_whisky_bottle_name(&path)
                .unwrap_or_else(|| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });

            let wine_version = Self::detect_wine_version(&path);
            let size_bytes = Self::estimate_size(&path);
            let has_steam = Self::check_steam_installed(&path);
            let game_count = Self::count_games(&path);
            let graphics_backend = Self::detect_graphics_backend(&path);

            bottles.push(DiscoveredBottle {
                name,
                path,
                source: BottleSource::Whisky,
                wine_version,
                size_bytes,
                has_steam,
                game_count,
                graphics_backend,
            });
        }
    }

    fn read_whisky_bottle_name(bottle_path: &Path) -> Option<String> {
        // Try Config.plist first, then bottle.plist
        for plist_name in &["Config.plist", "bottle.plist"] {
            let plist_path = bottle_path.join(plist_name);
            if let Ok(content) = fs::read_to_string(&plist_path) {
                // Simple extraction: look for <key>Name</key> or <key>name</key>
                // followed by <string>...</string>
                if let Some(name) = Self::extract_plist_string(&content, "Name")
                    .or_else(|| Self::extract_plist_string(&content, "name"))
                {
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
        None
    }

    /// Very simple plist string value extractor (avoids pulling in a plist crate).
    fn extract_plist_string(content: &str, key: &str) -> Option<String> {
        let key_tag = format!("<key>{}</key>", key);
        let pos = content.find(&key_tag)?;
        let after_key = &content[pos + key_tag.len()..];
        let string_start = after_key.find("<string>")?;
        let value_start = string_start + "<string>".len();
        let string_end = after_key[value_start..].find("</string>")?;
        Some(after_key[value_start..value_start + string_end].to_string())
    }

    /// Discover CrossOver bottles.
    pub fn discover_crossover() -> Vec<DiscoveredBottle> {
        let mut bottles = Vec::new();
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return bottles,
        };

        let crossover_dir = home.join("Library/Application Support/CrossOver/Bottles");
        let entries = match fs::read_dir(&crossover_dir) {
            Ok(e) => e,
            Err(_) => return bottles,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let (name, wine_ver) = Self::read_crossover_config(&path);
            let size_bytes = Self::estimate_size(&path);
            let has_steam = Self::check_steam_installed(&path);
            let game_count = Self::count_games(&path);
            let graphics_backend = Self::detect_graphics_backend(&path);

            bottles.push(DiscoveredBottle {
                name,
                path,
                source: BottleSource::CrossOver,
                wine_version: wine_ver,
                size_bytes,
                has_steam,
                game_count,
                graphics_backend,
            });
        }

        bottles
    }

    fn read_crossover_config(bottle_path: &Path) -> (String, String) {
        let conf_path = bottle_path.join("cxbottle.conf");
        let folder_name = bottle_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let content = match fs::read_to_string(&conf_path) {
            Ok(c) => c,
            Err(_) => return (folder_name, "unknown".to_string()),
        };

        let mut name = folder_name;
        let mut wine_ver = "unknown".to_string();

        // cxbottle.conf uses INI-like format with quoted keys:
        //   "Version" = "26.0.0.39794"
        //   "Template" = "win10_64"
        for line in content.lines() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with(";;") {
                continue;
            }
            // Parse "Key" = "Value" format
            if let Some((key, val)) = Self::parse_cx_kv(trimmed) {
                match key.as_str() {
                    "Version" => {
                        wine_ver = format!("CrossOver {}", val);
                    }
                    "Description" if !val.is_empty() => {
                        name = val;
                    }
                    _ => {}
                }
            }
            // Also check unquoted BottleName (older format)
            if let Some(val) = trimmed.strip_prefix("BottleName =") {
                let val = val.trim().trim_matches('"');
                if !val.is_empty() {
                    name = val.to_string();
                }
            } else if let Some(val) = trimmed.strip_prefix("BottleName=") {
                let val = val.trim().trim_matches('"');
                if !val.is_empty() {
                    name = val.to_string();
                }
            }
        }

        (name, wine_ver)
    }

    /// Parse a CrossOver config key-value line like `"Key" = "Value"`.
    fn parse_cx_kv(line: &str) -> Option<(String, String)> {
        let line = line.trim();
        if !line.starts_with('"') {
            return None;
        }
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return None;
        }
        let key = parts[0].trim().trim_matches('"').to_string();
        let val = parts[1].trim().trim_matches('"').to_string();
        if key.is_empty() {
            return None;
        }
        Some((key, val))
    }

    /// Discover Wineskin wrappers in ~/Applications.
    pub fn discover_wineskin() -> Vec<DiscoveredBottle> {
        let mut bottles = Vec::new();
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return bottles,
        };

        let apps_dir = home.join("Applications");
        let entries = match fs::read_dir(&apps_dir) {
            Ok(e) => e,
            Err(_) => return bottles,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if !name.ends_with(".app") {
                continue;
            }

            // Wineskin wrappers have Contents/SharedSupport/prefix/
            let prefix_path = path.join("Contents/SharedSupport/prefix");
            if !prefix_path.is_dir() {
                continue;
            }

            let display_name = name.trim_end_matches(".app").to_string();
            let wine_version = Self::detect_wine_version(&prefix_path);
            let size_bytes = Self::estimate_size(&prefix_path);
            let has_steam = Self::check_steam_installed(&prefix_path);
            let game_count = Self::count_games(&prefix_path);
            let graphics_backend = Self::detect_graphics_backend(&prefix_path);

            bottles.push(DiscoveredBottle {
                name: display_name,
                path: prefix_path,
                source: BottleSource::Wineskin,
                wine_version,
                size_bytes,
                has_steam,
                game_count,
                graphics_backend,
            });
        }

        bottles
    }

    /// Discover standalone Wine prefixes (~/.wine, ~/Wine Prefixes/).
    pub fn discover_standalone() -> Vec<DiscoveredBottle> {
        let mut bottles = Vec::new();
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return bottles,
        };

        // Default Wine prefix
        let default_prefix = home.join(".wine");
        if default_prefix.is_dir() && default_prefix.join("drive_c").is_dir() {
            let wine_version = Self::detect_wine_version(&default_prefix);
            let size_bytes = Self::estimate_size(&default_prefix);
            let has_steam = Self::check_steam_installed(&default_prefix);
            let game_count = Self::count_games(&default_prefix);
            let graphics_backend = Self::detect_graphics_backend(&default_prefix);

            bottles.push(DiscoveredBottle {
                name: "Default Wine Prefix".to_string(),
                path: default_prefix,
                source: BottleSource::StandaloneWine,
                wine_version,
                size_bytes,
                has_steam,
                game_count,
                graphics_backend,
            });
        }

        // ~/Wine Prefixes/ directory
        let wine_prefixes_dir = home.join("Wine Prefixes");
        if let Ok(entries) = fs::read_dir(&wine_prefixes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() || !path.join("drive_c").is_dir() {
                    continue;
                }

                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let wine_version = Self::detect_wine_version(&path);
                let size_bytes = Self::estimate_size(&path);
                let has_steam = Self::check_steam_installed(&path);
                let game_count = Self::count_games(&path);
                let graphics_backend = Self::detect_graphics_backend(&path);

                bottles.push(DiscoveredBottle {
                    name,
                    path,
                    source: BottleSource::StandaloneWine,
                    wine_version,
                    size_bytes,
                    has_steam,
                    game_count,
                    graphics_backend,
                });
            }
        }

        bottles
    }

    /// Discover Cauldron's own bottles.
    pub fn discover_cauldron(bottles_dir: &Path) -> Vec<DiscoveredBottle> {
        let mut bottles = Vec::new();
        let entries = match fs::read_dir(bottles_dir) {
            Ok(e) => e,
            Err(_) => return bottles,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Read bottle.toml for name/version info
            let config_path = path.join("bottle.toml");
            let (name, wine_version) = if config_path.exists() {
                Self::read_cauldron_bottle_config(&config_path)
            } else {
                let folder = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                (folder, "unknown".to_string())
            };

            let size_bytes = Self::estimate_size(&path);
            let has_steam = Self::check_steam_installed(&path);
            let game_count = Self::count_games(&path);
            let graphics_backend = Self::detect_graphics_backend(&path);

            bottles.push(DiscoveredBottle {
                name,
                path,
                source: BottleSource::Cauldron,
                wine_version,
                size_bytes,
                has_steam,
                game_count,
                graphics_backend,
            });
        }

        bottles
    }

    fn read_cauldron_bottle_config(config_path: &Path) -> (String, String) {
        let content = match fs::read_to_string(config_path) {
            Ok(c) => c,
            Err(_) => return ("unknown".to_string(), "unknown".to_string()),
        };

        #[derive(Deserialize)]
        struct BottleConfig {
            name: Option<String>,
            wine_version: Option<String>,
        }

        match toml::from_str::<BottleConfig>(&content) {
            Ok(cfg) => (
                cfg.name.unwrap_or_else(|| "unknown".to_string()),
                cfg.wine_version.unwrap_or_else(|| "unknown".to_string()),
            ),
            Err(_) => ("unknown".to_string(), "unknown".to_string()),
        }
    }

    /// Import a discovered bottle into Cauldron's bottles directory.
    /// If `symlink` is true, creates a symlink instead of copying.
    pub fn import_discovered(
        bottle: &DiscoveredBottle,
        target_dir: &Path,
        symlink: bool,
    ) -> Result<PathBuf, DiscoveryError> {
        use cauldron_db::GraphicsBackend;
        use std::collections::HashMap;

        fs::create_dir_all(target_dir)?;

        // Check if this bottle is already imported (symlink or dir pointing to same source)
        if let Ok(entries) = fs::read_dir(target_dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                // Check if it's a symlink pointing to the same source
                if let Ok(link_target) = fs::read_link(&entry_path) {
                    if link_target == bottle.path {
                        tracing::info!(
                            name = %bottle.name,
                            existing = %entry_path.display(),
                            "Bottle already imported, reusing existing symlink"
                        );
                        return Ok(entry_path);
                    }
                }
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let dest = target_dir.join(&id);

        if symlink {
            // Remove stale symlink if it exists (e.g., from interrupted previous import)
            if dest.exists() || dest.read_link().is_ok() {
                let _ = fs::remove_file(&dest);
            }

            #[cfg(unix)]
            std::os::unix::fs::symlink(&bottle.path, &dest)?;
            #[cfg(not(unix))]
            return Err(DiscoveryError::Import(
                "Symlinks not supported on this platform".to_string(),
            ));
        } else {
            copy_dir_recursive(&bottle.path, &dest, 0, 10)?;
        }

        // Parse the detected graphics backend string into the enum
        let gfx_backend = match bottle.graphics_backend.as_str() {
            "dxvk" => GraphicsBackend::DxvkMoltenVK,
            "dxmt" => GraphicsBackend::DXMT,
            "d3dmetal" => GraphicsBackend::D3DMetal,
            "moltenvk" => GraphicsBackend::DxvkMoltenVK,
            _ => GraphicsBackend::Auto,
        };

        // Write a proper Bottle-compatible bottle.toml that BottleManager can read
        let bottle_config = crate::bottle::Bottle {
            id: id.clone(),
            name: bottle.name.clone(),
            path: if symlink {
                bottle.path.clone()
            } else {
                dest.clone()
            },
            wine_version: bottle.wine_version.clone(),
            graphics_backend: gfx_backend,
            created_at: crate::bottle::chrono_like_timestamp(),
            env_overrides: HashMap::new(),
        };

        let config_toml = toml::to_string_pretty(&bottle_config)
            .map_err(|e| DiscoveryError::Import(format!("Failed to serialize bottle config: {e}")))?;

        // For symlinked bottles, write the config into the original path
        let config_path = if symlink {
            bottle.path.join("bottle.toml")
        } else {
            dest.join("bottle.toml")
        };
        fs::write(&config_path, config_toml)?;

        tracing::info!(
            name = %bottle.name,
            id = %id,
            source = %bottle.source,
            dest = %dest.display(),
            wine_version = %bottle.wine_version,
            graphics_backend = ?gfx_backend,
            symlink = symlink,
            "Imported discovered bottle"
        );

        Ok(dest)
    }

    // -----------------------------------------------------------------------
    // Heuristic helpers
    // -----------------------------------------------------------------------

    /// Get directory size using `du -sk` for accuracy and speed.
    fn estimate_size(path: &Path) -> u64 {
        match std::process::Command::new("du")
            .args(["-sk", &path.to_string_lossy()])
            .output()
        {
            Ok(output) if output.status.success() => {
                let out = String::from_utf8_lossy(&output.stdout);
                // du -sk output: "12345\t/path"
                out.split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|kb| kb * 1024)
                    .unwrap_or(0)
            }
            _ => 0,
        }
    }

    /// Check if Steam is installed in the bottle.
    fn check_steam_installed(prefix: &Path) -> bool {
        prefix
            .join("drive_c/Program Files (x86)/Steam/steam.exe")
            .exists()
    }

    /// Count games by inspecting Program Files directories and Steam ACF files.
    fn count_games(prefix: &Path) -> usize {
        let mut count = 0;

        // Count Steam ACF manifest files
        let steamapps = prefix.join("drive_c/Program Files (x86)/Steam/steamapps");
        if let Ok(entries) = fs::read_dir(&steamapps) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("appmanifest_") && name_str.ends_with(".acf") {
                    count += 1;
                }
            }
        }

        // Count non-system directories in Program Files and Program Files (x86)
        let system_dirs: &[&str] = &[
            "Common Files",
            "Internet Explorer",
            "Windows Defender",
            "Windows Media Player",
            "Windows NT",
            "Windows Mail",
            "WindowsPowerShell",
            "Microsoft.NET",
            "Steam",
        ];

        for pf_name in &["Program Files", "Program Files (x86)"] {
            let pf = prefix.join("drive_c").join(pf_name);
            if let Ok(entries) = fs::read_dir(&pf) {
                for entry in entries.flatten() {
                    if !entry.path().is_dir() {
                        continue;
                    }
                    let dirname = entry.file_name();
                    let dirname_str = dirname.to_string_lossy();
                    let is_system = system_dirs
                        .iter()
                        .any(|s| s.eq_ignore_ascii_case(&dirname_str));
                    if !is_system {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// Try to detect the Wine version from config or system files.
    fn detect_wine_version(prefix: &Path) -> String {
        // Try system.ini
        let system_ini = prefix.join("drive_c/windows/system.ini");
        if let Ok(content) = fs::read_to_string(&system_ini) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(ver) = trimmed.strip_prefix("wine=") {
                    return ver.trim().to_string();
                }
            }
        }

        // Try reading from the Wine version file (some setups write one)
        for version_file in &["wine_version", ".wine_version", "version"] {
            let vf = prefix.join(version_file);
            if let Ok(ver) = fs::read_to_string(&vf) {
                let ver = ver.trim();
                if !ver.is_empty() {
                    return ver.to_string();
                }
            }
        }

        "unknown".to_string()
    }

    /// Detect graphics backend by inspecting DLLs in the prefix.
    fn detect_graphics_backend(prefix: &Path) -> String {
        let system32 = prefix.join("drive_c/windows/system32");

        // Check for DXVK: d3d11.dll larger than 1 MB indicates DXVK override
        let d3d11 = system32.join("d3d11.dll");
        if let Ok(meta) = fs::metadata(&d3d11) {
            if meta.len() > 1_000_000 {
                return "dxvk".to_string();
            }
        }

        // Check for DXMT or D3DMetal indicators
        for indicator in &["d3dmt.dll", "dxmt.dll"] {
            if system32.join(indicator).exists() {
                return "dxmt".to_string();
            }
        }

        // Check for MoltenVK
        let lib_dir = prefix.join("drive_c/windows/system32");
        if lib_dir.join("libMoltenVK.dylib").exists()
            || lib_dir.join("MoltenVK_icd.json").exists()
        {
            return "moltenvk".to_string();
        }

        // Check for winemetal / d3dmetal
        if system32.join("d3dmetal.dll").exists() {
            return "d3dmetal".to_string();
        }

        "unknown".to_string()
    }
}

#[derive(Serialize)]
struct ImportedBottleConfig {
    name: String,
    wine_version: String,
    source: String,
    original_path: String,
    graphics_backend: String,
}

/// Create a filesystem-friendly slug from a name.
fn slug_name(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    // Collapse consecutive dashes
    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash {
                result.push(c);
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    result.trim_matches('-').to_string()
}

/// Recursively copy a directory up to max_depth levels.
fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    depth: u32,
    max_depth: u32,
) -> Result<(), DiscoveryError> {
    fs::create_dir_all(dst)?;

    if depth > max_depth {
        return Ok(());
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());

        if ft.is_file() {
            fs::copy(entry.path(), &dest_path)?;
        } else if ft.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path, depth + 1, max_depth)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottle_source_display() {
        assert_eq!(BottleSource::Whisky.to_string(), "Whisky");
        assert_eq!(BottleSource::CrossOver.to_string(), "CrossOver");
        assert_eq!(BottleSource::Wineskin.to_string(), "Wineskin");
        assert_eq!(BottleSource::StandaloneWine.to_string(), "Standalone Wine");
        assert_eq!(BottleSource::Cauldron.to_string(), "Cauldron");
        assert_eq!(BottleSource::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_slug_name() {
        assert_eq!(slug_name("My Cool Game"), "my-cool-game");
        assert_eq!(slug_name("  Test  Bottle  "), "test-bottle");
        assert_eq!(slug_name("hello_world"), "hello_world");
    }

    #[test]
    fn test_extract_plist_string() {
        let plist = r#"
<plist version="1.0">
<dict>
    <key>Name</key>
    <string>My Whisky Bottle</string>
    <key>Other</key>
    <string>value</string>
</dict>
</plist>"#;
        assert_eq!(
            BottleDiscovery::extract_plist_string(plist, "Name"),
            Some("My Whisky Bottle".to_string())
        );
        assert_eq!(
            BottleDiscovery::extract_plist_string(plist, "Missing"),
            None
        );
    }

    #[test]
    fn test_discover_all_does_not_panic() {
        // Should not panic even if no bottles exist on this system
        let _bottles = BottleDiscovery::discover_all();
    }

    #[test]
    fn test_discover_cauldron_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let bottles = BottleDiscovery::discover_cauldron(tmp.path());
        assert!(bottles.is_empty());
    }

    #[test]
    fn test_import_discovered_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("source_bottle");
        fs::create_dir_all(src.join("drive_c/windows/system32")).unwrap();
        fs::write(src.join("drive_c/test.txt"), "hello").unwrap();

        let bottle = DiscoveredBottle {
            name: "Test Bottle".to_string(),
            path: src,
            source: BottleSource::StandaloneWine,
            wine_version: "wine-9.0".to_string(),
            size_bytes: 100,
            has_steam: false,
            game_count: 0,
            graphics_backend: "unknown".to_string(),
        };

        let target = tmp.path().join("imported");
        let result = BottleDiscovery::import_discovered(&bottle, &target, false);
        assert!(result.is_ok());

        let imported_path = result.unwrap();
        assert!(imported_path.join("bottle.toml").exists());
        assert!(imported_path.join("drive_c/test.txt").exists());
    }
}
