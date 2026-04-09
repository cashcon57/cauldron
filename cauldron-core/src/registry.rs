// Canonical Wine registry writer — all user.reg modifications should go through
// this module to avoid conflicting with other registry-writing code paths.

use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Registry parse error: {0}")]
    Parse(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Invalid DLL override mode: {0}")]
    InvalidOverrideMode(String),
}

/// Which registry hive to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryHive {
    /// `user.reg` — HKEY_CURRENT_USER
    User,
    /// `system.reg` — HKEY_LOCAL_MACHINE
    System,
}

impl RegistryHive {
    /// Returns the filename Wine uses for this hive.
    pub fn filename(&self) -> &'static str {
        match self {
            RegistryHive::User => "user.reg",
            RegistryHive::System => "system.reg",
        }
    }

    fn registry_path(&self, bottle_path: &Path) -> PathBuf {
        bottle_path.join(self.filename())
    }
}

/// The type of a registry value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegValueType {
    /// REG_SZ — a string value.
    String,
    /// REG_DWORD — a 32-bit integer.
    Dword,
    /// REG_BINARY — binary data (hex-encoded).
    Binary,
    /// REG_MULTI_SZ — multiple strings.
    Multi,
    /// REG_EXPAND_SZ — a string with environment variable expansion.
    Expand,
}

impl fmt::Display for RegValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegValueType::String => write!(f, "str"),
            RegValueType::Dword => write!(f, "dword"),
            RegValueType::Binary => write!(f, "hex"),
            RegValueType::Multi => write!(f, "str(7)"),
            RegValueType::Expand => write!(f, "str(2)"),
        }
    }
}

/// A single registry value entry.
#[derive(Debug, Clone)]
pub struct RegistryValue {
    pub name: std::string::String,
    pub value_type: RegValueType,
    pub data: std::string::String,
}

/// A registry key containing zero or more values.
#[derive(Debug, Clone)]
pub struct RegistryKey {
    pub path: std::string::String,
    pub values: Vec<RegistryValue>,
}

/// Read and parse all keys from a registry hive file.
pub fn read_registry(
    bottle_path: &Path,
    hive: RegistryHive,
) -> Result<Vec<RegistryKey>, RegistryError> {
    let reg_path = hive.registry_path(bottle_path);
    tracing::debug!(hive = %hive.filename(), path = %reg_path.display(), "Reading registry hive");
    if !reg_path.exists() {
        tracing::debug!(hive = %hive.filename(), "Registry file does not exist, returning empty");
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&reg_path)?;
    let keys = parse_registry(&contents)?;
    tracing::debug!(hive = %hive.filename(), keys_found = keys.len(), "Registry parsed successfully");
    Ok(keys)
}

/// Parse Wine registry text into structured keys and values.
fn parse_registry(contents: &str) -> Result<Vec<RegistryKey>, RegistryError> {
    let mut keys = Vec::new();
    let mut current_key: Option<RegistryKey> = None;

    for line in contents.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(";;") {
            continue;
        }

        // Skip Wine registry preamble lines
        if trimmed.starts_with("WINE REGISTRY") || trimmed.starts_with("\\\\") {
            continue;
        }

        // Key header: [path] optional_timestamp
        if trimmed.starts_with('[') {
            // Save previous key
            if let Some(key) = current_key.take() {
                keys.push(key);
            }

            let key_path = extract_key_path(trimmed);
            current_key = Some(RegistryKey {
                path: key_path,
                values: Vec::new(),
            });
            continue;
        }

        // Value line — only parse if we're inside a key
        if let Some(ref mut key) = current_key {
            if let Some(value) = parse_value_line(trimmed) {
                key.values.push(value);
            }
        }
    }

    // Don't forget the last key
    if let Some(key) = current_key {
        keys.push(key);
    }

    Ok(keys)
}

/// Extract the key path from a header line like `[Software\\Wine\\DllOverrides] 1234567890`
fn extract_key_path(line: &str) -> std::string::String {
    // Find the closing bracket
    if let Some(end) = line.find(']') {
        // Skip the opening bracket
        line[1..end].to_string()
    } else {
        line[1..].to_string()
    }
}

/// Parse a single value line into a RegistryValue.
///
/// Wine registry value formats:
/// - `"name"="string value"` — REG_SZ
/// - `"name"=str(2):"expand string"` — REG_EXPAND_SZ
/// - `"name"=str(7):"multi string"` — REG_MULTI_SZ
/// - `"name"=dword:00000001` — REG_DWORD
/// - `"name"=hex:01,02,03` — REG_BINARY
/// - `@="default value"` — default value (name is `@`)
fn parse_value_line(line: &str) -> Option<RegistryValue> {
    // Handle default value
    let (name, rest) = if line.starts_with('@') {
        if !line.starts_with("@=") {
            return None;
        }
        ("@".to_string(), &line[2..])
    } else if line.starts_with('"') {
        // Find the closing quote for the name
        let name_end = find_closing_quote(line, 1)?;
        let name = &line[1..name_end];
        // Expect `=` after the closing quote
        let after = &line[name_end + 1..];
        if !after.starts_with('=') {
            return None;
        }
        (unescape_registry_string(name), &after[1..])
    } else {
        return None;
    };

    // Parse the value data
    if rest.starts_with('"') {
        // REG_SZ: "value"
        let val_end = find_closing_quote(rest, 1)?;
        let data = unescape_registry_string(&rest[1..val_end]);
        Some(RegistryValue {
            name,
            value_type: RegValueType::String,
            data,
        })
    } else if let Some(hex_data) = rest.strip_prefix("dword:") {
        Some(RegistryValue {
            name,
            value_type: RegValueType::Dword,
            data: hex_data.to_string(),
        })
    } else if let Some(hex_data) = rest.strip_prefix("hex:") {
        Some(RegistryValue {
            name,
            value_type: RegValueType::Binary,
            data: hex_data.to_string(),
        })
    } else if let Some(rest2) = rest.strip_prefix("str(2):") {
        // REG_EXPAND_SZ
        if rest2.starts_with('"') {
            let val_end = find_closing_quote(rest2, 1)?;
            let data = unescape_registry_string(&rest2[1..val_end]);
            Some(RegistryValue {
                name,
                value_type: RegValueType::Expand,
                data,
            })
        } else {
            Some(RegistryValue {
                name,
                value_type: RegValueType::Expand,
                data: rest2.to_string(),
            })
        }
    } else if let Some(rest7) = rest.strip_prefix("str(7):") {
        // REG_MULTI_SZ
        if rest7.starts_with('"') {
            let val_end = find_closing_quote(rest7, 1)?;
            let data = unescape_registry_string(&rest7[1..val_end]);
            Some(RegistryValue {
                name,
                value_type: RegValueType::Multi,
                data,
            })
        } else {
            Some(RegistryValue {
                name,
                value_type: RegValueType::Multi,
                data: rest7.to_string(),
            })
        }
    } else {
        None
    }
}

/// Find the position of the closing (unescaped) quote starting search at `start`.
fn find_closing_quote(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2; // skip escaped character
            continue;
        }
        if bytes[i] == b'"' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Unescape Wine registry string escapes (`\\` -> `\`, `\"` -> `"`).
fn unescape_registry_string(s: &str) -> std::string::String {
    let mut result = std::string::String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('0') => {} // null terminator
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Escape a string for writing into a Wine registry file.
fn escape_registry_string(s: &str) -> std::string::String {
    let mut result = std::string::String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            _ => result.push(c),
        }
    }
    result
}

/// Get a specific value from the registry.
pub fn get_value(
    bottle_path: &Path,
    hive: RegistryHive,
    key_path: &str,
    value_name: &str,
) -> Result<Option<RegistryValue>, RegistryError> {
    tracing::debug!(hive = %hive.filename(), key = %key_path, value = %value_name, "Getting registry value");
    let keys = read_registry(bottle_path, hive)?;
    for key in &keys {
        if key.path == key_path {
            for value in &key.values {
                if value.name == value_name {
                    return Ok(Some(value.clone()));
                }
            }
            return Ok(None);
        }
    }
    Ok(None)
}

/// Set a value in the registry. Creates the key if it doesn't exist.
///
/// This reads the raw file, performs a targeted edit, and writes it back,
/// preserving existing formatting and comments.
pub fn set_value(
    bottle_path: &Path,
    hive: RegistryHive,
    key_path: &str,
    name: &str,
    value_type: RegValueType,
    data: &str,
) -> Result<(), RegistryError> {
    let reg_path = hive.registry_path(bottle_path);

    let contents = if reg_path.exists() {
        std::fs::read_to_string(&reg_path)?
    } else {
        default_registry_preamble(hive)
    };

    let formatted_value = format_value_line(name, &value_type, data);
    let new_contents = set_value_in_text(&contents, key_path, name, &formatted_value);
    std::fs::write(&reg_path, new_contents)?;

    tracing::debug!("Set registry value [{key_path}] \"{name}\" in {}", hive.filename());
    Ok(())
}

/// Delete a value from the registry.
pub fn delete_value(
    bottle_path: &Path,
    hive: RegistryHive,
    key_path: &str,
    name: &str,
) -> Result<(), RegistryError> {
    let reg_path = hive.registry_path(bottle_path);
    if !reg_path.exists() {
        return Err(RegistryError::KeyNotFound(key_path.to_string()));
    }

    let contents = std::fs::read_to_string(&reg_path)?;
    let new_contents = delete_value_in_text(&contents, key_path, name);
    std::fs::write(&reg_path, new_contents)?;

    tracing::debug!("Deleted registry value [{key_path}] \"{name}\" from {}", hive.filename());
    Ok(())
}

/// List all DLL overrides configured in the bottle.
///
/// Reads `[Software\\Wine\\DllOverrides]` from `user.reg` and returns
/// `(dll_name, mode)` pairs.
pub fn list_dll_overrides(
    bottle_path: &Path,
) -> Result<Vec<(std::string::String, std::string::String)>, RegistryError> {
    tracing::debug!(bottle = %bottle_path.display(), "Listing DLL overrides");
    let keys = read_registry(bottle_path, RegistryHive::User)?;
    let override_key = "Software\\\\Wine\\\\DllOverrides";

    for key in &keys {
        if key.path == override_key {
            let pairs = key
                .values
                .iter()
                .filter(|v| v.value_type == RegValueType::String)
                .map(|v| (v.name.clone(), v.data.clone()))
                .collect();
            return Ok(pairs);
        }
    }

    Ok(Vec::new())
}

/// Set a DLL override in the Wine registry.
///
/// Valid modes: `"native"`, `"builtin"`, `"native,builtin"`, `"builtin,native"`, `"disabled"`, `""`.
pub fn set_dll_override(
    bottle_path: &Path,
    dll_name: &str,
    mode: &str,
) -> Result<(), RegistryError> {
    tracing::debug!(dll = %dll_name, mode = %mode, "Setting DLL override");
    let valid_modes = ["native", "builtin", "native,builtin", "builtin,native", "disabled", ""];
    if !valid_modes.contains(&mode) {
        tracing::warn!(mode = %mode, "Invalid DLL override mode");
        return Err(RegistryError::InvalidOverrideMode(mode.to_string()));
    }

    set_value(
        bottle_path,
        RegistryHive::User,
        "Software\\\\Wine\\\\DllOverrides",
        dll_name,
        RegValueType::String,
        mode,
    )
}

/// Set a per-application DLL override in the Wine registry.
///
/// Writes to `[Software\\Wine\\AppDefaults\\<exe_name>\\DllOverrides]` in `user.reg`.
/// This scopes the override to only that executable, preventing conflicts with
/// other Wine processes (e.g. steamwebhelper.exe crashing when d3d11=native is global).
pub fn set_app_dll_override(
    bottle_path: &Path,
    exe_name: &str,
    dll_name: &str,
    mode: &str,
) -> Result<(), RegistryError> {
    tracing::debug!(exe = %exe_name, dll = %dll_name, mode = %mode, "Setting per-app DLL override");
    let valid_modes = ["native", "builtin", "native,builtin", "builtin,native", "disabled", ""];
    if !valid_modes.contains(&mode) {
        tracing::warn!(mode = %mode, "Invalid DLL override mode");
        return Err(RegistryError::InvalidOverrideMode(mode.to_string()));
    }

    let key_path = format!("Software\\\\Wine\\\\AppDefaults\\\\{}\\\\DllOverrides", exe_name);
    set_value(
        bottle_path,
        RegistryHive::User,
        &key_path,
        dll_name,
        RegValueType::String,
        mode,
    )
}

// --- Internal text manipulation helpers ---

/// Format a value line for writing into a registry file.
fn format_value_line(name: &str, value_type: &RegValueType, data: &str) -> std::string::String {
    let name_part = if name == "@" {
        "@".to_string()
    } else {
        format!("\"{}\"", escape_registry_string(name))
    };

    match value_type {
        RegValueType::String => format!("{}=\"{}\"", name_part, escape_registry_string(data)),
        RegValueType::Dword => format!("{}=dword:{}", name_part, data),
        RegValueType::Binary => format!("{}=hex:{}", name_part, data),
        RegValueType::Multi => format!("{}=str(7):\"{}\"", name_part, escape_registry_string(data)),
        RegValueType::Expand => {
            format!("{}=str(2):\"{}\"", name_part, escape_registry_string(data))
        }
    }
}

/// Insert or update a value in the raw registry text, preserving formatting.
fn set_value_in_text(
    contents: &str,
    key_path: &str,
    name: &str,
    formatted_value: &str,
) -> std::string::String {
    let lines: Vec<&str> = contents.lines().collect();
    let mut result = Vec::new();
    let mut found_key = false;
    let mut replaced_value = false;
    let mut in_target_key = false;

    let name_prefix = if name == "@" {
        "@=".to_string()
    } else {
        format!("\"{}\"=", escape_registry_string(name))
    };

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            // We're entering a new key section
            if in_target_key && !replaced_value {
                // We were in the target key but never found the value — append it
                result.push(formatted_value.to_string());
                replaced_value = true;
            }
            in_target_key = false;

            let this_key = extract_key_path(trimmed);
            if this_key == key_path {
                found_key = true;
                in_target_key = true;
            }

            result.push(line.to_string());
            continue;
        }

        if in_target_key && !replaced_value && trimmed.starts_with(&name_prefix) {
            // Replace existing value
            result.push(formatted_value.to_string());
            replaced_value = true;
            continue;
        }

        result.push(line.to_string());
    }

    // Handle end-of-file: if we were still in the target key and didn't replace
    if in_target_key && !replaced_value {
        result.push(formatted_value.to_string());
        replaced_value = true;
    }

    // Key didn't exist at all — append new key section
    if !found_key {
        if !result.is_empty() && !result.last().unwrap().is_empty() {
            result.push(String::new());
        }
        result.push(format!("[{}]", key_path));
        result.push(formatted_value.to_string());
        let _ = replaced_value; // suppress unused warning
    }

    let mut output = result.join("\n");
    // Preserve trailing newline if original had one
    if contents.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Remove a value from the raw registry text, preserving everything else.
fn delete_value_in_text(
    contents: &str,
    key_path: &str,
    name: &str,
) -> std::string::String {
    let lines: Vec<&str> = contents.lines().collect();
    let mut result = Vec::new();
    let mut in_target_key = false;

    let name_prefix = if name == "@" {
        "@=".to_string()
    } else {
        format!("\"{}\"=", escape_registry_string(name))
    };

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            let this_key = extract_key_path(trimmed);
            in_target_key = this_key == key_path;
            result.push(line.to_string());
            continue;
        }

        // Skip the target value line
        if in_target_key && trimmed.starts_with(&name_prefix) {
            continue;
        }

        result.push(line.to_string());
    }

    let mut output = result.join("\n");
    if contents.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Provide a sensible default preamble for a new registry file.
fn default_registry_preamble(hive: RegistryHive) -> std::string::String {
    match hive {
        RegistryHive::User => "WINE REGISTRY Version 2\n;; All keys relative to \\\\User\\\\S-1-5-21-0-0-0-1000\n\n".to_string(),
        RegistryHive::System => "WINE REGISTRY Version 2\n;; All keys relative to \\\\Machine\n\n".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_REG: &str = r#"WINE REGISTRY Version 2
;; All keys relative to \\User\\S-1-5-21-0-0-0-1000

[Software\\Wine\\DllOverrides] 1712345678
"d3d11"="native"
"dxgi"="native,builtin"

[Software\\Wine\\Direct3D] 1712345679
#This is a comment
"MaxVersionGL"=dword:00040006
@="default value"
"shader_backend"="glsl"
"#;

    #[test]
    fn test_parse_registry() {
        let keys = parse_registry(SAMPLE_REG).unwrap();
        assert_eq!(keys.len(), 2);

        // First key: DllOverrides
        assert_eq!(keys[0].path, "Software\\\\Wine\\\\DllOverrides");
        assert_eq!(keys[0].values.len(), 2);
        assert_eq!(keys[0].values[0].name, "d3d11");
        assert_eq!(keys[0].values[0].data, "native");
        assert_eq!(keys[0].values[0].value_type, RegValueType::String);
        assert_eq!(keys[0].values[1].name, "dxgi");
        assert_eq!(keys[0].values[1].data, "native,builtin");

        // Second key: Direct3D
        assert_eq!(keys[1].path, "Software\\\\Wine\\\\Direct3D");
        assert_eq!(keys[1].values.len(), 3);
        // DWORD value
        assert_eq!(keys[1].values[0].name, "MaxVersionGL");
        assert_eq!(keys[1].values[0].value_type, RegValueType::Dword);
        assert_eq!(keys[1].values[0].data, "00040006");
        // Default value
        assert_eq!(keys[1].values[1].name, "@");
        assert_eq!(keys[1].values[1].data, "default value");
        // String value
        assert_eq!(keys[1].values[2].name, "shader_backend");
        assert_eq!(keys[1].values[2].data, "glsl");
    }

    #[test]
    fn test_set_value_existing_key() {
        let result = set_value_in_text(
            SAMPLE_REG,
            "Software\\\\Wine\\\\DllOverrides",
            "d3d11",
            "\"d3d11\"=\"builtin\"",
        );
        assert!(result.contains("\"d3d11\"=\"builtin\""));
        assert!(!result.contains("\"d3d11\"=\"native\""));
        // Other values preserved
        assert!(result.contains("\"dxgi\"=\"native,builtin\""));
    }

    #[test]
    fn test_set_value_new_in_existing_key() {
        let result = set_value_in_text(
            SAMPLE_REG,
            "Software\\\\Wine\\\\DllOverrides",
            "xinput1_3",
            "\"xinput1_3\"=\"native\"",
        );
        assert!(result.contains("\"xinput1_3\"=\"native\""));
        // Existing values preserved
        assert!(result.contains("\"d3d11\"=\"native\""));
    }

    #[test]
    fn test_set_value_new_key() {
        let result = set_value_in_text(
            SAMPLE_REG,
            "Software\\\\Wine\\\\NewKey",
            "foo",
            "\"foo\"=\"bar\"",
        );
        assert!(result.contains("[Software\\\\Wine\\\\NewKey]"));
        assert!(result.contains("\"foo\"=\"bar\""));
    }

    #[test]
    fn test_delete_value() {
        let result = delete_value_in_text(
            SAMPLE_REG,
            "Software\\\\Wine\\\\DllOverrides",
            "d3d11",
        );
        assert!(!result.contains("\"d3d11\""));
        // Other value still present
        assert!(result.contains("\"dxgi\"=\"native,builtin\""));
    }

    #[test]
    fn test_format_value_line() {
        assert_eq!(
            format_value_line("test", &RegValueType::String, "hello"),
            "\"test\"=\"hello\""
        );
        assert_eq!(
            format_value_line("count", &RegValueType::Dword, "00000001"),
            "\"count\"=dword:00000001"
        );
        assert_eq!(
            format_value_line("@", &RegValueType::String, "default"),
            "@=\"default\""
        );
    }
}
