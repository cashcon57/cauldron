use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// An action extracted from a protonfixes game script.
#[derive(Debug, Clone, PartialEq)]
pub enum FixAction {
    /// Install a verb via protontricks (e.g., `vcrun2019`, `dotnet48`).
    InstallVerb(String),
    /// Append a command-line argument to the game launch command.
    AppendArgument(String),
    /// Replace one executable with another in the launch command.
    ReplaceCommand { from: String, to: String },
    /// Set an environment variable.
    SetEnvVar { key: String, value: String },
    /// Apply a DLL override directive.
    DllOverride { dll: String, mode: String },
    /// Disable NVIDIA API (dxvk_nvapi).
    DisableNvapi,
    /// Create a file at a relative path with given content.
    CreateFile { path: String, content: String },
    /// Rename a file within the bottle.
    RenameFile { from: String, to: String },
    /// Delete a file within the bottle.
    DeleteFile { path: String },
    /// Copy a file within the bottle.
    CopyFile { from: String, to: String },
    /// Set a registry key in the Wine prefix.
    SetRegistry {
        hive: String,
        key: String,
        name: String,
        reg_type: String,
        data: String,
    },
    /// An unrecognized line from the script, preserved as-is.
    Unknown(String),
}

/// A parsed game fix, representing the actions a protonfixes script performs.
#[derive(Debug, Clone)]
pub struct GameFix {
    /// The Steam app ID for this game.
    pub app_id: String,
    /// Human-readable game name, extracted from comments or filename.
    pub game_name: String,
    /// The list of actions this fix applies.
    pub actions: Vec<FixAction>,
    /// Path to the source Python script.
    pub source_file: PathBuf,
}

/// Errors that can occur during protonfixes parsing or application.
#[derive(Debug, thiserror::Error)]
pub enum ProtonFixError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to extract app_id from path: {0}")]
    NoAppId(String),
    #[error("no main() function found in script: {0}")]
    NoMainFunction(String),
}

type Result<T> = std::result::Result<T, ProtonFixError>;

/// Parse a single protonfixes Python script into a `GameFix`.
///
/// The app_id is extracted from the parent directory name (protonfixes organizes
/// scripts as `gamefixes-steam/<app_id>.py`) or from a header comment.
pub fn parse_fix_script(path: &Path) -> Result<GameFix> {
    tracing::debug!(path = %path.display(), "Parsing protonfixes script");
    let content = fs::read_to_string(path)?;

    let app_id = extract_app_id(path, &content)?;
    let game_name = extract_game_name(&content, &app_id);
    let actions = parse_main_body(&content, path)?;

    Ok(GameFix {
        app_id,
        game_name,
        actions,
        source_file: path.to_path_buf(),
    })
}

/// Scan a directory of protonfixes scripts and parse each one.
///
/// Expects `.py` files in the directory (typically named `<app_id>.py`).
/// Files that fail to parse are skipped with a tracing warning.
pub fn scan_fixes_directory(dir: &Path) -> Result<Vec<GameFix>> {
    tracing::info!(dir = %dir.display(), "Scanning protonfixes directory");
    let mut fixes = Vec::new();

    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("py") {
            match parse_fix_script(&path) {
                Ok(fix) => fixes.push(fix),
                Err(e) => {
                    tracing::warn!("skipping {}: {}", path.display(), e);
                }
            }
        }
    }

    fixes.sort_by(|a, b| a.app_id.cmp(&b.app_id));
    tracing::info!(fixes_found = fixes.len(), "Protonfixes scan complete");
    Ok(fixes)
}

/// Apply a parsed game fix at launch time.
///
/// Actions that can be applied immediately (environment variables, DLL overrides,
/// argument modifications) are handled directly. Actions like `InstallVerb` that
/// require runtime support are logged for manual handling.
///
/// Returns a list of additional command-line arguments to append to the launch command.
pub fn apply_fix_to_bottle(
    fix: &GameFix,
    bottle_path: &Path,
    env: &mut HashMap<String, String>,
) -> Result<Vec<String>> {
    let mut extra_args = Vec::new();

    // Helper: validate a path is within the bottle to prevent traversal attacks
    let validate_path = |path: &Path, bottle: &Path| -> Result<PathBuf> {
        // Reject paths with .. components
        for component in path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(ProtonFixError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("path traversal rejected: {}", path.display()),
                )));
            }
        }
        // Reject absolute paths
        if path.is_absolute() {
            return Err(ProtonFixError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("absolute path rejected: {}", path.display()),
            )));
        }
        Ok(bottle.join(path))
    };

    for action in &fix.actions {
        match action {
            FixAction::SetEnvVar { key, value } => {
                tracing::info!("setting env {}={}", key, value);
                env.insert(key.clone(), value.clone());
            }
            FixAction::DllOverride { dll, mode } => {
                tracing::info!("DLL override: {}={}", dll, mode);
                // Append to WINEDLLOVERRIDES env var (Wine convention)
                let overrides = env
                    .entry("WINEDLLOVERRIDES".to_string())
                    .or_default();
                if !overrides.is_empty() {
                    overrides.push(';');
                }
                overrides.push_str(&format!("{}={}", dll, mode));

                // Also attempt to write into the bottle's user.reg
                let user_reg = bottle_path.join("user.reg");
                if user_reg.exists() {
                    if let Err(e) = append_dll_override_to_registry(&user_reg, dll, mode) {
                        tracing::warn!("could not update user.reg for DLL override: {}", e);
                    }
                }
            }
            FixAction::AppendArgument(arg) => {
                tracing::info!("appending launch argument: {}", arg);
                extra_args.push(arg.clone());
            }
            FixAction::DisableNvapi => {
                tracing::info!("disabling NVAPI");
                env.insert("DXVK_ENABLE_NVAPI".to_string(), "0".to_string());
                env.insert("PROTON_HIDE_NVIDIA_GPU".to_string(), "1".to_string());
                // Also set as DLL override
                let overrides = env
                    .entry("WINEDLLOVERRIDES".to_string())
                    .or_default();
                if !overrides.is_empty() {
                    overrides.push(';');
                }
                overrides.push_str("nvapi,nvapi64=d");
            }
            FixAction::ReplaceCommand { from, to } => {
                tracing::info!("replace command: {} -> {}", from, to);
                // Store as environment hint for the launcher to pick up
                env.insert("CAULDRON_REPLACE_EXE_FROM".to_string(), from.clone());
                env.insert("CAULDRON_REPLACE_EXE_TO".to_string(), to.clone());
            }
            FixAction::InstallVerb(verb) => {
                tracing::info!(
                    "fix requires verb '{}' — manual installation needed",
                    verb
                );
            }
            FixAction::CreateFile { path, content } => {
                let full_path = validate_path(Path::new(path), bottle_path)?;
                tracing::info!("creating file: {}", full_path.display());
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&full_path, content)?;
            }
            FixAction::RenameFile { from, to } => {
                let src = validate_path(Path::new(from), bottle_path)?;
                let dst = validate_path(Path::new(to), bottle_path)?;
                tracing::info!("renaming {} -> {}", src.display(), dst.display());
                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(&src, &dst)?;
                } else {
                    tracing::warn!("rename source not found: {}", src.display());
                }
            }
            FixAction::DeleteFile { path } => {
                let target = bottle_path.join(path);
                tracing::info!("deleting {}", target.display());
                if target.exists() {
                    fs::remove_file(&target)?;
                } else {
                    tracing::debug!("delete target not found: {}", target.display());
                }
            }
            FixAction::CopyFile { from, to } => {
                let src = bottle_path.join(from);
                let dst = bottle_path.join(to);
                tracing::info!("copying {} -> {}", src.display(), dst.display());
                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    fs::copy(&src, &dst)?;
                } else {
                    tracing::warn!("copy source not found: {}", src.display());
                }
            }
            FixAction::SetRegistry { hive, key, name, reg_type, data } => {
                tracing::info!("setting registry: {}\\{}\\{} = {} ({})", hive, key, name, data, reg_type);
                // Write a .reg file and import it
                let reg_file = bottle_path.join(".cauldron_temp.reg");
                let hive_prefix = match hive.as_str() {
                    "HKCU" | "HKEY_CURRENT_USER" => "HKEY_CURRENT_USER",
                    "HKLM" | "HKEY_LOCAL_MACHINE" => "HKEY_LOCAL_MACHINE",
                    other => other,
                };
                let reg_type_str = match reg_type.as_str() {
                    "REG_SZ" | "sz" => "\"",
                    "REG_DWORD" | "dword" => "dword:",
                    other => {
                        tracing::warn!("unsupported registry type: {}", other);
                        "\""
                    }
                };
                let value_str = if reg_type_str == "dword:" {
                    format!("\"{}\"={}{}", name, reg_type_str, data)
                } else {
                    format!("\"{}\"=\"{}\"", name, data)
                };
                let reg_content = format!(
                    "Windows Registry Editor Version 5.00\n\n[{}\\{}]\n{}\n",
                    hive_prefix, key, value_str
                );
                fs::write(&reg_file, &reg_content)?;
                // The actual regedit import would happen at Wine launch time;
                // store the path as a hint for the launcher.
                env.insert(
                    "CAULDRON_REG_IMPORT".to_string(),
                    reg_file.to_string_lossy().to_string(),
                );
            }
            FixAction::Unknown(line) => {
                tracing::debug!("unhandled fix action: {}", line);
            }
        }
    }

    Ok(extra_args)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract the app ID from the file path or script content.
///
/// Protonfixes scripts are typically named `<app_id>.py`, so we try the
/// file stem first. Falls back to a header comment pattern like
/// `# Game ID: 12345`.
fn extract_app_id(path: &Path, content: &str) -> Result<String> {
    // Try file stem (e.g., "489830.py" -> "489830")
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if stem.chars().all(|c| c.is_ascii_digit()) && !stem.is_empty() {
            return Ok(stem.to_string());
        }
    }

    // Try parent directory name
    if let Some(parent_name) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str())
    {
        if parent_name.chars().all(|c| c.is_ascii_digit()) && !parent_name.is_empty() {
            return Ok(parent_name.to_string());
        }
    }

    // Try a header comment like `# Game ID: 12345` or `"""...(12345)..."""`
    let id_re = Regex::new(r"(?i)(?:game\s*id|app\s*id)[:\s]+(\d+)").unwrap();
    if let Some(caps) = id_re.captures(content) {
        return Ok(caps[1].to_string());
    }

    Err(ProtonFixError::NoAppId(path.display().to_string()))
}

/// Extract a human-readable game name from comments or docstrings.
fn extract_game_name(content: &str, fallback: &str) -> String {
    // Look for patterns like `""" Game Name """` or `# Game: Name`
    let docstring_re = Regex::new(r#"^"""(.*?)""""#).unwrap();
    if let Some(caps) = docstring_re.captures(content) {
        let name = caps[1].trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }

    // Single-line docstring on second/third line
    let line_re = Regex::new(r#"(?m)^"""\s*(.+?)\s*""""#).unwrap();
    if let Some(caps) = line_re.captures(content) {
        let name = caps[1].trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }

    // Comment like `# Game Fix: Skyrim Special Edition`
    let comment_re = Regex::new(r"(?mi)^#\s*(?:game\s*(?:fix)?|fix\s*for)[:\s]+(.+)$").unwrap();
    if let Some(caps) = comment_re.captures(content) {
        return caps[1].trim().to_string();
    }

    fallback.to_string()
}

/// Parse the body of the `main()` function and extract fix actions.
fn parse_main_body(content: &str, path: &Path) -> Result<Vec<FixAction>> {
    // Find the main() function body
    let main_body = extract_main_body(content).ok_or_else(|| {
        ProtonFixError::NoMainFunction(path.display().to_string())
    })?;

    let mut actions = Vec::new();

    // Compile patterns
    let protontricks_re =
        Regex::new(r#"protontricks\(\s*['"](.*?)['"]\s*\)"#).unwrap();
    let append_arg_re =
        Regex::new(r#"append_argument\(\s*['"](.*?)['"]\s*\)"#).unwrap();
    let replace_cmd_re =
        Regex::new(r#"replace_command\(\s*['"](.*?)['"]\s*,\s*['"](.*?)['"]\s*\)"#).unwrap();
    let env_bracket_re =
        Regex::new(r#"os\.environ\[\s*['"](.*?)['"]\s*\]\s*=\s*['"](.*?)['"]"#).unwrap();
    let env_setdefault_re =
        Regex::new(r#"os\.environ\.setdefault\(\s*['"](.*?)['"]\s*,\s*['"](.*?)['"]\s*\)"#)
            .unwrap();
    let disable_nvapi_re = Regex::new(r"disable_nvapi\(\)").unwrap();
    let dll_override_re =
        Regex::new(r#"(?:set_dlloverride|winedll_override)\(\s*['"](.*?)['"]\s*,\s*['"](.*?)['"]\s*\)"#)
            .unwrap();
    let create_file_re =
        Regex::new(r#"create_dosbox_conf|write_file|open\(\s*['"](.*?)['"]\s*,\s*['"]w['"]\s*\)"#)
            .unwrap();

    for line in main_body.lines() {
        let trimmed = line.trim();

        // Skip blank lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(caps) = protontricks_re.captures(trimmed) {
            actions.push(FixAction::InstallVerb(caps[1].to_string()));
        } else if let Some(caps) = replace_cmd_re.captures(trimmed) {
            actions.push(FixAction::ReplaceCommand {
                from: caps[1].to_string(),
                to: caps[2].to_string(),
            });
        } else if let Some(caps) = append_arg_re.captures(trimmed) {
            actions.push(FixAction::AppendArgument(caps[1].to_string()));
        } else if let Some(caps) = env_bracket_re.captures(trimmed) {
            actions.push(FixAction::SetEnvVar {
                key: caps[1].to_string(),
                value: caps[2].to_string(),
            });
        } else if let Some(caps) = env_setdefault_re.captures(trimmed) {
            actions.push(FixAction::SetEnvVar {
                key: caps[1].to_string(),
                value: caps[2].to_string(),
            });
        } else if disable_nvapi_re.is_match(trimmed) {
            actions.push(FixAction::DisableNvapi);
        } else if let Some(caps) = dll_override_re.captures(trimmed) {
            actions.push(FixAction::DllOverride {
                dll: caps[1].to_string(),
                mode: caps[2].to_string(),
            });
        } else if create_file_re.is_match(trimmed) {
            // Basic detection; content extraction would require multi-line parsing
            if let Some(caps) =
                Regex::new(r#"open\(\s*['"](.*?)['"]\s*,\s*['"]w['"]\s*\)"#)
                    .unwrap()
                    .captures(trimmed)
            {
                actions.push(FixAction::CreateFile {
                    path: caps[1].to_string(),
                    content: String::new(),
                });
            } else {
                actions.push(FixAction::Unknown(trimmed.to_string()));
            }
        } else {
            actions.push(FixAction::Unknown(trimmed.to_string()));
        }
    }

    Ok(actions)
}

/// Extract the indented body of `def main():` from a Python script.
fn extract_main_body(content: &str) -> Option<String> {
    let mut lines = content.lines().peekable();
    let mut found_main = false;
    let mut body_lines = Vec::new();
    let mut body_indent: Option<usize> = None;

    while let Some(line) = lines.next() {
        if !found_main {
            let trimmed = line.trim();
            if trimmed.starts_with("def main(") && trimmed.contains(':') {
                found_main = true;
            }
            continue;
        }

        // We are inside main()
        let raw = line;

        // Blank lines are ok inside the body
        if raw.trim().is_empty() {
            body_lines.push(String::new());
            continue;
        }

        let indent = raw.len() - raw.trim_start().len();

        if let Some(expected) = body_indent {
            if indent < expected {
                // Dedented past the body — we've left main()
                break;
            }
        } else {
            // First non-blank line sets the body indent
            if indent == 0 {
                // Not indented — malformed, but bail
                break;
            }
            body_indent = Some(indent);
        }

        body_lines.push(raw.to_string());
    }

    if body_lines.is_empty() {
        return None;
    }

    Some(body_lines.join("\n"))
}

/// Append a DLL override entry into a Wine `user.reg` file.
fn append_dll_override_to_registry(
    user_reg: &Path,
    dll: &str,
    mode: &str,
) -> std::result::Result<(), std::io::Error> {
    let content = fs::read_to_string(user_reg)?;

    let section_header = "[Software\\\\Wine\\\\DllOverrides]";

    // Map mode shorthand to registry value
    let reg_value = match mode {
        "n" | "native" => "native",
        "b" | "builtin" => "builtin",
        "n,b" | "native,builtin" => "native,builtin",
        "b,n" | "builtin,native" => "builtin,native",
        "d" | "disabled" | "" => "",
        other => other,
    };

    let entry = format!("\"*{}\"=\"{}\"", dll, reg_value);

    if content.contains(&entry) {
        return Ok(());
    }

    let new_content = if content.contains(section_header) {
        // Insert the entry after the section header
        content.replacen(section_header, &format!("{}\n{}", section_header, entry), 1)
    } else {
        // Append the section
        format!("{}\n\n{}\n{}\n", content.trim_end(), section_header, entry)
    };

    fs::write(user_reg, new_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_script(name: &str, content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join(name);
        let mut f = fs::File::create(&file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        dir
    }

    #[test]
    fn test_parse_protontricks() {
        let script = r#"
"""Skyrim Special Edition"""
def main():
    protontricks('vcrun2019')
    protontricks("dotnet48")
"#;
        let dir = write_temp_script("489830.py", script);
        let fix = parse_fix_script(&dir.path().join("489830.py")).unwrap();

        assert_eq!(fix.app_id, "489830");
        assert_eq!(fix.game_name, "Skyrim Special Edition");
        assert_eq!(fix.actions.len(), 2);
        assert_eq!(fix.actions[0], FixAction::InstallVerb("vcrun2019".to_string()));
        assert_eq!(fix.actions[1], FixAction::InstallVerb("dotnet48".to_string()));
    }

    #[test]
    fn test_parse_replace_command() {
        let script = r#"
def main():
    replace_command('SkyrimSELauncher.exe', 'skse64_loader.exe')
"#;
        let dir = write_temp_script("489830.py", script);
        let fix = parse_fix_script(&dir.path().join("489830.py")).unwrap();

        assert_eq!(
            fix.actions[0],
            FixAction::ReplaceCommand {
                from: "SkyrimSELauncher.exe".to_string(),
                to: "skse64_loader.exe".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_env_var() {
        let script = r#"
def main():
    os.environ['WINEDLLOVERRIDES'] = 'xaudio2_7=n,b'
    os.environ["DXVK_HUD"] = "fps"
"#;
        let dir = write_temp_script("12345.py", script);
        let fix = parse_fix_script(&dir.path().join("12345.py")).unwrap();

        assert_eq!(fix.actions.len(), 2);
        assert_eq!(
            fix.actions[0],
            FixAction::SetEnvVar {
                key: "WINEDLLOVERRIDES".to_string(),
                value: "xaudio2_7=n,b".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_disable_nvapi() {
        let script = r#"
def main():
    disable_nvapi()
"#;
        let dir = write_temp_script("99999.py", script);
        let fix = parse_fix_script(&dir.path().join("99999.py")).unwrap();

        assert_eq!(fix.actions, vec![FixAction::DisableNvapi]);
    }

    #[test]
    fn test_parse_append_argument() {
        let script = r#"
def main():
    append_argument('-fullscreen')
    append_argument("-windowed")
"#;
        let dir = write_temp_script("11111.py", script);
        let fix = parse_fix_script(&dir.path().join("11111.py")).unwrap();

        assert_eq!(fix.actions.len(), 2);
        assert_eq!(
            fix.actions[0],
            FixAction::AppendArgument("-fullscreen".to_string())
        );
        assert_eq!(
            fix.actions[1],
            FixAction::AppendArgument("-windowed".to_string())
        );
    }

    #[test]
    fn test_parse_unknown_lines() {
        let script = r#"
def main():
    some_custom_function()
"#;
        let dir = write_temp_script("22222.py", script);
        let fix = parse_fix_script(&dir.path().join("22222.py")).unwrap();

        assert_eq!(
            fix.actions,
            vec![FixAction::Unknown("some_custom_function()".to_string())]
        );
    }

    #[test]
    fn test_apply_fix_env_vars() {
        let fix = GameFix {
            app_id: "1".to_string(),
            game_name: "Test".to_string(),
            actions: vec![
                FixAction::SetEnvVar {
                    key: "FOO".to_string(),
                    value: "bar".to_string(),
                },
                FixAction::AppendArgument("-test".to_string()),
                FixAction::DisableNvapi,
            ],
            source_file: PathBuf::from("test.py"),
        };

        let tmp = tempfile::tempdir().unwrap();
        let mut env = HashMap::new();
        let args = apply_fix_to_bottle(&fix, tmp.path(), &mut env).unwrap();

        assert_eq!(env.get("FOO").unwrap(), "bar");
        assert_eq!(env.get("DXVK_ENABLE_NVAPI").unwrap(), "0");
        assert_eq!(env.get("PROTON_HIDE_NVIDIA_GPU").unwrap(), "1");
        assert!(env.get("WINEDLLOVERRIDES").unwrap().contains("nvapi,nvapi64=d"));
        assert_eq!(args, vec!["-test".to_string()]);
    }

    #[test]
    fn test_scan_directory() {
        let dir = tempfile::tempdir().unwrap();
        let script1 = "def main():\n    protontricks('vcrun2019')\n";
        let script2 = "def main():\n    disable_nvapi()\n";

        fs::write(dir.path().join("100.py"), script1).unwrap();
        fs::write(dir.path().join("200.py"), script2).unwrap();
        fs::write(dir.path().join("readme.txt"), "not a script").unwrap();

        let fixes = scan_fixes_directory(dir.path()).unwrap();
        assert_eq!(fixes.len(), 2);
        assert_eq!(fixes[0].app_id, "100");
        assert_eq!(fixes[1].app_id, "200");
    }

    #[test]
    fn test_complex_script() {
        let script = r#"
"""The Elder Scrolls V: Skyrim Special Edition"""
# Game Fix: Skyrim Special Edition
def main():
    protontricks('vcrun2019')
    protontricks('dotnet48')
    append_argument('-fullscreen')
    replace_command('SkyrimSELauncher.exe', 'skse64_loader.exe')
    disable_nvapi()
    os.environ['DXVK_HUD'] = 'fps'
"#;
        let dir = write_temp_script("489830.py", script);
        let fix = parse_fix_script(&dir.path().join("489830.py")).unwrap();

        assert_eq!(fix.app_id, "489830");
        assert_eq!(fix.game_name, "The Elder Scrolls V: Skyrim Special Edition");
        assert_eq!(fix.actions.len(), 6);
    }
}
