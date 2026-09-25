//! 15-Class Failure Classifier with Deterministic Rules Engine and Tier 7 Fallback (FLC-01, D-05).
//!
//! Evaluates exit codes, compiler/test outputs, sandbox rejections, and policy denials
//! against high-precedence deterministic rules across all 15 specification failure classes:
//! Transient, Timeout, Permission, Policy, Environment, Dependency, Compilation, Test,
//! ToolContract, Model, Context, ResourceLimit, RepositoryState, Architecture, Unknown.
//! Ambiguous or unclassified errors fall back to the Tier 7 Model-Based Diagnostician.

use regex::Regex;
use std::sync::LazyLock;

pub use crate::kernel::seams::recovery::FailureClassification;
pub type FailureClass = FailureClassification;

use crate::verification::diagnostician::{DiagnosticianContext, ModelDiagnostician};

static RUSTC_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:error\[E(\d{4})\]|\[E(\d{4})\]|\bE(\d{4})\b)").unwrap());

/// High-precedence deterministic failure classifier with Tier 7 model fallback (D-05).
#[derive(Clone)]
pub struct FailureClassifier {
    diagnostician: Option<ModelDiagnostician>,
}

impl Default for FailureClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl FailureClassifier {
    pub fn new() -> Self {
        Self {
            diagnostician: None,
        }
    }

    pub fn with_diagnostician(diagnostician: ModelDiagnostician) -> Self {
        Self {
            diagnostician: Some(diagnostician),
        }
    }

    /// Pure deterministic classification without model fallback.
    pub fn classify_deterministic(
        exit_code: Option<i32>,
        error_msg: &str,
    ) -> FailureClassification {
        let msg = error_msg.to_lowercase();

        // 1. Policy Violation (Highest precedence: security denials)
        if msg.contains("policygate")
            || msg.contains("policy denied")
            || msg.contains("policy violation")
            || msg.contains("unauthorized tool")
            || msg.contains("unauthorized file")
            || msg.contains("forbidden by policy")
        {
            return FailureClassification::Policy;
        }

        // 2. Permission / Sandbox Violation
        if msg.contains("eacces")
            || msg.contains("eperm")
            || msg.contains("permission denied")
            || msg.contains("read-only file system")
            || msg.contains("operation not permitted")
            || msg.contains("sandbox escaping")
            || msg.contains("seccomp violation")
        {
            return FailureClassification::Permission;
        }

        // 3. Timeout
        if msg.contains("deadline exceeded")
            || msg.contains("task wall-clock timeout")
            || msg.contains("command timed out")
            || msg.contains("operation timed out")
            || msg.contains("execution timed out")
            || msg.contains("timeout reached")
        {
            return FailureClassification::Timeout;
        }

        // 4. Resource Limit
        if msg.contains("out of memory")
            || msg.contains("oom-killer")
            || msg.contains("memory exhaustion")
            || msg.contains("token budget ceiling hit")
            || msg.contains("disk quota exceeded")
            || msg.contains("resource exhaustion")
        {
            return FailureClassification::ResourceLimit;
        }

        // 5. Transient / Network / Upstream
        if msg.contains("429")
            || msg.contains("503")
            || msg.contains("502")
            || msg.contains("504")
            || msg.contains("connection reset")
            || msg.contains("connection refused")
            || msg.contains("network unreachable")
            || msg.contains("temporary network failure")
            || msg.contains("broken pipe")
            || msg.contains("dns resolution failed")
            || msg.contains("upstream service")
            || msg.contains("upstream error")
            || msg.contains("retry-after")
        {
            return FailureClassification::Transient;
        }

        // 6. Context Window Overflow
        if msg.contains("token window overflow")
            || msg.contains("context compaction error")
            || msg.contains("maximum context length exceeded")
            || msg.contains("prompt exceeds context window")
        {
            return FailureClassification::Context;
        }

        // 7. Model Provider Failure
        if msg.contains("provider 500")
            || msg.contains("stream disruption")
            || msg.contains("unparseable model response")
            || msg.contains("model refusal")
            || msg.contains("rate limit exceeded on model")
            || msg.contains("model overloaded")
            || msg.contains("no model provider configured")
            || msg.contains("model call failure")
        {
            return FailureClassification::Model;
        }

        // 8. Tool Contract / Schema Violation
        if msg.contains("invalid json")
            || msg.contains("schema validation failure")
            || msg.contains("schema violation")
            || msg.contains("missing required field")
            || msg.contains("invalid arguments")
            || msg.contains("malformed tool arguments")
            || msg.contains("deserialization error")
        {
            return FailureClassification::ToolContract;
        }

        // 9. Architecture / Deadlock / Cycles / Role Envelope Mismatches
        if msg.contains("cyclic dependency")
            || msg.contains("cycle detected in task graph")
            || msg.contains("violated modular boundary")
            || msg.contains("deadlock")
            || msg.contains("loop detected")
            || msg.contains("dag cycle")
            || msg.contains("capability envelope violation")
            || msg.contains("forbidden by role envelope")
            || msg.contains("worker allocation failed")
        {
            return FailureClassification::Architecture;
        }

        // 10. Repository State / Drift / Merge Conflicts
        if msg.contains("unexpected drift")
            || msg.contains("dirty working tree")
            || msg.contains("git conflict")
            || msg.contains("merge conflict")
            || msg.contains("stale state")
            || msg.contains("index.lock")
        {
            return FailureClassification::RepositoryState;
        }

        // 11. Environment Failure
        if msg.contains("missing compiler binary")
            || msg.contains("command not found")
            || msg.contains("missing system library")
            || msg.contains("bad path")
            || msg.contains("environment variable not set")
            || msg.contains("cannot find binary")
            || msg.contains("no such file or directory: 'cargo'")
            || msg.contains("no such file or directory: 'rustc'")
        {
            return FailureClassification::Environment;
        }

        // 12. Dependency Failure
        if msg.contains("failed to select a version")
            || msg.contains("crate resolution failure")
            || msg.contains("lockfile mismatch")
            || msg.contains("no matching package named")
            || msg.contains("unresolved dependency")
            || msg.contains("could not find `") && msg.contains("in registry")
        {
            return FailureClassification::Dependency;
        }

        // 13. Compilation Failure
        if RUSTC_CODE_RE.is_match(error_msg)
            || msg.contains("could not compile")
            || msg.contains("compilation failed")
            || msg.contains("compiler errors detected")
            || msg.contains("cargo check")
            || msg.contains("unclosed delimiter")
            || msg.contains("type mismatch")
            || msg.contains("syntax error")
            || msg.contains("cannot find value")
            || msg.contains("cannot find function")
            || msg.contains("cannot find type")
            || msg.contains("cannot find macro")
            || msg.contains("mismatched types")
            || msg.contains("trait bound not satisfied")
            || (msg.contains("expected `") && msg.contains("found `"))
            || (msg.contains("error:")
                && (msg.contains("-->") || msg.contains(".rs:") || msg.contains("aborting due to")))
        {
            return FailureClassification::Compilation;
        }

        // 14. Test Failure
        if msg.contains("panicked at")
            || msg.contains("assertion failed")
            || msg.contains("assertion `left == right` failed")
            || msg.contains("assertion left == right failed")
            || msg.contains("failed tests:")
            || msg.contains("panics:")
            || msg.contains("test result: failed")
            || msg.contains("test failed")
            || (exit_code.is_some() && exit_code != Some(0) && msg.contains("cargo test"))
            || (msg.contains("cargo test") && msg.contains("failed"))
        {
            return FailureClassification::Test;
        }

        FailureClassification::Unknown
    }

    /// Full classification with Tier 7 fallback for Unknown / ambiguous cases.
    pub async fn classify(&self, exit_code: Option<i32>, error_msg: &str) -> FailureClassification {
        let deterministic = Self::classify_deterministic(exit_code, error_msg);
        if deterministic != FailureClassification::Unknown {
            return deterministic;
        }

        // Tier 7 Diagnostician Fallback (D-05)
        if let Some(ref diagnostician) = self.diagnostician {
            let ctx = DiagnosticianContext::new(error_msg, exit_code, None, None);
            return diagnostician.diagnose(&ctx).await;
        }

        FailureClassification::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_15_deterministic_classes() {
        let cases = [
            (
                "HTTP 429 Too Many Requests: retry-after: 30",
                FailureClassification::Transient,
            ),
            (
                "task wall-clock timeout: deadline exceeded",
                FailureClassification::Timeout,
            ),
            (
                "EACCES: permission denied /etc/shadow",
                FailureClassification::Permission,
            ),
            (
                "PolicyGate: unauthorized tool invocation denied",
                FailureClassification::Policy,
            ),
            (
                "sh: cargo: command not found (missing compiler binary)",
                FailureClassification::Environment,
            ),
            (
                "cargo check: failed to select a version for crate 'tokio'",
                FailureClassification::Dependency,
            ),
            (
                "error[E0308]: mismatched types: expected u64, found i32",
                FailureClassification::Compilation,
            ),
            (
                "assertion failed: `left == right` at tests/common.rs:42",
                FailureClassification::Test,
            ),
            (
                "ToolContract error: schema validation failure: missing required field 'id'",
                FailureClassification::ToolContract,
            ),
            (
                "Model stream disruption: provider 500 internal server error",
                FailureClassification::Model,
            ),
            (
                "token window overflow: prompt exceeds context window",
                FailureClassification::Context,
            ),
            (
                "fatal: out of memory (oom-killer invoked)",
                FailureClassification::ResourceLimit,
            ),
            (
                "fatal: unexpected drift: dirty working tree and git conflict",
                FailureClassification::RepositoryState,
            ),
            (
                "Architecture error: cycle detected in task graph dependencies",
                FailureClassification::Architecture,
            ),
            (
                "Compiler errors detected: [E0308, E0284]",
                FailureClassification::Compilation,
            ),
            (
                "Compilation failed: error: this file contains an unclosed delimiter; --> src/parser.rs:9:3",
                FailureClassification::Compilation,
            ),
            (
                "Failed tests: [test_parse_multiplication, test_parse_simple_addition]; Panics: [assertion left == right failed]",
                FailureClassification::Test,
            ),
            (
                "unrecognized exotic system trap signal 99",
                FailureClassification::Unknown,
            ),
        ];

        for (error_msg, expected_class) in cases {
            let classified = FailureClassifier::classify_deterministic(None, error_msg);
            assert_eq!(
                classified, expected_class,
                "Failed deterministic match for '{}'",
                error_msg
            );
        }
    }

    #[tokio::test]
    async fn test_tier_7_diagnostician_fallback() {
        let diagnostician =
            ModelDiagnostician::with_simulated_diagnosis(FailureClassification::Dependency);
        let classifier = FailureClassifier::with_diagnostician(diagnostician);

        // Ambiguous error message
        let result = classifier
            .classify(Some(1), "weird build pipeline hiccup")
            .await;
        assert_eq!(result, FailureClassification::Dependency);
    }
}
