//! Feature Flag System
//!
//! Structured feature flag management with staged rollouts, targeting,
//! and audit trails for safe feature deployment.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Feature flag definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: FeatureCategory,
    pub state: FeatureState,
    pub rollout: RolloutConfig,
    pub targeting: TargetingRules,
    pub dependencies: Vec<String>,
    pub metadata: FeatureMetadata,
    pub created_at: u64,
    pub updated_at: u64,
    pub created_by: String,
}

/// Feature categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureCategory {
    Experimental,
    Beta,
    Stable,
    Deprecated,
    Internal,
}

/// Feature state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureState {
    Enabled,
    Disabled,
    Archived,
}

/// Rollout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutConfig {
    pub strategy: RolloutStrategy,
    pub percentage: u8, // 0-100
    pub stages: Vec<RolloutStage>,
    pub current_stage: usize,
}

/// Rollout strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RolloutStrategy {
    Instant,
    Gradual,
    Canary,
    Ring,
    Manual,
}

/// Rollout stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutStage {
    pub name: String,
    pub percentage: u8,
    pub duration_hours: u32,
    pub success_criteria: Vec<String>,
    pub rollback_criteria: Vec<String>,
}

/// Targeting rules for feature flags
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetingRules {
    pub user_ids: Vec<String>,
    pub groups: Vec<String>,
    pub environments: Vec<String>,
    pub custom_attributes: HashMap<String, Vec<String>>,
    pub exclude: Vec<String>,
}

/// Feature metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    pub jira_ticket: Option<String>,
    pub github_pr: Option<String>,
    pub owner_team: String,
    pub documentation_url: Option<String>,
    pub migration_guide_url: Option<String>,
    pub sunset_date: Option<u64>,
}

/// Feature flag evaluation context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub user_id: Option<String>,
    pub groups: Vec<String>,
    pub environment: String,
    pub custom_attributes: HashMap<String, String>,
    pub session_id: Option<String>,
}

/// Feature flag evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub flag_id: String,
    pub enabled: bool,
    pub variant: Option<String>,
    pub reason: EvaluationReason,
    pub timestamp: u64,
}

/// Evaluation reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvaluationReason {
    DefaultEnabled,
    DefaultDisabled,
    TargetedEnabled,
    TargetedDisabled,
    RolloutEnabled,
    RolloutDisabled,
    DependencyDisabled,
    Archived,
    Override,
}

/// Partial updates for a feature flag; `None` fields are left unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlagUpdates {
    pub state: Option<FeatureState>,
    pub rollout: Option<RolloutConfig>,
    pub targeting: Option<TargetingRules>,
    pub description: Option<String>,
    pub category: Option<FeatureCategory>,
    pub dependencies: Option<Vec<String>>,
    pub metadata: Option<FeatureMetadata>,
}

/// Feature flag audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub flag_id: String,
    pub action: AuditAction,
    pub actor: String,
    pub old_state: Option<FeatureState>,
    pub new_state: Option<FeatureState>,
    pub old_rollout: Option<RolloutConfig>,
    pub new_rollout: Option<RolloutConfig>,
    pub reason: String,
    pub timestamp: u64,
}

/// Audit actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditAction {
    Created,
    Updated,
    Enabled,
    Disabled,
    RolloutAdvanced,
    RolloutRolledBack,
    Archived,
    OverrideSet,
    OverrideCleared,
}

/// Feature flag manager
pub struct FeatureFlagManager {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
    overrides: Arc<RwLock<HashMap<String, FeatureState>>>, // user_id -> state
    storage_path: PathBuf,
}

impl FeatureFlagManager {
    pub fn new(storage_path: PathBuf) -> Result<Self> {
        let manager = Self {
            flags: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            overrides: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
        };
        manager.register_builtin_flags();
        manager.load_from_storage()?;
        Ok(manager)
    }

    /// Register built-in feature flags
    fn register_builtin_flags(&self) {
        let flags = vec![
            FeatureFlag {
                id: "code_mode".to_string(),
                name: "Code Mode".to_string(),
                description: "Enable Code Mode for interactive code execution".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Gradual,
                    percentage: 0,
                    stages: vec![
                        RolloutStage { name: "Internal".to_string(), percentage: 5, duration_hours: 24, success_criteria: vec!["No critical bugs".to_string()], rollback_criteria: vec!["Crash rate > 1%".to_string()] },
                        RolloutStage { name: "Beta".to_string(), percentage: 25, duration_hours: 72, success_criteria: vec!["Positive feedback > 80%".to_string()], rollback_criteria: vec!["Error rate > 5%".to_string()] },
                        RolloutStage { name: "General".to_string(), percentage: 100, duration_hours: 168, success_criteria: vec!["Stable metrics".to_string()], rollback_criteria: vec!["Critical issues".to_string()] },
                    ],
                    current_stage: 0,
                },
                targeting: TargetingRules {
                    user_ids: vec![],
                    groups: vec!["internal".to_string(), "beta-testers".to_string()],
                    environments: vec!["development".to_string(), "staging".to_string()],
                    custom_attributes: HashMap::new(),
                    exclude: vec![],
                },
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-100".to_string()),
                    github_pr: Some("pr/123".to_string()),
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/code_mode".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "voice_input".to_string(),
                name: "Voice Input".to_string(),
                description: "Enable voice-to-text input for prompts".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![
                        RolloutStage { name: "Canary".to_string(), percentage: 1, duration_hours: 48, success_criteria: vec!["Accuracy > 90%".to_string()], rollback_criteria: vec!["Crash rate > 0.5%".to_string()] },
                        RolloutStage { name: "Beta".to_string(), percentage: 10, duration_hours: 168, success_criteria: vec!["User satisfaction > 4/5".to_string()], rollback_criteria: vec!["Latency > 5s".to_string()] },
                        RolloutStage { name: "General".to_string(), percentage: 100, duration_hours: 336, success_criteria: vec!["Stable".to_string()], rollback_criteria: vec!["Critical issues".to_string()] },
                    ],
                    current_stage: 0,
                },
                targeting: TargetingRules {
                    user_ids: vec![],
                    groups: vec!["accessibility".to_string()],
                    environments: vec!["development".to_string()],
                    custom_attributes: HashMap::new(),
                    exclude: vec![],
                },
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-200".to_string()),
                    github_pr: None,
                    owner_team: "accessibility".to_string(),
                    documentation_url: None,
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "web_search".to_string(),
                name: "Web Search".to_string(),
                description: "Enable web search tool for agents".to_string(),
                category: FeatureCategory::Beta,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Gradual,
                    percentage: 50,
                    stages: vec![
                        RolloutStage { name: "Beta".to_string(), percentage: 25, duration_hours: 72, success_criteria: vec!["Relevant results > 80%".to_string()], rollback_criteria: vec!["API quota exceeded".to_string()] },
                        RolloutStage { name: "General".to_string(), percentage: 100, duration_hours: 168, success_criteria: vec!["Stable".to_string()], rollback_criteria: vec!["Cost overrun".to_string()] },
                    ],
                    current_stage: 1,
                },
                targeting: TargetingRules {
                    user_ids: vec![],
                    groups: vec!["researchers".to_string(), "developers".to_string()],
                    environments: vec!["development".to_string(), "staging".to_string(), "production".to_string()],
                    custom_attributes: HashMap::new(),
                    exclude: vec![],
                },
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-300".to_string()),
                    github_pr: Some("pr/456".to_string()),
                    owner_team: "tools".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/web_search".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            // Core execution and security feature flags
            FeatureFlag {
                id: "unified_exec".to_string(),
                name: "Unified Exec".to_string(),
                description: "Enable unified execution engine with PTY sessions and background process management".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules {
                    user_ids: vec![],
                    groups: vec![],
                    environments: vec!["development".to_string(), "staging".to_string(), "production".to_string()],
                    custom_attributes: HashMap::new(),
                    exclude: vec![],
                },
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-400".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/unified_exec".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "write_stdin_approval".to_string(),
                name: "Write Stdin Approval".to_string(),
                description: "Require approval for stdin writes to background processes".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["unified_exec".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-401".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/write_stdin_approval".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "exec_permission_approvals".to_string(),
                name: "Exec Permission Approvals".to_string(),
                description: "Enable granular permission approvals for exec commands (sandbox, rules, skills, MCP elicitations)".to_string(),
                category: FeatureCategory::Beta,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Gradual,
                    percentage: 50,
                    stages: vec![
                        RolloutStage { name: "Beta".to_string(), percentage: 25, duration_hours: 72, success_criteria: vec!["No security incidents".to_string()], rollback_criteria: vec!["User complaints > 10%".to_string()] },
                        RolloutStage { name: "General".to_string(), percentage: 100, duration_hours: 168, success_criteria: vec!["Stable".to_string()], rollback_criteria: vec!["Critical issues".to_string()] },
                    ],
                    current_stage: 1,
                },
                targeting: TargetingRules {
                    user_ids: vec![],
                    groups: vec!["security-team".to_string(), "admins".to_string()],
                    environments: vec!["development".to_string(), "staging".to_string(), "production".to_string()],
                    custom_attributes: HashMap::new(),
                    exclude: vec![],
                },
                dependencies: vec!["unified_exec".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-402".to_string()),
                    github_pr: None,
                    owner_team: "security".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/exec_permission_approvals".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "code_mode_only".to_string(),
                name: "Code Mode Only".to_string(),
                description: "Restrict tool surface to only code_mode exec/wait tools".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![
                        RolloutStage { name: "Canary".to_string(), percentage: 1, duration_hours: 48, success_criteria: vec!["No regressions".to_string()], rollback_criteria: vec!["User complaints".to_string()] },
                    ],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["code_mode".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-403".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/code_mode_only".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "code_mode_only_strict_3p_tools".to_string(),
                name: "Code Mode Strict 3P Tools".to_string(),
                description: "Enforce strict third-party tool restrictions in Code Mode".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["code_mode_only".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-404".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/code_mode_strict_3p".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "code_mode_tool_search".to_string(),
                name: "Code Mode Tool Search".to_string(),
                description: "Enable tool_search in Code Mode environment".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["code_mode".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-405".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/code_mode_tool_search".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "standalone_web_search".to_string(),
                name: "Standalone Web Search".to_string(),
                description: "Enable standalone web.run extension tool (requires external web search service)".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-406".to_string()),
                    github_pr: None,
                    owner_team: "tools".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/standalone_web_search".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "deferred_executor".to_string(),
                name: "Deferred Executor".to_string(),
                description: "Enable wait_for_environment tool for deferred execution".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-407".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/deferred_executor".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "token_budget".to_string(),
                name: "Token Budget".to_string(),
                description: "Enable token budget tracking with get_context_remaining and new_context_window tools".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-408".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/token_budget".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "current_time_reminder".to_string(),
                name: "Current Time Reminder".to_string(),
                description: "Enable current_time/curr_time tools for model time awareness".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-409".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/current_time_reminder".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "multi_agent_v2".to_string(),
                name: "Multi-Agent V2".to_string(),
                description: "Enable V2 multi-agent tools (spawn_agent, send_message, followup_task, wait_agent, interrupt_agent, list_agents)".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["unified_exec".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-410".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/multi_agent_v2".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "agent_message_board".to_string(),
                name: "Agent Message Board".to_string(),
                description: "Enable inter-agent communication via message board channels".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["multi_agent_v2".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-411".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/agent_message_board".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "image_generation".to_string(),
                name: "Image Generation".to_string(),
                description: "Enable imagegen tool for AI image generation".to_string(),
                category: FeatureCategory::Beta,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Gradual,
                    percentage: 25,
                    stages: vec![
                        RolloutStage { name: "Beta".to_string(), percentage: 25, duration_hours: 72, success_criteria: vec!["Quality acceptable".to_string()], rollback_criteria: vec!["API quota".to_string()] },
                        RolloutStage { name: "General".to_string(), percentage: 100, duration_hours: 168, success_criteria: vec!["Stable".to_string()], rollback_criteria: vec!["Cost overrun".to_string()] },
                    ],
                    current_stage: 1,
                },
                targeting: TargetingRules {
                    user_ids: vec![],
                    groups: vec!["designers".to_string(), "developers".to_string()],
                    environments: vec!["development".to_string(), "staging".to_string(), "production".to_string()],
                    custom_attributes: HashMap::new(),
                    exclude: vec![],
                },
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-412".to_string()),
                    github_pr: None,
                    owner_team: "tools".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/image_generation".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "unified_image_budget".to_string(),
                name: "Unified Image Budget".to_string(),
                description: "Enable unified image budget for image generation and viewing".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["image_generation".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-413".to_string()),
                    github_pr: None,
                    owner_team: "tools".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/unified_image_budget".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "apply_patch_streaming_events".to_string(),
                name: "Apply Patch Streaming Events".to_string(),
                description: "Emit streaming events during patch application (FileStarted, HunkApplied, FileCompleted)".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-414".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/apply_patch_streaming".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "guardian_native_multi_agent_history".to_string(),
                name: "Guardian Native Multi-Agent History".to_string(),
                description: "Enable Guardian review for native multi-agent tool calls".to_string(),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Canary,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec!["multi_agent_v2".to_string(), "guardian_v2".to_string()],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-415".to_string()),
                    github_pr: None,
                    owner_team: "security".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/guardian_native_multi_agent".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "deferred_tool_world_state".to_string(),
                name: "Deferred Tool World State".to_string(),
                description: "Enable deferred tool loading (MCP tools hidden until tool_search)".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-416".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/deferred_tool_world_state".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
            FeatureFlag {
                id: "non_prefixed_mcp_tool_names".to_string(),
                name: "Non-Prefixed MCP Tool Names".to_string(),
                description: "Disable mcp__ prefix for MCP tool names (use server__tool format)".to_string(),
                category: FeatureCategory::Stable,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: Some("M31A-417".to_string()),
                    github_pr: None,
                    owner_team: "core".to_string(),
                    documentation_url: Some("https://docs.m31a.dev/features/non_prefixed_mcp_tool_names".to_string()),
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                created_by: "system".to_string(),
            },
        ];

        for flag in flags {
            self.flags
                .try_write()
                .expect("uncontended")
                .insert(flag.id.clone(), flag);
        }
    }

    /// Load flags from storage
    fn load_from_storage(&self) -> Result<()> {
        let path = self.storage_path.join("feature_flags.json");
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read feature flags file: {}", path.display()))?;
        let stored_flags: HashMap<String, FeatureFlag> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse feature flags JSON: {}", path.display()))?;
        {
            let mut flags = self.flags.try_write().expect("uncontended");
            for (id, flag) in stored_flags {
                flags.insert(id, flag);
            }
        }

        let audit_path = self.storage_path.join("feature_flags_audit.json");
        if audit_path.exists() {
            let content = std::fs::read_to_string(&audit_path).with_context(|| {
                format!("Failed to read audit log file: {}", audit_path.display())
            })?;
            let audit: Vec<AuditEntry> = serde_json::from_str(&content).with_context(|| {
                format!("Failed to parse audit log JSON: {}", audit_path.display())
            })?;
            *self.audit_log.try_write().expect("uncontended") = audit;
        }
        Ok(())
    }

    /// Enable a feature flag by ID and persist to storage
    pub async fn enable_flag(&self, flag_id: &str, actor: String) -> Result<()> {
        if self.get_flag(flag_id).await.is_none() {
            let flag = FeatureFlag {
                id: flag_id.to_string(),
                name: flag_id.to_string(),
                description: format!("Feature flag {}", flag_id),
                category: FeatureCategory::Experimental,
                state: FeatureState::Enabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 100,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: None,
                    github_pr: None,
                    owner_team: "custom".to_string(),
                    documentation_url: None,
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                updated_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                created_by: actor.clone(),
            };
            self.create_flag(flag, actor).await
        } else {
            self.update_flag(
                flag_id,
                FlagUpdates {
                    state: Some(FeatureState::Enabled),
                    ..Default::default()
                },
                actor,
            )
            .await
        }
    }

    /// Disable a feature flag by ID and persist to storage
    pub async fn disable_flag(&self, flag_id: &str, actor: String) -> Result<()> {
        if self.get_flag(flag_id).await.is_none() {
            let flag = FeatureFlag {
                id: flag_id.to_string(),
                name: flag_id.to_string(),
                description: format!("Feature flag {}", flag_id),
                category: FeatureCategory::Experimental,
                state: FeatureState::Disabled,
                rollout: RolloutConfig {
                    strategy: RolloutStrategy::Instant,
                    percentage: 0,
                    stages: vec![],
                    current_stage: 0,
                },
                targeting: TargetingRules::default(),
                dependencies: vec![],
                metadata: FeatureMetadata {
                    jira_ticket: None,
                    github_pr: None,
                    owner_team: "custom".to_string(),
                    documentation_url: None,
                    migration_guide_url: None,
                    sunset_date: None,
                },
                created_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                updated_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                created_by: actor.clone(),
            };
            self.create_flag(flag, actor).await
        } else {
            self.update_flag(
                flag_id,
                FlagUpdates {
                    state: Some(FeatureState::Disabled),
                    ..Default::default()
                },
                actor,
            )
            .await
        }
    }

    /// Save flags to storage atomically
    async fn save_to_storage(&self) -> Result<()> {
        std::fs::create_dir_all(&self.storage_path)?;
        let path = self.storage_path.join("feature_flags.json");
        let flags = self.flags.read().await.clone();
        let content = serde_json::to_string_pretty(&flags)?;
        let tmp_path = self
            .storage_path
            .join(format!("feature_flags.{}.tmp", Uuid::new_v4()));
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, path)?;

        let audit_path = self.storage_path.join("feature_flags_audit.json");
        let audit = self.audit_log.read().await.clone();
        let audit_content = serde_json::to_string_pretty(&audit)?;
        let tmp_audit_path = self
            .storage_path
            .join(format!("feature_flags_audit.{}.tmp", Uuid::new_v4()));
        std::fs::write(&tmp_audit_path, audit_content)?;
        std::fs::rename(&tmp_audit_path, audit_path)?;
        Ok(())
    }

    /// Create a new feature flag
    pub async fn create_flag(&self, mut flag: FeatureFlag, actor: String) -> Result<()> {
        flag.id = flag.id.to_lowercase().replace(' ', "-");
        flag.created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        flag.updated_at = flag.created_at;
        flag.created_by = actor.clone();

        // Validate dependencies exist
        for dep in &flag.dependencies {
            if !self.flags.read().await.contains_key(dep) {
                return Err(anyhow::anyhow!("Dependency '{}' does not exist", dep));
            }
        }

        self.flags
            .write()
            .await
            .insert(flag.id.clone(), flag.clone());
        self.log_audit(AuditEntry {
            id: Uuid::new_v4().to_string(),
            flag_id: flag.id.clone(),
            action: AuditAction::Created,
            actor,
            old_state: None,
            new_state: Some(flag.state),
            old_rollout: None,
            new_rollout: Some(flag.rollout.clone()),
            reason: "Feature flag created".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
        .await;

        self.save_to_storage().await?;
        Ok(())
    }

    /// Update a feature flag
    pub async fn update_flag(
        &self,
        flag_id: &str,
        updates: FlagUpdates,
        actor: String,
    ) -> Result<()> {
        // Validate dependencies before taking the write lock (avoids
        // re-entering `self.flags` while it is held for write).
        if let Some(dependencies) = &updates.dependencies {
            let flags = self.flags.read().await;
            for dep in dependencies {
                if !flags.contains_key(dep) {
                    return Err(anyhow::anyhow!("Dependency '{}' does not exist", dep));
                }
            }
        }

        let mut flags = self.flags.write().await;
        let flag = flags.get_mut(flag_id).context("Flag not found")?;

        let old_state = flag.state;
        let old_rollout = flag.rollout.clone();
        // Snapshot what the caller changed before `updates` is partially moved.
        let state_changed = updates.state.is_some() && updates.state != Some(old_state);
        let rollout_changed = updates.rollout.is_some();

        if let Some(state) = updates.state {
            flag.state = state;
        }
        if let Some(rollout) = updates.rollout {
            flag.rollout = rollout;
        }
        if let Some(targeting) = updates.targeting {
            flag.targeting = targeting;
        }
        if let Some(description) = updates.description {
            flag.description = description;
        }
        if let Some(category) = updates.category {
            flag.category = category;
        }
        if let Some(dependencies) = updates.dependencies {
            flag.dependencies = dependencies;
        }
        if let Some(metadata) = updates.metadata {
            flag.metadata = metadata;
        }

        flag.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.log_audit(AuditEntry {
            id: Uuid::new_v4().to_string(),
            flag_id: flag_id.to_string(),
            action: if state_changed {
                if updates.state == Some(FeatureState::Enabled) {
                    AuditAction::Enabled
                } else {
                    AuditAction::Disabled
                }
            } else if rollout_changed {
                AuditAction::RolloutAdvanced
            } else {
                AuditAction::Updated
            },
            actor,
            old_state: Some(old_state),
            new_state: Some(flag.state),
            old_rollout: Some(old_rollout),
            new_rollout: Some(flag.rollout.clone()),
            reason: "Feature flag updated".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
        .await;

        drop(flags);
        self.save_to_storage().await?;
        Ok(())
    }

    /// Advance rollout to next stage
    pub async fn advance_rollout(&self, flag_id: &str, actor: String) -> Result<()> {
        let mut flags = self.flags.write().await;
        let flag = flags.get_mut(flag_id).context("Flag not found")?;

        if flag.rollout.current_stage + 1 >= flag.rollout.stages.len() {
            return Err(anyhow::anyhow!("Already at final rollout stage"));
        }

        let old_stage = flag.rollout.current_stage;
        flag.rollout.current_stage += 1;
        flag.rollout.percentage = flag.rollout.stages[flag.rollout.current_stage].percentage;
        flag.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.log_audit(AuditEntry {
            id: Uuid::new_v4().to_string(),
            flag_id: flag_id.to_string(),
            action: AuditAction::RolloutAdvanced,
            actor,
            old_state: None,
            new_state: None,
            old_rollout: None,
            new_rollout: Some(flag.rollout.clone()),
            reason: format!(
                "Rollout advanced from stage {} to {}",
                old_stage, flag.rollout.current_stage
            ),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
        .await;

        drop(flags);
        self.save_to_storage().await?;
        Ok(())
    }

    /// Rollback rollout to previous stage
    pub async fn rollback_rollout(&self, flag_id: &str, actor: String) -> Result<()> {
        let mut flags = self.flags.write().await;
        let flag = flags.get_mut(flag_id).context("Flag not found")?;

        if flag.rollout.current_stage == 0 {
            return Err(anyhow::anyhow!("Already at initial rollout stage"));
        }

        let old_stage = flag.rollout.current_stage;
        flag.rollout.current_stage -= 1;
        flag.rollout.percentage = flag.rollout.stages[flag.rollout.current_stage].percentage;
        flag.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.log_audit(AuditEntry {
            id: Uuid::new_v4().to_string(),
            flag_id: flag_id.to_string(),
            action: AuditAction::RolloutRolledBack,
            actor,
            old_state: None,
            new_state: None,
            old_rollout: None,
            new_rollout: Some(flag.rollout.clone()),
            reason: format!(
                "Rollback from stage {} to {}",
                old_stage, flag.rollout.current_stage
            ),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
        .await;

        drop(flags);
        self.save_to_storage().await?;
        Ok(())
    }

    /// Evaluate a feature flag for a context
    pub async fn evaluate(
        &self,
        flag_id: &str,
        context: &EvaluationContext,
    ) -> Result<EvaluationResult> {
        let flags = self.flags.read().await;
        let flag = flags.get(flag_id).context("Flag not found")?;

        // Check override first
        let override_key = format!(
            "{}:{}",
            flag_id,
            context.user_id.as_deref().unwrap_or("global")
        );
        if let Some(override_state) = self.overrides.read().await.get(&override_key) {
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled: *override_state == FeatureState::Enabled,
                variant: None,
                reason: EvaluationReason::Override,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check if archived
        if flag.state == FeatureState::Archived {
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled: false,
                variant: None,
                reason: EvaluationReason::Archived,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check if disabled
        if flag.state == FeatureState::Disabled {
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled: false,
                variant: None,
                reason: EvaluationReason::DefaultDisabled,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check dependencies
        for dep in &flag.dependencies {
            let flags_guard = self.flags.read().await;
            match flags_guard.get(dep) {
                Some(dep_flag) => {
                    if dep_flag.state == FeatureState::Archived {
                        return Ok(EvaluationResult {
                            flag_id: flag_id.to_string(),
                            enabled: false,
                            variant: None,
                            reason: EvaluationReason::Archived,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                    }
                    if dep_flag.state != FeatureState::Enabled {
                        return Ok(EvaluationResult {
                            flag_id: flag_id.to_string(),
                            enabled: false,
                            variant: None,
                            reason: EvaluationReason::DependencyDisabled,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                    }
                }
                None => {
                    return Ok(EvaluationResult {
                        flag_id: flag_id.to_string(),
                        enabled: false,
                        variant: None,
                        reason: EvaluationReason::DependencyDisabled,
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    });
                }
            }
        }

        // Check targeting exclusions
        if let Some(user_id) = &context.user_id
            && flag.targeting.exclude.contains(user_id)
        {
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled: false,
                variant: None,
                reason: EvaluationReason::TargetedDisabled,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check environment targeting
        if !flag.targeting.environments.is_empty()
            && !flag.targeting.environments.contains(&context.environment)
        {
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled: false,
                variant: None,
                reason: EvaluationReason::TargetedDisabled,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check custom attributes requirements
        let custom_attrs_match = if flag.targeting.custom_attributes.is_empty() {
            true
        } else {
            flag.targeting
                .custom_attributes
                .iter()
                .all(|(k, allowed_vals)| {
                    if let Some(actual) = context.custom_attributes.get(k) {
                        allowed_vals.contains(actual)
                    } else {
                        false
                    }
                })
        };

        if !flag.targeting.custom_attributes.is_empty() && !custom_attrs_match {
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled: false,
                variant: None,
                reason: EvaluationReason::TargetedDisabled,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check user/group targeting
        let is_targeted = if let Some(user_id) = &context.user_id {
            flag.targeting.user_ids.contains(user_id)
        } else {
            false
        } || context
            .groups
            .iter()
            .any(|g| flag.targeting.groups.contains(g));

        if is_targeted {
            // Check rollout percentage
            let enabled = self.check_rollout(flag, context)?;
            return Ok(EvaluationResult {
                flag_id: flag_id.to_string(),
                enabled,
                variant: None,
                reason: if enabled {
                    EvaluationReason::TargetedEnabled
                } else {
                    EvaluationReason::TargetedDisabled
                },
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }

        // Check rollout percentage for non-targeted users
        let enabled = self.check_rollout(flag, context)?;
        Ok(EvaluationResult {
            flag_id: flag_id.to_string(),
            enabled,
            variant: None,
            reason: if enabled {
                EvaluationReason::RolloutEnabled
            } else {
                EvaluationReason::RolloutDisabled
            },
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Check if user is in rollout percentage
    fn check_rollout(&self, flag: &FeatureFlag, context: &EvaluationContext) -> Result<bool> {
        let percentage = match flag.rollout.strategy {
            RolloutStrategy::Instant => flag.rollout.percentage,
            RolloutStrategy::Manual => flag.rollout.percentage,
            RolloutStrategy::Gradual | RolloutStrategy::Canary | RolloutStrategy::Ring => {
                if flag.rollout.stages.is_empty() {
                    flag.rollout.percentage
                } else {
                    let stage_idx = flag
                        .rollout
                        .current_stage
                        .min(flag.rollout.stages.len() - 1);
                    flag.rollout.stages[stage_idx].percentage
                }
            }
        };

        if percentage >= 100 {
            return Ok(true);
        }
        if percentage == 0 {
            return Ok(false);
        }

        // Deterministic assignment based on user_id, session_id, or anonymous
        let assign_key = context
            .user_id
            .as_deref()
            .or(context.session_id.as_deref())
            .unwrap_or("anonymous");
        let hash_input = format!("{}:{}", flag.id, assign_key);
        let hash = self.hash_string(&hash_input);
        let bucket = (hash % 100) as u8;

        Ok(bucket < percentage)
    }

    fn hash_string(&self, input: &str) -> u64 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        u64::from_be_bytes([
            result[0], result[1], result[2], result[3], result[4], result[5], result[6], result[7],
        ])
    }

    /// Get a flag
    pub async fn get_flag(&self, flag_id: &str) -> Option<FeatureFlag> {
        self.flags.read().await.get(flag_id).cloned()
    }

    /// List all flags
    pub async fn list_flags(&self) -> Vec<FeatureFlag> {
        self.flags.read().await.values().cloned().collect()
    }

    /// List flags by category
    pub async fn list_by_category(&self, category: FeatureCategory) -> Vec<FeatureFlag> {
        self.flags
            .read()
            .await
            .values()
            .filter(|f| f.category == category)
            .cloned()
            .collect()
    }

    /// Set user override
    pub async fn set_override(
        &self,
        flag_id: &str,
        user_id: &str,
        state: FeatureState,
    ) -> Result<()> {
        let key = format!("{}:{}", flag_id, user_id);
        self.overrides.write().await.insert(key, state);
        Ok(())
    }

    /// Clear user override
    pub async fn clear_override(&self, flag_id: &str, user_id: &str) -> Result<()> {
        let key = format!("{}:{}", flag_id, user_id);
        self.overrides.write().await.remove(&key);
        Ok(())
    }

    /// Get audit log
    pub async fn get_audit_log(&self, flag_id: Option<&str>, limit: usize) -> Vec<AuditEntry> {
        let audit = self.audit_log.read().await;
        let mut entries: Vec<AuditEntry> = if let Some(flag_id) = flag_id {
            audit
                .iter()
                .filter(|e| e.flag_id == flag_id)
                .cloned()
                .collect()
        } else {
            audit.clone()
        };
        entries.sort_by_key(|a| std::cmp::Reverse(a.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Log audit entry
    async fn log_audit(&self, entry: AuditEntry) {
        self.audit_log.write().await.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feature_flag_evaluation() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FeatureFlagManager::new(temp.path().to_path_buf()).unwrap();

        // Test builtin flags exist
        let flags = manager.list_flags().await;
        assert!(flags.iter().any(|f| f.id == "code_mode"));
        assert!(flags.iter().any(|f| f.id == "web_search"));
        assert!(flags.iter().any(|f| f.id == "voice_input"));
    }

    #[tokio::test]
    async fn test_evaluation() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FeatureFlagManager::new(temp.path().to_path_buf()).unwrap();

        let context = EvaluationContext {
            user_id: Some("test-user".to_string()),
            groups: vec!["internal".to_string()],
            environment: "development".to_string(),
            custom_attributes: HashMap::new(),
            session_id: None,
        };

        // web_search should be enabled for internal users in development
        let result = manager.evaluate("web_search", &context).await.unwrap();
        assert!(result.enabled);
    }

    #[tokio::test]
    async fn test_empty_stages_instant_rollout_does_not_panic() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FeatureFlagManager::new(temp.path().to_path_buf()).unwrap();

        let context = EvaluationContext {
            user_id: Some("user-1".to_string()),
            groups: vec![],
            environment: "production".to_string(),
            custom_attributes: HashMap::new(),
            session_id: None,
        };

        // unified_exec has strategy Instant with empty stages; must not panic
        let res = manager.evaluate("unified_exec", &context).await.unwrap();
        assert!(res.enabled);
        assert_eq!(res.reason, EvaluationReason::RolloutEnabled);
    }

    #[tokio::test]
    async fn test_custom_attributes_targeting_and_archived_dependency() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FeatureFlagManager::new(temp.path().to_path_buf()).unwrap();

        // Create base flag and dependent flag
        let base_flag = FeatureFlag {
            id: "base_feature".to_string(),
            name: "Base Feature".to_string(),
            description: "Base".to_string(),
            category: FeatureCategory::Stable,
            state: FeatureState::Archived,
            rollout: RolloutConfig {
                strategy: RolloutStrategy::Instant,
                percentage: 100,
                stages: vec![],
                current_stage: 0,
            },
            targeting: TargetingRules::default(),
            dependencies: vec![],
            metadata: FeatureMetadata {
                jira_ticket: None,
                github_pr: None,
                owner_team: "core".to_string(),
                documentation_url: None,
                migration_guide_url: None,
                sunset_date: None,
            },
            created_at: 0,
            updated_at: 0,
            created_by: "test".to_string(),
        };

        let mut custom_map = HashMap::new();
        custom_map.insert("tier".to_string(), vec!["enterprise".to_string()]);

        let child_flag = FeatureFlag {
            id: "child_feature".to_string(),
            name: "Child Feature".to_string(),
            description: "Child".to_string(),
            category: FeatureCategory::Beta,
            state: FeatureState::Enabled,
            rollout: RolloutConfig {
                strategy: RolloutStrategy::Instant,
                percentage: 100,
                stages: vec![],
                current_stage: 0,
            },
            targeting: TargetingRules {
                custom_attributes: custom_map,
                ..Default::default()
            },
            dependencies: vec!["base_feature".to_string()],
            metadata: FeatureMetadata {
                jira_ticket: None,
                github_pr: None,
                owner_team: "core".to_string(),
                documentation_url: None,
                migration_guide_url: None,
                sunset_date: None,
            },
            created_at: 0,
            updated_at: 0,
            created_by: "test".to_string(),
        };

        manager
            .create_flag(base_flag, "test".to_string())
            .await
            .unwrap();
        manager
            .create_flag(child_flag, "test".to_string())
            .await
            .unwrap();

        let ctx = EvaluationContext {
            user_id: Some("u1".to_string()),
            groups: vec![],
            environment: "production".to_string(),
            custom_attributes: HashMap::from([("tier".to_string(), "enterprise".to_string())]),
            session_id: None,
        };

        let res = manager.evaluate("child_feature", &ctx).await.unwrap();
        assert!(!res.enabled);
        assert_eq!(res.reason, EvaluationReason::Archived);
    }

    #[tokio::test]
    async fn test_builtin_feature_flag_state_persistence_across_reboots() {
        let temp = tempfile::tempdir().unwrap();
        let storage_path = temp.path().to_path_buf();

        // 1. Initial manager instance has web_search Enabled by default
        {
            let manager = FeatureFlagManager::new(storage_path.clone()).unwrap();
            let flag = manager.get_flag("web_search").await.unwrap();
            assert_eq!(flag.state, FeatureState::Enabled);

            // User disables web_search
            manager
                .disable_flag("web_search", "operator".to_string())
                .await
                .unwrap();
            let updated = manager.get_flag("web_search").await.unwrap();
            assert_eq!(updated.state, FeatureState::Disabled);
        }

        // 2. Restarted manager instance must preserve Disabled state and NOT overwrite with builtin default
        {
            let manager2 = FeatureFlagManager::new(storage_path).unwrap();
            let loaded = manager2.get_flag("web_search").await.unwrap();
            assert_eq!(
                loaded.state,
                FeatureState::Disabled,
                "Persisted disabled state for builtin flag must not be overwritten by register_builtin_flags"
            );
        }
    }
}
