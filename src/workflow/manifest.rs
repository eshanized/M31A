//! Declarative TOML workflow manifest parser, schema models, and strict validation.

use crate::kernel::plan::CapabilityRequirement;
use crate::state_machine::agent::AgentRole;
use crate::workflow::definition::{RecoveryStrategy, validate_artifact_path};
use crate::workflow::error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

/// Maximum permissible size for a workflow manifest file (1 MB).
pub const MAX_MANIFEST_SIZE_BYTES: u64 = 1_048_576;

/// Supported workflow manifest version.
pub const CURRENT_MANIFEST_VERSION: u32 = 1;

fn default_manifest_version() -> u32 {
    CURRENT_MANIFEST_VERSION
}

fn default_workflow_version() -> u32 {
    1
}

fn default_step_timeout() -> u64 {
    crate::config::canonical::DEFAULT_WORKFLOW_STEP_TIMEOUT_SECS
}

pub use crate::prompt::contract::{deserialize_flexible_role, parse_role_flexible};

/// Dynamic input binding manifest connecting an upstream step artifact to a downstream step parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBindingManifest {
    /// Parameter name passed to prompt rendering or execution context.
    pub parameter_name: String,
    /// Step key of the upstream step that produces the artifact.
    pub source_step_key: String,
    /// Name of the artifact emitted by the source step.
    pub artifact_name: String,
    /// Whether the absence of the upstream artifact is permissible.
    #[serde(default)]
    pub is_optional: bool,
}

/// Output binding manifest declaring an artifact produced by a workflow step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputBindingManifest {
    /// Logical name of the artifact (e.g. "PROJECT.md", "SUMMARY.md").
    pub artifact_name: String,
    /// Relative path within the project workspace or projection directory.
    pub relative_path: PathBuf,
    /// Expected schema or format identifier (e.g. "markdown", "json").
    pub schema_type: String,
}

/// Quality gate manifest evaluated before accepting a step run as Completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QualityGateManifest {
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

/// Manifest specification for a single discrete step in a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepManifest {
    /// Unique identifier for this step within the workflow (e.g. "discovery").
    pub key: String,
    /// Human-readable name of this step.
    pub name: String,
    /// Canonical agent role assigned to execute this step.
    #[serde(deserialize_with = "deserialize_flexible_role")]
    pub role: AgentRole,
    /// Prompt template identifier or reference (e.g. "genesis.discovery.v1" or "discovery").
    pub prompt: String,
    /// Explicit version number of the prompt contract (if not embedded in prompt reference).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<u32>,
    /// Upstream artifact bindings required as inputs.
    #[serde(default, alias = "inputs", skip_serializing_if = "Vec::is_empty")]
    pub required_inputs: Vec<InputBindingManifest>,
    /// Artifacts expected to be produced upon step completion.
    #[serde(default, alias = "outputs", skip_serializing_if = "Vec::is_empty")]
    pub expected_outputs: Vec<OutputBindingManifest>,
    /// Capability requirements granted to the agent for this step.
    #[serde(default, alias = "capabilities", skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<CapabilityRequirement>,
    /// Quality gate requirements that must pass before step completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_gate: Option<QualityGateManifest>,
    /// Keys of steps that must complete before this step can execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Execution timeout in seconds.
    #[serde(default = "default_step_timeout")]
    pub timeout_secs: u64,
    /// Whether this step can execute concurrently with other independent steps.
    #[serde(default)]
    pub allows_parallelism: bool,
    /// Recovery strategy override for this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_strategy: Option<RecoveryStrategy>,
}

/// Metadata header for a workflow manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowHeaderManifest {
    /// Unique identifier of the workflow (e.g. "genesis-greenfield").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Long-form description.
    #[serde(default)]
    pub description: String,
    /// Version number of the workflow definition.
    #[serde(default = "default_workflow_version")]
    pub version: u32,
    /// Default recovery strategy for steps that do not override it.
    #[serde(default)]
    pub default_recovery_strategy: RecoveryStrategy,
}

/// Root document representing a declarative workflow manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowManifest {
    /// Explicit manifest file format version.
    #[serde(default = "default_manifest_version")]
    pub manifest_version: u32,
    /// High-level workflow metadata.
    pub workflow: WorkflowHeaderManifest,
    /// Ordered step specifications.
    pub steps: Vec<WorkflowStepManifest>,
}

impl WorkflowManifest {
    /// Parse a workflow manifest from a TOML string.
    pub fn from_toml_str(toml_str: &str) -> Result<Self, WorkflowError> {
        let manifest: Self =
            toml::from_str(toml_str).map_err(|e| WorkflowError::ManifestParse(e.to_string()))?;
        Ok(manifest)
    }

    /// Safely load and validate a workflow manifest from the filesystem.
    ///
    /// # Security Guarantees
    /// - Rejects paths containing `..` or leading slashes outside workspace root.
    /// - Refuses to load files inside `.git` or `.m31a`.
    /// - Enforces maximum file size of 1 MB.
    /// - Strictly validates UTF-8.
    pub fn from_file(path: &Path, workspace_root: Option<&Path>) -> Result<Self, WorkflowError> {
        // 1. Path safety check
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    return Err(WorkflowError::PathViolation {
                        path: path.display().to_string(),
                        reason: "path cannot contain parent directory traversal '..'".to_string(),
                    });
                }
                Component::Normal(c) => {
                    let s = c.to_string_lossy();
                    if s == ".git" || s == ".m31a" {
                        return Err(WorkflowError::PathViolation {
                            path: path.display().to_string(),
                            reason: format!(
                                "refusing to load manifest from protected directory '{}'",
                                s
                            ),
                        });
                    }
                }
                _ => {}
            }
        }

        // 2. Workspace containment check
        let canonical_path = if let Some(root) = workspace_root {
            let combined = if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            };
            if !combined.starts_with(root) {
                return Err(WorkflowError::PathViolation {
                    path: path.display().to_string(),
                    reason: format!("path escapes workspace root '{}'", root.display()),
                });
            }
            combined
        } else {
            path.to_path_buf()
        };

        // 3. File metadata & size limit check
        let metadata =
            std::fs::metadata(&canonical_path).map_err(|e| WorkflowError::ManifestValidation {
                path: Some(path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to read manifest file metadata: {}", e),
            })?;

        if metadata.len() > MAX_MANIFEST_SIZE_BYTES {
            return Err(WorkflowError::ManifestValidation {
                path: Some(path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!(
                    "manifest file size {} bytes exceeds maximum allowed limit of {} bytes",
                    metadata.len(),
                    MAX_MANIFEST_SIZE_BYTES
                ),
            });
        }

        // 4. Read contents and parse
        let content_bytes =
            std::fs::read(&canonical_path).map_err(|e| WorkflowError::ManifestValidation {
                path: Some(path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to read manifest file: {}", e),
            })?;

        let content_str = std::str::from_utf8(&content_bytes).map_err(|e| {
            WorkflowError::ManifestParse(format!("manifest is not valid UTF-8: {}", e))
        })?;

        let manifest = Self::from_toml_str(content_str)?;
        manifest.validate(workspace_root)?;
        Ok(manifest)
    }

    /// Perform strict semantic and structural validation of the workflow manifest.
    pub fn validate(&self, workspace_root: Option<&Path>) -> Result<(), WorkflowError> {
        let wf_id = &self.workflow.id;

        // 1. Manifest version check
        if self.manifest_version != CURRENT_MANIFEST_VERSION {
            return Err(WorkflowError::UnsupportedVersion {
                kind: "manifest".to_string(),
                version: self.manifest_version,
                supported: CURRENT_MANIFEST_VERSION.to_string(),
            });
        }

        // 2. Workflow header validation
        if wf_id.trim().is_empty() {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: Some(wf_id.clone()),
                step_key: None,
                field: Some("workflow.id".to_string()),
                reason: "workflow id cannot be empty or whitespace".to_string(),
            });
        }

        if !wf_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: Some(wf_id.clone()),
                step_key: None,
                field: Some("workflow.id".to_string()),
                reason: "workflow id must contain only ASCII alphanumeric, '-', or '_' characters"
                    .to_string(),
            });
        }

        if self.workflow.name.trim().is_empty() {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: Some(wf_id.clone()),
                step_key: None,
                field: Some("workflow.name".to_string()),
                reason: "workflow name cannot be empty or whitespace".to_string(),
            });
        }

        if self.workflow.version == 0 {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: Some(wf_id.clone()),
                step_key: None,
                field: Some("workflow.version".to_string()),
                reason: "workflow version must be >= 1".to_string(),
            });
        }

        // 3. Step list validation
        if self.steps.is_empty() {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: Some(wf_id.clone()),
                step_key: None,
                field: Some("steps".to_string()),
                reason: "workflow must declare at least one step".to_string(),
            });
        }

        let mut seen_step_keys = HashSet::new();
        let mut step_key_map = HashSet::new();
        for step in &self.steps {
            let key = &step.key;
            if key.trim().is_empty() {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(key.clone()),
                    field: Some("key".to_string()),
                    reason: "step key cannot be empty or whitespace".to_string(),
                });
            }

            if !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(key.clone()),
                    field: Some("key".to_string()),
                    reason: "step key must contain only ASCII alphanumeric, '-', or '_' characters"
                        .to_string(),
                });
            }

            if !seen_step_keys.insert(key) {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(key.clone()),
                    field: Some("key".to_string()),
                    reason: format!("duplicate step key '{}'", key),
                });
            }

            step_key_map.insert(key.clone());
        }

        // 4. Per-step validation
        for step in &self.steps {
            let step_key = &step.key;

            if step.name.trim().is_empty() {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(step_key.clone()),
                    field: Some("name".to_string()),
                    reason: "step name cannot be empty".to_string(),
                });
            }

            if step.prompt.trim().is_empty() {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(step_key.clone()),
                    field: Some("prompt".to_string()),
                    reason: "prompt reference cannot be empty".to_string(),
                });
            }

            if step.timeout_secs == 0 || step.timeout_secs > 86400 {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(step_key.clone()),
                    field: Some("timeout_secs".to_string()),
                    reason: "timeout_secs must be between 1 and 86400 (24h)".to_string(),
                });
            }

            // Dependency checks
            let mut seen_deps = HashSet::new();
            for dep in &step.depends_on {
                if dep == step_key {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("depends_on".to_string()),
                        reason: format!("step '{}' cannot depend on itself", step_key),
                    });
                }

                if !step_key_map.contains(dep) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("depends_on".to_string()),
                        reason: format!("dependency '{}' not found in workflow steps", dep),
                    });
                }

                if !seen_deps.insert(dep) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("depends_on".to_string()),
                        reason: format!("duplicate dependency '{}'", dep),
                    });
                }
            }

            // Output binding checks
            let mut seen_outputs = HashSet::new();
            for out in &step.expected_outputs {
                if out.artifact_name.trim().is_empty() {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("expected_outputs.artifact_name".to_string()),
                        reason: "output artifact name cannot be empty".to_string(),
                    });
                }

                if !seen_outputs.insert(&out.artifact_name) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("expected_outputs.artifact_name".to_string()),
                        reason: format!(
                            "duplicate output artifact name '{}' in step '{}'",
                            out.artifact_name, step_key
                        ),
                    });
                }

                validate_artifact_path(&out.relative_path, workspace_root)?;

                if out.schema_type.trim().is_empty() {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("expected_outputs.schema_type".to_string()),
                        reason: "output schema_type cannot be empty".to_string(),
                    });
                }
            }

            // Input binding checks
            let mut seen_input_params = HashSet::new();
            for inp in &step.required_inputs {
                if inp.parameter_name.trim().is_empty() {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("required_inputs.parameter_name".to_string()),
                        reason: "input parameter_name cannot be empty".to_string(),
                    });
                }

                if !seen_input_params.insert(&inp.parameter_name) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("required_inputs.parameter_name".to_string()),
                        reason: format!(
                            "duplicate input parameter '{}' in step '{}'",
                            inp.parameter_name, step_key
                        ),
                    });
                }

                if !step_key_map.contains(&inp.source_step_key) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("required_inputs.source_step_key".to_string()),
                        reason: format!(
                            "input binding references non-existent source step '{}'",
                            inp.source_step_key
                        ),
                    });
                }

                if !step.depends_on.contains(&inp.source_step_key) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("required_inputs.source_step_key".to_string()),
                        reason: format!(
                            "step '{}' inputs from step '{}' without declaring it in depends_on",
                            step_key, inp.source_step_key
                        ),
                    });
                }
            }

            // Quality gate check
            if let Some(gate) = &step.quality_gate
                && let Some(pct) = gate.max_ambiguity_percent
                && pct > 100
            {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(wf_id.clone()),
                    step_key: Some(step_key.clone()),
                    field: Some("quality_gate.max_ambiguity_percent".to_string()),
                    reason: format!("max_ambiguity_percent {} cannot exceed 100", pct),
                });
            }

            // Capability validation
            for cap in &step.required_capabilities {
                cap.validate()
                    .map_err(|e| WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(wf_id.clone()),
                        step_key: Some(step_key.clone()),
                        field: Some("required_capabilities".to_string()),
                        reason: format!("invalid capability requirement: {}", e),
                    })?;
            }
        }

        // 5. Acyclicity validation via Kahn's algorithm
        self.detect_cycles()?;

        Ok(())
    }

    /// Detect circular dependencies using canonical DAG algorithm authority.
    fn detect_cycles(&self) -> Result<(), WorkflowError> {
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
        })?;

        Ok(())
    }
}
