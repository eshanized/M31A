//! Quality assurance and verification model-facing tools (TL-02, D-07).

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::verification::{
    VerificationKind, VerificationReport, VerificationService, VerificationTarget,
};
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn get_verification(ctx: &ToolExecutionContext) -> Result<Arc<dyn VerificationService>, ToolError> {
    ctx.capability_registry
        .verification()
        .ok_or_else(|| ToolError::capability_unavailable("verification", None))
}

// ---------------------------------------------------------------------------
// 24. run_tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RunTestsInput {
    pub target: Option<String>,
    #[serde(default, deserialize_with = "deserialize_flexible_string_vec")]
    pub args: Option<Vec<String>>,
}

pub struct RunTestsTool;

#[async_trait]
impl TypedTool for RunTestsTool {
    type Input = RunTestsInput;
    type Output = VerificationReport;

    fn id(&self) -> &str {
        "run_tests"
    }

    fn description(&self) -> &str {
        "Execute project test suites and return comprehensive test report."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Verification]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ProcessExecution
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(120, 5 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let verifier = get_verification(ctx)?;
        let mut args = input.args.unwrap_or_default();
        if let Some(target_name) = &input.target {
            let clean_target = target_name.trim().trim_start_matches('@').trim();
            if clean_target != "default" && !clean_target.is_empty() && args.is_empty() {
                if clean_target.starts_with("tests/") {
                    let test_name = clean_target
                        .strip_prefix("tests/")
                        .unwrap_or(clean_target)
                        .strip_suffix(".rs")
                        .unwrap_or(clean_target);
                    args.push("--test".to_string());
                    args.push(test_name.to_string());
                } else if clean_target.ends_with("_test") || clean_target.ends_with("_test.rs") {
                    let test_name = clean_target.strip_suffix(".rs").unwrap_or(clean_target);
                    args.push("--test".to_string());
                    args.push(test_name.to_string());
                }
            }
        }

        let target = VerificationTarget {
            name: input.target.unwrap_or_else(|| "default".to_string()),
            kind: VerificationKind::Test,
            args,
        };

        let mut report = verifier.run_verification(&target).await?;
        if report.passed || report.exit_code == 0 {
            report.output.push_str("\n\nAll tests passed successfully! Verification complete. You should now call the 'complete' tool with a summary of the fix.");
        } else if let Some(diag_header) = format_verification_diagnostic_header(&report.output) {
            report.output = format!("{}{}", diag_header, report.output);
        }
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// 25. run_formatter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RunFormatterInput {
    pub target: Option<String>,
    #[serde(default, deserialize_with = "deserialize_flexible_string_vec")]
    pub args: Option<Vec<String>>,
}

pub struct RunFormatterTool;

#[async_trait]
impl TypedTool for RunFormatterTool {
    type Input = RunFormatterInput;
    type Output = VerificationReport;

    fn id(&self) -> &str {
        "run_formatter"
    }

    fn description(&self) -> &str {
        "Run code formatting checks or apply canonical code formatting."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Verification]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(60, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let verifier = get_verification(ctx)?;
        let target = VerificationTarget {
            name: input.target.unwrap_or_else(|| "default".to_string()),
            kind: VerificationKind::Format,
            args: input.args.unwrap_or_default(),
        };

        let report = verifier.run_verification(&target).await?;
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// 26. run_linter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RunLinterInput {
    pub target: Option<String>,
    #[serde(default, deserialize_with = "deserialize_flexible_string_vec")]
    pub args: Option<Vec<String>>,
}

pub struct RunLinterTool;

#[async_trait]
impl TypedTool for RunLinterTool {
    type Input = RunLinterInput;
    type Output = VerificationReport;

    fn id(&self) -> &str {
        "run_linter"
    }

    fn description(&self) -> &str {
        "Run static analysis and linting checks on the workspace."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Verification]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(60, 5 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let verifier = get_verification(ctx)?;
        let target = VerificationTarget {
            name: input.target.unwrap_or_else(|| "default".to_string()),
            kind: VerificationKind::Lint,
            args: input.args.unwrap_or_default(),
        };

        let mut report = verifier.run_verification(&target).await?;
        if !report.passed
            && report.exit_code != 0
            && let Some(diag_header) = format_verification_diagnostic_header(&report.output)
        {
            report.output = format!("{}{}", diag_header, report.output);
        }
        Ok(report)
    }
}

/// Helper deserializer allowing `args` to be specified as:
/// - A list of strings: `["--test", "parser_test"]`
/// - A single string containing space-separated tokens: `"--test parser_test"`
/// - A Pythonic/JSON-stringified list: `"['--test', 'parser_test']"`
pub fn deserialize_flexible_string_vec<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FlexibleVecVisitor;

    impl<'de> serde::de::Visitor<'de> for FlexibleVecVisitor {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a list of strings or a single string")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(val) = seq.next_element::<serde_json::Value>()? {
                match val {
                    serde_json::Value::String(s) => vec.push(s),
                    other => vec.push(other.to_string()),
                }
            }
            Ok(Some(vec))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return Ok(Some(Vec::new()));
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
                    return Ok(Some(parsed));
                }
                let inner = &trimmed[1..trimmed.len() - 1];
                let parts: Vec<String> = inner
                    .split(',')
                    .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                return Ok(Some(parts));
            }

            let parts: Vec<String> = trimmed.split_whitespace().map(|s| s.to_string()).collect();
            Ok(Some(parts))
        }
    }

    deserializer.deserialize_option(FlexibleVecVisitor)
}

/// Extract focused, actionable diagnostics from compiler or test runner output (P3-C).
pub fn format_verification_diagnostic_header(output: &str) -> Option<String> {
    let mut failing_tests = Vec::new();
    let mut panics = Vec::new();
    let mut compiler_errors = Vec::new();

    let lines: Vec<&str> = output.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // 1. Failing tests: test foo::bar ... FAILED
        if trimmed.starts_with("test ")
            && trimmed.ends_with("... FAILED")
            && let Some(test_name) = trimmed.strip_prefix("test ")
            && let Some(test_name) = test_name.strip_suffix("... FAILED")
        {
            failing_tests.push(test_name.trim().to_string());
        }
        // 2. Panics and assertion failures
        if trimmed.contains("panicked at ") {
            let mut panic_detail = trimmed.to_string();
            for next_line in lines.iter().skip(i + 1).take(4) {
                let nt = next_line.trim();
                if nt.starts_with("assertion `")
                    || nt.starts_with("assertion failed")
                    || nt.starts_with("left:")
                    || nt.starts_with("right:")
                    || nt.starts_with("-->")
                {
                    panic_detail.push(' ');
                    panic_detail.push_str(nt);
                } else if nt.starts_with("note: run with") || nt.starts_with("stack backtrace:") {
                    break;
                }
            }
            panics.push(panic_detail);
        }
        // 3. Compiler errors (exclude cargo test exit messages)
        if (trimmed.starts_with("error[") || trimmed.starts_with("error:"))
            && !trimmed.contains("test failed, to rerun pass")
            && !trimmed.contains("could not compile")
        {
            let err_msg = trimmed.to_string();
            let mut location = None;
            // Check next few lines for ` --> path:line:col`
            for next_line in lines.iter().skip(i + 1).take(3) {
                let next_trim = next_line.trim();
                if next_trim.starts_with("-->") {
                    location = Some(
                        next_trim
                            .strip_prefix("-->")
                            .unwrap_or(next_trim)
                            .trim()
                            .to_string(),
                    );
                    break;
                }
            }
            if let Some(loc) = location {
                compiler_errors.push(format!("{}: {}", loc, err_msg));
            } else {
                compiler_errors.push(err_msg);
            }
        }
    }

    if failing_tests.is_empty() && panics.is_empty() && compiler_errors.is_empty() {
        return None;
    }

    let mut header = String::from("=== VERIFICATION FAILURE DIAGNOSTIC ===\n");
    let mut recommended_file = None;

    if !compiler_errors.is_empty() {
        header.push_str("COMPILER ERRORS:\n");
        for ce in compiler_errors.iter().take(5) {
            header.push_str(&format!("  - {}\n", ce));
            if recommended_file.is_none()
                && let Some(colon) = ce.find(':')
            {
                recommended_file = Some(ce[..colon].to_string());
            }
        }
        if compiler_errors.len() > 5 {
            header.push_str(&format!(
                "  ... and {} more compiler errors\n",
                compiler_errors.len() - 5
            ));
        }
    }

    if !failing_tests.is_empty() {
        header.push_str(&format!("FAILING TESTS: [{}]\n", failing_tests.join(", ")));
    }

    if !panics.is_empty() {
        header.push_str("PANICS:\n");
        for p in panics.iter().take(3) {
            header.push_str(&format!("  - {}\n", p));
        }
    }

    if let Some(target) = recommended_file {
        header.push_str(&format!(
            "RECOMMENDED NEXT ACTION: Use 'read_file' to inspect '{}' around the error line and apply fixes with 'edit_file'.\n",
            target
        ));
    } else if !failing_tests.is_empty() {
        header.push_str(
            "RECOMMENDED NEXT ACTION: Inspect the test assertion and the implementation code exercised by the failing tests.\n",
        );
    }

    header.push_str("=======================================\n\n");
    Some(header)
}
