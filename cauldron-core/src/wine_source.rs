use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WineSourceError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Source tree not initialized at {0}")]
    NotInitialized(PathBuf),
    #[error("Branch error: {0}")]
    Branch(String),
    #[error("Patch conflict: {0}")]
    PatchConflict(String),
}

/// Upstream Wine source repositories that Cauldron can track.
pub struct WineUpstream {
    /// Display name for this upstream.
    pub name: &'static str,
    /// Git remote URL.
    pub url: &'static str,
    /// Default branch to track.
    pub branch: &'static str,
}

/// Known upstream Wine sources, ordered by preference.
pub const UPSTREAMS: &[WineUpstream] = &[
    WineUpstream {
        name: "wine-mirror",
        url: "https://github.com/wine-mirror/wine.git",
        branch: "master",
    },
    WineUpstream {
        name: "valve-proton-wine",
        url: "https://github.com/ValveSoftware/wine.git",
        branch: "proton_9.0",
    },
    WineUpstream {
        name: "proton-ge-wine",
        url: "https://github.com/GloriousEggroll/proton-ge-custom.git",
        branch: "master",
    },
];

/// Manages Cauldron's local Wine source tree where Proton patches are applied.
///
/// The source tree has three key branches:
/// - `upstream/master` — tracks upstream Wine (read-only mirror)
/// - `cauldron/base` — our baseline with macOS-specific foundation patches
/// - `cauldron/patched` — base + automatically applied Proton patches
pub struct WineSourceManager {
    /// Root directory of the Wine source tree.
    pub source_dir: PathBuf,
    /// Which upstream to track.
    pub upstream_url: String,
    /// The upstream branch name.
    pub upstream_branch: String,
}

impl WineSourceManager {
    /// Create a new source manager. Does not clone or initialize anything yet.
    pub fn new(base_dir: PathBuf) -> Self {
        let source_dir = base_dir.join("wine-source");
        Self {
            source_dir,
            upstream_url: UPSTREAMS[0].url.to_string(),
            upstream_branch: UPSTREAMS[0].branch.to_string(),
        }
    }

    /// Create a source manager tracking a specific upstream.
    pub fn with_upstream(base_dir: PathBuf, url: String, branch: String) -> Self {
        let source_dir = base_dir.join("wine-source");
        Self {
            source_dir,
            upstream_url: url,
            upstream_branch: branch,
        }
    }

    /// Whether the source tree has been initialized (cloned).
    pub fn is_initialized(&self) -> bool {
        self.source_dir.join(".git").exists() || self.source_dir.join("HEAD").exists()
    }

    /// Clone the upstream Wine repository into the source directory.
    ///
    /// Uses a shallow clone (depth=1) by default to save disk space and time.
    /// The full history can be fetched later if needed for bisection.
    pub fn clone_upstream(&self, shallow: bool) -> Result<(), WineSourceError> {
        if self.is_initialized() {
            tracing::info!(
                "Wine source already initialized at {}",
                self.source_dir.display()
            );
            return Ok(());
        }

        tracing::info!(
            "Cloning Wine source from {} (branch: {}) into {}",
            self.upstream_url,
            self.upstream_branch,
            self.source_dir.display()
        );

        std::fs::create_dir_all(&self.source_dir)?;

        if shallow {
            // Use git CLI for shallow clone — git2 doesn't support --depth well
            let status = std::process::Command::new("git")
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "--branch",
                    &self.upstream_branch,
                    "--single-branch",
                    &self.upstream_url,
                    &self.source_dir.to_string_lossy(),
                ])
                .status()?;

            if !status.success() {
                return Err(WineSourceError::Branch(format!(
                    "git clone failed with status: {status}"
                )));
            }
        } else {
            git2::Repository::clone(&self.upstream_url, &self.source_dir)?;
        }

        // Create our working branches
        self.setup_cauldron_branches()?;

        tracing::info!("Wine source cloned and branches set up");
        Ok(())
    }

    /// Fetch latest commits from the upstream remote.
    pub fn fetch_upstream(&self) -> Result<FetchResult, WineSourceError> {
        self.ensure_initialized()?;

        tracing::info!("Fetching latest upstream Wine commits");

        let repo = git2::Repository::open(&self.source_dir)?;
        let mut remote = repo.find_remote("origin")?;

        // Get the current HEAD before fetch to detect new commits
        let before_oid = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .map(|o| o.to_string())
            .unwrap_or_default();

        remote.fetch(&[&self.upstream_branch], None, None)?;

        // Resolve FETCH_HEAD
        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let after_oid = fetch_head
            .target()
            .map(|o| o.to_string())
            .unwrap_or_default();

        let new_commits = if before_oid == after_oid {
            0
        } else {
            // Count commits between before and after
            count_commits_between(&repo, &before_oid, &after_oid).unwrap_or(0)
        };

        tracing::info!(
            new_commits = new_commits,
            "Upstream fetch complete"
        );

        Ok(FetchResult {
            new_commits,
            before_hash: before_oid,
            after_hash: after_oid,
        })
    }

    /// Create the `cauldron/base` and `cauldron/patched` branches if they
    /// don't exist yet.
    fn setup_cauldron_branches(&self) -> Result<(), WineSourceError> {
        let repo = git2::Repository::open(&self.source_dir)?;
        let head_commit = repo.head()?.peel_to_commit()?;

        // Create cauldron/base from current HEAD if it doesn't exist
        if repo.find_branch("cauldron/base", git2::BranchType::Local).is_err() {
            repo.branch("cauldron/base", &head_commit, false)?;
            tracing::info!("Created branch cauldron/base");
        }

        // Create cauldron/patched from cauldron/base
        if repo.find_branch("cauldron/patched", git2::BranchType::Local).is_err() {
            repo.branch("cauldron/patched", &head_commit, false)?;
            tracing::info!("Created branch cauldron/patched");
        }

        Ok(())
    }

    /// Switch the working tree to a specific branch.
    pub fn checkout_branch(&self, branch_name: &str) -> Result<(), WineSourceError> {
        self.ensure_initialized()?;

        let repo = git2::Repository::open(&self.source_dir)?;
        let branch = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|_| WineSourceError::Branch(format!("Branch '{}' not found", branch_name)))?;

        let refname = branch
            .get()
            .name()
            .ok_or_else(|| WineSourceError::Branch("Invalid branch ref".to_string()))?;

        let obj = repo
            .revparse_single(refname)?;
        repo.checkout_tree(&obj, None)?;
        repo.set_head(refname)?;

        tracing::info!("Checked out branch: {}", branch_name);
        Ok(())
    }

    /// Apply a unified diff patch to the current branch.
    ///
    /// Returns the number of hunks successfully applied.
    pub fn apply_patch(&self, patch_content: &str) -> Result<PatchResult, WineSourceError> {
        self.ensure_initialized()?;

        let repo = git2::Repository::open(&self.source_dir)?;
        let diff = git2::Diff::from_buffer(patch_content.as_bytes())?;

        // Try to apply the patch
        match repo.apply(&diff, git2::ApplyLocation::WorkDir, None) {
            Ok(()) => {
                let hunks = diff.stats()?.files_changed();
                tracing::info!(hunks = hunks, "Patch applied successfully");
                Ok(PatchResult {
                    success: true,
                    files_changed: hunks,
                    conflicts: Vec::new(),
                })
            }
            Err(e) => {
                tracing::warn!("Patch application failed: {}", e);
                Ok(PatchResult {
                    success: false,
                    files_changed: 0,
                    conflicts: vec![e.message().to_string()],
                })
            }
        }
    }

    /// Apply a patch and commit the result to the current branch.
    pub fn apply_and_commit(
        &self,
        patch_content: &str,
        commit_message: &str,
        author_name: &str,
        author_email: &str,
    ) -> Result<PatchResult, WineSourceError> {
        let result = self.apply_patch(patch_content)?;

        if !result.success {
            return Ok(result);
        }

        let repo = git2::Repository::open(&self.source_dir)?;

        // Stage all changes
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        // Create commit
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        let head = repo.head()?.peel_to_commit()?;
        let sig = git2::Signature::now(author_name, author_email)?;

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            commit_message,
            &tree,
            &[&head],
        )?;

        tracing::info!("Committed patch: {}", commit_message);
        Ok(result)
    }

    /// Get the current HEAD commit hash on the active branch.
    pub fn current_head(&self) -> Result<String, WineSourceError> {
        self.ensure_initialized()?;
        let repo = git2::Repository::open(&self.source_dir)?;
        let head = repo.head()?;
        Ok(head
            .target()
            .map(|o| o.to_string())
            .unwrap_or_default())
    }

    /// Get the name of the currently checked-out branch.
    pub fn current_branch(&self) -> Result<String, WineSourceError> {
        self.ensure_initialized()?;
        let repo = git2::Repository::open(&self.source_dir)?;
        let head = repo.head()?;
        Ok(head
            .shorthand()
            .unwrap_or("detached")
            .to_string())
    }

    /// List all local branches.
    pub fn list_branches(&self) -> Result<Vec<String>, WineSourceError> {
        self.ensure_initialized()?;
        let repo = git2::Repository::open(&self.source_dir)?;
        let branches = repo.branches(Some(git2::BranchType::Local))?;

        let mut names = Vec::new();
        for branch in branches {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                names.push(name.to_string());
            }
        }

        Ok(names)
    }

    /// Reset the `cauldron/patched` branch back to `cauldron/base`.
    ///
    /// This throws away all applied patches and starts fresh.
    pub fn reset_patched_to_base(&self) -> Result<(), WineSourceError> {
        self.ensure_initialized()?;

        let repo = git2::Repository::open(&self.source_dir)?;

        let base_branch = repo
            .find_branch("cauldron/base", git2::BranchType::Local)
            .map_err(|_| WineSourceError::Branch("cauldron/base not found".to_string()))?;

        let base_commit = base_branch.get().peel_to_commit()?;

        // Checkout cauldron/patched and reset to base
        self.checkout_branch("cauldron/patched")?;

        let base_obj = base_commit.into_object();
        repo.reset(&base_obj, git2::ResetType::Hard, None)?;

        tracing::info!("Reset cauldron/patched to cauldron/base");
        Ok(())
    }

    /// Rebase `cauldron/base` onto the latest upstream, then rebuild
    /// `cauldron/patched` on top of the new base.
    ///
    /// This is the main "update" operation that brings in new Wine commits
    /// while preserving Cauldron's patches.
    pub fn update_base_from_upstream(&self) -> Result<UpdateResult, WineSourceError> {
        self.ensure_initialized()?;

        // Fetch latest upstream
        let fetch = self.fetch_upstream()?;
        if fetch.new_commits == 0 {
            return Ok(UpdateResult {
                new_upstream_commits: 0,
                rebase_conflicts: Vec::new(),
            });
        }

        // Use git CLI for rebase since git2 rebase support is limited
        let status = std::process::Command::new("git")
            .args(["rebase", "origin/master", "cauldron/base"])
            .current_dir(&self.source_dir)
            .status()?;

        let mut conflicts = Vec::new();
        if !status.success() {
            // Abort the rebase and report conflicts
            let _ = std::process::Command::new("git")
                .args(["rebase", "--abort"])
                .current_dir(&self.source_dir)
                .status();
            conflicts.push("Rebase of cauldron/base onto upstream failed".to_string());
        }

        Ok(UpdateResult {
            new_upstream_commits: fetch.new_commits,
            rebase_conflicts: conflicts,
        })
    }

    fn ensure_initialized(&self) -> Result<(), WineSourceError> {
        if !self.is_initialized() {
            return Err(WineSourceError::NotInitialized(self.source_dir.clone()));
        }
        Ok(())
    }
}

/// Result of fetching upstream commits.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub new_commits: usize,
    pub before_hash: String,
    pub after_hash: String,
}

/// Result of applying a patch.
#[derive(Debug, Clone)]
pub struct PatchResult {
    pub success: bool,
    pub files_changed: usize,
    pub conflicts: Vec<String>,
}

/// Result of updating the base from upstream.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub new_upstream_commits: usize,
    pub rebase_conflicts: Vec<String>,
}

/// Count commits between two refs. Returns 0 on error.
fn count_commits_between(
    repo: &git2::Repository,
    from: &str,
    to: &str,
) -> Result<usize, git2::Error> {
    if from.is_empty() {
        return Ok(0);
    }

    let from_oid = git2::Oid::from_str(from)?;
    let to_oid = git2::Oid::from_str(to)?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(to_oid)?;
    revwalk.hide(from_oid)?;

    Ok(revwalk.count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wine_source_manager_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = WineSourceManager::new(tmp.path().to_path_buf());
        assert_eq!(mgr.source_dir, tmp.path().join("wine-source"));
        assert!(!mgr.is_initialized());
    }

    #[test]
    fn test_wine_source_manager_with_upstream() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = WineSourceManager::with_upstream(
            tmp.path().to_path_buf(),
            "https://example.com/wine.git".to_string(),
            "main".to_string(),
        );
        assert_eq!(mgr.upstream_url, "https://example.com/wine.git");
        assert_eq!(mgr.upstream_branch, "main");
    }

    #[test]
    fn test_upstreams_defined() {
        assert!(!UPSTREAMS.is_empty());
        assert_eq!(UPSTREAMS[0].name, "wine-mirror");
        assert!(UPSTREAMS[0].url.contains("wine-mirror"));
    }

    #[test]
    fn test_not_initialized_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = WineSourceManager::new(tmp.path().to_path_buf());
        let result = mgr.current_head();
        assert!(result.is_err());
    }

    #[test]
    fn test_patch_result_defaults() {
        let result = PatchResult {
            success: true,
            files_changed: 3,
            conflicts: vec![],
        };
        assert!(result.success);
        assert_eq!(result.files_changed, 3);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_fetch_result() {
        let result = FetchResult {
            new_commits: 5,
            before_hash: "abc123".to_string(),
            after_hash: "def456".to_string(),
        };
        assert_eq!(result.new_commits, 5);
    }

    #[test]
    fn test_update_result() {
        let result = UpdateResult {
            new_upstream_commits: 10,
            rebase_conflicts: vec![],
        };
        assert_eq!(result.new_upstream_commits, 10);
        assert!(result.rebase_conflicts.is_empty());
    }
}
