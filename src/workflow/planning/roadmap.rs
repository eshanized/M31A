//! Dependency-aware Roadmap DAG compilation, phase sequencing, and ROADMAP.md projection.

use super::adr::AdrRegistry;
use super::architecture::{ArchitectureDocument, ComponentStatus};
use super::requirements::RequirementsDocument;
use super::risks::RiskRegister;
use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use crate::planning::requirements::{RequirementCategory, RequirementKey, RequirementPriority};
use crate::state_machine::agent::AgentRole;
use crate::workflow::definition::{
    OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use crate::workflow::error::WorkflowError;
use crate::workflow::genesis::brownfield::BrownfieldMap;
use crate::workflow::genesis::project::ProjectCharter;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// Status of an individual phase within the roadmap.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum RoadmapPhaseStatus {
    #[default]
    Pending,
    Ready,
    Executing,
    Completed,
    Blocked,
    Superseded,
}

impl fmt::Display for RoadmapPhaseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Ready => write!(f, "ready"),
            Self::Executing => write!(f, "executing"),
            Self::Completed => write!(f, "completed"),
            Self::Blocked => write!(f, "blocked"),
            Self::Superseded => write!(f, "superseded"),
        }
    }
}

/// An execution unit in the project roadmap (Section 23, 25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadmapPhase {
    pub id: String,
    pub name: String,
    pub objective: String,
    pub requirement_refs: Vec<RequirementKey>,
    pub architecture_refs: Vec<String>,
    pub dependencies: Vec<String>,
    pub verification_strategy: String,
    pub exit_criteria: Vec<String>,
    pub is_enabling_phase: bool,
    pub remaining_risks: Vec<String>,
    pub status: RoadmapPhaseStatus,
}

impl RoadmapPhase {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        objective: impl Into<String>,
        verification_strategy: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            objective: objective.into(),
            requirement_refs: Vec::new(),
            architecture_refs: Vec::new(),
            dependencies: Vec::new(),
            verification_strategy: verification_strategy.into(),
            exit_criteria: Vec::new(),
            is_enabling_phase: false,
            remaining_risks: Vec::new(),
            status: RoadmapPhaseStatus::Pending,
        }
    }

    pub fn with_requirements(mut self, reqs: Vec<RequirementKey>) -> Self {
        self.requirement_refs = reqs;
        self
    }

    pub fn with_architecture(mut self, arch_refs: Vec<String>) -> Self {
        self.architecture_refs = arch_refs;
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn with_exit_criteria(mut self, criteria: Vec<String>) -> Self {
        self.exit_criteria = criteria;
        self
    }

    pub fn with_enabling(mut self, enabling: bool) -> Self {
        self.is_enabling_phase = enabling;
        self
    }

    pub fn with_risks(mut self, risks: Vec<String>) -> Self {
        self.remaining_risks = risks;
        self
    }
}

/// Explicit dependency link between roadmap phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapDependency {
    pub from_phase: String,
    pub to_phase: String,
    pub reason: String,
}

/// Full project roadmap domain model (Section 23).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roadmap {
    pub id: String,
    pub version: u32,
    pub objective: String,
    pub phases: Vec<RoadmapPhase>,
    pub dependencies: Vec<RoadmapDependency>,
}

impl Roadmap {
    pub fn new(id: impl Into<String>, objective: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 1,
            objective: objective.into(),
            phases: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    pub fn add_phase(&mut self, phase: RoadmapPhase) {
        self.phases.push(phase);
    }

    pub fn get_phase(&self, id: &str) -> Option<&RoadmapPhase> {
        self.phases.iter().find(|p| p.id == id)
    }

    pub fn get_phase_mut(&mut self, id: &str) -> Option<&mut RoadmapPhase> {
        self.phases.iter_mut().find(|p| p.id == id)
    }

    /// Validate roadmap structure and cycle freedom using Kahn's algorithm (Section 24).
    pub fn validate_dag(&self) -> Result<Vec<String>, WorkflowError> {
        let mut seen_ids = HashSet::new();
        for p in &self.phases {
            if p.id.trim().is_empty() {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(self.id.clone()),
                    step_key: None,
                    field: Some("phases.id".to_string()),
                    reason: "Phase ID cannot be empty".to_string(),
                });
            }
            if !seen_ids.insert(p.id.clone()) {
                return Err(WorkflowError::ManifestValidation {
                    path: None,
                    workflow_id: Some(self.id.clone()),
                    step_key: Some(p.id.clone()),
                    field: Some("phases.id".to_string()),
                    reason: format!("Duplicate phase ID '{}'", p.id),
                });
            }
        }

        // Validate dependencies exist and no self-loops
        for p in &self.phases {
            let mut seen_deps = HashSet::new();
            for dep in &p.dependencies {
                if dep == &p.id {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(self.id.clone()),
                        step_key: Some(p.id.clone()),
                        field: Some("dependencies".to_string()),
                        reason: format!("Phase '{}' cannot depend on itself", p.id),
                    });
                }
                if !seen_ids.contains(dep) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(self.id.clone()),
                        step_key: Some(p.id.clone()),
                        field: Some("dependencies".to_string()),
                        reason: format!("Phase '{}' depends on non-existent phase '{}'", p.id, dep),
                    });
                }
                if !seen_deps.insert(dep) {
                    return Err(WorkflowError::ManifestValidation {
                        path: None,
                        workflow_id: Some(self.id.clone()),
                        step_key: Some(p.id.clone()),
                        field: Some("dependencies".to_string()),
                        reason: format!("Phase '{}' has duplicate dependency '{}'", p.id, dep),
                    });
                }
            }
        }

        // Topological Sort via canonical DAG algorithm authority
        let keys: Vec<String> = self.phases.iter().map(|p| p.id.clone()).collect();
        let edges: Vec<(String, String)> = self
            .phases
            .iter()
            .flat_map(|p| p.dependencies.iter().map(|dep| (dep.clone(), p.id.clone())))
            .collect();

        let ordered = crate::dag::ops::topological_sort(keys, edges).map_err(|err| match err {
            crate::dag::ops::GraphError::CycleDetected { nodes, .. } => {
                WorkflowError::CycleDetected { cycle: nodes }
            }
            other => WorkflowError::InvalidDefinition(other.to_string()),
        })?;

        Ok(ordered)
    }

    /// Render canonical ROADMAP.md projection adhering to Section 27.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Engineering Roadmap: {}\n\n", self.id));

        out.push_str("## Roadmap Objective\n");
        out.push_str(self.objective.trim());
        out.push_str("\n\n");

        // Mermaid DAG
        out.push_str("## Dependency Graph\n");
        out.push_str("```mermaid\ngraph TD\n");
        for p in &self.phases {
            if p.dependencies.is_empty() {
                out.push_str(&format!(
                    "    {}[{}: {}]\n",
                    p.id.replace('-', "_"),
                    p.id,
                    p.name
                ));
            } else {
                for dep in &p.dependencies {
                    out.push_str(&format!(
                        "    {} --> {}\n",
                        dep.replace('-', "_"),
                        p.id.replace('-', "_")
                    ));
                }
            }
        }
        out.push_str("```\n\n");

        // Phases Detail
        out.push_str("## Implementation Phases\n");
        for p in &self.phases {
            out.push_str(&format!("### {}: {}\n\n", p.id, p.name));
            out.push_str(&format!("- **Objective**: {}\n", p.objective));
            out.push_str(&format!("- **Status**: `{}`\n", p.status));
            out.push_str(&format!("- **Enabling Phase**: {}\n", p.is_enabling_phase));

            let deps = if p.dependencies.is_empty() {
                "None (initial wave)".to_string()
            } else {
                p.dependencies
                    .iter()
                    .map(|d| format!("`{}`", d))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!("- **Dependencies**: {}\n", deps));

            if !p.requirement_refs.is_empty() {
                let req_str = p
                    .requirement_refs
                    .iter()
                    .map(|r| format!("`{}`", r))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("- **Satisfies Requirements**: {}\n", req_str));
            }
            if !p.architecture_refs.is_empty() {
                let comp_str = p
                    .architecture_refs
                    .iter()
                    .map(|c| format!("`{}`", c))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("- **Implements Components**: {}\n", comp_str));
            }

            out.push_str(&format!(
                "- **Verification Strategy**: {}\n",
                p.verification_strategy
            ));

            if !p.exit_criteria.is_empty() {
                out.push_str("- **Exit Criteria**:\n");
                for c in &p.exit_criteria {
                    out.push_str(&format!("  - [ ] {}\n", c));
                }
            }

            if !p.remaining_risks.is_empty() {
                out.push_str("- **Remaining Risks**:\n");
                for r in &p.remaining_risks {
                    out.push_str(&format!("  - {}\n", r));
                }
            }
            out.push('\n');
        }

        // Requirement Coverage Matrix
        out.push_str("## Requirement Coverage Matrix\n");
        out.push_str("| Requirement ID | Satisfying Phase | Verification Strategy |\n");
        out.push_str("|---|---|---|\n");
        let mut mapped_reqs = HashSet::new();
        for p in &self.phases {
            for r in &p.requirement_refs {
                mapped_reqs.insert(r.clone());
                out.push_str(&format!(
                    "| `{}` | `{}` | {} |\n",
                    r, p.id, p.verification_strategy
                ));
            }
        }
        out.push('\n');

        out
    }

    /// Persist to `<workspace_root>/<projection_dir>/ROADMAP.md`.
    pub fn save_to_dir(
        &self,
        workspace_root: &Path,
        projection_dir: &str,
    ) -> Result<PathBuf, WorkflowError> {
        let dir = workspace_root.join(projection_dir);
        std::fs::create_dir_all(&dir).map_err(|e| WorkflowError::ManifestValidation {
            path: Some(dir.display().to_string()),
            workflow_id: None,
            step_key: None,
            field: None,
            reason: format!("failed to create planning directory: {}", e),
        })?;
        let file_path = dir.join("ROADMAP.md");
        std::fs::write(&file_path, self.to_markdown()).map_err(|e| {
            WorkflowError::ManifestValidation {
                path: Some(file_path.display().to_string()),
                workflow_id: None,
                step_key: None,
                field: None,
                reason: format!("failed to write ROADMAP.md: {}", e),
            }
        })?;
        Ok(file_path)
    }

    /// Lower this dependency-ordered roadmap into an executable `WorkflowDefinition`.
    ///
    /// Preserves:
    /// - Strict topological phase sequencing
    /// - Explicit inter-phase dependencies
    /// - Quality gate verification requirements
    /// - Standard role envelopes and required capabilities
    /// - Output bindings to phase execution reports
    pub fn lower_to_workflow_definition(
        &self,
        projection_dir: &str,
    ) -> Result<WorkflowDefinition, WorkflowError> {
        // 1. Verify DAG cycle freedom and resolve execution ordering
        let _ordered_keys = self.validate_dag()?;

        let mut steps = Vec::with_capacity(self.phases.len());
        for phase in &self.phases {
            let step_key = phase.id.to_lowercase().replace('-', "_");

            // Define output binding for this phase execution report
            let report_filename = format!("{}_REPORT.md", phase.id);
            let report_rel_path = PathBuf::from(projection_dir)
                .join("phases")
                .join(&report_filename);

            let expected_outputs = vec![OutputBinding {
                artifact_name: report_filename.clone(),
                relative_path: report_rel_path,
                schema_type: "markdown".to_string(),
            }];

            // Map roadmap phase dependencies into workflow step keys
            let step_deps: Vec<String> = phase
                .dependencies
                .iter()
                .map(|dep| dep.to_lowercase().replace('-', "_"))
                .collect();

            // Quality gate requiring phase execution artifact
            let quality_gate = QualityGate {
                schema: None,
                required_artifacts: vec![report_filename],
                require_human_approval: false,
                max_ambiguity_percent: None,
            };

            let step_def = WorkflowStepDefinition {
                key: step_key,
                name: phase.name.clone(),
                role: AgentRole::implementer(),
                prompt_template: "execution.implementer".to_string(),
                required_inputs: Vec::new(),
                expected_outputs,
                required_capabilities: vec![
                    CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("fs.write", CapabilityAccessMode::ReadWrite),
                    CapabilityRequirement::new("shell.exec", CapabilityAccessMode::ReadWrite),
                    CapabilityRequirement::new("artifacts.write", CapabilityAccessMode::ReadWrite),
                ],
                quality_gate,
                depends_on: step_deps,
                timeout_secs: 1200,
                allows_parallelism: true,
                recovery_strategy: Some(RecoveryStrategy::Retry { max_retries: 2 }),
            };
            steps.push(step_def);
        }

        let workflow_id = format!("roadmap-{}", self.id.to_lowercase().replace(' ', "-"));
        let workflow_def = WorkflowDefinition {
            id: workflow_id,
            name: format!("Roadmap Workflow: {}", self.id),
            description: self.objective.clone(),
            version: self.version,
            steps,
            default_recovery_strategy: RecoveryStrategy::Fail,
        };

        workflow_def.validate()?;
        Ok(workflow_def)
    }
}

/// Autonomous synthesizer deriving phased execution DAGs from requirements, architecture, and ADRs.
pub struct RoadmapSynthesizer;

/// Order architecture subsystems by following component dependency edges
/// (foundation first) using Kahn's algorithm over subsystem nodes.
///
/// Generic mechanism: phase sequencing follows the derived dependency
/// structure, so presentational reordering of `arch.subsystems` (e.g.
/// deterministic alphabetical sorting) never changes delivery order.
fn ordered_subsystems_by_dependency(
    arch: &ArchitectureDocument,
) -> Vec<super::architecture::ArchitectureSubsystem> {
    use std::collections::HashMap;

    let comp_subsystem: HashMap<&str, &str> = arch
        .components
        .iter()
        .map(|c| (c.id.as_str(), c.subsystem.as_str()))
        .collect();

    // Subsystem-level predecessor sets derived from component edges.
    let mut predecessors: HashMap<&str, std::collections::HashSet<&str>> = arch
        .subsystems
        .iter()
        .map(|s| (s.id.as_str(), std::collections::HashSet::new()))
        .collect();
    for comp in &arch.components {
        let consumer = comp_subsystem
            .get(comp.id.as_str())
            .copied()
            .unwrap_or(comp.subsystem.as_str());
        for dep in &comp.depends_on {
            if let Some(provider) = comp_subsystem.get(dep.as_str()).copied()
                && provider != consumer
            {
                if let Some(entry) = predecessors.get_mut(consumer) {
                    entry.insert(provider);
                }
            }
        }
    }

    let mut ordered = Vec::new();
    let mut emitted: HashSet<&str> = HashSet::new();
    loop {
        let mut progress = false;
        for sub in &arch.subsystems {
            if emitted.contains(sub.id.as_str()) {
                continue;
            }
            let ready = predecessors
                .get(sub.id.as_str())
                .map(|preds| preds.iter().all(|p| emitted.contains(p)))
                .unwrap_or(true);
            if ready {
                emitted.insert(sub.id.as_str());
                ordered.push(sub.clone());
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    // Unorderable remainder (e.g. dependency cycle, validated later by
    // validate_dag) keeps document order rather than being dropped.
    for sub in &arch.subsystems {
        if !emitted.contains(sub.id.as_str()) {
            ordered.push(sub.clone());
        }
    }
    ordered
}

/// Presentation label for a requirement category (`functional` → `Functional`).
fn category_display(category: RequirementCategory) -> String {
    let display = category.to_string();
    let mut chars = display.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => "General".to_string(),
    }
}

/// Exit criteria derived from the scheduled requirements' own acceptance
/// criteria (shared by greenfield and brownfield phase builders).
fn phase_exit_criteria(
    requirements: &RequirementsDocument,
    phase_reqs: &[RequirementKey],
) -> Vec<String> {
    let mut criteria: Vec<String> = phase_reqs
        .iter()
        .filter_map(|k| {
            requirements
                .requirements
                .iter()
                .find(|r| &r.key == k)
                .and_then(|r| r.satisfaction_criteria.first().cloned())
        })
        .take(3)
        .collect();
    criteria.push(format!(
        "All {} scheduled requirement(s) satisfy acceptance criteria",
        phase_reqs.len()
    ));
    criteria
}

/// Verification strategy derived from exit criteria (shared builder).
fn phase_verification_strategy(criteria: &[String]) -> String {
    criteria
        .first()
        .map(|c| format!("Verify phase deliverables: {}", c))
        .unwrap_or_else(|| "Unit and integration verification of phase deliverables".to_string())
}

impl RoadmapSynthesizer {
    /// Synthesize dependency-ordered execution roadmap.
    pub fn synthesize(
        requirements: &RequirementsDocument,
        arch: &ArchitectureDocument,
        _adrs: &AdrRegistry,
        risks: &RiskRegister,
        charter: &ProjectCharter,
        brownfield: Option<&BrownfieldMap>,
    ) -> Result<Roadmap, WorkflowError> {
        let mut roadmap = Roadmap::new(
            &charter.project_name,
            format!(
                "Phased autonomous delivery roadmap for project '{}' satisfying {} requirements.",
                charter.project_name,
                requirements.requirements.len()
            ),
        );

        if let Some(bm) = brownfield {
            // Brownfield Sequencing: phases derive from
            // discovered topology and requirement evidence — never from a
            // fixed three-phase template. Structure:
            //   PHASE-01 baseline (host verification; always present because
            //     a scanned host repository exists by definition),
            //   PHASE-02..N one delivery phase per non-compatibility
            //     requirement category present,
            //   optional final regression phase when compatibility-scoped or
            //     otherwise uncovered requirements exist.
            // Phase counts legitimately vary (1, 2, 3, N) with the evidence.
            let existing_sub_ids: Vec<String> = arch
                .subsystems
                .iter()
                .filter(|s| {
                    arch.components
                        .iter()
                        .any(|c| c.subsystem == s.id && c.status == ComponentStatus::Existing)
                })
                .map(|s| s.id.clone())
                .collect();
            let delta_sub_ids: Vec<String> = arch
                .subsystems
                .iter()
                .filter(|s| {
                    arch.components
                        .iter()
                        .any(|c| c.subsystem == s.id && c.status == ComponentStatus::New)
                })
                .map(|s| s.id.clone())
                .collect();
            let delta_comp_ids: Vec<String> = arch
                .components
                .iter()
                .filter(|c| c.status == ComponentStatus::New)
                .map(|c| c.id.clone())
                .collect();
            let mut delta_arch_refs = delta_sub_ids.clone();
            delta_arch_refs.extend(delta_comp_ids.iter().cloned());

            let delta_name = bm.delta_scope.as_deref().unwrap_or("Extension Subsystem");

            // Baseline Phase: Pre-flight Regression & Invariant Verification
            let p1 = RoadmapPhase::new(
                "PHASE-01",
                "Pre-Flight Baseline & Invariant Verification",
                "Verify existing repository compiles, tests pass, and discovered topology invariants are satisfied",
                "Mechanical build and regression test execution across pre-existing test suites",
            )
            .with_architecture(existing_sub_ids.clone())
            .with_enabling(true)
            .with_exit_criteria(vec![
                "All pre-existing unit and integration tests pass cleanly".to_string(),
                format!("Host toolchain verified for primary language: {}", bm.topology.primary_language),
            ]);
            roadmap.add_phase(p1);
            let mut previous_phase = Some("PHASE-01".to_string());
            let mut phase_idx = 2u32;

            // Delta delivery phases: one per non-compatibility requirement
            // category present, chained after the baseline.
            let mut categories: Vec<RequirementCategory> = requirements
                .requirements
                .iter()
                .map(|r| r.category)
                .filter(|c| *c != RequirementCategory::Compatibility)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            categories.sort();
            for category in categories {
                let phase_reqs: Vec<RequirementKey> = requirements
                    .requirements
                    .iter()
                    .filter(|r| r.category == category)
                    .map(|r| r.key.clone())
                    .collect();
                if phase_reqs.is_empty() {
                    continue;
                }
                let label = category_display(category);
                let criteria = phase_exit_criteria(requirements, &phase_reqs);
                let verification = phase_verification_strategy(&criteria);
                let phase_id = format!("PHASE-{phase_idx:02}");
                let mut phase = RoadmapPhase::new(
                    phase_id.clone(),
                    format!("Delta {label}: {delta_name}"),
                    format!(
                        "Implement {label} requirements for '{delta_name}' (covers {} requirement(s))",
                        phase_reqs.len()
                    ),
                    verification,
                )
                .with_requirements(phase_reqs)
                .with_architecture(delta_arch_refs.clone())
                .with_exit_criteria(criteria);
                if let Some(prev) = previous_phase.clone() {
                    phase = phase.with_dependencies(vec![prev]);
                }
                roadmap.add_phase(phase);
                previous_phase = Some(phase_id);
                phase_idx += 1;
            }

            // Regression phase: compatibility-scoped requirements constrain
            // the integrated system, so they verify last against the full
            // suite. Any other uncovered requirement joins them; when
            // nothing remains uncovered, no regression phase is emitted.
            let scheduled: HashSet<_> = roadmap
                .phases
                .iter()
                .flat_map(|p| p.requirement_refs.iter().cloned())
                .collect();
            let mut remaining: Vec<RequirementKey> = requirements
                .requirements
                .iter()
                .filter(|r| {
                    r.priority != RequirementPriority::Deferred && !scheduled.contains(&r.key)
                })
                .map(|r| r.key.clone())
                .collect();
            remaining.sort_by_key(|k| k.to_string());
            if !remaining.is_empty() {
                let criteria = phase_exit_criteria(requirements, &remaining);
                let verification = phase_verification_strategy(&criteria);
                let phase_id = format!("PHASE-{phase_idx:02}");
                let mut arch_refs = existing_sub_ids.clone();
                arch_refs.extend(delta_arch_refs.iter().cloned());
                let mut phase = RoadmapPhase::new(
                    phase_id.clone(),
                    "End-to-End Regression & Compatibility Verification",
                    "Verify integrated system passes full regression test suite with zero invariant breaks",
                    verification,
                )
                .with_requirements(remaining)
                .with_architecture(arch_refs)
                .with_exit_criteria(criteria);
                if let Some(prev) = previous_phase.clone() {
                    phase = phase.with_dependencies(vec![prev]);
                }
                roadmap.add_phase(phase);
            }
        } else {
            // Greenfield sequencing: derive one delivery phase per emitted
            // architecture subsystem, ordered by the component dependency
            // chain (foundation first) — NOT by presentational subsystem
            // order, which deterministic sorting may alphabetize. Phase
            // count, names, requirement slices, and architecture references
            // all follow the derived architecture — no universal
            // persistence/engine/security/transport template is assumed.
            // The runtime owns DAG validity and coverage; the content is
            // target-derived.
            let mut previous_phase: Option<String> = None;
            for (idx, subsystem) in ordered_subsystems_by_dependency(arch).iter().enumerate() {
                let phase_reqs: Vec<RequirementKey> = arch
                    .components
                    .iter()
                    .filter(|c| c.subsystem == subsystem.id)
                    .flat_map(|c| c.requirement_refs.iter().cloned())
                    .collect();
                let mut arch_refs = vec![subsystem.id.clone()];
                arch_refs.extend(
                    arch.components
                        .iter()
                        .filter(|c| c.subsystem == subsystem.id)
                        .map(|c| c.id.clone()),
                );

                // Exit criteria and verification strategy derive from the
                // scheduled requirements' own acceptance criteria (shared
                // builder with the brownfield path).
                let criteria = phase_exit_criteria(requirements, &phase_reqs);
                let verification_strategy = phase_verification_strategy(&criteria);

                let phase_id = format!("PHASE-{:02}", idx + 1);
                let mut phase = RoadmapPhase::new(
                    phase_id.clone(),
                    format!("{} Delivery", subsystem.name),
                    format!(
                        "{} (covers {} requirement(s))",
                        subsystem.description,
                        phase_reqs.len()
                    ),
                    verification_strategy,
                )
                .with_requirements(phase_reqs)
                .with_architecture(arch_refs)
                .with_exit_criteria(criteria);
                if let Some(prev) = previous_phase.clone() {
                    phase = phase.with_dependencies(vec![prev]);
                } else {
                    phase = phase.with_enabling(true);
                }
                roadmap.add_phase(phase);
                previous_phase = Some(phase_id);
            }
        }

        // Coverage backstop (both modes): every non-deferred requirement must
        // be scheduled. An architecture that leaves requirements uncovered is
        // an explicit synthesis failure, not a silent gap.
        let scheduled: HashSet<_> = roadmap
            .phases
            .iter()
            .flat_map(|p| p.requirement_refs.iter().cloned())
            .collect();
        let uncovered: Vec<String> = requirements
            .requirements
            .iter()
            .filter(|r| r.priority != RequirementPriority::Deferred && !scheduled.contains(&r.key))
            .map(|r| r.key.to_string())
            .collect();
        if !uncovered.is_empty() {
            return Err(WorkflowError::ManifestValidation {
                path: None,
                workflow_id: None,
                step_key: Some("roadmap_coverage".to_string()),
                field: None,
                reason: format!(
                    "Derived architecture leaves {} requirement(s) unscheduled: {}",
                    uncovered.len(),
                    uncovered.join(", ")
                ),
            });
        }

        // Link risks to phases
        for risk in &risks.risks {
            for phase in &mut roadmap.phases {
                if phase
                    .requirement_refs
                    .iter()
                    .any(|r| risk.affected_requirements.contains(r))
                    || phase
                        .architecture_refs
                        .iter()
                        .any(|c| risk.affected_components.contains(c))
                {
                    phase
                        .remaining_risks
                        .push(format!("{}: {}", risk.id, risk.description));
                }
            }
        }

        roadmap.validate_dag()?;
        Ok(roadmap)
    }
}
