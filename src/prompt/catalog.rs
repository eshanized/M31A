//! Immutable prompt catalog index, directory discovery, and built-in genesis contracts.

use crate::prompt::contract::PromptContract;
use crate::prompt::error::PromptError;
use crate::prompt::provenance::PromptSourceKind;
use crate::state_machine::agent::AgentRole;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

/// Maximum permissible byte size for an individual prompt contract file (512 KB).
pub const MAX_PROMPT_FILE_SIZE_BYTES: u64 = 524_288;

/// Metadata summary of an indexed prompt contract in the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptContractMetadata {
    pub id: String,
    pub version: u32,
    pub role: AgentRole,
    pub description: String,
    pub content_hash: String,
    pub required_parameters: Vec<String>,
    pub optional_parameters: Vec<String>,
    #[serde(default)]
    pub source_kind: PromptSourceKind,
    #[serde(default)]
    pub is_overrideable: bool,
}

/// Catalog entry tracking the contract, origin source kind, and optional file path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub contract: PromptContract,
    pub source_kind: PromptSourceKind,
    pub source_path: Option<String>,
}

/// Check if a contract ID is security-sensitive and protected from workspace overrides.
pub fn is_protected_contract_id(id: &str) -> bool {
    is_layer0_contract_id(id) || is_behavioral_contract_id(id)
}

/// Layer 0 contracts: kernel safety, policy, sandbox, and core runtime
/// invariants. Workspace/project files targeting these IDs are REJECTED
/// outright ([`PromptError::PromptSecurityViolation`]).
pub fn is_layer0_contract_id(id: &str) -> bool {
    id == "runtime.safety_invariants"
        || id == "core.safety"
        || id == "core.safety.v2"
        || id.starts_with("runtime.")
        || id.starts_with("core.")
}

/// Behavioral contracts: agent role profiles and stage contracts that shape
/// trusted agent behavior (e.g. `agent.implementer`, `execution.*`,
/// `verification.*`). These are NOT replaceable by workspace files either —
/// but unlike Layer 0, repository customization is preserved by redirecting
/// the file content into lower-trust project guidance (see
/// [`InMemoryPromptCatalog::project_guidance_for`]) instead of rejecting it.
/// The built-in contract always remains the authoritative L1/L2 source.
pub fn is_behavioral_contract_id(id: &str) -> bool {
    id.starts_with("agent.")
        || id.starts_with("agents.")
        || id.starts_with("execution.")
        || id.starts_with("verification.")
}

/// Abstract contract for querying immutable prompt templates.
pub trait PromptCatalog: Send + Sync {
    /// Retrieve a prompt contract by exact ID and version.
    fn get(&self, id: &str, version: u32) -> Result<&PromptContract, PromptError>;

    /// Check if the catalog contains a contract with exact ID and version.
    fn contains(&self, id: &str, version: u32) -> bool;

    /// Enumerate all registered contracts in deterministic order.
    fn list(&self) -> Vec<PromptContractMetadata>;

    /// Determine the source origin of a registered contract.
    fn source_kind(&self, id: &str, version: u32) -> Option<PromptSourceKind> {
        if self.contains(id, version) {
            Some(PromptSourceKind::Builtin)
        } else {
            None
        }
    }

    /// Lower-trust project guidance recorded for a behavioral contract ID.
    ///
    /// Repository files targeting behavioral IDs (`agent.*`, `execution.*`,
    /// `verification.*`) are stored here instead of replacing the built-in
    /// contract. Compilers must inject this content ONLY as untrusted
    /// L5/context guidance — never as the authoritative role contract.
    /// Default: no guidance.
    fn project_guidance_for(&self, _id: &str, _version: u32) -> Vec<CatalogEntry> {
        Vec::new()
    }

    /// Check if a contract is allowed to be overridden by workspace/project prompts.
    fn is_overrideable(&self, id: &str, _version: u32) -> bool {
        !is_protected_contract_id(id)
    }

    /// Resolve canonical V2 contract, following deprecation pointers and compatibility aliases.
    fn resolve_canonical(&self, id: &str, version: u32) -> Result<&PromptContract, PromptError> {
        self.get(id, version)
    }

    /// Audit full prompt catalog inventory against canonical V2 architecture.
    fn audit_inventory(&self) -> crate::prompt::v2::PromptInventoryAudit {
        crate::prompt::v2::PromptInventoryAudit::default()
    }
}

/// In-memory catalog indexing prompt contracts with three-tier storage
/// (Built-in + Overrides + GlobalUserCommands) and alias routing.
///
/// The GlobalUserCommands tier holds contracts synthesized from global
/// user-defined slash commands
/// (`<global_config_dir>/prompts/commands/*.toml`, source kind
/// [`PromptSourceKind::GlobalUserCommand`]). It is populated explicitly via
/// [`Self::register_user_command`] — never by workspace/project directory
/// scans — and the `command.*` namespace is reserved for it: workspace and
/// project files targeting that namespace are rejected fail-closed so a
/// repository can never hijack a user's global commands.
#[derive(Debug, Clone, Default)]
pub struct InMemoryPromptCatalog {
    builtins: BTreeMap<(String, u32), PromptContract>,
    overrides: BTreeMap<(String, u32), CatalogEntry>,
    aliases: BTreeMap<(String, u32), (String, u32)>,
    /// Lower-trust repository guidance for behavioral contract IDs. Never
    /// consulted by `get()`/`resolve_canonical()`; only by explicit
    /// guidance-injection compilation paths as untrusted context.
    guidance: BTreeMap<(String, u32), Vec<CatalogEntry>>,
    /// Global user command contracts (`command.*` namespace only).
    user_commands: BTreeMap<(String, u32), CatalogEntry>,
}

impl InMemoryPromptCatalog {
    /// Create a new empty prompt catalog.
    pub fn new() -> Self {
        Self {
            builtins: BTreeMap::new(),
            overrides: BTreeMap::new(),
            aliases: BTreeMap::new(),
            guidance: BTreeMap::new(),
            user_commands: BTreeMap::new(),
        }
    }

    /// Create a catalog populated with M31A's built-in genesis and execution prompt contracts.
    pub fn with_builtins() -> Self {
        let mut catalog = Self::new();
        catalog
            .register_builtins()
            .expect("built-in prompts are valid");
        catalog
    }

    /// Create a catalog populated with built-ins and optional workspace prompt overrides.
    pub fn with_builtins_and_workspace(workspace_root: &Path) -> Self {
        let mut catalog = Self::with_builtins();
        if let Err(e) = catalog.load_workspace_overrides(workspace_root) {
            tracing::warn!(
                "failed to load workspace prompt overrides from '{}': {}",
                workspace_root.display(),
                e
            );
        }
        catalog
    }

    /// Create a catalog populated with built-ins, optional workspace prompt overrides,
    /// and global user-defined commands for the specified deployment channel.
    pub fn with_builtins_workspace_and_global(
        workspace_root: &Path,
        channel: crate::deployment::DeploymentChannel,
    ) -> Self {
        let mut catalog = Self::with_builtins_and_workspace(workspace_root);
        if let Err(e) = catalog.reload_user_commands_for_channel(channel) {
            tracing::warn!(
                "failed to load global user commands for channel '{:?}': {}",
                channel,
                e
            );
        }
        catalog
    }

    /// Register an immutable prompt contract into the built-in catalog tier.
    ///
    /// # Immutability Guarantees
    /// - Registering the exact same contract (identical content hash) is idempotent.
    /// - Registering a different body under an existing `(id, version)` returns `PromptDuplicate`.
    pub fn register(&mut self, contract: PromptContract) -> Result<(), PromptError> {
        self.register_with_source(contract, PromptSourceKind::Builtin, None)
    }

    /// Register a prompt contract with an explicit source origin classification.
    pub fn register_with_source(
        &mut self,
        contract: PromptContract,
        source_kind: PromptSourceKind,
        source_path: Option<String>,
    ) -> Result<(), PromptError> {
        let key = (contract.id.clone(), contract.version);

        match source_kind {
            PromptSourceKind::Builtin => {
                if let Some(existing) = self.builtins.get(&key) {
                    if existing.content_hash == contract.content_hash {
                        return Ok(());
                    }
                    return Err(PromptError::PromptDuplicate {
                        id: contract.id,
                        version: contract.version,
                        reason: format!(
                            "conflicting contract content: existing hash '{}' != new hash '{}'",
                            existing.content_hash, contract.content_hash
                        ),
                    });
                }
                self.builtins.insert(key, contract.clone());
            }
            PromptSourceKind::WorkspaceOverride | PromptSourceKind::ProjectOverride => {
                // Invariant: Layer 0 runtime safety invariants and core contracts cannot be overridden
                if is_layer0_contract_id(&contract.id)
                    || contract.authority == crate::prompt::v2::AuthorityLevel::Kernel
                {
                    return Err(PromptError::PromptSecurityViolation {
                        prompt_id: contract.id,
                        reason: "cannot override protected Layer 0 runtime safety contract"
                            .to_string(),
                    });
                }
                // Behavioral contracts (agent role profiles, stage contracts)
                // cannot be REPLACED by repository files either — but the
                // customization is preserved as lower-trust project guidance
                // injected at L5, never as the authoritative L1/L2 contract.
                if is_behavioral_contract_id(&contract.id) {
                    self.guidance.entry(key).or_default().push(CatalogEntry {
                        contract: contract.clone(),
                        source_kind,
                        source_path,
                    });
                    return Ok(());
                }

                // Global user commands are RESERVED for the user command tier.
                // Workspace/project files targeting the `command.*` namespace
                // are rejected fail-closed so a repository can never hijack a
                // user's global commands by dropping a similarly named TOML.
                if contract.id.starts_with("command.") {
                    return Err(PromptError::PromptSecurityViolation {
                        prompt_id: contract.id,
                        reason:
                            "global user command namespace 'command.*' is reserved; workspace/project files cannot register commands in this namespace"
                                .to_string(),
                    });
                }

                if let Some(existing) = self.overrides.get(&key) {
                    // Conflicting definition within the exact same override scope
                    if existing.source_kind == source_kind
                        && existing.contract.content_hash != contract.content_hash
                    {
                        return Err(PromptError::PromptDuplicate {
                            id: contract.id,
                            version: contract.version,
                            reason: format!(
                                "conflicting override in same scope '{}': existing hash '{}' != new hash '{}'",
                                source_kind, existing.contract.content_hash, contract.content_hash
                            ),
                        });
                    }
                    // WorkspaceOverride (.m31a/prompts) takes strict precedence over ProjectOverride (prompts)
                    if existing.source_kind == PromptSourceKind::WorkspaceOverride
                        && source_kind == PromptSourceKind::ProjectOverride
                    {
                        return Ok(());
                    }
                }

                self.overrides.insert(
                    key,
                    CatalogEntry {
                        contract: contract.clone(),
                        source_kind,
                        source_path,
                    },
                );
            }
            PromptSourceKind::GlobalUserCommand => {
                // Only the `command.*` namespace is accepted for global user commands.
                let contract_id = contract.id.clone();
                if !contract_id.starts_with("command.") {
                    return Err(PromptError::PromptSecurityViolation {
                        prompt_id: contract_id,
                        reason: "global user commands must use the reserved 'command.*' namespace"
                            .to_string(),
                    });
                }
                // Layer 0 and behavioral IDs are protected; the user command
                // loader already validates this but defense-in-depth here too.
                if is_layer0_contract_id(&contract_id) || is_behavioral_contract_id(&contract_id) {
                    return Err(PromptError::PromptSecurityViolation {
                        prompt_id: contract_id,
                        reason: "global user command cannot target protected contract id"
                            .to_string(),
                    });
                }
                if let Some(existing) = self.user_commands.get(&key) {
                    if existing.contract.content_hash != contract.content_hash {
                        return Err(PromptError::PromptDuplicate {
                            id: contract_id.clone(),
                            version: contract.version,
                            reason: format!(
                                "duplicate global user command '{} v{}': conflicting content hash",
                                contract_id, contract.version
                            ),
                        });
                    }
                    // Idempotent: same contract re-registration is a no-op.
                } else {
                    self.user_commands.insert(
                        key,
                        CatalogEntry {
                            contract: contract.clone(),
                            source_kind,
                            source_path,
                        },
                    );
                }
            }
        }

        if let Some(ref comp) = contract.compatibility {
            for alias in &comp.legacy_aliases {
                self.aliases
                    .insert((alias.clone(), 1), (contract.id.clone(), contract.version));
                self.aliases.insert(
                    (alias.clone(), contract.version),
                    (contract.id.clone(), contract.version),
                );
            }
        }

        Ok(())
    }

    /// Number of lower-trust guidance entries recorded for behavioral IDs.
    pub fn guidance_count(&self) -> usize {
        self.guidance.values().map(|v| v.len()).sum()
    }

    /// Render recorded guidance for a behavioral contract as delimited
    /// untrusted context text (empty when none was supplied).
    pub fn guidance_text_for(&self, id: &str, version: u32) -> String {
        use crate::prompt::catalog::PromptCatalog;
        let entries = PromptCatalog::project_guidance_for(self, id, version);
        entries
            .iter()
            .map(|e| {
                format!(
                    "[project guidance from {:?}: {}]",
                    e.source_kind, e.contract.template_body
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Load validated workspace prompt overrides from standard project/workspace locations.
    ///
    /// # Precedence Order
    /// 1. `.m31a/prompts/*.toml` (WorkspaceOverride - highest precedence)
    /// 2. `prompts/*.toml` (ProjectOverride - second precedence)
    /// 3. Embedded Built-ins (Tier 1 core fallback)
    pub fn load_workspace_overrides(
        &mut self,
        workspace_root: &Path,
    ) -> Result<usize, PromptError> {
        let mut loaded = 0;

        // 1. Project overrides: <workspace_root>/prompts
        let project_dir = workspace_root.join("prompts");
        if project_dir.exists() && project_dir.is_dir() {
            loaded += self.load_from_dir_with_source(
                Path::new("prompts"),
                Some(workspace_root),
                PromptSourceKind::ProjectOverride,
            )?;
        }

        // 2. Workspace overrides: <workspace_root>/.m31a/prompts
        let workspace_dir = workspace_root.join(".m31a").join("prompts");
        if workspace_dir.exists() && workspace_dir.is_dir() {
            loaded += self.load_from_dir_with_source(
                Path::new(".m31a/prompts"),
                Some(workspace_root),
                PromptSourceKind::WorkspaceOverride,
            )?;
        }

        Ok(loaded)
    }

    /// Clear all loaded workspace and project overrides, restoring pure built-in operation.
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
        // Guidance is workspace-derived state too: clearing overrides resets
        // the catalog to built-ins only (P0-04 trust reset).
        self.guidance.clear();
    }

    /// Reload workspace overrides deterministically.
    pub fn reload_workspace_overrides(
        &mut self,
        workspace_root: &Path,
    ) -> Result<usize, PromptError> {
        self.clear_overrides();
        self.load_workspace_overrides(workspace_root)
    }

    /// Number of registered built-in contracts.
    pub fn builtins_count(&self) -> usize {
        self.builtins.len()
    }

    /// Number of active workspace/project overrides.
    pub fn overrides_count(&self) -> usize {
        self.overrides.len()
    }

    /// Number of registered global user commands.
    pub fn user_commands_count(&self) -> usize {
        self.user_commands.len()
    }

    /// Register a synthesized global user command contract directly.
    ///
    /// This is the canonical entry point for loading global user commands
    /// (as opposed to `load_from_dir`, which is for workspace/project
    /// files). The contract MUST use the `command.*` namespace and have
    /// authority `AuthorityLevel::DynamicMission`.
    pub fn register_user_command(
        &mut self,
        contract: PromptContract,
        source_path: Option<String>,
    ) -> Result<(), PromptError> {
        self.register_with_source(contract, PromptSourceKind::GlobalUserCommand, source_path)
    }

    /// List all registered global user commands in deterministic order.
    pub fn list_user_commands(&self) -> Vec<PromptContractMetadata> {
        let mut out = Vec::with_capacity(self.user_commands.len());
        for entry in self.user_commands.values() {
            let contract = &entry.contract;
            let mut req = Vec::new();
            let mut opt = Vec::new();
            for p in &contract.input_parameters {
                if p.is_required {
                    req.push(p.name.clone());
                } else {
                    opt.push(p.name.clone());
                }
            }
            out.push(PromptContractMetadata {
                id: contract.id.clone(),
                version: contract.version,
                role: contract.role.clone(),
                description: contract.description.clone(),
                content_hash: contract.content_hash.clone(),
                required_parameters: req,
                optional_parameters: opt,
                source_kind: entry.source_kind,
                is_overrideable: false, // user commands are never overridden
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id).then(a.version.cmp(&b.version)));
        out
    }

    /// Clear all registered global user commands.
    pub fn clear_user_commands(&mut self) {
        self.user_commands.clear();
    }

    /// Access the raw catalog entry for a registered user command if present.
    pub fn get_user_command_entry(&self, id: &str, version: u32) -> Option<&CatalogEntry> {
        let key = (id.to_string(), version);
        self.user_commands.get(&key)
    }

    /// Find a registered user command entry by command name or alias (with or without hyphens).
    pub fn find_user_command_entry(&self, name_or_alias: &str) -> Option<&CatalogEntry> {
        self.find_user_command_entry_with_version(name_or_alias, None)
    }

    /// Find a registered user command entry by command name or alias with optional version filter.
    /// If version is None, selects the latest (highest) version registered.
    pub fn find_user_command_entry_with_version(
        &self,
        name_or_alias: &str,
        version: Option<u32>,
    ) -> Option<&CatalogEntry> {
        let clean = name_or_alias.trim_start_matches('/').to_lowercase();
        let normalized = clean.replace('-', "_");

        // 1. Direct match on user_commands keys if version is specified
        if let Some(v) = version {
            if let Some(entry) = self.user_commands.get(&(format!("command.{}", clean), v)) {
                return Some(entry);
            }
            if let Some(entry) = self
                .user_commands
                .get(&(format!("command.{}", normalized), v))
            {
                return Some(entry);
            }
            if let Some(target) = self.aliases.get(&(clean.clone(), v)) {
                if let Some(entry) = self.user_commands.get(target) {
                    return Some(entry);
                }
            }
            if let Some(target) = self.aliases.get(&(format!("command.{}", clean), v)) {
                if let Some(entry) = self.user_commands.get(target) {
                    return Some(entry);
                }
            }
            if let Some(target) = self.aliases.get(&(normalized.clone(), v)) {
                if let Some(entry) = self.user_commands.get(target) {
                    return Some(entry);
                }
            }
        }

        // 2. Scan user_commands entries: collect candidates and pick highest version
        let mut candidates: Vec<&CatalogEntry> = Vec::new();
        for entry in self.user_commands.values() {
            if let Some(v) = version {
                if entry.contract.version != v {
                    continue;
                }
            }
            let id = &entry.contract.id;
            if id == &format!("command.{}", clean)
                || id == &format!("command.{}", normalized)
                || id == &clean
                || id == &normalized
            {
                candidates.push(entry);
                continue;
            }
            if let Some(ref comp) = entry.contract.compatibility {
                if comp.legacy_aliases.iter().any(|a| {
                    a == &clean
                        || a == &normalized
                        || a == &format!("command.{}", clean)
                        || a == &format!("command.{}", normalized)
                }) {
                    candidates.push(entry);
                }
            }
        }

        candidates.into_iter().max_by_key(|e| e.contract.version)
    }

    /// Apply an immutable slice of synthesized user command definitions into the catalog.
    ///
    /// Clears existing user commands and registers contracts from the in-memory definitions.
    /// Invariant: `catalog.get_user_command_entry(...).contract == def.contract`.
    pub fn apply_user_command_definitions(
        &mut self,
        definitions: &[std::sync::Arc<crate::interaction::user_commands::UserCommandDefinition>],
    ) -> Result<usize, PromptError> {
        self.clear_user_commands();
        for def in definitions {
            self.register_user_command(def.contract.clone(), def.command.source_path.clone())?;
        }
        Ok(definitions.len())
    }

    /// Reload global user commands from an explicit filesystem directory.
    ///
    /// Atomic reload: all definitions are parsed, synthesized, and validated
    /// BEFORE mutating the catalog. If any contract is invalid, previous
    /// valid commands are retained.
    pub fn reload_user_commands_from_dir(&mut self, dir: &Path) -> Result<usize, PromptError> {
        let loaded = crate::interaction::user_commands::load_global_user_commands_from_dir(dir);
        let mut defs = Vec::new();
        for cmd in loaded.loaded {
            let cmd_arc = std::sync::Arc::new(cmd);
            let def =
                crate::interaction::user_commands::UserCommandDefinition::new(cmd_arc.clone())
                    .map_err(|e| PromptError::PromptInvalid {
                        id: cmd_arc.id.clone(),
                        version: cmd_arc.version,
                        reason: e.to_string(),
                    })?;
            defs.push(std::sync::Arc::new(def));
        }

        let count = self.apply_user_command_definitions(&defs)?;
        for rej in &loaded.rejected {
            tracing::warn!(
                "rejected global user command '{}': {}",
                rej.file,
                rej.reason
            );
        }
        Ok(count)
    }

    /// Reload global user commands from the standard global directory
    /// for a specific deployment channel.
    pub fn reload_user_commands_for_channel(
        &mut self,
        channel: crate::deployment::DeploymentChannel,
    ) -> Result<usize, PromptError> {
        match crate::interaction::user_commands::global_user_commands_dir_for_channel(channel) {
            Some(dir) => self.reload_user_commands_from_dir(&dir),
            None => {
                self.clear_user_commands();
                Ok(0)
            }
        }
    }

    /// Reload global user commands for the running artifact's channel.
    pub fn reload_user_commands(&mut self) -> Result<usize, PromptError> {
        self.reload_user_commands_for_channel(crate::deployment::DeploymentChannel::current())
    }

    /// Safely scan and load prompt contract files (`*.toml`) from a filesystem directory.
    pub fn load_from_dir(
        &mut self,
        dir_path: &Path,
        workspace_root: Option<&Path>,
    ) -> Result<usize, PromptError> {
        self.load_from_dir_with_source(
            dir_path,
            workspace_root,
            PromptSourceKind::WorkspaceOverride,
        )
    }

    /// Safely scan and load prompt contract files (`*.toml`) from a filesystem directory with designated source kind.
    ///
    /// # Security Constraints
    /// - Rejects directory paths containing parent traversal `..`.
    /// - Refuses to load from `.git` or unauthorized `.m31a` directories (only `.m31a/prompts` allowed).
    /// - Rejects symlinks escaping `workspace_root` or pointing to `.git`.
    /// - Bounded file size per prompt file (512 KB).
    /// - Validates TOML schema, MiniJinja syntax, and parameters before registration.
    pub fn load_from_dir_with_source(
        &mut self,
        dir_path: &Path,
        workspace_root: Option<&Path>,
        source_kind: PromptSourceKind,
    ) -> Result<usize, PromptError> {
        // 1. Path safety checks
        let mut has_m31a = false;
        let mut has_prompts_after_m31a = false;

        for component in dir_path.components() {
            match component {
                Component::ParentDir => {
                    return Err(PromptError::PathViolation {
                        path: dir_path.display().to_string(),
                        reason: "prompt directory path cannot contain parent traversal '..'"
                            .to_string(),
                    });
                }
                Component::Normal(c) => {
                    let s = c.to_string_lossy();
                    if s == ".git" {
                        return Err(PromptError::PathViolation {
                            path: dir_path.display().to_string(),
                            reason: "refusing to load prompts from protected directory '.git'"
                                .to_string(),
                        });
                    }
                    if s == ".m31a" {
                        has_m31a = true;
                    } else if has_m31a && (s == "prompts" || has_prompts_after_m31a) {
                        has_prompts_after_m31a = true;
                    } else if has_m31a && !has_prompts_after_m31a {
                        return Err(PromptError::PathViolation {
                            path: dir_path.display().to_string(),
                            reason: format!(
                                "refusing to load prompts from protected directory '.m31a/{}'",
                                s
                            ),
                        });
                    }
                }
                _ => {}
            }
        }

        if has_m31a && !has_prompts_after_m31a {
            return Err(PromptError::PathViolation {
                path: dir_path.display().to_string(),
                reason: "refusing to load prompts from protected root directory '.m31a'"
                    .to_string(),
            });
        }

        let canonical_dir = if let Some(root) = workspace_root {
            let combined = if dir_path.is_absolute() {
                dir_path.to_path_buf()
            } else {
                root.join(dir_path)
            };
            if !combined.starts_with(root) {
                return Err(PromptError::PathViolation {
                    path: dir_path.display().to_string(),
                    reason: format!("path escapes workspace root '{}'", root.display()),
                });
            }
            combined
        } else {
            dir_path.to_path_buf()
        };

        if !canonical_dir.exists() || !canonical_dir.is_dir() {
            return Ok(0);
        }

        let mut loaded_count = 0;
        let mut entries = Vec::new();
        self.collect_toml_files(&canonical_dir, workspace_root, &mut entries, 0)?;

        // Sort entries deterministically by path
        entries.sort();

        for file_path in entries {
            let metadata =
                std::fs::metadata(&file_path).map_err(|e| PromptError::PromptInvalid {
                    id: file_path.display().to_string(),
                    version: 0,
                    reason: format!("failed to read metadata: {}", e),
                })?;

            if metadata.len() > MAX_PROMPT_FILE_SIZE_BYTES {
                return Err(PromptError::PromptInvalid {
                    id: file_path.display().to_string(),
                    version: 0,
                    reason: format!(
                        "file size {} exceeds maximum permitted bound of {} bytes",
                        metadata.len(),
                        MAX_PROMPT_FILE_SIZE_BYTES
                    ),
                });
            }

            let content_bytes =
                std::fs::read(&file_path).map_err(|e| PromptError::PromptInvalid {
                    id: file_path.display().to_string(),
                    version: 0,
                    reason: format!("failed to read prompt file: {}", e),
                })?;

            let content_str =
                std::str::from_utf8(&content_bytes).map_err(|e| PromptError::PromptInvalid {
                    id: file_path.display().to_string(),
                    version: 0,
                    reason: format!("file is not valid UTF-8: {}", e),
                })?;

            let contract = PromptContract::from_toml_str(content_str)?;
            self.register_with_source(
                contract,
                source_kind,
                Some(file_path.display().to_string()),
            )?;
            loaded_count += 1;
        }

        Ok(loaded_count)
    }

    fn collect_toml_files(
        &self,
        dir: &Path,
        workspace_root: Option<&Path>,
        results: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<(), PromptError> {
        if depth > 10 {
            return Ok(());
        }

        let read_dir = std::fs::read_dir(dir).map_err(|e| PromptError::PromptInvalid {
            id: dir.display().to_string(),
            version: 0,
            reason: format!("failed to open directory: {}", e),
        })?;

        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            // Skip hidden entries unless it is part of .m31a/prompts root
            if file_name.starts_with('.') && depth > 0 {
                continue;
            }

            // Symlink containment validation
            let is_symlink = entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);

            if is_symlink {
                let resolved =
                    std::fs::canonicalize(&path).map_err(|e| PromptError::PathViolation {
                        path: path.display().to_string(),
                        reason: format!("failed to resolve symlink target: {}", e),
                    })?;

                if let Some(root) = workspace_root {
                    let canonical_root =
                        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
                    if !resolved.starts_with(&canonical_root) {
                        return Err(PromptError::PathViolation {
                            path: path.display().to_string(),
                            reason: format!(
                                "symlink target '{}' escapes workspace root '{}'",
                                resolved.display(),
                                root.display()
                            ),
                        });
                    }
                }

                if resolved.components().any(|c| c.as_os_str() == ".git") {
                    return Err(PromptError::PathViolation {
                        path: path.display().to_string(),
                        reason: "symlink target points into protected '.git' directory".to_string(),
                    });
                }
            }

            if path.is_dir() {
                self.collect_toml_files(&path, workspace_root, results, depth + 1)?;
            } else if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
                results.push(path);
            }
        }

        Ok(())
    }

    fn register_builtins(&mut self) -> Result<(), PromptError> {
        let contracts = crate::prompt::builtins::load_all_builtin_contracts()?;
        for contract in contracts {
            self.register(contract)?;
        }
        Ok(())
    }
}

impl PromptCatalog for InMemoryPromptCatalog {
    fn get(&self, id: &str, version: u32) -> Result<&PromptContract, PromptError> {
        let key = (id.to_string(), version);
        if let Some(entry) = self.overrides.get(&key) {
            return Ok(&entry.contract);
        }
        if let Some(contract) = self.builtins.get(&key) {
            return Ok(contract);
        }
        if let Some(entry) = self.user_commands.get(&key) {
            return Ok(&entry.contract);
        }
        if let Some(target) = self.aliases.get(&key) {
            if let Some(entry) = self.overrides.get(target) {
                return Ok(&entry.contract);
            }
            if let Some(contract) = self.builtins.get(target) {
                return Ok(contract);
            }
            if let Some(entry) = self.user_commands.get(target) {
                return Ok(&entry.contract);
            }
        }
        Err(PromptError::PromptNotFound {
            id: id.to_string(),
            version,
        })
    }

    fn contains(&self, id: &str, version: u32) -> bool {
        let key = (id.to_string(), version);
        self.overrides.contains_key(&key)
            || self.builtins.contains_key(&key)
            || self.user_commands.contains_key(&key)
            || self.aliases.contains_key(&key)
    }

    fn resolve_canonical(&self, id: &str, version: u32) -> Result<&PromptContract, PromptError> {
        let key = (id.to_string(), version);
        if let Some(entry) = self.overrides.get(&key) {
            return Ok(&entry.contract);
        }
        if let Some(entry) = self.user_commands.get(&key) {
            return Ok(&entry.contract);
        }
        // If caller requested version 1 or legacy reference, prefer canonical version 2 if available
        if version == 1 {
            if let Ok(v2) = self.get(id, 2) {
                return Ok(v2);
            }
        }
        let contract = self.get(id, version)?;
        if contract.is_deprecated() {
            if let Some(repl) = contract
                .compatibility
                .as_ref()
                .and_then(|c| c.canonical_replacement.as_ref())
            {
                if let Ok(c) = self.get(repl, 2) {
                    return Ok(c);
                }
                if let Ok(c) = self.get(repl, 1) {
                    return Ok(c);
                }
            }
        }
        Ok(contract)
    }

    fn audit_inventory(&self) -> crate::prompt::v2::PromptInventoryAudit {
        let mut canonical_contracts = Vec::new();
        let mut deprecated_aliases = Vec::new();
        let mut dead_or_unused = Vec::new();
        let mut test_only_contracts = Vec::new();
        let mut production_reachable = Vec::new();

        let all = self.list();
        for meta in &all {
            if meta.id.ends_with(".v2") || meta.version >= 2 {
                canonical_contracts.push(format!("{}.v{}", meta.id, meta.version));
            }

            if matches!(
                meta.id.as_str(),
                "implement" | "review" | "verify" | "diagnose"
            ) {
                test_only_contracts.push(meta.id.clone());
                deprecated_aliases.push((meta.id.clone(), "execution.*.v2".to_string()));
            } else if meta.id.starts_with("execution.") && meta.version == 1 {
                deprecated_aliases.push((meta.id.clone(), format!("{}.v2", meta.id)));
            }

            if meta.id.starts_with("agent.")
                || meta.id.starts_with("core.")
                || meta.id == "runtime.safety_invariants"
                || meta.id == "planning.decompose"
                || meta.id.starts_with("genesis.")
                || meta.id == "skill.in_task_guidance"
                || meta.id == "execution.reviewer"
                || meta.id == "execution.diagnostician"
            {
                production_reachable.push(format!("{}.v{}", meta.id, meta.version));
            } else {
                dead_or_unused.push(format!("{}.v{}", meta.id, meta.version));
            }
        }

        crate::prompt::v2::PromptInventoryAudit {
            canonical_contracts,
            deprecated_aliases,
            dead_or_unused,
            test_only_contracts,
            production_reachable,
        }
    }

    fn source_kind(&self, id: &str, version: u32) -> Option<PromptSourceKind> {
        let key = (id.to_string(), version);
        if let Some(entry) = self.overrides.get(&key) {
            Some(entry.source_kind)
        } else if self.builtins.contains_key(&key) {
            Some(PromptSourceKind::Builtin)
        } else {
            self.user_commands.get(&key).map(|entry| entry.source_kind)
        }
    }

    /// Lower-trust repository guidance for a behavioral contract (P0-04).
    /// Never consulted by `get()`/`resolve_canonical()` — the built-in
    /// contract always wins. Only explicit guidance-injection compilation
    /// may consume this, as untrusted L5 context.
    fn project_guidance_for(&self, id: &str, version: u32) -> Vec<CatalogEntry> {
        self.guidance
            .get(&(id.to_string(), version))
            .cloned()
            .unwrap_or_default()
    }

    fn is_overrideable(&self, id: &str, _version: u32) -> bool {
        !is_protected_contract_id(id)
    }

    fn list(&self) -> Vec<PromptContractMetadata> {
        let mut keys: BTreeSet<(String, u32)> = BTreeSet::new();
        for k in self.builtins.keys() {
            keys.insert(k.clone());
        }
        for k in self.overrides.keys() {
            keys.insert(k.clone());
        }
        for k in self.user_commands.keys() {
            keys.insert(k.clone());
        }

        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            let (contract, source_kind) = if let Some(entry) = self.overrides.get(&key) {
                (&entry.contract, entry.source_kind)
            } else if let Some(entry) = self.user_commands.get(&key) {
                (&entry.contract, entry.source_kind)
            } else if let Some(c) = self.builtins.get(&key) {
                (c, PromptSourceKind::Builtin)
            } else {
                continue;
            };

            let mut req = Vec::new();
            let mut opt = Vec::new();
            for p in &contract.input_parameters {
                if p.is_required {
                    req.push(p.name.clone());
                } else {
                    opt.push(p.name.clone());
                }
            }

            let is_overrideable = !is_protected_contract_id(&contract.id);

            results.push(PromptContractMetadata {
                id: contract.id.clone(),
                version: contract.version,
                role: contract.role.clone(),
                description: contract.description.clone(),
                content_hash: contract.content_hash.clone(),
                required_parameters: req,
                optional_parameters: opt,
                source_kind,
                is_overrideable,
            });
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::parameter::PromptParameter;
    use tempfile::tempdir;

    fn sample_contract(id: &str, version: u32, role: AgentRole, body: &str) -> PromptContract {
        PromptContract::new(
            id,
            version,
            role,
            "Sample contract",
            vec![PromptParameter {
                name: "param1".to_string(),
                description: "desc".to_string(),
                is_required: false,
                default_value: Some("default".to_string()),
            }],
            body,
            Some("markdown".to_string()),
        )
        .unwrap()
    }

    #[test]
    fn test_catalog_builtins_and_source_kind() {
        let catalog = InMemoryPromptCatalog::with_builtins();
        assert!(catalog.builtins_count() >= 42);
        assert_eq!(catalog.overrides_count(), 0);

        let contract = catalog.get("agent.implementer", 1).unwrap();
        assert_eq!(contract.id, "agent.implementer");
        assert_eq!(
            catalog.source_kind("agent.implementer", 1),
            Some(PromptSourceKind::Builtin)
        );
        // P0-04: behavioral role contracts are NOT replaceable by repository
        // files (customization survives only as lower-trust guidance).
        assert!(!catalog.is_overrideable("agent.implementer", 1));
        assert!(catalog.is_overrideable("project.custom_notes", 1));

        // Protected contract check
        assert!(!catalog.is_overrideable("runtime.safety_invariants", 1));
        assert_eq!(
            catalog.source_kind("runtime.safety_invariants", 1),
            Some(PromptSourceKind::Builtin)
        );
    }

    #[test]
    fn test_override_precedence_workspace_over_project_over_builtin() {
        let mut catalog = InMemoryPromptCatalog::new();
        let builtin = sample_contract("custom.coder", 1, AgentRole::implementer(), "Builtin Body");
        catalog.register(builtin).unwrap();

        assert_eq!(
            catalog.source_kind("custom.coder", 1),
            Some(PromptSourceKind::Builtin)
        );
        assert_eq!(
            catalog.get("custom.coder", 1).unwrap().template_body,
            "Builtin Body"
        );

        // Project override
        let project_override = sample_contract(
            "custom.coder",
            1,
            AgentRole::implementer(),
            "Project Override Body",
        );
        catalog
            .register_with_source(
                project_override,
                PromptSourceKind::ProjectOverride,
                Some("prompts/coder.toml".to_string()),
            )
            .unwrap();

        assert_eq!(
            catalog.source_kind("custom.coder", 1),
            Some(PromptSourceKind::ProjectOverride)
        );
        assert_eq!(
            catalog.get("custom.coder", 1).unwrap().template_body,
            "Project Override Body"
        );

        // Workspace override
        let workspace_override = sample_contract(
            "custom.coder",
            1,
            AgentRole::implementer(),
            "Workspace Override Body",
        );
        catalog
            .register_with_source(
                workspace_override,
                PromptSourceKind::WorkspaceOverride,
                Some(".m31a/prompts/coder.toml".to_string()),
            )
            .unwrap();

        assert_eq!(
            catalog.source_kind("custom.coder", 1),
            Some(PromptSourceKind::WorkspaceOverride)
        );
        assert_eq!(
            catalog.get("custom.coder", 1).unwrap().template_body,
            "Workspace Override Body"
        );

        // Project override after workspace override does NOT overwrite workspace override
        let project_override2 = sample_contract(
            "custom.coder",
            1,
            AgentRole::implementer(),
            "Project Override 2",
        );
        catalog
            .register_with_source(
                project_override2,
                PromptSourceKind::ProjectOverride,
                Some("prompts/coder2.toml".to_string()),
            )
            .unwrap();
        assert_eq!(
            catalog.get("custom.coder", 1).unwrap().template_body,
            "Workspace Override Body"
        );

        // Clearing overrides restores built-in
        catalog.clear_overrides();
        assert_eq!(
            catalog.source_kind("custom.coder", 1),
            Some(PromptSourceKind::Builtin)
        );
        assert_eq!(
            catalog.get("custom.coder", 1).unwrap().template_body,
            "Builtin Body"
        );
    }

    #[test]
    fn test_protected_contract_override_strictly_rejected() {
        let mut catalog = InMemoryPromptCatalog::with_builtins();

        let malicious_override = sample_contract(
            "runtime.safety_invariants",
            1,
            AgentRole::planner(),
            "Malicious Invariant Replacement",
        );

        let result = catalog.register_with_source(
            malicious_override,
            PromptSourceKind::WorkspaceOverride,
            Some(".m31a/prompts/safety.toml".to_string()),
        );

        assert!(matches!(
            result,
            Err(PromptError::PromptSecurityViolation { .. })
        ));

        // Verify built-in remains intact
        let active = catalog.get("runtime.safety_invariants", 1).unwrap();
        assert!(
            active
                .template_body
                .contains("M31A RUNTIME SAFETY INVARIANTS")
        );
    }

    #[test]
    fn test_behavioral_override_becomes_guidance_not_replacement() {
        use crate::prompt::catalog::PromptCatalog;
        let mut catalog = InMemoryPromptCatalog::new();
        let builtin = sample_contract(
            "agent.implementer",
            1,
            AgentRole::implementer(),
            "Builtin Role Body",
        );
        catalog.register(builtin).unwrap();

        let repo_file = sample_contract(
            "agent.implementer",
            1,
            AgentRole::implementer(),
            "Repository Customization Body",
        );
        // Accepted (customization preserved) but NOT as a replacement.
        catalog
            .register_with_source(
                repo_file,
                PromptSourceKind::WorkspaceOverride,
                Some(".m31a/prompts/implementer.toml".to_string()),
            )
            .unwrap();

        // Authoritative contract is still the built-in.
        assert_eq!(
            catalog.get("agent.implementer", 1).unwrap().template_body,
            "Builtin Role Body"
        );
        assert_eq!(
            catalog.source_kind("agent.implementer", 1),
            Some(PromptSourceKind::Builtin)
        );
        // Customization is recorded as lower-trust guidance.
        let guidance = catalog.project_guidance_for("agent.implementer", 1);
        assert_eq!(guidance.len(), 1);
        assert_eq!(
            guidance[0].contract.template_body,
            "Repository Customization Body"
        );
        assert_eq!(catalog.guidance_count(), 1);
        // Trust reset clears guidance along with overrides.
        catalog.clear_overrides();
        assert_eq!(catalog.guidance_count(), 0);
        assert_eq!(
            catalog.get("agent.implementer", 1).unwrap().template_body,
            "Builtin Role Body"
        );
    }

    #[test]
    fn test_duplicate_in_same_scope_rejected() {
        let mut catalog = InMemoryPromptCatalog::new();
        let c1 = sample_contract("custom.notes", 1, AgentRole::implementer(), "Body 1");
        let c2 = sample_contract("custom.notes", 1, AgentRole::implementer(), "Body 2");

        catalog
            .register_with_source(c1, PromptSourceKind::WorkspaceOverride, None)
            .unwrap();
        let err = catalog.register_with_source(c2, PromptSourceKind::WorkspaceOverride, None);

        assert!(matches!(err, Err(PromptError::PromptDuplicate { .. })));
    }

    #[test]
    fn test_path_safety_checks() {
        let mut catalog = InMemoryPromptCatalog::new();
        let temp = tempdir().unwrap();
        let root = temp.path();

        // 1. Parent traversal rejected
        let res = catalog.load_from_dir(Path::new("../escaped"), Some(root));
        assert!(matches!(res, Err(PromptError::PathViolation { .. })));

        // 2. .git directory rejected
        let res = catalog.load_from_dir(Path::new(".git"), Some(root));
        assert!(matches!(res, Err(PromptError::PathViolation { .. })));

        // 3. Unauthorized .m31a subfolder rejected (e.g. .m31a/state)
        let res = catalog.load_from_dir(Path::new(".m31a/state"), Some(root));
        assert!(matches!(res, Err(PromptError::PathViolation { .. })));

        // 4. Authorized .m31a/prompts allowed (missing dir returns Ok(0))
        let res = catalog.load_from_dir(Path::new(".m31a/prompts"), Some(root));
        assert_eq!(res.unwrap(), 0);
    }
}
