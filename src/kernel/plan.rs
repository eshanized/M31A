//! Canonical executable candidate plan contracts and verification primitives (PLN-01, PLN-05, D-01..D-04).
//!
//! Kernel-owned contracts defining planner proposals, task identity, dependencies,
//! capabilities, verification strategies, and resource estimates.
//!
//! Core principle: The model proposes. The runtime decides.
//! The planner produces candidate plan proposals; the runtime scheduler/DAG decides whether
//! to materialize and execute them.

use crate::ids::ArtifactId;
use crate::state_machine::agent::AgentRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

/// Execution outcome of a completed task (D-15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    pub summary: String,
    #[serde(default)]
    pub output_artifacts: Vec<ArtifactId>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl TaskResult {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            output_artifacts: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Maximum allowed nesting depth for Composite verification strategies (ASVS Level 1 mitigation).
pub const MAX_COMPOSITE_DEPTH: usize = 3;

/// Plan-local identifier for candidate tasks.
///
/// Authoritative `TaskId` values are only allocated when a validated candidate plan
/// is materialized into the runtime `TaskGraph` by the materializer (D-01).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CandidateTaskKey(pub String);

impl CandidateTaskKey {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for CandidateTaskKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for CandidateTaskKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for CandidateTaskKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for CandidateTaskKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Access mode requested for a capability (D-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAccessMode {
    Read,
    Write,
    ReadWrite,
}

impl fmt::Display for CapabilityAccessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::ReadWrite => write!(f, "read_write"),
        }
    }
}

impl std::str::FromStr for CapabilityAccessMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "read_write" | "readwrite" => Ok(Self::ReadWrite),
            other => Err(format!("Unknown capability access mode: {}", other)),
        }
    }
}

/// Strongly typed capability requirement requested by a task (D-03).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CapabilityRequirement {
    pub id: String,
    pub mode: CapabilityAccessMode,
}

impl<'de> serde::Deserialize<'de> for CapabilityRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Helper {
            Str(String),
            Obj {
                id: String,
                #[serde(default)]
                mode: Option<CapabilityAccessMode>,
            },
        }

        match Helper::deserialize(deserializer)? {
            Helper::Str(s) => {
                let mode = if s == "fs.write" || s.ends_with(".write") {
                    CapabilityAccessMode::Write
                } else if s == "proc.exec" || s == "evidence.record" {
                    CapabilityAccessMode::ReadWrite
                } else {
                    CapabilityAccessMode::Read
                };
                Ok(Self { id: s, mode })
            }
            Helper::Obj { id, mode } => {
                let m = mode.unwrap_or_else(|| {
                    if id == "fs.write" || id.ends_with(".write") {
                        CapabilityAccessMode::Write
                    } else if id == "proc.exec" || id == "evidence.record" {
                        CapabilityAccessMode::ReadWrite
                    } else {
                        CapabilityAccessMode::Read
                    }
                });
                Ok(Self { id, mode: m })
            }
        }
    }
}

impl CapabilityRequirement {
    pub fn new(id: impl Into<String>, mode: CapabilityAccessMode) -> Self {
        Self {
            id: id.into(),
            mode,
        }
    }

    /// Validates capability requirement formatting and prevents implicit privilege escalation.
    pub fn validate(&self) -> Result<(), String> {
        let trimmed = self.id.trim();
        if trimmed.is_empty() {
            return Err("Capability ID cannot be empty".to_string());
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == ':')
        {
            return Err(format!(
                "Capability ID contains invalid characters: '{}'",
                self.id
            ));
        }
        Ok(())
    }
}

/// Verification strategy defining how a task produces evidence of completion (D-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerificationStrategy {
    Compilation,
    AutomatedTest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },
    StaticAnalysis {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
    },
    ArtifactInspection {
        paths: Vec<String>,
    },
    ReviewGate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reviewer_role: Option<AgentRole>,
    },
    Composite {
        strategies: Vec<VerificationStrategy>,
    },
}

impl VerificationStrategy {
    /// Safe constructor for composite verification strategies that bounds nesting depth.
    pub fn composite(strategies: Vec<VerificationStrategy>) -> Result<Self, String> {
        if strategies.is_empty() {
            return Err("Composite verification strategy cannot be empty".to_string());
        }
        let strategy = Self::Composite { strategies };
        let depth = strategy.depth();
        if depth > MAX_COMPOSITE_DEPTH {
            return Err(format!(
                "Composite verification strategy exceeds maximum depth of {} (got {})",
                MAX_COMPOSITE_DEPTH, depth
            ));
        }
        Ok(strategy)
    }

    /// Computes the recursive depth of the verification strategy tree.
    pub fn depth(&self) -> usize {
        match self {
            Self::Composite { strategies } => {
                1 + strategies.iter().map(|s| s.depth()).max().unwrap_or(0)
            }
            _ => 1,
        }
    }

    /// Returns a human-readable name of the verification strategy.
    pub fn strategy_name(&self) -> &'static str {
        match self {
            Self::Compilation => "compilation",
            Self::AutomatedTest { .. } => "automated_test",
            Self::StaticAnalysis { .. } => "static_analysis",
            Self::ArtifactInspection { .. } => "artifact_inspection",
            Self::ReviewGate { .. } => "review_gate",
            Self::Composite { .. } => "composite",
        }
    }
}

/// Planner-estimated resource bounds for a candidate task (D-04).
///
/// Note: Estimates are planner predictions only and never grant runtime budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceEstimate {
    pub max_steps: u32,
    pub max_duration_secs: u64,
    pub max_tokens: u64,
    pub max_cost_usd: f64,
}

impl ResourceEstimate {
    pub fn new(max_steps: u32, max_duration_secs: u64, max_tokens: u64, max_cost_usd: f64) -> Self {
        Self {
            max_steps,
            max_duration_secs,
            max_tokens,
            max_cost_usd,
        }
    }

    /// Validates that all resource estimates are non-negative and finite.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_cost_usd.is_nan() || self.max_cost_usd.is_infinite() || self.max_cost_usd < 0.0
        {
            return Err(format!("Invalid max_cost_usd: {}", self.max_cost_usd));
        }
        if self.max_duration_secs == 0 || self.max_duration_secs > 7200 {
            return Err(format!(
                "Invalid max_duration_secs: {} (must be between 1 and 7200 seconds)",
                self.max_duration_secs
            ));
        }
        if self.max_steps == 0 || self.max_steps > 100 {
            return Err(format!(
                "Invalid max_steps: {} (must be between 1 and 100)",
                self.max_steps
            ));
        }
        Ok(())
    }
}

impl Default for ResourceEstimate {
    fn default() -> Self {
        Self {
            max_steps: 10,
            max_duration_secs: 300,
            max_tokens: 50_000,
            max_cost_usd: 1.0,
        }
    }
}

/// Task-level risk rating (L0 kernel contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl TaskRiskLevel {
    pub fn is_blocking_threshold(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

impl fmt::Display for TaskRiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Trigger cause for a plan revision (L0 kernel contract).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplanningTrigger {
    TaskFailure {
        failed_task_id: String,
        reason: String,
    },
    VerificationFailure {
        failed_task_id: String,
        details: String,
    },
    EvidenceDiscovery {
        unexpected_finding: String,
    },
    RepositoryDrift {
        changed_files: Vec<String>,
    },
    InvalidatedAssumption {
        assumption_id: String,
        description: String,
    },
    PolicyDenial {
        reason: String,
    },
    MissingCapability {
        capability_id: String,
    },
    ScopeExpansion {
        reason: String,
    },
    ManualIntervention {
        reason: String,
    },
}

impl fmt::Display for ReplanningTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskFailure {
                failed_task_id,
                reason,
            } => {
                write!(f, "task_failure({}, reason: {})", failed_task_id, reason)
            }
            Self::VerificationFailure {
                failed_task_id,
                details,
            } => {
                write!(
                    f,
                    "verification_failure({}, details: {})",
                    failed_task_id, details
                )
            }
            Self::EvidenceDiscovery { unexpected_finding } => {
                write!(f, "evidence_discovery({})", unexpected_finding)
            }
            Self::RepositoryDrift { changed_files } => {
                write!(f, "repository_drift({} files)", changed_files.len())
            }
            Self::InvalidatedAssumption {
                assumption_id,
                description,
            } => {
                write!(
                    f,
                    "invalidated_assumption({}, {})",
                    assumption_id, description
                )
            }
            Self::PolicyDenial { reason } => write!(f, "policy_denial({})", reason),
            Self::MissingCapability { capability_id } => {
                write!(f, "missing_capability({})", capability_id)
            }
            Self::ScopeExpansion { reason } => write!(f, "scope_expansion({})", reason),
            Self::ManualIntervention { reason } => write!(f, "manual_intervention({})", reason),
        }
    }
}

/// Provenance and audit record for a plan revision (L0 kernel contract).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRevisionRecord {
    pub revision: u32,
    pub parent_plan_id: String,
    pub reason: String,
    pub trigger: ReplanningTrigger,
    #[serde(default)]
    pub affected_tasks: Vec<CandidateTaskKey>,
    #[serde(default)]
    pub preserved_tasks: Vec<CandidateTaskKey>,
    #[serde(default)]
    pub added_tasks: Vec<CandidateTaskKey>,
    #[serde(default)]
    pub invalidated_tasks: Vec<CandidateTaskKey>,
    #[serde(default)]
    pub invalidated_assumptions: Vec<String>,
    #[serde(default)]
    pub changed_dependencies: Vec<(CandidateTaskKey, Vec<CandidateTaskKey>)>,
    pub timestamp: DateTime<Utc>,
}

impl PlanRevisionRecord {
    pub fn new(
        revision: u32,
        parent_plan_id: impl Into<String>,
        reason: impl Into<String>,
        trigger: ReplanningTrigger,
    ) -> Self {
        Self {
            revision,
            parent_plan_id: parent_plan_id.into(),
            reason: reason.into(),
            trigger,
            affected_tasks: Vec::new(),
            preserved_tasks: Vec::new(),
            added_tasks: Vec::new(),
            invalidated_tasks: Vec::new(),
            invalidated_assumptions: Vec::new(),
            changed_dependencies: Vec::new(),
            timestamp: Utc::now(),
        }
    }
}

/// An epistemic assumption recorded in a candidate plan (L0 kernel contract).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePlanAssumption {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub invalidated: bool,
}

impl CandidatePlanAssumption {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            invalidated: false,
        }
    }
}

/// Candidate task proposed by a planner (D-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateTask {
    #[serde(alias = "task_id")]
    pub id: CandidateTaskKey,
    #[serde(default, alias = "title")]
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, alias = "dependencies")]
    pub depends_on: Vec<CandidateTaskKey>,
    #[serde(default)]
    pub capabilities: Vec<CapabilityRequirement>,
    #[serde(default = "default_candidate_role")]
    pub role: AgentRole,
    #[serde(default = "default_candidate_verification")]
    pub verification: VerificationStrategy,
    #[serde(default)]
    pub estimates: ResourceEstimate,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default, alias = "acceptance_criteria")]
    pub completion_criteria: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<TaskRiskLevel>,
    #[serde(default)]
    pub requirement_keys: Vec<String>,
    /// Typed prompt execution binding selected for this task.
    ///
    /// This is the authoritative workflow/task prompt reference: it MUST be
    /// compiled by the canonical PromptCatalog/PromptCompiler and MUST NOT
    /// be downgraded into [`CandidateTask::description`] text. `None`
    /// (legacy plans) resolves through the role default at context
    /// compilation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_ref: Option<crate::prompt::PromptReference>,
}

fn default_candidate_role() -> AgentRole {
    AgentRole::implementer()
}

fn default_candidate_verification() -> VerificationStrategy {
    VerificationStrategy::Compilation
}

impl Default for CandidateTask {
    fn default() -> Self {
        Self {
            id: CandidateTaskKey::new("TASK-DEFAULT"),
            objective: String::new(),
            description: None,
            depends_on: Vec::new(),
            capabilities: Vec::new(),
            role: AgentRole::implementer(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::default(),
            affected_paths: Vec::new(),
            expected_outputs: Vec::new(),
            completion_criteria: Vec::new(),
            assumptions: Vec::new(),
            risk_level: None,
            requirement_keys: Vec::new(),
            prompt_ref: None,
        }
    }
}

impl CandidateTask {
    pub fn new(
        id: impl Into<CandidateTaskKey>,
        objective: impl Into<String>,
        role: AgentRole,
        verification: VerificationStrategy,
        estimates: ResourceEstimate,
    ) -> Self {
        Self {
            id: id.into(),
            objective: objective.into(),
            description: None,
            depends_on: Vec::new(),
            capabilities: Vec::new(),
            role,
            verification,
            estimates,
            affected_paths: Vec::new(),
            expected_outputs: Vec::new(),
            completion_criteria: Vec::new(),
            assumptions: Vec::new(),
            risk_level: None,
            requirement_keys: Vec::new(),
            prompt_ref: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Attach the typed prompt execution binding for this task.
    pub fn with_prompt_ref(mut self, prompt_ref: crate::prompt::PromptReference) -> Self {
        self.prompt_ref = Some(prompt_ref);
        self
    }

    pub fn with_depends_on(mut self, deps: Vec<CandidateTaskKey>) -> Self {
        self.depends_on = deps;
        self
    }

    pub fn with_capabilities(mut self, caps: Vec<CapabilityRequirement>) -> Self {
        self.capabilities = caps;
        self
    }

    pub fn with_affected_paths(mut self, paths: Vec<String>) -> Self {
        self.affected_paths = paths;
        self
    }

    pub fn with_expected_outputs(mut self, outputs: Vec<String>) -> Self {
        self.expected_outputs = outputs;
        self
    }

    pub fn with_completion_criteria(mut self, criteria: Vec<String>) -> Self {
        self.completion_criteria = criteria;
        self
    }

    pub fn with_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.assumptions = assumptions;
        self
    }

    pub fn with_risk_level(mut self, risk: TaskRiskLevel) -> Self {
        self.risk_level = Some(risk);
        self
    }

    pub fn with_requirement_keys(mut self, keys: Vec<String>) -> Self {
        self.requirement_keys = keys;
        self
    }

    pub fn is_mutating(&self) -> bool {
        self.capabilities.iter().any(|c| {
            c.id == "fs.write"
                || c.id.ends_with(".write")
                || c.mode == CapabilityAccessMode::Write
                || c.mode == CapabilityAccessMode::ReadWrite
        })
    }
}

fn default_plan_revision() -> u32 {
    1
}

/// Candidate plan proposed by a planner (D-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidatePlan {
    pub plan_id: String,
    pub objective: String,
    pub tasks: Vec<CandidateTask>,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_plan_revision")]
    pub revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_record: Option<PlanRevisionRecord>,
    #[serde(default)]
    pub assumptions: Vec<CandidatePlanAssumption>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl Default for CandidatePlan {
    fn default() -> Self {
        Self {
            plan_id: "plan-default".to_string(),
            objective: String::new(),
            tasks: Vec::new(),
            created_at: Utc::now(),
            revision: 1,
            parent_plan_id: None,
            revision_record: None,
            assumptions: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

impl CandidatePlan {
    pub fn new(
        plan_id: impl Into<String>,
        objective: impl Into<String>,
        tasks: Vec<CandidateTask>,
    ) -> Self {
        Self {
            plan_id: plan_id.into(),
            objective: objective.into(),
            tasks,
            created_at: Utc::now(),
            revision: 1,
            parent_plan_id: None,
            revision_record: None,
            assumptions: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_revision(mut self, revision: u32) -> Self {
        self.revision = revision;
        self
    }

    pub fn with_parent_plan_id(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_plan_id = Some(parent_id.into());
        self
    }

    pub fn with_revision_record(mut self, record: PlanRevisionRecord) -> Self {
        self.revision_record = Some(record);
        self
    }

    pub fn with_assumptions(mut self, assumptions: Vec<CandidatePlanAssumption>) -> Self {
        self.assumptions = assumptions;
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn find_task(&self, key: &CandidateTaskKey) -> Option<&CandidateTask> {
        self.tasks.iter().find(|t| &t.id == key)
    }

    pub fn find_task_mut(&mut self, key: &CandidateTaskKey) -> Option<&mut CandidateTask> {
        self.tasks.iter_mut().find(|t| &t.id == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_task_key_basic() {
        let key = CandidateTaskKey::new("task-init");
        assert_eq!(key.as_str(), "task-init");
        assert_eq!(key.to_string(), "task-init");
        assert_eq!(&*key, "task-init");

        let from_str: CandidateTaskKey = "task-2".into();
        assert_eq!(from_str.as_str(), "task-2");
    }

    #[test]
    fn test_capability_requirement_validation() {
        let valid = CapabilityRequirement::new("filesystem.read", CapabilityAccessMode::Read);
        assert!(valid.validate().is_ok());

        let invalid = CapabilityRequirement::new("   ", CapabilityAccessMode::Write);
        assert!(invalid.validate().is_err());

        let invalid_chars =
            CapabilityRequirement::new("bad id; drop table", CapabilityAccessMode::Write);
        assert!(invalid_chars.validate().is_err());
    }

    #[test]
    fn test_verification_strategy_composite_depth_bound() {
        let leaf1 = VerificationStrategy::Compilation;
        let leaf2 = VerificationStrategy::AutomatedTest {
            command: Some("cargo test".to_string()),
        };

        // Depth 2
        let comp1 = VerificationStrategy::composite(vec![leaf1.clone(), leaf2.clone()]).unwrap();
        assert_eq!(comp1.depth(), 2);

        // Depth 3 (allowed)
        let comp2 = VerificationStrategy::composite(vec![comp1.clone(), leaf1.clone()]).unwrap();
        assert_eq!(comp2.depth(), 3);

        // Depth 4 (rejected by MAX_COMPOSITE_DEPTH)
        let comp3_err = VerificationStrategy::composite(vec![comp2]);
        assert!(comp3_err.is_err());
        assert!(comp3_err.unwrap_err().contains("exceeds maximum depth"));

        // Empty composite rejected
        assert!(VerificationStrategy::composite(vec![]).is_err());
    }

    #[test]
    fn test_resource_estimate_validation() {
        let valid = ResourceEstimate::new(10, 120, 10_000, 0.50);
        assert!(valid.validate().is_ok());

        let invalid_nan = ResourceEstimate::new(10, 120, 10_000, f64::NAN);
        assert!(invalid_nan.validate().is_err());

        let invalid_neg = ResourceEstimate::new(10, 120, 10_000, -1.0);
        assert!(invalid_neg.validate().is_err());
    }

    #[test]
    fn test_candidate_plan_full_roundtrip_serde() {
        let task1 = CandidateTask {
            id: CandidateTaskKey::new("task-1"),
            objective: "Implement candidate models".to_string(),
            description: Some("Detailed task description".to_string()),
            depends_on: vec![],
            capabilities: vec![CapabilityRequirement::new(
                "fs.write",
                CapabilityAccessMode::Write,
            )],
            role: AgentRole::implementer(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::new(5, 60, 5_000, 0.10),
            ..Default::default()
        };

        let task2 = CandidateTask {
            id: CandidateTaskKey::new("task-2"),
            objective: "Verify tests".to_string(),
            description: None,
            depends_on: vec![CandidateTaskKey::new("task-1")],
            capabilities: vec![CapabilityRequirement::new(
                "proc.exec",
                CapabilityAccessMode::ReadWrite,
            )],
            role: AgentRole::verifier(),
            verification: VerificationStrategy::composite(vec![
                VerificationStrategy::AutomatedTest {
                    command: Some("cargo test".to_string()),
                },
                VerificationStrategy::ArtifactInspection {
                    paths: vec!["target/debug".to_string()],
                },
            ])
            .unwrap(),
            estimates: ResourceEstimate::new(3, 30, 2_000, 0.05),
            ..Default::default()
        };

        let plan = CandidatePlan::new(
            "plan-04-01",
            "Establish planning domain models",
            vec![task1, task2],
        );
        assert_eq!(plan.task_count(), 2);
        assert!(plan.find_task(&CandidateTaskKey::new("task-1")).is_some());

        let json = serde_json::to_string_pretty(&plan).expect("serialize plan");
        let deserialized: CandidatePlan = serde_json::from_str(&json).expect("deserialize plan");

        assert_eq!(plan.plan_id, deserialized.plan_id);
        assert_eq!(plan.objective, deserialized.objective);
        assert_eq!(plan.tasks.len(), deserialized.tasks.len());
        assert_eq!(plan.tasks[0].id, deserialized.tasks[0].id);
        assert_eq!(plan.tasks[0].role, deserialized.tasks[0].role);
        assert_eq!(
            plan.tasks[1].verification,
            deserialized.tasks[1].verification
        );
    }
}
