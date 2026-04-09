use serde::Serialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WineDownloadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Wine version not found: {0}")]
    VersionNotFound(String),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("Wine binary not found in: {0}")]
    BinaryNotFound(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

/// Represents a specific Wine version that can be downloaded or is installed.
#[derive(Debug, Clone, Serialize)]
pub struct WineVersion {
    /// The version string (e.g., "10.0", "9.0").
    pub version: String,
    /// The download URL for this version's tarball.
    pub url: String,
    /// Optional SHA-256 checksum for integrity verification.
    pub sha256: Option<String>,
    /// Whether this version is currently installed locally.
    pub installed: bool,
    /// The local filesystem path where this version is (or would be) installed.
    pub path: PathBuf,
    /// The category of the build (stable, devel, staging, gptk).
    pub category: String,
}

/// Manages downloading, extracting, and tracking Wine versions.
pub struct WineManager {
    /// Directory where Wine versions are stored.
    pub versions_dir: PathBuf,
    /// Cache of currently installed versions.
    pub installed_versions: Vec<WineVersion>,
}

impl WineManager {
    /// Create a new WineManager rooted at `base_dir`.
    ///
    /// Creates the versions directory if it does not exist, and scans for
    /// any already-installed Wine versions.
    pub fn new(base_dir: PathBuf) -> Self {
        let versions_dir = base_dir.join("wine-versions");
        tracing::debug!(versions_dir = %versions_dir.display(), "Initializing WineManager");
        if let Err(e) = std::fs::create_dir_all(&versions_dir) {
            tracing::warn!(error = %e, path = %versions_dir.display(), "Failed to create wine versions directory");
        }

        let mut manager = Self {
            versions_dir,
            installed_versions: Vec::new(),
        };
        manager.installed_versions = manager.scan_installed_versions();
        tracing::info!(installed_count = manager.installed_versions.len(), "WineManager initialized");
        manager
    }

    /// Returns a hardcoded list of known Wine builds available for download
    /// from Gcenx's macOS Wine builds repository.
    ///
    /// Gcenx releases are hosted at:
    ///   https://github.com/Gcenx/macOS_Wine_builds/releases
    ///
    /// The tarball filenames follow the pattern:
    ///   wine-stable-<version>-osx64.tar.xz   (stable)
    ///   wine-devel-<version>-osx64.tar.xz     (development)
    ///   wine-staging-<version>-osx64.tar.xz   (staging)
    pub fn available_versions(&self) -> Vec<WineVersion> {
        let known: Vec<(&str, &str, &str)> = vec![
            // Cauldron Wine — our patched fork (131 patches on Wine 11.6)
            // Includes: wine-staging, Proton, CrossOver macOS fixes, performance patches,
            // VirtualProtect COW fix, Mach write watches, GPU detection, and more.
            ("cauldron-11.6", "cauldron", "https://github.com/niceduckdev/cauldron-wine/releases/download/cauldron-11.6/cauldron-wine-11.6-macos-arm64.tar.xz"),
            // Upstream stable releases (vanilla, no patches)
            ("10.0", "stable", "https://github.com/Gcenx/macOS_Wine_builds/releases/download/10.0/wine-stable-10.0-osx64.tar.xz"),
            ("9.0", "stable", "https://github.com/Gcenx/macOS_Wine_builds/releases/download/9.0/wine-stable-9.0-osx64.tar.xz"),
            // Development releases (vanilla, bleeding-edge)
            ("11.6", "development", "https://github.com/Gcenx/macOS_Wine_builds/releases/download/11.6/wine-devel-11.6-osx64.tar.xz"),
            ("11.5", "development", "https://github.com/Gcenx/macOS_Wine_builds/releases/download/11.5/wine-devel-11.5-osx64.tar.xz"),
            // Staging releases
            ("10.3", "staging", "https://github.com/Gcenx/macOS_Wine_builds/releases/download/10.3/wine-staging-10.3-osx64.tar.xz"),
            // GPTK-style Wine (Apple Game Porting Toolkit compatible)
            ("gptk-2.0", "gptk", "https://github.com/Gcenx/macOS_Wine_builds/releases/download/gptk-2.0/wine-crossover-24.0.4-osx64.tar.xz"),
        ];

        let mut versions: Vec<WineVersion> = known
            .into_iter()
            .map(|(version, category, url)| {
                let install_path = self.versions_dir.join(version);
                let installed = install_path.exists();
                WineVersion {
                    version: version.to_string(),
                    url: url.to_string(),
                    sha256: None,
                    installed,
                    path: install_path,
                    category: category.to_string(),
                }
            })
            .collect();

        // Check for a local build from `make wine-build`
        let local_build = self.versions_dir.parent()
            .map(|base| base.join("build/wine-dist/bin/wine64"))
            .filter(|p| p.exists());
        if let Some(wine_bin) = local_build {
            let local_path = wine_bin.parent().unwrap().parent().unwrap().to_path_buf();
            // Insert at the top — local build takes priority
            versions.insert(0, WineVersion {
                version: "cauldron-11.6-local".to_string(),
                url: String::new(),
                sha256: None,
                installed: true,
                path: local_path,
                category: "cauldron".to_string(),
            });
        }

        versions
    }

    /// Scans the versions directory and returns all installed Wine versions.
    pub fn installed_versions(&self) -> Vec<WineVersion> {
        self.scan_installed_versions()
    }

    /// Download and extract a specific Wine version by its version string.
    ///
    /// Uses `curl` for the HTTP download (more reliable than reqwest for
    /// large files with progress indication) and `tar` for extraction.
    ///
    /// Returns the path to the Wine binary on success.
    pub fn download_version(
        &self,
        version: &str,
    ) -> Result<PathBuf, WineDownloadError> {
        tracing::info!(version = %version, "Starting Wine version download");
        let available = self.available_versions();
        let wine_version = available
            .iter()
            .find(|v| v.version == version)
            .ok_or_else(|| {
                tracing::error!(version = %version, "Wine version not found in available versions");
                WineDownloadError::VersionNotFound(version.to_string())
            })?;

        // 1. Check if already installed
        if wine_version.installed {
            tracing::info!(version = %version, "Wine version is already installed");
            let wine_bin = find_wine_binary(&wine_version.path)?;
            return Ok(wine_bin);
        }

        let install_dir = self.versions_dir.join(version);
        std::fs::create_dir_all(&install_dir)?;

        // 2. Create a temporary directory for the download
        let tmp_dir = tempfile::tempdir_in(&self.versions_dir)?;
        let url = &wine_version.url;

        // Determine filename from URL
        let filename = url
            .rsplit('/')
            .next()
            .unwrap_or("wine-download.tar.xz");
        let download_path = tmp_dir.path().join(filename);

        // 3. Download using curl
        tracing::info!(url = %url, dest = %download_path.display(), "Downloading Wine tarball with curl");
        let curl_status = std::process::Command::new("curl")
            .args([
                "-L",                        // follow redirects
                "--fail",                    // fail on HTTP errors
                "--progress-bar",            // show progress bar
                "-o",
                &download_path.to_string_lossy(),
                url,
            ])
            .status()
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to run curl");
                WineDownloadError::DownloadFailed(format!("Failed to run curl: {e}"))
            })?;

        if !curl_status.success() {
            // Clean up the install directory since we created it
            let _ = std::fs::remove_dir_all(&install_dir);
            return Err(WineDownloadError::DownloadFailed(format!(
                "curl exited with status: {curl_status}. URL: {url}"
            )));
        }

        if !download_path.exists() {
            let _ = std::fs::remove_dir_all(&install_dir);
            return Err(WineDownloadError::DownloadFailed(
                "Download file not found after curl completed".to_string(),
            ));
        }

        let file_size = std::fs::metadata(&download_path)
            .map(|m| m.len())
            .unwrap_or(0);
        tracing::info!(size_bytes = file_size, "Download complete");

        // 4. Extract based on file extension
        extract_archive(&download_path, &install_dir)?;

        // 5. Flatten the directory structure if needed.
        //    Some tarballs extract into a single subdirectory; if so, move
        //    its contents up to install_dir.
        flatten_single_subdir(&install_dir)?;

        // 6. Find and validate the wine binary
        let wine_bin = find_wine_binary(&install_dir)?;
        validate_installation(&wine_bin)?;

        // 7. Clean up temp files (handled by tempdir Drop, but be explicit)
        drop(tmp_dir);

        tracing::info!(version = %version, path = %install_dir.display(), wine_bin = %wine_bin.display(), "Wine version installed successfully");
        Ok(wine_bin)
    }

    /// Return the path to the latest installed Wine binary, if any.
    ///
    /// This is useful when creating a bottle without specifying a Wine path:
    /// Cauldron can automatically pick the most recent installed version.
    pub fn latest_installed_wine_binary(&self) -> Option<PathBuf> {
        let installed = self.scan_installed_versions();
        // Sort by version string descending (works for semver-like versions)
        let mut versions = installed;
        versions.sort_by(|a, b| b.version.cmp(&a.version));
        versions
            .first()
            .and_then(|v| find_wine_binary(&v.path).ok())
    }

    /// Internal: scan the versions directory for installed Wine versions.
    fn scan_installed_versions(&self) -> Vec<WineVersion> {
        let mut versions = Vec::new();

        let entries = match std::fs::read_dir(&self.versions_dir) {
            Ok(e) => e,
            Err(_) => return versions,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if this looks like a Wine installation
                    if find_wine_binary(&path).is_ok() {
                        versions.push(WineVersion {
                            version: name.to_string(),
                            url: String::new(),
                            sha256: None,
                            installed: true,
                            path,
                            category: String::new(),
                        });
                    }
                }
            }
        }

        versions
    }
}

/// Extract an archive into the given destination directory.
///
/// Supports `.tar.xz`, `.tar.gz`, `.tar.bz2`, and `.pkg` archives.
fn extract_archive(archive: &Path, dest: &Path) -> Result<(), WineDownloadError> {
    let filename = archive
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    tracing::info!(
        archive = %archive.display(),
        dest = %dest.display(),
        "Extracting Wine archive"
    );

    if filename.ends_with(".pkg") {
        // macOS .pkg files: use pkgutil to expand
        let status = std::process::Command::new("pkgutil")
            .args([
                "--expand-full",
                &archive.to_string_lossy(),
                &dest.to_string_lossy(),
            ])
            .status()
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to run pkgutil");
                WineDownloadError::ExtractionFailed(format!("Failed to run pkgutil: {e}"))
            })?;

        if !status.success() {
            return Err(WineDownloadError::ExtractionFailed(format!(
                "pkgutil exited with status: {status}"
            )));
        }
    } else {
        // tar-based archives: detect compression from extension
        let tar_flags = if filename.ends_with(".tar.xz") || filename.ends_with(".txz") {
            "xJf"
        } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
            "xzf"
        } else if filename.ends_with(".tar.bz2") {
            "xjf"
        } else {
            // Default: let tar auto-detect
            "xf"
        };

        let status = std::process::Command::new("tar")
            .args([tar_flags, &archive.to_string_lossy(), "-C", &dest.to_string_lossy()])
            .status()
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to run tar command");
                WineDownloadError::ExtractionFailed(format!("Failed to run tar: {e}"))
            })?;

        if !status.success() {
            tracing::error!(status = %status, "tar extraction failed");
            return Err(WineDownloadError::ExtractionFailed(format!(
                "tar exited with status: {status}"
            )));
        }
    }

    tracing::debug!(dest = %dest.display(), "Extraction complete");
    Ok(())
}

/// If the install directory contains exactly one subdirectory and no other
/// meaningful files, move its contents up one level. This handles tarballs
/// that extract into e.g. `wine-stable-10.0-osx64/` as a top-level dir.
fn flatten_single_subdir(dir: &Path) -> Result<(), WineDownloadError> {
    let entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();

    // Only flatten if there is exactly one entry and it is a directory
    if entries.len() == 1 && entries[0].path().is_dir() {
        let subdir = entries[0].path();
        tracing::debug!(
            "Flattening single subdirectory: {} -> {}",
            subdir.display(),
            dir.display()
        );

        // Move all contents of subdir up to dir
        for child in std::fs::read_dir(&subdir)?.filter_map(|e| e.ok()) {
            let child_path = child.path();
            let target = dir.join(child.file_name());
            std::fs::rename(&child_path, &target)?;
        }

        // Remove the now-empty subdirectory
        std::fs::remove_dir(&subdir)?;
    }

    Ok(())
}

/// Locate the wine binary within an extracted Wine distribution.
///
/// Handles the typical macOS Wine directory structures from Gcenx builds:
/// - `Wine Stable.app/Contents/Resources/wine/bin/wine`
/// - `Wine Devel.app/Contents/Resources/wine/bin/wine`
/// - `Wine Crossover.app/Contents/Resources/wine/bin/wine64`
/// - `wine/bin/wine`  (flat layout)
/// - `bin/wine`       (minimal layout)
pub fn find_wine_binary(wine_dir: &Path) -> Result<PathBuf, WineDownloadError> {
    // Common candidate paths relative to the wine directory.
    // Ordered from most specific (Gcenx .app bundles) to most generic.
    let candidates = [
        // Gcenx .app bundle layouts (Crossover-based builds, common for GPTK)
        "Wine Crossover.app/Contents/Resources/wine/bin/wine64",
        "Wine Crossover.app/Contents/Resources/wine/bin/wine",
        // Gcenx stable/devel/staging .app bundles
        "Wine Stable.app/Contents/Resources/wine/bin/wine64",
        "Wine Stable.app/Contents/Resources/wine/bin/wine",
        "Wine Devel.app/Contents/Resources/wine/bin/wine64",
        "Wine Devel.app/Contents/Resources/wine/bin/wine",
        "Wine Staging.app/Contents/Resources/wine/bin/wine64",
        "Wine Staging.app/Contents/Resources/wine/bin/wine",
        // Generic .app
        "Wine.app/Contents/Resources/wine/bin/wine64",
        "Wine.app/Contents/Resources/wine/bin/wine",
        // .app with direct bin (some CrossOver layouts)
        "Wine Crossover.app/Contents/SharedSupport/CrossOver/bin/wine64",
        "Wine Crossover.app/Contents/SharedSupport/CrossOver/bin/wine",
        // Flat tarball layouts (common when tarball extracts directly)
        "wine/bin/wine64",
        "wine/bin/wine",
        "bin/wine64",
        "bin/wine",
        // Homebrew-style layout
        "usr/local/bin/wine64",
        "usr/local/bin/wine",
        // pkg-expanded layout (payload inside Payload/)
        "Payload/usr/local/bin/wine64",
        "Payload/usr/local/bin/wine",
    ];

    for candidate in &candidates {
        let full_path = wine_dir.join(candidate);
        if full_path.exists() {
            tracing::debug!("Found wine binary at {}", full_path.display());
            return Ok(full_path);
        }
    }

    // Last resort: search recursively for a wine64 or wine binary
    if let Some(found) = search_for_wine_binary(wine_dir) {
        tracing::debug!("Found wine binary via search at {}", found.display());
        return Ok(found);
    }

    Err(WineDownloadError::BinaryNotFound(
        wine_dir.display().to_string(),
    ))
}

/// Recursively search a directory tree for a wine binary.
/// Returns the first `wine64` or `wine` found in a `bin/` directory.
fn search_for_wine_binary(dir: &Path) -> Option<PathBuf> {
    let walker = std::fs::read_dir(dir).ok()?;
    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // If this is a bin directory, check for wine binaries
            if path.file_name().map_or(false, |n| n == "bin") {
                let wine64 = path.join("wine64");
                if wine64.exists() {
                    return Some(wine64);
                }
                let wine = path.join("wine");
                if wine.exists() {
                    return Some(wine);
                }
            }
            // Recurse into subdirectories (limit depth to avoid going too deep)
            if let Some(found) = search_for_wine_binary(&path) {
                return Some(found);
            }
        }
    }
    None
}

/// Validate an installed Wine binary by running `wine --version`.
///
/// Returns the version string on success (e.g., "wine-10.0").
pub fn validate_installation(wine_bin: &Path) -> Result<String, WineDownloadError> {
    tracing::debug!(wine_bin = %wine_bin.display(), "Validating Wine installation");
    let output = std::process::Command::new(wine_bin)
        .arg("--version")
        .output()
        .map_err(|e| {
            tracing::warn!(wine_bin = %wine_bin.display(), error = %e, "Failed to execute Wine binary for validation");
            WineDownloadError::ValidationFailed(format!(
                "Failed to execute {}: {e}",
                wine_bin.display()
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(wine_bin = %wine_bin.display(), stderr = %stderr, "Wine validation failed: --version returned non-zero");
        return Err(WineDownloadError::ValidationFailed(format!(
            "wine --version failed: {stderr}"
        )));
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    tracing::info!(wine_bin = %wine_bin.display(), version = %version_str, "Wine installation validated");
    Ok(version_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_versions_returns_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = WineManager::new(tmp.path().to_path_buf());
        let versions = mgr.available_versions();

        assert!(!versions.is_empty());
        // Should contain known versions
        let version_strs: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        assert!(version_strs.contains(&"10.0"));
        assert!(version_strs.contains(&"9.0"));

        // Versions should have categories
        let stable_versions: Vec<_> = versions.iter().filter(|v| v.category == "stable").collect();
        assert!(!stable_versions.is_empty());
    }

    #[test]
    fn test_available_versions_none_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = WineManager::new(tmp.path().to_path_buf());
        let versions = mgr.available_versions();

        for v in &versions {
            assert!(!v.installed);
        }
    }

    #[test]
    fn test_find_wine_binary_flat_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("wine64"), "fake wine").unwrap();

        let result = find_wine_binary(tmp.path());
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("wine64"));
    }

    #[test]
    fn test_find_wine_binary_nested_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("wine/bin");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("wine"), "fake wine").unwrap();

        let result = find_wine_binary(tmp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_wine_binary_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = find_wine_binary(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_wine_manager_creates_versions_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let _mgr = WineManager::new(tmp.path().to_path_buf());
        assert!(tmp.path().join("wine-versions").exists());
    }

    #[test]
    fn test_installed_versions_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = WineManager::new(tmp.path().to_path_buf());
        let installed = mgr.installed_versions();
        assert!(installed.is_empty());
    }

    #[test]
    fn test_installed_versions_detects_install() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a fake wine install at versions_dir/9.0/bin/wine64
        let ver_dir = tmp.path().join("wine-versions/9.0/bin");
        std::fs::create_dir_all(&ver_dir).unwrap();
        std::fs::write(ver_dir.join("wine64"), "fake").unwrap();

        let mgr = WineManager::new(tmp.path().to_path_buf());
        let installed = mgr.installed_versions();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, "9.0");
        assert!(installed[0].installed);
    }
}