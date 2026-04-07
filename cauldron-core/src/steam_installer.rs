use crate::bottle::{BottleManager, BottleError};
use crate::registry;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

const STEAM_INSTALLER_URL: &str =
    "https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe";

/// Minimum disk space required for Steam installation, in bytes (2 GB).
#[allow(dead_code)]
const MIN_DISK_SPACE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Timeout for the Steam silent installer, in seconds.
const SETUP_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Error)]
pub enum SteamInstallError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bottle error: {0}")]
    Bottle(#[from] BottleError),
    #[error("Registry error: {0}")]
    Registry(#[from] registry::RegistryError),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("Setup failed: {0}")]
    SetupFailed(String),
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
    #[error("Prerequisites not met: {0}")]
    PrerequisitesNotMet(String),
    #[error("Progress channel closed")]
    ChannelClosed,
}

type Result<T> = std::result::Result<T, SteamInstallError>;

/// Represents each step of the Steam installation process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstallStep {
    CheckingPrerequisites,
    CreatingBottle,
    InitializingWinePrefix,
    DownloadingSteamInstaller,
    RunningSteamSetup,
    ConfiguringDllOverrides,
    InstallingRuntimes,
    VerifyingInstallation,
    Complete,
    Failed(String),
}

/// Progress information sent to the UI during installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    pub current_step: InstallStep,
    pub step_number: usize,
    pub total_steps: usize,
    pub detail: String,
    pub percentage: f32,
}

/// Status of prerequisite checks before installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrereqStatus {
    pub wine_available: bool,
    pub wine_version: String,
    pub disk_space_gb: f64,
    pub internet_available: bool,
    pub all_ok: bool,
}

/// Orchestrates downloading and installing Steam into a Wine bottle.
pub struct SteamInstaller {
    wine_bin: PathBuf,
    bottles_dir: PathBuf,
}

impl SteamInstaller {
    /// Create a new `SteamInstaller`.
    ///
    /// - `wine_bin`: path to the `wine` binary.
    /// - `bottles_dir`: base directory that contains (or will contain) bottles.
    pub fn new(wine_bin: PathBuf, bottles_dir: PathBuf) -> Self {
        Self {
            wine_bin,
            bottles_dir,
        }
    }

    /// Return the URL used to download the Steam installer.
    pub fn steam_installer_url() -> &'static str {
        STEAM_INSTALLER_URL
    }

    // ------------------------------------------------------------------
    // Prerequisite checks
    // ------------------------------------------------------------------

    /// Check whether the system is ready for a Steam install.
    pub fn check_prerequisites(&self) -> Result<PrereqStatus> {
        tracing::info!("Checking Steam installation prerequisites");

        let (wine_available, wine_version) = self.check_wine();
        let disk_space_gb = self.check_disk_space();
        let internet_available = Self::check_internet();

        let all_ok =
            wine_available && disk_space_gb >= 2.0 && internet_available;

        let status = PrereqStatus {
            wine_available,
            wine_version,
            disk_space_gb,
            internet_available,
            all_ok,
        };

        tracing::info!(
            wine = status.wine_available,
            disk_gb = status.disk_space_gb,
            internet = status.internet_available,
            ok = status.all_ok,
            "Prerequisite check complete"
        );

        Ok(status)
    }

    /// Probe for a working Wine binary and return `(available, version_string)`.
    fn check_wine(&self) -> (bool, String) {
        match std::process::Command::new(&self.wine_bin)
            .arg("--version")
            .output()
        {
            Ok(output) if output.status.success() => {
                let version =
                    String::from_utf8_lossy(&output.stdout).trim().to_string();
                tracing::debug!(version = %version, "Wine found");
                (true, version)
            }
            Ok(output) => {
                tracing::warn!(
                    status = %output.status,
                    "Wine binary returned non-zero"
                );
                (false, String::new())
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %self.wine_bin.display(), "Wine binary not found");
                (false, String::new())
            }
        }
    }

    /// Return available disk space on the bottles volume in GB.
    fn check_disk_space(&self) -> f64 {
        // Use `df` to query the bottles directory (or its closest ancestor).
        let target = if self.bottles_dir.exists() {
            self.bottles_dir.clone()
        } else {
            // Fall back to home directory if bottles dir doesn't exist yet.
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        };

        match std::process::Command::new("df")
            .arg("-k")
            .arg(&target)
            .output()
        {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                // Second line, fourth column (Available) in 1K-blocks on macOS.
                if let Some(line) = text.lines().nth(1) {
                    let cols: Vec<&str> = line.split_whitespace().collect();
                    if cols.len() >= 4 {
                        if let Ok(kb) = cols[3].parse::<u64>() {
                            let gb = kb as f64 / (1024.0 * 1024.0);
                            return gb;
                        }
                    }
                }
                0.0
            }
            Err(_) => 0.0,
        }
    }

    /// Check internet connectivity by attempting to resolve the Steam CDN host.
    fn check_internet() -> bool {
        // A lightweight check: use `curl --head` with a short timeout.
        match std::process::Command::new("curl")
            .args(["--head", "--silent", "--max-time", "5", STEAM_INSTALLER_URL])
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    // ------------------------------------------------------------------
    // Main installation flow
    // ------------------------------------------------------------------

    /// Run the full Steam installation flow, sending progress updates via `progress_tx`.
    ///
    /// Returns the path to the newly-created bottle on success.
    pub async fn install(
        &self,
        bottle_name: &str,
        progress_tx: tokio::sync::mpsc::Sender<InstallProgress>,
    ) -> Result<PathBuf> {
        const TOTAL: usize = 7;

        // Helper to send progress without failing the whole install if the
        // receiver has been dropped.
        let send = |step: InstallStep, n: usize, detail: String| {
            let tx = progress_tx.clone();
            async move {
                let pct = (n as f32 / TOTAL as f32) * 100.0;
                let _ = tx
                    .send(InstallProgress {
                        current_step: step,
                        step_number: n,
                        total_steps: TOTAL,
                        detail,
                        percentage: pct,
                    })
                    .await;
            }
        };

        // ---- Step 1: Create bottle ----
        send(
            InstallStep::CreatingBottle,
            1,
            format!("Creating bottle \"{bottle_name}\"..."),
        )
        .await;

        let bottle_mgr = BottleManager::new(self.bottles_dir.clone());
        let wine_version_str = self.detect_wine_version();
        let bottle = bottle_mgr.create(bottle_name, &wine_version_str)?;
        let bottle_path = bottle.path.clone();

        tracing::info!(
            bottle_id = %bottle.id,
            path = %bottle_path.display(),
            "Bottle created for Steam"
        );

        // ---- Step 2: Initialize Wine prefix ----
        send(
            InstallStep::InitializingWinePrefix,
            2,
            "Initializing Wine prefix (wineboot --init)...".into(),
        )
        .await;

        bottle_mgr.wine_prefix_init(&self.wine_bin, &bottle_path)?;

        // ---- Step 3: Download SteamSetup.exe ----
        send(
            InstallStep::DownloadingSteamInstaller,
            3,
            "Downloading SteamSetup.exe...".into(),
        )
        .await;

        let setup_path = self.download_steam_installer().await?;

        // ---- Step 4: Run SteamSetup.exe /S ----
        send(
            InstallStep::RunningSteamSetup,
            4,
            "Running Steam silent installer...".into(),
        )
        .await;

        self.run_steam_setup(&bottle_path, &setup_path).await?;

        // Clean up the downloaded installer.
        let _ = std::fs::remove_file(&setup_path);

        // ---- Step 5: Configure DLL overrides ----
        send(
            InstallStep::ConfiguringDllOverrides,
            5,
            "Setting DLL overrides for optimal gaming...".into(),
        )
        .await;

        self.configure_dll_overrides(&bottle_path)?;

        // ---- Step 6: Install runtimes / env config ----
        send(
            InstallStep::InstallingRuntimes,
            6,
            "Configuring runtimes and graphics backend...".into(),
        )
        .await;

        self.configure_runtimes(&bottle_path)?;

        // ---- Step 7: Verify ----
        send(
            InstallStep::VerifyingInstallation,
            7,
            "Verifying Steam installation...".into(),
        )
        .await;

        if !Self::verify_steam_installed(&bottle_path) {
            let msg = "steam.exe not found after installation".to_string();
            send(InstallStep::Failed(msg.clone()), 7, msg.clone()).await;
            return Err(SteamInstallError::VerificationFailed(msg));
        }

        send(
            InstallStep::Complete,
            7,
            "Steam installed successfully!".into(),
        )
        .await;

        tracing::info!(path = %bottle_path.display(), "Steam installation complete");
        Ok(bottle_path)
    }

    // ------------------------------------------------------------------
    // Individual step implementations
    // ------------------------------------------------------------------

    /// Detect the Wine version string from the binary.
    fn detect_wine_version(&self) -> String {
        match std::process::Command::new(&self.wine_bin)
            .arg("--version")
            .output()
        {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => "unknown".to_string(),
        }
    }

    /// Download `SteamSetup.exe` to a temporary file and return its path.
    async fn download_steam_installer(&self) -> Result<PathBuf> {
        let tmp_dir = std::env::temp_dir().join("cauldron");
        std::fs::create_dir_all(&tmp_dir)?;
        let dest = tmp_dir.join("SteamSetup.exe");

        tracing::info!(url = STEAM_INSTALLER_URL, dest = %dest.display(), "Downloading Steam installer");

        // Use curl for a straightforward large-file download.
        let status = tokio::process::Command::new("curl")
            .args([
                "-L",
                "--silent",
                "--show-error",
                "--fail",
                "-o",
                dest.to_str().unwrap_or("SteamSetup.exe"),
                STEAM_INSTALLER_URL,
            ])
            .status()
            .await
            .map_err(|e| SteamInstallError::DownloadFailed(e.to_string()))?;

        if !status.success() {
            return Err(SteamInstallError::DownloadFailed(format!(
                "curl exited with status {status}"
            )));
        }

        // Sanity-check: file should exist and be reasonably large (> 1 MB).
        let meta = std::fs::metadata(&dest)?;
        if meta.len() < 1_000_000 {
            return Err(SteamInstallError::DownloadFailed(
                "Downloaded file is suspiciously small".into(),
            ));
        }

        tracing::info!(size_bytes = meta.len(), "Steam installer downloaded");
        Ok(dest)
    }

    /// Run the Steam silent installer inside the bottle.
    async fn run_steam_setup(
        &self,
        bottle_path: &Path,
        setup_path: &Path,
    ) -> Result<()> {
        tracing::info!(
            setup = %setup_path.display(),
            prefix = %bottle_path.display(),
            "Launching SteamSetup.exe /S"
        );

        let child = tokio::process::Command::new(&self.wine_bin)
            .arg(setup_path)
            .arg("/S")
            .env("WINEPREFIX", bottle_path)
            .env("WINEDEBUG", "-all")
            .spawn()
            .map_err(|e| SteamInstallError::SetupFailed(e.to_string()))?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(SETUP_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| {
            SteamInstallError::SetupFailed(format!(
                "Steam installer timed out after {SETUP_TIMEOUT_SECS}s"
            ))
        })?
        .map_err(|e| SteamInstallError::SetupFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(status = %output.status, stderr = %stderr, "SteamSetup.exe failed");
            return Err(SteamInstallError::SetupFailed(format!(
                "SteamSetup.exe exited with status {}",
                output.status
            )));
        }

        tracing::info!("SteamSetup.exe completed successfully");
        Ok(())
    }

    /// Set DLL overrides for optimal gaming performance (DXVK / DXMT).
    fn configure_dll_overrides(&self, bottle_path: &Path) -> Result<()> {
        let overrides = [
            ("dxgi", "native"),
            ("d3d11", "native"),
            ("d3d10core", "native"),
            ("d3d9", "native"),
        ];

        for (dll, mode) in &overrides {
            tracing::debug!(dll = %dll, mode = %mode, "Setting DLL override");
            registry::set_dll_override(bottle_path, dll, mode)?;
        }

        Ok(())
    }

    /// Configure runtime environment in the bottle config for gaming.
    fn configure_runtimes(&self, bottle_path: &Path) -> Result<()> {
        // Read the existing bottle config so we can update env_overrides.
        let config_path = bottle_path.join("bottle.toml");
        let contents = std::fs::read_to_string(&config_path)?;
        let mut bottle: crate::bottle::Bottle = toml::from_str(&contents)
            .map_err(|e| SteamInstallError::SetupFailed(e.to_string()))?;

        // Enable msync for better multi-threaded performance.
        bottle
            .env_overrides
            .insert("WINEMSYNC".to_string(), "1".to_string());

        // Set optimal graphics backend hints.
        // DXMT for DX11 translation, D3DMetal for DX12.
        bottle
            .env_overrides
            .insert("DXMT_ENABLED".to_string(), "1".to_string());
        bottle
            .env_overrides
            .insert("D3DM_ENABLED".to_string(), "1".to_string());

        let updated_toml = toml::to_string_pretty(&bottle)
            .map_err(|e| SteamInstallError::SetupFailed(e.to_string()))?;
        std::fs::write(&config_path, updated_toml)?;

        tracing::info!("Runtime configuration written to bottle.toml");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Post-install helpers
    // ------------------------------------------------------------------

    /// Check whether `steam.exe` exists in the expected location inside a bottle.
    pub fn verify_steam_installed(bottle_path: &Path) -> bool {
        let steam_exe = bottle_path
            .join("drive_c/Program Files (x86)/Steam/steam.exe");
        let exists = steam_exe.exists();
        tracing::debug!(
            path = %steam_exe.display(),
            exists = exists,
            "Verifying Steam installation"
        );
        exists
    }

    /// Launch Steam inside the given bottle, returning the child process handle.
    pub fn launch_steam(
        wine_bin: &Path,
        bottle_path: &Path,
    ) -> Result<std::process::Child> {
        let steam_exe = bottle_path
            .join("drive_c/Program Files (x86)/Steam/steam.exe");

        if !steam_exe.exists() {
            return Err(SteamInstallError::VerificationFailed(
                "steam.exe not found".into(),
            ));
        }

        tracing::info!(
            wine = %wine_bin.display(),
            steam = %steam_exe.display(),
            "Launching Steam"
        );

        let child = std::process::Command::new(wine_bin)
            .arg(&steam_exe)
            .env("WINEPREFIX", bottle_path)
            .env("WINEMSYNC", "1")
            .env("WINEDEBUG", "-all")
            .spawn()?;

        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steam_installer_url() {
        let url = SteamInstaller::steam_installer_url();
        assert!(url.starts_with("https://"));
        assert!(url.contains("SteamSetup.exe"));
    }

    #[test]
    fn test_verify_steam_not_installed() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!SteamInstaller::verify_steam_installed(tmp.path()));
    }

    #[test]
    fn test_verify_steam_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_dir =
            tmp.path().join("drive_c/Program Files (x86)/Steam");
        std::fs::create_dir_all(&steam_dir).unwrap();
        std::fs::write(steam_dir.join("steam.exe"), b"fake").unwrap();
        assert!(SteamInstaller::verify_steam_installed(tmp.path()));
    }

    #[test]
    fn test_new_installer() {
        let installer = SteamInstaller::new(
            PathBuf::from("/usr/local/bin/wine"),
            PathBuf::from("/tmp/bottles"),
        );
        assert_eq!(installer.wine_bin, PathBuf::from("/usr/local/bin/wine"));
        assert_eq!(installer.bottles_dir, PathBuf::from("/tmp/bottles"));
    }
}
