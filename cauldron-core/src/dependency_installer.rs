//! Dependency installer for Wine bottles.
//!
//! Provides a list of available dependencies (winetricks verbs) and functions
//! to install them into a Wine prefix. These are commonly needed runtime
//! libraries for Windows games.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DependencyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Install failed for '{verb}': {message}")]
    InstallFailed { verb: String, message: String },
    #[error("Wine binary not found")]
    WineNotFound,
    #[error("Unknown dependency: {0}")]
    UnknownDependency(String),
}

/// Information about an installable dependency.
#[derive(Debug, Clone)]
pub struct DependencyInfo {
    /// The winetricks verb (e.g. "vcrun2019").
    pub verb: String,
    /// Human-readable description.
    pub description: String,
    /// Category for grouping in UI.
    pub category: String,
}

/// Return the list of all supported dependency verbs and their descriptions.
pub fn available_dependencies() -> Vec<DependencyInfo> {
    vec![
        // Visual C++ runtimes
        DependencyInfo {
            verb: "vcrun2019".to_string(),
            description: "Visual C++ 2015-2019 Redistributable".to_string(),
            category: "Runtime".to_string(),
        },
        DependencyInfo {
            verb: "vcrun2022".to_string(),
            description: "Visual C++ 2015-2022 Redistributable".to_string(),
            category: "Runtime".to_string(),
        },
        // .NET
        DependencyInfo {
            verb: "dotnet48".to_string(),
            description: ".NET Framework 4.8".to_string(),
            category: "Runtime".to_string(),
        },
        DependencyInfo {
            verb: "dotnet40".to_string(),
            description: ".NET Framework 4.0".to_string(),
            category: "Runtime".to_string(),
        },
        // DirectX
        DependencyInfo {
            verb: "d3dcompiler_47".to_string(),
            description: "DirectX D3DCompiler_47.dll".to_string(),
            category: "DirectX".to_string(),
        },
        DependencyInfo {
            verb: "d3dx9".to_string(),
            description: "DirectX 9 D3DX libraries".to_string(),
            category: "DirectX".to_string(),
        },
        DependencyInfo {
            verb: "d3dx10".to_string(),
            description: "DirectX 10 D3DX libraries".to_string(),
            category: "DirectX".to_string(),
        },
        DependencyInfo {
            verb: "d3dx11_43".to_string(),
            description: "DirectX 11 D3DX libraries".to_string(),
            category: "DirectX".to_string(),
        },
        DependencyInfo {
            verb: "dxvk".to_string(),
            description: "DXVK (DirectX 9/10/11 to Vulkan)".to_string(),
            category: "DirectX".to_string(),
        },
        // Media/codecs
        DependencyInfo {
            verb: "quartz".to_string(),
            description: "DirectShow runtime (quartz.dll)".to_string(),
            category: "Media".to_string(),
        },
        DependencyInfo {
            verb: "lavfilters".to_string(),
            description: "LAV Filters (open-source DirectShow codec pack)".to_string(),
            category: "Media".to_string(),
        },
        DependencyInfo {
            verb: "wmp9".to_string(),
            description: "Windows Media Player 9".to_string(),
            category: "Media".to_string(),
        },
        DependencyInfo {
            verb: "wmp11".to_string(),
            description: "Windows Media Player 11".to_string(),
            category: "Media".to_string(),
        },
        DependencyInfo {
            verb: "wmv9vcm".to_string(),
            description: "WMV9 Video Codec (wmv9vcm)".to_string(),
            category: "Media".to_string(),
        },
        DependencyInfo {
            verb: "devenum".to_string(),
            description: "Device Enumerator (devenum.dll)".to_string(),
            category: "Media".to_string(),
        },
        DependencyInfo {
            verb: "amstream".to_string(),
            description: "ActiveMovie Streaming (amstream.dll)".to_string(),
            category: "Media".to_string(),
        },
        // Other
        DependencyInfo {
            verb: "xact".to_string(),
            description: "XACT (Xbox Audio Cross-platform Toolkit)".to_string(),
            category: "Audio".to_string(),
        },
        DependencyInfo {
            verb: "xna40".to_string(),
            description: "XNA Framework 4.0".to_string(),
            category: "Runtime".to_string(),
        },
    ]
}

/// Check if a verb is in the known dependency list.
pub fn is_known_dependency(verb: &str) -> bool {
    available_dependencies().iter().any(|d| d.verb == verb)
}

/// Install a dependency verb into a Wine prefix using winetricks.
///
/// This function shells out to `winetricks` with the given prefix and verb.
/// Returns `Ok(())` on success.
pub fn install_dependency(
    wine_prefix: &Path,
    wine_bin: &Path,
    verb: &str,
) -> Result<(), DependencyError> {
    if !wine_bin.exists() {
        return Err(DependencyError::WineNotFound);
    }

    tracing::info!(verb = %verb, prefix = %wine_prefix.display(), "Installing dependency");

    // Try winetricks first
    let winetricks_paths = [
        "/opt/homebrew/bin/winetricks",
        "/usr/local/bin/winetricks",
    ];

    let winetricks = winetricks_paths
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| p.to_string());

    if let Some(wt) = winetricks {
        let output = Command::new(wt)
            .env("WINEPREFIX", wine_prefix)
            .env("WINE", wine_bin)
            .arg(verb)
            .output()?;

        if output.status.success() {
            tracing::info!(verb = %verb, "Dependency installed successfully via winetricks");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DependencyError::InstallFailed {
            verb: verb.to_string(),
            message: stderr.chars().take(500).collect(),
        });
    }

    Err(DependencyError::InstallFailed {
        verb: verb.to_string(),
        message: "winetricks not found".to_string(),
    })
}

/// Get dependencies grouped by category.
pub fn dependencies_by_category() -> HashMap<String, Vec<DependencyInfo>> {
    let mut map: HashMap<String, Vec<DependencyInfo>> = HashMap::new();
    for dep in available_dependencies() {
        map.entry(dep.category.clone()).or_default().push(dep);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_dependencies_not_empty() {
        let deps = available_dependencies();
        assert!(!deps.is_empty());
    }

    #[test]
    fn test_known_verbs() {
        assert!(is_known_dependency("vcrun2019"));
        assert!(is_known_dependency("vcrun2022"));
        assert!(is_known_dependency("d3dcompiler_47"));
        assert!(is_known_dependency("quartz"));
        assert!(is_known_dependency("lavfilters"));
        assert!(is_known_dependency("wmp9"));
        assert!(is_known_dependency("wmp11"));
        assert!(is_known_dependency("wmv9vcm"));
        assert!(is_known_dependency("devenum"));
        assert!(is_known_dependency("amstream"));
        assert!(!is_known_dependency("nonexistent_verb"));
    }

    #[test]
    fn test_dependencies_by_category() {
        let categories = dependencies_by_category();
        assert!(categories.contains_key("Runtime"));
        assert!(categories.contains_key("DirectX"));
        assert!(categories.contains_key("Media"));
    }

    #[test]
    fn test_media_verbs_present() {
        let categories = dependencies_by_category();
        let media = categories.get("Media").unwrap();
        let media_verbs: Vec<&str> = media.iter().map(|d| d.verb.as_str()).collect();
        assert!(media_verbs.contains(&"quartz"));
        assert!(media_verbs.contains(&"lavfilters"));
        assert!(media_verbs.contains(&"wmp9"));
        assert!(media_verbs.contains(&"wmp11"));
        assert!(media_verbs.contains(&"wmv9vcm"));
        assert!(media_verbs.contains(&"devenum"));
        assert!(media_verbs.contains(&"amstream"));
    }
}
