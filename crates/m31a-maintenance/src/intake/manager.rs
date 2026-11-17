//! Case Management and Signal Intake Orchestration (`m31a-maintenance::intake::manager`).
//!
//! Orchestrates signal deduplication, case grouping, lifecycle progression,
//! and backpressure limits across multiple providers.

use anyhow::Result;
use chrono::Utc;
use tracing::info;

use crate::domain::{MaintenanceCase, MaintenanceCategory, MaintenanceSignal};
use crate::storage::MaintenanceStore;

/// Result outcome of ingesting an individual signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionOutcome {
    /// Signal was duplicate and idempotently ignored.
    Duplicate { signal_id: String },
    /// Signal was grouped into an existing open case.
    CaseUpdated { case_id: String, signal_id: String },
    /// A new persistent maintenance case was created.
    CaseCreated { case_id: String, signal_id: String },
}

/// Central manager orchestrating intake and deduplication.
#[derive(Debug, Clone)]
pub struct SignalIntakeManager {
    store: MaintenanceStore,
}

impl SignalIntakeManager {
    pub fn new(store: MaintenanceStore) -> Self {
        Self { store }
    }

    /// Derives canonical grouping / deduplication key for case grouping.
    pub fn derive_case_grouping_key(signal: &MaintenanceSignal) -> String {
        match signal.category {
            MaintenanceCategory::BrokenCi => {
                // Group CI failures by observed commit or workflow
                if let Some(ref rev) = signal.observed_revision {
                    format!("ci_rev:{}", rev)
                } else if let Some(ref event_id) = signal.provider_event_id {
                    format!("ci_event:{}", event_id)
                } else {
                    format!("ci_fingerprint:{}", &signal.dedup_fingerprint[..16])
                }
            }
            MaintenanceCategory::DependencyUpdate => {
                // Group by target package name if available in metadata
                if let Some(pkg) = signal.metadata.get("package").and_then(|v| v.as_str()) {
                    format!("dep_pkg:{}", pkg)
                } else {
                    format!("dep_fingerprint:{}", &signal.dedup_fingerprint[..16])
                }
            }
            MaintenanceCategory::IssueBug => {
                if let Some(ref event_id) = signal.provider_event_id {
                    format!("issue:{}", event_id)
                } else {
                    format!("issue_fingerprint:{}", &signal.dedup_fingerprint[..16])
                }
            }
            MaintenanceCategory::SecurityAdvisory => {
                if let Some(ref event_id) = signal.provider_event_id {
                    format!("security:{}", event_id)
                } else {
                    format!("security_fingerprint:{}", &signal.dedup_fingerprint[..16])
                }
            }
            _ => format!(
                "cat_{}:{}",
                signal.category.as_str(),
                &signal.dedup_fingerprint[..16]
            ),
        }
    }

    /// Ingests a signal with atomic deduplication and case grouping.
    pub fn ingest_signal(&self, signal: &MaintenanceSignal) -> Result<IngestionOutcome> {
        // 1. Attempt to insert signal into storage (idempotent deduplication)
        let newly_inserted = self.store.record_signal(signal)?;
        if !newly_inserted {
            return Ok(IngestionOutcome::Duplicate {
                signal_id: signal.id.clone(),
            });
        }

        // 2. Derive case grouping key
        let dedup_key = Self::derive_case_grouping_key(signal);

        // 3. Search for existing open case
        if let Some(mut existing_case) = self
            .store
            .find_case_by_dedup_key(&signal.repo_id, &dedup_key)?
        {
            // Associate signal with existing case
            self.store
                .associate_signal_with_case(&existing_case.id, &signal.id)?;

            // Update case metadata and bump observed timestamp
            existing_case.last_observed_at_epoch_ms = Utc::now().timestamp_millis();
            existing_case.updated_at_epoch_ms = existing_case.last_observed_at_epoch_ms;

            // Escalate severity if incoming signal is more severe
            if signal.severity > existing_case.severity {
                existing_case.severity = signal.severity;
            }

            self.store.update_case(&existing_case)?;

            info!(
                case_id = %existing_case.id,
                signal_id = %signal.id,
                "Grouped maintenance signal into existing case"
            );

            Ok(IngestionOutcome::CaseUpdated {
                case_id: existing_case.id,
                signal_id: signal.id.clone(),
            })
        } else {
            // 4. Create new case
            let title = match signal.category {
                MaintenanceCategory::BrokenCi => {
                    format!("Remediate CI Failure in {}", signal.repo_id)
                }
                MaintenanceCategory::DependencyUpdate => {
                    format!(
                        "Review and Apply Dependency Maintenance in {}",
                        signal.repo_id
                    )
                }
                MaintenanceCategory::IssueBug => {
                    format!("Investigate Reported Issue in {}", signal.repo_id)
                }
                MaintenanceCategory::SecurityAdvisory => {
                    format!("Remediate Security Advisory in {}", signal.repo_id)
                }
                MaintenanceCategory::DocDrift => {
                    format!("Synchronize Documentation Drift in {}", signal.repo_id)
                }
                MaintenanceCategory::ApiMigration => {
                    format!("Execute API Migration in {}", signal.repo_id)
                }
                MaintenanceCategory::ArchitectureDrift => {
                    format!("Refactor Architectural Smell in {}", signal.repo_id)
                }
            };

            let problem_statement = format!(
                "Maintenance required from signal source '{}' (category: {}). Evidence: {:?}",
                signal.source_provider.as_str(),
                signal.category.as_str(),
                signal.metadata
            );

            let new_case = MaintenanceCase::new(
                &signal.repo_id,
                signal.category,
                title,
                problem_statement,
                dedup_key,
                signal.severity,
            );

            self.store.create_case(&new_case, Some(&signal.id))?;

            info!(
                case_id = %new_case.id,
                signal_id = %signal.id,
                "Created new persistent maintenance case"
            );

            Ok(IngestionOutcome::CaseCreated {
                case_id: new_case.id,
                signal_id: signal.id.clone(),
            })
        }
    }
}

pub type CaseManager = SignalIntakeManager;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SignalProvider, SignalSeverity, SignalTrustStatus};
    use m31a_storage::{BUILTIN_MIGRATIONS, MigrationEngine};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    fn setup_test_manager() -> SignalIntakeManager {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        let store = MaintenanceStore::new(Arc::new(Mutex::new(conn)));
        SignalIntakeManager::new(store)
    }

    #[test]
    fn test_ingest_duplicate_and_grouping() {
        let manager = setup_test_manager();

        let sig1 = MaintenanceSignal::new(
            "repo_1",
            SignalProvider::GitHub,
            MaintenanceCategory::BrokenCi,
            Some("run_1".to_string()),
            None,
            1000,
            Some("commit_aaa".to_string()),
            SignalSeverity::High,
            "cargo test failure in foo",
            SignalTrustStatus::Verified,
            serde_json::json!({}),
        );

        // First signal creates a case
        let outcome1 = manager.ingest_signal(&sig1).unwrap();
        let case_id = match outcome1 {
            IngestionOutcome::CaseCreated { case_id, .. } => case_id,
            _ => panic!("Expected CaseCreated"),
        };

        // Identical signal is duplicate
        let outcome_dup = manager.ingest_signal(&sig1).unwrap();
        assert!(matches!(outcome_dup, IngestionOutcome::Duplicate { .. }));

        // Second distinct signal for same commit is grouped into existing case!
        let sig2 = MaintenanceSignal::new(
            "repo_1",
            SignalProvider::GitHub,
            MaintenanceCategory::BrokenCi,
            Some("run_2".to_string()),
            None,
            2000,
            Some("commit_aaa".to_string()),
            SignalSeverity::Critical,
            "cargo build failure in bar",
            SignalTrustStatus::Verified,
            serde_json::json!({}),
        );

        let outcome2 = manager.ingest_signal(&sig2).unwrap();
        match outcome2 {
            IngestionOutcome::CaseUpdated {
                case_id: grouped_case_id,
                ..
            } => {
                assert_eq!(grouped_case_id, case_id);
            }
            _ => panic!("Expected CaseUpdated"),
        }
    }
}
