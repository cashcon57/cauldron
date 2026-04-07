use crate::monitor::RawCommit;
use regex::Regex;
use std::fmt;
use std::sync::LazyLock;

static WINE_API_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(dlls|server|loader)/").unwrap());
static DXVK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)dxvk/").unwrap());
static VKD3D_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)vkd3d-proton/").unwrap());
static GAME_CONFIG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(proton$|app_id|compatibilitytools)").unwrap());
static KERNEL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(futex|fsync|eventfd|/proc/|epoll)").unwrap());
static STEAM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(lsteamclient|vrclient|wineopenxr)/").unwrap());
static BUILD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(Makefile|configure|\.mk$|CMakeLists)").unwrap());

/// How transferable a Proton commit is to macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transferability {
    High,
    Medium,
    Low,
    None,
}

impl Transferability {
    pub fn from_str(s: &str) -> Self {
        match s {
            "High" => Self::High,
            "Medium" => Self::Medium,
            "Low" => Self::Low,
            "None" => Self::None,
            _ => Self::Medium,
        }
    }
}

impl fmt::Display for Transferability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "High"),
            Self::Medium => write!(f, "Medium"),
            Self::Low => write!(f, "Low"),
            Self::None => write!(f, "None"),
        }
    }
}

/// The category of change a Proton commit represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    WineApiFix,
    DxvkFix,
    Vkd3dFix,
    GameConfig,
    KernelWorkaround,
    SteamIntegration,
    BuildSystem,
    Unknown,
}

impl Classification {
    pub fn from_str(s: &str) -> Self {
        match s {
            "WineApiFix" => Self::WineApiFix,
            "DxvkFix" => Self::DxvkFix,
            "Vkd3dFix" => Self::Vkd3dFix,
            "GameConfig" => Self::GameConfig,
            "KernelWorkaround" => Self::KernelWorkaround,
            "SteamIntegration" => Self::SteamIntegration,
            "BuildSystem" => Self::BuildSystem,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WineApiFix => write!(f, "WineApiFix"),
            Self::DxvkFix => write!(f, "DxvkFix"),
            Self::Vkd3dFix => write!(f, "Vkd3dFix"),
            Self::GameConfig => write!(f, "GameConfig"),
            Self::KernelWorkaround => write!(f, "KernelWorkaround"),
            Self::SteamIntegration => write!(f, "SteamIntegration"),
            Self::BuildSystem => write!(f, "BuildSystem"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A commit that has been classified with transferability information.
#[derive(Debug, Clone)]
pub struct ClassifiedCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: String,
    pub diff: String,
    pub affected_files: Vec<String>,
    pub classification: Classification,
    pub transferability: Transferability,
    pub suggested_action: String,
}

/// Classify a raw commit based on which files it touches and its content.
pub fn classify(commit: &RawCommit) -> ClassifiedCommit {
    tracing::debug!(hash = %commit.hash, files_count = commit.affected_files.len(), "Classifying commit");
    let (classification, transferability) = determine_classification(&commit.affected_files, &commit.message, &commit.diff);

    let suggested_action = match (&classification, &transferability) {
        (Classification::WineApiFix, Transferability::High) => {
            "Safe to apply: Wine API fix is platform-independent".to_string()
        }
        (Classification::DxvkFix, _) => {
            "Submodule update: DXVK must be updated separately, not via patch".to_string()
        }
        (Classification::Vkd3dFix, _) => {
            "Submodule update: VKD3D-Proton must be updated separately, not via patch".to_string()
        }
        (Classification::GameConfig, _) => {
            "Safe to apply: game-specific config tweak, worst case is a no-op on macOS".to_string()
        }
        (Classification::KernelWorkaround, _) => {
            "Review: uses Linux-specific paths (/proc/). On macOS these checks gracefully fail, which is usually the correct behavior".to_string()
        }
        (Classification::SteamIntegration, _) => {
            "Skip: Steam/VR client integration. Not applicable on macOS — CrossOver/Cauldron uses its own Steam bridge".to_string()
        }
        (Classification::BuildSystem, _) => {
            "Skip: build system or infrastructure change, no runtime impact".to_string()
        }
        _ => "Manual review required".to_string(),
    };

    tracing::info!(
        "Classified commit {} as {:?}/{:?}",
        commit.hash,
        classification,
        transferability
    );

    ClassifiedCommit {
        hash: commit.hash.clone(),
        message: commit.message.clone(),
        author: commit.author.clone(),
        timestamp: commit.timestamp.clone(),
        diff: commit.diff.clone(),
        affected_files: commit.affected_files.clone(),
        classification,
        transferability,
        suggested_action,
    }
}

fn determine_classification(
    files: &[String],
    message: &str,
    diff: &str,
) -> (Classification, Transferability) {
    // Check files for classification patterns (regexes compiled once via LazyLock)
    for file in files {
        if WINE_API_RE.is_match(file) {
            return (Classification::WineApiFix, Transferability::High);
        }
        if DXVK_RE.is_match(file) {
            return (Classification::DxvkFix, Transferability::High);
        }
        if VKD3D_RE.is_match(file) {
            return (Classification::Vkd3dFix, Transferability::Medium);
        }
        if STEAM_RE.is_match(file) {
            return (Classification::SteamIntegration, Transferability::Low);
        }
        if BUILD_RE.is_match(file) {
            return (Classification::BuildSystem, Transferability::None);
        }
    }

    // Check for submodule/dependency updates by message patterns
    let msg_lower = message.to_ascii_lowercase();
    if msg_lower.starts_with("update wine")
        || msg_lower.starts_with("update mono")
        || msg_lower.starts_with("update wine mono")
        || msg_lower.starts_with("update gecko")
        || msg_lower.contains("update submodule")
    {
        return (Classification::BuildSystem, Transferability::None);
    }
    if msg_lower.starts_with("update vkd3d") {
        return (Classification::Vkd3dFix, Transferability::Medium);
    }
    if msg_lower.starts_with("update dxvk") {
        return (Classification::DxvkFix, Transferability::High);
    }
    if msg_lower.starts_with("docker:") || msg_lower.starts_with("ci:") || msg_lower.starts_with("ci/") {
        return (Classification::BuildSystem, Transferability::None);
    }

    // Check for README/docs updates
    for file in files {
        let f_lower = file.to_ascii_lowercase();
        if f_lower == "readme.md" || f_lower.starts_with("docker") || f_lower.starts_with(".github")
            || f_lower.ends_with(".yml") || f_lower.ends_with(".yaml")
        {
            return (Classification::BuildSystem, Transferability::None);
        }
    }

    // Check message and diff for patterns not caught by file paths
    let combined = format!("{message}\n{diff}");

    if GAME_CONFIG_RE.is_match(&combined) {
        return (Classification::GameConfig, Transferability::High);
    }

    if KERNEL_RE.is_match(&combined) {
        return (Classification::KernelWorkaround, Transferability::Low);
    }

    // VR/Steam client integration by message prefix
    if msg_lower.starts_with("vrclient:")
        || msg_lower.starts_with("wineopenxr:")
        || msg_lower.starts_with("lsteamclient:")
    {
        return (Classification::SteamIntegration, Transferability::Low);
    }

    // Proton-specific config/scripts
    if msg_lower.starts_with("proton:") || msg_lower.starts_with("proton ") {
        return (Classification::GameConfig, Transferability::Medium);
    }

    (Classification::Unknown, Transferability::Medium)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::RawCommit;

    fn make_commit(files: &[&str], message: &str, diff: &str) -> RawCommit {
        RawCommit {
            hash: "abc123".to_string(),
            message: message.to_string(),
            author: "test".to_string(),
            timestamp: "2024-01-01".to_string(),
            diff: diff.to_string(),
            affected_files: files.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_classify_wine_api_fix() {
        let commit = make_commit(&["dlls/ntdll/sync.c"], "Fix Wine sync issue", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::WineApiFix);
        assert_eq!(result.transferability, Transferability::High);
    }

    #[test]
    fn test_classify_dxvk_fix() {
        let commit = make_commit(&["dxvk/src/dxvk_device.cpp"], "Fix DXVK device creation", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::DxvkFix);
        assert_eq!(result.transferability, Transferability::High);
    }

    #[test]
    fn test_classify_vkd3d_fix() {
        let commit = make_commit(&["vkd3d-proton/libs/vkd3d/device.c"], "Fix vkd3d", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::Vkd3dFix);
        assert_eq!(result.transferability, Transferability::Medium);
    }

    #[test]
    fn test_classify_game_config() {
        let commit = make_commit(&["config.txt"], "Add app_id config", "app_id = 12345");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::GameConfig);
        assert_eq!(result.transferability, Transferability::High);
    }

    #[test]
    fn test_classify_kernel_workaround() {
        let commit = make_commit(&["src/misc.c"], "Add futex support", "futex_wait()");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::KernelWorkaround);
        assert_eq!(result.transferability, Transferability::Low);
    }

    #[test]
    fn test_classify_steam_integration() {
        let commit = make_commit(&["lsteamclient/client.c"], "Update Steam client", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::SteamIntegration);
        assert_eq!(result.transferability, Transferability::Low);
    }

    #[test]
    fn test_classify_build_system() {
        let commit = make_commit(&["Makefile"], "Update build", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::BuildSystem);
        assert_eq!(result.transferability, Transferability::None);
    }

    #[test]
    fn test_classify_unknown() {
        let commit = make_commit(&["random/file.txt"], "Some change", "nothing special");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::Unknown);
        assert_eq!(result.transferability, Transferability::Medium);
    }

    #[test]
    fn test_classify_server_file() {
        let commit = make_commit(&["server/thread.c"], "Fix thread handling", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::WineApiFix);
    }

    #[test]
    fn test_classify_loader_file() {
        let commit = make_commit(&["loader/main.c"], "Fix loader", "");
        let result = classify(&commit);
        assert_eq!(result.classification, Classification::WineApiFix);
    }
}
