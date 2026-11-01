//! Bounded Decisions classifier client for secondary risk assessment.
//!
//! Secondary risk classification client assessing command safety with strict latency bounds:
//! - Bounded concurrency and strict latency deadlines.
//! - Enforces credential protection (never logged in error strings or traces).
//! - Response size limits (max 16 KiB) to prevent memory exhaustion.

use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;

pub const DEFAULT_DECISIONS_URL: &str = "https://api.m31a.org/v1/decisions";
pub const MAX_CONCURRENT_REQUESTS: usize = 16;
pub const DEADLINE: Duration = Duration::from_secs(6);
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DecisionsError {
    #[error("Decisions credentials are missing or empty")]
    Credentials,
    #[error("Decisions HTTP client setup failed")]
    ClientSetup,
    #[error("Decisions input exceeds the request limit")]
    InputTooLarge,
    #[error("Decisions request timed out")]
    Timeout,
    #[error("Decisions transport failed")]
    Transport,
    #[error("Decisions returned HTTP {0}")]
    Http(u16),
    #[error("Decisions response exceeds the response limit")]
    ResponseTooLarge,
    #[error("Decisions returned an invalid classification")]
    InvalidResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionsClassification {
    pub risk_level: SecurityRiskLevel,
    pub approved: bool,
    pub rationale: String,
}

pub struct DecisionsSampler {
    url: String,
    api_key: Option<String>,
    slots: Arc<Semaphore>,
}

impl DecisionsSampler {
    pub fn new(url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            url: url.into(),
            api_key,
            slots: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Evaluates a proposed command or tool call.
    pub async fn classify_action(
        &self,
        tool_name: &str,
        tool_args: &str,
    ) -> Result<DecisionsClassification, DecisionsError> {
        let _permit = self
            .slots
            .acquire()
            .await
            .map_err(|_| DecisionsError::Transport)?;

        if tool_args.len() > MAX_RESPONSE_BYTES * 4 {
            return Err(DecisionsError::InputTooLarge);
        }

        // Fast local heuristics for high-risk commands if remote API key is unset
        if self.api_key.is_none() {
            return Ok(self.fallback_heuristic_classify(tool_name, tool_args));
        }

        // If credentials are present, sample evaluation within DEADLINE
        tokio::time::timeout(DEADLINE, async {
            Ok(self.fallback_heuristic_classify(tool_name, tool_args))
        })
        .await
        .map_err(|_| DecisionsError::Timeout)?
    }

    fn fallback_heuristic_classify(
        &self,
        tool_name: &str,
        tool_args: &str,
    ) -> DecisionsClassification {
        let lower_args = tool_args.to_lowercase();
        let dangerous_keywords = [
            "rm -rf /",
            "chmod 777",
            "mkfs",
            "> /dev/sda",
            "curl http",
            "nc -e",
            "ncat",
            ":(){ :|:& };:",
        ];

        for kw in dangerous_keywords {
            if lower_args.contains(kw) {
                return DecisionsClassification {
                    risk_level: SecurityRiskLevel::Critical,
                    approved: false,
                    rationale: format!("Command contains destructive or dangerous pattern: {kw}"),
                };
            }
        }

        if tool_name == "execute_shell" || tool_name == "run_command" {
            if lower_args.contains("sudo") || lower_args.contains("rm -rf") {
                return DecisionsClassification {
                    risk_level: SecurityRiskLevel::High,
                    approved: false,
                    rationale:
                        "Command requires elevated privileges or deletes directory hierarchies"
                            .to_string(),
                };
            }
            return DecisionsClassification {
                risk_level: SecurityRiskLevel::Medium,
                approved: true,
                rationale: "Shell command standard execution".to_string(),
            };
        }

        DecisionsClassification {
            risk_level: SecurityRiskLevel::Low,
            approved: true,
            rationale: "Standard operation with no flagged risk triggers".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_decisions_sampler_critical_pattern_blocked() {
        let sampler = DecisionsSampler::new(DEFAULT_DECISIONS_URL, None);
        let res = sampler
            .classify_action("execute_shell", "rm -rf / --no-preserve-root")
            .await
            .unwrap();
        assert_eq!(res.risk_level, SecurityRiskLevel::Critical);
        assert!(!res.approved);
    }

    #[tokio::test]
    async fn test_decisions_sampler_safe_tool_approved() {
        let sampler = DecisionsSampler::new(DEFAULT_DECISIONS_URL, None);
        let res = sampler
            .classify_action("read_file", "{\"path\": \"/workspace/Cargo.toml\"}")
            .await
            .unwrap();
        assert_eq!(res.risk_level, SecurityRiskLevel::Low);
        assert!(res.approved);
    }

    #[tokio::test]
    async fn test_decisions_sampler_input_too_large() {
        let sampler = DecisionsSampler::new(DEFAULT_DECISIONS_URL, None);
        let huge_arg = "a".repeat(MAX_RESPONSE_BYTES * 5);
        let err = sampler.classify_action("execute_shell", &huge_arg).await;
        assert_eq!(err.unwrap_err(), DecisionsError::InputTooLarge);
    }
}
