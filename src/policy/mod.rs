//! Policy engine, multi-layer evaluation hierarchy, and authorization enforcement (POL-01..POL-06).

pub mod approval;
pub mod audit;
pub mod defaults;
pub mod destination;
pub mod effective;
pub mod file;
pub mod layers;
pub mod matcher;
pub mod rule;

pub use approval::{
    ApprovalAction, ApprovalChannel, ApprovalCoordinator, ApprovalError, ApprovalExplanationPacket,
    ApprovalRequest, ApprovalRequestState, ApprovalResolutionScope, PolicyGrant, PolicyGrantStore,
    format_explanation_packet,
};
pub use audit::{DurablePolicyAuditor, PolicyDecisionAuditRow, hash_normalized_arguments};
pub use defaults::{built_in_safety_rules, developer_defaults};
pub use destination::{NetworkDestinationPolicy, NetworkSecurityError};
pub use effective::{EffectivePolicy, EffectivePolicyBuilder, PolicyDecisionRecord, PolicyLoadError};
pub use file::PolicyFileError;
pub use layers::{PolicyLayer, merge_preliminary_decision};
pub use matcher::{PolicyEvaluationContext, PolicyMatcher, canonicalize_and_validate_path};
pub use rule::{CURRENT_POLICY_SCHEMA_VERSION, PolicyDocument, PolicyRule};
