//! Authoritative prompt reachability classification (wiring remediation v0.1.1).
//!
//! Every shipped prompt asset has EXACTLY ONE classification describing how
//! (or whether) the production runtime can reach it. Embedding a TOML via
//! `include_str!` is NOT consumption: a prompt is production-reachable only
//! when its identifier can flow into an actual model invocation through the
//! production runtime spine
//! (`PromptReference` → task/role binding → `PromptCatalog` →
//! `PromptCompiler` → `EffectivePrompt` → `ModelCaller`).
//!
//! The CI gate ([`ci_bucket`]) maps each classification into one of four
//! release buckets. A shipped prompt with no classification fails CI.

use serde::{Deserialize, Serialize};

/// Authoritative reachability classification for one prompt asset.
///
/// Exactly one variant applies per asset (see
/// `docs/audits/PROMPT-REACHABILITY-v0.1.1.md` for the per-asset rationale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptReachability {
    /// Directly resolved and compiled by production execution
    /// (P0 safety, planning/decompose/revision, discovery questions,
    /// review/diagnosis executors).
    Direct,
    /// Resolved through `AgentRole` / `AgentProfile` / `PromptReference`
    /// role defaults (all `agent.*` contracts).
    RoleBound,
    /// Selected by a workflow/task step and passed into execution with a
    /// typed `PromptReference` (research wave, synthesis, roadmap steps).
    WorkflowBound,
    /// Reached through another typed runtime abstraction (catalog alias
    /// routing, canonical-version upgrades).
    Indirect,
    /// Only referenced by tests (bare compatibility aliases, skill
    /// guidance exercised through the test-only guidance path).
    TestOnly,
    /// Identifier exists only as artifact provenance / workflow metadata /
    /// description labels, never entering model context.
    MetadataOnly,
    /// Compatibility asset intentionally retained (superseded same-id v1
    /// generations, renamed legacy ids with alias routing).
    Legacy,
    /// No legitimate production path. Post-remediation this variant MUST
    /// have zero members: every shipped asset is classified into a CI
    /// bucket, and anything without a purpose is deprecated, not left
    /// unreachable.
    Unreachable,
}

impl PromptReachability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::RoleBound => "role_bound",
            Self::WorkflowBound => "workflow_bound",
            Self::Indirect => "indirect",
            Self::TestOnly => "test_only",
            Self::MetadataOnly => "metadata_only",
            Self::Legacy => "legacy",
            Self::Unreachable => "unreachable",
        }
    }
}

/// CI release bucket for a shipped prompt asset.
///
/// Every shipped prompt MUST fall into exactly one bucket. Prompts with no
/// legitimate purpose are explicitly deprecated (file retained for label
/// stability), never silently accumulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCiBucket {
    /// Compiled into a real model invocation on a production path.
    ProductionReachable,
    /// Retained for version migration (alias routing, v1 generations).
    IntentionalCompatibility,
    /// Exercised only by test fixtures / guidance paths.
    IntentionalTestFixture,
    /// Retained file with no execution role; documented in the audit.
    ExplicitlyDeprecated,
}

impl PromptCiBucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProductionReachable => "production_reachable",
            Self::IntentionalCompatibility => "intentional_compatibility",
            Self::IntentionalTestFixture => "intentional_test_fixture",
            Self::ExplicitlyDeprecated => "explicitly_deprecated",
        }
    }
}

/// Classify one built-in prompt contract by `(id, version)`.
///
/// Returns `None` when the asset is unknown to the remediation inventory
/// (CI treats unknown assets as failures).
pub fn classify_builtin(id: &str, version: u32) -> Option<(PromptReachability, PromptCiBucket)> {
    use PromptCiBucket as Bucket;
    use PromptReachability as Reach;
    let result = match (id, version) {
        // ── Layer 0 ──────────────────────────────────────────────
        ("core.safety", 2) => (Reach::Direct, Bucket::ProductionReachable),
        ("runtime.safety_invariants", 1) => (Reach::Legacy, Bucket::ExplicitlyDeprecated),

        // ── Agent role contracts (registry-bound) ────────────────
        ("agent.implementer", 2)
        | ("agent.reviewer", 2)
        | ("agent.verifier", 2)
        | ("agent.diagnostician", 2)
        | ("agent.auditor", 2)
        | ("agent.release_certifier", 2) => (Reach::RoleBound, Bucket::ProductionReachable),
        ("agent.planner", 1)
        | ("agent.researcher", 1)
        | ("agent.architect", 1)
        | ("agent.integrator", 1)
        | ("agent.discovery_analyst", 1)
        | ("agent.stack_researcher", 1)
        | ("agent.features_researcher", 1)
        | ("agent.architecture_researcher", 1)
        | ("agent.pitfalls_researcher", 1)
        | ("agent.security_researcher", 1)
        | ("agent.deployment_researcher", 1)
        | ("agent.synthesizer", 1) => (Reach::RoleBound, Bucket::ProductionReachable),
        // Superseded v1 generations of bumped roles (canonical is v2).
        ("agent.implementer", 1)
        | ("agent.reviewer", 1)
        | ("agent.verifier", 1)
        | ("agent.diagnostician", 1) => (Reach::Legacy, Bucket::IntentionalCompatibility),

        // ── Execution / workflow stage contracts ─────────────────
        ("execution.implementer", 2) => (Reach::WorkflowBound, Bucket::ProductionReachable),
        ("execution.implementer", 1) => (Reach::Legacy, Bucket::IntentionalCompatibility),
        ("execution.authorization_explanation", 1) => {
            (Reach::MetadataOnly, Bucket::ExplicitlyDeprecated)
        }
        ("execution.verifier", 1) => (Reach::Legacy, Bucket::ExplicitlyDeprecated),
        // Bare compatibility aliases (test fixtures only).
        ("implement", 1) | ("review", 1) | ("verify", 1) | ("diagnose", 1) => {
            (Reach::TestOnly, Bucket::IntentionalTestFixture)
        }

        // ── Genesis ──────────────────────────────────────────────
        ("genesis.research_stack", 1)
        | ("genesis.research_features", 1)
        | ("genesis.research_architecture", 1)
        | ("genesis.research_pitfalls", 1)
        | ("genesis.research_security", 1)
        | ("genesis.research_deployment", 1)
        | ("genesis.research_synthesis", 1) => (Reach::WorkflowBound, Bucket::ProductionReachable),
        ("genesis.dynamic_questions", 1) => (Reach::Direct, Bucket::ProductionReachable),
        // Deterministic-synthesis descriptors: artifact provenance labels
        // only (no model path). Retained, explicitly deprecated as
        // execution contracts.
        ("genesis.discovery", 1)
        | ("genesis.charter", 1)
        | ("genesis.synthesis", 1)
        | ("genesis.requirements", 1)
        | ("genesis.architecture", 1)
        | ("genesis.adr", 1)
        | ("genesis.risks", 1)
        | ("genesis.roadmap", 1) => (Reach::MetadataOnly, Bucket::ExplicitlyDeprecated),

        // ── Planning ─────────────────────────────────────────────
        ("planning.decompose", 2) => (Reach::Direct, Bucket::ProductionReachable),
        ("planning.decompose", 1) => (Reach::Legacy, Bucket::IntentionalCompatibility),
        ("planning.revision", 1) | ("planning.task_revision", 1) => {
            (Reach::Direct, Bucket::ProductionReachable)
        }

        // ── Verification / recovery ──────────────────────────────
        ("verification.reviewer", 2) => (Reach::Direct, Bucket::ProductionReachable),
        ("execution.reviewer", 1) => (Reach::Legacy, Bucket::IntentionalCompatibility),
        // Canonical task-verification contract, bound by verification
        // workflow steps through the production worker path.
        ("verification.task", 2) => (Reach::WorkflowBound, Bucket::ProductionReachable),
        ("recovery.diagnostician", 2) => (Reach::Direct, Bucket::ProductionReachable),
        ("execution.diagnostician", 1) => (Reach::Legacy, Bucket::IntentionalCompatibility),

        // ── Skill guidance (lower trust, test-exercised) ─────────
        ("skill.in_task_guidance", 1) => (Reach::TestOnly, Bucket::IntentionalTestFixture),

        _ => return None,
    };
    Some(result)
}

/// Canonical version for a prompt id: the generation all normal production
/// consumers must resolve to. `None` means the id has a single generation.
pub fn canonical_version(id: &str) -> Option<u32> {
    match id {
        "agent.implementer"
        | "agent.reviewer"
        | "agent.verifier"
        | "agent.diagnostician"
        | "execution.implementer"
        | "planning.decompose" => Some(2),
        _ => None,
    }
}

/// One classified catalog row: `(id, version, reachability, CI bucket)`.
pub type ReachabilityRow = (String, u32, PromptReachability, PromptCiBucket);

/// One unclassified catalog entry: `(id, version)`.
pub type UnclassifiedRow = (String, u32);

/// Enumerate the full reachability matrix for every contract in a catalog.
///
/// Returns `(classified rows, unclassified entries)` — the latter must be
/// empty for CI.
pub fn builtin_reachability_matrix(
    catalog: &dyn crate::prompt::PromptCatalog,
) -> (Vec<ReachabilityRow>, Vec<UnclassifiedRow>) {
    let mut rows = Vec::new();
    let mut unclassified = Vec::new();
    for meta in catalog.list() {
        match classify_builtin(&meta.id, meta.version) {
            Some((reach, bucket)) => rows.push((meta.id, meta.version, reach, bucket)),
            None => unclassified.push((meta.id, meta.version)),
        }
    }
    rows.sort();
    unclassified.sort();
    (rows, unclassified)
}
