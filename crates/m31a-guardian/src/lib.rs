//! `m31a-guardian`: Defensive security guard, prompt injection detector, and risk evaluator.
//!
//! Operates as an adversarial security reviewer that inspects proposed actions, detects prompt injection
//! or exfiltration attempts, and halts high-risk operations with interactive circuit breakers.

pub mod assessment;
pub mod async_scorer;
pub mod audit_trail;
pub mod blocking;
pub mod circuit_breaker;
pub mod context;
pub mod data_exfiltration_guard;
pub mod deadline;
pub mod decisions_classifier;
pub mod package_legitimacy;
pub mod prompt_injection_detector;
pub mod reviewer;
pub mod trusted_context;

pub use package_legitimacy::{
    Ecosystem, PackageCheckResult, PackageLegitimacyChecker, PackageSignals, PackageThresholds,
    PackageVerdict,
};

pub use assessment::{
    GuardianAssessmentOutcome, GuardianRiskLevel, GuardianUserAuthorization,
    StructuredGuardianAssessment, guardian_output_contract_prompt, guardian_output_schema,
    parse_guardian_assessment,
};
pub use async_scorer::{
    AsyncThreatScorer, AsyncThreatScorerConfig, ConversationThreatScore, ThreatVector,
    TurnRiskEvent,
};
pub use audit_trail::{
    AuditTrailSummary, DecisionSource, GuardianAuditRecord, GuardianAuditTrail,
    GuardianDecisionOutcome,
};
pub use blocking::ReviewBlocking;
pub use circuit_breaker::{
    AUTO_REVIEW_DENIAL_WINDOW_SIZE, GuardianRejectionCircuitBreaker,
    GuardianRejectionCircuitBreakerAction, GuardianRejectionCircuitBreakerPolicy,
    MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN, MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN,
    MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN, MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN,
};
pub use context::{
    DEFAULT_MAX_INSTRUCTION_TOKENS, GuardianContextBuilder, OMISSION_NOTICE,
    RETAINED_INSTRUCTIONS_FOOTER, RETAINED_INSTRUCTIONS_HEADER, RetainedInstructionItem,
    RetainedInstructionKind, RetentionPriority,
};
pub use data_exfiltration_guard::{
    DataExfiltrationGuard, ExfiltrationCheckResult, ExfiltrationFinding, SecretSanitizer,
    SecretType,
};
pub use deadline::{DEFAULT_REVIEW_TIMEOUT, ReviewDeadline};
pub use decisions_classifier::{
    DEADLINE as DECISIONS_DEADLINE, DEFAULT_DECISIONS_URL, DecisionsClassification, DecisionsError,
    DecisionsSampler, MAX_CONCURRENT_REQUESTS as MAX_DECISIONS_CONCURRENT_REQUESTS,
    MAX_IMAGE_BYTES as MAX_DECISIONS_IMAGE_BYTES,
    MAX_RESPONSE_BYTES as MAX_DECISIONS_RESPONSE_BYTES, SecurityRiskLevel,
};
pub use prompt_injection_detector::{
    InjectionCategory, PromptInjectionDetector, PromptInjectionFinding, PromptInjectionReport,
};
pub use reviewer::{
    GuardianAssessment, GuardianReviewer, PlannedAction, PlannedActionKind, RiskLevel,
};
pub use trusted_context::{TrustedRegistry, TrustedSkill, TrustedTool, TrustedToolSource};
