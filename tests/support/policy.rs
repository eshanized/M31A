//! Explicit allow-all policy gate for isolated tests (test-only).
//!
//! Relocated out of production runtime modules (`src/kernel/seams/policy.rs`).

use async_trait::async_trait;
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};

/// Explicit test-only allow-all policy gate.
/// Never use on production paths: missing authoritative policy must fail closed.
#[derive(Debug, Clone, Default)]
pub struct AllowAllTestPolicy;

#[async_trait]
impl PolicyGate for AllowAllTestPolicy {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Allow)
    }
}
