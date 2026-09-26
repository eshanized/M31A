//! Model-available specialized skills tools consuming SkillRegistry.

use crate::capability::family::CapabilityFamily;
use crate::skill::registry::SkillRegistry;
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub origin: String,
    pub verification_tier: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SkillsListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillsListOutput {
    pub skills: Vec<SkillSummaryDto>,
}

pub struct SkillsListTool;

#[async_trait]
impl TypedTool for SkillsListTool {
    type Input = SkillsListInput;
    type Output = SkillsListOutput;

    fn id(&self) -> &str {
        "skills_list"
    }

    fn description(&self) -> &str {
        "List all available specialized engineering skills in the workspace registry."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let registry = SkillRegistry::load_discovered(Some(&ctx.workspace_root))
            .map_err(|e| ToolError::execution_failed(e.to_string(), None, None))?;
        let mut list = Vec::new();
        for pkg in registry.list() {
            list.push(SkillSummaryDto {
                id: pkg.manifest.id.clone(),
                name: pkg.manifest.name.clone(),
                description: pkg
                    .manifest
                    .procedure
                    .instructions
                    .chars()
                    .take(120)
                    .collect(),
                origin: pkg.origin.to_string(),
                verification_tier: pkg.manifest.verification.tier,
            });
        }
        Ok(SkillsListOutput { skills: list })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillsInspectInput {
    pub skill_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillsInspectOutput {
    pub skill_id: String,
    pub name: String,
    pub instructions: String,
    pub steps: Vec<String>,
    pub verification_commands: Vec<String>,
    pub evidence_required: Vec<String>,
}

pub struct SkillsInspectTool;

#[async_trait]
impl TypedTool for SkillsInspectTool {
    type Input = SkillsInspectInput;
    type Output = SkillsInspectOutput;

    fn id(&self) -> &str {
        "skills_inspect"
    }

    fn description(&self) -> &str {
        "Inspect a specialized engineering skill by ID to retrieve its detailed procedure and verification commands."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let registry = SkillRegistry::load_discovered(Some(&ctx.workspace_root))
            .map_err(|e| ToolError::execution_failed(e.to_string(), None, None))?;
        let pkg = registry.get(&input.skill_id).ok_or_else(|| {
            ToolError::resource_not_found(format!("skill '{}' not found", input.skill_id), None)
        })?;

        let steps: Vec<String> = pkg
            .manifest
            .procedure
            .steps
            .iter()
            .map(|s| format!("{}: {}", s.name, s.instruction))
            .collect();

        Ok(SkillsInspectOutput {
            skill_id: pkg.manifest.id.clone(),
            name: pkg.manifest.name.clone(),
            instructions: pkg.manifest.procedure.instructions.clone(),
            steps,
            verification_commands: pkg.manifest.verification.commands.clone(),
            evidence_required: pkg.manifest.verification.evidence_required.clone(),
        })
    }
}
