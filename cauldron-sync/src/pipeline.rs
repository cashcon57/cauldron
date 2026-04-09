use crate::adapter::{suggest_adaptation, AdaptationResult};
use crate::applicator::{PatchApplicator, PatchOutcome};
use crate::classifier::{classify, Classification, ClassifiedCommit};
use crate::monitor::ProtonMonitor;
use cauldron_db::models::ProtonCommit;
use cauldron_db::{insert_commit, insert_patch_log, mark_commit_applied, record_sync_run, schema};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("Monitor error: {0}")]
    Monitor(#[from] crate::monitor::MonitorError),
    #[error("Database error: {0}")]
    Db(#[from] cauldron_db::DbError),
    #[error("Schema error: {0}")]
    Schema(#[from] cauldron_db::SchemaError),
    #[error("Sync status error: {0}")]
    SyncStatus(#[from] cauldron_db::SyncStatusError),
}

/// Summary of a single sync cycle.
#[derive(Debug, Clone)]
pub struct SyncRunResult {
    pub total_commits: usize,
    pub classified: ClassificationBreakdown,
    pub applied: usize,
    pub pending_review: usize,
    pub skipped: usize,
    pub duration: Duration,
    pub errors: Vec<String>,
}

/// Breakdown of commits by classification category.
#[derive(Debug, Clone, Default)]
pub struct ClassificationBreakdown {
    pub wine_api_fix: usize,
    pub dxvk_fix: usize,
    pub vkd3d_fix: usize,
    pub game_config: usize,
    pub kernel_workaround: usize,
    pub steam_integration: usize,
    pub build_system: usize,
    pub unknown: usize,
}

impl ClassificationBreakdown {
    fn record(&mut self, c: &Classification) {
        match c {
            Classification::WineApiFix => self.wine_api_fix += 1,
            Classification::DxvkFix => self.dxvk_fix += 1,
            Classification::Vkd3dFix => self.vkd3d_fix += 1,
            Classification::GameConfig => self.game_config += 1,
            Classification::KernelWorkaround => self.kernel_workaround += 1,
            Classification::SteamIntegration => self.steam_integration += 1,
            Classification::BuildSystem => self.build_system += 1,
            Classification::Unknown => self.unknown += 1,
        }
    }
}

/// Orchestrates the monitor -> classifier -> adapter -> applicator pipeline.
pub struct SyncPipeline {
    pub monitor: ProtonMonitor,
    pub db_path: PathBuf,
    /// Optional patch applicator. When set, high-transferability patches are
    /// automatically applied to the Wine source tree.
    pub applicator: Option<PatchApplicator>,
    /// Source label for commits: "proton" or "crossover"
    pub source: String,
}

impl SyncPipeline {
    /// Create a new pipeline without patch application (classify-only mode).
    pub fn new(
        repo_path: PathBuf,
        remote_url: String,
        db_path: PathBuf,
        poll_interval: Duration,
    ) -> Self {
        let monitor = ProtonMonitor::new(repo_path, remote_url, poll_interval);
        Self {
            monitor,
            db_path,
            applicator: None,
            source: "proton".to_string(),
        }
    }

    /// Create a pipeline with patch application enabled.
    pub fn with_applicator(
        repo_path: PathBuf,
        remote_url: String,
        db_path: PathBuf,
        poll_interval: Duration,
        wine_source_dir: PathBuf,
    ) -> Self {
        let monitor = ProtonMonitor::new(repo_path, remote_url, poll_interval);
        Self {
            monitor,
            db_path,
            applicator: Some(PatchApplicator::new(wine_source_dir)),
            source: "proton".to_string(),
        }
    }

    /// Set the source label for commits stored by this pipeline.
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_string();
        self
    }

    /// Execute one full sync cycle: poll, classify, adapt, store, record status.
    pub async fn run_once(&self) -> Result<SyncRunResult, PipelineError> {
        tracing::info!(db_path = %self.db_path.display(), "Starting sync pipeline run");
        let start = Instant::now();
        let mut errors = Vec::new();

        // 1. Poll for new commits (pass last-seen hash to avoid replaying old ones)
        let last_hash = {
            let conn = cauldron_db::init_db(&self.db_path)?;
            cauldron_db::sync_status::get_sync_status(&conn)
                .ok()
                .flatten()
                .and_then(|s| {
                    if s.last_commit_hash.is_empty() { None } else { Some(s.last_commit_hash) }
                })
        };
        let raw_commits = self.monitor.poll_once(last_hash.as_deref()).await?;
        let total_commits = raw_commits.len();
        tracing::info!(total_commits = total_commits, "Polled commits from repository");

        // 2-3. Classify and suggest adaptation for each commit
        let mut classified_commits: Vec<(ClassifiedCommit, AdaptationResult)> = Vec::new();
        for raw in &raw_commits {
            let classified = classify(raw);
            let adaptation = suggest_adaptation(&classified);
            classified_commits.push((classified, adaptation));
        }

        // Build classification breakdown
        let mut breakdown = ClassificationBreakdown::default();
        for (cc, _) in &classified_commits {
            breakdown.record(&cc.classification);
        }

        // 4-5. Open DB, store commits, and apply patches where possible
        let conn = schema::init_db(&self.db_path)?;

        let mut applied = 0usize;
        let mut pending_review = 0usize;
        let mut skipped = 0usize;

        for (cc, adaptation) in &classified_commits {
            let affected_json =
                serde_json::to_string(&cc.affected_files).unwrap_or_else(|_| "[]".to_string());

            let commit_record = ProtonCommit {
                hash: cc.hash.clone(),
                message: cc.message.clone(),
                author: cc.author.clone(),
                timestamp: cc.timestamp.clone(),
                affected_files: affected_json,
                classification: cc.classification.to_string(),
                transferability: cc.transferability.to_string(),
                applied: false,
                source: self.source.clone(),
            };

            if let Err(e) = insert_commit(&conn, &commit_record) {
                errors.push(format!("Failed to insert commit {}: {}", cc.hash, e));
                continue;
            }

            // If we have an applicator, try to apply the patch
            if let Some(ref applicator) = self.applicator {
                match applicator.apply_one(cc, adaptation) {
                    Ok(outcome) => {
                        let (outcome_str, files, conflicts) = match &outcome {
                            PatchOutcome::Applied { files_changed, .. } => {
                                applied += 1;
                                if let Err(e) = mark_commit_applied(&conn, &cc.hash) {
                                    errors.push(format!(
                                        "Applied patch {} but failed to update DB: {}",
                                        cc.hash, e
                                    ));
                                }
                                ("applied", *files_changed, vec![])
                            }
                            PatchOutcome::Conflicted { conflicts, .. } => {
                                pending_review += 1;
                                ("conflicted", 0, conflicts.clone())
                            }
                            PatchOutcome::Skipped { reason, .. } => {
                                skipped += 1;
                                ("skipped", 0, vec![reason.clone()])
                            }
                            PatchOutcome::Deferred { reason, .. } => {
                                pending_review += 1;
                                ("deferred", 0, vec![reason.clone()])
                            }
                        };

                        if let Err(e) = insert_patch_log(
                            &conn,
                            &cc.hash,
                            outcome_str,
                            files,
                            &conflicts,
                        ) {
                            errors.push(format!(
                                "Failed to log patch outcome for {}: {}",
                                cc.hash, e
                            ));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Applicator error for {}: {}", cc.hash, e));
                        pending_review += 1;
                    }
                }
            } else {
                // No applicator — classify-only mode
                let is_skip = matches!(adaptation, AdaptationResult::Skip(_));
                if is_skip {
                    skipped += 1;
                } else {
                    pending_review += 1;
                }
            }
        }

        // 6. Record sync status
        let duration = start.elapsed();
        let error_summary = if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        };

        if let Err(e) = record_sync_run(
            &conn,
            total_commits,
            applied,
            skipped,
            duration.as_millis() as u64,
            error_summary.as_deref(),
        ) {
            errors.push(format!("Failed to record sync status: {}", e));
        }

        // 7. Return result
        Ok(SyncRunResult {
            total_commits,
            classified: breakdown,
            applied,
            pending_review,
            skipped,
            duration,
            errors,
        })
    }

    /// Run the sync pipeline continuously until a shutdown signal is received.
    pub async fn run_continuous(
        &self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        tracing::info!(
            "Starting continuous sync with {}ms poll interval",
            self.monitor.poll_interval.as_millis()
        );

        loop {
            match self.run_once().await {
                Ok(result) => {
                    tracing::info!(
                        "Sync cycle complete: {} commits, {} applied, {} pending, {} skipped in {:?}",
                        result.total_commits,
                        result.applied,
                        result.pending_review,
                        result.skipped,
                        result.duration,
                    );
                    if !result.errors.is_empty() {
                        tracing::warn!("Sync cycle had {} errors", result.errors.len());
                    }
                }
                Err(e) => {
                    tracing::error!("Sync cycle failed: {}", e);
                }
            }

            // Wait for either the poll interval or a shutdown signal
            tokio::select! {
                _ = tokio::time::sleep(self.monitor.poll_interval) => {
                    // Continue to next cycle
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::info!("Shutdown signal received, stopping sync pipeline");
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_breakdown_default() {
        let breakdown = ClassificationBreakdown::default();
        assert_eq!(breakdown.wine_api_fix, 0);
        assert_eq!(breakdown.dxvk_fix, 0);
        assert_eq!(breakdown.vkd3d_fix, 0);
        assert_eq!(breakdown.game_config, 0);
        assert_eq!(breakdown.kernel_workaround, 0);
        assert_eq!(breakdown.steam_integration, 0);
        assert_eq!(breakdown.build_system, 0);
        assert_eq!(breakdown.unknown, 0);
    }

    #[test]
    fn test_classification_breakdown_record() {
        let mut breakdown = ClassificationBreakdown::default();
        breakdown.record(&Classification::WineApiFix);
        breakdown.record(&Classification::WineApiFix);
        breakdown.record(&Classification::DxvkFix);
        breakdown.record(&Classification::BuildSystem);

        assert_eq!(breakdown.wine_api_fix, 2);
        assert_eq!(breakdown.dxvk_fix, 1);
        assert_eq!(breakdown.build_system, 1);
        assert_eq!(breakdown.unknown, 0);
    }

    #[test]
    fn test_sync_run_result_construction() {
        let result = SyncRunResult {
            total_commits: 10,
            classified: ClassificationBreakdown::default(),
            applied: 5,
            pending_review: 3,
            skipped: 2,
            duration: Duration::from_millis(500),
            errors: vec![],
        };

        assert_eq!(result.total_commits, 10);
        assert_eq!(result.applied, 5);
        assert_eq!(result.pending_review, 3);
        assert_eq!(result.skipped, 2);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sync_pipeline_creation() {
        let pipeline = SyncPipeline::new(
            PathBuf::from("/tmp/repo"),
            "https://github.com/test/repo.git".to_string(),
            PathBuf::from("/tmp/db.sqlite"),
            Duration::from_secs(60),
        );

        assert_eq!(pipeline.db_path, PathBuf::from("/tmp/db.sqlite"));
        assert_eq!(pipeline.monitor.poll_interval, Duration::from_secs(60));
    }
}
