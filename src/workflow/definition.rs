//! Declarative workflow definitions, step specifications, and input/output bindings.

use crate::kernel::plan::CapabilityRequirement;
use crate::state_machine::agent::AgentRole;
use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};

fn default_step_timeout() -> u64 {
    600
}

fn default_workflow_version() -> u32 {
    1
}

fn default_max_retries() -> u32 {
    3
}

/// Recovery strategy applied when a workflow step encounters a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecoveryStrategy {
    /// Step failure immediately halts the workflow run as Failed.
    #[default]
    Fail,
    /// Retry the step up to `max_retries` attempts.
    Retry {
        #[serde(default = "default_max_retries")]
        max_retries: u32,
    },
    /// Halt and request operator intervention via approval/decision modal.
    AskOperator,
    /// Mark step skipped and proceed to downstream steps if permitted.
    Skip,
    /// Request replanning of downstream steps from this point.
    Replan,
}

/// Quality gate specification evaluated before accepting a step run as Completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QualityGate {
    /// Optional schema name to validate against (e.g. "discovery_v1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    /// Explicit list of artifact names that must exist and be non-empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_artifacts: Vec<String>,

    /// Whether human operator sign-off is required before completion.
    #[serde(default)]
    pub require_human_approval: bool,

    /// Maximum permissible ambiguity score percentage (0..=100) if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ambiguity_percent: Option<u8>,
}

/// Dynamic input binding connecting an upstream step artifact to a downstream step parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBinding {
    /// Parameter name passed to the prompt template or context compiler.
    pub parameter_name: String,
    /// Step key of the upstream step that produces the artifact.
    pub source_step_key: String,
    /// Name of the artifact emitted by the source step.
    pub artifact_name: String,
    /// Whether the absence of the upstream artifact is permissible.
    #[serde(default)]
    pub is_optional: bool,
}

/// Output binding declaring an artifact produced by a workflow step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputBinding {
    /// Logical name of the artifact (e.g. "PROJECT.md", "SUMMARY.md").
    pub artifact_name: String,
    /// Relative path within the project workspace or projection directory.
    pub relative_path: PathBuf,
    /// Expected schema or format identifier (e.g. "markdown", "json").
    pub schema_type: String,
}

/// Specification for a single discrete step in a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepDefinition {
    /// Unique identifier for this step within the workflow (e.g. "discovery").
    pub key: String,
    /// Human-readable name of this step.
    pub name: String,
    /// Canonical agent role assigned to execute this step.
    pub role: AgentRole,
    /// Prompt template identifier (e.g. "genesis/discovery").
    ///
    /// Legacy string binding, retained for manifest compatibility. New code
    /// MUST set [`WorkflowStepDefinition::prompt_ref`]: when present it is
    /// the authoritative execution binding and `prompt_template` is only a
    /// human-readable label. When absent, it is parsed leniently
    /// (`/`-separated or version-less ids default to the canonical
    /// generation via the catalog).
    pub prompt_template: String,
    /// Typed prompt execution binding for this step.
    ///
    /// First-class execution binding (wiring remediation v0.1.1): this
    /// reference — never [`WorkflowStepDefinition::prompt_template`] text —
    /// flows through lowering into the candidate task, the durable task
    /// record, the work request, the worker, and context compilation. It is
    /// NEVER downgraded into task description metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_ref: Option<crate::prompt::PromptReference>,
    /// Upstream artifact bindings required as inputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_inputs: Vec<InputBinding>,
    /// Artifacts expected to be produced upon step completion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_outputs: Vec<OutputBinding>,
    /// Capability requirements granted to the agent for this step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<CapabilityRequirement>,
    /// Quality gate requirements that must pass before step completion.
    #[serde(default)]
    pub quality_gate: QualityGate,
    /// Keys of steps that must complete before this step can execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Execution timeout in seconds.
    #[serde(default = "default_step_timeout")]
    pub timeout_secs: u64,
    /// Whether this step can execute concurrently with other independent steps.
    #[serde(default)]
    pub allows_parallelism: bool,
    /// Optional step-specific recovery strategy overriding workflow default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_strategy: Option<RecoveryStrategy>,
}

impl WorkflowStepDefinition {
    /// Return the effective recovery strategy for this step, falling back to the workflow default.
    pub fn effective_recovery_strategy(
        &self,
        default_strategy: &RecoveryStrategy,
    ) -> RecoveryStrategy {
        self.recovery_strategy
            .clone()
            .unwrap_or_else(|| default_strategy.clone())
    }

    /// Return the authoritative typed prompt execution binding for this step.
    ///
    /// An explicit [`WorkflowStepDefinition::prompt_ref`] always wins. A
    /// bare `prompt_template` string is parsed leniently (slash/dot
    /// separators, optional version; version-less ids bind the catalog
    /// canonical generation). Fails closed when the template names no
    /// parseable contract.
    pub fn effective_prompt_ref(
        &self,
    ) -> Result<crate::prompt::PromptReference, crate::workflow::error::WorkflowError> {
        if let Some(ref explicit) = self.prompt_ref {
            return Ok(explicit.clone());
        }
        crate::prompt::PromptReference::parse_lenient(&self.prompt_template).map_err(|_| {
            crate::workflow::error::WorkflowError::PromptNotFound {
                id: self.prompt_template.clone(),
                version: 0,
            }
        })
    }
}

/// Declarative specification of a multi-stage workflow.
///
/// Workflow definitions are immutable after load and validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Unique identifier of the workflow definition (e.g. "genesis-greenfield").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Long-form description.
    pub description: String,
    /// Version number of the workflow definition schema/content.
    #[serde(default = "default_workflow_version")]
    pub version: u32,
    /// Ordered steps comprising the workflow.
    pub steps: Vec<WorkflowStepDefinition>,
    /// Default recovery strategy for steps that do not override it.
    #[serde(default)]
    pub default_recovery_strategy: RecoveryStrategy,
}

impl WorkflowDefinition {
    /// Validates the workflow definition ensuring:
    /// - Non-empty ID and name
    /// - Version >= 1
    /// - Non-empty steps with unique keys
    /// - Valid step configurations (timeouts, roles, capabilities)
    /// - Valid artifact paths (relative, no traversal, no internal escape)
    /// - Valid input/output bindings
    /// - Dependencies exist and contain no cycles
    pub fn validate(&self) -> Result<(), WorkflowError> {
        // 1. Identity validation
        if self.id.trim().is_empty() {
            return Err(WorkflowError::InvalidDefinition(
                "workflow id cannot be empty".to_string(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(WorkflowError::InvalidDefinition(
                "workflow name cannot be empty".to_string(),
            ));
        }
        if self.version == 0 {
            return Err(WorkflowError::InvalidDefinition(
                "workflow version must be at least 1".to_string(),
            ));
        }
        if self.steps.is_empty() {
            return Err(WorkflowError::InvalidDefinition(
                "workflow definition must contain at least one step".to_string(),
            ));
        }

        // 2. Validate step uniqueness and per-step sanity
        let mut seen_keys = BTreeSet::new();
        let mut step_outputs: BTreeMap<String, HashSet<String>> = BTreeMap::new();

        for step in &self.steps {
            let trimmed_key = step.key.trim();
            if trimmed_key.is_empty() {
                return Err(WorkflowError::InvalidDefinition(
                    "step key cannot be empty".to_string(),
                ));
            }
            if !seen_keys.insert(step.key.clone()) {
                return Err(WorkflowError::DuplicateStep(step.key.clone()));
            }

            if step.name.trim().is_empty() {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "step '{}' name cannot be empty",
                    step.key
                )));
            }

            if step.prompt_template.trim().is_empty() {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "step '{}' prompt_template cannot be empty",
                    step.key
                )));
            }

            if step.timeout_secs == 0 {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "step '{}' timeout_secs must be greater than 0",
                    step.key
                )));
            }

            // Validate capabilities
            for cap in &step.required_capabilities {
                cap.validate().map_err(|e| {
                    WorkflowError::InvalidDefinition(format!(
                        "step '{}' has invalid capability '{}': {}",
                        step.key, cap.id, e
                    ))
                })?;
            }

            // Validate outputs and collect artifact names
            let mut step_artifact_names = HashSet::new();
            for output in &step.expected_outputs {
                if output.artifact_name.trim().is_empty() {
                    return Err(WorkflowError::InvalidBinding {
                        step: step.key.clone(),
                        binding: output.artifact_name.clone(),
                        reason: "output artifact name cannot be empty".to_string(),
                    });
                }
                if !step_artifact_names.insert(output.artifact_name.clone()) {
                    return Err(WorkflowError::InvalidBinding {
                        step: step.key.clone(),
                        binding: output.artifact_name.clone(),
                        reason: format!(
                            "duplicate output artifact name '{}' in step '{}'",
                            output.artifact_name, step.key
                        ),
                    });
                }
                validate_artifact_path(&output.relative_path, None)?;
            }
            step_outputs.insert(step.key.clone(), step_artifact_names);

            // Validate quality gate
            if let Some(ambiguity) = step.quality_gate.max_ambiguity_percent
                && ambiguity > 100
            {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "step '{}' quality_gate.max_ambiguity_percent must be <= 100, got {}",
                    step.key, ambiguity
                )));
            }
        }

        // 3. Validate dependencies and input bindings
        for step in &self.steps {
            let mut seen_deps = HashSet::new();
            for dep in &step.depends_on {
                if dep == &step.key {
                    return Err(WorkflowError::InvalidDefinition(format!(
                        "step '{}' cannot depend on itself",
                        step.key
                    )));
                }
                if !seen_keys.contains(dep) {
                    return Err(WorkflowError::MissingDependency {
                        step: step.key.clone(),
                        prerequisite: dep.clone(),
                    });
                }
                if !seen_deps.insert(dep.clone()) {
                    return Err(WorkflowError::InvalidDefinition(format!(
                        "step '{}' contains duplicate dependency on '{}'",
                        step.key, dep
                    )));
                }
            }

            // Validate input bindings against declared dependencies and outputs
            for input in &step.required_inputs {
                if input.parameter_name.trim().is_empty() {
                    return Err(WorkflowError::InvalidBinding {
                        step: step.key.clone(),
                        binding: input.parameter_name.clone(),
                        reason: "input binding parameter_name cannot be empty".to_string(),
                    });
                }
                if !seen_keys.contains(&input.source_step_key) {
                    return Err(WorkflowError::InvalidBinding {
                        step: step.key.clone(),
                        binding: input.parameter_name.clone(),
                        reason: format!(
                            "source_step_key '{}' does not exist in workflow",
                            input.source_step_key
                        ),
                    });
                }
                if !seen_deps.contains(&input.source_step_key) {
                    return Err(WorkflowError::InvalidBinding {
                        step: step.key.clone(),
                        binding: input.parameter_name.clone(),
                        reason: format!(
                            "source step '{}' must be declared in depends_on of step '{}'",
                            input.source_step_key, step.key
                        ),
                    });
                }

                // Check that source step actually declares the artifact
                if let Some(artifacts) = step_outputs.get(&input.source_step_key)
                    && !artifacts.contains(&input.artifact_name)
                    && !input.is_optional
                {
                    return Err(WorkflowError::InvalidBinding {
                        step: step.key.clone(),
                        binding: input.parameter_name.clone(),
                        reason: format!(
                            "source step '{}' does not declare expected output artifact '{}'",
                            input.source_step_key, input.artifact_name
                        ),
                    });
                }
            }
        }

        // 4. Validate acyclicity and topological order
        self.topological_order()?;

        Ok(())
    }

    /// Computes a deterministic topological ordering of the workflow steps.
    ///
    /// Uses Kahn's algorithm with a lexicographical tie-breaker (`BTreeSet`),
    /// guaranteeing deterministic ordering for parallel siblings.
    /// Returns `WorkflowError::CycleDetected` if a cycle is found.
    pub fn topological_order(&self) -> Result<Vec<String>, WorkflowError> {
        let keys: Vec<String> = self.steps.iter().map(|s| s.key.clone()).collect();
        let edges: Vec<(String, String)> = self
            .steps
            .iter()
            .flat_map(|s| s.depends_on.iter().map(|dep| (dep.clone(), s.key.clone())))
            .collect();

        crate::dag::ops::topological_sort(keys, edges).map_err(|err| match err {
            crate::dag::ops::GraphError::CycleDetected { nodes, .. } => {
                WorkflowError::CycleDetected { cycle: nodes }
            }
            other => WorkflowError::InvalidDefinition(other.to_string()),
        })
    }

    /// Retrieve a step definition by key.
    pub fn get_step(&self, key: &str) -> Option<&WorkflowStepDefinition> {
        self.steps.iter().find(|s| s.key == key)
    }
}

/// Strictly validates an artifact path before persistence or binding registration.
///
/// Enforces:
/// 1. Path is relative (absolute paths rejected)
/// 2. No directory traversal (`..` components rejected)
/// 3. Cannot point inside internal directories (`.m31a`, `.git`)
/// 4. When `workspace_root` is provided, resolves and verifies path containment.
pub fn validate_artifact_path(
    path: &Path,
    workspace_root: Option<&Path>,
) -> Result<PathBuf, WorkflowError> {
    let path_str = path.to_string_lossy();
    if path_str.trim().is_empty() {
        return Err(WorkflowError::InvalidArtifactPath {
            path: path_str.into_owned(),
            reason: "artifact path cannot be empty".to_string(),
        });
    }

    // 1. Must be relative
    if path.is_absolute() {
        return Err(WorkflowError::InvalidArtifactPath {
            path: path_str.into_owned(),
            reason: "artifact path must be relative, not absolute".to_string(),
        });
    }

    // 2. Lexical component inspection
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                return Err(WorkflowError::InvalidArtifactPath {
                    path: path_str.into_owned(),
                    reason: "path traversal ('..') is prohibited in artifact paths".to_string(),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(WorkflowError::InvalidArtifactPath {
                    path: path_str.into_owned(),
                    reason: "artifact path must be relative without root or prefix components"
                        .to_string(),
                });
            }
            Component::Normal(c) => {
                let s = c.to_string_lossy();
                if s == ".m31a" || s == ".git" {
                    return Err(WorkflowError::InvalidArtifactPath {
                        path: path_str.into_owned(),
                        reason: format!(
                            "artifact path cannot target internal runtime directory '{}'",
                            s
                        ),
                    });
                }
            }
            Component::CurDir => {}
        }
    }

    // 3. Workspace root confinement check if provided
    if let Some(root) = workspace_root {
        let full_path = root.join(path);
        let mut normalized = PathBuf::new();
        for comp in full_path.components() {
            match comp {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::CurDir => {}
                c => normalized.push(c.as_os_str()),
            }
        }
        if !normalized.starts_with(root) {
            return Err(WorkflowError::InvalidArtifactPath {
                path: path_str.into_owned(),
                reason: format!("artifact path escapes workspace root '{}'", root.display()),
            });
        }
    }

    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::plan::CapabilityAccessMode;

    fn sample_step(key: &str, deps: Vec<&str>) -> WorkflowStepDefinition {
        WorkflowStepDefinition {
            key: key.to_string(),
            name: format!("Step {}", key),
            role: AgentRole::researcher(),
            prompt_template: format!("prompts/{}", key),
            required_inputs: Vec::new(),
            expected_outputs: vec![OutputBinding {
                artifact_name: format!("{}.md", key),
                relative_path: PathBuf::from(format!("artifacts/{}.md", key)),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![CapabilityRequirement::new(
                "fs.read",
                CapabilityAccessMode::Read,
            )],
            quality_gate: QualityGate::default(),
            depends_on: deps.into_iter().map(String::from).collect(),
            timeout_secs: 300,
            allows_parallelism: true,
            recovery_strategy: None,
            prompt_ref: None,
        }
    }

    #[test]
    fn test_valid_linear_workflow() {
        let def = WorkflowDefinition {
            id: "test-linear".to_string(),
            name: "Test Linear".to_string(),
            description: "A linear test workflow".to_string(),
            version: 1,
            steps: vec![
                sample_step("step1", vec![]),
                sample_step("step2", vec!["step1"]),
                sample_step("step3", vec!["step2"]),
            ],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        assert!(def.validate().is_ok());
        let order = def.topological_order().unwrap();
        assert_eq!(order, vec!["step1", "step2", "step3"]);
    }

    #[test]
    fn test_parallel_siblings_topological_order() {
        let def = WorkflowDefinition {
            id: "test-parallel".to_string(),
            name: "Test Parallel".to_string(),
            description: "Parallel siblings workflow".to_string(),
            version: 1,
            steps: vec![
                sample_step("beta", vec![]),
                sample_step("alpha", vec![]),
                sample_step("gamma", vec!["alpha", "beta"]),
            ],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        assert!(def.validate().is_ok());
        let order = def.topological_order().unwrap();
        // Deterministic alphabetical ordering for siblings
        assert_eq!(order, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_converging_dependencies() {
        let def = WorkflowDefinition {
            id: "test-diamond".to_string(),
            name: "Test Diamond".to_string(),
            description: "Diamond dependency workflow".to_string(),
            version: 1,
            steps: vec![
                sample_step("root", vec![]),
                sample_step("left", vec!["root"]),
                sample_step("right", vec!["root"]),
                sample_step("sink", vec!["left", "right"]),
            ],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        assert!(def.validate().is_ok());
        let order = def.topological_order().unwrap();
        assert_eq!(order, vec!["root", "left", "right", "sink"]);
    }

    #[test]
    fn test_cycle_detection() {
        let def = WorkflowDefinition {
            id: "test-cycle".to_string(),
            name: "Test Cycle".to_string(),
            description: "Cyclic workflow".to_string(),
            version: 1,
            steps: vec![
                sample_step("step1", vec!["step3"]),
                sample_step("step2", vec!["step1"]),
                sample_step("step3", vec!["step2"]),
            ],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        let err = def.validate().unwrap_err();
        match err {
            WorkflowError::CycleDetected { cycle } => {
                assert_eq!(cycle.len(), 3);
            }
            other => panic!("expected CycleDetected, got {:?}", other),
        }
    }

    #[test]
    fn test_self_dependency_rejected() {
        let def = WorkflowDefinition {
            id: "test-self".to_string(),
            name: "Test Self".to_string(),
            description: "Self loop".to_string(),
            version: 1,
            steps: vec![sample_step("step1", vec!["step1"])],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        assert!(def.validate().is_err());
    }

    #[test]
    fn test_missing_dependency_rejected() {
        let def = WorkflowDefinition {
            id: "test-missing".to_string(),
            name: "Test Missing".to_string(),
            description: "Missing dep".to_string(),
            version: 1,
            steps: vec![sample_step("step1", vec!["nonexistent"])],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        let err = def.validate().unwrap_err();
        match err {
            WorkflowError::MissingDependency { step, prerequisite } => {
                assert_eq!(step, "step1");
                assert_eq!(prerequisite, "nonexistent");
            }
            other => panic!("expected MissingDependency, got {:?}", other),
        }
    }

    #[test]
    fn test_duplicate_step_key_rejected() {
        let def = WorkflowDefinition {
            id: "test-dup".to_string(),
            name: "Test Dup".to_string(),
            description: "Duplicate step key".to_string(),
            version: 1,
            steps: vec![sample_step("step1", vec![]), sample_step("step1", vec![])],
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        let err = def.validate().unwrap_err();
        assert!(matches!(err, WorkflowError::DuplicateStep(_)));
    }

    #[test]
    fn test_artifact_path_validation() {
        // Valid paths
        assert!(validate_artifact_path(Path::new("valid/project/file.md"), None).is_ok());
        assert!(validate_artifact_path(Path::new("PROJECT.md"), None).is_ok());
        assert!(validate_artifact_path(Path::new("research/STACK.md"), None).is_ok());

        // Traversal rejection
        assert!(validate_artifact_path(Path::new("../etc/passwd"), None).is_err());
        assert!(validate_artifact_path(Path::new("../../secret"), None).is_err());
        assert!(validate_artifact_path(Path::new("a/b/../../secret"), None).is_err());

        // Absolute path rejection
        assert!(validate_artifact_path(Path::new("/etc/passwd"), None).is_err());

        // Internal directories rejection
        assert!(validate_artifact_path(Path::new(".m31a/m31a.db"), None).is_err());
        assert!(validate_artifact_path(Path::new("subdir/.m31a/data"), None).is_err());
        assert!(validate_artifact_path(Path::new(".git/config"), None).is_err());
    }
}
