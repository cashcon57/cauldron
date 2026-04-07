use crate::classifier::{ClassifiedCommit, Transferability};
use crate::adapter::AdaptationResult;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplicatorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("Patch failed for {hash}: {reason}")]
    PatchFailed { hash: String, reason: String },
    #[error("Source tree not initialized")]
    SourceNotReady,
    #[error("Database error: {0}")]
    Db(String),
}

/// The outcome of attempting to apply a single patch.
#[derive(Debug, Clone)]
pub enum PatchOutcome {
    /// Patch applied cleanly to the Wine source tree.
    Applied {
        hash: String,
        files_changed: usize,
    },
    /// Patch could not be applied (conflicts or malformed diff).
    Conflicted {
        hash: String,
        conflicts: Vec<String>,
    },
    /// Patch was skipped (build system, low transferability, etc.).
    Skipped {
        hash: String,
        reason: String,
    },
    /// Patch needs manual review before it can be applied.
    Deferred {
        hash: String,
        reason: String,
    },
}

impl PatchOutcome {
    pub fn hash(&self) -> &str {
        match self {
            Self::Applied { hash, .. }
            | Self::Conflicted { hash, .. }
            | Self::Skipped { hash, .. }
            | Self::Deferred { hash, .. } => hash,
        }
    }

    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied { .. })
    }
}

/// Summary of a batch patch application run.
#[derive(Debug, Clone, Default)]
pub struct ApplyBatchResult {
    pub applied: usize,
    pub conflicted: usize,
    pub skipped: usize,
    pub deferred: usize,
    pub outcomes: Vec<PatchOutcome>,
}

/// Applies classified Proton patches to a Wine source tree.
///
/// The applicator is the bridge between the sync pipeline (which classifies
/// commits) and the Wine source tree (where patches land). It implements
/// the decision logic from the architecture doc:
///
/// - **High transferability + DirectApply** → apply directly via `git apply`
/// - **Medium transferability** → defer for manual review
/// - **Low/None** → skip or translate
pub struct PatchApplicator {
    /// Path to the Wine source tree (managed by WineSourceManager).
    pub wine_source_dir: PathBuf,
    /// Whether to stop on the first conflict or continue with remaining patches.
    pub stop_on_conflict: bool,
    /// Whether to create a git commit for each applied patch.
    pub auto_commit: bool,
}

impl PatchApplicator {
    pub fn new(wine_source_dir: PathBuf) -> Self {
        Self {
            wine_source_dir,
            stop_on_conflict: false,
            auto_commit: true,
        }
    }

    /// Decide whether a classified commit should be applied, skipped, or deferred.
    pub fn triage(
        &self,
        commit: &ClassifiedCommit,
        adaptation: &AdaptationResult,
    ) -> TriageDecision {
        match adaptation {
            AdaptationResult::DirectApply(_) => {
                // Only auto-apply high-transferability commits
                match commit.transferability {
                    Transferability::High => TriageDecision::Apply,
                    Transferability::Medium => TriageDecision::Defer(
                        "Medium transferability — review before applying".to_string(),
                    ),
                    _ => TriageDecision::Skip(
                        "Low transferability on DirectApply — unusual, skipping".to_string(),
                    ),
                }
            }
            AdaptationResult::NeedsTranslation {
                linux_mechanism,
                macos_equivalent,
                ..
            } => TriageDecision::Defer(format!(
                "Needs translation: {} → {}",
                linux_mechanism, macos_equivalent
            )),
            AdaptationResult::Skip(reason) => TriageDecision::Skip(reason.clone()),
            AdaptationResult::ManualReview(reason) => TriageDecision::Defer(reason.clone()),
        }
    }

    /// Apply a single patch (unified diff) to the Wine source tree.
    /// When `force` is false, triage decides whether to apply, skip, or defer.
    /// When `force` is true (user explicitly clicked Apply), triage is bypassed.
    pub fn apply_one(
        &self,
        commit: &ClassifiedCommit,
        adaptation: &AdaptationResult,
    ) -> Result<PatchOutcome, ApplicatorError> {
        self.apply_one_inner(commit, adaptation, false)
    }

    /// Force-apply a patch, bypassing triage. Used when the user explicitly
    /// clicks Apply in the UI.
    pub fn force_apply_one(
        &self,
        commit: &ClassifiedCommit,
    ) -> Result<PatchOutcome, ApplicatorError> {
        // Use a dummy DirectApply adaptation since we're skipping triage anyway
        let dummy = AdaptationResult::DirectApply("User-initiated apply".to_string());
        self.apply_one_inner(commit, &dummy, true)
    }

    fn apply_one_inner(
        &self,
        commit: &ClassifiedCommit,
        adaptation: &AdaptationResult,
        force: bool,
    ) -> Result<PatchOutcome, ApplicatorError> {
        if !self.wine_source_dir.join(".git").exists() {
            return Err(ApplicatorError::SourceNotReady);
        }

        if !force {
            let decision = self.triage(commit, adaptation);

            match decision {
                TriageDecision::Skip(reason) => {
                    tracing::info!(hash = %commit.hash, reason = %reason, "Skipping patch");
                    return Ok(PatchOutcome::Skipped {
                        hash: commit.hash.clone(),
                        reason,
                    });
                }
                TriageDecision::Defer(reason) => {
                    tracing::info!(hash = %commit.hash, reason = %reason, "Deferring patch");
                    return Ok(PatchOutcome::Deferred {
                        hash: commit.hash.clone(),
                        reason,
                    });
                }
                TriageDecision::Apply => {
                    tracing::info!(
                        hash = %commit.hash,
                        classification = ?commit.classification,
                        "Applying patch"
                    );
                }
            }
        } else {
            tracing::info!(
                hash = %commit.hash,
                classification = ?commit.classification,
                "Force-applying patch (user-initiated)"
            );
        }

        // The diff from the Proton monitor is the raw unified diff text
        if commit.diff.is_empty() {
            return Ok(PatchOutcome::Skipped {
                hash: commit.hash.clone(),
                reason: "Empty diff".to_string(),
            });
        }

        // Try to apply the patch using git apply
        let apply_result = self.git_apply_patch(&commit.diff, &commit.hash)?;

        if apply_result.success {
            if self.auto_commit {
                self.commit_patch(commit)?;
            }

            Ok(PatchOutcome::Applied {
                hash: commit.hash.clone(),
                files_changed: apply_result.files_changed,
            })
        } else {
            Ok(PatchOutcome::Conflicted {
                hash: commit.hash.clone(),
                conflicts: apply_result.conflicts,
            })
        }
    }

    /// Apply a batch of classified commits with their adaptations.
    pub fn apply_batch(
        &self,
        commits: &[(ClassifiedCommit, AdaptationResult)],
    ) -> Result<ApplyBatchResult, ApplicatorError> {
        let mut result = ApplyBatchResult::default();

        tracing::info!(
            count = commits.len(),
            "Starting batch patch application"
        );

        for (commit, adaptation) in commits {
            let outcome = self.apply_one(commit, adaptation)?;

            match &outcome {
                PatchOutcome::Applied { .. } => result.applied += 1,
                PatchOutcome::Conflicted { .. } => {
                    result.conflicted += 1;
                    if self.stop_on_conflict {
                        result.outcomes.push(outcome);
                        tracing::warn!(
                            hash = %commit.hash,
                            "Stopping batch due to conflict (stop_on_conflict=true)"
                        );
                        break;
                    }
                }
                PatchOutcome::Skipped { .. } => result.skipped += 1,
                PatchOutcome::Deferred { .. } => result.deferred += 1,
            }

            result.outcomes.push(outcome);
        }

        tracing::info!(
            applied = result.applied,
            conflicted = result.conflicted,
            skipped = result.skipped,
            deferred = result.deferred,
            "Batch patch application complete"
        );

        Ok(result)
    }

    /// Low-level: apply a unified diff using `git apply`.
    ///
    /// We use the git CLI rather than libgit2 for patch application because
    /// `git apply` handles fuzzy matching, whitespace normalization, and
    /// partial application better than git2's `Repository::apply`.
    ///
    /// Patch files are written to a temp directory (not the source tree) so
    /// they never become orphaned even if the process is interrupted.
    fn git_apply_patch(
        &self,
        diff: &str,
        hash: &str,
    ) -> Result<ApplyResult, ApplicatorError> {
        // Write the diff to a system temp directory — never inside the source
        // tree, so interrupted runs can't leave orphaned .diff files.
        let tmp_dir = tempfile::tempdir().map_err(|e| {
            ApplicatorError::PatchFailed {
                hash: hash.to_string(),
                reason: format!("Failed to create temp dir: {e}"),
            }
        })?;
        let patch_file = tmp_dir.path().join(format!("cauldron-patch-{}.diff", hash));
        std::fs::write(&patch_file, diff)?;

        // First, do a dry run to check if the patch applies
        let check = std::process::Command::new("git")
            .args([
                "apply",
                "--check",
                "--verbose",
                // Allow some fuzz for patches that are slightly offset
                "--recount",
                &patch_file.to_string_lossy(),
            ])
            .current_dir(&self.wine_source_dir)
            .output()?;

        if !check.status.success() {
            let stderr = String::from_utf8_lossy(&check.stderr).to_string();
            // tmp_dir drops here, cleaning up the patch file automatically

            tracing::debug!(
                hash = %hash,
                stderr = %stderr,
                "Patch dry-run failed"
            );

            return Ok(ApplyResult {
                success: false,
                files_changed: 0,
                conflicts: parse_apply_errors(&stderr),
            });
        }

        // Apply for real
        let apply = std::process::Command::new("git")
            .args([
                "apply",
                "--verbose",
                "--recount",
                &patch_file.to_string_lossy(),
            ])
            .current_dir(&self.wine_source_dir)
            .output()?;

        // tmp_dir drops here, cleaning up the patch file automatically

        if apply.status.success() {
            let stdout = String::from_utf8_lossy(&apply.stderr);
            let files_changed = stdout.lines().filter(|l| l.contains("Applied patch")).count();

            Ok(ApplyResult {
                success: true,
                files_changed: if files_changed > 0 { files_changed } else { 1 },
                conflicts: vec![],
            })
        } else {
            let stderr = String::from_utf8_lossy(&apply.stderr).to_string();
            Ok(ApplyResult {
                success: false,
                files_changed: 0,
                conflicts: parse_apply_errors(&stderr),
            })
        }
    }

    /// Clean up any orphaned patch artifacts from the Wine source tree.
    ///
    /// Scans for `.cauldron-patch-*.diff` files and `.rej` reject files
    /// that may have been left behind by older versions or interrupted runs.
    pub fn cleanup_orphans(&self) -> Result<usize, ApplicatorError> {
        let mut cleaned = 0;

        if !self.wine_source_dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(&self.wine_source_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".cauldron-patch-") && name.ends_with(".diff") {
                tracing::info!("Cleaning orphaned patch file: {}", name);
                std::fs::remove_file(entry.path())?;
                cleaned += 1;
            }
        }

        // Also clean .rej files anywhere in the tree (from failed patches)
        let output = std::process::Command::new("find")
            .args([
                &self.wine_source_dir.to_string_lossy().to_string(),
                "-name",
                "*.rej",
                "-type",
                "f",
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let path = std::path::Path::new(line.trim());
                if path.exists() {
                    tracing::info!("Cleaning reject file: {}", path.display());
                    let _ = std::fs::remove_file(path);
                    cleaned += 1;
                }
            }
        }

        if cleaned > 0 {
            tracing::info!("Cleaned {} orphaned files", cleaned);
        }

        Ok(cleaned)
    }

    /// Commit the currently staged changes as a Proton patch.
    fn commit_patch(&self, commit: &ClassifiedCommit) -> Result<(), ApplicatorError> {
        let message = format!(
            "[cauldron-sync] Apply Proton patch: {}\n\nOriginal commit: {}\nAuthor: {}\nClassification: {:?}\nTransferability: {:?}",
            truncate_first_line(&commit.message, 72),
            commit.hash,
            commit.author,
            commit.classification,
            commit.transferability,
        );

        let status = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.wine_source_dir)
            .status()?;

        if !status.success() {
            return Err(ApplicatorError::PatchFailed {
                hash: commit.hash.clone(),
                reason: "git add -A failed".to_string(),
            });
        }

        let status = std::process::Command::new("git")
            .args([
                "commit",
                "-m",
                &message,
                "--author",
                &format!("{} <cauldron-sync@cauldron.dev>", commit.author),
            ])
            .current_dir(&self.wine_source_dir)
            .status()?;

        if !status.success() {
            tracing::warn!(hash = %commit.hash, "git commit failed — may have no changes to commit");
        }

        Ok(())
    }
}

/// The triage decision for a single commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriageDecision {
    /// Apply the patch directly.
    Apply,
    /// Skip this patch entirely.
    Skip(String),
    /// Defer for manual review.
    Defer(String),
}

/// Internal result of a git apply attempt.
#[derive(Debug)]
struct ApplyResult {
    success: bool,
    files_changed: usize,
    conflicts: Vec<String>,
}

/// Parse error messages from `git apply` stderr into structured conflict descriptions.
fn parse_apply_errors(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| {
            line.contains("error:") || line.contains("patch does not apply")
        })
        .map(|line| line.trim().to_string())
        .collect()
}

fn truncate_first_line(msg: &str, max: usize) -> String {
    let first = msg.lines().next().unwrap_or(msg);
    if first.len() > max {
        format!("{}...", &first[..max - 3])
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::{Classification, Transferability};
    use crate::adapter::AdaptationResult;

    fn make_commit(
        hash: &str,
        classification: Classification,
        transferability: Transferability,
        diff: &str,
    ) -> ClassifiedCommit {
        ClassifiedCommit {
            hash: hash.to_string(),
            message: "Test commit".to_string(),
            author: "dev".to_string(),
            timestamp: "2024-01-01".to_string(),
            diff: diff.to_string(),
            affected_files: vec![],
            classification,
            transferability,
            suggested_action: String::new(),
        }
    }

    #[test]
    fn test_triage_high_direct_apply() {
        let applicator = PatchApplicator::new(PathBuf::from("/tmp/wine"));
        let commit = make_commit("abc", Classification::WineApiFix, Transferability::High, "");
        let adaptation = AdaptationResult::DirectApply("test".to_string());

        assert_eq!(applicator.triage(&commit, &adaptation), TriageDecision::Apply);
    }

    #[test]
    fn test_triage_medium_direct_apply_defers() {
        let applicator = PatchApplicator::new(PathBuf::from("/tmp/wine"));
        let commit = make_commit("abc", Classification::Vkd3dFix, Transferability::Medium, "");
        let adaptation = AdaptationResult::DirectApply("test".to_string());

        matches!(applicator.triage(&commit, &adaptation), TriageDecision::Defer(_));
    }

    #[test]
    fn test_triage_skip() {
        let applicator = PatchApplicator::new(PathBuf::from("/tmp/wine"));
        let commit = make_commit("abc", Classification::BuildSystem, Transferability::None, "");
        let adaptation = AdaptationResult::Skip("build system".to_string());

        matches!(applicator.triage(&commit, &adaptation), TriageDecision::Skip(_));
    }

    #[test]
    fn test_triage_needs_translation_defers() {
        let applicator = PatchApplicator::new(PathBuf::from("/tmp/wine"));
        let commit = make_commit("abc", Classification::KernelWorkaround, Transferability::Low, "");
        let adaptation = AdaptationResult::NeedsTranslation {
            linux_mechanism: "futex".to_string(),
            macos_equivalent: "MSync".to_string(),
            notes: "test".to_string(),
        };

        matches!(applicator.triage(&commit, &adaptation), TriageDecision::Defer(_));
    }

    #[test]
    fn test_apply_one_empty_diff_skips() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a fake git dir so the source check passes
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();

        let applicator = PatchApplicator::new(tmp.path().to_path_buf());
        let commit = make_commit("abc", Classification::WineApiFix, Transferability::High, "");
        let adaptation = AdaptationResult::DirectApply("test".to_string());

        let result = applicator.apply_one(&commit, &adaptation).unwrap();
        assert!(matches!(result, PatchOutcome::Skipped { .. }));
    }

    #[test]
    fn test_apply_one_source_not_ready() {
        let applicator = PatchApplicator::new(PathBuf::from("/nonexistent/wine/source"));
        let commit = make_commit("abc", Classification::WineApiFix, Transferability::High, "diff");
        let adaptation = AdaptationResult::DirectApply("test".to_string());

        let result = applicator.apply_one(&commit, &adaptation);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_result_default() {
        let result = ApplyBatchResult::default();
        assert_eq!(result.applied, 0);
        assert_eq!(result.conflicted, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.deferred, 0);
        assert!(result.outcomes.is_empty());
    }

    #[test]
    fn test_patch_outcome_hash() {
        let outcome = PatchOutcome::Applied {
            hash: "abc123".to_string(),
            files_changed: 2,
        };
        assert_eq!(outcome.hash(), "abc123");
        assert!(outcome.is_applied());

        let skipped = PatchOutcome::Skipped {
            hash: "def456".to_string(),
            reason: "test".to_string(),
        };
        assert_eq!(skipped.hash(), "def456");
        assert!(!skipped.is_applied());
    }

    #[test]
    fn test_parse_apply_errors() {
        let stderr = "error: patch failed: dlls/ntdll/sync.c:42\nerror: dlls/ntdll/sync.c: patch does not apply\n";
        let errors = parse_apply_errors(stderr);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("patch failed"));
    }

    #[test]
    fn test_truncate_first_line() {
        assert_eq!(truncate_first_line("short", 80), "short");
        let long = "a".repeat(100);
        let truncated = truncate_first_line(&long, 80);
        assert_eq!(truncated.len(), 80);
        assert!(truncated.ends_with("..."));
    }
}
