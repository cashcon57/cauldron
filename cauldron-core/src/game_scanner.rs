use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ACF parse error: {0}")]
    AcfParse(String),
}

type Result<T> = std::result::Result<T, ScanError>;

/// A game detected inside a bottle.
#[derive(Debug, Clone)]
pub struct DetectedGame {
    pub title: String,
    pub exe_path: PathBuf,
    pub exe_hash: Option<String>,
    pub steam_app_id: Option<u32>,
    pub bottle_id: String,
    pub size_bytes: u64,
    pub dx_version: Option<u8>,
}

/// Parsed contents of a Steam `.acf` app manifest.
#[derive(Debug, Clone)]
pub struct AcfManifest {
    pub app_id: u32,
    pub name: String,
    pub install_dir: String,
    pub size_on_disk: u64,
    pub state: u32,
}

/// System/Wine executables that should be excluded from game detection.
const SYSTEM_EXES: &[&str] = &[
    "notepad.exe",
    "regedit.exe",
    "cmd.exe",
    "explorer.exe",
    "winecfg.exe",
    "wineboot.exe",
    "taskmgr.exe",
    "control.exe",
    "winefile.exe",
    "winemine.exe",
    "wordpad.exe",
    "msiexec.exe",
    "regsvr32.exe",
    "rundll32.exe",
    "start.exe",
    "uninstaller.exe",
    "plugplay.exe",
    "services.exe",
    "winedevice.exe",
    "conhost.exe",
    "svchost.exe",
    "rpcss.exe",
    "tabtip.exe",
    "wmplayer.exe",
    "iexplore.exe",
    // Steam runtime & tools
    "steam.exe",
    "steamerrorreporter.exe",
    "steamerrorreporter64.exe",
    "steamservice.exe",
    "steamwebhelper.exe",
    "gameoverlayui.exe",
    "gameoverlayui64.exe",
    "streaming_client.exe",
    "uninstall.exe",
    "crashhandler.exe",
    "crashhandler64.exe",
    "steamless.exe",
    "steamless.cli.exe",
    "x64launcher.exe",
    "gldriverquery.exe",
    "gldriverquery64.exe",
    "vulkandriverquery.exe",
    "vulkandriverquery64.exe",
    "drivers.exe",
    "secure_desktop_capture.exe",
    "fossilize-replay.exe",
    "fossilize_replay.exe",
    "fossilize-layer.exe",
    "html5app_steam.exe",
    "steamtours.exe",
    "writeminidump.exe",
    "minidumps.exe",
    "steamnew.exe",
    "steam_monitor.exe",
    "bootstrapper.exe",
    // Redistributables & installers
    "vc_redist.x86.exe",
    "vc_redist.x64.exe",
    "vcredist_x86.exe",
    "vcredist_x64.exe",
    "dxsetup.exe",
    "dxwebsetup.exe",
    "dotnetfx35setup.exe",
    "setup.exe",
    "install.exe",
    "installer.exe",
];

/// Directory names that should be skipped entirely during scanning.
const SKIP_DIRS: &[&str] = &[
    "windows",
    "programdata",
    "users",
    "__installerdata",
    "package cache",
    "steamless",
    "common files",
    "bin",
    "redist",
    "redistributables",
    "_commonredist",
    "__support",
    "directx",
    "vcredist",
    "dotnet",
    "installers",
    "support",
    "prerequisites",
];

/// Scans bottles for installed games.
pub struct GameScanner;

impl GameScanner {
    /// Recursively scan a bottle's `drive_c/` for `.exe` files, filtering out
    /// known Wine/system executables. Returns a list of detected games.
    pub fn scan_bottle(bottle_path: &Path, bottle_id: &str) -> Result<Vec<DetectedGame>> {
        tracing::info!(bottle_id = %bottle_id, path = %bottle_path.display(), "Scanning bottle for games");
        let drive_c = bottle_path.join("drive_c");
        if !drive_c.exists() {
            tracing::debug!(bottle_id = %bottle_id, "No drive_c directory found, skipping scan");
            return Ok(Vec::new());
        }

        let system_exes: HashSet<&str> = SYSTEM_EXES.iter().copied().collect();
        let mut games = Vec::new();

        Self::walk_for_exes(&drive_c, bottle_id, &system_exes, &mut games)?;
        tracing::info!(bottle_id = %bottle_id, games_found = games.len(), "Bottle scan complete");
        Ok(games)
    }

    fn walk_for_exes(
        dir: &Path,
        bottle_id: &str,
        system_exes: &HashSet<&str>,
        results: &mut Vec<DetectedGame>,
    ) -> Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if SKIP_DIRS.iter().any(|s| dir_name == *s) {
                    continue;
                }
                Self::walk_for_exes(&entry.path(), bottle_id, system_exes, results)?;
            } else if file_type.is_file() {
                let path = entry.path();
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if !name_str.to_ascii_lowercase().ends_with(".exe") {
                    continue;
                }

                if system_exes.contains(name_str.to_ascii_lowercase().as_str()) {
                    continue;
                }

                // Skip names matching common non-game patterns
                let lower_name = name_str.to_ascii_lowercase();
                if lower_name.contains("redist")
                    || lower_name.contains("uninstall")
                    || lower_name.contains("crashhandler")
                    || lower_name.contains("crashreport")
                    || lower_name.contains("errorreporter")
                    || lower_name.starts_with("vc_")
                    || lower_name.starts_with("vcredist")
                    || lower_name.starts_with("dxsetup")
                    || lower_name.starts_with("dotnet")
                {
                    continue;
                }

                let metadata = fs::metadata(&path)?;
                let size = metadata.len();

                // Skip very small exes (likely stubs)
                if size < 1024 {
                    continue;
                }

                let exe_hash = Self::hash_exe_head(&path).ok();
                let dx_version = Self::detect_dx_version(&path);

                // Derive a title from the file stem
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| name_str.to_string());

                tracing::debug!(exe = %path.display(), title = %title, size = size, dx = ?dx_version, "Detected game executable");
                results.push(DetectedGame {
                    title,
                    exe_path: path,
                    exe_hash,
                    steam_app_id: None,
                    bottle_id: bottle_id.to_string(),
                    size_bytes: size,
                    dx_version,
                });
            }
        }

        Ok(())
    }

    /// Look for a Steam library inside the bottle and parse `.acf` manifests
    /// to detect installed Steam games.
    pub fn detect_steam_apps(bottle_path: &Path) -> Result<Vec<DetectedGame>> {
        tracing::info!(path = %bottle_path.display(), "Detecting Steam apps in bottle");
        let steamapps = bottle_path
            .join("drive_c")
            .join("Program Files (x86)")
            .join("Steam")
            .join("steamapps");

        if !steamapps.exists() {
            tracing::debug!("No Steam library found in bottle");
            return Ok(Vec::new());
        }

        let mut games = Vec::new();

        for entry in fs::read_dir(&steamapps)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if !name_str.starts_with("appmanifest_") || !name_str.ends_with(".acf") {
                continue;
            }

            let manifest = match Self::parse_acf_file(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Try to find the main exe in the install directory
            let install_path = steamapps.join("common").join(&manifest.install_dir);

            let bottle_id = bottle_path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();

            if install_path.exists() {
                // Find the first exe in the install dir (shallow)
                let mut found_exe = false;
                if let Ok(entries) = fs::read_dir(&install_path) {
                    for exe_entry in entries.flatten() {
                        let exe_path = exe_entry.path();
                        if exe_path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                            && exe_path.is_file()
                        {
                            let metadata = fs::metadata(&exe_path)?;
                            let exe_hash = Self::hash_exe_head(&exe_path).ok();
                            let dx_version = Self::detect_dx_version(&exe_path);

                            games.push(DetectedGame {
                                title: manifest.name.clone(),
                                exe_path,
                                exe_hash,
                                steam_app_id: Some(manifest.app_id),
                                bottle_id: bottle_id.clone(),
                                size_bytes: metadata.len(),
                                dx_version,
                            });
                            found_exe = true;
                            break;
                        }
                    }
                }

                // If no exe found at top level, still record the game
                if !found_exe {
                    games.push(DetectedGame {
                        title: manifest.name.clone(),
                        exe_path: install_path,
                        exe_hash: None,
                        steam_app_id: Some(manifest.app_id),
                        bottle_id: bottle_id.clone(),
                        size_bytes: manifest.size_on_disk,
                        dx_version: None,
                    });
                }
            } else {
                // Install directory doesn't exist; record with manifest info
                let bottle_id = bottle_path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();

                games.push(DetectedGame {
                    title: manifest.name,
                    exe_path: install_path,
                    exe_hash: None,
                    steam_app_id: Some(manifest.app_id),
                    bottle_id,
                    size_bytes: manifest.size_on_disk,
                    dx_version: None,
                });
            }
        }

        Ok(games)
    }

    /// Parse a Valve ACF manifest file. The format uses quoted key-value pairs
    /// and nested braces, similar to VDF (Valve Data Format).
    pub fn parse_acf_file(path: &Path) -> Result<AcfManifest> {
        let content = fs::read_to_string(path)?;
        let kv = Self::parse_acf_top_level(&content)?;

        let app_id = kv
            .get("appid")
            .ok_or_else(|| ScanError::AcfParse("missing appid".into()))?
            .parse::<u32>()
            .map_err(|e| ScanError::AcfParse(format!("invalid appid: {e}")))?;

        let name = kv
            .get("name")
            .cloned()
            .unwrap_or_default();

        let install_dir = kv
            .get("installdir")
            .cloned()
            .unwrap_or_default();

        let size_on_disk = kv
            .get("SizeOnDisk")
            .or_else(|| kv.get("sizeondisk"))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let state = kv
            .get("StateFlags")
            .or_else(|| kv.get("stateflags"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        Ok(AcfManifest {
            app_id,
            name,
            install_dir,
            size_on_disk,
            state,
        })
    }

    /// Parse the top-level key-value pairs from ACF content, ignoring nested sections.
    fn parse_acf_top_level(content: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut map = std::collections::HashMap::new();
        let mut chars = content.chars().peekable();

        // Helper to skip whitespace
        fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars>) {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        }

        // Helper to read a quoted string
        fn read_quoted(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
            skip_ws(chars);
            if chars.peek() != Some(&'"') {
                return None;
            }
            chars.next(); // consume opening quote
            let mut s = String::new();
            loop {
                match chars.next() {
                    None => break,
                    Some('"') => return Some(s),
                    Some('\\') => {
                        if let Some(next) = chars.next() {
                            s.push(next);
                        }
                    }
                    Some(c) => s.push(c),
                }
            }
            Some(s)
        }

        // Skip to the first '{' (the root object)
        while chars.peek().is_some_and(|c| *c != '{') {
            chars.next();
        }
        if chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
        }

        let mut depth = 0;
        loop {
            skip_ws(&mut chars);
            match chars.peek() {
                None => break,
                Some(&'}') => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    chars.next();
                }
                Some(&'{') => {
                    depth += 1;
                    chars.next();
                }
                Some(&'"') if depth == 0 => {
                    let key = match read_quoted(&mut chars) {
                        Some(k) => k,
                        None => break,
                    };
                    skip_ws(&mut chars);
                    if chars.peek() == Some(&'{') {
                        // Nested section — skip it
                        chars.next();
                        depth += 1;
                    } else if chars.peek() == Some(&'"') {
                        if let Some(value) = read_quoted(&mut chars) {
                            map.insert(key, value);
                        }
                    }
                }
                _ => {
                    // Skip unexpected characters
                    chars.next();
                }
            }
        }

        Ok(map)
    }

    /// Detect the DirectX version an executable targets by reading its PE
    /// import table and looking for D3D DLL imports.
    pub fn detect_dx_version(exe_path: &Path) -> Option<u8> {
        tracing::trace!(exe = %exe_path.display(), "Detecting DirectX version via PE imports");
        let data = fs::read(exe_path).ok()?;
        let imports = Self::read_pe_imports(&data)?;

        let mut max_dx: Option<u8> = None;

        for dll_name in &imports {
            let lower = dll_name.to_ascii_lowercase();
            let dx = if lower == "d3d12.dll" {
                Some(12)
            } else if lower == "d3d11.dll" {
                Some(11)
            } else if lower == "d3d10.dll" || lower == "d3d10core.dll" {
                Some(10)
            } else if lower == "d3d9.dll" {
                Some(9)
            } else {
                None
            };

            if let Some(v) = dx {
                max_dx = Some(max_dx.map_or(v, |cur| cur.max(v)));
            }
        }

        max_dx
    }

    /// Read DLL import names from a PE file's import directory table.
    /// Handles both PE32 and PE32+ (64-bit) formats.
    fn read_pe_imports(data: &[u8]) -> Option<Vec<String>> {
        // Check DOS signature "MZ"
        if data.len() < 64 || data[0] != b'M' || data[1] != b'Z' {
            return None;
        }

        // e_lfanew at offset 0x3C (4 bytes, little-endian)
        let e_lfanew = u32::from_le_bytes(data[0x3C..0x40].try_into().ok()?) as usize;

        // Check PE signature "PE\0\0"
        if data.len() < e_lfanew + 4 {
            return None;
        }
        if &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
            return None;
        }

        let coff_offset = e_lfanew + 4;
        if data.len() < coff_offset + 20 {
            return None;
        }

        let number_of_sections =
            u16::from_le_bytes(data[coff_offset + 2..coff_offset + 4].try_into().ok()?) as usize;
        let size_of_optional =
            u16::from_le_bytes(data[coff_offset + 16..coff_offset + 18].try_into().ok()?) as usize;

        let optional_offset = coff_offset + 20;
        if data.len() < optional_offset + size_of_optional {
            return None;
        }

        // Determine PE32 vs PE32+
        let magic = u16::from_le_bytes(
            data[optional_offset..optional_offset + 2].try_into().ok()?,
        );

        let (import_dir_rva, import_dir_size, _is_pe32_plus) = match magic {
            0x10b => {
                // PE32
                let dd_offset = optional_offset + 96; // data directories start at offset 96
                if data.len() < dd_offset + 16 {
                    return None;
                }
                // Import table is the 2nd data directory entry (index 1)
                let rva =
                    u32::from_le_bytes(data[dd_offset + 8..dd_offset + 12].try_into().ok()?);
                let size =
                    u32::from_le_bytes(data[dd_offset + 12..dd_offset + 16].try_into().ok()?);
                (rva, size, false)
            }
            0x20b => {
                // PE32+ (64-bit)
                let dd_offset = optional_offset + 112;
                if data.len() < dd_offset + 16 {
                    return None;
                }
                let rva =
                    u32::from_le_bytes(data[dd_offset + 8..dd_offset + 12].try_into().ok()?);
                let size =
                    u32::from_le_bytes(data[dd_offset + 12..dd_offset + 16].try_into().ok()?);
                (rva, size, true)
            }
            _ => return None,
        };

        if import_dir_rva == 0 || import_dir_size == 0 {
            return None;
        }

        // Build section table to translate RVA -> file offset
        let sections_offset = optional_offset + size_of_optional;
        let sections = Self::parse_sections(data, sections_offset, number_of_sections)?;

        let import_offset = Self::rva_to_offset(import_dir_rva, &sections)?;

        let mut imports = Vec::new();

        // Each import descriptor is 20 bytes; the table is null-terminated
        let mut idx = import_offset;
        loop {
            if idx + 20 > data.len() {
                break;
            }

            let name_rva = u32::from_le_bytes(data[idx + 12..idx + 16].try_into().ok()?);

            // Null entry signals end of table
            if name_rva == 0 {
                break;
            }

            if let Some(name_offset) = Self::rva_to_offset(name_rva, &sections) {
                if let Some(name) = Self::read_cstring(data, name_offset) {
                    imports.push(name);
                }
            }

            idx += 20;
        }

        Some(imports)
    }

    fn parse_sections(
        data: &[u8],
        offset: usize,
        count: usize,
    ) -> Option<Vec<PeSection>> {
        let mut sections = Vec::with_capacity(count);
        for i in 0..count {
            let base = offset + i * 40;
            if base + 40 > data.len() {
                return None;
            }
            let virtual_size =
                u32::from_le_bytes(data[base + 8..base + 12].try_into().ok()?);
            let virtual_address =
                u32::from_le_bytes(data[base + 12..base + 16].try_into().ok()?);
            let pointer_to_raw_data =
                u32::from_le_bytes(data[base + 20..base + 24].try_into().ok()?);
            sections.push(PeSection {
                virtual_address,
                virtual_size,
                pointer_to_raw_data,
            });
        }
        Some(sections)
    }

    /// Convert an RVA to a file offset using the section table.
    fn rva_to_offset(rva: u32, sections: &[PeSection]) -> Option<usize> {
        for s in sections {
            if rva >= s.virtual_address && rva < s.virtual_address + s.virtual_size {
                return Some((rva - s.virtual_address + s.pointer_to_raw_data) as usize);
            }
        }
        None
    }

    /// Read a null-terminated C string from a byte slice.
    fn read_cstring(data: &[u8], offset: usize) -> Option<String> {
        if offset >= data.len() {
            return None;
        }
        let end = data[offset..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| offset + p)
            .unwrap_or(data.len());
        // Limit to a reasonable length to avoid garbage
        let len = end - offset;
        if len > 256 {
            return None;
        }
        String::from_utf8(data[offset..end].to_vec()).ok()
    }

    /// Read PE import DLL names from an executable file.
    /// Only reads the first 64KB (enough for PE headers + import table).
    pub fn read_pe_import_names(path: &Path) -> Vec<String> {
        let mut file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        // 64KB is sufficient for PE headers + import directory in virtually all executables
        let mut buf = vec![0u8; 65536];
        let bytes_read = match file.read(&mut buf) {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        };
        buf.truncate(bytes_read);
        Self::read_pe_imports(&buf).unwrap_or_default()
    }

    /// Compute the SHA-256 hash of the first 1 MB of a file for fast
    /// identification.
    pub fn hash_exe_head(path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)?;
        let mut buffer = vec![0u8; 1024 * 1024]; // 1 MB
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read);

        let mut hasher = Sha256::new();
        hasher.update(&buffer);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }
}

/// PE section header data used for RVA-to-offset translation.
struct PeSection {
    virtual_address: u32,
    virtual_size: u32,
    pointer_to_raw_data: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_acf_file() {
        let tmp = tempfile::tempdir().unwrap();
        let acf_path = tmp.path().join("appmanifest_1245620.acf");
        let content = r#"
"AppState"
{
    "appid"		"1245620"
    "Universe"		"1"
    "name"		"ELDEN RING"
    "StateFlags"		"4"
    "installdir"		"ELDEN RING"
    "SizeOnDisk"		"53687091200"
    "UserConfig"
    {
        "language"		"english"
    }
}
"#;
        std::fs::write(&acf_path, content).unwrap();

        let manifest = GameScanner::parse_acf_file(&acf_path).unwrap();
        assert_eq!(manifest.app_id, 1245620);
        assert_eq!(manifest.name, "ELDEN RING");
        assert_eq!(manifest.install_dir, "ELDEN RING");
        assert_eq!(manifest.size_on_disk, 53687091200);
        assert_eq!(manifest.state, 4);
    }

    #[test]
    fn test_parse_acf_file_missing_appid() {
        let tmp = tempfile::tempdir().unwrap();
        let acf_path = tmp.path().join("bad.acf");
        let content = r#"
"AppState"
{
    "name"		"Test Game"
}
"#;
        std::fs::write(&acf_path, content).unwrap();

        let result = GameScanner::parse_acf_file(&acf_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_exe_head() {
        let tmp = tempfile::tempdir().unwrap();
        let exe_path = tmp.path().join("test.exe");
        std::fs::write(&exe_path, b"some binary content for hashing").unwrap();

        let hash = GameScanner::hash_exe_head(&exe_path).unwrap();
        assert!(!hash.is_empty());
        // Hash should be hex
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        // SHA-256 hash is 64 hex chars
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_exe_head_consistent() {
        let tmp = tempfile::tempdir().unwrap();
        let exe_path = tmp.path().join("test.exe");
        std::fs::write(&exe_path, b"deterministic content").unwrap();

        let hash1 = GameScanner::hash_exe_head(&exe_path).unwrap();
        let hash2 = GameScanner::hash_exe_head(&exe_path).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_scan_bottle_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = tmp.path().join("bottle");
        std::fs::create_dir_all(bottle.join("drive_c")).unwrap();

        let games = GameScanner::scan_bottle(&bottle, "test-id").unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn test_scan_bottle_finds_exe() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = tmp.path().join("bottle");
        let game_dir = bottle.join("drive_c/games/TestGame");
        std::fs::create_dir_all(&game_dir).unwrap();

        // Create a fake exe (> 1024 bytes to not be filtered as stub)
        let exe_content = vec![0u8; 2048];
        std::fs::write(game_dir.join("game.exe"), &exe_content).unwrap();

        let games = GameScanner::scan_bottle(&bottle, "test-id").unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "game");
        assert_eq!(games[0].bottle_id, "test-id");
    }

    #[test]
    fn test_scan_bottle_filters_system_exes() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = tmp.path().join("bottle");
        let sys_dir = bottle.join("drive_c/windows/system32");
        std::fs::create_dir_all(&sys_dir).unwrap();

        let exe_content = vec![0u8; 2048];
        std::fs::write(sys_dir.join("notepad.exe"), &exe_content).unwrap();
        std::fs::write(sys_dir.join("regedit.exe"), &exe_content).unwrap();

        let games = GameScanner::scan_bottle(&bottle, "test-id").unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn test_scan_bottle_filters_small_exes() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = tmp.path().join("bottle");
        let dir = bottle.join("drive_c/games");
        std::fs::create_dir_all(&dir).unwrap();

        // Too small -- should be filtered
        std::fs::write(dir.join("stub.exe"), b"tiny").unwrap();

        let games = GameScanner::scan_bottle(&bottle, "test-id").unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn test_scan_bottle_no_drive_c() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = tmp.path().join("bottle");
        std::fs::create_dir_all(&bottle).unwrap();

        let games = GameScanner::scan_bottle(&bottle, "test-id").unwrap();
        assert!(games.is_empty());
    }
}
