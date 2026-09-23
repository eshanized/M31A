//! Repository-Aware Memory Reconciler.
//!
//! Subordinates memory claims to repository reality:
//! - Validates file existence and content hashes against actual workspace filesystem.
//! - Marks assumptions invalidated when target files drift or symbols disappear.
//! - Marks verification records stale when verified source files are mutated.
//! - Ensures repository reality ALWAYS wins over stale memory.

use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

use crate::ids::MissionId;
use crate::memory::repository::EngineeringMemoryStore;
use crate::memory::types::{AssumptionStatus, VerificationValidity};

/// Diagnostic report resulting from memory reconciliation against workspace reality.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryReconciliationReport {
    pub invalidated_assumptions: Vec<(Uuid, String)>,
    pub stale_verifications: Vec<(Uuid, String)>,
    pub checked_count: usize,
}

/// Reconciler verifying memory entries against repository files and hashes.
pub struct MemoryReconciler<'a, S: EngineeringMemoryStore> {
    store: &'a S,
    workspace_root: &'a Path,
}

impl<'a, S: EngineeringMemoryStore> MemoryReconciler<'a, S> {
    pub fn new(store: &'a S, workspace_root: &'a Path) -> Self {
        Self {
            store,
            workspace_root,
        }
    }

    /// Reconcile all active assumptions and valid verifications for a mission.
    pub async fn reconcile_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<MemoryReconciliationReport, sqlx::Error> {
        let mut report = MemoryReconciliationReport::default();

        // 1. Reconcile Active Assumptions
        let assumptions = self
            .store
            .list_assumptions(mission_id, None, Some(AssumptionStatus::Active))
            .await?;

        for asm in assumptions {
            report.checked_count += 1;
            if let Some(ref rel_path) = asm.target_file {
                let full_path = self.workspace_root.join(rel_path.trim_start_matches("./"));
                if !full_path.exists() {
                    let reason =
                        format!("Target file '{}' no longer exists in repository", rel_path);
                    self.store
                        .update_assumption_status(
                            asm.id,
                            AssumptionStatus::Invalidated,
                            None,
                            Some(&reason),
                        )
                        .await?;
                    report.invalidated_assumptions.push((asm.id, reason));
                    continue;
                }

                if let (Some(expected), Ok(bytes)) =
                    (&asm.expected_hash, tokio::fs::read(&full_path).await)
                {
                    let mut hasher = Sha256::new();
                    hasher.update(&bytes);
                    let actual = format!("{:x}", hasher.finalize());
                    if &actual != expected {
                        let reason = format!(
                            "File '{}' modified: expected hash {}, found {}",
                            rel_path, expected, actual
                        );
                        self.store
                            .update_assumption_status(
                                asm.id,
                                AssumptionStatus::Invalidated,
                                None,
                                Some(&reason),
                            )
                            .await?;
                        report.invalidated_assumptions.push((asm.id, reason));
                    }
                }
            }
        }

        // 2. Reconcile Verification Records
        let verifications = self
            .store
            .list_verification_records(mission_id, None)
            .await?;

        for v in verifications {
            if v.validity_status != VerificationValidity::Valid {
                continue;
            }
            report.checked_count += 1;

            let mut is_stale = false;
            let mut stale_reason = String::new();

            for f in &v.files_verified {
                let full_path = self.workspace_root.join(f.path.trim_start_matches("./"));
                if !full_path.exists() {
                    is_stale = true;
                    stale_reason = format!("Verified file '{}' was deleted", f.path);
                    break;
                }

                if let Ok(bytes) = tokio::fs::read(&full_path).await {
                    let mut hasher = Sha256::new();
                    hasher.update(&bytes);
                    let actual = format!("{:x}", hasher.finalize());
                    if actual != f.hash {
                        is_stale = true;
                        stale_reason =
                            format!("Verified file '{}' changed since verification", f.path);
                        break;
                    }
                } else {
                    is_stale = true;
                    stale_reason = format!("Failed to read verified file '{}'", f.path);
                    break;
                }
            }

            if is_stale {
                self.store
                    .invalidate_verification(v.id, VerificationValidity::StaleDueToDrift)
                    .await?;
                report.stale_verifications.push((v.id, stale_reason));
            }
        }

        Ok(report)
    }
}
