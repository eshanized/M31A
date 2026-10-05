//! Socratic discovery dialogue, ambiguity scoring, and convergence engine.

use super::environment::WorkspaceEnvironment;
use super::errors::GenesisError;
use super::intake::GenesisRequest;
use super::project::{AmbiguityAssessment, ProjectCharter, TargetDomainModel, WorkflowTier};
use crate::planning::requirements::{Provenance, ProvenanceSourceType, TrustLevel};
use crate::planning::risks::{Criticality, PlanningUnknown, UnknownFate};
use serde::{Deserialize, Serialize};

/// The four foundational pillars of Project Genesis discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryPillar {
    /// Pillar 1: Problem & Personas (pain solved, audience, success metrics).
    ProblemAndPersonas,
    /// Pillar 2: Boundaries & Non-Goals (scope limits, deferred features, anti-features).
    BoundariesAndNonGoals,
    /// Pillar 3: Technical Preferences (platforms, languages, storage, deployment).
    TechnicalPreferences,
    /// Pillar 4: Operational Invariants (scale, concurrency, security, licensing).
    OperationalInvariants,
}

impl std::fmt::Display for DiscoveryPillar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProblemAndPersonas => write!(f, "Problem & Personas"),
            Self::BoundariesAndNonGoals => write!(f, "Boundaries & Non-Goals"),
            Self::TechnicalPreferences => write!(f, "Technical Preferences"),
            Self::OperationalInvariants => write!(f, "Operational Invariants"),
        }
    }
}

/// A specific fact or constraint extracted during discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryFact {
    pub pillar: DiscoveryPillar,
    pub key: String,
    pub value: String,
}

impl DiscoveryFact {
    pub fn new(pillar: DiscoveryPillar, key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            pillar,
            key: key.into(),
            value: value.into(),
        }
    }
}

/// A single turn in the interactive Socratic discovery interview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryTurn {
    pub turn_number: u32,
    pub pillar_focus: DiscoveryPillar,
    pub question: String,
    pub options: Vec<String>,
    pub recommended_option: Option<String>,
    pub user_response: Option<String>,
    pub extracted_facts: Vec<DiscoveryFact>,
    pub ambiguity_after: u8,
}

/// Rationale for concluding the discovery interview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceReason {
    AmbiguityThresholdReached,
    MaxTurnsReached,
    DiminishingReturns,
    OperatorOverride,
}

impl std::fmt::Display for ConvergenceReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmbiguityThresholdReached => write!(f, "Ambiguity threshold reached"),
            Self::MaxTurnsReached => write!(f, "Max discovery turns reached"),
            Self::DiminishingReturns => write!(f, "Diminishing returns observed"),
            Self::OperatorOverride => write!(f, "Operator override requested"),
        }
    }
}

/// Active interactive discovery session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoverySession {
    pub request: GenesisRequest,
    pub environment: WorkspaceEnvironment,
    pub turns: Vec<DiscoveryTurn>,
    pub facts: Vec<DiscoveryFact>,
    pub current_ambiguity: u8,
    pub is_converged: bool,
    pub convergence_reason: Option<ConvergenceReason>,
    #[serde(default)]
    pub workflow_tier: WorkflowTier,
}

impl DiscoverySession {
    /// Initialize a new discovery session from a request and probed environment.
    pub fn new(request: GenesisRequest, environment: WorkspaceEnvironment) -> Self {
        let tier = classify_workflow_tier(&request.prompt, &environment);
        let prompt_len = request.prompt.trim().len();
        let initial_ambiguity = match tier {
            WorkflowTier::Tiny | WorkflowTier::Medium => 0,
            _ => {
                if prompt_len > 300 {
                    70
                } else if prompt_len > 100 {
                    85
                } else {
                    95
                }
            }
        };

        let is_converged = initial_ambiguity <= request.options.ambiguity_threshold_percent;
        let convergence_reason = if is_converged {
            Some(ConvergenceReason::AmbiguityThresholdReached)
        } else {
            None
        };

        Self {
            request,
            environment,
            turns: Vec::new(),
            facts: Vec::new(),
            current_ambiguity: initial_ambiguity,
            is_converged,
            convergence_reason,
            workflow_tier: tier,
        }
    }

    /// Access the classified workflow tier.
    pub fn workflow_tier(&self) -> WorkflowTier {
        self.workflow_tier
    }

    /// Extract classified unknowns for this session.
    pub fn unknowns(&self) -> Vec<PlanningUnknown> {
        extract_unknowns(&self.request.prompt, self.workflow_tier)
    }

    /// Returns true if this session has any unknowns that strictly require human decision.
    pub fn requires_user_decision(&self) -> bool {
        self.unknowns()
            .iter()
            .any(|u| u.fate == UnknownFate::UserDecisionRequired || u.fate == UnknownFate::Blocking)
    }

    /// Check whether discovery has converged according to rules.
    pub fn check_convergence(&self) -> (bool, Option<ConvergenceReason>) {
        if self.is_converged {
            return (true, self.convergence_reason.clone());
        }

        if self.current_ambiguity <= self.request.options.ambiguity_threshold_percent {
            return (true, Some(ConvergenceReason::AmbiguityThresholdReached));
        }

        if self.turns.len() >= self.request.options.max_discovery_turns {
            return (true, Some(ConvergenceReason::MaxTurnsReached));
        }

        if self.turns.len() >= 2 {
            let last_facts = self
                .turns
                .last()
                .map(|t| t.extracted_facts.len())
                .unwrap_or(0);
            let prev_facts = self
                .turns
                .get(self.turns.len() - 2)
                .map(|t| t.extracted_facts.len())
                .unwrap_or(0);
            if last_facts < 2 && prev_facts < 2 && self.current_ambiguity <= 35 {
                return (true, Some(ConvergenceReason::DiminishingReturns));
            }
        }

        (false, None)
    }

    /// Generate the next discovery turn question based on current missing pillars and facts.
    pub fn next_turn(&mut self) -> Result<Option<DiscoveryTurn>, GenesisError> {
        let (converged, reason) = self.check_convergence();
        if converged {
            self.is_converged = true;
            self.convergence_reason = reason;
            return Ok(None);
        }

        if self.workflow_tier == WorkflowTier::Tiny || self.workflow_tier == WorkflowTier::Medium {
            self.is_converged = true;
            self.convergence_reason = Some(ConvergenceReason::AmbiguityThresholdReached);
            return Ok(None);
        }

        let unknowns = extract_unknowns(&self.request.prompt, self.workflow_tier);
        let user_decision_unknown = unknowns.into_iter().find(|u| {
            u.fate == UnknownFate::UserDecisionRequired || u.fate == UnknownFate::Blocking
        });

        let turn_number = (self.turns.len() as u32) + 1;

        let (pillar_focus, question, options, recommended) =
            if let Some(ref unk) = user_decision_unknown {
                if turn_number == 1 {
                    (
                        DiscoveryPillar::TechnicalPreferences,
                        format!(
                            "{}: How should this architectural choice be resolved?",
                            unk.description
                        ),
                        vec![
                            unk.resolution
                                .clone()
                                .unwrap_or_else(|| "Safe reversible standard default".to_string()),
                            "Custom isolated enterprise deployment".to_string(),
                        ],
                        unk.resolution
                            .clone()
                            .unwrap_or_else(|| "Safe reversible standard default".to_string()),
                    )
                } else {
                    let pillar = match turn_number {
                        2 => DiscoveryPillar::ProblemAndPersonas,
                        3 => DiscoveryPillar::BoundariesAndNonGoals,
                        _ => DiscoveryPillar::OperationalInvariants,
                    };
                    let (q, opts, rec) = self.formulate_question_for_pillar(pillar);
                    (pillar, q, opts, rec)
                }
            } else {
                let pillar = match turn_number {
                    1 => DiscoveryPillar::ProblemAndPersonas,
                    2 => DiscoveryPillar::BoundariesAndNonGoals,
                    3 => DiscoveryPillar::TechnicalPreferences,
                    _ => DiscoveryPillar::OperationalInvariants,
                };
                let (q, opts, rec) = self.formulate_question_for_pillar(pillar);
                (pillar, q, opts, rec)
            };

        let turn = DiscoveryTurn {
            turn_number,
            pillar_focus,
            question,
            options,
            recommended_option: Some(recommended),
            user_response: None,
            extracted_facts: Vec::new(),
            ambiguity_after: self.current_ambiguity,
        };

        Ok(Some(turn))
    }

    /// Submit a user response to the current pending turn.
    pub fn submit_response(&mut self, response: &str) -> Result<bool, GenesisError> {
        if self.is_converged {
            return Ok(true);
        }

        let trimmed = response.trim();

        // 1. Check for operator override keywords
        let lower = trimmed.to_lowercase();
        let is_override = lower == "/skip"
            || lower == "skip"
            || lower == "proceed"
            || lower == "good enough"
            || lower == "continue"
            || lower.contains("good enough")
            || lower.contains("proceed")
            || lower.starts_with("/skip");

        if is_override {
            self.is_converged = true;
            self.convergence_reason = Some(ConvergenceReason::OperatorOverride);
            self.current_ambiguity = self
                .current_ambiguity
                .min(self.request.options.ambiguity_threshold_percent);
            return Ok(true);
        }

        // 2. Locate or create current turn
        let turn_idx = if let Some(idx) = self.turns.iter().rposition(|t| t.user_response.is_none())
        {
            idx
        } else {
            let turn_number = (self.turns.len() as u32) + 1;
            let pillar = match turn_number {
                1 => DiscoveryPillar::ProblemAndPersonas,
                2 => DiscoveryPillar::BoundariesAndNonGoals,
                3 => DiscoveryPillar::TechnicalPreferences,
                _ => DiscoveryPillar::OperationalInvariants,
            };
            let (q, opts, rec) = self.formulate_question_for_pillar(pillar);
            self.turns.push(DiscoveryTurn {
                turn_number,
                pillar_focus: pillar,
                question: q,
                options: opts,
                recommended_option: Some(rec),
                user_response: None,
                extracted_facts: Vec::new(),
                ambiguity_after: self.current_ambiguity,
            });
            self.turns.len() - 1
        };

        // 3. Extract facts from user response
        let pillar = self.turns[turn_idx].pillar_focus;
        let facts = extract_facts_from_response(pillar, trimmed);
        for fact in &facts {
            self.facts.push(fact.clone());
        }

        // 4. Update turn & ambiguity
        let reduction = (facts.len() as u8 * 12).min(25);
        self.current_ambiguity = self.current_ambiguity.saturating_sub(reduction);

        self.turns[turn_idx].user_response = Some(trimmed.to_string());
        self.turns[turn_idx].extracted_facts = facts;
        self.turns[turn_idx].ambiguity_after = self.current_ambiguity;

        // 5. Evaluate convergence
        let (converged, reason) = self.check_convergence();
        if converged {
            self.is_converged = true;
            self.convergence_reason = reason;
        }

        Ok(self.is_converged)
    }

    /// Synthesize all gathered facts, prompt, and environment into a formal ProjectCharter.
    pub fn synthesize_charter(&self) -> Result<ProjectCharter, GenesisError> {
        let title = extract_project_title(&self.request.prompt);
        let mut charter = ProjectCharter::new(title, &self.request.prompt);
        charter.workflow_tier = self.workflow_tier;
        let domain_model = infer_domain_model(&self.request.prompt);

        // Problem statement
        let mut problems = Vec::new();
        for fact in &self.facts {
            if fact.pillar == DiscoveryPillar::ProblemAndPersonas {
                if fact.key.contains("problem") || fact.key.contains("pain") {
                    problems.push(fact.value.clone());
                } else if fact.key.contains("persona") || fact.key.contains("audience") {
                    charter.target_personas.push(fact.value.clone());
                } else if fact.key.contains("metric") || fact.key.contains("success") {
                    charter.success_metrics.push(fact.value.clone());
                }
            } else if fact.pillar == DiscoveryPillar::BoundariesAndNonGoals {
                if fact.key.contains("scope") || fact.key.contains("in_scope") {
                    charter.boundaries.in_scope.push(fact.value.clone());
                } else if fact.key.contains("non_goal") || fact.key.contains("deferred") {
                    charter.boundaries.non_goals.push(fact.value.clone());
                } else if fact.key.contains("anti_feature") {
                    charter.boundaries.anti_features.push(fact.value.clone());
                }
            } else if fact.pillar == DiscoveryPillar::TechnicalPreferences {
                if fact.key.contains("language") {
                    charter
                        .technical_preferences
                        .languages
                        .push(fact.value.clone());
                } else if fact.key.contains("storage") {
                    charter.technical_preferences.storage = Some(fact.value.clone());
                } else if fact.key.contains("deployment") {
                    charter.technical_preferences.deployment_target = Some(fact.value.clone());
                } else if fact.key.contains("architecture") {
                    charter.technical_preferences.architecture_style = Some(fact.value.clone());
                } else if fact.key.contains("framework") {
                    charter
                        .technical_preferences
                        .frameworks
                        .push(fact.value.clone());
                }
            } else if fact.pillar == DiscoveryPillar::OperationalInvariants {
                if fact.key.contains("license") {
                    charter.operational_invariants.licensing = Some(fact.value.clone());
                } else if fact.key.contains("security") {
                    charter
                        .operational_invariants
                        .security_requirements
                        .push(fact.value.clone());
                } else if fact.key.contains("performance") {
                    charter
                        .operational_invariants
                        .performance_targets
                        .push(fact.value.clone());
                } else if fact.key.contains("scale") {
                    charter
                        .operational_invariants
                        .scale_targets
                        .push(fact.value.clone());
                }
            }
        }

        // Target personas come from dialogue facts only. When the operator
        // supplied none, the list stays empty and the ambiguity assessment
        // below records the gap — the runtime MUST NOT invent a generic
        // persona.

        // Technical preferences come from dialogue facts and probed
        // environment evidence only. When no evidence exists the fields stay
        // unset (honest "undecided") instead of asserting a runtime-chosen
        // stack: the runtime must not impose its own implementation choices
        // (language, storage, architecture style) on the target project.
        // Downstream synthesis records open decisions explicitly.
        if charter.technical_preferences.languages.is_empty()
            && let Some(stack) = &self.environment.detected_stack
        {
            charter.technical_preferences.languages.push(stack.clone());
        }

        // Incorporate domain model non-goals into charter boundaries
        for non_goal in &domain_model.non_goals {
            if !charter.boundaries.non_goals.contains(non_goal) {
                charter.boundaries.non_goals.push(non_goal.clone());
            }
        }

        if !problems.is_empty() {
            charter.problem_statement = problems.join("\n");
        } else {
            charter.problem_statement =
                format!("Address user requirements: {}", self.request.prompt);
        }

        let scope_from_evidence = !charter.boundaries.in_scope.is_empty();
        if !scope_from_evidence {
            // No scope evidence was gathered. The charter validation gate
            // requires at least one scope item, so record the request itself
            // explicitly marked as unrefined — a restatement of the operator's
            // words with UserPrompt provenance, never an invented domain
            // workflow.
            let prompt_echo: String = self
                .request
                .prompt
                .split_whitespace()
                .take(24)
                .collect::<Vec<_>>()
                .join(" ");
            charter
                .boundaries
                .in_scope
                .push(format!("User-requested scope (unrefined): {}", prompt_echo));
        }

        charter.domain_model = domain_model;

        // Ambiguity assessment
        let mut unresolved = Vec::new();
        let unknowns = extract_unknowns(&self.request.prompt, self.workflow_tier);
        for u in &unknowns {
            if u.fate == UnknownFate::UserDecisionRequired || u.fate == UnknownFate::Blocking {
                unresolved.push(format!("{}: {}", u.id, u.description));
            }
        }
        // Unevidenced charter areas are explicit unknowns, never silently
        // treated as resolved.
        if charter.target_personas.is_empty() {
            unresolved.push("Personas: no target persona elicited during discovery".to_string());
        }
        if !scope_from_evidence {
            unresolved.push(
                "Scope: no scope evidence gathered; recorded scope is the unrefined operator request"
                    .to_string(),
            );
        }

        let mut resolved = Vec::new();
        if scope_from_evidence {
            resolved.push("Core functional scope boundaries defined".to_string());
        }
        if !charter.technical_preferences.languages.is_empty() {
            resolved.push(format!(
                "Primary languages: {}",
                charter.technical_preferences.languages.join(", ")
            ));
        }
        for default_item in &charter.domain_model.inferred_defaults {
            resolved.push(format!("Inferred default: {}", default_item));
        }

        charter.ambiguity_assessment = AmbiguityAssessment {
            score_percent: self.current_ambiguity,
            unresolved_areas: unresolved,
            resolved_invariants: resolved,
            converged: self.is_converged,
        };

        charter.confirmed_by_user = self.is_converged;

        Ok(charter)
    }

    fn formulate_question_for_pillar(
        &self,
        pillar: DiscoveryPillar,
    ) -> (String, Vec<String>, String) {
        // Declarative authority boundary: questions are generated
        // from a pillar-generic template parameterized by the project title.
        // No tier- or product-specific question/answer scripts live here;
        // substantive options come from recorded unknowns and user evidence
        // (see the UserDecisionRequired path in `next_turn`).
        let project_name = extract_project_title(&self.request.prompt);
        match pillar {
            DiscoveryPillar::ProblemAndPersonas => (
                format!(
                    "What is the primary target persona and user workflow for {}?",
                    project_name
                ),
                vec![
                    format!("Individual operators managing {} directly", project_name),
                    "Team collaboration and shared multi-user workspaces".to_string(),
                    "Automated systems, scriptable CLI, or API service consumers".to_string(),
                ],
                format!("Individual operators managing {} directly", project_name),
            ),
            DiscoveryPillar::BoundariesAndNonGoals => (
                format!(
                    "What are the hard boundaries, deferred capabilities, or explicit non-goals for {} v1?",
                    project_name
                ),
                vec![
                    "Standalone single-service architecture with local or embedded data".to_string(),
                    "Core domain capabilities first (advanced external integrations deferred)".to_string(),
                    "Standard localized reporting and exports".to_string(),
                ],
                "Standalone single-service architecture with local or embedded data".to_string(),
            ),
            DiscoveryPillar::TechnicalPreferences => (
                format!(
                    "What technology stack preference should govern the implementation of {}?",
                    project_name
                ),
                vec![
                    "Type-safe language with strong compiler verification (e.g. Rust)".to_string(),
                    "Dynamic or web-focused stack (e.g. TypeScript / Python)".to_string(),
                    "Polyglot / agnostic standard architecture".to_string(),
                ],
                "Type-safe language with strong compiler verification (e.g. Rust)".to_string(),
            ),
            DiscoveryPillar::OperationalInvariants => (
                format!(
                    "What are the essential operational, security, and persistence requirements for {}?",
                    project_name
                ),
                vec![
                    "Deterministic local execution, zero unexpected network egress, persistent storage".to_string(),
                    "High-concurrency async I/O with structured audit logging".to_string(),
                    "Standard permissive open-source license with local regression tests".to_string(),
                ],
                "Deterministic local execution, zero unexpected network egress, persistent storage".to_string(),
            ),
        }
    }
}

/// Classify the workflow tier from intent shape and workspace evidence.
///
/// Declarative authority boundary: this classifier reasons about
/// *generic* properties only — the speech-act shape of the request
/// (read-only inquiry vs localized mutation vs new construction), observable
/// workspace evidence (greenfield vs existing codebase), and consequence
/// characteristics (tenancy, compliance, billing risk). It MUST NOT route on
/// product nouns or domain vocabulary: encountering a previously unseen
/// software domain must never require a new branch here. Domain content
/// (requirements, architecture, task plans) is model-derived downstream.
pub fn classify_workflow_tier(prompt: &str, env: &WorkspaceEnvironment) -> WorkflowTier {
    let lower = prompt.to_lowercase();
    let trimmed = lower.trim();

    // 1. Explicit Consequence Markers:
    // Risk properties and compliance/financial constraints that escalate to Consequential across any workspace.
    const CONSEQUENCE_MARKERS: &[&str] = &[
        "multi-tenant",
        "multi tenancy",
        "tenant isolation",
        "hipaa",
        "compliance boundary",
        "financial transaction",
        "live billing",
        "pci-dss",
        "soc2",
        "gdpr",
    ];
    if CONSEQUENCE_MARKERS.iter().any(|m| trimmed.contains(m)) {
        return WorkflowTier::Consequential;
    }

    // 2. Evidence-backed escalation: broad architectural-shift verbs applied to
    // an EXISTING codebase indicate a consequential change.
    const SHIFT_VERBS: &[&str] = &[
        "migrate",
        "rewrite",
        "redesign",
        "restructur",
        "convert",
        "re-architect",
        "rearchitect",
        "replace storage",
        "replace backend",
        "replace database",
        "database migration",
    ];
    if env.is_brownfield() && SHIFT_VERBS.iter().any(|v| trimmed.contains(v)) {
        return WorkflowTier::Consequential;
    }

    // 3. Read-Only / Inspection Intent:
    // Non-mutating queries, codebase status, architectural audits.
    // Inspect repository does NOT trigger unnecessary genesis.
    const READ_ONLY_PREFIXES: &[&str] = &[
        "inspect", "check", "show", "view", "list", "query", "status", "explain", "describe",
        "find", "search", "where is", "what is", "audit",
    ];
    if READ_ONLY_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
        return WorkflowTier::Tiny;
    }

    // 4. Localized code modification or extension:
    if trimmed.starts_with("fix ")
        || trimmed.starts_with("patch ")
        || trimmed.starts_with("update ")
        || trimmed.starts_with("refactor ")
        || trimmed.starts_with("modify ")
        || trimmed.starts_with("tweak ")
        || trimmed.starts_with("add ")
    {
        return WorkflowTier::Medium;
    }

    // 5. Trivial Edits:
    // Typo fixes, comment edits, version bumps.
    const TRIVIAL_INDICATORS: &[&str] =
        &["typo", "bump version", "tweak comment", "rename variable"];
    if TRIVIAL_INDICATORS.iter().any(|ind| trimmed.contains(ind)) {
        return WorkflowTier::Tiny;
    }

    // 5. Existing Codebase (Brownfield):
    // In an existing codebase, any constructive, mutating, or extending intent
    // (e.g. "build authentication", "implement oauth", "create login endpoint")
    // is Medium tier feature work. It NEVER becomes Greenfield / Standard.
    if env.is_brownfield() {
        return WorkflowTier::Medium;
    }

    // 6. Greenfield Workspace (Empty / Clean Slate):
    // Constructive application building in empty workspace is Standard.
    WorkflowTier::Standard
}

fn make_unknown(id: &str, desc: &str, criticality: Criticality) -> PlanningUnknown {
    PlanningUnknown::new(
        id,
        desc,
        criticality,
        Provenance::new(
            ProvenanceSourceType::UserPrompt,
            TrustLevel::AuthoritativeRuntime,
            "intent_expansion",
        ),
    )
}

/// Extract classified unknowns from tier and workspace evidence.
///
/// Declarative authority boundary: unknowns describe *generic*
/// uncertainty classes (persistence needs, schema shape, isolation strategy).
/// Product-specific unknowns (per-domain payment gateways, taxonomies,
/// curricula) MUST NOT be enumerated here — they are model-derived during
/// research/dialogue from the actual user intent. Adding a new software
/// domain must never require a new branch in this function.
pub fn extract_unknowns(prompt: &str, tier: WorkflowTier) -> Vec<PlanningUnknown> {
    let mut unknowns = Vec::new();
    let _prompt = prompt;

    match tier {
        WorkflowTier::Tiny => {
            // Tiny operations have no blocking or researchable unknowns
        }
        WorkflowTier::Medium => {
            // Localized edits have safe-to-infer unknowns regarding file structure
            unknowns.push(
                make_unknown(
                    "UNK-LOCAL-IMPL",
                    "Specific code location and styling conventions for change",
                    Criticality::Low,
                )
                .with_fate(UnknownFate::SafeToInfer)
                .with_resolution(
                    "Inspect existing codebase structure and conform to local patterns",
                ),
            );
        }
        WorkflowTier::Standard | WorkflowTier::Greenfield => {
            // General stack/architecture unknowns are safe to infer
            unknowns.push(
                make_unknown(
                    "UNK-STORAGE-BACKEND",
                    "Durable persistence needs and storage technology selection",
                    Criticality::Medium,
                )
                .with_fate(UnknownFate::SafeToInfer)
                .with_resolution(
                    "Select storage from durability and deployment evidence during synthesis",
                ),
            );

            unknowns.push(
                make_unknown(
                    "UNK-SCHEMA-DESIGN",
                    "Initial data model shape and entity relationships",
                    Criticality::Medium,
                )
                .with_fate(UnknownFate::SafeToInfer)
                .with_resolution(
                    "Derive data model from target domain entities during charter synthesis",
                ),
            );
        }
        WorkflowTier::Consequential => {
            // Consequential tier requires explicit user decision on critical architecture
            unknowns.push(
                make_unknown(
                    "UNK-TENANT-ISOLATION",
                    "Multi-tenant data isolation strategy (shared table with tenant_id vs schema-per-tenant)",
                    Criticality::High,
                )
                .with_fate(UnknownFate::UserDecisionRequired)
                .with_resolution("Present trade-offs to operator: row-level isolation vs separate schemas"),
            );
            unknowns.push(
                make_unknown(
                    "UNK-BILLING-TIERS",
                    "Subscription tiering and quota enforcement limits",
                    Criticality::Medium,
                )
                .with_fate(UnknownFate::SafeToInfer)
                .with_resolution("Infer default free and pro tier quotas with configurable limits"),
            );
        }
    }

    unknowns
}

/// Provide the target domain scaffold for charter synthesis.
///
/// Domain content (entities, workflows, defaults,
/// non-goals for a SPECIFIC product) is model-derived from user intent +
/// repository state + research — it MUST NOT come from a static template in
/// Rust, not even a "neutral" one. This function therefore returns an empty
/// scaffold: dialogue facts, research, and model reasoning supply the actual
/// domain substance downstream. A previously unseen software domain flows
/// through this path with zero source changes, and absence of evidence is
/// visible as emptiness rather than masked by placeholder entities.
pub fn infer_domain_model(_prompt: &str) -> TargetDomainModel {
    TargetDomainModel {
        entities: Vec::new(),
        core_workflows: Vec::new(),
        inferred_defaults: Vec::new(),
        non_goals: Vec::new(),
    }
}

fn extract_facts_from_response(pillar: DiscoveryPillar, text: &str) -> Vec<DiscoveryFact> {
    let mut facts = Vec::new();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return facts;
    }

    match pillar {
        DiscoveryPillar::ProblemAndPersonas => {
            facts.push(DiscoveryFact::new(pillar, "persona.primary", trimmed));
            facts.push(DiscoveryFact::new(pillar, "problem.statement", trimmed));
        }
        DiscoveryPillar::BoundariesAndNonGoals => {
            facts.push(DiscoveryFact::new(pillar, "scope.in_scope", trimmed));
            if trimmed.to_lowercase().contains("no ") || trimmed.to_lowercase().contains("deferred")
            {
                facts.push(DiscoveryFact::new(pillar, "scope.non_goal", trimmed));
            }
        }
        DiscoveryPillar::TechnicalPreferences => {
            facts.push(DiscoveryFact::new(
                pillar,
                "technical.architecture",
                trimmed,
            ));
            if trimmed.to_lowercase().contains("rust") {
                facts.push(DiscoveryFact::new(pillar, "technical.language", "Rust"));
            } else if trimmed.to_lowercase().contains("python") {
                facts.push(DiscoveryFact::new(pillar, "technical.language", "Python"));
            } else if trimmed.to_lowercase().contains("typescript")
                || trimmed.to_lowercase().contains("node")
            {
                facts.push(DiscoveryFact::new(
                    pillar,
                    "technical.language",
                    "TypeScript",
                ));
            } else if trimmed.to_lowercase().contains("go") {
                facts.push(DiscoveryFact::new(pillar, "technical.language", "Go"));
            }
        }
        DiscoveryPillar::OperationalInvariants => {
            facts.push(DiscoveryFact::new(
                pillar,
                "operational.invariants",
                trimmed,
            ));
            if trimmed.to_lowercase().contains("mit") || trimmed.to_lowercase().contains("apache") {
                facts.push(DiscoveryFact::new(pillar, "operational.license", trimmed));
            }
            if trimmed.to_lowercase().contains("security")
                || trimmed.to_lowercase().contains("sandbox")
            {
                facts.push(DiscoveryFact::new(pillar, "operational.security", trimmed));
            }
        }
    }

    facts
}

/// Derive a project title from the prompt using generic prefix stripping.
///
/// Only domain-neutral imperative prefixes are removed; no product table is
/// consulted. Unrecognized prompts fall back to a generic title so that
/// unseen domains never require a new branch here.
fn extract_project_title(prompt: &str) -> String {
    let lines: Vec<&str> = prompt
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if let Some(first) = lines.first() {
        let cleaned = first
            .trim_start_matches("Build a ")
            .trim_start_matches("build a ")
            .trim_start_matches("Build me an ")
            .trim_start_matches("build me an ")
            .trim_start_matches("Build me a ")
            .trim_start_matches("build me a ")
            .trim_start_matches("Create a ")
            .trim_start_matches("create a ")
            .trim_start_matches("Implement a ")
            .trim_start_matches("implement a ")
            .trim_start_matches("Make something like ")
            .trim_start_matches("make something like ")
            .trim_start_matches("Make me a ")
            .trim_start_matches("make me a ")
            .trim_start_matches('#')
            .trim();
        if !cleaned.is_empty() && cleaned.len() <= 60 {
            return to_title_case(cleaned);
        }
    }
    "M31A Generated Project".to_string()
}

/// Generic presentation helper: capitalize the first letter of each word.
/// Domain-neutral; carries no product knowledge.
fn to_title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Dynamic Uncertainty-Driven Questions ──────────────────────────────────

fn default_true() -> bool {
    true
}

/// Dynamic question proposed by model reasoning from unresolved unknowns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicQuestion {
    pub question_id: String,
    pub reason: String,
    pub target_unknown: String,
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default = "default_true")]
    pub allow_freeform: bool,
    #[serde(default = "default_true")]
    pub blocking: bool,
}

impl DynamicQuestion {
    pub fn new(
        question_id: impl Into<String>,
        reason: impl Into<String>,
        target_unknown: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            question_id: question_id.into(),
            reason: reason.into(),
            target_unknown: target_unknown.into(),
            text: text.into(),
            options: Vec::new(),
            allow_freeform: true,
            blocking: true,
        }
    }

    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.options = options;
        self
    }

    pub fn with_blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }

    pub fn with_allow_freeform(mut self, allow: bool) -> Self {
        self.allow_freeform = allow;
        self
    }
}

/// Failure modes during dynamic question validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum QuestionValidationError {
    #[error("Question ID cannot be empty")]
    EmptyQuestionId,

    #[error("Target unknown '{0}' does not exist or is already resolved in IntentState")]
    TargetUnknownNotFound(String),

    #[error("Question '{0}' targeting unknown '{1}' has already been answered")]
    QuestionAlreadyAnswered(String, String),

    #[error("Question contradicts previously resolved decision '{0}'")]
    ContradictsResolvedDecision(String),

    #[error("Options list is invalid: {0}")]
    InvalidOptions(String),

    #[error("Question text cannot be empty")]
    EmptyQuestionText,
}

/// Validate a model-proposed question against current IntentState and question history.
pub fn validate_dynamic_question(
    question: &DynamicQuestion,
    intent: &crate::agent::intent::IntentState,
    answered_question_ids: &[String],
) -> Result<(), QuestionValidationError> {
    if question.question_id.trim().is_empty() {
        return Err(QuestionValidationError::EmptyQuestionId);
    }
    if question.text.trim().is_empty() {
        return Err(QuestionValidationError::EmptyQuestionText);
    }

    // 1. Target unknown exists in intent state and is unresolved
    let unknown_exists = intent
        .unknowns
        .iter()
        .any(|u| u.id == question.target_unknown && !u.is_resolved());
    if !unknown_exists {
        return Err(QuestionValidationError::TargetUnknownNotFound(
            question.target_unknown.clone(),
        ));
    }

    // 2. Question has not already been answered
    if answered_question_ids.contains(&question.question_id) {
        return Err(QuestionValidationError::QuestionAlreadyAnswered(
            question.question_id.clone(),
            question.target_unknown.clone(),
        ));
    }

    // 3. Question does not contradict resolved decisions
    for decision in &intent.decisions {
        if decision.status == crate::agent::intent::DecisionStatus::Resolved
            && decision
                .question
                .trim()
                .eq_ignore_ascii_case(question.text.trim())
        {
            return Err(QuestionValidationError::ContradictsResolvedDecision(
                decision.id.clone(),
            ));
        }
    }

    // 4. Options are structurally valid
    if !question.options.is_empty() {
        let mut seen = std::collections::HashSet::new();
        for opt in &question.options {
            if opt.trim().is_empty() {
                return Err(QuestionValidationError::InvalidOptions(
                    "Option text cannot be empty".to_string(),
                ));
            }
            if !seen.insert(opt.trim().to_lowercase()) {
                return Err(QuestionValidationError::InvalidOptions(format!(
                    "Duplicate option '{}'",
                    opt
                )));
            }
        }
    } else if !question.allow_freeform {
        return Err(QuestionValidationError::InvalidOptions(
            "Question must allow freeform if no options are specified".to_string(),
        ));
    }

    Ok(())
}

/// Ingest user answer into IntentState with authoritative UserProvided provenance.
pub fn apply_question_answer(
    intent: &mut crate::agent::intent::IntentState,
    question: &DynamicQuestion,
    answer: &str,
    _operator: &str,
) {
    let now = chrono::Utc::now();

    // 1. Resolve target unknown
    if let Some(unk) = intent
        .unknowns
        .iter_mut()
        .find(|u| u.id == question.target_unknown)
    {
        unk.resolve(crate::agent::intent::UnknownResolution::UserDecision {
            user_answer: answer.to_string(),
            question_asked: question.text.clone(),
        });
    }

    // 2. Record consequential decision
    intent.decisions.push(crate::agent::intent::IntentDecision {
        id: format!("dec-{}", question.question_id),
        question: question.text.clone(),
        options_considered: question.options.clone(),
        status: crate::agent::intent::DecisionStatus::Resolved,
        resolution: Some(crate::agent::intent::DecisionResolution::UserSelected {
            selected_option: answer.to_string(),
            raw_answer: answer.to_string(),
        }),
        consequence_rationale: question.reason.clone(),
        created_at: now,
    });

    // 3. Record known fact with UserProvided provenance
    intent.known_facts.push(crate::agent::intent::IntentFact {
        key: format!("question_{}", question.question_id),
        value: format!("{}: {}", question.text, answer),
        origin: crate::agent::intent::FactOrigin::UserProvided,
        created_at: now,
    });

    intent.version += 1;
    intent.updated_at = now;
}

/// Determine whether dynamic discovery has converged based on unresolved unknowns and proposed questions.
///
/// Discovery converges ONLY when:
/// 1. There are NO unresolved blocking, critical, or user-decision unknowns in IntentState.
/// 2. There are NO proposed blocking questions awaiting operator input.
pub fn is_discovery_converged(
    intent: &crate::agent::intent::IntentState,
    proposed_questions: &[DynamicQuestion],
) -> bool {
    let has_blocking_unknowns = intent.unknowns.iter().any(|u| {
        !u.is_resolved()
            && (u.fate == crate::planning::risks::UnknownFate::Blocking
                || u.fate == crate::planning::risks::UnknownFate::UserDecisionRequired
                || u.criticality.is_blocking_threshold())
    });

    if has_blocking_unknowns {
        return false;
    }

    let has_blocking_question = proposed_questions.iter().any(|q| q.blocking);
    !has_blocking_question
}

/// Failure modes during dynamic question answer validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum AnswerValidationError {
    #[error("Answer text cannot be empty")]
    EmptyAnswer,

    #[error("Target unknown '{0}' for question '{1}' is already resolved")]
    TargetUnknownAlreadyResolved(String, String),

    #[error("Question '{0}' has already been answered")]
    QuestionAlreadyAnswered(String),

    #[error(
        "Answer '{0}' does not match any allowed option for question '{1}'. Allowed options: {2:?}"
    )]
    OptionMismatch(String, String, Vec<String>),
}

/// Validate an answer to a DynamicQuestion against the question definition and IntentState.
pub fn validate_question_answer(
    question: &DynamicQuestion,
    answer: &str,
    intent: &crate::agent::intent::IntentState,
) -> Result<(), AnswerValidationError> {
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Err(AnswerValidationError::EmptyAnswer);
    }

    // Check if target unknown is already resolved
    if let Some(unk) = intent
        .unknowns
        .iter()
        .find(|u| u.id == question.target_unknown)
    {
        if unk.is_resolved() {
            return Err(AnswerValidationError::TargetUnknownAlreadyResolved(
                question.target_unknown.clone(),
                question.question_id.clone(),
            ));
        }
    }

    // If options are specified and freeform is not allowed, must match one of the canonical options
    if !question.options.is_empty() && !question.allow_freeform {
        let matches = question
            .options
            .iter()
            .any(|opt| opt.trim().eq_ignore_ascii_case(trimmed) || opt == answer);
        if !matches {
            return Err(AnswerValidationError::OptionMismatch(
                answer.to_string(),
                question.question_id.clone(),
                question.options.clone(),
            ));
        }
    }

    Ok(())
}
