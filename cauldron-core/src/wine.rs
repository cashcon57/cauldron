use crate::bottle::Bottle;
use crate::graphics::{build_env_vars, GraphicsConfig};
use crate::log_capture::LogCapture;
use crate::performance::PerfMonitor;
use crate::rosettax87;
use crate::shader_cache::ShaderCacheManager;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum WineError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Wine binary not found at: {0}")]
    BinaryNotFound(PathBuf),
    #[error("Process launch failed: {0}")]
    LaunchFailed(String),
    #[error("Process kill failed: {0}")]
    KillFailed(String),
}

/// A running Wine process.
pub struct WineProcess {
    pub child: Option<tokio::process::Child>,
    pub bottle_id: String,
    pub bottle_path: PathBuf,
    pub exe_path: PathBuf,
}

/// Launches and manages Wine processes.
pub struct WineRunner {
    pub wine_bin: PathBuf,
    /// Optional base directory for shader caches. When set, shader cache
    /// environment variables are automatically injected during launch.
    pub cache_dir: Option<PathBuf>,
    /// Optional performance monitor. When set, performance environment
    /// variables (Metal HUD, DXVK HUD, etc.) are injected during launch.
    pub perf_monitor: Option<PerfMonitor>,
    /// Optional log capture. When set, Wine/DXVK log redirection
    /// environment variables are injected during launch.
    pub log_capture: Option<LogCapture>,
    /// When true, inject ROSETTA_X87_PATH for faster x87 FP operations
    /// via the RosettaX87 patched Rosetta runtime.
    pub rosettax87_enabled: bool,
}

impl WineRunner {
    /// Create a new WineRunner pointing at the Wine binary.
    pub fn new(wine_bin: PathBuf) -> Self {
        Self {
            wine_bin,
            cache_dir: None,
            perf_monitor: None,
            log_capture: None,
            rosettax87_enabled: false,
        }
    }

    /// Create a new WineRunner with shader cache support.
    pub fn with_cache_dir(wine_bin: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            wine_bin,
            cache_dir: Some(cache_dir),
            perf_monitor: None,
            log_capture: None,
            rosettax87_enabled: false,
        }
    }

    /// Launch an executable inside the given bottle.
    pub async fn launch(
        &self,
        bottle: &Bottle,
        exe_path: &Path,
        args: &[&str],
    ) -> Result<WineProcess, WineError> {
        if !self.wine_bin.exists() {
            tracing::warn!(wine_bin = %self.wine_bin.display(), "Wine binary not found");
            return Err(WineError::BinaryNotFound(self.wine_bin.clone()));
        }

        tracing::info!(
            exe = %exe_path.display(),
            bottle_name = %bottle.name,
            bottle_id = %bottle.id,
            wine_bin = %self.wine_bin.display(),
            "Launching executable in bottle"
        );

        // Build environment variables
        let mut env_vars = std::collections::HashMap::new();

        // Core Wine prefix
        env_vars.insert(
            "WINEPREFIX".to_string(),
            bottle.path.to_string_lossy().to_string(),
        );
        env_vars.insert("WINEMSYNC".to_string(), "1".to_string());

        // Graphics environment from the bottle's backend config
        let gfx_config = GraphicsConfig {
            backend: bottle.graphics_backend,
            dxvk_async: true,
            metalfx_spatial: false,
            metalfx_upscale_factor: 2.0,
            dlss_metalfx: false,
            metal_hud: false,
            dxr_enabled: false,
            mvk_argument_buffers: true,
        };
        let gfx_env = build_env_vars(&gfx_config);
        env_vars.extend(gfx_env);

        // Shader cache environment variables
        if let Some(ref cache_base) = self.cache_dir {
            let backend_name = format!("{:?}", bottle.graphics_backend).to_lowercase();
            let cache_mgr = ShaderCacheManager::new(cache_base.clone());
            let cache_env = cache_mgr.setup_cache_env(&bottle.id, &backend_name);
            env_vars.extend(cache_env);
        }

        // Performance monitoring environment variables
        if let Some(ref perf) = self.perf_monitor {
            let perf_env = perf.build_perf_env();
            env_vars.extend(perf_env);
        }

        // Log capture environment variables
        if let Some(ref log_cap) = self.log_capture {
            let log_env = log_cap.setup_log_env();
            env_vars.extend(log_env);
        }

        // RosettaX87 — faster x87 FP via patched Rosetta
        if self.rosettax87_enabled {
            let rx87_env = rosettax87::build_rosettax87_env(true);
            env_vars.extend(rx87_env);
        }

        // User-specified overrides from the bottle
        env_vars.extend(bottle.env_overrides.clone());

        tracing::debug!(
            env_count = env_vars.len(),
            backend = ?bottle.graphics_backend,
            "Environment assembled for Wine process"
        );

        // Capture Wine stderr for debugging (never suppress errors)
        let child = Command::new(&self.wine_bin)
            .arg(exe_path)
            .args(args)
            .envs(&env_vars)
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                tracing::error!(exe = %exe_path.display(), error = %e, "Failed to spawn Wine process");
                WineError::LaunchFailed(e.to_string())
            })?;

        tracing::info!(exe = %exe_path.display(), bottle_id = %bottle.id, "Wine process spawned successfully");

        Ok(WineProcess {
            child: Some(child),
            bottle_id: bottle.id.clone(),
            bottle_path: bottle.path.clone(),
            exe_path: exe_path.to_path_buf(),
        })
    }

    /// Kill a running Wine process.
    pub async fn kill(&self, process: &mut WineProcess) -> Result<(), WineError> {
        if let Some(ref mut child) = process.child {
            tracing::info!(exe = %process.exe_path.display(), bottle_id = %process.bottle_id, "Killing Wine process");
            child
                .kill()
                .await
                .map_err(|e| {
                    tracing::error!(exe = %process.exe_path.display(), error = %e, "Failed to kill Wine process");
                    WineError::KillFailed(e.to_string())
                })?;
            process.child = None;
            tracing::info!(exe = %process.exe_path.display(), "Wine process killed");
        } else {
            tracing::debug!(exe = %process.exe_path.display(), "No child process to kill");
        }
        Ok(())
    }
}
