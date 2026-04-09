use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeDownloadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("Version not found: {0}")]
    VersionNotFound(String),
    #[error("Checksum mismatch for {file}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },
}

/// A downloadable runtime component.
#[derive(Debug, Clone)]
pub struct RuntimeRelease {
    /// Display name (e.g., "DXVK 1.10.3").
    pub name: String,
    /// Component type.
    pub component: RuntimeComponent,
    /// Version string.
    pub version: String,
    /// Download URL for the release tarball/zip.
    pub url: String,
    /// Optional SHA-256 checksum.
    pub sha256: Option<String>,
    /// Whether this version is installed locally.
    pub installed: bool,
    /// Local installation path.
    pub path: PathBuf,
}

/// The type of runtime component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeComponent {
    /// DXVK — DirectX 9/10/11 to Vulkan.
    Dxvk,
    /// DXMT — DirectX 10/11 to Metal.
    Dxmt,
    /// MoltenVK — Vulkan ICD on Metal.
    MoltenVK,
    /// D3DMetal — Apple Game Porting Toolkit.
    D3DMetal,
    /// vkd3d-proton — DirectX 12 to Vulkan.
    Vkd3dProton,
}

impl std::fmt::Display for RuntimeComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dxvk => write!(f, "DXVK"),
            Self::Dxmt => write!(f, "DXMT"),
            Self::MoltenVK => write!(f, "MoltenVK"),
            Self::D3DMetal => write!(f, "D3DMetal"),
            Self::Vkd3dProton => write!(f, "vkd3d-proton"),
        }
    }
}

/// Manages downloading and extracting graphics runtime components.
///
/// Each component lives in its own versioned subdirectory under `runtimes/`:
/// ```text
/// runtimes/
///   dxvk/
///     1.10.3/
///       x64/d3d9.dll, d3d10core.dll, d3d11.dll, dxgi.dll
///       x32/...
///   dxmt/
///     0.72/
///       x64/d3d10core.dll, d3d11.dll, dxgi.dll
///   moltenvk/
///     1.2.11/
///       libMoltenVK.dylib
///   vkd3d-proton/
///     2.13/
///       x64/d3d12.dll
/// ```
pub struct RuntimeDownloader {
    /// Base directory for runtime storage.
    pub runtimes_dir: PathBuf,
}

impl RuntimeDownloader {
    pub fn new(base_dir: PathBuf) -> Self {
        let runtimes_dir = base_dir.join("runtimes");
        Self { runtimes_dir }
    }

    /// Return the list of known downloadable runtime releases.
    ///
    /// These are curated releases known to work on macOS with Wine.
    pub fn available_releases(&self) -> Vec<RuntimeRelease> {
        let mut known: Vec<(RuntimeComponent, &str, &str)> = vec![
            // DXVK — Gcenx's macOS fork (stuck at 1.10.3 due to MoltenVK extension gap)
            (RuntimeComponent::Dxvk, "1.10.3",
             "https://github.com/Gcenx/DXVK-macOS/releases/download/v1.10.3/dxvk-macOS-async-v1.10.3.tar.gz"),
            // DXMT — 3Shain's Metal-native DX11 (latest stable)
            (RuntimeComponent::Dxmt, "0.72",
             "https://github.com/3Shain/dxmt/releases/download/v0.72/dxmt-0.72-macos.tar.gz"),
            // MoltenVK — latest release
            (RuntimeComponent::MoltenVK, "1.2.11",
             "https://github.com/KhronosGroup/MoltenVK/releases/download/v1.2.11/MoltenVK-macos.tar"),
            // vkd3d-proton — Gcenx's macOS fork
            (RuntimeComponent::Vkd3dProton, "2.13",
             "https://github.com/Gcenx/vkd3d-proton-macOS/releases/download/v2.13/vkd3d-proton-macOS-v2.13.tar.gz"),
        ];

        // D3DMetal — auto-detect from CrossOver or GPTK install
        if Self::detect_d3dmetal_source().is_some() {
            known.push((RuntimeComponent::D3DMetal, "crossover", "local://crossover"));
        }

        known
            .into_iter()
            .map(|(component, version, url)| {
                let component_dir = component_dir_name(component);
                let install_path = self.runtimes_dir.join(component_dir).join(version);
                let installed = install_path.exists();
                RuntimeRelease {
                    name: format!("{} {}", component, version),
                    component,
                    version: version.to_string(),
                    url: url.to_string(),
                    sha256: None,
                    installed,
                    path: install_path,
                }
            })
            .collect()
    }

    /// List locally installed runtime versions.
    pub fn installed_versions(&self) -> Vec<RuntimeRelease> {
        self.available_releases()
            .into_iter()
            .filter(|r| r.installed)
            .collect()
    }

    /// Download and extract a specific runtime release.
    ///
    /// Uses a temp directory for the download, only moving files to the final
    /// location on success. This prevents orphaned partial downloads.
    pub fn download(&self, component: RuntimeComponent, version: &str) -> Result<PathBuf, RuntimeDownloadError> {
        let releases = self.available_releases();
        let release = releases
            .iter()
            .find(|r| r.component == component && r.version == version)
            .ok_or_else(|| {
                RuntimeDownloadError::VersionNotFound(format!("{} {}", component, version))
            })?;

        if release.installed {
            tracing::info!("{} {} already installed", component, version);
            return Ok(release.path.clone());
        }

        tracing::info!("Downloading {} {} from {}", component, version, release.url);

        let component_dir = self.runtimes_dir.join(component_dir_name(component));
        fs::create_dir_all(&component_dir)?;

        // Download to a temp directory first to avoid orphaned files on failure
        let tmp_dir = tempfile::tempdir_in(&component_dir)?;
        let filename = release.url.rsplit('/').next().unwrap_or("download.tar.gz");
        let download_path = tmp_dir.path().join(filename);

        // Download using curl
        let curl_status = std::process::Command::new("curl")
            .args([
                "-L",
                "--fail",
                "--progress-bar",
                "-o",
                &download_path.to_string_lossy(),
                &release.url,
            ])
            .status()
            .map_err(|e| {
                RuntimeDownloadError::DownloadFailed(format!("curl failed: {e}"))
            })?;

        if !curl_status.success() {
            // tmp_dir drops automatically, cleaning up partial download
            return Err(RuntimeDownloadError::DownloadFailed(format!(
                "curl failed for {}",
                release.url
            )));
        }

        // Verify checksum if available
        if let Some(ref expected_sha) = release.sha256 {
            let actual_sha = compute_sha256(&download_path)?;
            if actual_sha != *expected_sha {
                // tmp_dir drops automatically, cleaning up
                return Err(RuntimeDownloadError::ChecksumMismatch {
                    file: filename.to_string(),
                    expected: expected_sha.clone(),
                    actual: actual_sha,
                });
            }
        }

        // Extract to a staging directory within tmp
        let staging = tmp_dir.path().join("extracted");
        fs::create_dir_all(&staging)?;
        extract_runtime_archive(&download_path, &staging)?;

        // Flatten if single subdirectory
        flatten_if_single_subdir(&staging)?;

        // Atomically move staging to final location
        let final_path = component_dir.join(version);
        if final_path.exists() {
            fs::remove_dir_all(&final_path)?;
        }
        fs::rename(&staging, &final_path)?;

        // tmp_dir cleanup happens automatically on drop, removing the
        // downloaded tarball and any other temp files
        tracing::info!(
            "{} {} installed to {}",
            component,
            version,
            final_path.display()
        );

        Ok(final_path)
    }

    /// Download all available runtimes that aren't already installed.
    pub fn download_all_missing(&self) -> Vec<Result<PathBuf, RuntimeDownloadError>> {
        let releases = self.available_releases();
        let missing: Vec<_> = releases.iter().filter(|r| !r.installed).collect();

        tracing::info!(
            "Downloading {} missing runtime components",
            missing.len()
        );

        missing
            .iter()
            .map(|r| self.download(r.component, &r.version))
            .collect()
    }

    /// Remove a specific installed runtime version.
    pub fn remove(&self, component: RuntimeComponent, version: &str) -> Result<(), RuntimeDownloadError> {
        let component_dir = self.runtimes_dir.join(component_dir_name(component));
        let version_dir = component_dir.join(version);

        if version_dir.exists() {
            tracing::info!("Removing {} {}", component, version);
            fs::remove_dir_all(&version_dir)?;
        }

        Ok(())
    }

    /// Clean up any orphaned temp files in the runtimes directory.
    ///
    /// Looks for directories matching temp patterns (`.tmp*`, `.cauldron-*`)
    /// and removes them. These can be left behind if a previous download
    /// was interrupted.
    pub fn cleanup_orphans(&self) -> Result<usize, RuntimeDownloadError> {
        let mut cleaned = 0;

        if !self.runtimes_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&self.runtimes_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".tmp") || name.starts_with(".cauldron-") {
                tracing::info!("Cleaning orphaned temp dir: {}", entry.path().display());
                fs::remove_dir_all(entry.path())?;
                cleaned += 1;
            }

            // Also check inside component directories
            if entry.path().is_dir() {
                if let Ok(children) = fs::read_dir(entry.path()) {
                    for child in children.flatten() {
                        let child_name = child.file_name().to_string_lossy().to_string();
                        if child_name.starts_with(".tmp") || child_name.starts_with(".cauldron-") {
                            tracing::info!("Cleaning orphaned temp dir: {}", child.path().display());
                            fs::remove_dir_all(child.path())?;
                            cleaned += 1;
                        }
                    }
                }
            }
        }

        if cleaned > 0 {
            tracing::info!("Cleaned {} orphaned temp directories", cleaned);
        }

        Ok(cleaned)
    }
}

/// Info about a detected D3DMetal source.
#[derive(Debug, Clone, serde::Serialize)]
pub struct D3DMetalSource {
    /// Where it was found: "crossover", "gptk", "custom", or "none"
    pub source: String,
    /// Human-readable label
    pub label: String,
    /// Path to D3DMetal.framework
    pub path: String,
    /// Whether it's been imported into Cauldron's runtime directory
    pub imported: bool,
    /// Version string from Info.plist (e.g. "3.0.1"), if detected
    pub version: Option<String>,
}

impl RuntimeDownloader {
    /// Detect D3DMetal.framework from all known sources.
    /// Priority: CrossOver > GPTK > Cauldron deps > None
    pub fn detect_d3dmetal_source() -> Option<std::path::PathBuf> {
        Self::detect_d3dmetal_detailed().filter(|d| d.source != "none").map(|d| std::path::PathBuf::from(&d.path))
    }

    /// Detailed D3DMetal detection with source attribution.
    pub fn detect_d3dmetal_detailed() -> Option<D3DMetalSource> {
        let home = dirs::home_dir().unwrap_or_default();

        // 1. CrossOver — prioritized. We complement CrossOver, not compete.
        let crossover_paths = [
            "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external/D3DMetal.framework".to_string(),
            format!("{}/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external/D3DMetal.framework", home.display()),
        ];
        for path in &crossover_paths {
            if std::path::Path::new(path).exists() {
                return Some(D3DMetalSource {
                    source: "crossover".into(),
                    label: "CrossOver (detected)".into(),
                    path: path.clone(),
                    imported: false,
                    version: extract_d3dmetal_version(std::path::Path::new(path)),
                });
            }
        }

        // 2. Game Porting Toolkit — developer install
        let gptk_paths = [
            "/opt/homebrew/opt/game-porting-toolkit/lib/D3DMetal.framework",
            "/usr/local/opt/game-porting-toolkit/lib/D3DMetal.framework",
            "/Library/Frameworks/D3DMetal.framework",
        ];
        for path in &gptk_paths {
            if std::path::Path::new(path).exists() {
                return Some(D3DMetalSource {
                    source: "gptk".into(),
                    label: "Game Porting Toolkit (detected)".into(),
                    path: path.to_string(),
                    imported: false,
                    version: extract_d3dmetal_version(std::path::Path::new(path)),
                });
            }
        }

        // 3. Cauldron's own deps/runtime directory (previously imported)
        let cauldron_paths = [
            "deps/cxpatcher/lib/CrossOver/lib64/apple_gptk/external/D3DMetal.framework",
        ];
        for path in &cauldron_paths {
            if std::path::Path::new(path).exists() {
                return Some(D3DMetalSource {
                    source: "imported".into(),
                    label: "Previously imported".into(),
                    path: path.to_string(),
                    imported: true,
                    version: extract_d3dmetal_version(std::path::Path::new(path)),
                });
            }
        }

        // Not found
        Some(D3DMetalSource {
            source: "none".into(),
            label: "Not found".into(),
            path: String::new(),
            imported: false,
            version: None,
        })
    }

    /// Import D3DMetal from a specific path into Cauldron's runtime directory.
    pub fn import_d3dmetal(source_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<std::path::PathBuf, RuntimeDownloadError> {
        let dest = dest_dir.join("d3dmetal").join("D3DMetal.framework");
        if dest.exists() {
            // Already imported — check if source is newer
            tracing::info!("D3DMetal already imported at {}", dest.display());
            return Ok(dest);
        }

        std::fs::create_dir_all(dest.parent().unwrap())?;

        // Use ditto for framework copy (preserves symlinks, code signatures)
        let output = std::process::Command::new("ditto")
            .args([source_path.to_str().unwrap_or(""), dest.to_str().unwrap_or("")])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RuntimeDownloadError::ExtractionFailed(
                format!("ditto failed: {}", stderr.chars().take(200).collect::<String>())
            ));
        }

        tracing::info!("D3DMetal imported from {} to {}", source_path.display(), dest.display());
        Ok(dest)
    }
}

/// Map a RuntimeComponent to its directory name.
fn component_dir_name(component: RuntimeComponent) -> &'static str {
    match component {
        RuntimeComponent::Dxvk => "dxvk",
        RuntimeComponent::Dxmt => "dxmt",
        RuntimeComponent::MoltenVK => "moltenvk",
        RuntimeComponent::D3DMetal => "d3dmetal",
        RuntimeComponent::Vkd3dProton => "vkd3d-proton",
    }
}

/// Extract a runtime archive (tar.gz, tar.xz, tar, zip).
fn extract_runtime_archive(archive: &Path, dest: &Path) -> Result<(), RuntimeDownloadError> {
    let filename = archive
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let tar_flags = if filename.ends_with(".tar.xz") || filename.ends_with(".txz") {
        "xJf"
    } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        "xzf"
    } else if filename.ends_with(".tar.bz2") {
        "xjf"
    } else if filename.ends_with(".tar") {
        "xf"
    } else if filename.ends_with(".zip") {
        // Use unzip instead of tar
        let status = std::process::Command::new("unzip")
            .args(["-q", "-o", &archive.to_string_lossy(), "-d", &dest.to_string_lossy()])
            .status()
            .map_err(|e| RuntimeDownloadError::ExtractionFailed(format!("unzip failed: {e}")))?;

        if !status.success() {
            return Err(RuntimeDownloadError::ExtractionFailed(
                "unzip failed".to_string(),
            ));
        }
        return Ok(());
    } else {
        "xf" // Let tar auto-detect
    };

    let status = std::process::Command::new("tar")
        .args([tar_flags, &archive.to_string_lossy(), "-C", &dest.to_string_lossy()])
        .status()
        .map_err(|e| RuntimeDownloadError::ExtractionFailed(format!("tar failed: {e}")))?;

    if !status.success() {
        return Err(RuntimeDownloadError::ExtractionFailed(
            "tar extraction failed".to_string(),
        ));
    }

    Ok(())
}

/// If the extracted directory contains exactly one subdirectory and nothing else,
/// move its contents up one level to flatten the structure.
fn flatten_if_single_subdir(dir: &Path) -> Result<(), RuntimeDownloadError> {
    let entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();

    if entries.len() == 1 && entries[0].path().is_dir() {
        let subdir = entries[0].path();
        for child in fs::read_dir(&subdir)?.filter_map(|e| e.ok()) {
            let dest = dir.join(child.file_name());
            fs::rename(child.path(), &dest)?;
        }
        fs::remove_dir(&subdir)?;
    }

    Ok(())
}

/// Compute SHA-256 hash of a file.
fn compute_sha256(path: &Path) -> Result<String, RuntimeDownloadError> {
    use sha2::{Digest, Sha256};

    let data = fs::read(path)?;
    let hash = Sha256::digest(&data);
    Ok(format!("{:x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_downloader_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        assert_eq!(dl.runtimes_dir, tmp.path().join("runtimes"));
    }

    #[test]
    fn test_available_releases() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        let releases = dl.available_releases();

        assert!(!releases.is_empty());

        let dxvk: Vec<_> = releases.iter().filter(|r| r.component == RuntimeComponent::Dxvk).collect();
        assert!(!dxvk.is_empty());
        assert_eq!(dxvk[0].version, "1.10.3");

        let dxmt: Vec<_> = releases.iter().filter(|r| r.component == RuntimeComponent::Dxmt).collect();
        assert!(!dxmt.is_empty());
    }

    #[test]
    fn test_none_installed_initially() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        let installed = dl.installed_versions();
        assert!(installed.is_empty());
    }

    #[test]
    fn test_component_dir_names() {
        assert_eq!(component_dir_name(RuntimeComponent::Dxvk), "dxvk");
        assert_eq!(component_dir_name(RuntimeComponent::Dxmt), "dxmt");
        assert_eq!(component_dir_name(RuntimeComponent::MoltenVK), "moltenvk");
        assert_eq!(component_dir_name(RuntimeComponent::D3DMetal), "d3dmetal");
        assert_eq!(component_dir_name(RuntimeComponent::Vkd3dProton), "vkd3d-proton");
    }

    #[test]
    fn test_component_display() {
        assert_eq!(format!("{}", RuntimeComponent::Dxvk), "DXVK");
        assert_eq!(format!("{}", RuntimeComponent::Dxmt), "DXMT");
        assert_eq!(format!("{}", RuntimeComponent::MoltenVK), "MoltenVK");
    }

    #[test]
    fn test_version_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        let result = dl.download(RuntimeComponent::Dxvk, "99.99.99");
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_orphans_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        let cleaned = dl.cleanup_orphans().unwrap();
        assert_eq!(cleaned, 0);
    }

    #[test]
    fn test_cleanup_orphans_removes_temps() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        fs::create_dir_all(dl.runtimes_dir.join(".tmp12345")).unwrap();
        fs::create_dir_all(dl.runtimes_dir.join(".cauldron-staging")).unwrap();
        fs::create_dir_all(dl.runtimes_dir.join("dxvk")).unwrap(); // legit dir

        let cleaned = dl.cleanup_orphans().unwrap();
        assert_eq!(cleaned, 2);
        assert!(!dl.runtimes_dir.join(".tmp12345").exists());
        assert!(!dl.runtimes_dir.join(".cauldron-staging").exists());
        assert!(dl.runtimes_dir.join("dxvk").exists()); // still there
    }

    #[test]
    fn test_remove_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RuntimeDownloader::new(tmp.path().to_path_buf());
        // Should not error when removing something that doesn't exist
        dl.remove(RuntimeComponent::Dxvk, "1.0.0").unwrap();
    }

    #[test]
    fn test_flatten_if_single_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path().join("parent");
        let child = parent.join("only-child");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join("file.txt"), "content").unwrap();

        flatten_if_single_subdir(&parent).unwrap();

        // File should now be directly under parent
        assert!(parent.join("file.txt").exists());
        assert!(!child.exists());
    }
}

/// Extract CFBundleShortVersionString from a D3DMetal.framework's Info.plist.
fn extract_d3dmetal_version(framework_path: &std::path::Path) -> Option<String> {
    let candidates = [
        framework_path.join("Resources/Info.plist"),
        framework_path.join("Info.plist"),
    ];
    for plist_path in &candidates {
        if let Ok(content) = fs::read_to_string(plist_path) {
            if let Some(key_pos) = content.find("<key>CFBundleShortVersionString</key>") {
                let after_key = &content[key_pos..];
                if let Some(str_start) = after_key.find("<string>") {
                    let value_start = str_start + "<string>".len();
                    if let Some(str_end) = after_key[value_start..].find("</string>") {
                        let version = after_key[value_start..value_start + str_end].trim().to_string();
                        if !version.is_empty() {
                            return Some(version);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Set up DLSS-to-MetalFX translation in a bottle (GPTK 3.0+).
/// Copies nvngx-on-metalfx.dll -> nvngx.dll and nvapi64.dll into system32.
pub fn setup_dlss_metalfx(d3dmetal_path: &std::path::Path, bottle_path: &std::path::Path) -> Result<Vec<std::path::PathBuf>, RuntimeDownloadError> {
    let base = d3dmetal_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(d3dmetal_path);

    let dll_dir = base.join("wine/x86_64-windows");
    let nvngx_src = dll_dir.join("nvngx-on-metalfx.dll");

    if !nvngx_src.exists() {
        return Ok(Vec::new());
    }

    let system32 = bottle_path.join("drive_c/windows/system32");
    if !system32.exists() {
        fs::create_dir_all(&system32)?;
    }

    let mut copied = Vec::new();

    let nvngx_dest = system32.join("nvngx.dll");
    if nvngx_dest.exists() {
        let backup = nvngx_dest.with_extension("dll.cauldron_backup");
        fs::copy(&nvngx_dest, &backup)?;
    }
    fs::copy(&nvngx_src, &nvngx_dest)?;
    copied.push(nvngx_dest);

    let nvapi_src = dll_dir.join("nvapi64.dll");
    if nvapi_src.exists() {
        let nvapi_dest = system32.join("nvapi64.dll");
        if nvapi_dest.exists() {
            let backup = nvapi_dest.with_extension("dll.cauldron_backup");
            fs::copy(&nvapi_dest, &backup)?;
        }
        fs::copy(&nvapi_src, &nvapi_dest)?;
        copied.push(nvapi_dest);
    }

    Ok(copied)
}
