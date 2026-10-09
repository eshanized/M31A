//! Settings surface — projection/editor of the canonical configuration system.
//!
//! `/settings` is a projection/editor of `ResolvedConfiguration`, never a
//! second configuration system. Categories mirror the actual `AppConfig`
//! schema; every row shows value, source/provenance, and restart-required
//! state. Edits validate through the typed schema, preview, confirm,
//! atomically persist, and reload the effective configuration.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::tui::theme::ThemeTokens;

/// Top-level settings categories (mirror the canonical `AppConfig` schema).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsCategory {
    General,
    Provider,
    Models,
    AgentsRoles,
    Runtime,
    Budgets,
    Execution,
    Verification,
    ToolsLimits,
    Workflow,
    Git,
    PromptsSkills,
    Cache,
    TuiInterface,
    EnvOverrides,
    EffectiveConfig,
    About,
}

impl SettingsCategory {
    pub fn all() -> &'static [Self] {
        &[
            Self::General,
            Self::Provider,
            Self::Models,
            Self::AgentsRoles,
            Self::Runtime,
            Self::Budgets,
            Self::Execution,
            Self::Verification,
            Self::ToolsLimits,
            Self::Workflow,
            Self::Git,
            Self::PromptsSkills,
            Self::Cache,
            Self::TuiInterface,
            Self::EnvOverrides,
            Self::EffectiveConfig,
            Self::About,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Provider => "Provider",
            Self::Models => "Models",
            Self::AgentsRoles => "Agents / Roles",
            Self::Runtime => "Runtime",
            Self::Budgets => "Budgets",
            Self::Execution => "Execution",
            Self::Verification => "Verification",
            Self::ToolsLimits => "Tools / Resource Limits",
            Self::Workflow => "Workflow",
            Self::Git => "Git",
            Self::PromptsSkills => "Prompts / Skills",
            Self::Cache => "Cache",
            Self::TuiInterface => "TUI / Interface",
            Self::EnvOverrides => "Environment / Overrides",
            Self::EffectiveConfig => "About / Effective Configuration",
            Self::About => "About",
        }
    }

    pub fn from_str_relaxed(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "general" => Some(Self::General),
            "provider" => Some(Self::Provider),
            "models" | "model" => Some(Self::Models),
            "agents" | "agents/roles" | "agents_roles" | "roles" => Some(Self::AgentsRoles),
            "runtime" => Some(Self::Runtime),
            "budgets" | "budget" => Some(Self::Budgets),
            "execution" => Some(Self::Execution),
            "verification" => Some(Self::Verification),
            "tools" | "tools/resource limits" | "tools_limits" => Some(Self::ToolsLimits),
            "workflow" => Some(Self::Workflow),
            "git" => Some(Self::Git),
            "prompts" | "skills" | "prompts/skills" => Some(Self::PromptsSkills),
            "cache" => Some(Self::Cache),
            "tui" | "tui/interface" | "interface" => Some(Self::TuiInterface),
            "env" | "environment" | "environment/overrides" | "overrides" => {
                Some(Self::EnvOverrides)
            }
            "effective" | "effective configuration" | "effectiveconfig" => {
                Some(Self::EffectiveConfig)
            }
            "about" => Some(Self::About),
            _ => None,
        }
    }
}

/// One settings row: configured value, effective value, source, and editability.
#[derive(Debug, Clone)]
pub struct SettingsRow {
    pub key: String,
    pub label: String,
    pub configured: String,
    pub effective: String,
    pub source: String,
    pub editable: bool,
    pub secret: bool,
    pub restart_required: bool,
    pub ceiling: Option<String>,
}

impl SettingsRow {
    pub fn text(
        key: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let v = value.into();
        Self {
            key: key.into(),
            label: label.into(),
            configured: v.clone(),
            effective: v,
            source: "default".to_string(),
            editable: true,
            secret: false,
            restart_required: false,
            ceiling: None,
        }
    }

    pub fn read_only(
        key: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let v = value.into();
        Self {
            key: key.into(),
            label: label.into(),
            configured: v.clone(),
            effective: v,
            source: "runtime".to_string(),
            editable: false,
            secret: false,
            restart_required: false,
            ceiling: None,
        }
    }
}

/// Editor state for the settings surface.
#[derive(Debug, Clone, Default)]
pub struct SettingsState {
    pub category_index: usize,
    pub row_index: usize,
    pub category_list: ListState,
    pub row_list: ListState,
    pub editing: bool,
    pub edit_buffer: String,
    pub message: Option<String>,
    pub confirm_save: bool,
}

impl SettingsState {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.category_list.select(Some(0));
        s.row_list.select(Some(0));
        s
    }

    pub fn category(&self) -> SettingsCategory {
        SettingsCategory::all()
            .get(self.category_index)
            .copied()
            .unwrap_or(SettingsCategory::General)
    }

    pub fn select_category(&mut self, cat: SettingsCategory) {
        if let Some(pos) = SettingsCategory::all().iter().position(|&c| c == cat) {
            self.category_index = pos;
            self.row_index = 0;
            self.category_list.select(Some(pos));
            self.row_list.select(Some(0));
        }
    }
}

/// Mask a secret value: show only configured/unconfigured, never the secret.
pub fn mask_secret_configured(present: bool) -> String {
    if present {
        "configured (hidden)".to_string()
    } else {
        "not configured".to_string()
    }
}

/// Build rows for a category from the canonical resolved configuration.
///
/// Provenance comes from `ResolvedConfiguration::explain` where available;
/// secrets are masked. No configuration logic is duplicated here.
pub fn rows_for_category(
    category: SettingsCategory,
    config: &crate::config::ResolvedConfiguration,
    catalog: Option<&crate::model::catalog::ModelCatalog>,
    env_present: &std::collections::HashMap<String, bool>,
) -> Vec<SettingsRow> {
    let app = &config.app_config;
    let provenance = |key: &str| -> String {
        config
            .explain(key)
            .map(|e| e.winning_tier.clone())
            .unwrap_or_else(|| "default".to_string())
    };
    match category {
        SettingsCategory::General => vec![
            SettingsRow {
                key: "profile".to_string(),
                label: "Active profile".to_string(),
                configured: config.active_profile.clone().unwrap_or_default(),
                effective: config.active_profile.clone().unwrap_or_default(),
                source: provenance("profile"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow::read_only(
                "workspace_root",
                "Workspace root",
                config.workspace_root.display().to_string(),
            ),
        ],
        SettingsCategory::Provider => {
            let (provider, model, _) = config.effective_model_selection();
            let endpoint = app
                .provider
                .nvidia_nim
                .as_ref()
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| crate::config::canonical::CANONICAL_NVIDIA_BASE_URL.to_string());
            vec![
                SettingsRow {
                    key: "provider.default".to_string(),
                    label: "Provider".to_string(),
                    configured: provider.clone(),
                    effective: provider,
                    source: provenance("provider.default"),
                    editable: true,
                    secret: false,
                    restart_required: true,
                    ceiling: None,
                },
                SettingsRow {
                    key: "provider.nvidia_nim.base_url".to_string(),
                    label: "Endpoint".to_string(),
                    configured: endpoint.clone(),
                    effective: endpoint,
                    source: format!("{:?}", config.provider_endpoint_source()),
                    editable: true,
                    secret: false,
                    restart_required: true,
                    ceiling: None,
                },
                SettingsRow {
                    key: "credential".to_string(),
                    label: "Authentication".to_string(),
                    configured: mask_secret_configured(
                        env_present.get("NVIDIA_API_KEY").copied().unwrap_or(false),
                    ),
                    effective: mask_secret_configured(
                        env_present.get("NVIDIA_API_KEY").copied().unwrap_or(false),
                    ),
                    source: "environment/file".to_string(),
                    editable: false,
                    secret: true,
                    restart_required: false,
                    ceiling: None,
                },
                SettingsRow {
                    key: "agents.default_model".to_string(),
                    label: "Current model".to_string(),
                    configured: model.clone(),
                    effective: model,
                    source: provenance("agents.default_model"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: None,
                },
            ]
        }
        SettingsCategory::Models => {
            let mut rows = vec![
                SettingsRow {
                    key: "agents.default_model".to_string(),
                    label: "Primary model".to_string(),
                    configured: app.agents.default_model.clone(),
                    effective: config.active_model.clone(),
                    source: provenance("agents.default_model"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: None,
                },
                SettingsRow {
                    key: "agents.fast_auxiliary_model".to_string(),
                    label: "Fast/auxiliary model".to_string(),
                    configured: app.agents.fast_auxiliary_model.clone().unwrap_or_default(),
                    effective: app.agents.fast_auxiliary_model.clone().unwrap_or_default(),
                    source: provenance("agents.fast_auxiliary_model"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: None,
                },
                SettingsRow {
                    key: "agents.max_tokens".to_string(),
                    label: "Max tokens".to_string(),
                    configured: app.agents.max_tokens.to_string(),
                    effective: app.agents.max_tokens.to_string(),
                    source: provenance("agents.max_tokens"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: None,
                },
            ];
            match catalog {
                Some(cat) if !cat.models.is_empty() => {
                    for m in cat.models.iter().take(20) {
                        rows.push(SettingsRow::read_only(
                            format!("catalog.{}", m.model_id),
                            format!("{} [{}]", m.model_id, m.availability),
                            format!(
                                "ctx={} tools={} tier={:?} src={}",
                                m.context_capacity, m.supports_tools, m.tier, m.source
                            ),
                        ));
                    }
                }
                _ => {
                    rows.push(SettingsRow::read_only(
                        "catalog.empty",
                        "Catalog state",
                        format!(
                            "empty/unavailable — configured model '{}' shown; run Refresh models after discovery",
                            config.active_model
                        ),
                    ));
                }
            }
            rows
        }
        SettingsCategory::AgentsRoles => vec![
            SettingsRow::read_only(
                "roles",
                "Registered roles",
                "see Agents screen (registry authority)".to_string(),
            ),
            SettingsRow {
                key: "agents.default_model".to_string(),
                label: "Role default model".to_string(),
                configured: app.agents.default_model.clone(),
                effective: config.active_model.clone(),
                source: provenance("agents.default_model"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "resources.per_role_concurrency".to_string(),
                label: "Per-role concurrency".to_string(),
                configured: app.resources.per_role_concurrency.to_string(),
                effective: app.resources.per_role_concurrency.to_string(),
                source: provenance("resources.per_role_concurrency"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: Some("32".to_string()),
            },
        ],
        SettingsCategory::Runtime => {
            let tp = config.timeout_policy();
            let rp = config.resource_policy();
            vec![
                SettingsRow {
                    key: "runtime.concurrency_limit".to_string(),
                    label: "Runtime concurrency".to_string(),
                    configured: app.runtime.concurrency_limit.to_string(),
                    effective: rp.runtime_concurrency.to_string(),
                    source: provenance("runtime.concurrency_limit"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("32".to_string()),
                },
                SettingsRow {
                    key: "runtime.timeout_secs".to_string(),
                    label: "Mission wall clock (s)".to_string(),
                    configured: app.runtime.timeout_secs.to_string(),
                    effective: tp.runtime_secs.to_string(),
                    source: provenance("runtime.timeout_secs"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("86400".to_string()),
                },
                SettingsRow {
                    key: "runtime.sandbox_mode".to_string(),
                    label: "Sandbox mode".to_string(),
                    configured: app.runtime.sandbox_mode.clone(),
                    effective: app.runtime.sandbox_mode.clone(),
                    source: provenance("runtime.sandbox_mode"),
                    editable: true,
                    secret: false,
                    restart_required: true,
                    ceiling: None,
                },
            ]
        }
        SettingsCategory::Budgets => vec![
            SettingsRow {
                key: "budget.max_agent_steps".to_string(),
                label: "Max agent steps".to_string(),
                configured: app
                    .budget
                    .max_agent_steps
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                effective: app
                    .budget
                    .max_agent_steps
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                source: provenance("budget.max_agent_steps"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "budget.max_model_calls".to_string(),
                label: "Max model calls".to_string(),
                configured: app
                    .budget
                    .max_model_calls
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                effective: app
                    .budget
                    .max_model_calls
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                source: provenance("budget.max_model_calls"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "budget.max_tokens".to_string(),
                label: "Max tokens".to_string(),
                configured: app
                    .budget
                    .max_tokens
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                effective: app
                    .budget
                    .max_tokens
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                source: provenance("budget.max_tokens"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "budget.max_cost_usd".to_string(),
                label: "Max cost (USD)".to_string(),
                configured: app
                    .budget
                    .max_cost_usd
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                effective: app
                    .budget
                    .max_cost_usd
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                source: provenance("budget.max_cost_usd"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "budget.max_wall_clock_seconds".to_string(),
                label: "Max wall clock (s)".to_string(),
                configured: app
                    .budget
                    .max_wall_clock_seconds
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                effective: app
                    .budget
                    .max_wall_clock_seconds
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                source: provenance("budget.max_wall_clock_seconds"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "budget.max_retries".to_string(),
                label: "Max retries".to_string(),
                configured: app
                    .budget
                    .max_retries
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                effective: app
                    .budget
                    .max_retries
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                source: provenance("budget.max_retries"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
        ],
        SettingsCategory::Execution => {
            let tp = config.timeout_policy();
            vec![
                SettingsRow {
                    key: "timeouts.process_secs".to_string(),
                    label: "Process/tool timeout (s)".to_string(),
                    configured: app.timeouts.process_secs.to_string(),
                    effective: tp.process_secs.to_string(),
                    source: provenance("timeouts.process_secs"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("86400".to_string()),
                },
                SettingsRow {
                    key: "timeouts.workflow_step_secs".to_string(),
                    label: "Workflow step timeout (s)".to_string(),
                    configured: app.timeouts.workflow_step_secs.to_string(),
                    effective: tp.workflow_step_secs.to_string(),
                    source: provenance("timeouts.workflow_step_secs"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("86400".to_string()),
                },
                SettingsRow {
                    key: "timeouts.verification_secs".to_string(),
                    label: "Verification timeout (s)".to_string(),
                    configured: app.timeouts.verification_secs.to_string(),
                    effective: tp.verification_secs.to_string(),
                    source: provenance("timeouts.verification_secs"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("86400".to_string()),
                },
                SettingsRow {
                    key: "timeouts.approval_secs".to_string(),
                    label: "Approval wait (s)".to_string(),
                    configured: app.timeouts.approval_secs.to_string(),
                    effective: tp.approval_secs.to_string(),
                    source: provenance("timeouts.approval_secs"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("86400".to_string()),
                },
            ]
        }
        SettingsCategory::Verification => vec![
            SettingsRow {
                key: "workspace.project_type".to_string(),
                label: "Project type override".to_string(),
                configured: app
                    .workspace
                    .project_type
                    .clone()
                    .unwrap_or_else(|| "auto-detect".to_string()),
                effective: app
                    .workspace
                    .project_type
                    .clone()
                    .unwrap_or_else(|| "auto-detect".to_string()),
                source: provenance("workspace.project_type"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "workspace.verification.tier3_tests".to_string(),
                label: "Test command".to_string(),
                configured: app
                    .workspace
                    .verification
                    .as_ref()
                    .and_then(|v| v.tier3_tests.clone())
                    .unwrap_or_else(|| "adapter default".to_string()),
                effective: app
                    .workspace
                    .verification
                    .as_ref()
                    .and_then(|v| v.tier3_tests.clone())
                    .unwrap_or_else(|| "adapter default".to_string()),
                source: provenance("workspace.verification"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "timeouts.verification_secs".to_string(),
                label: "Verification timeout (s)".to_string(),
                configured: app.timeouts.verification_secs.to_string(),
                effective: app.timeouts.verification_secs.to_string(),
                source: provenance("timeouts.verification_secs"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: Some("86400".to_string()),
            },
        ],
        SettingsCategory::ToolsLimits => {
            let rp = config.resource_policy();
            vec![
                SettingsRow {
                    key: "resources.tool_timeout_secs".to_string(),
                    label: "Tool timeout default (s)".to_string(),
                    configured: app.resources.tool_timeout_secs.to_string(),
                    effective: rp.tool_timeout_secs.to_string(),
                    source: provenance("resources.tool_timeout_secs"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("600".to_string()),
                },
                SettingsRow {
                    key: "resources.tool_max_output_bytes".to_string(),
                    label: "Tool max output (bytes)".to_string(),
                    configured: app.resources.tool_max_output_bytes.to_string(),
                    effective: rp.tool_max_output_bytes.to_string(),
                    source: provenance("resources.tool_max_output_bytes"),
                    editable: true,
                    secret: false,
                    restart_required: false,
                    ceiling: Some("10485760".to_string()),
                },
                SettingsRow::read_only(
                    "repo.query.ceilings",
                    "Repo query ceilings",
                    "results≤200 depth≤4 bytes≤65536 (immutable)",
                ),
            ]
        }
        SettingsCategory::Workflow => vec![
            SettingsRow {
                key: "workflow.research".to_string(),
                label: "Research enabled".to_string(),
                configured: app.workflow.research.to_string(),
                effective: app.workflow.research.to_string(),
                source: provenance("workflow.research"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "workflow.research_concurrency".to_string(),
                label: "Research concurrency".to_string(),
                configured: app.workflow.research_concurrency.to_string(),
                effective: app.workflow.research_concurrency.to_string(),
                source: provenance("workflow.research_concurrency"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: Some("8".to_string()),
            },
            SettingsRow {
                key: "workflow.auto_advance".to_string(),
                label: "Auto advance".to_string(),
                configured: app.workflow.auto_advance.to_string(),
                effective: app.workflow.auto_advance.to_string(),
                source: provenance("workflow.auto_advance"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
        ],
        SettingsCategory::Git => vec![
            SettingsRow {
                key: "git.enabled".to_string(),
                label: "Git enabled".to_string(),
                configured: app.git.enabled.to_string(),
                effective: app.git.enabled.to_string(),
                source: provenance("git.enabled"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "git.auto_commit".to_string(),
                label: "Auto commit".to_string(),
                configured: app.git.auto_commit.to_string(),
                effective: app.git.auto_commit.to_string(),
                source: provenance("git.auto_commit"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "git.push_policy".to_string(),
                label: "Push policy".to_string(),
                configured: app.git.push_policy.to_string(),
                effective: app.git.push_policy.to_string(),
                source: provenance("git.push_policy"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "git.branch_prefix".to_string(),
                label: "Branch prefix".to_string(),
                configured: app.git.branch_prefix.clone(),
                effective: app.git.branch_prefix.clone(),
                source: provenance("git.branch_prefix"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
        ],
        SettingsCategory::PromptsSkills => vec![
            SettingsRow::read_only(
                "prompts",
                "Installed prompts",
                "see /skills and prompt catalog (registry authority)",
            ),
            SettingsRow::read_only(
                "skills",
                "Installed skills",
                "see /skills (skill discovery authority)",
            ),
        ],
        SettingsCategory::Cache => vec![
            SettingsRow {
                key: "cache.catalog_freshness_secs".to_string(),
                label: "Catalog freshness (s)".to_string(),
                configured: app.cache.catalog_freshness_secs.to_string(),
                effective: app.cache.catalog_freshness_secs.to_string(),
                source: provenance("cache.catalog_freshness_secs"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "cache.metadata_ttl_secs".to_string(),
                label: "Metadata TTL (s)".to_string(),
                configured: app.cache.metadata_ttl_secs.to_string(),
                effective: app.cache.metadata_ttl_secs.to_string(),
                source: provenance("cache.metadata_ttl_secs"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
        ],
        SettingsCategory::TuiInterface => vec![
            SettingsRow {
                key: "tui.fps".to_string(),
                label: "FPS".to_string(),
                configured: app.tui.fps.to_string(),
                effective: app.tui.fps.to_string(),
                source: provenance("tui.fps"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: Some("120".to_string()),
            },
            SettingsRow {
                key: "tui.theme".to_string(),
                label: "Theme".to_string(),
                configured: app.tui.theme.clone(),
                effective: app.tui.theme.clone(),
                source: provenance("tui.theme"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
            SettingsRow {
                key: "tui.compact_mode".to_string(),
                label: "Compact mode".to_string(),
                configured: app.tui.compact_mode.to_string(),
                effective: app.tui.compact_mode.to_string(),
                source: provenance("tui.compact_mode"),
                editable: true,
                secret: false,
                restart_required: false,
                ceiling: None,
            },
        ],
        SettingsCategory::EnvOverrides => {
            let keys = [
                "M31A_MODEL",
                "M31A_CONCURRENCY",
                "M31A_TIMEOUT_SECS",
                "M31A_THEME",
                "M31A_MAX_STEPS",
                "NVIDIA_API_KEY",
            ];
            keys.iter()
                .map(|k| {
                    let present = env_present.get(*k).copied().unwrap_or(false);
                    let secret = k.contains("KEY");
                    SettingsRow {
                        key: format!("env.{k}"),
                        label: k.to_string(),
                        configured: if secret {
                            mask_secret_configured(present)
                        } else if present {
                            "set".to_string()
                        } else {
                            "not set".to_string()
                        },
                        effective: if secret {
                            mask_secret_configured(present)
                        } else if present {
                            "set".to_string()
                        } else {
                            "not set".to_string()
                        },
                        source: "Tier 5 environment".to_string(),
                        editable: false,
                        secret,
                        restart_required: false,
                        ceiling: None,
                    }
                })
                .collect()
        }
        SettingsCategory::EffectiveConfig => vec![
            SettingsRow::read_only(
                "effective.provider",
                "Provider",
                config.active_provider.clone(),
            ),
            SettingsRow::read_only(
                "effective.model",
                "Primary model",
                config.active_model.clone(),
            ),
            SettingsRow::read_only(
                "effective.profile",
                "Profile",
                config
                    .active_profile
                    .clone()
                    .unwrap_or_else(|| "(none)".to_string()),
            ),
            SettingsRow::read_only(
                "effective.workspace",
                "Workspace",
                config.workspace_root.display().to_string(),
            ),
        ],
        SettingsCategory::About => vec![
            SettingsRow::read_only(
                "about.version",
                "Version",
                env!("CARGO_PKG_VERSION").to_string(),
            ),
            SettingsRow::read_only(
                "about.authority",
                "Configuration authority",
                "ResolvedConfiguration (8-tier resolver)".to_string(),
            ),
        ],
    }
}

/// Handle a key event for the settings surface. Returns an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsAction {
    Close,
    EditStarted,
    EditCommitted { key: String, value: String },
    EditCancelled,
    SaveRequested,
    ResetToDefault { key: String },
    Noop,
}

pub fn handle_settings_key(
    key: crossterm::event::KeyEvent,
    state: &mut SettingsState,
    row_count: usize,
    editing_row: Option<&SettingsRow>,
) -> Option<SettingsAction> {
    use crossterm::event::KeyCode;
    if state.editing {
        match key.code {
            KeyCode::Enter => {
                state.editing = false;
                if let Some(row) = editing_row {
                    return Some(SettingsAction::EditCommitted {
                        key: row.key.clone(),
                        value: state.edit_buffer.clone(),
                    });
                }
                return Some(SettingsAction::Noop);
            }
            KeyCode::Esc => {
                state.editing = false;
                state.edit_buffer.clear();
                return Some(SettingsAction::EditCancelled);
            }
            KeyCode::Backspace => {
                state.edit_buffer.pop();
                return Some(SettingsAction::Noop);
            }
            KeyCode::Char(c) => {
                state.edit_buffer.push(c);
                return Some(SettingsAction::Noop);
            }
            _ => return Some(SettingsAction::Noop),
        }
    }
    match key.code {
        KeyCode::Esc => Some(SettingsAction::Close),
        KeyCode::Left | KeyCode::Char('h') => {
            if state.category_index > 0 {
                state.category_index -= 1;
                state.row_index = 0;
                state.row_list.select(Some(0));
                state.category_list.select(Some(state.category_index));
            }
            Some(SettingsAction::Noop)
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            if state.category_index + 1 < SettingsCategory::all().len() {
                state.category_index += 1;
                state.row_index = 0;
                state.row_list.select(Some(0));
                state.category_list.select(Some(state.category_index));
            }
            Some(SettingsAction::Noop)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.row_index > 0 {
                state.row_index -= 1;
                state.row_list.select(Some(state.row_index));
            }
            Some(SettingsAction::Noop)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if row_count > 0 && state.row_index + 1 < row_count {
                state.row_index += 1;
                state.row_list.select(Some(state.row_index));
            }
            Some(SettingsAction::Noop)
        }
        KeyCode::Enter | KeyCode::Char('e') => {
            if let Some(row) = editing_row
                && row.editable
            {
                state.editing = true;
                state.edit_buffer = row.configured.clone();
                return Some(SettingsAction::EditStarted);
            }
            Some(SettingsAction::Noop)
        }
        KeyCode::Char('s') => Some(SettingsAction::SaveRequested),
        KeyCode::Char('r') => {
            if let Some(row) = editing_row {
                return Some(SettingsAction::ResetToDefault {
                    key: row.key.clone(),
                });
            }
            Some(SettingsAction::Noop)
        }
        _ => Some(SettingsAction::Noop),
    }
}

/// Render the settings surface: category navigation + rows + help.
pub fn render_settings(
    f: &mut Frame,
    area: Rect,
    state: &mut SettingsState,
    rows: &[SettingsRow],
    tokens: &ThemeTokens,
    is_focused: bool,
) {
    if area.width < 30 || area.height < 10 {
        return;
    }
    let border_style = if is_focused {
        tokens.border_focused
    } else {
        tokens.border_default
    };
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(30)])
        .split(area);

    let cat_items: Vec<ListItem> = SettingsCategory::all()
        .iter()
        .map(|c| {
            let active = *c == state.category();
            let style = if active {
                tokens.text_primary.add_modifier(Modifier::BOLD)
            } else {
                tokens.text_muted
            };
            ListItem::new(Line::styled(
                format!("{} {}", if active { "▸" } else { " " }, c.title()),
                style,
            ))
        })
        .collect();
    let cat_list = List::new(cat_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Settings ")
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_stateful_widget(cat_list, chunks[0], &mut state.category_list);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(6)])
        .split(chunks[1]);

    if rows.is_empty() {
        let p = Paragraph::new("No settings in this category.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(state.category().title())
                    .border_style(border_style),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(p, right[0]);
    } else {
        let items: Vec<ListItem> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let active = i == state.row_index;
                let mut spans = vec![
                    Span::styled(
                        format!("{} ", if active { "▸" } else { " " }),
                        tokens.text_muted,
                    ),
                    Span::styled(&r.label, tokens.text_primary.add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(&r.effective, tokens.text_secondary),
                ];
                if r.secret {
                    spans.push(Span::styled("  [secret hidden]", tokens.status_warning));
                }
                if !r.editable {
                    spans.push(Span::styled("  [read-only]", tokens.text_muted));
                }
                if r.restart_required {
                    spans.push(Span::styled("  [restart-required]", tokens.status_warning));
                }
                if let Some(c) = &r.ceiling {
                    spans.push(Span::styled(format!("  [ceiling {c}]"), tokens.text_muted));
                }
                spans.push(Span::raw(format!("  (src: {})", r.source)));
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", state.category().title()))
                    .border_style(border_style),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        f.render_stateful_widget(list, right[0], &mut state.row_list);
    }

    let mut help = vec![Line::styled(
        "←/→ categories · ↑/↓ rows · Enter edit · Esc close/cancel · s save · r reset",
        tokens.text_muted,
    )];
    if state.editing {
        help.push(Line::from(vec![
            Span::styled(
                "Editing: ",
                tokens.accent_primary.add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.edit_buffer, tokens.text_primary),
            Span::styled("█", tokens.text_muted),
        ]));
    }
    if let Some(m) = &state.message {
        help.push(Line::styled(m.clone(), tokens.status_warning));
    }
    let p = Paragraph::new(help)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Keys ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, right[1]);
}

/// Apply a single edited key/value to an in-memory `AppConfig` draft.
///
/// Only keys the settings UI exposes are accepted; secrets and read-only
/// rows are rejected. Validation runs through the typed schema.
pub fn apply_edit_to_draft(
    draft: &mut crate::config::schema::AppConfig,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    let v = value.trim();
    match key {
        "profile" => {
            draft.profile = if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            };
            Ok(false)
        }
        "provider.default" => {
            let n = crate::config::provider_registry::normalize_provider_id(v);
            if n != crate::config::provider_registry::PRODUCTION_PROVIDER_ID {
                return Err(format!(
                    "unsupported provider '{v}'. {}",
                    crate::config::provider_registry::NVIDIA_ONLY_ERROR
                ));
            }
            draft.provider.default = n;
            Ok(true)
        }
        "provider.nvidia_nim.base_url" => {
            if v.is_empty() {
                draft.provider.nvidia_nim = None;
            } else {
                let mut tbl = draft.provider.nvidia_nim.clone().unwrap_or_default();
                tbl.base_url = Some(v.to_string());
                draft.provider.nvidia_nim = Some(tbl);
            }
            Ok(true)
        }
        "agents.default_model" | "agents.fast_auxiliary_model" | "role default model" => {
            if v.is_empty() {
                return Err("model id cannot be empty".to_string());
            }
            if key == "agents.fast_auxiliary_model" {
                draft.agents.fast_auxiliary_model = Some(v.to_string());
            } else {
                draft.agents.default_model = v.to_string();
            }
            Ok(false)
        }
        "agents.max_tokens" => {
            let n: u32 = v
                .parse()
                .map_err(|_| "max_tokens must be a number".to_string())?;
            draft.agents.max_tokens = n;
            Ok(false)
        }
        "runtime.concurrency_limit" => {
            draft.runtime.concurrency_limit = v
                .parse()
                .map_err(|_| "concurrency must be a number".to_string())?;
            Ok(false)
        }
        "runtime.timeout_secs" => {
            draft.runtime.timeout_secs = v
                .parse()
                .map_err(|_| "timeout must be a number".to_string())?;
            Ok(false)
        }
        "runtime.sandbox_mode" => {
            draft.runtime.sandbox_mode = v.to_string();
            Ok(true)
        }
        "resources.per_role_concurrency" => {
            draft.resources.per_role_concurrency =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "resources.tool_timeout_secs" => {
            draft.resources.tool_timeout_secs =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "resources.tool_max_output_bytes" => {
            draft.resources.tool_max_output_bytes =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "timeouts.process_secs" => {
            draft.timeouts.process_secs = v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "timeouts.workflow_step_secs" => {
            draft.timeouts.workflow_step_secs =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "timeouts.verification_secs" => {
            draft.timeouts.verification_secs =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "timeouts.approval_secs" => {
            draft.timeouts.approval_secs = v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "budget.max_agent_steps" => {
            draft.budget.max_agent_steps = if v.eq_ignore_ascii_case("unlimited") || v.is_empty() {
                None
            } else {
                Some(
                    v.parse()
                        .map_err(|_| "must be a number or unlimited".to_string())?,
                )
            };
            Ok(false)
        }
        "budget.max_model_calls" => {
            draft.budget.max_model_calls = if v.eq_ignore_ascii_case("unlimited") || v.is_empty() {
                None
            } else {
                Some(
                    v.parse()
                        .map_err(|_| "must be a number or unlimited".to_string())?,
                )
            };
            Ok(false)
        }
        "budget.max_tokens" => {
            draft.budget.max_tokens = if v.eq_ignore_ascii_case("unlimited") || v.is_empty() {
                None
            } else {
                Some(
                    v.parse()
                        .map_err(|_| "must be a number or unlimited".to_string())?,
                )
            };
            Ok(false)
        }
        "budget.max_cost_usd" => {
            draft.budget.max_cost_usd = if v.eq_ignore_ascii_case("unlimited") || v.is_empty() {
                None
            } else {
                Some(
                    v.parse()
                        .map_err(|_| "must be a number or unlimited".to_string())?,
                )
            };
            Ok(false)
        }
        "budget.max_wall_clock_seconds" => {
            draft.budget.max_wall_clock_seconds =
                if v.eq_ignore_ascii_case("unlimited") || v.is_empty() {
                    None
                } else {
                    Some(
                        v.parse()
                            .map_err(|_| "must be a number or unlimited".to_string())?,
                    )
                };
            Ok(false)
        }
        "budget.max_retries" => {
            draft.budget.max_retries = if v.eq_ignore_ascii_case("unlimited") || v.is_empty() {
                None
            } else {
                Some(
                    v.parse()
                        .map_err(|_| "must be a number or unlimited".to_string())?,
                )
            };
            Ok(false)
        }
        "workspace.project_type" => {
            draft.workspace.project_type = if v.eq_ignore_ascii_case("auto-detect") || v.is_empty()
            {
                None
            } else {
                Some(v.to_string())
            };
            Ok(false)
        }
        "workflow.research" => {
            draft.workflow.research = v.parse().map_err(|_| "must be true/false".to_string())?;
            Ok(false)
        }
        "workflow.research_concurrency" => {
            draft.workflow.research_concurrency =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "workflow.auto_advance" => {
            draft.workflow.auto_advance =
                v.parse().map_err(|_| "must be true/false".to_string())?;
            Ok(false)
        }
        "git.enabled" => {
            draft.git.enabled = v.parse().map_err(|_| "must be true/false".to_string())?;
            Ok(false)
        }
        "git.auto_commit" => {
            draft.git.auto_commit = v.parse().map_err(|_| "must be true/false".to_string())?;
            Ok(false)
        }
        "git.push_policy" => {
            draft.git.push_policy = v.parse().map_err(|e: String| e)?;
            Ok(false)
        }
        "git.branch_prefix" => {
            draft.git.branch_prefix = v.to_string();
            Ok(false)
        }
        "cache.catalog_freshness_secs" => {
            draft.cache.catalog_freshness_secs =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "cache.metadata_ttl_secs" => {
            draft.cache.metadata_ttl_secs =
                v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "tui.fps" => {
            draft.tui.fps = v.parse().map_err(|_| "must be a number".to_string())?;
            Ok(false)
        }
        "tui.theme" => {
            draft.tui.theme = v.to_string();
            Ok(false)
        }
        "tui.compact_mode" => {
            draft.tui.compact_mode = v.parse().map_err(|_| "must be true/false".to_string())?;
            Ok(false)
        }
        _ => Err(format!("setting '{key}' is read-only or unknown")),
    }
}

/// Atomically persist an `AppConfig` draft to the workspace config file.
///
/// Uses safe atomic replacement (temp file + rename) and validates through
/// the typed schema before writing.
pub fn persist_workspace_config(
    workspace_root: &std::path::Path,
    draft: &crate::config::schema::AppConfig,
) -> Result<std::path::PathBuf, String> {
    crate::config::schema::validate_config(draft).map_err(|e| format!("validation failed: {e}"))?;
    let path = crate::config::paths::PlatformPaths::workspace_config_file(workspace_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
    }
    let toml_str =
        toml::to_string_pretty(draft).map_err(|e| format!("serialization failed: {e}"))?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, toml_str).map_err(|e| format!("write failed: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("atomic replace failed: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_categories_cover_schema() {
        let titles: Vec<&str> = SettingsCategory::all().iter().map(|c| c.title()).collect();
        for expected in [
            "General",
            "Provider",
            "Models",
            "Agents / Roles",
            "Runtime",
            "Budgets",
            "Execution",
            "Verification",
            "Tools / Resource Limits",
            "Workflow",
            "Git",
            "Prompts / Skills",
            "Cache",
            "TUI / Interface",
            "Environment / Overrides",
            "About / Effective Configuration",
        ] {
            assert!(titles.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn empty_catalog_renders_explicit_state() {
        let ws = tempfile::tempdir().unwrap();
        let cfg = crate::config::ResolvedConfiguration::builder(ws.path()).build_fallback();
        let rows = rows_for_category(
            SettingsCategory::Models,
            &cfg,
            None,
            &std::collections::HashMap::new(),
        );
        assert!(rows.iter().any(|r| r.key == "catalog.empty"));
        // No fabricated inventory: empty catalog yields zero model rows plus
        // the explicit empty-state row.
        assert!(!rows.iter().any(|r| r.key.starts_with("catalog.meta/")));
    }

    #[test]
    fn secret_values_are_masked() {
        assert_eq!(mask_secret_configured(true), "configured (hidden)");
        assert_eq!(mask_secret_configured(false), "not configured");
    }
}
