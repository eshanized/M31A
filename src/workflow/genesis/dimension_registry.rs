//! Open research-dimension registry: existence authority for Genesis research dimensions.
//!
//! ## Authority model
//!
//! ```text
//! dimension definition (built-in data or registered extension)
//!     ↓
//! ResearchDimensionRegistry (existence authority)
//!     ↓
//! generic orchestration (researcher, synthesis, requirements, architecture)
//! ```
//!
//! [`ResearchDimension`] is pure identity. Everything behavioral — prompt
//! contract, researcher role, artifact filename, and the requirements-
//! synthesis mapping (key prefix, category, priority, rationale) — lives in
//! [`ResearchDimensionDefinition`]. Adding a dimension registers a
//! definition; the research engine iterates the decision's selected ids
//! without any per-dimension branch.
//!
//! ## No silent loss
//!
//! Every consumer resolves dimensions through this registry and fails
//! explicitly on unknown ids. Unknown artifact filenames are preserved in
//! an explicitly-marked unregistered bucket, never misattributed to an
//! unrelated dimension.

use crate::agent::registry::RoleRegistry;
use crate::planning::requirements::{RequirementCategory, RequirementPriority};
use crate::state_machine::agent::AgentRole;
use crate::workflow::genesis::research_decision::ResearchDimension;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use thiserror::Error;

/// Errors raised by dimension registration and resolution.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DimensionError {
    #[error("unknown research dimension '{id}': no registered dimension definition")]
    UnknownDimension { id: String },
    #[error("duplicate research dimension '{id}': already registered")]
    DuplicateDimension { id: String },
    #[error("duplicate research artifact filename '{filename}': already bound to '{existing}'")]
    DuplicateArtifact { filename: String, existing: String },
    #[error("invalid dimension definition for '{id}': {reason}")]
    InvalidDefinition { id: String, reason: String },
}

/// Full behavioral definition of one research dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchDimensionDefinition {
    /// Stable dimension id (`"stack"`, `"accessibility"`).
    pub id: ResearchDimension,
    /// Display name used in derived titles.
    pub display_name: String,
    /// Purpose description.
    pub description: String,
    /// PromptOS contract id for the dimension researcher step.
    pub prompt_contract: String,
    /// Researcher role executing the dimension step (must be registered).
    pub agent_role: AgentRole,
    /// Artifact filename produced by the dimension step (`"STACK.md"`).
    pub artifact_filename: String,
    /// Requirement key prefix for findings (`"SEC"` → `REQ-SEC-01`).
    pub requirement_prefix: String,
    /// Requirement category for finding-derived requirements.
    pub requirement_category: RequirementCategory,
    /// Requirement priority for finding-derived requirements.
    pub requirement_priority: RequirementPriority,
    /// Rationale recorded on finding-derived requirements.
    pub rationale: String,
    /// Member of the default exhaustive greenfield set (built-ins only).
    pub full_set: bool,
    /// Member of the targeted brownfield subset (built-ins only).
    pub brownfield_targeted: bool,
    /// Whether this definition ships with the runtime.
    pub builtin: bool,
}

/// Existence authority for research dimensions.
#[derive(Debug, Clone)]
pub struct ResearchDimensionRegistry {
    definitions: HashMap<String, ResearchDimensionDefinition>,
    by_filename: HashMap<String, String>,
    builtin_order: Vec<String>,
}

impl ResearchDimensionRegistry {
    /// Registry pre-populated with the six built-in dimension definitions.
    pub fn with_builtins() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            by_filename: HashMap::new(),
            builtin_order: Vec::new(),
        };
        for def in builtin_dimension_definitions() {
            registry.builtin_order.push(def.id.as_str().to_string());
            registry
                .by_filename
                .insert(def.artifact_filename.clone(), def.id.as_str().to_string());
            registry
                .definitions
                .insert(def.id.as_str().to_string(), def);
        }
        registry
    }

    /// Canonical global registry used by production call paths.
    pub fn global() -> &'static RwLock<ResearchDimensionRegistry> {
        static GLOBAL: OnceLock<RwLock<ResearchDimensionRegistry>> = OnceLock::new();
        GLOBAL.get_or_init(|| RwLock::new(ResearchDimensionRegistry::with_builtins()))
    }

    /// Register a new dimension definition (extension point).
    ///
    /// Custom definitions must pass `full_set: false` and
    /// `brownfield_targeted: false`: default research sets stay built-in so
    /// that registering an extension can never silently widen every
    /// greenfield decision. Extensions join a decision by explicit selection.
    /// The researcher role must already be registered (cross-registry check).
    pub fn register(&mut self, def: ResearchDimensionDefinition) -> Result<(), DimensionError> {
        let id = def.id.as_str().to_string();
        validate_dimension_definition(&def)?;
        if self.definitions.contains_key(&id) {
            return Err(DimensionError::DuplicateDimension { id });
        }
        if let Some(existing) = self.by_filename.get(&def.artifact_filename) {
            return Err(DimensionError::DuplicateArtifact {
                filename: def.artifact_filename.clone(),
                existing: existing.clone(),
            });
        }
        if !RoleRegistry::global()
            .read()
            .map(|roles| roles.contains(&def.agent_role))
            .unwrap_or(false)
        {
            return Err(DimensionError::InvalidDefinition {
                id: id.clone(),
                reason: format!(
                    "agent_role '{}' is not a registered role",
                    def.agent_role.as_str()
                ),
            });
        }
        if !def.builtin && (def.full_set || def.brownfield_targeted) {
            return Err(DimensionError::InvalidDefinition {
                id: id.clone(),
                reason: "custom dimensions cannot join default research sets; select them explicitly in a ResearchDecision".to_string(),
            });
        }
        self.by_filename
            .insert(def.artifact_filename.clone(), id.clone());
        self.definitions.insert(id, def);
        Ok(())
    }

    /// Resolve a dimension id to its definition. `None` = not registered.
    pub fn resolve(&self, id: &ResearchDimension) -> Option<&ResearchDimensionDefinition> {
        self.definitions.get(id.as_str())
    }

    /// Resolve an artifact filename to its dimension definition.
    pub fn resolve_filename(&self, filename: &str) -> Option<&ResearchDimensionDefinition> {
        self.by_filename
            .get(filename)
            .and_then(|id| self.definitions.get(id))
    }

    /// Whether a dimension id is registered.
    pub fn contains(&self, id: &ResearchDimension) -> bool {
        self.definitions.contains_key(id.as_str())
    }

    /// Default exhaustive greenfield set (built-ins flagged `full_set`).
    pub fn full_set(&self) -> Vec<ResearchDimension> {
        self.builtin_order
            .iter()
            .filter_map(|id| self.definitions.get(id))
            .filter(|def| def.full_set)
            .map(|def| def.id.clone())
            .collect()
    }

    /// Targeted brownfield subset (built-ins flagged `brownfield_targeted`).
    pub fn targeted_set(&self) -> Vec<ResearchDimension> {
        self.builtin_order
            .iter()
            .filter_map(|id| self.definitions.get(id))
            .filter(|def| def.brownfield_targeted)
            .map(|def| def.id.clone())
            .collect()
    }

    /// Built-in dimension ids in canonical order.
    pub fn builtin_ids(&self) -> Vec<String> {
        self.builtin_order.clone()
    }

    /// All registered dimension ids (built-ins first, then extensions).
    pub fn all_ids(&self) -> Vec<String> {
        let mut ids = self.builtin_order.clone();
        let mut extra: Vec<String> = self
            .definitions
            .keys()
            .filter(|k| !ids.contains(k))
            .cloned()
            .collect();
        extra.sort();
        ids.extend(extra);
        ids
    }

    /// Whether a selected set equals the default full set (order-insensitive).
    pub fn is_full_set(&self, selected: &[ResearchDimension]) -> bool {
        let mut full: Vec<String> = self
            .full_set()
            .iter()
            .map(|d| d.as_str().to_string())
            .collect();
        let mut sel: Vec<String> = selected.iter().map(|d| d.as_str().to_string()).collect();
        full.sort();
        sel.sort();
        full == sel
    }

    // -- Global convenience accessors --

    pub fn register_global(def: ResearchDimensionDefinition) -> Result<(), DimensionError> {
        Self::global()
            .write()
            .map_err(|_| DimensionError::InvalidDefinition {
                id: def.id.as_str().to_string(),
                reason: "dimension registry lock poisoned".to_string(),
            })
            .and_then(|mut r| r.register(def))
    }

    pub fn resolve_global(id: &ResearchDimension) -> Option<ResearchDimensionDefinition> {
        Self::global()
            .read()
            .ok()
            .and_then(|r| r.resolve(id).cloned())
    }
}

/// Validate a dimension definition's internal consistency.
fn validate_dimension_definition(def: &ResearchDimensionDefinition) -> Result<(), DimensionError> {
    let id = def.id.as_str().to_string();
    let invalid = |reason: String| DimensionError::InvalidDefinition {
        id: id.clone(),
        reason,
    };
    if id.is_empty() {
        return Err(invalid("dimension id cannot be empty".to_string()));
    }
    if !def.id.is_well_formed() {
        return Err(invalid(format!(
            "dimension id '{id}' must match [a-z0-9_]+"
        )));
    }
    if def.display_name.trim().is_empty() {
        return Err(invalid("display_name cannot be empty".to_string()));
    }
    if def.description.trim().is_empty() {
        return Err(invalid("description cannot be empty".to_string()));
    }
    if def.prompt_contract.trim().is_empty() {
        return Err(invalid("prompt_contract cannot be empty".to_string()));
    }
    if def.artifact_filename.trim().is_empty() || !def.artifact_filename.ends_with(".md") {
        return Err(invalid(
            "artifact_filename must be a non-empty Markdown filename".to_string(),
        ));
    }
    if def.requirement_prefix.trim().is_empty()
        || !def
            .requirement_prefix
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err(invalid(
            "requirement_prefix must be non-empty uppercase alphanumeric (e.g. \"SEC\")"
                .to_string(),
        ));
    }
    if def.rationale.trim().is_empty() {
        return Err(invalid("rationale cannot be empty".to_string()));
    }
    Ok(())
}

/// The six built-in dimension definitions, transcribed from the pre-27.5
/// `ResearchDimension` match arms (single authority since 27.5).
fn builtin_dimension_definitions() -> Vec<ResearchDimensionDefinition> {
    vec![
        ResearchDimensionDefinition {
            id: ResearchDimension::stack(),
            display_name: "Stack".to_string(),
            description: "Language editions, crates, versions, async runtimes, toolchains"
                .to_string(),
            prompt_contract: "genesis.research_stack".to_string(),
            agent_role: AgentRole::stack_researcher(),
            artifact_filename: "STACK.md".to_string(),
            requirement_prefix: "COMPAT".to_string(),
            requirement_category: RequirementCategory::Compatibility,
            requirement_priority: RequirementPriority::Must,
            rationale: "Toolchain and dependency compatibility".to_string(),
            full_set: true,
            brownfield_targeted: true,
            builtin: true,
        },
        ResearchDimensionDefinition {
            id: ResearchDimension::features(),
            display_name: "Features".to_string(),
            description: "Table stakes, competitive differentiators, anti-features, workflows"
                .to_string(),
            prompt_contract: "genesis.research_features".to_string(),
            agent_role: AgentRole::features_researcher(),
            artifact_filename: "FEATURES.md".to_string(),
            requirement_prefix: "FUNC".to_string(),
            requirement_category: RequirementCategory::Functional,
            requirement_priority: RequirementPriority::Should,
            rationale: "Researched capability or competitive differentiator".to_string(),
            full_set: true,
            brownfield_targeted: false,
            builtin: true,
        },
        ResearchDimensionDefinition {
            id: ResearchDimension::architecture(),
            display_name: "Architecture".to_string(),
            description: "Topologies, state management, IPC/network protocols, storage schemas"
                .to_string(),
            prompt_contract: "genesis.research_architecture".to_string(),
            agent_role: AgentRole::architecture_researcher(),
            artifact_filename: "ARCHITECTURE.md".to_string(),
            requirement_prefix: "ARCH".to_string(),
            requirement_category: RequirementCategory::Functional,
            requirement_priority: RequirementPriority::Should,
            rationale: "Researched architectural constraint or topology requirement".to_string(),
            full_set: true,
            brownfield_targeted: true,
            builtin: true,
        },
        ResearchDimensionDefinition {
            id: ResearchDimension::pitfalls(),
            display_name: "Pitfalls".to_string(),
            description: "Bottlenecks, concurrency hazards, upstream bugs, deprecations"
                .to_string(),
            prompt_contract: "genesis.research_pitfalls".to_string(),
            agent_role: AgentRole::pitfalls_researcher(),
            artifact_filename: "PITFALLS.md".to_string(),
            requirement_prefix: "NF".to_string(),
            requirement_category: RequirementCategory::Reliability,
            requirement_priority: RequirementPriority::Should,
            rationale: "Identified potential pitfall or bottleneck".to_string(),
            full_set: true,
            brownfield_targeted: true,
            builtin: true,
        },
        ResearchDimensionDefinition {
            id: ResearchDimension::security(),
            display_name: "Security".to_string(),
            description: "Auth models, permission checks, crypto requirements, sandboxing"
                .to_string(),
            prompt_contract: "genesis.research_security".to_string(),
            agent_role: AgentRole::security_researcher(),
            artifact_filename: "SECURITY.md".to_string(),
            requirement_prefix: "SEC".to_string(),
            requirement_category: RequirementCategory::Security,
            requirement_priority: RequirementPriority::Must,
            rationale: "Security research finding and recommendation".to_string(),
            full_set: true,
            brownfield_targeted: true,
            builtin: true,
        },
        ResearchDimensionDefinition {
            id: ResearchDimension::deployment(),
            display_name: "Deployment".to_string(),
            description: "Process supervisors, systemd, packaging, resource bounds".to_string(),
            prompt_contract: "genesis.research_deployment".to_string(),
            agent_role: AgentRole::deployment_researcher(),
            artifact_filename: "DEPLOYMENT.md".to_string(),
            requirement_prefix: "OPS".to_string(),
            requirement_category: RequirementCategory::Operational,
            requirement_priority: RequirementPriority::Should,
            rationale: "Deployment and supervision model".to_string(),
            full_set: true,
            brownfield_targeted: false,
            builtin: true,
        },
    ]
}
