//! Deterministic workflow manifest compiler resolving prompt contracts and attaching provenance.

use crate::dag::validator::{GraphValidationError, TaskGraphValidator};
use crate::ids::MissionId;
use crate::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use crate::prompt::{PromptCatalog, PromptReference};
use crate::state_machine::agent::AgentRole;
use crate::workflow::definition::{
    InputBinding, OutputBinding, QualityGate, WorkflowDefinition, WorkflowStepDefinition,
};
use crate::workflow::error::WorkflowError;
use crate::workflow::manifest::WorkflowManifest;
use crate::workflow::provenance::{StepProvenance, WorkflowProvenance};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// A fully compiled, validated, runtime-ready workflow contract with attached provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledWorkflow {
    /// Domain workflow definition conforming to workflow definition contracts.
    pub definition: WorkflowDefinition,
    /// Cryptographic origin provenance.
    pub provenance: WorkflowProvenance,
    /// Deterministic execution order computed via Kahn's algorithm.
    pub topological_order: Vec<String>,
}

impl CompiledWorkflow {
    /// Construct a runtime-ready `CompiledWorkflow` from a validated domain `WorkflowDefinition`.
    pub fn from_definition(definition: WorkflowDefinition) -> Result<Self, WorkflowError> {
        definition.validate()?;
        let topological_order = definition.topological_order()?;
        let mut hasher = Sha256::new();
        hasher.update(definition.id.as_bytes());
        hasher.update(definition.version.to_le_bytes());
        let manifest_hash = format!("{:x}", hasher.finalize());

        let provenance = WorkflowProvenance::new(
            definition.id.clone(),
            definition.version,
            manifest_hash,
            1,
            BTreeMap::new(),
            None,
            Utc::now(),
        );

        Ok(Self {
            definition,
            provenance,
            topological_order,
        })
    }

    /// Map a step definition's outputs and quality gate to a canonical VerificationStrategy.
    pub fn map_step_verification(step: &WorkflowStepDefinition) -> VerificationStrategy {
        let has_artifacts = !step.expected_outputs.is_empty();
        let has_human_approval = step.quality_gate.require_human_approval;

        let artifact_strat = if has_artifacts {
            let paths: Vec<String> = step
                .expected_outputs
                .iter()
                .map(|o| o.relative_path.to_string_lossy().to_string())
                .collect();
            Some(VerificationStrategy::ArtifactInspection { paths })
        } else {
            None
        };

        let review_strat = if has_human_approval {
            Some(VerificationStrategy::ReviewGate {
                reviewer_role: Some(AgentRole::reviewer()),
            })
        } else {
            None
        };

        match (artifact_strat, review_strat) {
            (Some(art), Some(rev)) => {
                VerificationStrategy::composite(vec![art.clone(), rev]).unwrap_or(art)
            }
            (Some(art), None) => art,
            (None, Some(rev)) => rev,
            // Role-declared verification default (registry data, not a
            // role table). Registration is enforced explicitly at lowering
            // time (`profile_for_global` errors on unregistered roles), so
            // this fallback is unreachable through production paths; the
            // empty inspection default is a safe terminal value only.
            (None, None) => crate::agent::registry::RoleRegistry::global()
                .read()
                .ok()
                .and_then(|guard| {
                    guard
                        .resolve(&step.role)
                        .map(|def| def.default_verification.clone())
                })
                .unwrap_or(VerificationStrategy::ArtifactInspection { paths: Vec::new() }),
        }
    }

    /// Compute the set of step keys that are unblocked from execution given the set of approved step keys.
    ///
    /// A step is blocked if any of its ancestors requires human approval and has not yet been approved.
    pub fn compute_unblocked_step_keys(
        &self,
        approved_keys: &std::collections::HashSet<String>,
    ) -> std::collections::HashSet<String> {
        let mut blocked = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        for step in &self.definition.steps {
            if step.quality_gate.require_human_approval && !approved_keys.contains(&step.key) {
                for other in &self.definition.steps {
                    if other.depends_on.contains(&step.key) && blocked.insert(other.key.clone()) {
                        queue.push_back(other.key.clone());
                    }
                }
            }
        }

        while let Some(blocked_key) = queue.pop_front() {
            for other in &self.definition.steps {
                if other.depends_on.contains(&blocked_key) && blocked.insert(other.key.clone()) {
                    queue.push_back(other.key.clone());
                }
            }
        }

        self.definition
            .steps
            .iter()
            .filter(|s| !blocked.contains(&s.key))
            .map(|s| s.key.clone())
            .collect()
    }

    /// Lower this compiled workflow into a canonical Mission DAG and CandidatePlan (AD-003).
    ///
    /// # Invariants
    /// - Fails closed: validates the DAG structure defensively using `TaskGraphValidator`.
    /// - Reject any cycles, self-loops, missing dependencies, or empty plans.
    /// - Preserves 1:1 step mapping with deterministic `CandidateTaskKey` IDs.
    pub fn lower(&self, mission_id: MissionId) -> Result<LoweredWorkflow, WorkflowError> {
        self.lower_subset(mission_id, None, 0)
    }

    /// Lower a subset of steps in this compiled workflow into a canonical CandidatePlan.
    pub fn lower_subset(
        &self,
        mission_id: MissionId,
        included_step_keys: Option<&std::collections::HashSet<String>>,
        revision: u32,
    ) -> Result<LoweredWorkflow, WorkflowError> {
        self.definition.validate()?;

        let mut candidate_tasks = Vec::with_capacity(self.definition.steps.len());
        let mut step_task_keys = BTreeMap::new();

        for step in &self.definition.steps {
            if let Some(keys) = included_step_keys
                && !keys.contains(&step.key)
            {
                continue;
            }

            let task_key = CandidateTaskKey::new(&step.key);
            step_task_keys.insert(step.key.clone(), task_key.clone());

            let verification = Self::map_step_verification(step);
            let timeout = if step.timeout_secs == 0 {
                300
            } else {
                step.timeout_secs
            };
            let estimates = ResourceEstimate::new(10, timeout, 50_000, 1.0);

            let mut capabilities = step.required_capabilities.clone();
            if capabilities.is_empty() {
                let profile = crate::agent::registry::RoleRegistry::profile_for_global(&step.role)
                    .map_err(|e| {
                        WorkflowError::InvalidDefinition(format!(
                            "step '{}' references {}: {}",
                            step.key,
                            step.role.as_str(),
                            e
                        ))
                    })?;
                for cap in profile.capability_policy.allowed_capabilities {
                    capabilities.push(CapabilityRequirement::new(
                        cap,
                        CapabilityAccessMode::ReadWrite,
                    ));
                }
            }
            let role_tag = format!("role:{}", step.role.to_string().to_lowercase());
            if !capabilities.iter().any(|c| c.id == role_tag) {
                capabilities.push(CapabilityRequirement::new(
                    role_tag,
                    CapabilityAccessMode::Read,
                ));
            }

            let mut task = CandidateTask::new(
                task_key,
                step.name.clone(),
                step.role.clone(),
                verification,
                estimates,
            );
            // Typed prompt execution binding: the workflow-selected prompt
            // flows into the candidate task as structured authority state.
            // The description carries ONLY human-readable step identity —
            // never the prompt identifier (metadata-only prompt ids are a
            // wiring defect: embedding is not consumption).
            let step_prompt_ref = step.effective_prompt_ref().map_err(|_| {
                WorkflowError::PromptNotFound {
                    id: step.prompt_template.clone(),
                    version: 0,
                }
            })?;
            task = task.with_prompt_ref(step_prompt_ref);
            task.description = Some(format!("Step {}: {}", step.key, step.name));
            task.depends_on = step
                .depends_on
                .iter()
                .filter(|dep| {
                    if let Some(keys) = included_step_keys {
                        keys.contains(*dep)
                    } else {
                        true
                    }
                })
                .map(CandidateTaskKey::new)
                .collect();
            task.capabilities = capabilities;

            candidate_tasks.push(task);
        }

        let plan_id = if revision == 0 {
            format!("plan-{}", mission_id)
        } else {
            format!("plan-{}-{}", mission_id, revision)
        };
        let plan_objective = if candidate_tasks.len() == 1 {
            candidate_tasks[0].objective.clone()
        } else {
            self.definition.name.clone()
        };

        let candidate_plan = CandidatePlan::new(plan_id, plan_objective, candidate_tasks);

        // Defensive DAG validation via TaskGraphValidator
        let validator = TaskGraphValidator::new();
        validator
            .validate_candidate_plan(&candidate_plan)
            .map_err(|err| match err {
                GraphValidationError::CycleDetected => WorkflowError::CycleDetected {
                    cycle: vec!["cyclic dependency detected in candidate plan".to_string()],
                },
                GraphValidationError::EmptyGraph => WorkflowError::InvalidDefinition(
                    "Workflow candidate plan contains no tasks".to_string(),
                ),
                GraphValidationError::MissingEndpoint { task_id } => {
                    WorkflowError::InvalidDefinition(format!(
                        "Missing prerequisite endpoint: {}",
                        task_id
                    ))
                }
                GraphValidationError::SelfLoop { task_id } => WorkflowError::InvalidDefinition(
                    format!("Self-loop detected on task: {}", task_id),
                ),
                GraphValidationError::DuplicateEdge { from, to } => {
                    WorkflowError::InvalidDefinition(format!(
                        "Duplicate edge from {} to {}",
                        from, to
                    ))
                }
                GraphValidationError::InvalidPlan { reason } => {
                    WorkflowError::InvalidDefinition(format!("Invalid candidate plan: {}", reason))
                }
            })?;

        Ok(LoweredWorkflow {
            mission_id,
            candidate_plan,
            step_task_keys,
        })
    }

    /// Lower this compiled workflow using a freshly allocated `MissionId`.
    pub fn lower_default(&self) -> Result<LoweredWorkflow, WorkflowError> {
        self.lower(MissionId::new())
    }
}

/// Lowered representation of a declarative workflow compiled into a canonical Mission DAG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoweredWorkflow {
    /// Authoritative Mission identifier.
    pub mission_id: MissionId,
    /// Canonical candidate plan DAG ready for materialization into SchedulerEngine.
    pub candidate_plan: CandidatePlan,
    /// Mapping of step keys to their authoritative CandidateTaskKeys.
    pub step_task_keys: BTreeMap<String, CandidateTaskKey>,
}

/// Compiler transforming declarative `WorkflowManifest` into a runtime-ready `CompiledWorkflow`.
pub struct WorkflowCompiler<'a> {
    catalog: &'a dyn PromptCatalog,
    strict_roles: bool,
}

impl<'a> WorkflowCompiler<'a> {
    /// Construct a new compiler backed by a prompt catalog.
    pub fn new(catalog: &'a dyn PromptCatalog) -> Self {
        Self {
            catalog,
            strict_roles: true,
        }
    }

    /// Configure whether role compatibility between steps and prompt contracts is strictly enforced.
    pub fn with_strict_roles(mut self, strict: bool) -> Self {
        self.strict_roles = strict;
        self
    }

    /// Compile a workflow manifest into a validated `CompiledWorkflow`.
    ///
    /// # Invariants
    /// - Fails closed: malformed manifests or missing prompt contracts reject immediately.
    /// - Never emits partial definitions.
    /// - Output is completely deterministic.
    pub fn compile(
        &self,
        manifest: &WorkflowManifest,
        source_path: Option<&str>,
        workspace_root: Option<&Path>,
    ) -> Result<CompiledWorkflow, WorkflowError> {
        // 1. Strict manifest validation
        manifest.validate(workspace_root)?;

        // 2. Resolve prompt contracts and check role compatibility
        let mut step_provenance = BTreeMap::new();
        let mut domain_steps = Vec::new();

        for step_manifest in &manifest.steps {
            // Resolve prompt reference (either parsed from string or combining prompt + prompt_version)
            let prompt_ref = if let Some(v) = step_manifest.prompt_version {
                PromptReference::new(&step_manifest.prompt, v)
            } else {
                PromptReference::parse(&step_manifest.prompt)?
            };

            // Query catalog
            let prompt_contract = self
                .catalog
                .get(&prompt_ref.id, prompt_ref.version)
                .map_err(|_| WorkflowError::PromptNotFound {
                    id: prompt_ref.id.clone(),
                    version: prompt_ref.version,
                })?;

            // Validate role compatibility via the registry. Unregistered
            // step roles are never compatible: existence is enforced here,
            // not assumed.
            let roles_compatible = crate::agent::registry::RoleRegistry::global()
                .read()
                .map(|guard| guard.is_compatible(&step_manifest.role, &prompt_contract.role))
                .unwrap_or(false);
            if self.strict_roles && !roles_compatible {
                return Err(WorkflowError::RoleMismatch {
                    step_key: step_manifest.key.clone(),
                    step_role: step_manifest.role.to_string(),
                    prompt_id: prompt_contract.id.clone(),
                    prompt_role: prompt_contract.role.to_string(),
                });
            }

            // Record provenance for this step
            step_provenance.insert(
                step_manifest.key.clone(),
                StepProvenance::new(
                    step_manifest.key.clone(),
                    prompt_contract.id.clone(),
                    prompt_contract.version,
                    prompt_contract.content_hash.clone(),
                    step_manifest.role.clone(),
                ),
            );

            // Convert inputs
            let required_inputs: Vec<InputBinding> = step_manifest
                .required_inputs
                .iter()
                .map(|inp| InputBinding {
                    parameter_name: inp.parameter_name.clone(),
                    source_step_key: inp.source_step_key.clone(),
                    artifact_name: inp.artifact_name.clone(),
                    is_optional: inp.is_optional,
                })
                .collect();

            // Convert outputs
            let expected_outputs: Vec<OutputBinding> = step_manifest
                .expected_outputs
                .iter()
                .map(|out| OutputBinding {
                    artifact_name: out.artifact_name.clone(),
                    relative_path: out.relative_path.clone(),
                    schema_type: out.schema_type.clone(),
                })
                .collect();

            // Convert quality gate
            let quality_gate = if let Some(gate) = &step_manifest.quality_gate {
                QualityGate {
                    schema: gate.schema.clone(),
                    required_artifacts: gate.required_artifacts.clone(),
                    require_human_approval: gate.require_human_approval,
                    max_ambiguity_percent: gate.max_ambiguity_percent,
                }
            } else {
                QualityGate::default()
            };

            // Build domain step specification. The typed prompt reference is
            // the first-class execution binding: it survives lowering into
            // the candidate task, the durable task record, and worker
            // context. `prompt_template` remains as a human-readable label
            // only and MUST NOT be re-parsed downstream.
            domain_steps.push(WorkflowStepDefinition {
                key: step_manifest.key.clone(),
                name: step_manifest.name.clone(),
                role: step_manifest.role.clone(),
                prompt_template: prompt_ref.to_string(),
                prompt_ref: Some(prompt_ref.clone()),
                required_inputs,
                expected_outputs,
                required_capabilities: step_manifest.required_capabilities.clone(),
                quality_gate,
                depends_on: step_manifest.depends_on.clone(),
                timeout_secs: step_manifest.timeout_secs,
                allows_parallelism: step_manifest.allows_parallelism,
                recovery_strategy: step_manifest.recovery_strategy.clone(),
            });
        }

        // 3. Assemble domain WorkflowDefinition
        let definition = WorkflowDefinition {
            id: manifest.workflow.id.clone(),
            name: manifest.workflow.name.clone(),
            description: manifest.workflow.description.clone(),
            version: manifest.workflow.version,
            steps: domain_steps,
            default_recovery_strategy: manifest.workflow.default_recovery_strategy.clone(),
        };

        // 4. Validate domain definition contracts and compute topological ordering
        definition.validate()?;
        let topological_order = definition.topological_order()?;

        // 5. Compute deterministic manifest hash
        let manifest_toml =
            toml::to_string(manifest).map_err(|e| WorkflowError::ManifestParse(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(manifest_toml.as_bytes());
        let manifest_hash = format!("{:x}", hasher.finalize());

        // 6. Build provenance record
        let provenance = WorkflowProvenance::new(
            manifest.workflow.id.clone(),
            manifest.manifest_version,
            manifest_hash,
            manifest.workflow.version,
            step_provenance,
            source_path.map(|s| s.to_string()),
            Utc::now(),
        );

        Ok(CompiledWorkflow {
            definition,
            provenance,
            topological_order,
        })
    }

    /// Compile and lower a workflow manifest into a canonical Mission DAG in one pass (AD-003).
    pub fn lower(
        &self,
        manifest: &WorkflowManifest,
        source_path: Option<&str>,
        workspace_root: Option<&Path>,
        mission_id: MissionId,
    ) -> Result<LoweredWorkflow, WorkflowError> {
        let compiled = self.compile(manifest, source_path, workspace_root)?;
        compiled.lower(mission_id)
    }
}
