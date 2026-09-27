//! Tier 5 Diff & Invariant Verification Runner (VER-01, VER-03).
//!
//! Reconciles workspace state against `RepositoryBaseline` using `repo::drift::detect_drift`,
//! enforcing authorized mutation scopes and preventing unexpected workspace drift.

use async_trait::async_trait;
use std::collections::BTreeSet;
use std::path::Path;

use super::VerificationRunner;
use crate::ids::{MissionId, TaskId};
use crate::repo::drift::{RepositoryBaseline, detect_drift};
use crate::verification::types::{CheckStatus, CheckTier, VerificationCheck};

#[derive(Debug, Clone, Default)]
pub struct DiffInvariantsRunner {
    pub baseline: Option<RepositoryBaseline>,
    pub authorized_mutations: BTreeSet<String>,
    pub simulated_result: Option<(CheckStatus, String)>,
}

impl DiffInvariantsRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_baseline(mut self, baseline: RepositoryBaseline) -> Self {
        self.baseline = Some(baseline);
        self
    }

    pub fn with_authorized_mutations(
        mut self,
        mutations: impl IntoIterator<Item = String>,
    ) -> Self {
        self.authorized_mutations = mutations.into_iter().collect();
        self
    }

    pub fn with_simulated(mut self, status: CheckStatus, summary: impl Into<String>) -> Self {
        self.simulated_result = Some((status, summary.into()));
        self
    }
}

#[async_trait]
impl VerificationRunner for DiffInvariantsRunner {
    async fn execute(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
    ) -> Result<VerificationCheck, String> {
        if let Some((ref status, ref summary)) = self.simulated_result {
            return if status.is_passed() {
                Ok(VerificationCheck::passed(
                    mission_id,
                    task_id,
                    CheckTier::DiffInvariants,
                    "diff_invariants_check",
                    format!("workspace_root={}", workspace_root.display()),
                    None,
                    summary.clone(),
                    snapshot_hash,
                ))
            } else {
                Ok(VerificationCheck::failed(
                    mission_id,
                    task_id,
                    CheckTier::DiffInvariants,
                    "diff_invariants_check",
                    format!("workspace_root={}", workspace_root.display()),
                    None,
                    summary.clone(),
                    Some("RepositoryState".to_string()),
                    snapshot_hash,
                ))
            };
        }

        let current_baseline =
            RepositoryBaseline::capture(workspace_root, mission_id, Some(task_id), None)
                .map_err(|e| format!("Failed to capture workspace snapshot: {}", e))?;

        if let Some(ref baseline) = self.baseline {
            let report = detect_drift(
                baseline,
                &current_baseline.file_hashes,
                &self.authorized_mutations,
            );

            if !report.has_drift {
                Ok(VerificationCheck::passed(
                    mission_id,
                    task_id,
                    CheckTier::DiffInvariants,
                    "repo_drift_detector",
                    format!("authorized_mutations={}", report.authorized_mutation_count),
                    None,
                    "Workspace modifications conform strictly to authorized mutation scope",
                    snapshot_hash,
                ))
            } else {
                let summary = format!(
                    "Unauthorized workspace drift detected: {} unexpected additions, {} modifications, {} deletions",
                    report.unexpected_additions.len(),
                    report.unexpected_modifications.len(),
                    report.unexpected_deletions.len()
                );
                Ok(VerificationCheck::failed(
                    mission_id,
                    task_id,
                    CheckTier::DiffInvariants,
                    "repo_drift_detector",
                    format!(
                        "additions=[{}], modifications=[{}]",
                        report.unexpected_additions.join(", "),
                        report.unexpected_modifications.join(", ")
                    ),
                    None,
                    summary,
                    Some("RepositoryState".to_string()),
                    snapshot_hash,
                ))
            }
        } else {
            // No prior baseline to compare against; pass baseline initialization
            Ok(VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::DiffInvariants,
                "repo_drift_detector",
                format!("tracked_files={}", current_baseline.file_hashes.len()),
                None,
                "Baseline captured cleanly with 0 unauthorized drift",
                snapshot_hash,
            ))
        }
    }
}
