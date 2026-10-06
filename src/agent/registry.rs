//! Open role registry: existence and metadata authority for agent roles.
//!
//! ## Authority model
//!
//! ```text
//! role definition (built-in data or registered extension)
//!     ↓
//! RoleRegistry (existence authority: resolve / profile_for / infer)
//!     ↓
//! generic consumers (dispatcher, planner, compiler, verification, scheduler)
//! ```
//!
//! [`AgentRole`](crate::state_machine::agent::AgentRole) is pure identity.
//! Everything behavioral — prompt binding, capability envelope, verification
//! defaults, lifecycle stage, concurrency limit, inference triggers — lives
//! in [`RoleDefinition`] records. Adding a role registers a definition; no
//! core `match` arm needs to learn that the role exists.
//!
//! ## Safety
//!
//! The registry declares *intended* envelopes; it never authorizes side
//! effects. Enforcement stays in Rust runtime authority:
//!
//! - dispatch-time envelope intersection
//!   ([`calculate_eligible_capabilities`](crate::agent::envelope::calculate_eligible_capabilities));
//! - per-tool policy evaluation in the tool pipeline;
//! - sandbox construction from the validated `sandbox_policy` set;
//! - step ceilings via [`AgentProfile::apply_override`](crate::agent::profile::AgentProfile::apply_override).
//!
//! Registration validates internal consistency (flags match the envelope,
//! required metadata present) and rejects incoherent definitions explicitly.

use crate::agent::profile::AgentProfile;
use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement, VerificationStrategy};
use crate::prompt::{MissionStage, PromptReference};
use crate::state_machine::agent::AgentRole;
use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};
use thiserror::Error;

/// Sandbox policies understood by the runtime sandbox construction.
///
/// Registration rejects any other value explicitly: an unknown sandbox
/// policy has undefined enforcement, so it must never silently load.
pub const KNOWN_SANDBOX_POLICIES: &[&str] = &["read_only", "workspace_write", "workspace_verify"];

/// Default per-role concurrency when a definition declares no explicit limit.
pub const DEFAULT_PER_ROLE_CONCURRENCY: usize =
    crate::config::canonical::DEFAULT_PER_ROLE_CONCURRENCY;

/// Fallback temperature (milli-Celsius → float at use) when a role has no
/// registered model policy. Reachable only for unregistered ids, which
/// production paths reject before routing.
pub const DEFAULT_ROLE_TEMPERATURE_MILLICELSIUS: u32 = 200;

/// Errors raised by role registration and resolution.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RoleError {
    #[error("unknown agent role '{id}': no registered role definition")]
    UnknownRole { id: String },
    #[error("duplicate agent role '{id}': already registered")]
    DuplicateRole { id: String },
    #[error("invalid role definition for '{id}': {reason}")]
    InvalidDefinition { id: String, reason: String },
    #[error(
        "duplicate inference rule for capability '{capability}' (already bound to '{existing}')"
    )]
    DuplicateInferenceRule {
        capability: String,
        existing: String,
    },
}

/// Full behavioral definition of one agent role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDefinition {
    /// Stable role id (`"implementer"`, `"database_architect"`).
    pub id: AgentRole,
    /// Human-readable description.
    pub description: String,
    /// Normalized historic aliases resolving to this id.
    pub aliases: Vec<String>,
    /// PromptOS contract id bound to this role.
    pub prompt_contract: String,
    /// PromptOS contract version.
    pub prompt_version: u32,
    /// Canonical operating stage (contract `stage` remains authoritative;
    /// this is the registry fallback, replacing `canonical_stage`).
    pub stage: MissionStage,
    /// Researcher-class membership (replaces `is_researcher`).
    pub researcher_class: bool,
    /// Read-only verification fast-path eligibility (gates/hierarchy).
    pub read_only_verification: bool,
    /// Whether the role may invoke workspace write tools (runner gate).
    /// Registration requires the envelope to actually grant file write.
    pub write_tools_permitted: bool,
    /// Whether completion requires observed workspace modification
    /// (runner premature-completion gate; implementer-class only).
    pub requires_modification: bool,
    /// Default verification strategy for steps of this role
    /// (replaces the `map_step_verification` role table).
    pub default_verification: VerificationStrategy,
    /// Default capabilities assigned when a plan omits them
    /// (replaces the planner role→capability table).
    pub default_capabilities: Vec<CapabilityRequirement>,
    /// Per-role concurrency limit; `None` → [`DEFAULT_PER_ROLE_CONCURRENCY`].
    pub concurrency_limit: Option<usize>,
    /// Agent profile template materialized by [`RoleRegistry::profile_for`].
    pub profile: AgentProfile,
}

/// Ordered capability→role inference rule.
///
/// Rules are consulted in order; the first rule whose capability appears in
/// the request wins. The built-in order reproduces the pre-27.5 dispatcher /
/// planner precedence exactly. New rules may only bind previously unbound
/// capabilities (overriding built-in precedence is rejected explicitly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceRule {
    pub capability: String,
    pub role: AgentRole,
}

/// Existence + metadata authority for agent roles.
#[derive(Debug, Clone)]
pub struct RoleRegistry {
    definitions: HashMap<String, RoleDefinition>,
    inference_rules: Vec<InferenceRule>,
    builtin_order: Vec<String>,
}

impl RoleRegistry {
    /// Registry pre-populated with the 18 built-in role definitions.
    pub fn with_builtins() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            inference_rules: Vec::new(),
            builtin_order: Vec::new(),
        };
        for def in builtin_role_definitions() {
            registry.builtin_order.push(def.id.as_str().to_string());
            registry
                .definitions
                .insert(def.id.as_str().to_string(), def);
        }
        for (capability, role) in builtin_inference_rules() {
            registry.inference_rules.push(InferenceRule {
                capability: capability.to_string(),
                role: AgentRole::new(role),
            });
        }
        registry
    }

    /// Canonical global registry used by production call paths.
    pub fn global() -> &'static RwLock<RoleRegistry> {
        static GLOBAL: OnceLock<RwLock<RoleRegistry>> = OnceLock::new();
        GLOBAL.get_or_init(|| RwLock::new(RoleRegistry::with_builtins()))
    }

    /// Register a new role definition (extension point).
    ///
    /// Fails explicitly on duplicates, malformed metadata, or internally
    /// inconsistent privilege declarations. Registration never grants
    /// authority — enforcement stays with envelope intersection, policy,
    /// and sandbox construction.
    pub fn register(&mut self, def: RoleDefinition) -> Result<(), RoleError> {
        let id = def.id.as_str().to_string();
        validate_role_definition(&def)?;
        if self.definitions.contains_key(&id) {
            return Err(RoleError::DuplicateRole { id });
        }
        // Aliases must not shadow another role id or alias (hijack guard).
        let mut taken: HashSet<String> = self.definitions.keys().cloned().collect();
        for existing in self.definitions.values() {
            taken.extend(existing.aliases.iter().cloned());
        }
        for alias in &def.aliases {
            if taken.contains(alias) {
                return Err(RoleError::InvalidDefinition {
                    id: id.clone(),
                    reason: format!("alias '{alias}' collides with an existing role id or alias"),
                });
            }
        }
        self.definitions.insert(id, def);
        Ok(())
    }

    /// Register an additional capability→role inference rule.
    ///
    /// Appended after built-in rules; binding an already-bound capability
    /// is rejected so extensions cannot silently hijack built-in routing.
    /// Explicit `role:` task tags always take precedence over inference.
    pub fn register_inference_rule(
        &mut self,
        capability: impl Into<String>,
        role: &AgentRole,
    ) -> Result<(), RoleError> {
        let capability = capability.into();
        if !self.definitions.contains_key(role.as_str()) {
            return Err(RoleError::UnknownRole {
                id: role.as_str().to_string(),
            });
        }
        if let Some(existing) = self
            .inference_rules
            .iter()
            .find(|r| r.capability == capability)
        {
            return Err(RoleError::DuplicateInferenceRule {
                capability,
                existing: existing.role.as_str().to_string(),
            });
        }
        self.inference_rules.push(InferenceRule {
            capability,
            role: role.clone(),
        });
        Ok(())
    }

    /// Resolve a role id to its definition. `None` = not registered.
    pub fn resolve(&self, id: &AgentRole) -> Option<&RoleDefinition> {
        self.definitions.get(id.as_str())
    }

    /// Whether a role id is registered.
    pub fn contains(&self, id: &AgentRole) -> bool {
        self.definitions.contains_key(id.as_str())
    }

    /// Materialize the immutable agent profile for a registered role.
    pub fn profile_for(&self, id: &AgentRole) -> Result<AgentProfile, RoleError> {
        self.resolve(id)
            .map(|def| {
                let mut profile = def.profile.clone();
                profile.role = def.id.clone();
                profile.prompt_ref =
                    PromptReference::new(def.prompt_contract.clone(), def.prompt_version);
                profile
            })
            .ok_or_else(|| RoleError::UnknownRole {
                id: id.as_str().to_string(),
            })
    }

    /// Infer a role from requested capabilities using ordered rules.
    ///
    /// Shared by the planner and the dispatcher (single authority replacing
    /// both hardcoded if-chains). Returns `None` when no rule matches; each
    /// caller applies its own documented fallback.
    pub fn infer_role_for_capabilities(&self, capabilities: &[String]) -> Option<AgentRole> {
        for rule in &self.inference_rules {
            if capabilities.iter().any(|c| c == &rule.capability) {
                return Some(rule.role.clone());
            }
        }
        None
    }

    /// First role (in inference-rule order) permitted write tools.
    ///
    /// Generic replacement for the hardcoded "coerce mutating tasks to
    /// Implementer" rule: with built-ins this resolves to `implementer`.
    pub fn first_writable_role(&self) -> Option<AgentRole> {
        for rule in &self.inference_rules {
            if let Some(def) = self.resolve(&rule.role)
                && def.write_tools_permitted
            {
                return Some(rule.role.clone());
            }
        }
        None
    }

    /// Canonical prompt reference for a role id.
    ///
    /// Registered roles resolve to their declared contract; unregistered ids
    /// follow the `agent.{id}` v1 naming convention. The convention alone
    /// grants nothing — prompt existence is enforced by the catalog and role
    /// existence by this registry at validation boundaries.
    pub fn prompt_reference_for(&self, id: &AgentRole) -> PromptReference {
        if let Some(def) = self.resolve(id) {
            PromptReference::new(def.prompt_contract.clone(), def.prompt_version)
        } else {
            PromptReference::new(format!("agent.{}", id.as_str()), 1)
        }
    }

    /// Canonical stage fallback for a role id.
    pub fn stage_for(&self, id: &AgentRole) -> Option<MissionStage> {
        self.resolve(id).map(|def| def.stage)
    }

    /// Researcher-class compatibility check (replaces `is_compatible_with`).
    ///
    /// Compatible when ids are equal, or when both resolve to
    /// researcher-class definitions. Unregistered ids are never compatible:
    /// callers surface an explicit mismatch error.
    pub fn is_compatible(&self, role: &AgentRole, prompt_role: &AgentRole) -> bool {
        if role == prompt_role {
            return true;
        }
        match (self.resolve(role), self.resolve(prompt_role)) {
            (Some(a), Some(b)) => a.researcher_class && b.researcher_class,
            _ => false,
        }
    }

    /// Read-only verification eligibility flag, if registered.
    pub fn read_only_flag(&self, id: &AgentRole) -> Option<bool> {
        self.resolve(id).map(|def| def.read_only_verification)
    }

    /// Per-role temperature in float form (replaces `role_default_temperature`).
    pub fn temperature_for(&self, id: &AgentRole) -> f32 {
        self.resolve(id)
            .map(|def| def.profile.model_policy.temperature_millicelsius as f32 / 1000.0)
            .unwrap_or(DEFAULT_ROLE_TEMPERATURE_MILLICELSIUS as f32 / 1000.0)
    }

    /// Per-role concurrency limit (explicit or default).
    pub fn concurrency_limit_for(&self, id: &AgentRole) -> usize {
        self.resolve(id)
            .and_then(|def| def.concurrency_limit)
            .unwrap_or(DEFAULT_PER_ROLE_CONCURRENCY)
    }

    /// Built-in role ids in canonical order.
    pub fn builtin_ids(&self) -> Vec<String> {
        self.builtin_order.clone()
    }

    /// All registered role ids (built-ins first, then extensions).
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

    // -- Global convenience accessors (production call paths) --

    fn flag_global(id: &AgentRole, f: impl FnOnce(&RoleDefinition) -> bool) -> Option<bool> {
        Self::global()
            .read()
            .ok()
            .and_then(|guard| guard.resolve(id).map(f))
    }

    /// Write-tool permission flag; unregistered ids fail closed (`false`).
    pub fn write_tools_permitted_for(id: &AgentRole) -> bool {
        Self::flag_global(id, |def| def.write_tools_permitted).unwrap_or(false)
    }

    /// Modification-requirement flag; unregistered ids report `false`
    /// (the write gate above already denies them any mutation).
    pub fn requires_modification_for(id: &AgentRole) -> bool {
        Self::flag_global(id, |def| def.requires_modification).unwrap_or(false)
    }

    pub fn resolve_global(id: &AgentRole) -> Option<RoleDefinition> {
        Self::global()
            .read()
            .ok()
            .and_then(|r| r.resolve(id).cloned())
    }

    pub fn profile_for_global(id: &AgentRole) -> Result<AgentProfile, RoleError> {
        Self::global()
            .read()
            .map_err(|_| RoleError::UnknownRole {
                id: id.as_str().to_string(),
            })
            .and_then(|r| r.profile_for(id))
    }

    pub fn register_global(def: RoleDefinition) -> Result<(), RoleError> {
        Self::global()
            .write()
            .map_err(|_| RoleError::InvalidDefinition {
                id: def.id.as_str().to_string(),
                reason: "role registry lock poisoned".to_string(),
            })
            .and_then(|mut r| r.register(def))
    }
}

/// Validate a role definition's internal consistency (Test G/H boundary).
fn validate_role_definition(def: &RoleDefinition) -> Result<(), RoleError> {
    let id = def.id.as_str().to_string();
    let invalid = |reason: String| RoleError::InvalidDefinition {
        id: id.clone(),
        reason,
    };
    if id.is_empty() {
        return Err(invalid("role id cannot be empty".to_string()));
    }
    if !def.id.is_well_formed() {
        return Err(invalid(format!("role id '{id}' must match [a-z0-9_]+")));
    }
    if def.description.trim().is_empty() {
        return Err(invalid("description cannot be empty".to_string()));
    }
    if def.prompt_contract.trim().is_empty() {
        return Err(invalid("prompt_contract cannot be empty".to_string()));
    }
    if def.prompt_version == 0 {
        return Err(invalid("prompt_version must be >= 1".to_string()));
    }
    if def.profile.max_steps == 0 {
        return Err(invalid("profile.max_steps must be >= 1".to_string()));
    }
    if def
        .profile
        .capability_policy
        .allowed_capabilities
        .is_empty()
    {
        return Err(invalid(
            "capability envelope must allow at least one capability".to_string(),
        ));
    }
    if !KNOWN_SANDBOX_POLICIES.contains(&def.profile.sandbox_policy.as_str()) {
        return Err(invalid(format!(
            "sandbox_policy '{}' is not one of {:?}",
            def.profile.sandbox_policy, KNOWN_SANDBOX_POLICIES
        )));
    }
    // Privilege-consistency guards: flags must not claim what the envelope
    // forbids, and modification requirements need write tools.
    if def.write_tools_permitted && !def.profile.capability_policy.allow_file_write {
        return Err(invalid(
            "write_tools_permitted requires an envelope granting file write".to_string(),
        ));
    }
    if def.requires_modification && !def.write_tools_permitted {
        return Err(invalid(
            "requires_modification requires write_tools_permitted".to_string(),
        ));
    }
    for cap in &def.default_capabilities {
        if !def
            .profile
            .capability_policy
            .allowed_capabilities
            .contains(&cap.id)
        {
            return Err(invalid(format!(
                "default capability '{}' is outside the role envelope",
                cap.id
            )));
        }
        if (cap.mode == CapabilityAccessMode::Write || cap.mode == CapabilityAccessMode::ReadWrite)
            && !def.profile.capability_policy.allow_file_write
        {
            return Err(invalid(format!(
                "default capability '{}' requires write access the envelope forbids",
                cap.id
            )));
        }
    }
    for alias in &def.aliases {
        let a = alias.trim().to_lowercase();
        if a.is_empty()
            || !a
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(invalid(format!("alias '{alias}' must match [a-z0-9_]+")));
        }
    }
    Ok(())
}

/// Built-in capability→role inference rules in precedence order.
///
/// Reproduces the pre-27.5 dispatcher/planner precedence exactly; consulted
/// generically by [`RoleRegistry::infer_role_for_capabilities`].
fn builtin_inference_rules() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fs.write", "implementer"),
        ("shell.exec", "implementer"),
        ("plan.propose", "planner"),
        ("evidence.record", "verifier"),
        ("cargo.test", "verifier"),
        ("cargo.check", "implementer"),
        ("logs.inspect", "diagnostician"),
        ("diff.analyze", "reviewer"),
        ("git.merge", "integrator"),
        ("spec.propose", "architect"),
        ("docs.search", "researcher"),
        ("web.search", "researcher"),
        ("repo.read", "researcher"),
        ("fs.read", "researcher"),
    ]
}

mod builtins;

/// The 18 built-in role definitions, transcribed from the pre-27.5
/// `AgentProfile::built_in` match (single authority since 27.5).
fn builtin_role_definitions() -> Vec<RoleDefinition> {
    builtins::builtin_role_definitions()
}
