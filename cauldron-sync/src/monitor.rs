use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Repository not initialized at {0}")]
    NotInitialized(PathBuf),
}

/// A raw commit extracted from the Proton git repository.
#[derive(Debug, Clone)]
pub struct RawCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: String,
    pub diff: String,
    pub affected_files: Vec<String>,
}

/// Monitors a Proton git repository for new commits.
pub struct ProtonMonitor {
    pub repo_path: PathBuf,
    pub remote_url: String,
    pub poll_interval: Duration,
}

impl ProtonMonitor {
    /// Create a new monitor for the given repository path and remote.
    pub fn new(repo_path: PathBuf, remote_url: String, poll_interval: Duration) -> Self {
        Self {
            repo_path,
            remote_url,
            poll_interval,
        }
    }

    /// Poll the remote repository once and return any new commits.
    pub async fn poll_once(&self) -> Result<Vec<RawCommit>, MonitorError> {
        tracing::info!(
            "Polling Proton repo at {} from {}",
            self.repo_path.display(),
            self.remote_url
        );

        let repo = if self.repo_path.join(".git").exists() || self.repo_path.join("HEAD").exists() {
            git2::Repository::open(&self.repo_path)?
        } else {
            tracing::info!("Cloning {} into {}", self.remote_url, self.repo_path.display());
            git2::Repository::clone(&self.remote_url, &self.repo_path)?
        };

        // Fetch from origin
        let mut remote = repo.find_remote("origin").or_else(|_| {
            repo.remote_anonymous(&self.remote_url)
        })?;

        // Proton uses versioned branches, not main/master.
        // Fetch the latest stable and experimental branches.
        remote.fetch(
            &["proton_10.0", "experimental_10.0", "bleeding-edge"],
            None,
            None,
        )?;

        // Walk commits on FETCH_HEAD
        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_oid = fetch_head.target().ok_or_else(|| {
            MonitorError::NotInitialized(self.repo_path.clone())
        })?;

        let mut revwalk = repo.revwalk()?;
        revwalk.push(fetch_oid)?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        // Collect up to 100 recent commits
        let mut commits = Vec::new();
        for oid_result in revwalk.take(100) {
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;

            let message = commit.message().unwrap_or("").to_string();
            let author = commit
                .author()
                .name()
                .unwrap_or("unknown")
                .to_string();
            let timestamp = commit.time().seconds().to_string();

            // Compute diff against parent
            let mut affected_files = Vec::new();
            let mut diff_text = String::new();

            let tree = commit.tree()?;
            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

            let diff = repo.diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&tree),
                None,
            )?;

            diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                if let Ok(content) = std::str::from_utf8(line.content()) {
                    diff_text.push_str(content);
                }
                true
            })?;

            for delta_idx in 0..diff.deltas().len() {
                if let Some(delta) = diff.deltas().nth(delta_idx) {
                    if let Some(path) = delta.new_file().path() {
                        affected_files.push(path.to_string_lossy().to_string());
                    }
                }
            }

            commits.push(RawCommit {
                hash: oid.to_string(),
                message,
                author,
                timestamp,
                diff: diff_text,
                affected_files,
            });
        }

        tracing::info!("Found {} commits from poll", commits.len());
        Ok(commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_commit_creation() {
        let commit = RawCommit {
            hash: "abc123def456".to_string(),
            message: "Fix Wine sync primitives".to_string(),
            author: "developer@example.com".to_string(),
            timestamp: "2024-03-15T10:00:00Z".to_string(),
            diff: "+some added line\n-some removed line".to_string(),
            affected_files: vec![
                "dlls/ntdll/sync.c".to_string(),
                "server/thread.c".to_string(),
            ],
        };

        assert_eq!(commit.hash, "abc123def456");
        assert_eq!(commit.affected_files.len(), 2);
        assert!(commit.diff.contains("+some added line"));
    }

    #[test]
    fn test_proton_monitor_creation() {
        let monitor = ProtonMonitor::new(
            PathBuf::from("/tmp/proton-repo"),
            "https://github.com/ValveSoftware/Proton.git".to_string(),
            Duration::from_secs(300),
        );

        assert_eq!(monitor.repo_path, PathBuf::from("/tmp/proton-repo"));
        assert_eq!(monitor.poll_interval, Duration::from_secs(300));
        assert!(monitor.remote_url.contains("Proton"));
    }

    #[test]
    fn test_raw_commit_empty_fields() {
        let commit = RawCommit {
            hash: String::new(),
            message: String::new(),
            author: String::new(),
            timestamp: String::new(),
            diff: String::new(),
            affected_files: vec![],
        };

        assert!(commit.hash.is_empty());
        assert!(commit.affected_files.is_empty());
    }
}
