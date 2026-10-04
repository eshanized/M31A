//! Global User-Defined Slash Commands.
//!
//! Lets an individual M31A user define their own slash commands as TOML
//! prompt definitions in their **global user configuration directory**,
//! without modifying M31A source code:
//!
//! ```text
//! <global_config_dir>/prompts/commands/atomic-commit.v1.toml  →  /atomic-commit
//! ```
//!
//! # Architecture
//!
//! ```text
//! Built-in slash commands + Global user slash commands
//!         ↓
//! SlashCommandRegistry (deterministic lookup/parsing; built-ins are reserved)
//!         ↓
//! PromptCommand (typed command contract — this module)
//!         ↓
//! PromptReference → canonical PromptCatalog → canonical PromptCompiler
//!         ↓
//! typed model invocation (ModelInvocationKind::UserCommand)
//!         ↓
//! model tool proposals → PolicyGate → ApprovalCoordinator → ToolPipeline
//!         ↓
//! Verification → CommandOutput
//! ```
//!
//! # Security model
//!
//! A user-defined command is **declarative orchestration, not executable
//! arbitrary code**. The TOML definition can never directly execute shell
//! commands, load arbitrary code, grant capabilities, bypass approval,
//! disable sandboxing, or replace trusted runtime contracts:
//!
//! - `capabilities.required` is a *request declaration only*. Grants come
//!   exclusively from the canonical [`crate::capability::registry::CapabilityRegistry`]
//!   via the role envelope, enforced by the policy gate and tool pipeline.
//! - The synthesized [`crate::prompt::contract::PromptContract`] is forced to
//!   [`crate::prompt::v2::AuthorityLevel::DynamicMission`] (lowest trust).
//!   Layer-0 (`runtime.*`, `core.*`) and behavioral (`agent.*`,
//!   `execution.*`, `verification.*`) contract ids are rejected, and the
//!   `command.*` namespace is reserved for global user commands so project
//!   prompt files can never hijack them.
//! - Loading is fail-closed per file: oversized files, invalid UTF-8, TOML
//!   errors, schema violations, path traversal, symlink escape, and `.git`
//!   traversal are rejected with diagnostics while valid commands still load.

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::deployment::DeploymentChannel;
use crate::error::M31AError;
use crate::interaction::commands::CommandSideEffect;
use crate::prompt::catalog::{is_behavioral_contract_id, is_layer0_contract_id};
use crate::prompt::contract::PromptContract;
use crate::prompt::parameter::PromptParameter;
use crate::prompt::reference::PromptReference;
use crate::prompt::{AuthorityLevel, PromptKind};
use crate::state_machine::agent::AgentRole;

// ── Constants ────────────────────────────────────────────────────────────────

/// Subdirectory (relative to the global config dir) holding user commands.
pub const GLOBAL_USER_COMMANDS_SUBDIR: &str = "prompts/commands";

/// Maximum byte size of a single user command TOML file (mirrors the prompt
/// catalog bound so global commands can never exceed prompt limits).
pub const MAX_USER_COMMAND_FILE_SIZE_BYTES: u64 =
    crate::prompt::catalog::MAX_PROMPT_FILE_SIZE_BYTES;

/// Reserved contract namespace for global user commands.
///
/// Project/workspace prompt files may NOT register ids in this namespace
/// (rejected fail-closed), so a repository can never hijack a user's global
/// command by dropping a similarly named TOML file.
pub const USER_COMMAND_CONTRACT_PREFIX: &str = "command.";

/// Maximum recursion depth when discovering command files.
pub const MAX_COMMAND_DISCOVERY_DEPTH: usize = 8;

/// Upper bound for `execution.max_steps` (bounded retries only).
pub const MAX_USER_COMMAND_STEPS: u32 = 100;

/// Default step budget when the TOML omits `execution.max_steps`.
pub const DEFAULT_USER_COMMAND_STEPS: u32 = 20;

/// Conventional Commit types accepted by the `conventional_commit_message`
/// verification check.
pub const CONVENTIONAL_COMMIT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

/// Verification checks a user command may declare. Closed allowlist:
/// unknown checks are rejected fail-closed.
pub const KNOWN_VERIFICATION_CHECKS: &[&str] = &[
    "working_tree_status",
    "commit_contains_single_file",
    "conventional_commit_message",
];

// ── Errors ───────────────────────────────────────────────────────────────────

/// Fail-closed error for global user command definition/loading/validation.
#[derive(Debug, Error)]
pub enum UserCommandError {
    #[error("invalid user command '{command}': {reason}")]
    Invalid { command: String, reason: String },
    #[error("user command file '{file}': {reason}")]
    FileRejected { file: String, reason: String },
    #[error(
        "duplicate user command {what}: '{name}' (first defined in '{first}', also in '{second}')"
    )]
    Duplicate {
        what: String,
        name: String,
        first: String,
        second: String,
    },
    #[error("user command name/alias '{name}' collides with built-in command '/{name}'")]
    BuiltinCollision { name: String },
    #[error("unknown user command '/{name}'")]
    UnknownCommand { name: String },
    #[error("user command '/{name}': argument error: {reason}")]
    ArgumentError { name: String, reason: String },
}

impl UserCommandError {
    pub fn invalid(command: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Invalid {
            command: command.into(),
            reason: reason.into(),
        }
    }
}

impl From<UserCommandError> for M31AError {
    fn from(e: UserCommandError) -> Self {
        match &e {
            UserCommandError::UnknownCommand { .. } => M31AError::not_found(e.to_string()),
            _ => M31AError::validation(e.to_string()),
        }
    }
}

// ── Typed contract ───────────────────────────────────────────────────────────

/// Declared input value type for a user command argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserCommandInputType {
    Boolean,
    String,
    Integer,
}

impl UserCommandInputType {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "boolean" | "bool" | "flag" => Ok(Self::Boolean),
            "string" | "str" | "text" => Ok(Self::String),
            "integer" | "int" | "number" => Ok(Self::Integer),
            other => Err(format!(
                "unsupported input type '{other}': expected 'boolean', 'string', or 'integer'"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::String => "string",
            Self::Integer => "integer",
        }
    }

    /// Validate a bound string value against this type.
    pub fn validate_value(&self, name: &str, value: &str) -> Result<(), String> {
        match self {
            Self::Boolean => match value.trim().to_lowercase().as_str() {
                "true" | "false" => Ok(()),
                _ => Err(format!(
                    "flag '--{name}' expects a boolean ('true'/'false'), got '{value}'"
                )),
            },
            Self::String => {
                if value.is_empty() {
                    Err(format!("argument '--{name}' requires a non-empty value"))
                } else {
                    Ok(())
                }
            }
            Self::Integer => value
                .trim()
                .parse::<i64>()
                .map(|_| ())
                .map_err(|_| format!("argument '--{name}' expects an integer, got '{value}'")),
        }
    }
}

/// A single declared command argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCommandInput {
    pub name: String,
    pub description: String,
    pub input_type: UserCommandInputType,
    pub required: bool,
    /// String-encoded default (validated against `input_type`).
    pub default: Option<String>,
}

/// Typed representation of a global user-defined slash command.
///
/// This is the explicit command contract: declarative orchestration metadata
/// plus a [`PromptReference`] into the canonical prompt catalog. It is never
/// executable code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptCommand {
    /// Canonical prompt contract id (always `command.*`).
    pub id: String,
    pub version: u32,
    /// Slash command name without leading `/` (e.g. `atomic-commit`).
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub usage: String,
    pub role: AgentRole,
    pub side_effect: CommandSideEffect,
    /// Whether a command-level approval is required before execution.
    pub requires_approval: bool,
    /// Bounded model/tool iteration budget.
    pub max_steps: u32,
    /// Declared capability *requests* (advisory only — never grants).
    pub requested_capabilities: Vec<String>,
    pub inputs_required: Vec<UserCommandInput>,
    pub inputs_optional: Vec<UserCommandInput>,
    pub verification_required: bool,
    pub verification_checks: Vec<String>,
    /// MiniJinja template body (validated at load; rendered canonically).
    pub template_body: String,
    /// Source file this command was loaded from (provenance).
    pub source_path: Option<String>,
}

impl PromptCommand {
    /// Canonical prompt reference for this command (`command.<name> @ vn`).
    pub fn prompt_reference(&self) -> PromptReference {
        PromptReference::new(self.id.clone(), self.version)
    }

    /// Synthesize the canonical [`PromptContract`] for this command.
    ///
    /// The contract authority is FORCED to
    /// [`AuthorityLevel::DynamicMission`] (lowest trust): a TOML definition
    /// can never escalate itself to kernel/system/role authority no matter
    /// what it declares.
    pub fn to_prompt_contract(&self) -> Result<PromptContract, UserCommandError> {
        let mut params =
            Vec::with_capacity(self.inputs_required.len() + self.inputs_optional.len());
        for input in self
            .inputs_required
            .iter()
            .chain(self.inputs_optional.iter())
        {
            params.push(PromptParameter {
                name: input.name.clone(),
                description: input.description.clone(),
                is_required: input.required,
                default_value: input.default.clone(),
            });
        }
        let mut contract = PromptContract::new_v2(
            self.id.clone(),
            self.version,
            self.role.clone(),
            PromptKind::Execution,
            Some(crate::prompt::context::MissionStage::Execute),
            self.description.clone(),
            AuthorityLevel::DynamicMission,
            params,
            self.template_body.clone(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(|e| {
            UserCommandError::invalid(
                self.name.clone(),
                format!("prompt contract synthesis failed: {e}"),
            )
        })?;
        // Defense in depth: the constructor derives authority from the id
        // prefix; a `command.*` id yields TaskContract — always downgrade to
        // the lowest trust tier regardless of constructor behavior.
        contract.authority = AuthorityLevel::DynamicMission;

        // Populate alias metadata so catalog can resolve command names with hyphens/aliases
        let mut aliases = Vec::new();
        aliases.push(format!("command.{}", self.name));
        aliases.push(self.name.clone());
        for a in &self.aliases {
            aliases.push(format!("command.{}", a));
            aliases.push(a.clone());
        }
        contract.compatibility = Some(crate::prompt::v2::CompatibilityMetadata {
            legacy_aliases: aliases,
            is_deprecated: false,
            canonical_replacement: None,
            deprecation_note: None,
        });
        // Reject templates referencing undeclared parameters: probe-render
        // with dummy values for every declared input under strict undefined
        // behavior so any undeclared `{{ variable }}` fails closed here
        // instead of silently rendering as empty text at runtime.
        probe_undeclared_template_variables(&contract)
            .map_err(|reason| UserCommandError::invalid(self.name.clone(), reason))?;
        Ok(contract)
    }

    /// Look up a declared input by flag name (`--dry-run` → `dry_run`;
    /// dashes and underscores are equivalent).
    pub fn find_input(&self, flag: &str) -> Option<&UserCommandInput> {
        let normalized = flag.trim().to_lowercase().replace('-', "_");
        self.inputs_required
            .iter()
            .chain(self.inputs_optional.iter())
            .find(|i| i.name == normalized)
    }

    /// Deterministically bind raw CLI tokens to declared inputs.
    ///
    /// Accepted syntax (no LLM involved):
    /// - `--flag` (boolean inputs only; sets `true`)
    /// - `--key value` / `--key=value` (string/integer/boolean inputs)
    /// - `--help` / `-h` (always accepted; returns `help_requested`)
    ///
    /// Positional arguments, unknown flags, missing required inputs, and
    /// type violations are deterministic errors.
    pub fn bind_arguments(&self, args: &[String]) -> Result<BoundArguments, UserCommandError> {
        let err = |reason: String| UserCommandError::ArgumentError {
            name: self.name.clone(),
            reason,
        };
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        let mut help_requested = false;
        let mut i = 0;
        while i < args.len() {
            let token = args[i].trim();
            if token == "--help" || token == "-h" || token == "help" && i == 0 && args.len() == 1 {
                help_requested = true;
                i += 1;
                continue;
            }
            if let Some(flag_body) = token.strip_prefix("--") {
                let (key, inline_value) = match flag_body.split_once('=') {
                    Some((k, v)) => (k.to_string(), Some(v.to_string())),
                    None => (flag_body.to_string(), None),
                };
                let key = key.trim();
                if key.is_empty() {
                    return Err(err("empty flag '--' is not accepted".to_string()));
                }
                let Some(decl) = self.find_input(key) else {
                    return Err(err(format!(
                        "unknown flag '--{key}'. {}",
                        self.valid_flags_hint()
                    )));
                };
                let raw_value = if let Some(v) = inline_value {
                    v
                } else if decl.input_type == UserCommandInputType::Boolean {
                    // Bare `--flag` sets booleans to true; `--flag <value>`
                    // with an explicit next token also accepted below.
                    if let Some(next) = args.get(i + 1)
                        && !next.trim_start().starts_with("--")
                        && (next.trim().eq_ignore_ascii_case("true")
                            || next.trim().eq_ignore_ascii_case("false"))
                    {
                        i += 1;
                        next.trim().to_string()
                    } else {
                        "true".to_string()
                    }
                } else if let Some(next) = args.get(i + 1) {
                    if next.trim_start().starts_with("--") {
                        return Err(err(format!(
                            "flag '--{key}' requires a value (got another flag)"
                        )));
                    }
                    i += 1;
                    next.trim().to_string()
                } else {
                    return Err(err(format!("flag '--{key}' requires a value")));
                };
                decl.input_type
                    .validate_value(&decl.name, &raw_value)
                    .map_err(err)?;
                if values.insert(decl.name.clone(), raw_value).is_some() {
                    return Err(err(format!("duplicate flag '--{key}'")));
                }
                i += 1;
                continue;
            }
            return Err(err(format!(
                "unexpected positional argument '{token}': user commands accept '--flag' options only. {}",
                self.valid_flags_hint()
            )));
        }

        // Apply defaults and enforce required inputs.
        for input in self
            .inputs_required
            .iter()
            .chain(self.inputs_optional.iter())
        {
            if !values.contains_key(&input.name) {
                if let Some(default) = &input.default {
                    values.insert(input.name.clone(), default.clone());
                } else if input.required {
                    return Err(err(format!(
                        "missing required flag '--{}' ({})",
                        input.name.replace('_', "-"),
                        input.description
                    )));
                }
            }
        }
        // Boolean optionals without explicit defaults default to false.
        for input in self.inputs_optional.iter() {
            if input.input_type == UserCommandInputType::Boolean
                && !values.contains_key(&input.name)
            {
                values.insert(input.name.clone(), "false".to_string());
            }
        }
        Ok(BoundArguments {
            values,
            help_requested,
        })
    }

    fn valid_flags_hint(&self) -> String {
        let mut flags: Vec<String> = self
            .inputs_required
            .iter()
            .chain(self.inputs_optional.iter())
            .map(|i| format!("--{}", i.name.replace('_', "-")))
            .collect();
        flags.sort();
        if flags.is_empty() {
            "this command accepts no flags (besides --help).".to_string()
        } else {
            format!("valid flags: {}.", flags.join(", "))
        }
    }

    /// Generated usage line derived from the declared argument schema
    /// (used when the TOML omits an explicit `usage`).
    pub fn generated_usage(&self) -> String {
        let mut parts = vec![format!("/{}", self.name)];
        let mut req: Vec<&UserCommandInput> = self.inputs_required.iter().collect();
        let mut opt: Vec<&UserCommandInput> = self.inputs_optional.iter().collect();
        req.sort_by(|a, b| a.name.cmp(&b.name));
        opt.sort_by(|a, b| a.name.cmp(&b.name));
        for input in req {
            parts.push(format!(
                "--{} <{}>",
                input.name.replace('_', "-"),
                input.input_type.as_str()
            ));
        }
        for input in opt {
            if input.input_type == UserCommandInputType::Boolean {
                parts.push(format!("[--{}]", input.name.replace('_', "-")));
            } else {
                parts.push(format!(
                    "[--{} <{}>]",
                    input.name.replace('_', "-"),
                    input.input_type.as_str()
                ));
            }
        }
        parts.join(" ")
    }

    /// Human-readable help block derived from TOML metadata (never hardcoded).
    pub fn describe(&self) -> String {
        let aliases = if self.aliases.is_empty() {
            "none".to_string()
        } else {
            self.aliases
                .iter()
                .map(|a| format!("/{a}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let effect = match self.side_effect {
            CommandSideEffect::ReadOnly => "Read-only",
            CommandSideEffect::Mutating => "Mutating",
            CommandSideEffect::SessionControl => "Session control",
            CommandSideEffect::Terminal => "Terminal",
        };
        let mut out = format!(
            "Command: /{name}\nUsage:   {usage}\nAliases: {aliases}\nEffect:  {effect}\nApproval required: {approval}\nMax steps: {max}\nPrompt:  {prompt_ref} ({source})\n\n{desc}",
            approval = self.requires_approval,
            desc = self.description,
            name = self.name,
            usage = if self.usage.trim().is_empty() {
                self.generated_usage()
            } else {
                self.usage.clone()
            },
            max = self.max_steps,
            prompt_ref = self.prompt_reference(),
            source = self
                .source_path
                .clone()
                .unwrap_or_else(|| "<global user commands>".to_string()),
        );
        let mut inputs: Vec<&UserCommandInput> = self
            .inputs_required
            .iter()
            .chain(self.inputs_optional.iter())
            .collect();
        if !inputs.is_empty() {
            inputs.sort_by(|a, b| a.name.cmp(&b.name));
            out.push_str("\n\nArguments:");
            for input in inputs {
                let req = if input.required {
                    "required"
                } else {
                    "optional"
                };
                let default = input
                    .default
                    .as_ref()
                    .map(|d| format!(" [default: {d}]"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "\n  --{} <{}> ({req}{default}) — {}",
                    input.name.replace('_', "-"),
                    input.input_type.as_str(),
                    input.description
                ));
            }
        }
        if !self.requested_capabilities.is_empty() {
            out.push_str(&format!(
                "\n\nRequested capabilities (advisory; granted only by runtime policy): {}",
                self.requested_capabilities.join(", ")
            ));
        }
        if self.verification_required && !self.verification_checks.is_empty() {
            out.push_str(&format!(
                "\nVerification checks: {}",
                self.verification_checks.join(", ")
            ));
        }
        out
    }
}

/// Deterministically bound command arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundArguments {
    /// Parameter name → string value (defaults applied).
    pub values: BTreeMap<String, String>,
    pub help_requested: bool,
}

/// Immutable runtime snapshot of a user command contract, reference, and definition (Phase E).
///
/// Encapsulates the complete immutable execution snapshot so execution never
/// re-reads TOML from the filesystem or resolves divergent prompt bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCommandDefinition {
    pub command: Arc<PromptCommand>,
    pub contract: PromptContract,
    pub prompt_ref: PromptReference,
}

impl UserCommandDefinition {
    pub fn new(command: Arc<PromptCommand>) -> Result<Self, UserCommandError> {
        let contract = command.to_prompt_contract()?;
        let prompt_ref = command.prompt_reference();
        Ok(Self {
            command,
            contract,
            prompt_ref,
        })
    }
}

// ── TOML schema ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct UserCommandFile {
    id: Option<String>,
    version: Option<u32>,
    kind: Option<String>,
    #[serde(default)]
    command: Option<CommandSection>,
    #[serde(default)]
    execution: Option<ExecutionSection>,
    #[serde(default)]
    capabilities: Option<CapabilitiesSection>,
    #[serde(default)]
    inputs: Option<InputsSection>,
    #[serde(default)]
    verification: Option<VerificationSection>,
    #[serde(default)]
    template: Option<TemplateSection>,
}

#[derive(Debug, Default, Deserialize)]
struct CommandSection {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    usage: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ExecutionSection {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    side_effect: Option<String>,
    #[serde(default)]
    requires_approval: Option<bool>,
    #[serde(default)]
    max_steps: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct CapabilitiesSection {
    #[serde(default)]
    required: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct InputsSection {
    #[serde(default)]
    required: Vec<InputEntry>,
    #[serde(default)]
    optional: Vec<InputEntry>,
}

#[derive(Debug, Deserialize)]
struct InputEntry {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "type")]
    type_name: Option<String>,
    #[serde(default)]
    default: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct VerificationSection {
    #[serde(default)]
    required: Option<bool>,
    #[serde(default)]
    checks: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TemplateSection {
    #[serde(default)]
    body: Option<String>,
}

/// Parse and validate a user command TOML document.
///
/// `source_path` is provenance only (display/diagnostics), never trusted.
pub fn parse_user_command_toml(
    content: &str,
    source_path: Option<String>,
) -> Result<PromptCommand, UserCommandError> {
    let file: UserCommandFile =
        toml::from_str(content).map_err(|e| UserCommandError::FileRejected {
            file: source_path
                .clone()
                .unwrap_or_else(|| "<inline>".to_string()),
            reason: format!("TOML parse error: {e}"),
        })?;
    build_prompt_command(file, source_path)
}

fn build_prompt_command(
    file: UserCommandFile,
    source_path: Option<String>,
) -> Result<PromptCommand, UserCommandError> {
    let label = source_path
        .clone()
        .unwrap_or_else(|| "<inline>".to_string());
    let reject = |reason: String| UserCommandError::FileRejected {
        file: label.clone(),
        reason,
    };

    let id = file
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| reject("missing required top-level 'id'".to_string()))?;
    validate_contract_id(id).map_err(reject)?;
    let version = file.version.filter(|v| *v >= 1).ok_or_else(|| {
        reject("missing or invalid top-level 'version' (must be >= 1)".to_string())
    })?;
    let kind = file.kind.as_deref().map(str::trim).unwrap_or("");
    if kind != "command" {
        return Err(reject(format!(
            "top-level 'kind' must be \"command\" for global user commands (got '{kind}')"
        )));
    }

    let cmd = file
        .command
        .ok_or_else(|| reject("missing required '[command]' section".to_string()))?;
    let raw_name = cmd
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| reject("missing required '[command] name'".to_string()))?;
    let name = validate_command_name(raw_name).map_err(reject)?;
    let mut aliases = Vec::with_capacity(cmd.aliases.len());
    {
        let mut seen = HashSet::new();
        seen.insert(name.clone());
        for raw in &cmd.aliases {
            let alias = validate_command_name(raw).map_err(reject)?;
            if !seen.insert(alias.clone()) {
                return Err(reject(format!(
                    "duplicate command alias '{raw}' (collides with command name or another alias)"
                )));
            }
            aliases.push(alias);
        }
        aliases.sort();
    }
    let description = cmd
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| reject("missing required '[command] description'".to_string()))?;
    if description.len() > 2000 {
        return Err(reject(
            "'[command] description' exceeds 2000 characters".to_string(),
        ));
    }

    let exec = file.execution.unwrap_or_default();
    let role = exec
        .role
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| reject("missing required '[execution] role'".to_string()))?;
    let role = role
        .parse::<AgentRole>()
        .map_err(|e| reject(format!("invalid '[execution] role': {e}")))?;
    if let Ok(guard) = crate::agent::registry::RoleRegistry::global().read() {
        if !guard.contains(&role) {
            return Err(reject(format!(
                "unknown '[execution] role' '{}': role is not registered in RoleRegistry",
                role
            )));
        }
    }
    let side_effect = match exec
        .side_effect
        .as_deref()
        .map(|s| s.trim().to_lowercase().replace('-', "_"))
        .unwrap_or_else(|| "mutating".to_string())
        .as_str()
    {
        "readonly" | "read_only" | "read" => CommandSideEffect::ReadOnly,
        "mutating" | "mutate" | "write" => CommandSideEffect::Mutating,
        other => {
            return Err(reject(format!(
                "invalid '[execution] side_effect' '{other}': expected 'readonly' or 'mutating' (session-control/terminal commands cannot be user-defined)"
            )));
        }
    };
    let requires_approval = exec
        .requires_approval
        .unwrap_or(matches!(side_effect, CommandSideEffect::Mutating));
    let max_steps = exec.max_steps.unwrap_or(DEFAULT_USER_COMMAND_STEPS);
    if max_steps == 0 || max_steps > MAX_USER_COMMAND_STEPS {
        return Err(reject(format!(
            "'[execution] max_steps' must be in 1..={MAX_USER_COMMAND_STEPS} (got {max_steps})"
        )));
    }

    let mut requested_capabilities = file.capabilities.map(|c| c.required).unwrap_or_default();
    for cap in &requested_capabilities {
        validate_capability_request(cap).map_err(reject)?;
    }
    requested_capabilities.sort();
    requested_capabilities.dedup();

    let inputs = file.inputs.unwrap_or_default();
    let inputs_required = parse_inputs(inputs.required, true).map_err(reject)?;
    let inputs_optional = parse_inputs(inputs.optional, false).map_err(reject)?;
    {
        let mut seen = HashSet::new();
        for input in inputs_required.iter().chain(inputs_optional.iter()) {
            if !seen.insert(input.name.clone()) {
                return Err(reject(format!(
                    "duplicate input parameter '{}' (required and optional must be disjoint)",
                    input.name
                )));
            }
        }
    }

    let verification = file.verification.unwrap_or_default();
    let verification_required = verification.required.unwrap_or(false);
    for check in &verification.checks {
        if !KNOWN_VERIFICATION_CHECKS.contains(&check.trim()) {
            return Err(reject(format!(
                "unknown '[verification] check' '{check}': expected one of {}",
                KNOWN_VERIFICATION_CHECKS.join(", ")
            )));
        }
    }
    let mut verification_checks = verification.checks;
    verification_checks.sort();
    verification_checks.dedup();

    let template_body = file
        .template
        .and_then(|t| t.body)
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
        .ok_or_else(|| reject("missing required '[template] body'".to_string()))?;
    if template_body.len() > MAX_USER_COMMAND_FILE_SIZE_BYTES as usize {
        return Err(reject(format!(
            "'[template] body' size ({}) exceeds maximum ({} bytes)",
            template_body.len(),
            MAX_USER_COMMAND_FILE_SIZE_BYTES
        )));
    }

    let usage = cmd
        .usage
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();

    let command = PromptCommand {
        id: id.to_string(),
        version,
        name,
        aliases,
        description: description.to_string(),
        usage,
        role,
        side_effect,
        requires_approval,
        max_steps,
        requested_capabilities,
        inputs_required,
        inputs_optional,
        verification_required,
        verification_checks,
        template_body,
        source_path,
    };
    // Validate eagerly: contract synthesis (MiniJinja syntax + undeclared
    // template variables) must fail at load, never at first invocation.
    command
        .to_prompt_contract()
        .map_err(|e| reject(format!("template validation failed: {e}")))?;
    Ok(command)
}

fn validate_contract_id(id: &str) -> Result<(), String> {
    if !id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return Err(format!(
            "contract id '{id}' must be non-empty and contain only alphanumeric, '_', or '.'"
        ));
    }
    if !id.starts_with(USER_COMMAND_CONTRACT_PREFIX) {
        return Err(format!(
            "global user command contract id '{id}' must use the reserved '{USER_COMMAND_CONTRACT_PREFIX}' namespace"
        ));
    }
    if is_layer0_contract_id(id) {
        return Err(format!(
            "contract id '{id}' targets protected Layer-0 runtime contracts and cannot be user-defined"
        ));
    }
    if is_behavioral_contract_id(id) {
        return Err(format!(
            "contract id '{id}' targets protected behavioral contracts and cannot be user-defined"
        ));
    }
    Ok(())
}

/// Validate a slash command name or alias.
///
/// Stored WITHOUT a leading `/`; lookup accepts both forms. Rules mirror the
/// deterministic parser: no whitespace, no empty names, ASCII
/// letters/digits with `-`/`_` separators, normalized to lowercase.
pub fn validate_command_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("command name cannot be empty".to_string());
    }
    if trimmed.starts_with('/') {
        return Err(format!(
            "command name '{raw}' must be stored without a leading '/'"
        ));
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return Err(format!("command name '{raw}' must not contain whitespace"));
    }
    if trimmed.len() > 64 {
        return Err(format!("command name '{raw}' exceeds 64 characters"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "command name '{raw}' may only contain ASCII letters, digits, '-', and '_'"
        ));
    }
    if !trimmed
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        return Err(format!(
            "command name '{raw}' must start with a letter or digit"
        ));
    }
    Ok(trimmed.to_lowercase())
}

fn validate_capability_request(cap: &str) -> Result<(), String> {
    let trimmed = cap.trim();
    if trimmed.is_empty() {
        return Err("capability request cannot be empty".to_string());
    }
    if trimmed.len() > 128 {
        return Err(format!(
            "capability request '{trimmed}' exceeds 128 characters"
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '*')
    {
        return Err(format!(
            "capability request '{trimmed}' contains unsafe characters"
        ));
    }
    Ok(())
}

fn parse_inputs(entries: Vec<InputEntry>, required: bool) -> Result<Vec<UserCommandInput>, String> {
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = entry.name.trim().to_lowercase().replace('-', "_");
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!(
                "input parameter name '{}' must be non-empty snake_case ([a-z0-9_])",
                entry.name
            ));
        }
        if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
            return Err(format!(
                "input parameter name '{name}' must start with a lowercase letter"
            ));
        }
        let input_type =
            UserCommandInputType::parse(entry.type_name.as_deref().unwrap_or("string"))?;
        let default = match entry.default {
            None => None,
            Some(toml::Value::String(s)) => Some(s),
            Some(toml::Value::Boolean(b)) => Some(b.to_string()),
            Some(toml::Value::Integer(n)) => Some(n.to_string()),
            Some(other) => {
                return Err(format!(
                    "input parameter '{name}': unsupported default value type ({other})"
                ));
            }
        };
        if required && default.is_some() {
            return Err(format!(
                "input parameter '{name}' is required and must not declare a default"
            ));
        }
        if let Some(ref d) = default {
            input_type.validate_value(&name, d)?;
        }
        out.push(UserCommandInput {
            name,
            description: entry.description.unwrap_or_default(),
            input_type,
            required,
            default,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Probe-render the contract template with dummy values for every declared
/// input under strict undefined behavior so templates referencing
/// undeclared `{{ variables }}` fail closed at load time.
fn probe_undeclared_template_variables(contract: &PromptContract) -> Result<(), String> {
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    env.add_template(&contract.id, &contract.template_body)
        .map_err(|e| format!("MiniJinja template syntax error: {e}"))?;
    let tmpl = env
        .get_template(&contract.id)
        .map_err(|e| format!("failed to retrieve compiled template: {e}"))?;
    let mut probe: BTreeMap<String, String> = BTreeMap::new();
    for param in &contract.input_parameters {
        probe.insert(param.name.clone(), "__m31a_probe__".to_string());
    }
    tmpl.render(&probe)
        .map(|_| ())
        .map_err(|e| format!("template references undeclared parameter: {e}"))?;
    Ok(())
}

// ── Global directory resolution & loading ────────────────────────────────────

/// Canonical global user command directory for a deployment channel:
///
/// ```text
/// <global_config_dir>/prompts/commands/
/// ```
///
/// The root always comes from M31A's existing path abstraction
/// ([`crate::persistence::paths::global_config_dir_for_channel`]) — never a
/// hardcoded `~/.config/m31a`. Channel isolation is therefore inherited:
/// development builds resolve the isolated `m31a-dev` root.
pub fn global_user_commands_dir_for_channel(channel: DeploymentChannel) -> Option<PathBuf> {
    crate::persistence::paths::global_config_dir_for_channel(channel)
        .map(|root| root.join(GLOBAL_USER_COMMANDS_SUBDIR))
}

/// Canonical global user command directory for the running artifact's channel.
pub fn global_user_commands_dir() -> Option<PathBuf> {
    global_user_commands_dir_for_channel(DeploymentChannel::current())
}

/// Outcome of scanning the global user command directory.
///
/// Loading is fail-closed per file: malformed definitions are reported in
/// `rejected` and never partially registered, while valid commands still
/// load. Callers must surface `rejected` as diagnostics and must never
/// crash the session because of them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UserCommandLoadReport {
    pub loaded: Vec<PromptCommand>,
    pub rejected: Vec<UserCommandRejection>,
}

/// A single rejected command file with a deterministic diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCommandRejection {
    pub file: String,
    pub reason: String,
}

impl UserCommandLoadReport {
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty() && self.rejected.is_empty()
    }

    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    pub fn rejected_count(&self) -> usize {
        self.rejected.len()
    }

    /// Count of rejections caused by command/alias collisions.
    pub fn collision_count(&self) -> usize {
        self.rejected
            .iter()
            .filter(|r| r.reason.contains("collides") || r.reason.contains("duplicate"))
            .count()
    }

    /// File rejections caused by TOML parsing, UTF-8, or size violations.
    pub fn parse_failures(&self) -> Vec<&UserCommandRejection> {
        self.rejected
            .iter()
            .filter(|r| {
                r.reason.contains("parse error")
                    || r.reason.contains("not valid UTF-8")
                    || r.reason.contains("file size")
                    || r.reason.contains("TOML")
            })
            .collect()
    }

    /// Rejections caused by invalid MiniJinja templates or undeclared variables.
    pub fn invalid_template_failures(&self) -> Vec<&UserCommandRejection> {
        self.rejected
            .iter()
            .filter(|r| {
                r.reason.contains("template")
                    || r.reason.contains("MiniJinja")
                    || r.reason.contains("undeclared")
            })
            .collect()
    }

    /// Rejections caused by protected namespaces or built-in collisions.
    pub fn protected_name_failures(&self) -> Vec<&UserCommandRejection> {
        self.rejected
            .iter()
            .filter(|r| {
                r.reason.contains("collides with built-in")
                    || r.reason.contains("protected")
                    || r.reason.contains("reserved")
            })
            .collect()
    }

    /// Deterministic human-readable diagnostics for rejected files.
    pub fn diagnostics(&self) -> Vec<String> {
        self.rejected
            .iter()
            .map(|r| format!("rejected user command '{}': {}", r.file, r.reason))
            .collect()
    }
}

/// Load global user commands for the running artifact's deployment channel.
///
/// A missing directory is harmless (empty report): users without custom
/// commands see exactly the built-in set.
pub fn load_global_user_commands() -> UserCommandLoadReport {
    match global_user_commands_dir() {
        Some(dir) => load_global_user_commands_from_dir(&dir),
        None => UserCommandLoadReport::default(),
    }
}

/// Load global user commands for an explicit deployment channel (respects
/// channel isolation; used by tests and channel-aware callers).
pub fn load_global_user_commands_for_channel(channel: DeploymentChannel) -> UserCommandLoadReport {
    match global_user_commands_dir_for_channel(channel) {
        Some(dir) => load_global_user_commands_from_dir(&dir),
        None => UserCommandLoadReport::default(),
    }
}

/// Load and validate `*.toml` command definitions from a directory.
///
/// Safety limits (mirroring the prompt catalog, scoped to this directory):
/// bounded recursion depth, bounded file size, UTF-8 enforcement, no path
/// traversal, no symlink escape from the commands directory, no `.git`
/// traversal, deterministic ordering, deterministic duplicate detection.
pub fn load_global_user_commands_from_dir(dir: &Path) -> UserCommandLoadReport {
    let mut report = UserCommandLoadReport::default();
    if !dir.exists() {
        return report;
    }
    if !dir.is_dir() {
        report.rejected.push(UserCommandRejection {
            file: dir.display().to_string(),
            reason: "global user command location is not a directory".to_string(),
        });
        return report;
    }

    let canonical_root = match std::fs::canonicalize(dir) {
        Ok(p) => p,
        Err(e) => {
            report.rejected.push(UserCommandRejection {
                file: dir.display().to_string(),
                reason: format!("failed to resolve command directory: {e}"),
            });
            return report;
        }
    };

    let mut files = Vec::new();
    if let Err(e) = collect_command_files(&canonical_root, &canonical_root, &mut files, 0) {
        report.rejected.push(UserCommandRejection {
            file: canonical_root.display().to_string(),
            reason: e,
        });
        return report;
    }
    files.sort();

    // Deterministic duplicate detection across files.
    let mut seen_contracts: BTreeMap<(String, u32), String> = BTreeMap::new();
    let mut seen_names: BTreeMap<String, String> = BTreeMap::new();

    for file_path in files {
        let file_label = file_path.display().to_string();
        let reject = |reason: String| UserCommandRejection {
            file: file_label.clone(),
            reason,
        };
        let metadata = match std::fs::metadata(&file_path) {
            Ok(m) => m,
            Err(e) => {
                report
                    .rejected
                    .push(reject(format!("failed to read metadata: {e}")));
                continue;
            }
        };
        if metadata.len() > MAX_USER_COMMAND_FILE_SIZE_BYTES {
            report.rejected.push(reject(format!(
                "file size {} exceeds maximum permitted bound of {} bytes",
                metadata.len(),
                MAX_USER_COMMAND_FILE_SIZE_BYTES
            )));
            continue;
        }
        let bytes = match std::fs::read(&file_path) {
            Ok(b) => b,
            Err(e) => {
                report
                    .rejected
                    .push(reject(format!("failed to read file: {e}")));
                continue;
            }
        };
        let content = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) => {
                report
                    .rejected
                    .push(reject(format!("file is not valid UTF-8: {e}")));
                continue;
            }
        };
        let command = match parse_user_command_toml(content, Some(file_label.clone())) {
            Ok(c) => c,
            Err(e) => {
                report.rejected.push(reject(e.to_string()));
                continue;
            }
        };
        let contract_key = (command.id.clone(), command.version);
        if let Some(first) = seen_contracts.get(&contract_key) {
            report.rejected.push(reject(format!(
                "duplicate (id, version) '{} v{}' (first defined in '{first}')",
                command.id, command.version
            )));
            continue;
        }
        let mut name_collision: Option<String> = None;
        for candidate in std::iter::once(&command.name).chain(command.aliases.iter()) {
            if let Some(first) = seen_names.get(candidate) {
                name_collision = Some(format!(
                    "duplicate command name/alias '{candidate}' (first defined in '{first}')"
                ));
                break;
            }
        }
        if let Some(reason) = name_collision {
            report.rejected.push(reject(reason));
            continue;
        }
        seen_contracts.insert(contract_key, file_label.clone());
        seen_names.insert(command.name.clone(), file_label.clone());
        for alias in &command.aliases {
            seen_names.insert(alias.clone(), file_label.clone());
        }
        report.loaded.push(command);
    }
    // Deterministic command ordering by (name, version).
    report
        .loaded
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));
    report
}

fn collect_command_files(
    dir: &Path,
    root: &Path,
    results: &mut Vec<PathBuf>,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_COMMAND_DISCOVERY_DEPTH {
        return Ok(());
    }
    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to open directory '{}': {e}", dir.display()))?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        if file_name.starts_with('.') {
            continue;
        }
        if path
            .components()
            .any(|c| matches!(c, Component::Normal(s) if s == ".git"))
        {
            continue;
        }
        let is_symlink = entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);
        if is_symlink {
            let resolved = std::fs::canonicalize(&path).map_err(|e| {
                format!("failed to resolve symlink target '{}': {e}", path.display())
            })?;
            if !resolved.starts_with(root) {
                return Err(format!(
                    "symlink '{}' escapes the global user command directory",
                    path.display()
                ));
            }
            if resolved
                .components()
                .any(|c| matches!(c, Component::Normal(s) if s == ".git"))
            {
                return Err(format!(
                    "symlink '{}' points into protected '.git' directory",
                    path.display()
                ));
            }
        }
        if path.is_dir() {
            collect_command_files(&path, root, results, depth + 1)?;
        } else if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
            results.push(path);
        }
    }
    Ok(())
}

// ── Conventional Commit helpers ──────────────────────────────────────────────

/// Check whether a commit message follows Conventional Commit structure
/// (`type(scope)?: description` on the first line).
pub fn is_conventional_commit_message(message: &str) -> bool {
    let first_line = message.lines().next().unwrap_or("").trim();
    let Some((ty, rest)) = first_line.split_once(':') else {
        return false;
    };
    let ty = ty.trim();
    // Optional `!` (breaking) and optional `(scope)`.
    let (ty_base, scoped) = if let Some(bang_stripped) = ty.strip_suffix('!') {
        (bang_stripped.trim(), true)
    } else {
        (ty, false)
    };
    let _ = scoped;
    let base = if let Some(open) = ty_base.find('(') {
        if !ty_base.ends_with(')') {
            return false;
        }
        let scope = &ty_base[open + 1..ty_base.len() - 1];
        if scope.trim().is_empty() || scope.contains(['(', ')', '\n']) {
            return false;
        }
        &ty_base[..open]
    } else {
        ty_base
    };
    if !CONVENTIONAL_COMMIT_TYPES.contains(&base) {
        return false;
    }
    !rest.trim().is_empty()
}

/// Infer a Conventional Commit type from a file path + diff summary using a
/// deterministic heuristic (documentation, tests, chores, features, fixes).
pub fn infer_conventional_type(path: &str, diff_summary: &str) -> &'static str {
    let p = path.to_lowercase();
    let d = diff_summary.to_lowercase();
    if p.ends_with(".md")
        || p.contains("readme")
        || p.starts_with("docs/")
        || p.starts_with("docs\\")
    {
        return "docs";
    }
    if p.contains("test") || p.starts_with("tests/") || d.contains("test") {
        return "test";
    }
    if p.contains("cargo.toml")
        || p.contains("cargo.lock")
        || p.starts_with(".github/")
        || p.contains("ci")
    {
        return "chore";
    }
    if d.contains("fix") || d.contains("bug") || d.contains("error") {
        return "fix";
    }
    if p.starts_with("src/") || p.ends_with(".rs") {
        return "feat";
    }
    "chore"
}

/// Build a Conventional Commit subject line (`type(scope): summary`).
pub fn format_conventional_subject(
    commit_type: &str,
    scope: Option<&str>,
    summary: &str,
) -> String {
    let summary = summary.trim().trim_end_matches('.');
    let summary = if summary.is_empty() {
        "update"
    } else {
        summary
    };
    match scope.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => format!("{commit_type}({s}): {summary}"),
        None => format!("{commit_type}: {summary}"),
    }
}

// ===========================================================================
// PromptCommandHandler — generic handler for user-defined slash commands
// ===========================================================================

use crate::interaction::commands::{CommandContext, CommandHandler, CommandOutput};
use async_trait::async_trait;

/// Generic handler for global user-defined slash commands.
///
/// All user commands share this single handler implementation. It routes
/// execution through the canonical runtime path:
///
/// ```text
/// PromptCommand (typed contract)
///     ↓
/// PromptReference
///     ↓
/// canonical PromptCatalog
///     ↓
/// canonical PromptCompiler
///     ↓
/// ModelInvocationKind::UserCommand → ModelCaller
///     ↓
/// model tool proposals → PolicyGate → ApprovalCoordinator → ToolPipeline
///     ↓
/// Verification → CommandOutput
/// ```
///
/// The handler NEVER directly executes shell commands, loads arbitrary
/// code, bypasses the policy system, grants capabilities, bypasses
/// approval, or disables sandboxing.
pub struct PromptCommandHandler {
    command: Arc<PromptCommand>,
}

impl PromptCommandHandler {
    pub fn new(command: Arc<PromptCommand>) -> Self {
        Self { command }
    }

    pub fn command(&self) -> &Arc<PromptCommand> {
        &self.command
    }
}

#[async_trait]
impl CommandHandler for PromptCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        match self.command.bind_arguments(args) {
            Ok(bound) => {
                if bound.help_requested {
                    Ok(CommandOutput::info(self.command.describe()))
                } else {
                    Ok(CommandOutput::ApplicationAction(
                        crate::interaction::action::ApplicationAction::UserCommandRequested {
                            command: self.command.name.clone(),
                            args: args.to_vec(),
                        },
                    ))
                }
            }
            Err(e) => Ok(CommandOutput::error(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_TOML: &str = r#"
id = "command.atomic_commit"
version = 1
kind = "command"

[command]
name = "atomic-commit"
aliases = ["ac"]
description = "Commit each changed file independently using conventional commits."
usage = "/atomic-commit [--dry-run]"

[execution]
role = "integrator"
side_effect = "mutating"
requires_approval = true
max_steps = 40

[capabilities]
required = [
    "git.read",
    "git.add",
    "git.commit",
]

[inputs]
optional = [
    { name = "dry_run", type = "boolean", default = false },
    { name = "invocation", type = "string", default = "user-command" },
]

[verification]
required = true
checks = [
    "working_tree_status",
    "commit_contains_single_file",
    "conventional_commit_message",
]

[template]
body = """
You are executing the user's /atomic-commit command for {{ invocation }}.
"""
"#;

    #[test]
    fn example_toml_parses_to_typed_contract() {
        let cmd = parse_user_command_toml(EXAMPLE_TOML, None).unwrap();
        assert_eq!(cmd.id, "command.atomic_commit");
        assert_eq!(cmd.version, 1);
        assert_eq!(cmd.name, "atomic-commit");
        assert_eq!(cmd.aliases, vec!["ac"]);
        assert_eq!(cmd.role.as_str(), "integrator");
        assert_eq!(cmd.side_effect, CommandSideEffect::Mutating);
        assert!(cmd.requires_approval);
        assert_eq!(cmd.max_steps, 40);
        assert_eq!(
            cmd.requested_capabilities,
            vec!["git.add", "git.commit", "git.read"]
        );
        assert_eq!(cmd.inputs_required.len(), 0);
        assert_eq!(cmd.inputs_optional.len(), 2);
        assert!(cmd.verification_required);
        let contract = cmd.to_prompt_contract().unwrap();
        assert_eq!(contract.authority, AuthorityLevel::DynamicMission);
        assert_eq!(contract.id, "command.atomic_commit");
        // Command identity round-trips through a first-class prompt reference.
        assert_eq!(
            cmd.prompt_reference(),
            PromptReference::new("command.atomic_commit", 1)
        );
    }

    #[test]
    fn non_command_kind_rejected() {
        let toml = EXAMPLE_TOML.replace("kind = \"command\"", "kind = \"execution\"");
        assert!(parse_user_command_toml(&toml, None).is_err());
    }

    #[test]
    fn non_command_namespace_rejected() {
        let toml = EXAMPLE_TOML.replace("command.atomic_commit", "execution.atomic_commit");
        assert!(parse_user_command_toml(&toml, None).is_err());
    }

    #[test]
    fn layer0_and_behavioral_ids_rejected() {
        for id in [
            "runtime.safety_invariants",
            "core.safety",
            "agent.implementer",
        ] {
            let toml = EXAMPLE_TOML.replace("command.atomic_commit", id);
            assert!(
                parse_user_command_toml(&toml, None).is_err(),
                "id '{id}' must be rejected"
            );
        }
    }

    #[test]
    fn invalid_names_rejected() {
        assert!(validate_command_name("/atomic-commit").is_err());
        assert!(validate_command_name("atomic commit").is_err());
        assert!(validate_command_name("").is_err());
        assert!(validate_command_name("Atomic-Commit").is_ok());
        assert_eq!(
            validate_command_name("Atomic-Commit").unwrap(),
            "atomic-commit"
        );
    }

    #[test]
    fn undeclared_template_variable_rejected() {
        let toml = EXAMPLE_TOML.replace("{{ invocation }}", "{{ something_undeclared }}");
        assert!(parse_user_command_toml(&toml, None).is_err());
    }

    #[test]
    fn malformed_template_rejected() {
        let toml = EXAMPLE_TOML.replace("You are executing", "You are executing {% if %}");
        assert!(parse_user_command_toml(&toml, None).is_err());
    }

    #[test]
    fn argument_binding_validates_deterministically() {
        let cmd = parse_user_command_toml(EXAMPLE_TOML, None).unwrap();
        let bound = cmd.bind_arguments(&[]).unwrap();
        assert_eq!(
            bound.values.get("dry_run").map(String::as_str),
            Some("false")
        );
        let bound = cmd.bind_arguments(&["--dry-run".to_string()]).unwrap();
        assert_eq!(
            bound.values.get("dry_run").map(String::as_str),
            Some("true")
        );
        let bound = cmd
            .bind_arguments(&["--dry-run=false".to_string()])
            .unwrap();
        assert_eq!(
            bound.values.get("dry_run").map(String::as_str),
            Some("false")
        );
        assert!(cmd.bind_arguments(&["--bogus".to_string()]).is_err());
        assert!(cmd.bind_arguments(&["positional".to_string()]).is_err());
        let bound = cmd.bind_arguments(&["--help".to_string()]).unwrap();
        assert!(bound.help_requested);
    }

    #[test]
    fn conventional_commit_helpers_behave() {
        assert!(is_conventional_commit_message("feat(auth): add login"));
        assert!(is_conventional_commit_message("fix: correct off-by-one"));
        assert!(is_conventional_commit_message("docs!: rewrite readme"));
        assert!(!is_conventional_commit_message("add login"));
        assert!(!is_conventional_commit_message("bogus: something"));
        assert!(!is_conventional_commit_message("feat:"));
        assert_eq!(infer_conventional_type("README.md", "update"), "docs");
        assert_eq!(infer_conventional_type("src/main.rs", "fix bug"), "fix");
        assert_eq!(
            format_conventional_subject("feat", Some("auth"), "add login."),
            "feat(auth): add login"
        );
    }
}
