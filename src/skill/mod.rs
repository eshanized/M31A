//! Skills Architecture & Reusable Capabilities Engine (SKL-01, SKL-02, SKL-03).

pub mod compiler;
pub mod discovery;
pub mod manifest;
pub mod registry;
pub mod verification;

pub use compiler::{CompiledSubDag, CompiledSubTask, SkillCompiler};
pub use discovery::{SkillDiscovery, SkillOriginTier, SkillPackage, builtin_skills};
pub use manifest::{
    CURRENT_SKILL_SCHEMA_VERSION, PluginManifest, PluginValidationError, SkillExecutionConfig,
    SkillExecutionMode, SkillManifest, SkillProcedure, SkillRiskLevel, SkillRiskProfile,
    SkillStepDefinition, SkillVerificationSpec,
};
pub use registry::{SkillError, SkillRegistry};
pub use verification::{SkillVerificationCompiler, SkillVerificationPlan};
