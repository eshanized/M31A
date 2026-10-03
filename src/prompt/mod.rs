//! PromptOS core module: versioned PromptContracts, MiniJinja rendering, composition, and catalog.

pub mod builtins;
pub mod catalog;
pub mod compiler;
pub mod composer;
pub mod context;
pub mod contract;
pub mod error;
pub mod model_profile;
pub mod parameter;
pub mod provenance;
pub mod reference;
pub mod renderer;
pub mod strategy;
pub mod v2;

pub use catalog::{
    CatalogEntry, InMemoryPromptCatalog, MAX_PROMPT_FILE_SIZE_BYTES, PromptCatalog,
    PromptContractMetadata, is_protected_contract_id,
};
pub use compiler::{CompilationOptions, DefaultPromptCompiler, EffectivePrompt, PromptCompiler};
pub use composer::{ArtifactEvidenceSection, PromptComposition, PromptLayerEntry, PromptLayerKind};
pub use context::{
    ArtifactEvidence, DurableStateContext, MissionStage, PromptBudget, PromptContext,
    QualityGateContext, RepoContext, RepoFileContext, TaskObjectiveContext, ToolOutputEvidence,
    TrustedPolicyContext, TrustedSystemContext, UserInputContext,
};
pub use contract::{
    MAX_RENDERED_BYTES, PromptContract, RUNTIME_SAFETY_INVARIANTS, deserialize_flexible_role,
    parse_role_flexible,
};
pub use error::PromptError;
pub use model_profile::{CapabilityRating, ModelProfile, StructuredOutputSupport};
pub use parameter::{MAX_PARAMETER_BYTES, PromptParameter};
pub use provenance::{
    PromptCompactionMetadata, PromptCompilationTrace, PromptInvocationProvenance,
    PromptLayerMetadata, PromptProvenance, PromptSourceKind,
};
pub use reference::{PromptPurpose, PromptReference};
pub use renderer::{RenderedPrompt, render_prompt};
pub use strategy::{PromptStrategy, resolve_strategy};
pub use v2::{
    AuthorityLevel, ClaimRecord, ClaimVerificationStatus, CompatibilityMetadata, EpistemicStatus,
    EvidenceRequirements, FailureBehavior, FailurePolicy, OutputContract, ProductionReachability,
    PromptInventoryAudit, PromptKind, PromptResponseKind, ReasoningDepth, ReasoningMode,
    ReasoningPolicy, VerificationRequirements,
};
