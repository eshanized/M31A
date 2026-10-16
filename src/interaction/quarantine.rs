//! Shared runtime-authoritative quarantine projection query.
//!
//! Both interactive and terminal command surfaces consume this projection so
//! status, IDs, naming, and unavailable-reason semantics cannot drift.

use crate::interaction::events::QuarantineItemRecord;

/// Query the canonical artifact inventory and return only quarantined records.
///
/// The artifact service currently does not persist a human-readable containment
/// reason. The projection therefore reports that limitation instead of
/// inventing a reason from producer metadata or a path.
pub async fn query_quarantined_artifacts(
    runtime: &crate::runtime::AppRuntime,
) -> Result<Vec<QuarantineItemRecord>, String> {
    let records = runtime
        .artifact_service()
        .list_all()
        .await
        .map_err(|error| format!("Failed to query artifacts: {error}"))?;

    Ok(records
        .into_iter()
        .filter(|record| {
            record.status
                == crate::persistence::artifacts::service::ArtifactStatus::Quarantined
        })
        .map(|record| QuarantineItemRecord {
            id: record.id.to_string(),
            name: record.logical_path.display().to_string(),
            reason: "reason unavailable".to_string(),
            quarantined_at: Some(record.created_at),
        })
        .collect())
}
