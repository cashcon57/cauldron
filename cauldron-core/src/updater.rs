use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Failed to check for updates: {0}")]
    CheckFailed(String),

    #[error("Failed to parse version string: {0}")]
    VersionParse(String),

    #[error("Network error: {0}")]
    Network(String),
}

pub type Result<T> = std::result::Result<T, UpdateError>;

/// Information about a release from GitHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag: String,
    pub download_url: String,
    pub release_notes: String,
    pub published_at: String,
    pub size_bytes: u64,
    pub is_prerelease: bool,
}

/// Status of an update check.
#[derive(Debug)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable(ReleaseInfo),
    CheckFailed(String),
}

/// Checks for new releases of Cauldron via the GitHub releases API.
pub struct UpdateChecker {
    pub current_version: String,
    pub github_repo: String,
}

impl UpdateChecker {
    /// Create a new UpdateChecker for the given current version.
    /// Uses the default GitHub repository "user/cauldron".
    pub fn new(current_version: &str) -> Self {
        Self {
            current_version: current_version.to_string(),
            github_repo: "user/cauldron".to_string(),
        }
    }

    /// Check whether a newer release is available on GitHub.
    ///
    /// Constructs the GitHub API URL and compares the latest release tag
    /// against the current version. Returns `Some(ReleaseInfo)` if a newer
    /// version is available, or `None` if already up to date.
    ///
    /// The actual HTTP call is currently stubbed out. When the network layer
    /// is wired up, this will query:
    ///   `https://api.github.com/repos/{repo}/releases/latest`
    pub fn check_for_update(&self) -> Result<Option<ReleaseInfo>> {
        tracing::info!(current_version = %self.current_version, repo = %self.github_repo, "Checking for updates");
        let _api_url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            self.github_repo
        );

        // Stub: In a real implementation this would use reqwest to fetch
        // the latest release JSON from the GitHub API. For now we simulate
        // a response indicating that the current version is the latest.
        let latest_tag = self.current_version.clone();

        tracing::debug!(current = %self.current_version, latest = %latest_tag, "Comparing versions");
        match compare_versions(&self.current_version, &latest_tag) {
            Ordering::Less => {
                tracing::info!(current = %self.current_version, latest = %latest_tag, "Update available");
                let release = ReleaseInfo {
                    version: latest_tag.clone(),
                    tag: format!("v{}", latest_tag),
                    download_url: format!(
                        "https://github.com/{}/releases/download/v{}/Cauldron-{}.dmg",
                        self.github_repo, latest_tag, latest_tag
                    ),
                    release_notes: String::new(),
                    published_at: String::new(),
                    size_bytes: 0,
                    is_prerelease: false,
                };
                Ok(Some(release))
            }
            _ => {
                tracing::info!(current = %self.current_version, "Already up to date");
                Ok(None)
            }
        }
    }

    /// Convenience wrapper that returns an `UpdateStatus` instead of a Result.
    pub fn status(&self) -> UpdateStatus {
        match self.check_for_update() {
            Ok(Some(info)) => UpdateStatus::UpdateAvailable(info),
            Ok(None) => UpdateStatus::UpToDate,
            Err(e) => UpdateStatus::CheckFailed(e.to_string()),
        }
    }
}

/// Compare two semver-like version strings (major.minor.patch).
///
/// Each component is parsed as a u64 and compared numerically.
/// Missing components are treated as 0 (e.g. "1.2" is equivalent to "1.2.0").
pub fn compare_versions(current: &str, latest: &str) -> Ordering {
    let parse = |v: &str| -> std::result::Result<Vec<u64>, UpdateError> {
        let stripped = v.strip_prefix('v').unwrap_or(v);
        stripped
            .split('.')
            .map(|part| {
                part.parse::<u64>()
                    .map_err(|_| UpdateError::VersionParse(v.to_string()))
            })
            .collect()
    };

    let current_parts = match parse(current) {
        Ok(p) => p,
        Err(_) => return Ordering::Equal,
    };
    let latest_parts = match parse(latest) {
        Ok(p) => p,
        Err(_) => return Ordering::Equal,
    };

    let max_len = current_parts.len().max(latest_parts.len());
    for i in 0..max_len {
        let c = current_parts.get(i).copied().unwrap_or(0);
        let l = latest_parts.get(i).copied().unwrap_or(0);
        match c.cmp(&l) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    Ordering::Equal
}

/// Stub for fetching release notes for a given tag from GitHub.
///
/// In a full implementation this would call the GitHub API at:
///   `https://api.github.com/repos/{repo}/releases/tags/{tag}`
/// and extract the body field from the response JSON.
pub fn get_release_notes(_tag: &str) -> Result<String> {
    // Stub implementation
    Ok(String::from(
        "Release notes are not yet available (stub implementation).",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
    }

    #[test]
    fn test_compare_less() {
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "1.1.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "2.0.0"), Ordering::Less);
    }

    #[test]
    fn test_compare_greater() {
        assert_eq!(compare_versions("2.0.0", "1.9.9"), Ordering::Greater);
    }

    #[test]
    fn test_compare_with_v_prefix() {
        assert_eq!(compare_versions("v1.0.0", "v1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("v2.0.0", "1.0.0"), Ordering::Greater);
    }

    #[test]
    fn test_compare_different_lengths() {
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1.0.1"), Ordering::Less);
    }

    #[test]
    fn test_checker_up_to_date() {
        let checker = UpdateChecker::new("0.1.0");
        // Stub always returns current version as latest, so no update
        let result = checker.check_for_update().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_release_notes_stub() {
        let notes = get_release_notes("v0.1.0").unwrap();
        assert!(!notes.is_empty());
    }
}
