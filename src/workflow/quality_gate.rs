//! QualityGate contracts, deterministic Tier-1 and Tier-2 evaluation, and violation reporting.

use crate::prompt::PromptContract;
use crate::workflow::definition::{
    QualityGate, WorkflowDefinition, WorkflowStepDefinition, validate_artifact_path,
};
use crate::workflow::error::WorkflowError;
use crate::workflow::state::WorkflowArtifact;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// Evaluation tier for quality gate assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QualityGateTier {
    /// Tier 1: Deterministic schema, syntax, non-empty, and artifact presence checks.
    Tier1SyntaxAndArtifacts,
    /// Tier 2: Relational consistency, upstream binding integrity, and role alignment.
    Tier2RelationalConsistency,
    /// Tier 3: Human operator approval or model-driven evaluation (deferred to runtime).
    Tier3HumanOrModelReview,
}

/// Severity level for quality gate violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QualityGateSeverity {
    Warning,
    Error,
}

/// Structured diagnostic detail describing a quality gate violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityGateViolation {
    pub code: String,
    pub severity: QualityGateSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_name: Option<String>,
}

impl QualityGateViolation {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        step_key: Option<String>,
        artifact_name: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: QualityGateSeverity::Error,
            message: message.into(),
            step_key,
            artifact_name,
        }
    }

    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        step_key: Option<String>,
        artifact_name: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: QualityGateSeverity::Warning,
            message: message.into(),
            step_key,
            artifact_name,
        }
    }
}

/// Result of evaluating quality gates against step execution outputs or definition relationships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityGateResult {
    pub is_passed: bool,
    pub tier: QualityGateTier,
    pub violations: Vec<QualityGateViolation>,
}

impl QualityGateResult {
    pub fn pass(tier: QualityGateTier) -> Self {
        Self {
            is_passed: true,
            tier,
            violations: Vec::new(),
        }
    }

    pub fn fail(tier: QualityGateTier, mut violations: Vec<QualityGateViolation>) -> Self {
        // Deterministically sort violations
        violations.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.code.cmp(&b.code))
                .then_with(|| a.step_key.cmp(&b.step_key))
                .then_with(|| a.artifact_name.cmp(&b.artifact_name))
                .then_with(|| a.message.cmp(&b.message))
        });
        Self {
            is_passed: false,
            tier,
            violations,
        }
    }
}

/// Explicit context provided to a quality gate evaluator.
#[derive(Debug, Clone)]
pub struct QualityGateContext<'a> {
    pub step_key: &'a str,
    pub workspace_root: &'a Path,
    pub step_definition: Option<&'a WorkflowStepDefinition>,
    pub workflow_definition: Option<&'a WorkflowDefinition>,
    pub produced_artifacts: &'a [WorkflowArtifact],
    pub upstream_artifacts: &'a [WorkflowArtifact],
    pub prompt_contract: Option<&'a PromptContract>,
}

/// Abstract evaluator interface for assessing step artifacts and definitions against quality gates.
pub trait QualityGateEvaluator: Send + Sync {
    /// Evaluate the gate against the provided execution or definition context.
    fn evaluate(
        &self,
        gate: &QualityGate,
        context: &QualityGateContext<'_>,
    ) -> Result<QualityGateResult, WorkflowError>;
}

/// Deterministic Tier-1 Evaluator: syntax, non-empty, artifact existence, and schema checks.
///
/// Tier-1 NEVER calls an LLM or makes probabilistic judgments.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tier1QualityGateEvaluator;

impl Tier1QualityGateEvaluator {
    pub fn new() -> Self {
        Self
    }
}

impl QualityGateEvaluator for Tier1QualityGateEvaluator {
    fn evaluate(
        &self,
        gate: &QualityGate,
        context: &QualityGateContext<'_>,
    ) -> Result<QualityGateResult, WorkflowError> {
        let mut violations = Vec::new();
        let step_key = context.step_key;

        // Map produced artifacts by name
        let mut produced_by_name: BTreeMap<&str, &WorkflowArtifact> = BTreeMap::new();
        for art in context.produced_artifacts {
            let file_name = art
                .path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            // Store by filename and by full relative path
            produced_by_name.insert(art.path.to_str().unwrap_or_default(), art);
            if !file_name.is_empty() {
                // Also index by simple filename if matched by required_artifacts
                produced_by_name
                    .entry(art.path.file_name().unwrap().to_str().unwrap())
                    .or_insert(art);
            }
        }

        // 1. Required Artifact Presence & Integrity
        for req_art in &gate.required_artifacts {
            let found = produced_by_name
                .iter()
                .find(|(k, _)| **k == req_art.as_str() || Path::new(*k).ends_with(req_art));

            match found {
                None => {
                    // Check if file physically exists in workspace_root
                    let target_path = context.workspace_root.join(req_art);
                    if !target_path.exists() {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-01",
                            format!(
                                "required artifact '{}' was not emitted and does not exist on disk",
                                req_art
                            ),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                        continue;
                    }

                    // Validate path security
                    if let Err(e) =
                        validate_artifact_path(Path::new(req_art), Some(context.workspace_root))
                    {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-02",
                            format!("artifact path violates security constraints: {}", e),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                        continue;
                    }

                    // Check non-empty on disk
                    if let Ok(meta) = std::fs::metadata(&target_path)
                        && meta.len() == 0
                    {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-03",
                            format!(
                                "artifact '{}' exists on disk but is empty (0 bytes)",
                                req_art
                            ),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                        continue;
                    }

                    // Check valid UTF-8
                    if let Ok(bytes) = std::fs::read(&target_path)
                        && std::str::from_utf8(&bytes).is_err()
                    {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-04",
                            format!("artifact '{}' contains invalid non-UTF-8 bytes", req_art),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                    }
                }
                Some((_, art)) => {
                    // Validate path security
                    if let Err(e) = validate_artifact_path(&art.path, Some(context.workspace_root))
                    {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-02",
                            format!("artifact path violates security constraints: {}", e),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                    }

                    // Check non-empty on disk if present
                    let target_path = context.workspace_root.join(&art.path);
                    if let Ok(meta) = std::fs::metadata(&target_path)
                        && meta.len() == 0
                    {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-03",
                            format!("artifact '{}' is empty (0 bytes)", req_art),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                    }

                    // Check valid SHA-256 content_hash
                    if art.content_hash.len() != 64
                        || !art.content_hash.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        violations.push(QualityGateViolation::error(
                            "QG-T1-05",
                            format!(
                                "artifact '{}' has missing or invalid SHA-256 hash '{}'",
                                req_art, art.content_hash
                            ),
                            Some(step_key.to_string()),
                            Some(req_art.clone()),
                        ));
                    }
                }
            }
        }

        // 2. Schema Identifier Check
        if let Some(expected_schema) = &gate.schema {
            for art in context.produced_artifacts {
                let path_str = art.path.to_string_lossy().to_lowercase();
                let name_lower = art.name.to_lowercase();
                let schema_lower = expected_schema.to_lowercase();
                if !path_str.contains(&schema_lower)
                    && !name_lower.contains(&schema_lower)
                    && !expected_schema.is_empty()
                {
                    violations.push(QualityGateViolation::warning(
                        "QG-T1-06",
                        format!(
                            "artifact '{}' may not match expected gate schema '{}'",
                            art.path.display(),
                            expected_schema
                        ),
                        Some(step_key.to_string()),
                        Some(art.path.display().to_string()),
                    ));
                }
            }
        }

        // 3. Ambiguity Bounds Check
        if let Some(ambiguity) = gate.max_ambiguity_percent
            && ambiguity > 100
        {
            violations.push(QualityGateViolation::error(
                "QG-T1-07",
                format!("gate max_ambiguity_percent {} cannot exceed 100", ambiguity),
                Some(step_key.to_string()),
                None,
            ));
        }

        if violations
            .iter()
            .any(|v| v.severity == QualityGateSeverity::Error)
        {
            Ok(QualityGateResult::fail(
                QualityGateTier::Tier1SyntaxAndArtifacts,
                violations,
            ))
        } else {
            Ok(QualityGateResult::pass(
                QualityGateTier::Tier1SyntaxAndArtifacts,
            ))
        }
    }
}

/// Deterministic Tier-2 Evaluator: relational completeness, binding consistency, and role alignment.
///
/// Tier-2 NEVER calls an LLM or makes probabilistic judgments.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tier2QualityGateEvaluator;

impl Tier2QualityGateEvaluator {
    pub fn new() -> Self {
        Self
    }
}

impl QualityGateEvaluator for Tier2QualityGateEvaluator {
    fn evaluate(
        &self,
        gate: &QualityGate,
        context: &QualityGateContext<'_>,
    ) -> Result<QualityGateResult, WorkflowError> {
        let mut violations = Vec::new();
        let step_key = context.step_key;

        // 1. Relational step definition checks
        if let Some(step) = context.step_definition {
            // Quality gate required artifacts must be declared in step expected outputs
            let declared_output_names: HashSet<&str> = step
                .expected_outputs
                .iter()
                .map(|o| o.artifact_name.as_str())
                .collect();

            for req in &gate.required_artifacts {
                if !declared_output_names.contains(req.as_str()) {
                    violations.push(QualityGateViolation::error(
                        "QG-T2-01",
                        format!(
                            "quality gate requires artifact '{}' which is not declared in step's expected_outputs",
                            req
                        ),
                        Some(step_key.to_string()),
                        Some(req.clone()),
                    ));
                }
            }

            // Upstream input binding relational integrity
            if let Some(wf) = context.workflow_definition {
                let steps_by_key: BTreeMap<&str, &WorkflowStepDefinition> =
                    wf.steps.iter().map(|s| (s.key.as_str(), s)).collect();

                for input in &step.required_inputs {
                    match steps_by_key.get(input.source_step_key.as_str()) {
                        None => {
                            violations.push(QualityGateViolation::error(
                                "QG-T2-02",
                                format!(
                                    "input binding parameter '{}' references non-existent upstream step '{}'",
                                    input.parameter_name, input.source_step_key
                                ),
                                Some(step_key.to_string()),
                                Some(input.artifact_name.clone()),
                            ));
                        }
                        Some(source_step) => {
                            // Check source step declares the artifact
                            let source_emits = source_step
                                .expected_outputs
                                .iter()
                                .any(|o| o.artifact_name == input.artifact_name);

                            if !source_emits && !input.is_optional {
                                violations.push(QualityGateViolation::error(
                                    "QG-T2-03",
                                    format!(
                                        "upstream step '{}' does not declare required artifact '{}' requested by '{}'",
                                        input.source_step_key, input.artifact_name, step_key
                                    ),
                                    Some(step_key.to_string()),
                                    Some(input.artifact_name.clone()),
                                ));
                            }

                            // Check dependency is declared in depends_on
                            if !step.depends_on.contains(&input.source_step_key) {
                                violations.push(QualityGateViolation::error(
                                    "QG-T2-04",
                                    format!(
                                        "step '{}' binds input from step '{}' without declaring it in depends_on",
                                        step_key, input.source_step_key
                                    ),
                                    Some(step_key.to_string()),
                                    Some(input.artifact_name.clone()),
                                ));
                            }
                        }
                    }
                }
            }

            // Role and prompt compatibility check (registry-resolved).
            if let Some(prompt) = context.prompt_contract.as_ref() {
                let roles_compatible = crate::agent::registry::RoleRegistry::global()
                    .read()
                    .map(|guard| guard.is_compatible(&step.role, &prompt.role))
                    .unwrap_or(false);
                if !roles_compatible {
                    violations.push(QualityGateViolation::error(
                        "QG-T2-05",
                        format!(
                            "role mismatch: step '{}' requires role '{:?}' but prompt contract '{}' specifies role '{:?}'",
                            step_key, step.role, prompt.id, prompt.role
                        ),
                        Some(step_key.to_string()),
                        None,
                    ));
                }
            }
        }

        if violations
            .iter()
            .any(|v| v.severity == QualityGateSeverity::Error)
        {
            Ok(QualityGateResult::fail(
                QualityGateTier::Tier2RelationalConsistency,
                violations,
            ))
        } else {
            Ok(QualityGateResult::pass(
                QualityGateTier::Tier2RelationalConsistency,
            ))
        }
    }
}
