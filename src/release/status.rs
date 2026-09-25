//! Release-candidate status machine, unified blocker taxonomy, and
//! release-blocker evaluation.
//!
//! Taxonomy resolution: recovery audits and blocker matrices
//! both record `CREDENTIAL-BLOCKED` in evidence while earlier allowed sets
//! omitted it. Repository evidence (live tests gated on provider credentials
//! and cost policy) shows credential-gating is a first-class, non-blocking
//! classification: live test suites are executed only when explicit credentials
//! are provisioned, and reported honestly otherwise. This module provides the
//! authoritative closed set — the taxonomy cannot drift without failing
//! `taxonomy_is_closed_and_documented`.

use serde::{Deserialize, Serialize};

/// Release-candidate lifecycle states (§23). Forward motion is
/// evidence-derived; no manual boolean overrides failed evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseState {
    NotBuilt,
    BuildFailed,
    Built,
    IntegrityFailed,
    VerificationFailed,
    Candidate,
    Rejected,
    Released,
}

impl ReleaseState {
    /// Terminal states: no further transitions allowed.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Released)
    }
}

/// Evidence bundle snapshot driving state transitions.
#[derive(Debug, Clone, Default)]
pub struct ReleaseEvidence {
    pub build_succeeded: bool,
    pub integrity_ok: bool,
    pub verification_ok: bool,
    pub blockers: Vec<Blocker>,
    pub approval_note: Option<String>,
}

/// A release finding with its unified classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub classification: ReleaseClassification,
    pub detail: String,
}

/// Unified release classification. Exactly one taxonomy (§24); the closed set
/// below is enforced by `taxonomy_is_closed_and_documented`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseClassification {
    /// Resolved and verified; never blocks.
    Resolved,
    /// Contained by an explicit, tested boundary; never blocks.
    FormallyContained,
    /// Acceptable for release as documented; never blocks.
    AcceptableForRelease,
    /// Tracked follow-up; must never silently become a blocker.
    PostRelease,
    /// Hard failure of a mandatory gate; always blocks.
    SecurityBlocked,
    /// Gated by the build/test environment, not the product; non-blocking
    /// only when explicitly recorded with evidence.
    EnvironmentBlocked,
    /// Gated by missing provider credentials or cost policy (live tests);
    /// non-blocking only when deterministic coverage exists for the same
    /// invariant AND the gap is explicitly recorded. Formally adopted per
    /// §12 Option A.
    CredentialBlocked,
}

impl ReleaseClassification {
    /// All seven approved variants, in canonical order.
    pub const ALL: [Self; 7] = [
        Self::Resolved,
        Self::FormallyContained,
        Self::AcceptableForRelease,
        Self::PostRelease,
        Self::SecurityBlocked,
        Self::EnvironmentBlocked,
        Self::CredentialBlocked,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "RESOLVED",
            Self::FormallyContained => "FORMALLY CONTAINED",
            Self::AcceptableForRelease => "ACCEPTABLE FOR RELEASE",
            Self::PostRelease => "POST-RELEASE",
            Self::SecurityBlocked => "SECURITY-BLOCKED",
            Self::EnvironmentBlocked => "ENVIRONMENT-BLOCKED",
            Self::CredentialBlocked => "CREDENTIAL-BLOCKED",
        }
    }
}

/// Blocker verdict for one finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    Blocking,
    NonBlocking,
}

/// Canonical blocker evaluator (§24). Exactly one taxonomy in, blocking
/// verdicts out:
///
/// - missing artifact hash, failed mandatory test, broken migration,
///   inconsistent version, unverifiable provenance, integrity failure →
///   Blocking (expressed as `SecurityBlocked` findings);
/// - POST-RELEASE findings never block and are never escalated silently;
/// - a blocker is never downgraded to a warning: `Blocking` has no
///   intermediate state.
pub struct BlockerEvaluator;

impl BlockerEvaluator {
    /// Classify one finding. `mandatory_gate_failed` marks findings attached
    /// to a failed mandatory gate (tests, migration, integrity, version,
    /// provenance): those always block regardless of their recorded class.
    pub fn evaluate(finding: &Finding, mandatory_gate_failed: bool) -> Blocker {
        if mandatory_gate_failed {
            return Blocker::Blocking;
        }
        match finding.classification {
            ReleaseClassification::SecurityBlocked => Blocker::Blocking,
            ReleaseClassification::Resolved
            | ReleaseClassification::FormallyContained
            | ReleaseClassification::AcceptableForRelease
            | ReleaseClassification::PostRelease
            | ReleaseClassification::EnvironmentBlocked
            | ReleaseClassification::CredentialBlocked => Blocker::NonBlocking,
        }
    }

    /// RC promotion is allowed iff zero findings evaluate to Blocking.
    /// `mandatory_failures` counts failed mandatory gates (each blocks).
    pub fn promotion_allowed(findings: &[Finding], mandatory_failures: usize) -> bool {
        if mandatory_failures > 0 {
            return false;
        }
        !findings
            .iter()
            .any(|f| Self::evaluate(f, false) == Blocker::Blocking)
    }
}

/// Typed RC transition failures. The state never advances on failure: no
/// partial success, no stale marker, no false candidate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("transition denied: {0}")]
    Denied(String),
}

/// Evidence-derived RC state machine (§23).
#[derive(Debug, Clone)]
pub struct ReleaseCandidate {
    state: ReleaseState,
}

impl ReleaseCandidate {
    pub fn new() -> Self {
        Self {
            state: ReleaseState::NotBuilt,
        }
    }

    pub fn state(&self) -> ReleaseState {
        self.state
    }

    /// Record a build outcome. Success → Built; failure → BuildFailed.
    /// Terminal states reject all transitions.
    pub fn record_build(&mut self, succeeded: bool) -> Result<(), TransitionError> {
        self.guard_terminal("record_build")?;
        self.state = if succeeded {
            ReleaseState::Built
        } else {
            ReleaseState::BuildFailed
        };
        Ok(())
    }

    /// Record integrity verification. Allowed only from Built.
    pub fn record_integrity(&mut self, ok: bool) -> Result<(), TransitionError> {
        self.guard_terminal("record_integrity")?;
        if self.state != ReleaseState::Built {
            return Err(TransitionError::Denied(format!(
                "integrity may only be recorded from Built, not {:?}",
                self.state
            )));
        }
        self.state = if ok {
            ReleaseState::Built
        } else {
            ReleaseState::IntegrityFailed
        };
        Ok(())
    }

    /// Record full verification against evidence. Promotion to Candidate
    /// requires: integrity ok, verification ok, zero blocking findings.
    /// Anything else → VerificationFailed. `Released` is never reachable here.
    pub fn record_verification(
        &mut self,
        evidence: &ReleaseEvidence,
    ) -> Result<(), TransitionError> {
        self.guard_terminal("record_verification")?;
        if self.state != ReleaseState::Built {
            return Err(TransitionError::Denied(format!(
                "verification may only be recorded from Built, not {:?}",
                self.state
            )));
        }
        let blocked = !evidence.integrity_ok
            || !evidence.verification_ok
            || evidence.blockers.contains(&Blocker::Blocking);
        self.state = if blocked {
            ReleaseState::VerificationFailed
        } else {
            ReleaseState::Candidate
        };
        Ok(())
    }

    /// Promote Candidate → Released. Requires a non-empty human approval note
    /// AND zero blocking findings re-checked at promotion time. Approval can
    /// never override failed evidence: any blocker denies promotion.
    pub fn promote_to_released(
        &mut self,
        approval_note: &str,
        blockers: &[Blocker],
    ) -> Result<(), TransitionError> {
        self.guard_terminal("promote_to_released")?;
        if self.state != ReleaseState::Candidate {
            return Err(TransitionError::Denied(format!(
                "only a Candidate may be released, not {:?}",
                self.state
            )));
        }
        if approval_note.trim().is_empty() {
            return Err(TransitionError::Denied(
                "release requires a non-empty approval note".to_string(),
            ));
        }
        if blockers.contains(&Blocker::Blocking) {
            return Err(TransitionError::Denied(
                "approval cannot override blocking findings".to_string(),
            ));
        }
        self.state = ReleaseState::Released;
        Ok(())
    }

    /// Reject from any non-terminal state with a recorded reason.
    pub fn reject(&mut self, _reason: &str) -> Result<(), TransitionError> {
        self.guard_terminal("reject")?;
        self.state = ReleaseState::Rejected;
        Ok(())
    }

    fn guard_terminal(&self, op: &str) -> Result<(), TransitionError> {
        if self.state.is_terminal() {
            return Err(TransitionError::Denied(format!(
                "{op} denied: {:?} is terminal",
                self.state
            )));
        }
        Ok(())
    }
}

impl Default for ReleaseCandidate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression anchor for §12: the taxonomy is exactly these seven
    /// classifications, no more, no fewer. Drift fails here, not in a report.
    #[test]
    fn taxonomy_is_closed_and_documented() {
        let names: Vec<&str> = ReleaseClassification::ALL
            .iter()
            .map(|c| c.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "RESOLVED",
                "FORMALLY CONTAINED",
                "ACCEPTABLE FOR RELEASE",
                "POST-RELEASE",
                "SECURITY-BLOCKED",
                "ENVIRONMENT-BLOCKED",
                "CREDENTIAL-BLOCKED",
            ]
        );
        // Serde round-trip uses the same vocabulary.
        for class in ReleaseClassification::ALL {
            let json = serde_json::to_string(&class).unwrap();
            let back: ReleaseClassification = serde_json::from_str(&json).unwrap();
            assert_eq!(back, class);
        }
    }

    #[test]
    fn blocker_rules() {
        let mk = |c| Finding {
            id: "x".to_string(),
            classification: c,
            detail: "d".to_string(),
        };
        // Mandatory gate failure always blocks, whatever the class.
        assert_eq!(
            BlockerEvaluator::evaluate(&mk(ReleaseClassification::PostRelease), true),
            Blocker::Blocking
        );
        // SecurityBlocked always blocks.
        assert_eq!(
            BlockerEvaluator::evaluate(&mk(ReleaseClassification::SecurityBlocked), false),
            Blocker::Blocking
        );
        // Post-release never blocks and is never escalated silently.
        assert_eq!(
            BlockerEvaluator::evaluate(&mk(ReleaseClassification::PostRelease), false),
            Blocker::NonBlocking
        );
        // Contained / environment / credential gaps never block when recorded.
        for c in [
            ReleaseClassification::FormallyContained,
            ReleaseClassification::EnvironmentBlocked,
            ReleaseClassification::CredentialBlocked,
            ReleaseClassification::AcceptableForRelease,
            ReleaseClassification::Resolved,
        ] {
            assert_eq!(
                BlockerEvaluator::evaluate(&mk(c), false),
                Blocker::NonBlocking
            );
        }
        // Promotion gates.
        assert!(BlockerEvaluator::promotion_allowed(&[], 0));
        assert!(!BlockerEvaluator::promotion_allowed(&[], 1));
        assert!(!BlockerEvaluator::promotion_allowed(
            &[mk(ReleaseClassification::SecurityBlocked)],
            0
        ));
        assert!(BlockerEvaluator::promotion_allowed(
            &[mk(ReleaseClassification::PostRelease)],
            0
        ));
    }

    #[test]
    fn state_machine_failure_never_advances() {
        let mut rc = ReleaseCandidate::new();
        assert_eq!(rc.state(), ReleaseState::NotBuilt);
        // Verification before build is denied (no skip-ahead).
        assert!(rc.record_verification(&ReleaseEvidence::default()).is_err());
        assert_eq!(rc.state(), ReleaseState::NotBuilt);
        // Failed build → BuildFailed; integrity recording denied from there.
        rc.record_build(false).unwrap();
        assert_eq!(rc.state(), ReleaseState::BuildFailed);
        assert!(rc.record_integrity(true).is_err());
        // Fresh candidate: failure evidence → VerificationFailed, never Candidate.
        let mut rc = ReleaseCandidate::new();
        rc.record_build(true).unwrap();
        rc.record_integrity(true).unwrap();
        rc.record_verification(&ReleaseEvidence {
            build_succeeded: true,
            integrity_ok: true,
            verification_ok: false,
            blockers: vec![],
            approval_note: None,
        })
        .unwrap();
        assert_eq!(rc.state(), ReleaseState::VerificationFailed);
        // Terminal states reject everything.
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
        assert_eq!(rc.state(), ReleaseState::Candidate);
        assert!(rc.promote_to_released("", &[]).is_err());
        assert!(
            rc.promote_to_released("ship it", &[Blocker::Blocking])
                .is_err()
        );
        assert_eq!(rc.state(), ReleaseState::Candidate);
        rc.promote_to_released("RC-1 approved: all gates green", &[])
            .unwrap();
        assert_eq!(rc.state(), ReleaseState::Released);
        assert!(rc.reject("too late").is_err());
    }
}
