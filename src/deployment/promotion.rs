//! Production promotion: development build → verified RC → production.
//!
//! Reuses the existing `src/release/*` state machine (`ReleaseCandidate`,
//! `ReleaseEvidence`, `Blocker`, `BlockerEvaluator`). This module adds only
//! the deployment-channel layer: a development artifact can never claim
//! production status by itself; promotion requires a Candidate with full
//! evidence, integrity, gates, and human approval.

use super::channel::DeploymentChannel;
use super::context::DeploymentContext;
use crate::release::status::{Blocker, ReleaseCandidate, ReleaseEvidence};

/// Typed promotion failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromotionError {
    #[error("promotion denied: {0}")]
    Denied(String),
}

/// Evaluate whether `candidate` + `evidence` + `approval` authorizes stamping
/// a production artifact for `ctx` (normally the release pipeline's context).
///
/// Requirements (all must hold):
/// - target channel is Production (development builds never self-promote);
/// - source was built from a clean tree (`dirty == false`);
/// - build succeeded, integrity ok, verification ok;
/// - zero blocking findings;
/// - non-empty approval note.
pub fn evaluate_promotion(
    target_channel: DeploymentChannel,
    source: &DeploymentContext,
    candidate: &ReleaseCandidate,
    evidence: &ReleaseEvidence,
    approval_note: &str,
) -> Result<(), PromotionError> {
    if target_channel != DeploymentChannel::Production {
        return Err(PromotionError::Denied(
            "promotion targets production only".to_string(),
        ));
    }
    if source.is_development() {
        // Development provenance is recorded, but status promotion still
        // requires the full RC path below — a dev build alone never implies
        // production. (No early Ok here; fall through to evidence gates.)
    }
    if source.dirty {
        return Err(PromotionError::Denied(
            "refusing production promotion from a dirty source tree".to_string(),
        ));
    }
    if !evidence.build_succeeded {
        return Err(PromotionError::Denied("build did not succeed".to_string()));
    }
    if !evidence.integrity_ok {
        return Err(PromotionError::Denied(
            "integrity verification failed".to_string(),
        ));
    }
    if !evidence.verification_ok {
        return Err(PromotionError::Denied(
            "verification gates failed".to_string(),
        ));
    }
    if evidence.blockers.contains(&Blocker::Blocking) {
        return Err(PromotionError::Denied(
            "blocking release findings present".to_string(),
        ));
    }
    if approval_note.trim().is_empty() {
        return Err(PromotionError::Denied(
            "production promotion requires a non-empty approval note".to_string(),
        ));
    }
    if candidate.state() != crate::release::status::ReleaseState::Candidate {
        return Err(PromotionError::Denied(format!(
            "only a Candidate may be promoted, not {:?}",
            candidate.state()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> ReleaseCandidate {
        let mut rc = ReleaseCandidate::new();
        rc.record_build(true).unwrap();
        rc.record_integrity(true).unwrap();
        rc.record_verification(&ReleaseEvidence {
            build_succeeded: true,
            integrity_ok: true,
            verification_ok: true,
            blockers: vec![],
            approval_note: None,
        })
        .unwrap();
        rc
    }

    fn ctx(channel: DeploymentChannel, dirty: bool) -> DeploymentContext {
        DeploymentContext::from_parts(
            channel,
            "0.1.1",
            "abc",
            "master",
            "2026-01-01T00:00:00Z",
            "x86_64-unknown-linux-gnu",
            dirty,
        )
    }

    fn evidence() -> ReleaseEvidence {
        ReleaseEvidence {
            build_succeeded: true,
            integrity_ok: true,
            verification_ok: true,
            blockers: vec![],
            approval_note: Some("ok".to_string()),
        }
    }

    #[test]
    fn dev_build_cannot_self_promote_without_evidence() {
        let rc = ReleaseCandidate::new(); // NotBuilt, not Candidate
        let r = evaluate_promotion(
            DeploymentChannel::Production,
            &ctx(DeploymentChannel::Development, false),
            &rc,
            &evidence(),
            "approved",
        );
        assert!(r.is_err());
    }

    #[test]
    fn full_evidence_promotes() {
        let rc = candidate();
        assert!(
            evaluate_promotion(
                DeploymentChannel::Production,
                &ctx(DeploymentChannel::Development, false),
                &rc,
                &evidence(),
                "RC-1 approved: all gates green"
            )
            .is_ok()
        );
    }

    #[test]
    fn dirty_or_blocked_denied() {
        let rc = candidate();
        assert!(
            evaluate_promotion(
                DeploymentChannel::Production,
                &ctx(DeploymentChannel::Development, true),
                &rc,
                &evidence(),
                "approved"
            )
            .is_err()
        );
        let mut ev = evidence();
        ev.blockers = vec![Blocker::Blocking];
        assert!(
            evaluate_promotion(
                DeploymentChannel::Production,
                &ctx(DeploymentChannel::Production, false),
                &rc,
                &ev,
                "approved"
            )
            .is_err()
        );
        assert!(
            evaluate_promotion(
                DeploymentChannel::Production,
                &ctx(DeploymentChannel::Production, false),
                &rc,
                &evidence(),
                "  "
            )
            .is_err()
        );
    }
}
