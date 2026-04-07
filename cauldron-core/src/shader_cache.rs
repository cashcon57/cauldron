use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShaderCacheError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Cache not found: {game_id}/{backend}")]
    NotFound { game_id: String, backend: String },
    #[error("Invalid archive: {0}")]
    InvalidArchive(String),
    #[error("tar command failed: {0}")]
    TarFailed(String),
}

type Result<T> = std::result::Result<T, ShaderCacheError>;

/// Metadata about a shader cache for a specific game and backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInfo {
    pub game_id: String,
    pub backend: String,
    pub shader_count: usize,
    pub size_bytes: u64,
    pub last_updated: String,
    pub version: String,
}

/// A handle to a specific game's shader cache directory.
#[derive(Debug, Clone)]
pub struct ShaderCache {
    pub cache_dir: PathBuf,
    pub game_id: String,
}

/// Manages per-game shader cache storage, sharing, and import/export.
///
/// Cache layout:
/// ```text
/// <base_cache_dir>/shader-cache/<game_id>/<backend>/
///     cache_meta.json
///     <shader files...>
/// ```
pub struct ShaderCacheManager {
    pub base_cache_dir: PathBuf,
}

impl ShaderCacheManager {
    /// Create a new ShaderCacheManager. Creates the `shader-cache/` directory
    /// under the given base directory if it does not exist.
    pub fn new(base_dir: PathBuf) -> Self {
        let base_cache_dir = base_dir.join("shader-cache");
        if let Err(e) = std::fs::create_dir_all(&base_cache_dir) {
            tracing::warn!("Failed to create shader cache directory: {e}");
        }
        Self { base_cache_dir }
    }

    /// Returns the path for a specific game and backend's shader cache.
    pub fn get_cache_dir(&self, game_id: &str, backend: &str) -> PathBuf {
        self.base_cache_dir.join(game_id).join(backend)
    }

    /// Reads cache metadata for a specific game and backend.
    /// Returns `Ok(None)` if the cache directory or metadata file does not exist.
    pub fn cache_info(&self, game_id: &str, backend: &str) -> Result<Option<CacheInfo>> {
        let meta_path = self.get_cache_dir(game_id, backend).join("cache_meta.json");
        if !meta_path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&meta_path)?;
        let info: CacheInfo = serde_json::from_str(&contents)?;
        Ok(Some(info))
    }

    /// Scans all cached games and returns their metadata.
    pub fn list_caches(&self) -> Result<Vec<CacheInfo>> {
        let mut caches = Vec::new();

        if !self.base_cache_dir.exists() {
            return Ok(caches);
        }

        let game_dirs = std::fs::read_dir(&self.base_cache_dir)?;
        for game_entry in game_dirs {
            let game_entry = game_entry?;
            if !game_entry.file_type()?.is_dir() {
                continue;
            }
            let backend_dirs = std::fs::read_dir(game_entry.path())?;
            for backend_entry in backend_dirs {
                let backend_entry = backend_entry?;
                if !backend_entry.file_type()?.is_dir() {
                    continue;
                }
                let meta_path = backend_entry.path().join("cache_meta.json");
                if meta_path.exists() {
                    let contents = std::fs::read_to_string(&meta_path)?;
                    match serde_json::from_str::<CacheInfo>(&contents) {
                        Ok(info) => caches.push(info),
                        Err(e) => {
                            tracing::warn!(
                                "Skipping invalid cache metadata at {}: {e}",
                                meta_path.display()
                            );
                        }
                    }
                }
            }
        }

        Ok(caches)
    }

    /// Creates a `.tar.gz` archive of the shader cache for sharing.
    pub fn export_cache(
        &self,
        game_id: &str,
        backend: &str,
        output_path: &Path,
    ) -> Result<()> {
        let cache_dir = self.get_cache_dir(game_id, backend);
        if !cache_dir.exists() {
            return Err(ShaderCacheError::NotFound {
                game_id: game_id.to_string(),
                backend: backend.to_string(),
            });
        }

        // Use tar to create a compressed archive
        let status = std::process::Command::new("tar")
            .arg("czf")
            .arg(output_path)
            .arg("-C")
            .arg(&self.base_cache_dir)
            .arg(format!("{}/{}", game_id, backend))
            .status()?;

        if !status.success() {
            return Err(ShaderCacheError::TarFailed(format!(
                "tar exited with status: {status}"
            )));
        }

        tracing::info!(
            "Exported shader cache {game_id}/{backend} to {}",
            output_path.display()
        );
        Ok(())
    }

    /// Imports a shared `.tar.gz` shader cache archive.
    ///
    /// The archive is expected to contain a `<game_id>/<backend>/` directory
    /// structure with a `cache_meta.json` file inside.
    pub fn import_cache(&self, archive_path: &Path) -> Result<CacheInfo> {
        if !archive_path.exists() {
            return Err(ShaderCacheError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Archive not found: {}", archive_path.display()),
            )));
        }

        // Extract into the base cache directory
        let status = std::process::Command::new("tar")
            .arg("xzf")
            .arg(archive_path)
            .arg("-C")
            .arg(&self.base_cache_dir)
            .status()?;

        if !status.success() {
            return Err(ShaderCacheError::TarFailed(format!(
                "tar extract exited with status: {status}"
            )));
        }

        // Find the cache_meta.json that was just extracted by scanning for
        // any newly present metadata files. We list archive contents to
        // determine the game_id and backend.
        let output = std::process::Command::new("tar")
            .arg("tzf")
            .arg(archive_path)
            .output()?;

        if !output.status.success() {
            return Err(ShaderCacheError::TarFailed(
                "Failed to list archive contents".to_string(),
            ));
        }

        let listing = String::from_utf8_lossy(&output.stdout);
        let meta_entry = listing
            .lines()
            .find(|line| line.ends_with("cache_meta.json"))
            .ok_or_else(|| {
                ShaderCacheError::InvalidArchive(
                    "No cache_meta.json found in archive".to_string(),
                )
            })?;

        let meta_path = self.base_cache_dir.join(meta_entry);
        let contents = std::fs::read_to_string(&meta_path)?;
        let info: CacheInfo = serde_json::from_str(&contents)?;

        tracing::info!(
            "Imported shader cache for {}/{} ({} shaders)",
            info.game_id,
            info.backend,
            info.shader_count
        );

        Ok(info)
    }

    /// Deletes a game's shader cache for the given backend.
    pub fn clear_cache(&self, game_id: &str, backend: &str) -> Result<()> {
        let cache_dir = self.get_cache_dir(game_id, backend);
        if !cache_dir.exists() {
            return Err(ShaderCacheError::NotFound {
                game_id: game_id.to_string(),
                backend: backend.to_string(),
            });
        }
        std::fs::remove_dir_all(&cache_dir)?;
        tracing::info!("Cleared shader cache for {game_id}/{backend}");

        // Clean up empty game directory if no backends remain
        let game_dir = self.base_cache_dir.join(game_id);
        if game_dir.exists() {
            if let Ok(mut entries) = std::fs::read_dir(&game_dir) {
                if entries.next().is_none() {
                    let _ = std::fs::remove_dir(&game_dir);
                }
            }
        }

        Ok(())
    }

    /// Returns the total size in bytes of all shader caches.
    pub fn total_cache_size(&self) -> Result<u64> {
        if !self.base_cache_dir.exists() {
            return Ok(0);
        }
        Ok(dir_size(&self.base_cache_dir)?)
    }

    /// Returns environment variables that point DXVK, D3DMetal, and other
    /// shader pipelines at the correct cache directory for the given game.
    ///
    /// This ensures the cache directory exists before returning.
    pub fn setup_cache_env(
        &self,
        game_id: &str,
        backend: &str,
    ) -> HashMap<String, String> {
        let cache_dir = self.get_cache_dir(game_id, backend);
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!("Failed to create shader cache dir: {e}");
        }

        let cache_path = cache_dir.to_string_lossy().to_string();
        let mut env = HashMap::new();

        // DXVK state cache
        env.insert("DXVK_STATE_CACHE_PATH".to_string(), cache_path.clone());

        // D3DMetal shader cache
        env.insert("D3DM_SHADER_CACHE_PATH".to_string(), cache_path.clone());

        // General GL shader disk cache
        env.insert(
            "__GL_SHADER_DISK_CACHE_PATH".to_string(),
            cache_path.clone(),
        );

        // Enable GL shader disk cache
        env.insert("__GL_SHADER_DISK_CACHE".to_string(), "1".to_string());

        tracing::debug!(
            "Shader cache env configured for {game_id}/{backend} at {cache_path}"
        );

        env
    }

    /// Write or update the cache metadata file for a given game and backend.
    pub fn write_cache_meta(&self, info: &CacheInfo) -> Result<()> {
        let cache_dir = self.get_cache_dir(&info.game_id, &info.backend);
        std::fs::create_dir_all(&cache_dir)?;
        let meta_path = cache_dir.join("cache_meta.json");
        let json = serde_json::to_string_pretty(info)?;
        std::fs::write(&meta_path, json)?;
        Ok(())
    }
}

/// Recursively compute the total size of a directory.
fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                total += dir_size(&entry.path())?;
            } else {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cache_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());
        let dir = mgr.get_cache_dir("game123", "dxvk");
        assert!(dir.ends_with("shader-cache/game123/dxvk"));
    }

    #[test]
    fn test_cache_info_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());
        let info = mgr.cache_info("game123", "dxvk").unwrap();
        assert!(info.is_none());
    }

    #[test]
    fn test_cache_info_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());

        let info = CacheInfo {
            game_id: "game123".to_string(),
            backend: "dxvk".to_string(),
            shader_count: 42,
            size_bytes: 1024,
            last_updated: "2024-01-01".to_string(),
            version: "1.0".to_string(),
        };
        mgr.write_cache_meta(&info).unwrap();

        let read_info = mgr.cache_info("game123", "dxvk").unwrap().unwrap();
        assert_eq!(read_info.shader_count, 42);
        assert_eq!(read_info.game_id, "game123");
    }

    #[test]
    fn test_setup_cache_env() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());
        let env = mgr.setup_cache_env("game456", "d3dmetal");

        assert!(env.contains_key("DXVK_STATE_CACHE_PATH"));
        assert!(env.contains_key("D3DM_SHADER_CACHE_PATH"));
        assert!(env.contains_key("__GL_SHADER_DISK_CACHE_PATH"));
        assert_eq!(env.get("__GL_SHADER_DISK_CACHE"), Some(&"1".to_string()));

        // The cache dir should have been created
        let cache_dir = mgr.get_cache_dir("game456", "d3dmetal");
        assert!(cache_dir.exists());
    }

    #[test]
    fn test_list_caches_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());
        let caches = mgr.list_caches().unwrap();
        assert!(caches.is_empty());
    }

    #[test]
    fn test_list_caches_with_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());

        // Write two caches
        mgr.write_cache_meta(&CacheInfo {
            game_id: "game1".to_string(),
            backend: "dxvk".to_string(),
            shader_count: 10,
            size_bytes: 500,
            last_updated: "2024-01-01".to_string(),
            version: "1.0".to_string(),
        }).unwrap();

        mgr.write_cache_meta(&CacheInfo {
            game_id: "game2".to_string(),
            backend: "d3dmetal".to_string(),
            shader_count: 20,
            size_bytes: 1000,
            last_updated: "2024-02-01".to_string(),
            version: "1.0".to_string(),
        }).unwrap();

        let caches = mgr.list_caches().unwrap();
        assert_eq!(caches.len(), 2);
    }

    #[test]
    fn test_total_cache_size() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());

        // No caches = 0 bytes (or just metadata dir)
        let size = mgr.total_cache_size().unwrap();
        // Should be 0 or very small (just the empty dir)
        assert!(size < 1024);
    }

    #[test]
    fn test_clear_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());

        mgr.write_cache_meta(&CacheInfo {
            game_id: "game1".to_string(),
            backend: "dxvk".to_string(),
            shader_count: 10,
            size_bytes: 500,
            last_updated: "2024-01-01".to_string(),
            version: "1.0".to_string(),
        }).unwrap();

        assert!(mgr.get_cache_dir("game1", "dxvk").exists());
        mgr.clear_cache("game1", "dxvk").unwrap();
        assert!(!mgr.get_cache_dir("game1", "dxvk").exists());
    }

    #[test]
    fn test_clear_cache_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = ShaderCacheManager::new(tmp.path().to_path_buf());
        let result = mgr.clear_cache("nonexistent", "dxvk");
        assert!(result.is_err());
    }
}
