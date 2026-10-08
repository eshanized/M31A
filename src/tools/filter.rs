//! Deterministic multi-stage model-visible tool filtering and pruning (TL-05, D-06).

use crate::agent::envelope::CapabilityEnvelope;
use crate::capability::family::CapabilityFamily;
use crate::capability::health::CapabilityHealthState;
use crate::capability::registry::CapabilityRegistry;
use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use crate::state::intake::AutonomyMode;
use crate::tools::definition::{AnyTool, to_openai_tool};
use crate::tools::registry::ToolRegistry;
use crate::tools::risk::RiskClass;
use std::collections::HashSet;
use std::sync::Arc;

/// Filter criteria governing tool eligibility for model context (TL-05, D-06).
pub struct FilterCriteria<'a> {
    pub capability_registry: Arc<CapabilityRegistry>,
    pub role_envelope: Option<&'a CapabilityEnvelope>,
    pub task_requirements: Option<&'a [CapabilityRequirement]>,
    pub autonomy_mode: Option<AutonomyMode>,
    pub denied_tools: HashSet<String>,
    pub context_budget_bytes: Option<usize>,
}

impl<'a> FilterCriteria<'a> {
    pub fn new(capability_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            capability_registry,
            role_envelope: None,
            task_requirements: None,
            autonomy_mode: None,
            denied_tools: HashSet::new(),
            context_budget_bytes: None,
        }
    }

    pub fn with_role_envelope(mut self, envelope: &'a CapabilityEnvelope) -> Self {
        self.role_envelope = Some(envelope);
        self
    }

    pub fn with_task_requirements(mut self, requirements: &'a [CapabilityRequirement]) -> Self {
        self.task_requirements = Some(requirements);
        self
    }

    pub fn with_autonomy_mode(mut self, mode: AutonomyMode) -> Self {
        self.autonomy_mode = Some(mode);
        self
    }

    pub fn with_denied_tools(
        mut self,
        denied: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.denied_tools = denied.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_context_budget(mut self, bytes: usize) -> Self {
        self.context_budget_bytes = Some(bytes);
        self
    }
}

/// Deterministic multi-stage tool filtering engine (TL-05).
pub struct ToolFilter {
    registry: Arc<ToolRegistry>,
}

impl ToolFilter {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// Filter tools according to the 8 canonical stages (D-06):
    /// 1. Start with all registered tools
    /// 2. Health check (drop Unavailable)
    /// 3. Global denials (drop denied_tools)
    /// 4. Role capability envelope (drop out-of-envelope)
    /// 5. Task requirements (intersect task_requirements)
    /// 6. AutonomyMode exclusions (drop mutating in Plan mode)
    /// 7. Deterministic sorting (lexicographical by ToolId)
    /// 8. Context budget check (omit oversized schemas with diagnostic, never truncate)
    pub fn filter_tools(&self, criteria: &FilterCriteria) -> Vec<Arc<dyn AnyTool>> {
        // Stage 1: Registered tools
        let all_tools = self.registry.list_tools();
        let cap_instances = criteria.capability_registry.list_capabilities();

        let mut eligible: Vec<Arc<dyn AnyTool>> = Vec::new();

        for tool in all_tools {
            // Stage 2: Health check
            // Exclude tool if all registered instances for any required capability are Unavailable
            let mut unavailable = false;
            for fam in tool.required_capabilities() {
                let fam_instances: Vec<_> = cap_instances
                    .iter()
                    .filter(|inst| &inst.family == fam)
                    .collect();

                if !fam_instances.is_empty()
                    && fam_instances
                        .iter()
                        .all(|inst| inst.health == CapabilityHealthState::Unavailable)
                {
                    unavailable = true;
                    break;
                }
            }
            if unavailable {
                continue;
            }

            // Stage 3: Global denials
            if criteria.denied_tools.contains(tool.id()) {
                continue;
            }

            // Stage 4: Role capability envelope
            if let Some(envelope) = criteria.role_envelope
                && !tool_allowed_by_envelope(&*tool, envelope)
            {
                continue;
            }

            // Stage 5: Task capability requirements
            if let Some(requirements) = criteria.task_requirements
                && !requirements.is_empty()
                && !tool_satisfies_task_requirements(&*tool, requirements)
            {
                continue;
            }

            // Stage 6: AutonomyMode exclusions
            // In Plan mode, all mutating tools are pruned
            if criteria.autonomy_mode == Some(AutonomyMode::Plan)
                && tool.base_risk() != RiskClass::ReadOnly
            {
                continue;
            }

            eligible.push(tool);
        }

        // Stage 7: Deterministic sorting lexicographically by ToolId
        eligible.sort_by(|a, b| a.id().cmp(b.id()));

        // Stage 8: Context budget check
        // Accumulate JSON schema bytes; if individual schema exceeds budget, omit tool with diagnostic
        let mut budgeted = Vec::new();
        let mut accumulated_bytes = 0;

        for tool in eligible {
            let tool_wire = to_openai_tool(&*tool);
            let schema_bytes = serde_json::to_vec(&tool_wire).map(|v| v.len()).unwrap_or(0);

            if let Some(budget) = criteria.context_budget_bytes
                && accumulated_bytes + schema_bytes > budget
            {
                tracing::warn!(
                    tool_id = tool.id(),
                    schema_bytes = schema_bytes,
                    accumulated_bytes = accumulated_bytes,
                    budget = budget,
                    "Omitting tool schema from context due to budget ceiling (never truncate)"
                );
                continue;
            }

            accumulated_bytes += schema_bytes;
            budgeted.push(tool);
        }

        budgeted
    }

    /// Produce the final model-visible wire format tools[] array.
    pub fn filter_to_wire_format(&self, criteria: &FilterCriteria) -> Vec<serde_json::Value> {
        let tools = self.filter_tools(criteria);
        tools.into_iter().map(|t| to_openai_tool(&*t)).collect()
    }
}

/// Helper evaluating whether a tool is admitted by a role's CapabilityEnvelope.
fn tool_allowed_by_envelope(tool: &dyn AnyTool, envelope: &CapabilityEnvelope) -> bool {
    // Universal lifecycle completion tool is always permitted across all agent roles
    if tool.id() == "complete" {
        return true;
    }

    let is_mutating = tool.base_risk() != RiskClass::ReadOnly;
    let req_caps = tool.required_capabilities();

    // 1. Filesystem write ceiling
    if is_mutating && req_caps.contains(&CapabilityFamily::Filesystem) && !envelope.allow_file_write
    {
        return false;
    }

    // 2. Git write ceiling
    if is_mutating && req_caps.contains(&CapabilityFamily::Git) && !envelope.allow_file_write {
        return false;
    }

    // 3. Shell / process execution ceiling
    if req_caps.iter().any(|c| {
        matches!(
            c,
            CapabilityFamily::Process
                | CapabilityFamily::Jobs
                | CapabilityFamily::Shell
                | CapabilityFamily::Terminal
        )
    }) && !envelope.allow_shell_execution
    {
        return false;
    }

    // 4. Network access ceiling
    if req_caps
        .iter()
        .any(|c| matches!(c, CapabilityFamily::Network | CapabilityFamily::Web))
        && !envelope.allow_network_access
    {
        return false;
    }

    // 5. Explicit capability set membership if non-empty
    if !envelope.allowed_capabilities.is_empty() {
        let matches_any = envelope
            .allowed_capabilities
            .iter()
            .any(|cap_str| match_tool_to_cap_pattern(tool, cap_str));

        if !matches_any {
            return false;
        }
    }

    true
}

/// Helper evaluating whether a tool satisfies any task capability requirement.
fn tool_satisfies_task_requirements(
    tool: &dyn AnyTool,
    requirements: &[CapabilityRequirement],
) -> bool {
    requirements.iter().any(|req| {
        // Mode compatibility check
        let mode_ok = match req.mode {
            CapabilityAccessMode::Read => tool.base_risk() == RiskClass::ReadOnly,
            CapabilityAccessMode::Write => tool.base_risk() != RiskClass::ReadOnly,
            CapabilityAccessMode::ReadWrite => true,
        };

        mode_ok && match_tool_to_cap_pattern(tool, &req.id)
    })
}

/// Matches a tool against a capability identifier pattern (e.g. "fs.read", "repo", "run_command", "*").
fn match_tool_to_cap_pattern(tool: &dyn AnyTool, pattern: &str) -> bool {
    if pattern == "*" || pattern == tool.id() || tool.id() == "complete" {
        return true;
    }

    let is_mutating = tool.base_risk() != RiskClass::ReadOnly;

    // Direct domain aliases used in canonical AgentProfile envelopes (AGT-01, AGT-04)
    match pattern {
        "git_ops" => {
            return tool
                .required_capabilities()
                .contains(&CapabilityFamily::Git);
        }
        "repo_index" => {
            return tool
                .required_capabilities()
                .contains(&CapabilityFamily::Repository);
        }
        "test_runner" => {
            return tool
                .required_capabilities()
                .contains(&CapabilityFamily::Verification);
        }
        "workspace_fs_write" => {
            return tool
                .required_capabilities()
                .contains(&CapabilityFamily::Filesystem)
                && is_mutating;
        }
        "workspace_fs_read" => {
            return tool
                .required_capabilities()
                .contains(&CapabilityFamily::Filesystem)
                && !is_mutating;
        }
        "compiler_exec" | "proc.exec" => {
            return tool.required_capabilities().iter().any(|c| {
                matches!(
                    c,
                    CapabilityFamily::Process
                        | CapabilityFamily::Jobs
                        | CapabilityFamily::Shell
                        | CapabilityFamily::Verification
                )
            });
        }
        "diff.analyze" => {
            return tool.id() == "git_diff" || tool.id() == "git_show" || tool.id() == "git_status";
        }
        "git.merge" => {
            return tool.id() == "git_branch"
                || tool.id() == "git_checkout"
                || tool.id() == "git_status"
                || tool.id() == "git_diff";
        }
        _ => {}
    }

    for fam in tool.required_capabilities() {
        let fam_names = family_names(fam);
        for fam_name in fam_names {
            if pattern == *fam_name {
                return true;
            }
            if let Some(suffix) = pattern.strip_prefix(&format!("{fam_name}."))
                && match_suffix(suffix, is_mutating, tool)
            {
                return true;
            }
            if let Some(suffix) = pattern.strip_prefix(&format!("{fam_name}:"))
                && match_suffix(suffix, is_mutating, tool)
            {
                return true;
            }
        }
    }

    // Verification aliases
    if tool
        .required_capabilities()
        .contains(&CapabilityFamily::Verification)
        && (pattern.starts_with("cargo.") || pattern.starts_with("qa."))
    {
        return true;
    }

    false
}

fn match_suffix(suffix: &str, is_mutating: bool, tool: &dyn AnyTool) -> bool {
    match suffix {
        "read" => !is_mutating,
        "write" => true, // Write permission in an envelope encompasses reading that domain
        "exec" | "execute" => {
            tool.base_risk() == RiskClass::ProcessExecution
                || tool.required_capabilities().iter().any(|c| {
                    matches!(
                        c,
                        CapabilityFamily::Process
                            | CapabilityFamily::Jobs
                            | CapabilityFamily::Shell
                    )
                })
        }
        "inspect" | "symbols" | "search" => !is_mutating,
        _ => true,
    }
}

fn family_names(fam: &CapabilityFamily) -> &'static [&'static str] {
    match fam {
        CapabilityFamily::Filesystem => &["fs", "filesystem"],
        CapabilityFamily::Shell => &["shell"],
        CapabilityFamily::Process => &["process"],
        CapabilityFamily::Terminal => &["terminal"],
        CapabilityFamily::Repository => &["repo", "repository"],
        CapabilityFamily::Git => &["git"],
        CapabilityFamily::Web => &["web"],
        CapabilityFamily::Network => &["network"],
        CapabilityFamily::Jobs => &["jobs", "job"],
        CapabilityFamily::Sandbox => &["sandbox"],
        CapabilityFamily::Model => &["model"],
        CapabilityFamily::Memory => &["memory"],
        CapabilityFamily::Verification => &["verification", "qa"],
        CapabilityFamily::Artifacts => &["artifacts", "artifact"],
        CapabilityFamily::Telemetry => &["telemetry"],
    }
}

/// Single authoritative scoped tool authority binding (Issue 4, 5).
///
/// Governs:
/// 1. Model-visible tool schemas for the active role and capability envelope
/// 2. Tool execution permissions and capabilities
/// 3. Delegated-agent role tool scoping
#[derive(Clone)]
pub struct ToolAuthorityScope {
    pub agent_id: Option<crate::ids::AgentId>,
    pub role: crate::state_machine::agent::AgentRole,
    pub capability_registry: Arc<CapabilityRegistry>,
    pub tool_registry: Arc<ToolRegistry>,
    pub denied_tools: Vec<String>,
    pub autonomy_mode: AutonomyMode,
}

impl ToolAuthorityScope {
    pub fn new(
        role: crate::state_machine::agent::AgentRole,
        capability_registry: Arc<CapabilityRegistry>,
        tool_registry: Arc<ToolRegistry>,
        autonomy_mode: AutonomyMode,
    ) -> Self {
        Self {
            agent_id: None,
            role,
            capability_registry,
            tool_registry,
            denied_tools: Vec::new(),
            autonomy_mode,
        }
    }

    pub fn with_agent_id(mut self, agent_id: crate::ids::AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    pub fn with_denied_tools(mut self, denied: Vec<String>) -> Self {
        self.denied_tools = denied;
        self
    }

    /// Derives model-visible tool schemas governed strictly by this scope's role and envelope.
    pub fn model_tool_schemas(&self) -> Vec<serde_json::Value> {
        let profile = crate::agent::profile::AgentProfile::built_in(self.role.clone());
        let criteria = FilterCriteria::new(self.capability_registry.clone())
            .with_role_envelope(&profile.capability_policy)
            .with_denied_tools(self.denied_tools.iter().cloned())
            .with_autonomy_mode(self.autonomy_mode);
        ToolFilter::new(self.tool_registry.clone()).filter_to_wire_format(&criteria)
    }

    /// Rebinds this scope to a new role, ensuring all authorities and schemas derive from the new role.
    pub fn for_role(&self, new_role: crate::state_machine::agent::AgentRole) -> Self {
        let mut cloned = self.clone();
        cloned.role = new_role;
        cloned
    }

    /// returns all tool names visible to the model under this scope.
    pub fn visible_tools(&self) -> Vec<String> {
        let profile = crate::agent::profile::AgentProfile::built_in(self.role.clone());
        let criteria = FilterCriteria::new(self.capability_registry.clone())
            .with_role_envelope(&profile.capability_policy)
            .with_denied_tools(self.denied_tools.iter().cloned())
            .with_autonomy_mode(self.autonomy_mode);
        ToolFilter::new(self.tool_registry.clone())
            .filter_tools(&criteria)
            .into_iter()
            .map(|t| t.id().to_string())
            .collect()
    }

    /// returns all tool names executable under this scope.
    /// hard invariant: visible_tools(scope) == executable_tools(scope)
    pub fn executable_tools(&self) -> Vec<String> {
        self.visible_tools()
    }

    /// check whether a tool is executable under this scope.
    pub fn is_tool_executable(&self, tool_name: &str) -> bool {
        self.executable_tools().iter().any(|t| t == tool_name)
    }

    /// create a delegated child scope ensuring it cannot inherit broader tool visibility
    /// than its role permits and preserves all parent denied tools.
    pub fn child_scope(
        &self,
        child_agent_id: crate::ids::AgentId,
        child_role: crate::state_machine::agent::AgentRole,
    ) -> Self {
        Self {
            agent_id: Some(child_agent_id),
            role: child_role,
            capability_registry: self.capability_registry.clone(),
            tool_registry: self.tool_registry.clone(),
            denied_tools: self.denied_tools.clone(),
            autonomy_mode: self.autonomy_mode,
        }
    }

    /// build a tool execution context bound to this scope identity and authorities.
    pub fn build_execution_context(
        &self,
        workspace_root: std::path::PathBuf,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> crate::tools::definition::ToolExecutionContext {
        let profile = crate::agent::profile::AgentProfile::built_in(self.role.clone());
        let mut ctx = crate::tools::definition::ToolExecutionContext::new(
            self.capability_registry.clone(),
            workspace_root,
            cancel_token,
        )
        .with_role_envelope(profile.capability_policy.clone())
        .with_agent_role(self.role.clone())
        .with_autonomy_mode(self.autonomy_mode);

        if let Some(aid) = self.agent_id {
            ctx = ctx.with_agent_id(aid);
        }
        ctx
    }
}
