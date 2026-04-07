//! Binary patching for game executables.
//!
//! Supports two modes:
//! - **pattern**: Search for a byte pattern and replace it (anywhere in the file).
//! - **offset**: Seek to a specific file offset and overwrite bytes (requires matching exe_hash).

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("DB error: {0}")]
    Db(#[from] cauldron_db::DbError),
    #[error("Pattern not found in executable")]
    PatternNotFound,
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("Offset out of bounds: offset {offset}, file size {file_size}")]
    OffsetOutOfBounds { offset: u64, file_size: u64 },
}

/// Compute the SHA-256 hash of a file.
pub fn hash_file(path: &Path) -> Result<String, PatchError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Apply all enabled binary patches for a game to the given executable.
///
/// Returns the number of patches successfully applied.
pub fn apply_game_patches(
    conn: &Connection,
    app_id: u32,
    exe_path: &Path,
) -> Result<usize, PatchError> {
    let exe_name = exe_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let patches = cauldron_db::get_game_binary_patches(conn, app_id, exe_name)?;
    if patches.is_empty() {
        return Ok(0);
    }

    let file_hash = hash_file(exe_path)?;
    let mut applied = 0;

    for patch in &patches {
        match patch.patch_mode.as_str() {
            "offset" => {
                // Offset mode: requires hash match and valid offset
                if patch.exe_hash != file_hash {
                    tracing::warn!(
                        patch_id = patch.id,
                        "Skipping offset patch: hash mismatch (expected {}, got {})",
                        patch.exe_hash,
                        file_hash,
                    );
                    continue;
                }

                let offset = match patch.file_offset {
                    Some(o) if o >= 0 => o as u64,
                    _ => {
                        tracing::warn!(patch_id = patch.id, "Skipping offset patch: no valid offset");
                        continue;
                    }
                };

                let file_size = fs::metadata(exe_path)?.len();
                if offset + patch.replace_pattern.len() as u64 > file_size {
                    tracing::warn!(
                        patch_id = patch.id,
                        "Skipping offset patch: offset out of bounds"
                    );
                    continue;
                }

                let mut file = fs::OpenOptions::new().write(true).open(exe_path)?;
                file.seek(SeekFrom::Start(offset))?;
                file.write_all(&patch.replace_pattern)?;
                applied += 1;
                tracing::info!(
                    patch_id = patch.id,
                    offset = offset,
                    "Applied offset patch: {}",
                    patch.description
                );
            }
            _ => {
                // Pattern mode (default): search and replace byte pattern
                let mut data = fs::read(exe_path)?;
                if let Some(pos) = find_pattern(&data, &patch.search_pattern) {
                    data[pos..pos + patch.replace_pattern.len()]
                        .copy_from_slice(&patch.replace_pattern);
                    fs::write(exe_path, &data)?;
                    applied += 1;
                    tracing::info!(
                        patch_id = patch.id,
                        position = pos,
                        "Applied pattern patch: {}",
                        patch.description
                    );
                } else {
                    tracing::debug!(
                        patch_id = patch.id,
                        "Pattern not found, skipping: {}",
                        patch.description
                    );
                }
            }
        }
    }

    Ok(applied)
}

/// Find the first occurrence of `pattern` in `data`.
fn find_pattern(data: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() || pattern.len() > data.len() {
        return None;
    }
    data.windows(pattern.len())
        .position(|window| window == pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cauldron_db::schema::run_migrations;
    use rusqlite::Connection;
    use std::io::Write as _;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_find_pattern() {
        let data = b"hello world test pattern here";
        assert_eq!(find_pattern(data, b"test"), Some(12));
        assert_eq!(find_pattern(data, b"missing"), None);
        assert_eq!(find_pattern(data, b""), None);
    }

    #[test]
    fn test_apply_pattern_patch() {
        let conn = setup_db();
        let dir = tempfile::tempdir().unwrap();
        let exe_path = dir.path().join("game.exe");

        // Write a fake executable
        let mut f = fs::File::create(&exe_path).unwrap();
        f.write_all(b"HEADER__SEARCH_PAT__FOOTER").unwrap();

        let hash = hash_file(&exe_path).unwrap();

        let patch = cauldron_db::GameBinaryPatchRecord {
            id: 0,
            steam_app_id: 100,
            exe_name: "game.exe".to_string(),
            exe_hash: hash,
            description: "test patch".to_string(),
            search_pattern: b"SEARCH_PAT".to_vec(),
            replace_pattern: b"REPLACE_OK".to_vec(),
            enabled: true,
            patch_mode: "pattern".to_string(),
            file_offset: None,
        };
        cauldron_db::insert_game_binary_patch(&conn, &patch).unwrap();

        let applied = apply_game_patches(&conn, 100, &exe_path).unwrap();
        assert_eq!(applied, 1);

        let content = fs::read(&exe_path).unwrap();
        assert!(content.windows(10).any(|w| w == b"REPLACE_OK"));
    }

    #[test]
    fn test_apply_offset_patch() {
        let conn = setup_db();
        let dir = tempfile::tempdir().unwrap();
        let exe_path = dir.path().join("game.exe");

        let data = b"0123456789ABCDEF";
        fs::write(&exe_path, data).unwrap();

        let hash = hash_file(&exe_path).unwrap();

        let patch = cauldron_db::GameBinaryPatchRecord {
            id: 0,
            steam_app_id: 200,
            exe_name: "game.exe".to_string(),
            exe_hash: hash,
            description: "offset patch".to_string(),
            search_pattern: vec![],
            replace_pattern: b"XX".to_vec(),
            enabled: true,
            patch_mode: "offset".to_string(),
            file_offset: Some(4),
        };
        cauldron_db::insert_game_binary_patch(&conn, &patch).unwrap();

        let applied = apply_game_patches(&conn, 200, &exe_path).unwrap();
        assert_eq!(applied, 1);

        let content = fs::read(&exe_path).unwrap();
        assert_eq!(&content[4..6], b"XX");
    }

    #[test]
    fn test_offset_patch_hash_mismatch() {
        let conn = setup_db();
        let dir = tempfile::tempdir().unwrap();
        let exe_path = dir.path().join("game.exe");
        fs::write(&exe_path, b"some data").unwrap();

        let patch = cauldron_db::GameBinaryPatchRecord {
            id: 0,
            steam_app_id: 300,
            exe_name: "game.exe".to_string(),
            exe_hash: "wrong_hash".to_string(),
            description: "bad hash patch".to_string(),
            search_pattern: vec![],
            replace_pattern: b"XX".to_vec(),
            enabled: true,
            patch_mode: "offset".to_string(),
            file_offset: Some(0),
        };
        cauldron_db::insert_game_binary_patch(&conn, &patch).unwrap();

        let applied = apply_game_patches(&conn, 300, &exe_path).unwrap();
        assert_eq!(applied, 0); // Should skip due to hash mismatch
    }
}
