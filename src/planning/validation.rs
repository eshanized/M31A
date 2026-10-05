//! Deterministic multi-stage plan validation pipeline (PLN-02, D-09..D-12).
//!
//! Validates candidate plans across three sequential stages:
//! 1. Graph / Structural validation (cycles, missing prerequisites, duplicates)
//! 2. Capability / Policy satisfiability (authorization dry-run, ASK never becomes ALLOW)
//! 3. Safety / Bounds / Evidence validation (resource limits, verification strategies, contradictory criteria)

use crate::kernel::seams::policy::{
    PolicyEvaluationRequest, PolicyGate, ResolvedAction, resolve_decision,
};

use crate::ids::{MissionId, TaskId};
use crate::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, MAX_COMPOSITE_DEPTH, VerificationStrategy,
};
use crate::planning::risks::PlanningUnknown;
use crate::state::budget::ResourceBudget;
use crate::state::intake::AutonomyMode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

/// A blocking validation failure rejecting a candidate plan (PLN-02, D-09).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationError {
    EmptyPlan,
    DuplicateTaskId {
        task_key: CandidateTaskKey,
    },
    UnknownRole {
        task_key: CandidateTaskKey,
        role: String,
    },
    SelfDependency {
        task_key: CandidateTaskKey,
    },
    CycleDetected {
        path: Vec<CandidateTaskKey>,
    },
    MissingPrerequisite {
        task_key: CandidateTaskKey,
        prerequisite: CandidateTaskKey,
    },
    DuplicateDependency {
        task_key: CandidateTaskKey,
        prerequisite: CandidateTaskKey,
    },
    MissingCapabilities {
        task_key: CandidateTaskKey,
        capability_id: String,
    },
    PolicyConflict {
        task_key: CandidateTaskKey,
        capability_id: String,
        reason: String,
    },
    InvalidVerification {
        task_key: CandidateTaskKey,
        reason: String,
    },
    BoundsExceeded {
        task_key: CandidateTaskKey,
        metric: String,
        limit: String,
        requested: String,
    },
    CriticalUnknowns {
        task_key: CandidateTaskKey,
        unknown_id: String,
        description: String,
    },
    ContradictoryCriteria {
        task_key: CandidateTaskKey,
        description: String,
    },
}

/// A non-blocking warning generated during plan validation (D-09).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationWarning {
    HighResourceUsage {
        task_key: CandidateTaskKey,
        metric: String,
        ratio: f64,
    },
    NonCriticalAssumptionUnresolved {
        assumption_id: String,
        description: String,
    },
    UnusedCapability {
        task_key: CandidateTaskKey,
        capability_id: String,
    },
    Informational {
        message: String,
    },
}

/// Aggregated validation report containing all blocking errors and warnings (D-09).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plan is valid only if there are zero blocking errors.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add_error(&mut self, err: ValidationError) {
        self.errors.push(err);
    }

    pub fn add_warning(&mut self, warn: ValidationWarning) {
        self.warnings.push(warn);
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

/// Internal normalized predicate representation for contradictory criteria detection (D-12).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NormalizedPredicate {
    FileMustExist(String),
    FileMustNotExist(String),
    TestsMustPass,
    TestsMustFail,
    CompilationMustSucceed,
    CompilationMustFail,
}

impl NormalizedPredicate {
    fn parse(text: &str) -> Option<Self> {
        let lower = text.trim().to_lowercase();
        if lower.contains("must not exist")
            || lower.contains("does not exist")
            || lower.starts_with("delete file ")
            || lower.starts_with("remove file ")
        {
            let path = lower
                .replace("must not exist", "")
                .replace("does not exist", "")
                .replace("delete file", "")
                .replace("remove file", "")
                .replace("file", "")
                .trim()
                .to_string();
            if !path.is_empty() {
                return Some(Self::FileMustNotExist(path));
            }
        } else if lower.contains("must exist")
            || lower.contains("file exists")
            || lower.starts_with("create file ")
        {
            let path = lower
                .replace("must exist", "")
                .replace("file exists", "")
                .replace("create file", "")
                .replace("file", "")
                .trim()
                .to_string();
            if !path.is_empty() {
                return Some(Self::FileMustExist(path));
            }
        } else if lower.contains("test")
            && (lower.contains("must pass") || lower.contains("passes"))
        {
            return Some(Self::TestsMustPass);
        } else if lower.contains("test") && (lower.contains("must fail") || lower.contains("fails"))
        {
            return Some(Self::TestsMustFail);
        } else if lower.contains("compil")
            && (lower.contains("must succeed") || lower.contains("succeeds"))
        {
            return Some(Self::CompilationMustSucceed);
        } else if lower.contains("compil")
            && (lower.contains("must fail") || lower.contains("fails"))
        {
            return Some(Self::CompilationMustFail);
        }
        None
    }

    fn contradicts(&self, other: &Self) -> Option<String> {
        match (self, other) {
            (Self::FileMustExist(a), Self::FileMustNotExist(b)) | (Self::FileMustNotExist(b), Self::FileMustExist(a)) if a == b => {
                Some(format!("Contradictory file existence predicates for '{}'", a))
            }
            (Self::TestsMustPass, Self::TestsMustFail) | (Self::TestsMustFail, Self::TestsMustPass) => {
                Some("Contradictory test outcome criteria: TestsMustPass vs TestsMustFail".to_string())
            }
            (Self::CompilationMustSucceed, Self::CompilationMustFail) | (Self::CompilationMustFail, Self::CompilationMustSucceed) => {
                Some("Contradictory compilation outcome criteria: CompilationMustSucceed vs CompilationMustFail".to_string())
            }
            _ => None,
        }
    }
}

/// Deterministic multi-stage plan validator (PLN-02, D-09).
#[derive(Clone)]
pub struct PlanValidator {
    available_capabilities: Option<HashSet<String>>,
    policy_gate: Option<Arc<dyn PolicyGate>>,
    autonomy_mode: AutonomyMode,
    budget: Option<ResourceBudget>,
    critical_unknowns: Vec<PlanningUnknown>,
    mission_id: MissionId,
    allow_empty: bool,
    workspace_root: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for PlanValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanValidator")
            .field("available_capabilities", &self.available_capabilities)
            .field("autonomy_mode", &self.autonomy_mode)
            .field("critical_unknowns", &self.critical_unknowns)
            .field("mission_id", &self.mission_id)
            .field("allow_empty", &self.allow_empty)
            .finish()
    }
}

impl PlanValidator {
    pub fn new() -> Self {
        Self {
            available_capabilities: None,
            policy_gate: None,
            autonomy_mode: AutonomyMode::Safe,
            budget: None,
            critical_unknowns: Vec::new(),
            mission_id: MissionId::new(),
            allow_empty: false,
            workspace_root: None,
        }
    }

    pub fn with_allow_empty(mut self, allow: bool) -> Self {
        self.allow_empty = allow;
        self
    }

    pub fn with_available_capabilities(mut self, caps: HashSet<String>) -> Self {
        self.available_capabilities = Some(caps);
        self
    }

    pub fn with_policy_gate(mut self, gate: Arc<dyn PolicyGate>) -> Self {
        self.policy_gate = Some(gate);
        self
    }

    pub fn with_autonomy_mode(mut self, mode: AutonomyMode) -> Self {
        self.autonomy_mode = mode;
        self
    }

    pub fn with_budget(mut self, budget: ResourceBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    pub fn with_critical_unknowns(mut self, unknowns: Vec<PlanningUnknown>) -> Self {
        self.critical_unknowns = unknowns;
        self
    }

    pub fn with_mission_id(mut self, mission_id: MissionId) -> Self {
        self.mission_id = mission_id;
        self
    }

    pub fn with_workspace_root(mut self, root: std::path::PathBuf) -> Self {
        self.workspace_root = Some(root);
        self
    }

    /// Validate a role string against the registered role definitions.
    ///
    /// Existence authority is the role registry: any registered id (built-in
    /// or extension) validates; anything else is an explicit `UnknownRole`
    /// error. Well-formed but unregistered ids never silently map anywhere.
    pub fn validate_role(
        &self,
        task_key: impl Into<CandidateTaskKey>,
        role_str: &str,
    ) -> Result<crate::state_machine::agent::AgentRole, ValidationError> {
        use std::str::FromStr;
        let key = task_key.into();
        let role = crate::state_machine::agent::AgentRole::from_str(role_str).map_err(|_| {
            ValidationError::UnknownRole {
                task_key: key.clone(),
                role: role_str.to_string(),
            }
        })?;
        let registered = crate::agent::registry::RoleRegistry::global()
            .read()
            .map(|r| r.contains(&role))
            .unwrap_or(false);
        if registered {
            Ok(role)
        } else {
            Err(ValidationError::UnknownRole {
                task_key: key,
                role: role_str.to_string(),
            })
        }
    }

    /// Executes all three validation passes in order: Structural -> Capability/Policy -> Safety/Bounds.
    pub async fn validate(&self, plan: &CandidatePlan) -> ValidationReport {
        let mut report = ValidationReport::new();

        self.run_structural_pass(plan, &mut report);
        self.run_capability_pass(plan, &mut report).await;
        self.run_safety_bounds_pass(plan, &mut report);

        report
    }

    /// Stage 1: Deterministic structural graph validation (D-10).
    pub fn run_structural_pass(&self, plan: &CandidatePlan, report: &mut ValidationReport) {
        if plan.tasks.is_empty() {
            if !self.allow_empty {
                report.add_error(ValidationError::EmptyPlan);
            }
            return;
        }

        // BTreeMap ensures deterministic ordering independent of hash seed
        let mut task_map: BTreeMap<&CandidateTaskKey, &CandidateTask> = BTreeMap::new();
        for task in &plan.tasks {
            if task_map.contains_key(&task.id) {
                report.add_error(ValidationError::DuplicateTaskId {
                    task_key: task.id.clone(),
                });
            } else {
                task_map.insert(&task.id, task);
            }
        }

        // 1. Check self-deps, duplicates, and missing prerequisites via canonical DAG authority
        let known_keys: BTreeSet<CandidateTaskKey> = task_map.keys().copied().cloned().collect();
        let anomalies = crate::dag::ops::inspect_dependencies(
            &known_keys,
            plan.tasks
                .iter()
                .map(|t| (t.id.clone(), t.depends_on.clone())),
        );

        for self_loop in anomalies.self_loops {
            report.add_error(ValidationError::SelfDependency {
                task_key: self_loop,
            });
        }
        for (task_key, missing) in anomalies.missing_prerequisites {
            report.add_error(ValidationError::MissingPrerequisite {
                task_key,
                prerequisite: missing,
            });
        }
        for (task_key, dup) in anomalies.duplicate_dependencies {
            report.add_error(ValidationError::DuplicateDependency {
                task_key,
                prerequisite: dup,
            });
        }

        // 2. Cycle detection via canonical DAG algorithm authority
        let mut prereqs_map = BTreeMap::new();
        for task in &plan.tasks {
            let deps: BTreeSet<CandidateTaskKey> = task.depends_on.iter().cloned().collect();
            prereqs_map.insert(task.id.clone(), deps);
        }

        let cycles = crate::dag::ops::find_all_cycle_paths(&known_keys, &prereqs_map);
        for cycle in cycles {
            report.add_error(ValidationError::CycleDetected { path: cycle });
        }

        // 3. Deduplication check: warn on duplicate task objectives across distinct tasks
        let mut seen_objectives: BTreeMap<String, &CandidateTaskKey> = BTreeMap::new();
        for task in &plan.tasks {
            let normalized_obj = task.objective.trim().to_lowercase();
            if !normalized_obj.is_empty() {
                if let Some(prev_key) = seen_objectives.get(&normalized_obj) {
                    report.add_warning(ValidationWarning::Informational {
                        message: format!(
                            "Potential duplicate task detected: '{}' and '{}' share identical objective '{}'",
                            prev_key, task.id, task.objective
                        ),
                    });
                } else {
                    seen_objectives.insert(normalized_obj, &task.id);
                }
            }
        }
    }

    /// Stage 2: Capability and Policy satisfiability pre-flight pass (D-11).
    pub async fn run_capability_pass(&self, plan: &CandidatePlan, report: &mut ValidationReport) {
        for task in &plan.tasks {
            let profile = crate::agent::profile::AgentProfile::built_in(task.role.clone());

            // Role envelope capability check: enforce calculate_eligible_capabilities against role's CapabilityEnvelope (GAP-02)
            if let Err(err) = crate::agent::envelope::calculate_eligible_capabilities(
                &task.capabilities,
                &profile.capability_policy,
            ) {
                let cap_id = match &err {
                    crate::agent::envelope::CapabilityIntersectionError::ForbiddenCapability(
                        id,
                    ) => id.clone(),
                    _ => "role_envelope".to_string(),
                };
                report.add_error(ValidationError::PolicyConflict {
                    task_key: task.id.clone(),
                    capability_id: cap_id,
                    reason: format!(
                        "Role '{}' capability envelope violation: {}",
                        task.role, err
                    ),
                });
            }

            for cap in &task.capabilities {
                // Formatting validation
                if let Err(msg) = cap.validate() {
                    report.add_error(ValidationError::PolicyConflict {
                        task_key: task.id.clone(),
                        capability_id: cap.id.clone(),
                        reason: msg,
                    });
                    continue;
                }

                // Check available capabilities snapshot
                if let Some(ref available) = self.available_capabilities
                    && !available.contains(&cap.id)
                {
                    report.add_error(ValidationError::MissingCapabilities {
                        task_key: task.id.clone(),
                        capability_id: cap.id.clone(),
                    });
                }

                // Policy pre-flight evaluation (typed: role/mode/workspace flow
                // as typed values; missing workspace fails closed and is
                // reported as a plan conflict, never silently defaulted).
                if let Some(ref gate) = self.policy_gate {
                    let workspace = match self.workspace_root.clone() {
                        Some(ws) => ws,
                        None => {
                            report.add_error(ValidationError::PolicyConflict {
                                task_key: task.id.clone(),
                                capability_id: cap.id.clone(),
                                reason: "missing security-critical policy attribute: workspace_root is required for policy pre-flight".to_string(),
                            });
                            continue;
                        }
                    };
                    let req = PolicyEvaluationRequest::new(
                        self.mission_id,
                        TaskId::new(),
                        cap.id.clone(),
                    )
                    .with_role(task.role.clone())
                    .with_autonomy_mode(self.autonomy_mode)
                    .with_workspace(workspace);

                    match gate.evaluate(req).await {
                        Ok(decision) => {
                            let resolved = resolve_decision(
                                self.autonomy_mode,
                                decision,
                                &format!("Planning pre-flight capability check for {}", cap.id),
                                false,
                            );

                            // Invariant: ASK/ESCALATE is NEVER implicit ALLOW. Only Proceed is allowed.
                            if let ResolvedAction::Deny { reason }
                            | ResolvedAction::PauseForApproval { reason }
                            | ResolvedAction::Escalate { reason } = resolved
                            {
                                report.add_error(ValidationError::PolicyConflict {
                                    task_key: task.id.clone(),
                                    capability_id: cap.id.clone(),
                                    reason: format!(
                                        "Policy gate decision '{:?}' resolved to non-proceeding: {}",
                                        decision, reason
                                    ),
                                });
                            }
                        }
                        Err(err) => {
                            report.add_error(ValidationError::PolicyConflict {
                                task_key: task.id.clone(),
                                capability_id: cap.id.clone(),
                                reason: format!("Policy gate evaluation failed: {}", err),
                            });
                        }
                    }
                }
            }
        }
    }

    /// Stage 3: Safety, Resource Bounds, Verification, and Contradictory Criteria pass (D-12).
    pub fn run_safety_bounds_pass(&self, plan: &CandidatePlan, report: &mut ValidationReport) {
        let mut total_steps: u64 = 0;
        let mut total_duration: u64 = 0;
        let mut total_tokens: u64 = 0;
        let mut total_cost: f64 = 0.0;

        for task in &plan.tasks {
            // 1. Validate resource estimate fields
            if let Err(msg) = task.estimates.validate() {
                report.add_error(ValidationError::BoundsExceeded {
                    task_key: task.id.clone(),
                    metric: "estimates".to_string(),
                    limit: "valid numbers".to_string(),
                    requested: msg,
                });
            }

            // 2. Verification strategy check
            if let VerificationStrategy::Composite { .. } = task.verification
                && task.verification.depth() > MAX_COMPOSITE_DEPTH
            {
                report.add_error(ValidationError::InvalidVerification {
                    task_key: task.id.clone(),
                    reason: format!(
                        "Composite verification depth {} exceeds maximum {}",
                        task.verification.depth(),
                        MAX_COMPOSITE_DEPTH
                    ),
                });
            }

            // Accumulate
            total_steps += task.estimates.max_steps as u64;
            total_duration += task.estimates.max_duration_secs;
            total_tokens += task.estimates.max_tokens;
            total_cost += task.estimates.max_cost_usd;

            // 3. Per-task budget limits
            if let Some(ref b) = self.budget {
                if let Some(max_steps) = b.max_agent_steps
                    && task.estimates.max_steps as usize > max_steps
                {
                    report.add_error(ValidationError::BoundsExceeded {
                        task_key: task.id.clone(),
                        metric: "max_agent_steps".to_string(),
                        limit: max_steps.to_string(),
                        requested: task.estimates.max_steps.to_string(),
                    });
                }
                if let Some(max_secs) = b.max_wall_clock_seconds
                    && task.estimates.max_duration_secs > max_secs
                {
                    report.add_error(ValidationError::BoundsExceeded {
                        task_key: task.id.clone(),
                        metric: "max_wall_clock_seconds".to_string(),
                        limit: max_secs.to_string(),
                        requested: task.estimates.max_duration_secs.to_string(),
                    });
                }
                if let Some(max_tok) = b.max_tokens
                    && task.estimates.max_tokens > max_tok
                {
                    report.add_error(ValidationError::BoundsExceeded {
                        task_key: task.id.clone(),
                        metric: "max_tokens".to_string(),
                        limit: max_tok.to_string(),
                        requested: task.estimates.max_tokens.to_string(),
                    });
                }
                if let Some(max_c) = b.max_cost_usd
                    && task.estimates.max_cost_usd > max_c
                {
                    report.add_error(ValidationError::BoundsExceeded {
                        task_key: task.id.clone(),
                        metric: "max_cost_usd".to_string(),
                        limit: format!("{:.2}", max_c),
                        requested: format!("{:.2}", task.estimates.max_cost_usd),
                    });
                }
            }

            // 4. Contradictory criteria check
            let mut predicates = Vec::new();
            if let Some(ref desc) = task.description {
                for line in desc.lines() {
                    if let Some(p) = NormalizedPredicate::parse(line) {
                        predicates.push(p);
                    }
                }
            }
            if let Some(p) = NormalizedPredicate::parse(&task.objective) {
                predicates.push(p);
            }

            for i in 0..predicates.len() {
                for j in (i + 1)..predicates.len() {
                    if let Some(contradiction) = predicates[i].contradicts(&predicates[j]) {
                        report.add_error(ValidationError::ContradictoryCriteria {
                            task_key: task.id.clone(),
                            description: contradiction,
                        });
                    }
                }
            }

            // 5. Critical unknowns gating
            for unk in &self.critical_unknowns {
                if unk.criticality.is_blocking_threshold() {
                    // Critical unknown blocks executable tasks if unmitigated/unresolved
                    report.add_error(ValidationError::CriticalUnknowns {
                        task_key: task.id.clone(),
                        unknown_id: unk.id.clone(),
                        description: unk.description.clone(),
                    });
                }
            }

            // 6. Verification contract check: warn if mutating task has no completion criteria
            if task.is_mutating() && task.completion_criteria.is_empty() {
                report.add_warning(ValidationWarning::Informational {
                    message: format!(
                        "Mutating task '{}' has no explicit completion_criteria declared",
                        task.id
                    ),
                });
            }
        }

        // 6. Plan-level total budget checks
        if let Some(ref b) = self.budget {
            if let Some(max_steps) = b.max_agent_steps
                && total_steps as usize > max_steps
            {
                report.add_warning(ValidationWarning::HighResourceUsage {
                    task_key: CandidateTaskKey::new("plan-total"),
                    metric: "total_agent_steps".to_string(),
                    ratio: total_steps as f64 / max_steps as f64,
                });
            }
            if let Some(max_secs) = b.max_wall_clock_seconds
                && total_duration > max_secs
            {
                report.add_warning(ValidationWarning::HighResourceUsage {
                    task_key: CandidateTaskKey::new("plan-total"),
                    metric: "total_duration_secs".to_string(),
                    ratio: total_duration as f64 / max_secs as f64,
                });
            }
            if let Some(max_tokens) = b.max_tokens
                && total_tokens > max_tokens
            {
                report.add_warning(ValidationWarning::HighResourceUsage {
                    task_key: CandidateTaskKey::new("plan-total"),
                    metric: "total_tokens".to_string(),
                    ratio: total_tokens as f64 / max_tokens as f64,
                });
            }
            if let Some(max_cost) = b.max_cost_usd
                && total_cost > max_cost
            {
                report.add_warning(ValidationWarning::HighResourceUsage {
                    task_key: CandidateTaskKey::new("plan-total"),
                    metric: "total_cost_usd".to_string(),
                    ratio: total_cost / max_cost,
                });
            }
        }
    }
}

impl Default for PlanValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::seams::policy::{PolicyDecision, PolicyError, PolicyEvaluationRequest};

    use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement, ResourceEstimate};
    use crate::planning::requirements::{Provenance, ProvenanceSourceType, TrustLevel};
    use crate::planning::risks::Criticality;
    use crate::state_machine::agent::AgentRole;
    use async_trait::async_trait;

    struct MockPolicyGate {
        decision: PolicyDecision,
    }

    #[async_trait]
    impl PolicyGate for MockPolicyGate {
        async fn evaluate(
            &self,
            _req: PolicyEvaluationRequest,
        ) -> Result<PolicyDecision, PolicyError> {
            Ok(self.decision)
        }
    }

    fn sample_task(id: &str, depends_on: Vec<&str>) -> CandidateTask {
        CandidateTask {
            id: CandidateTaskKey::new(id),
            objective: format!("Objective for {}", id),
            description: None,
            depends_on: depends_on.into_iter().map(CandidateTaskKey::new).collect(),
            capabilities: vec![],
            role: AgentRole::implementer(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::new(5, 60, 1000, 0.10),
            ..Default::default()
        }
    }

    #[test]
    fn test_validation_report_validity() {
        let mut report = ValidationReport::new();
        assert!(report.is_valid());

        report.add_warning(ValidationWarning::Informational {
            message: "Heads up".to_string(),
        });
        assert!(report.is_valid());

        report.add_error(ValidationError::EmptyPlan);
        assert!(!report.is_valid());
    }

    #[test]
    fn test_structural_pass_cycle_and_missing_keys() {
        let task_a = sample_task("A", vec!["B"]);
        let task_b = sample_task("B", vec!["C"]);
        let task_c = sample_task("C", vec!["A"]); // Cycle: A -> B -> C -> A
        let task_d = sample_task("D", vec!["NONEXISTENT"]); // Missing prereq
        let mut task_self = sample_task("SELF", vec!["SELF"]); // Self dependency
        task_self.depends_on.push(CandidateTaskKey::new("SELF")); // Duplicate dependency

        let plan = CandidatePlan::new(
            "plan-cycle",
            "Test cycles",
            vec![task_a, task_b, task_c, task_d, task_self],
        );

        let validator = PlanValidator::new();
        let mut report = ValidationReport::new();
        validator.run_structural_pass(&plan, &mut report);

        assert!(!report.is_valid());
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::CycleDetected { .. }))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::MissingPrerequisite { .. }))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::SelfDependency { .. }))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::DuplicateDependency { .. }))
        );
    }

    #[tokio::test]
    async fn test_capability_pass_ask_and_escalate_never_allow() {
        let mut task = sample_task("task-ask", vec![]);
        task.capabilities.push(CapabilityRequirement::new(
            "proc.exec",
            CapabilityAccessMode::ReadWrite,
        ));

        let plan = CandidatePlan::new("plan-cap", "Test policy gate", vec![task]);

        // Policy gate returns Ask in Unattended mode -> Must DENY / produce PolicyConflict
        let gate = Arc::new(MockPolicyGate {
            decision: PolicyDecision::Ask,
        });

        let validator = PlanValidator::new()
            .with_policy_gate(gate)
            .with_autonomy_mode(AutonomyMode::Unattended);

        let report = validator.validate(&plan).await;
        assert!(!report.is_valid());
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::PolicyConflict { .. }))
        );
    }

    #[test]
    fn test_safety_bounds_and_contradictory_criteria() {
        let mut task = sample_task("task-bounds", vec![]);
        task.estimates.max_steps = 100;
        task.description = Some("Create file config.json\nDelete file config.json".to_string());

        let budget = ResourceBudget {
            max_agent_steps: Some(20), // 100 > 20 -> BoundsExceeded
            ..Default::default()
        };

        let prov = Provenance::new(
            ProvenanceSourceType::UserPrompt,
            TrustLevel::UntrustedExternal,
            "user",
        );
        let unknown = PlanningUnknown::new(
            "UNK-01",
            "Architecture unknown",
            Criticality::Critical,
            prov,
        );

        let plan = CandidatePlan::new("plan-bounds", "Bounds test", vec![task]);

        let validator = PlanValidator::new()
            .with_budget(budget)
            .with_critical_unknowns(vec![unknown]);

        let mut report = ValidationReport::new();
        validator.run_safety_bounds_pass(&plan, &mut report);

        assert!(!report.is_valid());
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::BoundsExceeded { .. }))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::ContradictoryCriteria { .. }))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::CriticalUnknowns { .. }))
        );
    }

    #[tokio::test]
    async fn test_capability_pass_rejects_role_envelope_violations() {
        // Researcher attempting to execute cargo.test (not in researcher envelope)
        let mut task = sample_task("task-researcher-invalid", vec![]);
        task.role = AgentRole::researcher();
        task.capabilities = vec![CapabilityRequirement::new(
            "cargo.test",
            CapabilityAccessMode::Read,
        )];

        let plan = CandidatePlan::new("plan-invalid-role", "Test envelope violation", vec![task]);
        let validator = PlanValidator::new();
        let mut report = ValidationReport::new();
        validator.run_capability_pass(&plan, &mut report).await;

        assert!(!report.is_valid());
        assert!(
            report.errors.iter().any(|e| matches!(
                e,
                ValidationError::PolicyConflict {
                    capability_id,
                    reason,
                    ..
                } if capability_id == "cargo.test" && reason.contains("capability envelope violation")
            )),
            "Expected PolicyConflict for cargo.test violating researcher role envelope, got: {:?}",
            report.errors
        );
    }

    #[tokio::test]
    async fn test_capability_pass_allows_valid_role_envelope() {
        // Researcher requesting valid read-only capabilities
        let mut task = sample_task("task-researcher-valid", vec![]);
        task.role = AgentRole::researcher();
        task.capabilities = vec![
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
            CapabilityRequirement::new("repo.read", CapabilityAccessMode::Read),
        ];

        let plan = CandidatePlan::new("plan-valid-role", "Test valid envelope", vec![task]);
        let validator = PlanValidator::new();
        let mut report = ValidationReport::new();
        validator.run_capability_pass(&plan, &mut report).await;

        assert!(
            report.is_valid(),
            "Expected valid report for allowed researcher capabilities, got errors: {:?}",
            report.errors
        );
    }
}
