//! Research decision logic, dimension definitions, and skip/trigger evaluation.

use super::dimension_registry::ResearchDimensionRegistry;
use super::environment::WorkspaceEnvironment;
use super::intake::{GenesisMode, GenesisOptions};
use super::project::{ProjectCharter, WorkflowTier};
use serde::{Deserialize, Serialize};

/// Open research-dimension identity.
///
/// A dimension is identified by a stable snake_case string (`"stack"`).
/// The six built-in ids are associated constructors; any other well-formed
/// id is a legitimate identity whose *existence and behavior* resolve
/// through [`super::dimension_registry::ResearchDimensionRegistry`].
///
/// This type carries NO behavioral metadata: prompt contracts, researcher
/// roles, artifact filenames, and synthesis mappings live in the registry.
/// Wire format is unchanged (plain strings), so persisted decisions
/// round-trip without migration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResearchDimension(String);

impl ResearchDimension {
    /// Construct a dimension identity, normalizing case and whitespace.
    ///
    /// Construction always succeeds for non-empty input: identity is cheap,
    /// existence is enforced by the registry, never by this constructor.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into().trim().to_lowercase())
    }

    /// Raw identity string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this identity is well-formed (non-empty, `a-z0-9_` charset).
    pub fn is_well_formed(&self) -> bool {
        !self.0.is_empty()
            && self
                .0
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    // Built-in dimension identities (stable; wire-compatible with pre-27.5).
    pub fn stack() -> Self {
        Self("stack".to_string())
    }
    pub fn features() -> Self {
        Self("features".to_string())
    }
    pub fn architecture() -> Self {
        Self("architecture".to_string())
    }
    pub fn pitfalls() -> Self {
        Self("pitfalls".to_string())
    }
    pub fn security() -> Self {
        Self("security".to_string())
    }
    pub fn deployment() -> Self {
        Self("deployment".to_string())
    }
}

impl std::fmt::Display for ResearchDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ResearchDimension {
    type Err = String;

    /// Parse a dimension identity. Accepts any well-formed id (registered
    /// or not); fails only on empty input. Unknown ids parse successfully
    /// here and are rejected explicitly by the registry at decision and
    /// synthesis boundaries.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase();
        if normalized.is_empty() {
            return Err("research dimension id cannot be empty".to_string());
        }
        Ok(Self(normalized))
    }
}

/// Evaluated research decision determining whether and which research dimensions run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchDecision {
    pub execute_research: bool,
    pub selected_dimensions: Vec<ResearchDimension>,
    pub rationale: String,
    pub skip_reason: Option<String>,
    pub trigger_reason: Option<String>,
}

impl ResearchDecision {
    pub fn skip(reason: impl Into<String>) -> Self {
        let reason_str = reason.into();
        Self {
            execute_research: false,
            selected_dimensions: Vec::new(),
            rationale: format!("Research skipped: {}", reason_str),
            skip_reason: Some(reason_str),
            trigger_reason: None,
        }
    }

    pub fn execute(dimensions: Vec<ResearchDimension>, trigger_reason: impl Into<String>) -> Self {
        let reason_str = trigger_reason.into();
        Self {
            execute_research: true,
            selected_dimensions: dimensions,
            rationale: format!("Research triggered for dimensions: {}", reason_str),
            skip_reason: None,
            trigger_reason: Some(reason_str),
        }
    }

    /// Whether this decision selected the registry's default exhaustive
    /// greenfield set (order-insensitive set equality, not a length check,
    /// so partially-overlapping custom selections never masquerade as full).
    pub fn is_full_greenfield(&self) -> bool {
        self.execute_research
            && ResearchDimensionRegistry::global()
                .read()
                .map(|registry| registry.is_full_set(&self.selected_dimensions))
                .unwrap_or(false)
    }

    /// Whether this decision selected targeted (non-exhaustive) research.
    pub fn is_targeted_delta(&self) -> bool {
        self.execute_research && !self.selected_dimensions.is_empty() && !self.is_full_greenfield()
    }

    /// Whether research was skipped (due to configuration, localized fix, or full specification).
    pub fn is_skip(&self) -> bool {
        !self.execute_research
    }
}

/// Evaluates research decision gate against configuration, project charter, and workspace environment.
///
/// Research decisions are governed by:
/// 1. Configuration (enable_research)
/// 2. Workspace-evidence-derived tier (Tiny/Medium skip)
/// 3. Ambiguity assessment (fully specified → skip)
/// 4. Genesis mode (Greenfield → exhaustive, Brownfield → targeted or full)
///
/// The model's IntentPipeline (AgentEngine) decides strategy for AgentEngine sessions.
/// Genesis research decisions use workspace evidence only — no prompt keyword scanning.
pub fn evaluate_research_decision(
    charter: &ProjectCharter,
    env: &WorkspaceEnvironment,
    options: &GenesisOptions,
) -> ResearchDecision {
    // 1. Skip condition: Explicitly disabled in configuration
    if !options.enable_research {
        return ResearchDecision::skip("workflow.research is disabled in configuration");
    }

    // 2. Skip condition: Fast-path workflow tiers. Tiny
    // and Medium tiers cover localized inspections and edits whose scope is
    // bounded by existing code; multi-dimensional research would cost more
    // than the work itself. The tier already encodes intent shape plus
    // workspace evidence, so no new enumeration is introduced here.
    if matches!(
        charter.workflow_tier,
        WorkflowTier::Tiny | WorkflowTier::Medium
    ) {
        return ResearchDecision::skip(
            "fast-path workflow tier (Tiny/Medium): scope bounded by existing code, research disproportionate",
        );
    }

    // 3. Skip condition: Fully specified tech stack and architecture with zero unresolved areas
    let is_fully_specified = charter.ambiguity_assessment.score_percent == 0
        && charter.ambiguity_assessment.unresolved_areas.is_empty()
        && !charter.technical_preferences.languages.is_empty()
        && charter.technical_preferences.architecture_style.is_some()
        && charter.technical_preferences.storage.is_some();

    if is_fully_specified {
        return ResearchDecision::skip(
            "Fully specified tech stack, architecture, and zero unresolved areas in charter",
        );
    }

    // 4. Trigger condition: Greenfield project initialization. The default
    // exhaustive set comes from the dimension registry, not a closed enum.
    if env.detected_mode == GenesisMode::Greenfield {
        let dimensions = ResearchDimensionRegistry::global()
            .read()
            .map(|registry| registry.full_set())
            .unwrap_or_default();
        return ResearchDecision::execute(
            dimensions,
            "Greenfield project initialization requires exhaustive multi-dimensional domain research",
        );
    }

    // 5. Default for Brownfield/Consequential with unresolved ambiguities:
    // execute targeted research using the registry's targeted subset.
    // The model's IntentPipeline makes the fine-grained decision
    // about which specific dimensions are needed; Genesis provides the
    // registry-defined targeted baseline.
    let brownfield_dims = ResearchDimensionRegistry::global()
        .read()
        .map(|registry| registry.targeted_set())
        .unwrap_or_default();
    ResearchDecision::execute(
        brownfield_dims,
        "Brownfield/Consequential project with unresolved ambiguities requires targeted domain research",
    )
}
