use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{MissionId, TaskId};
use crate::kernel::plan::CandidatePlan;

/// Structured upstream product and architectural context prepared prior to canonical planning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UpstreamPlanContext {
    pub project_name: String,
    pub charter: String,
    pub architecture: String,
    #[serde(default)]
    pub requirements: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_summary: Option<String>,
    #[serde(default)]
    pub workflow_tier: String,
    /// Unresolved epistemic unknowns from discovery and research.
    #[serde(default)]
    pub unknowns: Vec<String>,
    /// Critical decisions requiring explicit operator direction (UserDecisionRequired).
    #[serde(default)]
    pub user_decisions: Vec<String>,
    /// Epistemic invariants definitively resolved earlier during discovery/synthesis.
    #[serde(default)]
    pub resolved_invariants: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectiveClassification {
    DirectAnswer,
    RepositoryInspection,
    RepositoryResearch,
    RepositoryAudit,
    Implementation,
    Verification,
    Investigation,
    Documentation,
    Mixed,
}

impl ObjectiveClassification {
    /// Whether this classification represents read-only (non-mutating) work.
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            ObjectiveClassification::DirectAnswer
                | ObjectiveClassification::RepositoryInspection
                | ObjectiveClassification::RepositoryResearch
                | ObjectiveClassification::RepositoryAudit
                | ObjectiveClassification::Investigation
                | ObjectiveClassification::Verification
        )
    }

    /// Whether this classification expects at least one executable task.
    pub fn expects_executable_tasks(self) -> bool {
        !matches!(self, ObjectiveClassification::DirectAnswer)
    }
}

/// Canonical resolved intent contract carrying validated request intent across pipeline stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRequestIntent {
    pub raw_prompt: String,
    pub classification: ObjectiveClassification,
    pub is_mutation_forbidden: bool,
    pub is_read_only: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_ambiguity: Option<String>,
}

impl ResolvedRequestIntent {
    pub fn new(
        raw_prompt: impl Into<String>,
        classification: ObjectiveClassification,
        is_mutation_forbidden: bool,
    ) -> Self {
        let is_read_only = is_mutation_forbidden || classification.is_read_only();
        Self {
            raw_prompt: raw_prompt.into(),
            classification,
            is_mutation_forbidden,
            is_read_only,
            execution_constraints: Vec::new(),
            unresolved_ambiguity: None,
        }
    }

    pub fn is_read_only(&self) -> bool {
        self.is_read_only || self.is_mutation_forbidden || self.classification.is_read_only()
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.execution_constraints.push(constraint.into());
        self
    }

    pub fn with_ambiguity(mut self, ambiguity: impl Into<String>) -> Self {
        self.unresolved_ambiguity = Some(ambiguity.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanRequest {
    pub mission_id: MissionId,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_context: Option<UpstreamPlanContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
    #[serde(default)]
    pub is_read_only: bool,
    #[serde(default)]
    pub is_mutation_forbidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_intent: Option<ResolvedRequestIntent>,
}

impl PlanRequest {
    pub fn new(mission_id: MissionId, objective: impl Into<String>) -> Self {
        let obj_str: String = objective.into();
        let resolved = resolve_request_intent(&obj_str);
        Self {
            mission_id,
            objective: obj_str,
            upstream_context: None,
            classification: Some(format!("{:?}", resolved.classification)),
            is_read_only: resolved.is_read_only(),
            is_mutation_forbidden: resolved.is_mutation_forbidden,
            resolved_intent: Some(resolved),
        }
    }

    pub fn with_resolved_intent(mut self, intent: ResolvedRequestIntent) -> Self {
        self.classification = Some(format!("{:?}", intent.classification));
        self.is_read_only = intent.is_read_only();
        self.is_mutation_forbidden = intent.is_mutation_forbidden;
        self.resolved_intent = Some(intent);
        self
    }

    pub fn with_upstream_context(mut self, ctx: UpstreamPlanContext) -> Self {
        self.upstream_context = Some(ctx);
        self
    }

    pub fn with_classification(
        mut self,
        classification: impl Into<String>,
        is_read_only: bool,
    ) -> Self {
        let cls_str = classification.into();
        self.classification = Some(cls_str);
        self.is_read_only = is_read_only;
        if is_read_only {
            if let Some(ref mut intent) = self.resolved_intent {
                intent.is_read_only = true;
            }
        }
        self
    }

    pub fn with_mutation_forbidden(mut self, forbidden: bool) -> Self {
        self.is_mutation_forbidden = forbidden;
        if forbidden {
            self.is_read_only = true;
            if let Some(ref mut intent) = self.resolved_intent {
                intent.is_mutation_forbidden = true;
                intent.is_read_only = true;
            }
        }
        self
    }

    /// Resolve canonical contract, preserving read-only and mutation-forbidden invariants.
    pub fn resolved_contract(&self) -> ResolvedRequestIntent {
        let mut contract = if let Some(ref intent) = self.resolved_intent {
            intent.clone()
        } else {
            resolve_request_intent(&self.objective)
        };
        if self.is_mutation_forbidden {
            contract.is_mutation_forbidden = true;
            contract.is_read_only = true;
        }
        if self.is_read_only {
            contract.is_read_only = true;
        }
        contract
    }
}

/// Tokenize text into words while stripping punctuation and filtering out filenames/paths.
pub fn extract_non_filename_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|token| {
            let t = token.trim_matches(|c: char| !c.is_alphanumeric());
            !t.contains('/')
                && !t.contains('\\')
                && !t.contains('.')
                && !t.ends_with(".rs")
                && !t.ends_with(".toml")
                && !t.ends_with(".json")
        })
        .map(|token| {
            token
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// Check if non-filename tokens contain a specific standalone word.
pub fn tokens_contain_word(tokens: &[String], word: &str) -> bool {
    let lower_word = word.to_lowercase();
    tokens.iter().any(|t| t == &lower_word)
}

/// Check if the prompt explicitly forbids mutations, edits, writes, or workspace changes.
pub fn is_mutation_forbidden(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    let negation_markers = [
        "do not modify",
        "don't modify",
        "does not modify",
        "never modify",
        "without modifying",
        "do not change",
        "don't change",
        "does not change",
        "never change",
        "without changing",
        "do not edit",
        "don't edit",
        "does not edit",
        "never edit",
        "without editing",
        "do not write",
        "don't write",
        "never write",
        "without writing",
        "do not fix",
        "don't fix",
        "never fix",
        "without fixing",
        "not fix anything",
        "do not touch",
        "don't touch",
        "never touch",
        "without touching",
        "do not alter",
        "don't alter",
        "never alter",
        "without altering",
        "do not mutate",
        "don't mutate",
        "never mutate",
        "without mutating",
        "read-only",
        "read only",
        "readonly",
        "inspect only",
        "only inspect",
        "explain only",
        "only explain",
        "report only",
        "only report",
        "audit only",
        "only audit",
        "view only",
        "only view",
        "only read",
        "only check",
        "only analyze",
        "just report",
        "just inspect",
        "just explain",
        "just read",
        "just view",
        "just audit",
        "just check",
        "just analyze",
        "no changes",
        "no modifications",
        "no edits",
        "no code changes",
        "make no changes",
        "make no modifications",
        "without making changes",
        "without making any changes",
    ];
    negation_markers.iter().any(|m| lower.contains(m))
}

/// Check if the prompt references specific repository files, paths, or code symbols.
pub fn contains_repository_targets(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    let extensions = [
        ".rs", ".toml", ".json", ".md", ".yaml", ".yml", ".ts", ".js", ".py", ".sh", ".sql",
    ];
    let path_prefixes = [
        "src/",
        "tests/",
        "docs/",
        "migrations/",
        "benches/",
        "examples/",
        "prompts/",
        "scripts/",
        "crates/",
    ];
    let file_names = [
        "cargo.toml",
        "cargo.lock",
        "readme.md",
        "main.rs",
        "lib.rs",
        "auth.rs",
        "build.rs",
    ];

    extensions.iter().any(|ext| lower.contains(ext))
        || path_prefixes.iter().any(|p| lower.contains(p))
        || file_names.iter().any(|f| lower.contains(f))
        || lower.contains("in this repository")
        || lower.contains("in this codebase")
        || lower.contains("in this project")
        || lower.contains("this repository")
        || lower.contains("this codebase")
        || lower.contains("where is")
}

/// Classify a mission objective into a strongly typed semantic category.
///
/// Precedence:
/// 1. Explicit negation ("do not modify files...", "read-only", "report only") guarantees read-only classification.
/// 2. Mixed request detection (investigation/audit/test followed by fix sequence).
/// 3. Explicit read-only inquiry framing ("what does <file> do", "can you inspect", "explain how... works").
/// 4. Verification keywords → `Verification`.
/// 5. Diagnostic keywords → `Investigation`.
/// 6. Documentation requests (writing README, docs without modifying code).
/// 7. Security / Reliability Audit keywords → `RepositoryAudit`.
/// 8. Research / Study keywords → `RepositoryResearch`.
/// 9. Mutation keywords (whole-word only, non-filename) → `Implementation`.
/// 10. Secondary research / inspection verbs.
/// 11. Repository targets without mutation → `RepositoryInspection`.
/// 12. General conversational questions → `DirectAnswer`.
/// 13. Fail-closed conservative default: `RepositoryInspection` (never unearned write authority).
pub fn classify_objective(objective: &str) -> ObjectiveClassification {
    let lower = objective.to_lowercase();
    let trimmed = lower.trim();
    let mutation_forbidden = is_mutation_forbidden(objective);
    let has_repo_targets = contains_repository_targets(objective);
    let non_filename_tokens = extract_non_filename_tokens(trimmed);

    // 1. Explicit negation overrides any mutation keywords
    if mutation_forbidden {
        if lower.contains("run test")
            || lower.contains("run the test")
            || lower.contains("verify")
            || lower.contains("check compilation")
            || lower.contains("check whether the build passes")
            || lower.contains("test suite")
        {
            return ObjectiveClassification::Verification;
        }
        if lower.contains("audit")
            || lower.contains("security review")
            || lower.contains("vulnerability")
            || lower.contains("compliance")
        {
            return ObjectiveClassification::RepositoryAudit;
        }
        if lower.contains("diagnose")
            || lower.contains("root cause")
            || lower.contains("find why")
            || (lower.contains("why") && (lower.contains("fail") || lower.contains("error")))
            || lower.contains("find the bug")
            || lower.contains("explain the bug")
            || lower.contains("bug")
            || lower.contains("debug")
            || lower.contains("troubleshoot")
            || lower.contains("analyze the error")
            || lower.contains("error")
        {
            return ObjectiveClassification::Investigation;
        }
        if lower.contains("study")
            || lower.contains("analyze")
            || lower.contains("analyse")
            || lower.contains("architecture")
            || lower.contains("explore")
            || lower.contains("understand")
            || lower.contains("map ")
            || lower.contains("review the codebase")
        {
            return ObjectiveClassification::RepositoryResearch;
        }
        if lower.contains("inspect") || has_repo_targets {
            return ObjectiveClassification::RepositoryInspection;
        }
        if lower.starts_with("what is") || lower.starts_with("what are") {
            return ObjectiveClassification::DirectAnswer;
        }
        return ObjectiveClassification::RepositoryInspection;
    }

    // 2. Check for mixed multi-phase requests (e.g. audit then fix, run tests and fix)
    let mixed_sequences = [
        "then fix",
        "then repair",
        "then implement",
        "then patch",
        "then update",
        "then resolve",
        "after that fix",
        "followed by fixing",
        "and fix any failures",
        "and fix failures",
        "and fix the failures",
        "and fix any errors",
        "and fix errors",
        "run tests and fix",
        "run the tests and fix",
        "test and fix",
        "audit and fix",
        "and fix critical",
    ];
    let has_mixed_seq = mixed_sequences.iter().any(|ms| lower.contains(ms));
    let has_read_or_verify_phase = lower.contains("study")
        || lower.contains("audit")
        || lower.contains("investigate")
        || lower.contains("root cause")
        || lower.contains("find the")
        || lower.contains("find why")
        || lower.contains("run test")
        || lower.contains("run the test")
        || lower.contains("verify")
        || lower.contains("check whether")
        || lower.contains("test the")
        || lower.contains("test this");

    if has_mixed_seq && has_read_or_verify_phase {
        return ObjectiveClassification::Mixed;
    }

    // 3. Explicit read-only inquiry framing:
    // "What does <file> do?", "Can you inspect...", "Explain how... works", "Find why... and report"
    if (trimmed.starts_with("what does") || trimmed.starts_with("what do"))
        && trimmed.contains("do")
    {
        if has_repo_targets
            || lower.contains(".rs")
            || lower.contains(".toml")
            || lower.contains("src/")
        {
            return ObjectiveClassification::RepositoryInspection;
        }
        return ObjectiveClassification::DirectAnswer;
    }

    if (trimmed.starts_with("can you inspect")
        || trimmed.starts_with("inspect the code")
        || trimmed.starts_with("please inspect"))
        && !has_mixed_seq
    {
        return ObjectiveClassification::RepositoryInspection;
    }

    if (lower.contains("find why")
        || lower.contains("find the root cause")
        || lower.contains("diagnose why"))
        && (lower.contains("report")
            || lower.contains("cause")
            || lower.contains("failing")
            || lower.contains("fails"))
        && !has_mixed_seq
    {
        return ObjectiveClassification::Investigation;
    }

    if lower.contains("write a report") || lower.contains("write a summary") {
        return ObjectiveClassification::RepositoryResearch;
    }

    if (trimmed.starts_with("explain how") || trimmed.starts_with("describe how"))
        && (lower.contains("system")
            || lower.contains("works")
            || lower.contains("architecture")
            || lower.contains("runtime"))
    {
        return ObjectiveClassification::RepositoryResearch;
    }

    // 4. Verification requests (check compilation, run tests, verify build)
    let verification_markers = [
        "check whether the build passes",
        "check if the build passes",
        "check whether tests pass",
        "check if tests pass",
        "check compilation",
        "verify the build passes",
        "verify the build",
        "verify build passes",
        "verify compilation",
        "run the existing test suite",
        "run existing test suite",
        "run the test suite",
        "run test suite",
        "run tests",
        "run the tests",
        "verify the test suite",
        "verify tests",
        "cargo test",
        "cargo check",
    ];
    if verification_markers.iter().any(|m| lower.contains(m)) && !has_mixed_seq {
        return ObjectiveClassification::Verification;
    }

    // 5. Diagnostic investigation requests
    let diagnostic_markers = [
        "diagnose",
        "debug",
        "troubleshoot",
        "root cause",
        "find the failure",
        "investigate the failure",
        "explain the bug",
    ];
    if diagnostic_markers.iter().any(|m| lower.contains(m))
        && !has_mixed_seq
        && !tokens_contain_word(&non_filename_tokens, "fix")
    {
        return ObjectiveClassification::Investigation;
    }

    // 6. Documentation requests (writing README, docs, guides)
    let doc_targets = [
        "readme",
        "documentation",
        "docs",
        "guide",
        "tutorial",
        "api reference",
        "installation instructions",
        "license",
        "contributing",
        "manual",
    ];
    let doc_actions = ["write", "create", "update", "generate", "draft", "add"];
    let is_doc = doc_targets.iter().any(|dt| lower.contains(dt))
        && doc_actions
            .iter()
            .any(|da| tokens_contain_word(&non_filename_tokens, da))
        && !lower.contains("bug")
        && !lower.contains("parser")
        && !lower.contains("test failure")
        && !lower.contains("function");
    if is_doc {
        return ObjectiveClassification::Documentation;
    }

    // 7. Security / Reliability Audit
    let audit_markers = [
        "audit",
        "security review",
        "security audit",
        "security",
        "reliability review",
        "reliability audit",
        "reliability",
        "vulnerability",
        "compliance review",
        "penetration test",
    ];
    if audit_markers.iter().any(|m| lower.contains(m)) && !has_mixed_seq {
        return ObjectiveClassification::RepositoryAudit;
    }

    // 8. Research / Study / Architecture analysis
    let research_markers = [
        "study the codebase",
        "study the repository",
        "study the architecture",
        "analyze the codebase",
        "analyze the architecture",
        "analyze the parser",
        "analyse the codebase",
        "analyse the architecture",
        "review the codebase",
        "review the repository",
        "understand the project structure",
        "understand the codebase",
        "investigate the runtime architecture",
        "investigate system architecture",
        "map the modules",
        "explore the codebase",
        "explore the repository",
    ];
    if research_markers.iter().any(|m| lower.contains(m))
        && !has_mixed_seq
        && !tokens_contain_word(&non_filename_tokens, "fix")
        && !tokens_contain_word(&non_filename_tokens, "implement")
    {
        return ObjectiveClassification::RepositoryResearch;
    }

    // 9. Mutating Implementation keywords (whole word only, outside filenames)
    let has_fix_verb = tokens_contain_word(&non_filename_tokens, "fix")
        || tokens_contain_word(&non_filename_tokens, "repair")
        || tokens_contain_word(&non_filename_tokens, "implement")
        || tokens_contain_word(&non_filename_tokens, "refactor")
        || tokens_contain_word(&non_filename_tokens, "migrate")
        || tokens_contain_word(&non_filename_tokens, "patch")
        || tokens_contain_word(&non_filename_tokens, "delete")
        || tokens_contain_word(&non_filename_tokens, "remove")
        || tokens_contain_word(&non_filename_tokens, "wire")
        || tokens_contain_word(&non_filename_tokens, "deploy")
        || tokens_contain_word(&non_filename_tokens, "create")
        || tokens_contain_word(&non_filename_tokens, "develop")
        || tokens_contain_word(&non_filename_tokens, "provision")
        || tokens_contain_word(&non_filename_tokens, "mission")
        || tokens_contain_word(&non_filename_tokens, "perform")
        || tokens_contain_word(&non_filename_tokens, "execute")
        || (lower.contains("need")
            && (lower.contains("application")
                || lower.contains("app ")
                || lower.contains("software")))
        || (tokens_contain_word(&non_filename_tokens, "add")
            && (lower.contains("logging")
                || lower.contains("feature")
                || lower.contains("support")
                || lower.contains("endpoint")
                || lower.contains("test")))
        || (tokens_contain_word(&non_filename_tokens, "build")
            && !lower.contains("build system")
            && !lower.contains("the build")
            && !lower.contains("build passes")
            && !lower.contains("build fail"))
        || lower.contains("test failure")
        || lower.contains("assertion failed")
        || lower.contains("assertion failure")
        || lower.contains("test failed");

    if has_fix_verb {
        return ObjectiveClassification::Implementation;
    }

    // 10. Inspection verbs ("inspect", "examine")
    let inspection_verbs = ["inspect", "examine"];
    if inspection_verbs
        .iter()
        .any(|&v| tokens_contain_word(&non_filename_tokens, v))
    {
        return ObjectiveClassification::RepositoryInspection;
    }

    // 10b. Secondary Research verbs ("study", "analyze", "analyse", "explore", "survey")
    let secondary_research_verbs = ["study", "analyze", "analyse", "explore", "survey"];
    if secondary_research_verbs
        .iter()
        .any(|&v| tokens_contain_word(&non_filename_tokens, v))
    {
        return ObjectiveClassification::RepositoryResearch;
    }

    // 11. Repository Inspection for repo targets without mutation
    if has_repo_targets {
        return ObjectiveClassification::RepositoryInspection;
    }

    // 12. Direct Answer for pure conversational questions
    let direct_answer_starters = [
        "what is",
        "what are",
        "what does this function return",
        "explain this error message",
        "explain the repository module structure",
        "how does",
        "why does",
        "tell me about",
        "define",
        "summarize",
        "list",
        "show me",
    ];
    if direct_answer_starters
        .iter()
        .any(|&starter| lower.starts_with(starter) || lower.contains(starter))
    {
        return ObjectiveClassification::DirectAnswer;
    }

    // 13. Conservative fail-closed default: unrecognized or ambiguous requests
    // must NEVER receive write privileges! Bounded read-only inspection.
    ObjectiveClassification::RepositoryInspection
}

/// Fully resolve a prompt's request intent contract.
pub fn resolve_request_intent(prompt: &str) -> ResolvedRequestIntent {
    let classification = classify_objective(prompt);
    let is_mut_forbidden = is_mutation_forbidden(prompt);
    let is_read_only = is_mut_forbidden || classification.is_read_only();

    ResolvedRequestIntent {
        raw_prompt: prompt.to_string(),
        classification,
        is_mutation_forbidden: is_mut_forbidden,
        is_read_only,
        execution_constraints: Vec::new(),
        unresolved_ambiguity: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanResponse {
    pub plan_id: String,
    pub task_count: usize,
    pub candidate_plan: CandidatePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplanRequest {
    pub mission_id: MissionId,
    pub failed_task_id: TaskId,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplanResponse {
    pub new_plan_id: String,
    pub modified_tasks: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_plan: Option<CandidatePlan>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("plan generation failed: {0}")]
    GenerationFailed(String),

    #[error("replan failed: {0}")]
    ReplanFailed(String),
}

#[async_trait]
pub trait PlanService: Send + Sync {
    async fn has_valid_plan(&self, mission_id: MissionId) -> Result<bool, PlanError>;
    async fn generate_initial_plan(&self, req: PlanRequest) -> Result<PlanResponse, PlanError>;
    async fn replan(&self, req: ReplanRequest) -> Result<ReplanResponse, PlanError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_dto_serde() {
        let req = PlanRequest::new(MissionId::new(), "Build M31A");
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: PlanRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req, deserialized);

        let req_with_context = req.with_upstream_context(UpstreamPlanContext {
            project_name: "ExpenseTracker".to_string(),
            charter: "Charter details".to_string(),
            architecture: "Modular CLI".to_string(),
            requirements: vec!["REQ-01".to_string()],
            assumptions: vec!["ASSUME-01".to_string()],
            decisions: vec!["DEC-01".to_string()],
            research_summary: Some("Summary".to_string()),
            workflow_tier: "greenfield".to_string(),
            unknowns: vec!["UNK-01".to_string()],
            user_decisions: vec!["DECIDE-01".to_string()],
            resolved_invariants: vec!["INV-01".to_string()],
        });
        let s2 = serde_json::to_string(&req_with_context).unwrap();
        let d2: PlanRequest = serde_json::from_str(&s2).unwrap();
        assert_eq!(req_with_context, d2);
    }

    #[test]
    fn test_replan_dto_serde() {
        let req = ReplanRequest {
            mission_id: MissionId::new(),
            failed_task_id: TaskId::new(),
            reason: "Compiler error".to_string(),
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: ReplanRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req, deserialized);
    }
}
