use std::path::{Path, PathBuf};

/// Known installation paths for the RosettaX87 binary.
const ROSETTAX87_SEARCH_PATHS: &[&str] = &[
    "/opt/homebrew/bin/rosettax87",
    "/usr/local/bin/rosettax87",
    "/opt/rosettax87/rosettax87",
];

/// Status of the RosettaX87 installation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RosettaX87Status {
    /// Whether RosettaX87 is installed and detected.
    pub available: bool,
    /// Path to the RosettaX87 binary, if found.
    pub path: String,
    /// Human-readable label.
    pub label: String,
}

/// Detect whether RosettaX87 is installed on this system.
///
/// RosettaX87 patches Apple's Rosetta to use faster (less precise) x87
/// floating-point handlers, providing 4-10x performance improvement on x87
/// FP operations. This benefits older games and mod loaders (SKSE, F4SE)
/// that rely heavily on x87 instructions.
///
/// See: <https://github.com/WineAndAqua/rosettax87>
pub fn detect_rosettax87() -> RosettaX87Status {
    // Check user-local install first (Cauldron-managed)
    if let Some(home) = dirs::home_dir() {
        let local_path = home.join("Library/Cauldron/rosettax87/rosettax87");
        if local_path.exists() {
            return RosettaX87Status {
                available: true,
                path: local_path.to_string_lossy().to_string(),
                label: "Installed (Cauldron-managed)".to_string(),
            };
        }
    }

    // Check system-wide paths
    for search_path in ROSETTAX87_SEARCH_PATHS {
        let p = Path::new(search_path);
        if p.exists() {
            return RosettaX87Status {
                available: true,
                path: search_path.to_string(),
                label: "Installed (system)".to_string(),
            };
        }
    }

    RosettaX87Status {
        available: false,
        path: String::new(),
        label: "Not installed".to_string(),
    }
}

/// Build the environment variables needed to enable RosettaX87.
///
/// When enabled, Wine processes launched through Rosetta will use the
/// patched x87 FP handlers for significantly faster floating-point
/// operations. Gcenx's Wine builds already support `ROSETTA_X87_PATH`.
pub fn build_rosettax87_env(enabled: bool) -> std::collections::HashMap<String, String> {
    let mut vars = std::collections::HashMap::new();

    if !enabled {
        return vars;
    }

    let status = detect_rosettax87();
    if status.available {
        vars.insert("ROSETTA_X87_PATH".to_string(), status.path);
        tracing::info!("RosettaX87 enabled — x87 FP acceleration active");
    } else {
        tracing::warn!("RosettaX87 requested but not found on system");
    }

    vars
}

/// Get the RosettaX87 binary path if available.
pub fn rosettax87_path() -> Option<PathBuf> {
    let status = detect_rosettax87();
    if status.available {
        Some(PathBuf::from(status.path))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_returns_valid_status() {
        let status = detect_rosettax87();
        // On most dev machines it won't be installed, but the function should not panic
        assert!(!status.label.is_empty());
        if status.available {
            assert!(!status.path.is_empty());
        }
    }

    #[test]
    fn test_build_env_disabled() {
        let vars = build_rosettax87_env(false);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_build_env_enabled_sets_path_if_available() {
        let vars = build_rosettax87_env(true);
        let status = detect_rosettax87();
        if status.available {
            assert!(vars.contains_key("ROSETTA_X87_PATH"));
        } else {
            assert!(vars.is_empty());
        }
    }
}
