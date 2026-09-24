//! Stage 5: Capability Check Stage (TL-03, per D-09).

use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::capability::family::CapabilityFamily;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::semantic_validation::SemanticallyValidatedState;
use crate::tools::definition::{AnyTool, ToolExecutionContext};
use crate::tools::risk::RiskClass;

/// Typed state envelope proving capability authorization succeeded.
pub struct CapabilityAuthorizedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
}

/// Stage 5: Verifies required capabilities in `CapabilityRegistry` and role's `CapabilityEnvelope`.
pub struct CapabilityCheckStage;

impl CapabilityCheckStage {
    pub fn execute(
        state: SemanticallyValidatedState,
        context: &ToolExecutionContext,
    ) -> Result<CapabilityAuthorizedState, ToolError> {
        let tool = &state.tool;
        let required_caps = tool.required_capabilities();

        // 1. Check capability availability in central registry
        for family in required_caps {
            if !context.capability_registry.is_available(family) {
                return Err(ToolError::permission_denied(
                    "CAPABILITY_UNAVAILABLE",
                    format!(
                        "Required capability family '{:?}' is currently unavailable or not registered",
                        family
                    ),
                    Some(format!(
                        "Ensure capability '{:?}' provider is registered and healthy",
                        family
                    )),
                )
                .with_provenance("CapabilityCheckStage"));
            }
        }

        // 2. Check against agent role's CapabilityEnvelope if present
        if let Some(ref envelope) = context.role_envelope {
            for family in required_caps {
                match family {
                    CapabilityFamily::Filesystem
                        if tool.base_risk() >= RiskClass::LowRiskMutation
                            && !envelope.allow_file_write =>
                    {
                        return Err(ToolError::permission_denied(
                            "WRITE_ACCESS_FORBIDDEN",
                            format!(
                                "Tool '{}' requires file write access, but role envelope is strictly read-only",
                                tool.id()
                            ),
                            Some("Task role envelope must permit file write operations".to_string()),
                        )
                        .with_provenance("CapabilityCheckStage"));
                    }
                    CapabilityFamily::Shell | CapabilityFamily::Process
                        if !envelope.allow_shell_execution =>
                    {
                        return Err(ToolError::permission_denied(
                            "SHELL_EXECUTION_FORBIDDEN",
                            format!(
                                "Tool '{}' requires shell execution, but role envelope forbids shell execution",
                                tool.id()
                            ),
                            Some("Task role envelope must permit shell execution".to_string()),
                        )
                        .with_provenance("CapabilityCheckStage"));
                    }
                    CapabilityFamily::Network | CapabilityFamily::Web
                        if !envelope.allow_network_access =>
                    {
                        return Err(ToolError::permission_denied(
                            "NETWORK_ACCESS_FORBIDDEN",
                            format!(
                                "Tool '{}' requires network access, but role envelope forbids network access",
                                tool.id()
                            ),
                            Some("Task role envelope must permit network access".to_string()),
                        )
                        .with_provenance("CapabilityCheckStage"));
                    }
                    _ => {}
                }

                // If envelope has specific allowed capability list, verify membership
                if !envelope.allowed_capabilities.is_empty() {
                    let fam_str = family.as_str();
                    let tool_id = tool.id();
                    let matches_envelope = envelope.allowed_capabilities.iter().any(|allowed| {
                        allowed == fam_str
                            || allowed == tool_id
                            || allowed.starts_with(fam_str)
                            || tool_id.starts_with(allowed)
                            || (fam_str == "filesystem"
                                && (allowed.starts_with("fs.")
                                    || allowed == "write_file"
                                    || allowed == "read_file"))
                            || (fam_str == "verification"
                                && (allowed.starts_with("cargo.")
                                    || allowed.starts_with("verification")
                                    || allowed == "run_tests"))
                            || (fam_str == "process"
                                && (allowed.starts_with("shell.")
                                    || allowed.starts_with("proc.")
                                    || allowed == "run_command"))
                    });
                    if !matches_envelope {
                        return Err(ToolError::permission_denied(
                            "CAPABILITY_NOT_IN_ENVELOPE",
                            format!(
                                "Tool '{}' (capability '{:?}') is not within role envelope allowed capabilities",
                                tool_id, family
                            ),
                            Some("Request role envelope elevation if capability is genuinely required".to_string()),
                        )
                        .with_provenance("CapabilityCheckStage"));
                    }
                }
            }
        }

        Ok(CapabilityAuthorizedState {
            action: state.action,
            tool: state.tool,
            decoded_args: state.decoded_args,
        })
    }
}
