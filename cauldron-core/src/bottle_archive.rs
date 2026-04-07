use crate::bottle::Bottle;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Archive operation failed: {0}")]
    ArchiveFailed(String),
    #[error("Invalid archive: {0}")]
    InvalidArchive(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

/// Metadata about an exported bottle archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInfo {
    pub name: String,
    pub wine_version: String,
    pub graphics_backend: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub game_count: usize,
}

/// Export a bottle directory as a `.tar.gz` archive.
///
/// Creates a `manifest.json` inside the archive root containing [`ArchiveInfo`].
/// Uses the system `tar` command for compression.
pub fn export_bottle(bottle_path: &Path, output_path: &Path) -> Result<ArchiveInfo, ArchiveError> {
    tracing::info!(bottle_path = %bottle_path.display(), output = %output_path.display(), "Starting bottle export");
    if !bottle_path.exists() {
        return Err(ArchiveError::InvalidArchive(format!(
            "Bottle path does not exist: {}",
            bottle_path.display()
        )));
    }

    // Read the bottle config to populate archive info
    let config_path = bottle_path.join("bottle.toml");
    let config_contents = std::fs::read_to_string(&config_path).map_err(|e| {
        ArchiveError::InvalidArchive(format!("Cannot read bottle.toml: {e}"))
    })?;
    let bottle: Bottle = toml::from_str(&config_contents)?;

    let size_bytes = estimate_archive_size(bottle_path)?;

    // Count games by looking for .exe files in drive_c (rough heuristic)
    let game_count = count_executables(bottle_path);

    let info = ArchiveInfo {
        name: bottle.name.clone(),
        wine_version: bottle.wine_version.clone(),
        graphics_backend: format!("{:?}", bottle.graphics_backend),
        created_at: bottle.created_at.clone(),
        size_bytes,
        game_count,
    };

    // Write manifest.json into the bottle directory temporarily
    let manifest_path = bottle_path.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&info)?;
    std::fs::write(&manifest_path, &manifest_json)?;

    // Determine the parent directory and folder name for tar
    let parent = bottle_path.parent().ok_or_else(|| {
        ArchiveError::ArchiveFailed("Bottle path has no parent directory".to_string())
    })?;
    let folder_name = bottle_path
        .file_name()
        .ok_or_else(|| {
            ArchiveError::ArchiveFailed("Bottle path has no folder name".to_string())
        })?
        .to_string_lossy();

    // Create the tar.gz archive
    let status = std::process::Command::new("tar")
        .arg("czf")
        .arg(output_path)
        .arg("-C")
        .arg(parent)
        .arg(folder_name.as_ref())
        .status()?;

    // Clean up temporary manifest from source directory
    let _ = std::fs::remove_file(&manifest_path);

    if !status.success() {
        return Err(ArchiveError::ArchiveFailed(format!(
            "tar exited with status: {status}"
        )));
    }

    tracing::info!(
        bottle_name = %bottle.name,
        output = %output_path.display(),
        size_bytes = size_bytes,
        game_count = game_count,
        "Bottle export complete"
    );
    Ok(info)
}

/// Import a bottle from a `.tar.gz` archive into the bottles directory.
///
/// Extracts the archive, validates the manifest, and returns the path to the
/// imported bottle.
pub fn import_bottle(archive_path: &Path, bottles_dir: &Path) -> Result<PathBuf, ArchiveError> {
    tracing::info!(archive = %archive_path.display(), dest = %bottles_dir.display(), "Starting bottle import");
    if !archive_path.exists() {
        return Err(ArchiveError::InvalidArchive(format!(
            "Archive not found: {}",
            archive_path.display()
        )));
    }

    std::fs::create_dir_all(bottles_dir)?;

    // Extract into a temporary location first so we can discover the folder name
    let temp_dir = tempfile::tempdir_in(bottles_dir)?;

    let status = std::process::Command::new("tar")
        .arg("xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(temp_dir.path())
        .status()?;

    if !status.success() {
        return Err(ArchiveError::ArchiveFailed(format!(
            "tar extraction failed with status: {status}"
        )));
    }

    // Find the extracted folder (should be a single top-level directory)
    let mut entries = std::fs::read_dir(temp_dir.path())?;
    let extracted = entries
        .next()
        .ok_or_else(|| ArchiveError::InvalidArchive("Archive is empty".to_string()))??;
    let extracted_path = extracted.path();

    if !extracted_path.is_dir() {
        return Err(ArchiveError::InvalidArchive(
            "Archive does not contain a directory at root level".to_string(),
        ));
    }

    // Validate manifest
    let manifest_path = extracted_path.join("manifest.json");
    if manifest_path.exists() {
        let manifest_contents = std::fs::read_to_string(&manifest_path)?;
        let _info: ArchiveInfo = serde_json::from_str(&manifest_contents).map_err(|e| {
            ArchiveError::InvalidArchive(format!("Invalid manifest.json: {e}"))
        })?;
        // Clean up manifest from the imported bottle
        let _ = std::fs::remove_file(&manifest_path);
    }

    // Validate bottle.toml exists
    let config_path = extracted_path.join("bottle.toml");
    if !config_path.exists() {
        return Err(ArchiveError::InvalidArchive(
            "Archive does not contain a bottle.toml".to_string(),
        ));
    }

    // Move extracted bottle to the final location
    let folder_name = extracted_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let final_path = bottles_dir.join(&folder_name);

    if final_path.exists() {
        return Err(ArchiveError::ArchiveFailed(format!(
            "Destination already exists: {}",
            final_path.display()
        )));
    }

    // Use rename if same filesystem, otherwise fall back to copy
    if std::fs::rename(&extracted_path, &final_path).is_err() {
        copy_dir_recursive(&extracted_path, &final_path)?;
        std::fs::remove_dir_all(&extracted_path)?;
    }

    tracing::info!(path = %final_path.display(), "Bottle imported successfully");
    Ok(final_path)
}

/// Estimate the total size of a bottle directory by walking the file tree.
pub fn estimate_archive_size(bottle_path: &Path) -> Result<u64, ArchiveError> {
    let mut total: u64 = 0;
    walk_dir_size(bottle_path, &mut total)?;
    Ok(total)
}

fn walk_dir_size(dir: &Path, total: &mut u64) -> Result<(), ArchiveError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_file() {
            *total += entry.metadata()?.len();
        } else if ft.is_dir() {
            walk_dir_size(&entry.path(), total)?;
        }
        // Skip symlinks to avoid double-counting
    }
    Ok(())
}

/// Duplicate a bottle to a new directory with a new name and ID.
pub fn duplicate_bottle(
    source_path: &Path,
    bottles_dir: &Path,
    new_name: &str,
) -> Result<PathBuf, ArchiveError> {
    tracing::info!(source = %source_path.display(), new_name = %new_name, "Duplicating bottle");
    if !source_path.exists() {
        return Err(ArchiveError::InvalidArchive(format!(
            "Source bottle not found: {}",
            source_path.display()
        )));
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let dest_path = bottles_dir.join(&new_id);

    std::fs::create_dir_all(bottles_dir)?;
    copy_dir_recursive(source_path, &dest_path)?;

    // Update bottle.toml with new name and ID
    let config_path = dest_path.join("bottle.toml");
    if config_path.exists() {
        let contents = std::fs::read_to_string(&config_path)?;
        let mut bottle: Bottle = toml::from_str(&contents)?;
        bottle.id = new_id.clone();
        bottle.name = new_name.to_string();
        bottle.path = dest_path.clone();
        let updated = toml::to_string_pretty(&bottle)?;
        std::fs::write(&config_path, updated)?;
    }

    tracing::info!(
        "Duplicated bottle to '{}' at {}",
        new_name,
        dest_path.display()
    );
    Ok(dest_path)
}

/// Recursively copy a directory tree.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), ArchiveError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let dest_entry = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_entry)?;
        } else if ft.is_symlink() {
            #[cfg(unix)]
            {
                let target = std::fs::read_link(entry.path())?;
                std::os::unix::fs::symlink(target, &dest_entry)?;
            }
        } else {
            std::fs::copy(entry.path(), &dest_entry)?;
        }
    }
    Ok(())
}

/// Count .exe files in the bottle's drive_c to roughly estimate game count.
fn count_executables(bottle_path: &Path) -> usize {
    let drive_c = bottle_path.join("drive_c");
    if !drive_c.exists() {
        return 0;
    }
    count_exe_recursive(&drive_c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal bottle directory for testing.
    fn create_test_bottle(tmp: &std::path::Path, name: &str) -> PathBuf {
        let mgr = crate::bottle::BottleManager::new(tmp.to_path_buf());
        let bottle = mgr.create(name, "wine-9.0").unwrap();
        bottle.path
    }

    #[test]
    fn test_estimate_archive_size() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle_path = create_test_bottle(tmp.path(), "SizeTest");

        // Write some extra data
        std::fs::write(
            bottle_path.join("drive_c/test.txt"),
            "hello world",
        ).unwrap();

        let size = estimate_archive_size(&bottle_path).unwrap();
        assert!(size > 0);
    }

    #[test]
    fn test_estimate_archive_size_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();

        let size = estimate_archive_size(&empty).unwrap();
        assert_eq!(size, 0);
    }

    #[test]
    fn test_duplicate_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle_path = create_test_bottle(tmp.path(), "Original");
        let bottles_dir = tmp.path().join("bottles");

        let dup_path = duplicate_bottle(&bottle_path, &bottles_dir, "Clone").unwrap();

        assert!(dup_path.exists());
        assert!(dup_path.join("bottle.toml").exists());
        assert!(dup_path.join("drive_c/windows/system32").exists());

        // The duplicated bottle should have a different name and id
        let contents = std::fs::read_to_string(dup_path.join("bottle.toml")).unwrap();
        let dup_bottle: crate::bottle::Bottle = toml::from_str(&contents).unwrap();
        assert_eq!(dup_bottle.name, "Clone");
        assert_ne!(dup_bottle.path, bottle_path);
    }

    #[test]
    fn test_duplicate_bottle_source_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = duplicate_bottle(
            &tmp.path().join("nonexistent"),
            &tmp.path().join("bottles"),
            "Clone",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_count_executables() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = tmp.path().join("bottle");
        let game_dir = bottle.join("drive_c/games");
        std::fs::create_dir_all(&game_dir).unwrap();

        std::fs::write(game_dir.join("game1.exe"), "fake").unwrap();
        std::fs::write(game_dir.join("game2.exe"), "fake").unwrap();
        std::fs::write(game_dir.join("readme.txt"), "text").unwrap();

        let count = count_executables(&bottle);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_executables_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let count = count_executables(tmp.path());
        assert_eq!(count, 0);
    }
}

fn count_exe_recursive(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    count += count_exe_recursive(&entry.path());
                } else if ft.is_file() {
                    if let Some(ext) = entry.path().extension() {
                        if ext.eq_ignore_ascii_case("exe") {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    count
}
