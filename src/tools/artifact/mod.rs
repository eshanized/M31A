//! Immutable artifact storage model-facing tools (TL-02, D-07).

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::artifacts::ArtifactStoreService;
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn get_artifacts(ctx: &ToolExecutionContext) -> Result<Arc<dyn ArtifactStoreService>, ToolError> {
    ctx.capability_registry
        .artifacts()
        .ok_or_else(|| ToolError::capability_unavailable("artifacts", None))
}

// ---------------------------------------------------------------------------
// 27. create_artifact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateArtifactInput {
    pub artifact_id: String,
    pub data: String,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateArtifactOutput {
    pub artifact_id: String,
    pub path: String,
    pub bytes_stored: usize,
}

pub struct CreateArtifactTool;

#[async_trait]
impl TypedTool for CreateArtifactTool {
    type Input = CreateArtifactInput;
    type Output = CreateArtifactOutput;

    fn id(&self) -> &str {
        "create_artifact"
    }

    fn description(&self) -> &str {
        "Store content immutably in the artifact store under the specified ID and extension."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Artifacts]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 10 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let store = get_artifacts(ctx)?;
        let ext = input.extension.as_deref().unwrap_or("txt");
        let bytes = input.data.as_bytes();
        let bytes_stored = bytes.len();

        let path = store.store_artifact(&input.artifact_id, bytes, ext).await?;

        Ok(CreateArtifactOutput {
            artifact_id: input.artifact_id,
            path: path.display().to_string(),
            bytes_stored,
        })
    }
}

// ---------------------------------------------------------------------------
// 28. read_artifact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadArtifactInput {
    pub artifact_id: String,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadArtifactOutput {
    pub artifact_id: String,
    pub data: String,
}

pub struct ReadArtifactTool;

#[async_trait]
impl TypedTool for ReadArtifactTool {
    type Input = ReadArtifactInput;
    type Output = ReadArtifactOutput;

    fn id(&self) -> &str {
        "read_artifact"
    }

    fn description(&self) -> &str {
        "Retrieve stored artifact content by artifact ID and optional extension."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Artifacts]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 10 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let store = get_artifacts(ctx)?;
        let ext = input.extension.as_deref().unwrap_or("txt");
        let raw = store.load_artifact(&input.artifact_id, ext).await?;
        let data = String::from_utf8_lossy(&raw).to_string();

        Ok(ReadArtifactOutput {
            artifact_id: input.artifact_id,
            data,
        })
    }
}
