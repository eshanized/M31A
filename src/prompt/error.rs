//! Strongly typed errors for prompt contract validation, rendering, and catalog operations.

use thiserror::Error;

/// Strongly typed prompt subsystem error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PromptError {
    /// Requested prompt contract was not found in the catalog.
    #[error("prompt contract '{id}' (version {version}) not found")]
    PromptNotFound { id: String, version: u32 },

    /// Attempted to register a duplicate prompt contract with inconsistent content.
    #[error("duplicate prompt contract '{id}' (version {version}): {reason}")]
    PromptDuplicate {
        id: String,
        version: u32,
        reason: String,
    },

    /// Prompt contract is structurally or semantically invalid.
    #[error("invalid prompt contract '{id}' (version {version}): {reason}")]
    PromptInvalid {
        id: String,
        version: u32,
        reason: String,
    },

    /// Required parameter for prompt template was missing.
    #[error("missing required prompt parameter '{parameter}' for prompt '{prompt_id}'")]
    PromptParameterMissing {
        prompt_id: String,
        parameter: String,
    },

    /// Prompt parameter failed validation.
    #[error("invalid parameter '{parameter}' for prompt '{prompt_id}': {reason}")]
    PromptParameterInvalid {
        prompt_id: String,
        parameter: String,
        reason: String,
    },

    /// Template rendering error.
    #[error("failed to render prompt '{prompt_id}': {reason}")]
    PromptRenderFailure { prompt_id: String, reason: String },

    /// Prompt version mismatch between step expectation and catalog.
    #[error(
        "version mismatch for prompt '{prompt_id}': requested {requested}, available {available}"
    )]
    PromptVersionMismatch {
        prompt_id: String,
        requested: u32,
        available: u32,
    },

    /// A path safety violation occurred during prompt file discovery.
    #[error("invalid prompt path '{path}': {reason}")]
    PathViolation { path: String, reason: String },

    /// Compiled prompt size exceeded permitted budget.
    #[error(
        "compiled prompt budget exceeded for '{prompt_id}': size {size_bytes} bytes exceeds maximum {max_bytes} bytes: {reason}"
    )]
    PromptBudgetExceeded {
        prompt_id: String,
        size_bytes: usize,
        max_bytes: usize,
        reason: String,
    },

    /// Prompt composition error.
    #[error("failed to compose prompt '{prompt_id}': {reason}")]
    PromptCompositionError { prompt_id: String, reason: String },

    /// Prompt security or authority violation.
    #[error("prompt security violation for '{prompt_id}': {reason}")]
    PromptSecurityViolation { prompt_id: String, reason: String },
}
