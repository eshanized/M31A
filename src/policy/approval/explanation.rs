//! Structured approval explanation packet generator with progressive disclosure (POL-01, D-12).

use serde::{Deserialize, Serialize};

use crate::ids::ApprovalRequestId;
use crate::policy::approval::ApprovalRequest;
use crate::tools::risk::RiskClass;

/// Redact sensitive keys in JSON arguments (tokens, passwords, secrets, api keys).
pub fn redact_sensitive_arguments(args: &serde_json::Value) -> serde_json::Value {
    match args {
        serde_json::Value::Object(map) => {
            let mut redacted_map = serde_json::Map::new();
            for (k, v) in map {
                let lower_k = k.to_lowercase();
                if lower_k.contains("token")
                    || lower_k.contains("secret")
                    || lower_k.contains("password")
                    || lower_k.contains("api_key")
                    || lower_k.contains("auth")
                    || lower_k.contains("credential")
                {
                    redacted_map.insert(
                        k.clone(),
                        serde_json::Value::String("[REDACTED]".to_string()),
                    );
                } else {
                    redacted_map.insert(k.clone(), redact_sensitive_arguments(v));
                }
            }
            serde_json::Value::Object(redacted_map)
        }
        serde_json::Value::Array(arr) => {
            let redacted_arr = arr.iter().map(redact_sensitive_arguments).collect();
            serde_json::Value::Array(redacted_arr)
        }
        other => other.clone(),
    }
}

/// Structured explanation packet presented to interactive operators (D-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalExplanationPacket {
    pub request_id: ApprovalRequestId,
    pub tool_or_capability: String,
    pub affected_resources: Vec<String>,
    pub risk_classification: RiskClass,
    pub matched_rule_id: Option<String>,
    pub policy_hash: String,
    pub reason: String,
    pub summary: String,
    pub redacted_args: serde_json::Value,
}

/// Format an authoritative explanation packet from runtime state.
pub fn format_explanation_packet(req: &ApprovalRequest) -> ApprovalExplanationPacket {
    let resources_str = if req.affected_resources.is_empty() {
        "None declared".to_string()
    } else {
        req.affected_resources.join(", ")
    };

    let rule_str = req
        .matched_rule_id
        .as_deref()
        .unwrap_or("None (default requirement)");

    let summary = match &req.target {
        crate::policy::approval::ApprovalTarget::UserCommand {
            command_id,
            command_version,
            ..
        } => format!(
            "Command-level authorization: User command '/{}' (version {}) requires operator authorization before execution. Risk: {:?}. Target resources: {}. Reason: {}",
            command_id.trim_start_matches("command."),
            command_version,
            req.risk_classification,
            resources_str,
            req.reason
        ),
        crate::policy::approval::ApprovalTarget::ToolCall { .. } => format!(
            "Tool-level authorization: Tool '{}' requires operator authorization. Risk: {:?}. Rule: {}. Target resources: {}. Reason: {}",
            req.tool_or_capability, req.risk_classification, rule_str, resources_str, req.reason
        ),
    };

    ApprovalExplanationPacket {
        request_id: req.id,
        tool_or_capability: req.tool_or_capability.clone(),
        affected_resources: req.affected_resources.clone(),
        risk_classification: req.risk_classification,
        matched_rule_id: req.matched_rule_id.clone(),
        policy_hash: req.policy_hash.clone(),
        reason: req.reason.clone(),
        summary,
        redacted_args: req.redacted_args.clone(),
    }
}
