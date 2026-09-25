//! Parameter models and limits for prompt template evaluation.

use serde::{Deserialize, Serialize};

/// Maximum permissible byte size for all prompt parameters combined (2 MB).
pub const MAX_PARAMETER_BYTES: usize = 2 * 1024 * 1024;

/// Specification for a single input parameter accepted by a prompt contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptParameter {
    /// Parameter identifier expected in template rendering.
    pub name: String,
    /// Human-readable description of parameter purpose.
    pub description: String,
    /// Whether this parameter must be supplied by the caller.
    pub is_required: bool,
    /// Default string value to use if parameter is optional and omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}
