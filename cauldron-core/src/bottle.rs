use cauldron_db::GraphicsBackend;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BottleError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bottle not found: {0}")]
    NotFound(String),
    #[error("Bottle already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid bottle config: {0}")]
    InvalidConfig(String),
    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
}

/// A Wine bottle (prefix) managed by Cauldron.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottle {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub wine_version: String,
    pub graphics_backend: GraphicsBackend,
    pub created_at: String,
    pub env_overrides: HashMap<String, String>,
}

/// Manages creation, listing, and deletion of Wine bottles.
pub struct BottleManager {
    pub bottles_dir: PathBuf,
    pub base_dir: PathBuf,
}

impl BottleManager {
    /// Create a new BottleManager rooted at the given base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        let bottles_dir = base_dir.join("bottles");
        tracing::debug!(bottles_dir = %bottles_dir.display(), "BottleManager initialized");
        Self {
            bottles_dir,
            base_dir,
        }
    }

    /// Try to find a Wine binary for the given version string.
    ///
    /// Looks in the `wine-versions/<version>` directory managed by the
    /// WineManager. If no Wine version is installed, returns `None`.
    pub fn find_wine_for_version(&self, wine_version: &str) -> Option<PathBuf> {
        let version_dir = self.base_dir.join("wine-versions").join(wine_version);
        if version_dir.exists() {
            crate::wine_downloader::find_wine_binary(&version_dir).ok()
        } else {
            None
        }
    }

    /// Return the path to the latest installed Wine binary, if any.
    pub fn find_latest_wine(&self) -> Option<PathBuf> {
        let wine_mgr = crate::wine_downloader::WineManager::new(self.base_dir.clone());
        wine_mgr.latest_installed_wine_binary()
    }

    /// Create a new bottle with the given name and Wine version.
    pub fn create(
        &self,
        name: &str,
        wine_version: &str,
    ) -> Result<Bottle, BottleError> {
        let id = uuid::Uuid::new_v4().to_string();
        let bottle_path = self.bottles_dir.join(&id);

        if bottle_path.exists() {
            return Err(BottleError::AlreadyExists(id));
        }

        tracing::info!(bottle_name = %name, bottle_id = %id, path = %bottle_path.display(), "Creating new bottle");

        // Create the standard Wine prefix directory structure
        tracing::debug!(path = %bottle_path.display(), "Creating directory structure: drive_c, system32, dosdevices");
        std::fs::create_dir_all(bottle_path.join("drive_c/windows/system32"))?;
        std::fs::create_dir_all(bottle_path.join("drive_c/users/crossover"))?;
        std::fs::create_dir_all(bottle_path.join("dosdevices"))?;

        // Symlink drive_c as c:
        #[cfg(unix)]
        {
            let c_link = bottle_path.join("dosdevices/c:");
            if !c_link.exists() {
                std::os::unix::fs::symlink(bottle_path.join("drive_c"), &c_link)?;
            }
        }

        let now = chrono_like_timestamp();

        let bottle = Bottle {
            id,
            name: name.to_string(),
            path: bottle_path.clone(),
            wine_version: wine_version.to_string(),
            graphics_backend: GraphicsBackend::Auto,
            created_at: now,
            env_overrides: HashMap::new(),
        };

        // Write bottle.toml config
        let config_toml = toml::to_string_pretty(&bottle)?;
        std::fs::write(bottle_path.join("bottle.toml"), config_toml)?;

        tracing::info!(bottle_id = %bottle.id, bottle_name = %bottle.name, wine_version = %wine_version, "Bottle created successfully");
        Ok(bottle)
    }

    /// List all bottles by scanning the bottles directory.
    pub fn list(&self) -> Result<Vec<Bottle>, BottleError> {
        let mut bottles = Vec::new();

        if !self.bottles_dir.exists() {
            tracing::info!("Bottles directory does not exist yet, returning empty list");
            return Ok(bottles);
        }

        for entry in std::fs::read_dir(&self.bottles_dir)? {
            let entry = entry?;
            let config_path = entry.path().join("bottle.toml");
            if config_path.exists() {
                let contents = std::fs::read_to_string(&config_path)?;
                match toml::from_str::<Bottle>(&contents) {
                    Ok(bottle) => bottles.push(bottle),
                    Err(e) => {
                        tracing::warn!(
                            "Skipping invalid bottle config at {}: {e}",
                            config_path.display()
                        );
                    }
                }
            }
        }

        tracing::info!("Found {} bottles", bottles.len());
        Ok(bottles)
    }

    /// Delete a bottle by its ID.
    pub fn delete(&self, id: &str) -> Result<(), BottleError> {
        let bottle_path = self.bottles_dir.join(id);
        if !bottle_path.exists() {
            tracing::error!(bottle_id = %id, "Cannot delete bottle: not found");
            return Err(BottleError::NotFound(id.to_string()));
        }
        tracing::info!(bottle_id = %id, path = %bottle_path.display(), "Deleting bottle");
        std::fs::remove_dir_all(&bottle_path)?;
        tracing::info!(bottle_id = %id, "Bottle deleted successfully");
        Ok(())
    }

    /// Initialize a Wine prefix by running `wineboot --init`.
    ///
    /// This properly sets up a new bottle with all the registry entries and
    /// directory structures that Wine expects. Should be called after `create()`
    /// with the path to a valid Wine binary.
    pub fn wine_prefix_init(
        &self,
        wine_bin: &std::path::Path,
        bottle_path: &std::path::Path,
    ) -> Result<(), BottleError> {
        tracing::info!(
            "Initializing Wine prefix at {} with {}",
            bottle_path.display(),
            wine_bin.display()
        );

        let wineboot = wine_bin
            .parent()
            .map(|bin_dir| bin_dir.join("wineboot"))
            .unwrap_or_else(|| std::path::PathBuf::from("wineboot"));

        let status = std::process::Command::new(&wineboot)
            .arg("--init")
            .env("WINEPREFIX", bottle_path)
            .env("WINEDEBUG", "-all")
            .status()?;

        if !status.success() {
            tracing::error!(status = %status, path = %bottle_path.display(), "wineboot --init failed");
            return Err(BottleError::InvalidConfig(format!(
                "wineboot --init exited with status: {status}"
            )));
        }

        tracing::info!(path = %bottle_path.display(), "Wine prefix initialized successfully");
        Ok(())
    }

    /// Get a single bottle by its ID.
    pub fn get(&self, id: &str) -> Result<Bottle, BottleError> {
        let config_path = self.bottles_dir.join(id).join("bottle.toml");
        if !config_path.exists() {
            tracing::debug!(bottle_id = %id, "Bottle not found");
            return Err(BottleError::NotFound(id.to_string()));
        }
        let contents = std::fs::read_to_string(&config_path)?;
        let bottle: Bottle = toml::from_str(&contents)?;
        tracing::debug!(bottle_id = %id, bottle_name = %bottle.name, "Loaded bottle config");
        Ok(bottle)
    }
}

/// Generate a simple ISO-8601-ish timestamp without pulling in chrono.
pub fn chrono_like_timestamp() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Simple UTC timestamp: days since epoch math
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    // Compute year/month/day from days since epoch (1970-01-01)
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let year_days = if is_leap(y) { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            m = i;
            break;
        }
        remaining -= md as i64;
    }
    let d = remaining + 1;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        d,
        hours,
        minutes,
        seconds
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        let bottle = mgr.create("TestBottle", "wine-9.0").unwrap();

        assert_eq!(bottle.name, "TestBottle");
        assert_eq!(bottle.wine_version, "wine-9.0");
        assert!(bottle.path.exists());
        assert!(bottle.path.join("drive_c/windows/system32").exists());
        assert!(bottle.path.join("drive_c/users/crossover").exists());
        assert!(bottle.path.join("dosdevices").exists());
        assert!(bottle.path.join("bottle.toml").exists());
        assert_eq!(bottle.graphics_backend, GraphicsBackend::Auto);
    }

    #[test]
    fn test_list_bottles_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        let bottles = mgr.list().unwrap();
        assert!(bottles.is_empty());
    }

    #[test]
    fn test_list_bottles_after_create() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        mgr.create("Bottle1", "wine-9.0").unwrap();
        mgr.create("Bottle2", "wine-10.0").unwrap();

        let bottles = mgr.list().unwrap();
        assert_eq!(bottles.len(), 2);
        let names: Vec<&str> = bottles.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"Bottle1"));
        assert!(names.contains(&"Bottle2"));
    }

    #[test]
    fn test_get_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        let created = mgr.create("GetTest", "wine-9.0").unwrap();

        let fetched = mgr.get(&created.id).unwrap();
        assert_eq!(fetched.name, "GetTest");
        assert_eq!(fetched.id, created.id);
    }

    #[test]
    fn test_get_bottle_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        let result = mgr.get("nonexistent-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        let bottle = mgr.create("DeleteMe", "wine-9.0").unwrap();
        let path = bottle.path.clone();

        assert!(path.exists());
        mgr.delete(&bottle.id).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_bottle_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = BottleManager::new(tmp.path().to_path_buf());
        let result = mgr.delete("nonexistent-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_chrono_like_timestamp_format() {
        let ts = chrono_like_timestamp();
        // Should match ISO-8601 pattern: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap(2000));
        assert!(is_leap(2024));
        assert!(!is_leap(1900));
        assert!(!is_leap(2023));
    }
}
