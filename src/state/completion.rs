//! Authority classification (D3): this gate is a
//! NON-AUTHORITATIVE local control-flow projection. It evaluates six
//! in-memory booleans for early local decisions (intake gating, report
//! scaffolding) and MUST NEVER emit final completion. The durable,
//! authoritative completion decision belongs SOLELY to
//! `EvidenceCompletionGate` (`verification/gate.rs`), which decides over
//! persisted verification evidence and records the decision in
//! `completion_gate_decisions` / the mission-task repositories.
//!
//! ```text
//! EvidenceCompletionGate → completion decision → mission/task repository
//!        (authoritative; persists; may veto)
//! state::CompletionGate → local boolean projection
//!        (advisory only; never completes a mission)
//! ```
//!
//! Any caller treating `CompletionGate::can_complete` / `evaluate` as final
//! completion is a defect. Mission/task/session code must consult the
//! evidence gate; integrity tests prove that a satisfied local context alone
//! cannot complete anything durable.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Reasons why a mission cannot be marked completed.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum CompletionGateError {
    #[error("Not all mandatory tasks have succeeded")]
    UnresolvedTasks,

    #[error("Mandatory requirements have not been verified with evidence")]
    UnverifiedRequirements,

    #[error("Required review has not passed")]
    ReviewNotPassed,

    #[error("Fatal policy issues remain unresolved")]
    FatalPolicyState,

    #[error("Integration policy checks have not been satisfied")]
    IntegrationPolicyFailed,

    #[error("Final completion report has not been persisted")]
    MissingCompletionReport,
}

/// Context containing evidence and criteria needed to evaluate mission completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompletionContext {
    pub all_mandatory_tasks_succeeded: bool,
    pub mandatory_requirements_verified: bool,
    pub required_review_passed: bool,
    pub no_fatal_policy_issues: bool,
    pub integration_policy_satisfied: bool,
    pub final_report_persisted: bool,
}

impl CompletionContext {
    /// Create a context where all 6 criteria are satisfied.
    pub fn all_satisfied() -> Self {
        Self {
            all_mandatory_tasks_succeeded: true,
            mandatory_requirements_verified: true,
            required_review_passed: true,
            no_fatal_policy_issues: true,
            integration_policy_satisfied: true,
            final_report_persisted: true,
        }
    }
}

/// Evaluator enforcing deterministic mission completion gates (MSN-03).
///
/// Authority classification (D3): NON-AUTHORITATIVE. Local control-flow projection only.
/// Final completion authority is `EvidenceCompletionGate`. This evaluator
/// never persists, never emits completion events, and never overrides the
/// evidence gate. See module docs.
pub struct CompletionGate;

impl CompletionGate {
    /// Authority classification (D3): always `false`. The local projection is never the
    /// authoritative completion decider; call sites can assert this.
    pub const fn is_authoritative() -> bool {
        false
    }

    /// Evaluate all mandatory completion criteria deterministically.
    ///
    /// Per MSN-03: Accumulates all failing conditions rather than short-circuiting on the first,
    /// providing comprehensive diagnostics to operators.
    pub fn evaluate(context: &CompletionContext) -> Result<(), Vec<CompletionGateError>> {
        let mut errors = Vec::new();

        if !context.all_mandatory_tasks_succeeded {
            errors.push(CompletionGateError::UnresolvedTasks);
        }
        if !context.mandatory_requirements_verified {
            errors.push(CompletionGateError::UnverifiedRequirements);
        }
        if !context.required_review_passed {
            errors.push(CompletionGateError::ReviewNotPassed);
        }
        if !context.no_fatal_policy_issues {
            errors.push(CompletionGateError::FatalPolicyState);
        }
        if !context.integration_policy_satisfied {
            errors.push(CompletionGateError::IntegrationPolicyFailed);
        }
        if !context.final_report_persisted {
            errors.push(CompletionGateError::MissingCompletionReport);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check if a mission can transition to Completed based on the given context.
    pub fn can_complete(context: &CompletionContext) -> bool {
        Self::evaluate(context).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_gate_all_satisfied() {
        let ctx = CompletionContext::all_satisfied();
        assert!(CompletionGate::can_complete(&ctx));
        assert!(CompletionGate::evaluate(&ctx).is_ok());
    }

    #[test]
    fn test_completion_gate_empty_context_accumulates_all_errors() {
        let ctx = CompletionContext::default();
        let errors = CompletionGate::evaluate(&ctx).unwrap_err();
        assert_eq!(errors.len(), 6);
        assert!(errors.contains(&CompletionGateError::UnresolvedTasks));
        assert!(errors.contains(&CompletionGateError::UnverifiedRequirements));
        assert!(errors.contains(&CompletionGateError::ReviewNotPassed));
        assert!(errors.contains(&CompletionGateError::FatalPolicyState));
        assert!(errors.contains(&CompletionGateError::IntegrationPolicyFailed));
        assert!(errors.contains(&CompletionGateError::MissingCompletionReport));
    }

    #[test]
    fn test_completion_gate_single_failure() {
        let mut ctx = CompletionContext::all_satisfied();
        ctx.final_report_persisted = false;

        assert!(!CompletionGate::can_complete(&ctx));
        let errors = CompletionGate::evaluate(&ctx).unwrap_err();
        assert_eq!(errors, vec![CompletionGateError::MissingCompletionReport]);
    }
}
