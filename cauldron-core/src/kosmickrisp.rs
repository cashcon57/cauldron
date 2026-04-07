//! KosmicKrisp integration — detection, extension checking, and driver selection.
//!
//! KosmicKrisp is Mesa's Vulkan 1.3 driver that runs on Apple's Metal 4 API.
//! It can serve as an alternative to MoltenVK and may enable DXVK 2.x on macOS
//! by exposing extensions like `VK_EXT_graphics_pipeline_library`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Status of the KosmicKrisp driver on this system.
#[derive(Debug, Clone)]
pub struct KosmicKrispStatus {
    /// Whether a KosmicKrisp ICD JSON was found on disk.
    pub installed: bool,
    /// Path to the ICD JSON file, if found.
    pub icd_path: Option<PathBuf>,
    /// Vulkan API version reported by the driver (e.g. "1.3.290").
    pub vulkan_version: Option<String>,
    /// All Vulkan extensions the driver reports.
    pub supported_extensions: Vec<String>,
    /// Whether `VK_EXT_graphics_pipeline_library` is present (required by DXVK 2.x).
    pub has_graphics_pipeline_library: bool,
    /// Whether `VK_EXT_transform_feedback` is present.
    pub has_transform_feedback: bool,
}

/// Which Vulkan driver to use when launching a game with DXVK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VulkanDriver {
    /// Use KosmicKrisp (Mesa on Metal). Contains the path to the ICD JSON.
    KosmicKrisp(PathBuf),
    /// Use MoltenVK (the traditional Vulkan-on-Metal layer).
    MoltenVK,
    /// No usable Vulkan driver was found.
    None,
}

/// Detect whether KosmicKrisp is installed by searching well-known ICD locations.
///
/// The search order is:
/// 1. Cauldron's own build output (`build/kosmickrisp-install/share/vulkan/icd.d/`)
/// 2. Homebrew / system-wide (`/usr/local/share/vulkan/icd.d/`)
/// 3. Vulkan SDK default path (`/usr/local/etc/vulkan/icd.d/`)
/// 4. Home-local Vulkan path (`~/.local/share/vulkan/icd.d/`)
pub fn detect_kosmickrisp() -> KosmicKrispStatus {
    tracing::info!("Detecting KosmicKrisp installation");

    let search_dirs = build_search_dirs();

    for dir in &search_dirs {
        tracing::debug!(dir = %dir.display(), "Searching for KosmicKrisp ICD");
        if let Some(icd) = find_icd_in_dir(dir) {
            tracing::info!(icd = %icd.display(), "Found KosmicKrisp ICD");

            let extensions = check_extensions(&icd).unwrap_or_default();
            let vulkan_version = query_vulkan_version(&icd);
            let has_gpl = extensions
                .iter()
                .any(|e| e.contains("VK_EXT_graphics_pipeline_library"));
            let has_tf = extensions
                .iter()
                .any(|e| e.contains("VK_EXT_transform_feedback"));

            return KosmicKrispStatus {
                installed: true,
                icd_path: Some(icd),
                vulkan_version,
                supported_extensions: extensions,
                has_graphics_pipeline_library: has_gpl,
                has_transform_feedback: has_tf,
            };
        }
    }

    tracing::info!("KosmicKrisp not found");
    KosmicKrispStatus {
        installed: false,
        icd_path: None,
        vulkan_version: None,
        supported_extensions: Vec::new(),
        has_graphics_pipeline_library: false,
        has_transform_feedback: false,
    }
}

/// Run `vulkaninfo` with `VK_DRIVER_FILES` pointing at the given ICD JSON and
/// return the list of device extensions reported by the driver.
pub fn check_extensions(icd_path: &Path) -> Result<Vec<String>, String> {
    tracing::debug!(icd = %icd_path.display(), "Checking Vulkan extensions via vulkaninfo");

    let output = Command::new("vulkaninfo")
        .env("VK_DRIVER_FILES", icd_path)
        .output()
        .map_err(|e| format!("Failed to run vulkaninfo: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("vulkaninfo exited with error: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let extensions = parse_extensions(&stdout);

    tracing::info!(count = extensions.len(), "Parsed Vulkan extensions");
    Ok(extensions)
}

/// Returns `true` if the KosmicKrisp status indicates DXVK 2.x compatibility.
///
/// DXVK 2.x requires `VK_EXT_graphics_pipeline_library` at minimum.
pub fn is_dxvk2_compatible(status: &KosmicKrispStatus) -> bool {
    status.installed && status.has_graphics_pipeline_library
}

/// Build the environment variable map needed to direct the Vulkan loader to use
/// the KosmicKrisp driver.
pub fn build_kosmickrisp_env(icd_path: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert(
        "VK_DRIVER_FILES".to_string(),
        icd_path.to_string_lossy().into_owned(),
    );
    env.insert(
        "VK_LOADER_DRIVERS_SELECT".to_string(),
        "*kosmickrisp*".to_string(),
    );
    env
}

/// Choose the best Vulkan driver to use.
///
/// When `prefer_kosmickrisp` is `true` and KosmicKrisp is installed, it will be
/// selected.  Otherwise falls back to MoltenVK (assumed present if macOS Vulkan
/// SDK is installed) or `VulkanDriver::None`.
pub fn select_vulkan_driver(prefer_kosmickrisp: bool) -> VulkanDriver {
    if prefer_kosmickrisp {
        let status = detect_kosmickrisp();
        if let Some(icd) = status.icd_path {
            tracing::info!(icd = %icd.display(), "Selected KosmicKrisp as Vulkan driver");
            return VulkanDriver::KosmicKrisp(icd);
        }
        tracing::warn!("KosmicKrisp preferred but not found, falling back to MoltenVK");
    }

    // Check if MoltenVK is available (look for its ICD or dylib)
    if moltenvk_available() {
        tracing::info!("Selected MoltenVK as Vulkan driver");
        VulkanDriver::MoltenVK
    } else {
        tracing::warn!("No Vulkan driver found");
        VulkanDriver::None
    }
}

/// Build environment variables appropriate for the selected Vulkan driver.
pub fn build_vulkan_env(driver: &VulkanDriver) -> HashMap<String, String> {
    match driver {
        VulkanDriver::KosmicKrisp(icd) => build_kosmickrisp_env(icd),
        VulkanDriver::MoltenVK => {
            // MoltenVK typically works without extra env vars, but we can set
            // the driver select pattern to be explicit.
            let mut env = HashMap::new();
            env.insert(
                "VK_LOADER_DRIVERS_SELECT".to_string(),
                "*MoltenVK*".to_string(),
            );
            env
        }
        VulkanDriver::None => HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build the list of directories to search for KosmicKrisp ICD JSON files.
fn build_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Cauldron build output (relative to current working directory)
    dirs.push(PathBuf::from("build/kosmickrisp-install/share/vulkan/icd.d"));
    dirs.push(PathBuf::from(
        "build/kosmickrisp-install/etc/vulkan/icd.d",
    ));

    // 2. System-wide paths
    dirs.push(PathBuf::from("/usr/local/share/vulkan/icd.d"));
    dirs.push(PathBuf::from("/usr/local/etc/vulkan/icd.d"));
    dirs.push(PathBuf::from("/opt/homebrew/share/vulkan/icd.d"));

    // 3. Home-local
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/vulkan/icd.d"));
    }

    // 4. VulkanSDK paths
    if let Ok(sdk) = std::env::var("VULKAN_SDK") {
        dirs.push(PathBuf::from(format!("{sdk}/share/vulkan/icd.d")));
        dirs.push(PathBuf::from(format!("{sdk}/etc/vulkan/icd.d")));
    }

    dirs
}

/// Look for a file matching `*kosmickrisp*` inside the given directory.
fn find_icd_in_dir(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains("kosmickrisp") && name_str.ends_with(".json") {
            return Some(entry.path());
        }
    }
    None
}

/// Parse Vulkan extension names from `vulkaninfo` output.
///
/// Extensions appear in lines like:
///   `VK_KHR_swapchain                    : extension revision 70`
/// or inside a `Device Extensions` block.
fn parse_extensions(output: &str) -> Vec<String> {
    let mut extensions = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("VK_") {
            // Take just the extension name (first whitespace-delimited token)
            if let Some(name) = trimmed.split_whitespace().next() {
                extensions.push(name.to_string());
            }
        }
    }
    // Deduplicate (vulkaninfo may list instance + device extensions)
    extensions.sort();
    extensions.dedup();
    extensions
}

/// Try to extract the Vulkan API version from `vulkaninfo` output.
fn query_vulkan_version(icd_path: &Path) -> Option<String> {
    let output = Command::new("vulkaninfo")
        .arg("--summary")
        .env("VK_DRIVER_FILES", icd_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains("apiVersion") || trimmed.contains("Vulkan Instance Version") {
            // Try to extract a version like "1.3.290"
            for token in trimmed.split_whitespace() {
                if token.contains('.') && token.chars().next().map_or(false, |c| c.is_ascii_digit())
                {
                    return Some(token.trim_end_matches(')').to_string());
                }
            }
        }
    }
    None
}

/// Check whether MoltenVK is available on this system.
fn moltenvk_available() -> bool {
    let search_paths = [
        "/usr/local/share/vulkan/icd.d",
        "/opt/homebrew/share/vulkan/icd.d",
        "/usr/local/lib",
    ];

    for dir_path in &search_paths {
        let dir = Path::new(dir_path);
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.contains("MoltenVK") {
                    return true;
                }
            }
        }
    }

    // Also check if VULKAN_SDK is set (Vulkan SDK bundles MoltenVK)
    if std::env::var("VULKAN_SDK").is_ok() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extensions_typical() {
        let output = r#"
Device Extensions (count = 3):
    VK_KHR_swapchain                     : extension revision 70
    VK_EXT_graphics_pipeline_library     : extension revision  1
    VK_EXT_transform_feedback            : extension revision  1
"#;
        let exts = parse_extensions(output);
        assert_eq!(exts.len(), 3);
        assert!(exts.contains(&"VK_KHR_swapchain".to_string()));
        assert!(exts.contains(&"VK_EXT_graphics_pipeline_library".to_string()));
        assert!(exts.contains(&"VK_EXT_transform_feedback".to_string()));
    }

    #[test]
    fn test_parse_extensions_empty() {
        let exts = parse_extensions("");
        assert!(exts.is_empty());
    }

    #[test]
    fn test_parse_extensions_dedup() {
        let output = "VK_KHR_swapchain : rev 70\nVK_KHR_swapchain : rev 70\n";
        let exts = parse_extensions(output);
        assert_eq!(exts.len(), 1);
    }

    #[test]
    fn test_is_dxvk2_compatible_true() {
        let status = KosmicKrispStatus {
            installed: true,
            icd_path: Some(PathBuf::from("/tmp/test.json")),
            vulkan_version: Some("1.3.290".to_string()),
            supported_extensions: vec!["VK_EXT_graphics_pipeline_library".to_string()],
            has_graphics_pipeline_library: true,
            has_transform_feedback: false,
        };
        assert!(is_dxvk2_compatible(&status));
    }

    #[test]
    fn test_is_dxvk2_compatible_false_not_installed() {
        let status = KosmicKrispStatus {
            installed: false,
            icd_path: None,
            vulkan_version: None,
            supported_extensions: Vec::new(),
            has_graphics_pipeline_library: false,
            has_transform_feedback: false,
        };
        assert!(!is_dxvk2_compatible(&status));
    }

    #[test]
    fn test_is_dxvk2_compatible_false_missing_ext() {
        let status = KosmicKrispStatus {
            installed: true,
            icd_path: Some(PathBuf::from("/tmp/test.json")),
            vulkan_version: Some("1.3.290".to_string()),
            supported_extensions: vec!["VK_KHR_swapchain".to_string()],
            has_graphics_pipeline_library: false,
            has_transform_feedback: false,
        };
        assert!(!is_dxvk2_compatible(&status));
    }

    #[test]
    fn test_build_kosmickrisp_env() {
        let icd = PathBuf::from("/usr/local/share/vulkan/icd.d/kosmickrisp.json");
        let env = build_kosmickrisp_env(&icd);
        assert_eq!(
            env.get("VK_DRIVER_FILES").unwrap(),
            "/usr/local/share/vulkan/icd.d/kosmickrisp.json"
        );
        assert_eq!(
            env.get("VK_LOADER_DRIVERS_SELECT").unwrap(),
            "*kosmickrisp*"
        );
    }

    #[test]
    fn test_build_vulkan_env_moltenvk() {
        let env = build_vulkan_env(&VulkanDriver::MoltenVK);
        assert_eq!(
            env.get("VK_LOADER_DRIVERS_SELECT").unwrap(),
            "*MoltenVK*"
        );
    }

    #[test]
    fn test_build_vulkan_env_none() {
        let env = build_vulkan_env(&VulkanDriver::None);
        assert!(env.is_empty());
    }

    #[test]
    fn test_build_search_dirs_not_empty() {
        let dirs = build_search_dirs();
        assert!(!dirs.is_empty());
    }
}
