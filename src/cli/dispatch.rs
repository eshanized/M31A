//! Shared Runtime Application Command Dispatch Layer (CLI-01, CLI-04, D-18).

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::capability::registry::CapabilityRegistry;
use crate::checkpoint::manager::CheckpointManager;
use crate::cli::args::{
    Cli, Commands, ConfigCommands, MissionCommands, PolicyCommands, TaskCommands,
};
use crate::cli::doctor::DoctorRunner;
use crate::config::schema::parse_and_validate_config;
use crate::events::bus::{BroadcastEventBus, EventBus};
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{MissionId, TaskId};
use crate::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use crate::persistence::artifacts::ArtifactStore;
use crate::persistence::sqlite::repositories::MissionRepository;
use crate::policy::approval::ApprovalCoordinator;

/// Unified application command structure shared by CLI and TUI (CLI-04).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuntimeCommand {
    NewSession,
    ListSessions,
    ShowSession {
        id: String,
    },
    ResumeSession {
        id: String,
    },
    RunMission {
        prompt: String,
        profile: Option<String>,
        wait_for_approval: bool,
    },
    ListMissions {
        all: bool,
    },
    ShowMission {
        id: String,
    },
    PauseMission {
        id: String,
    },
    ResumeMission {
        id: String,
    },
    CancelMission {
        id: String,
        reason: Option<String>,
    },
    ForkMission {
        id: String,
        prompt: Option<String>,
    },
    ListTasks {
        mission_id: Option<String>,
    },
    ShowTask {
        id: String,
    },
    ListAgents,
    ListCapabilities,
    CheckPolicy {
        tool: String,
        mission_id: Option<String>,
    },
    ResolveApproval {
        approval_id: String,
        decision: crate::tui::approval::ApprovalDecision,
    },
    ListCheckpoints {
        mission_id: Option<String>,
    },
    RestoreCheckpoint {
        id: String,
    },
    ListArtifacts {
        mission_id: Option<String>,
    },
    ShowArtifact {
        id: String,
    },
    RunDoctor {
        category: Option<String>,
        json: bool,
    },
    ValidateConfig {
        path: Option<std::path::PathBuf>,
    },
    ConfigGet {
        key: String,
    },
    ConfigSet {
        key: String,
        value: String,
    },
    ConfigSources,
    ConfigExplain {
        key: String,
    },
    RunInit {
        force: bool,
    },
    InspectTelemetry {
        mission_id: String,
        summary: bool,
        spans: bool,
        metrics: bool,
        export: Option<String>,
    },
    RunEval {
        scenario: Option<String>,
        all: bool,
        iterations: Option<usize>,
        output: Option<String>,
    },
    Version {
        verbose: bool,
    },
    ShowDeployment {
        verbose: bool,
    },
    Update {
        check_only: bool,
        manifest: Option<std::path::PathBuf>,
        install_dir: Option<std::path::PathBuf>,
        target: Option<String>,
    },
    Rollback {
        install_dir: Option<std::path::PathBuf>,
    },
}

/// Standardized execution result returned by all runtime commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliOutput {
    pub text: String,
    pub data: serde_json::Value,
    pub exit_code: i32,
}

impl CliOutput {
    pub fn success(text: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            text: text.into(),
            data,
            exit_code: 0,
        }
    }

    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }
}

impl From<crate::telemetry::TelemetryInspectionReport> for CliOutput {
    fn from(r: crate::telemetry::TelemetryInspectionReport) -> Self {
        Self {
            text: r.text,
            data: r.data,
            exit_code: r.exit_code,
        }
    }
}

/// Application command execution errors.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CliError {
    #[error("Policy violation: {0}")]
    PolicyViolation(String),
    #[error("Configuration validation error: {0}")]
    ConfigError(String),
    #[error("Entity not found: {0}")]
    NotFound(String),
    #[error("Command execution error: {0}")]
    ExecutionFailed(String),
}

/// Central dispatcher executing commands against runtime services (CLI-04).
#[derive(Clone)]
pub struct CliDispatcher {
    pub policy_gate: Option<Arc<dyn PolicyGate>>,
    pub capability_registry: Option<Arc<CapabilityRegistry>>,
    pub event_bus: Option<Arc<BroadcastEventBus>>,
    pub pool: Option<SqlitePool>,
    pub mission_repo: Option<Arc<dyn MissionRepository>>,
    pub checkpoint_manager: Option<Arc<CheckpointManager>>,
    pub artifact_store: Option<Arc<dyn ArtifactStore>>,
    pub approval_coordinator: Option<Arc<ApprovalCoordinator>>,
    pub runtime: Option<Arc<crate::runtime::AppRuntime>>,
    pub workspace_root: Option<std::path::PathBuf>,
    pub config: Option<Arc<crate::config::ResolvedConfiguration>>,
}

impl Default for CliDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl CliDispatcher {
    pub fn new() -> Self {
        Self {
            policy_gate: None,
            capability_registry: None,
            event_bus: None,
            pool: None,
            mission_repo: None,
            checkpoint_manager: None,
            artifact_store: None,
            approval_coordinator: None,
            runtime: None,
            workspace_root: None,
            config: None,
        }
    }

    pub fn with_policy_gate(mut self, gate: Arc<dyn PolicyGate>) -> Self {
        self.policy_gate = Some(gate);
        self
    }

    pub fn with_capabilities(mut self, reg: Arc<CapabilityRegistry>) -> Self {
        self.capability_registry = Some(reg);
        self
    }

    pub fn with_event_bus(mut self, bus: Arc<BroadcastEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn with_pool(mut self, pool: SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn with_mission_repo(mut self, repo: Arc<dyn MissionRepository>) -> Self {
        self.mission_repo = Some(repo);
        self
    }

    pub fn with_checkpoint_manager(mut self, mgr: Arc<CheckpointManager>) -> Self {
        self.checkpoint_manager = Some(mgr);
        self
    }

    pub fn with_artifact_store(mut self, store: Arc<dyn ArtifactStore>) -> Self {
        self.artifact_store = Some(store);
        self
    }

    pub fn with_approval_coordinator(mut self, coord: Arc<ApprovalCoordinator>) -> Self {
        self.approval_coordinator = Some(coord);
        self
    }

    /// Attach the canonical production runtime (one authority per scope).
    /// All shareable authorities resolve from the runtime: policy,
    /// capabilities, artifacts, checkpoints, pool, and event bus. Callers
    /// that reconfigure the runtime via `with_config` must re-attach, since
    /// `with_config` performs atomic reconstruction.
    pub fn with_runtime(mut self, rt: Arc<crate::runtime::AppRuntime>) -> Self {
        self.config = Some(rt.config().clone());
        self.policy_gate = Some(rt.policy().clone() as Arc<dyn PolicyGate>);
        self.approval_coordinator = Some(rt.approval_coordinator().clone());
        self.capability_registry = Some(rt.capability_registry().clone());
        self.artifact_store = Some(rt.artifact_store().clone() as Arc<dyn ArtifactStore>);
        self.checkpoint_manager = Some(rt.checkpoint_manager());
        self.pool = Some(rt.pool().clone());
        self.event_bus = Some(rt.event_bus().clone());
        self.workspace_root = Some(rt.workspace_root().to_path_buf());
        self.runtime = Some(rt);
        self
    }

    pub fn with_config(mut self, config: Arc<crate::config::ResolvedConfiguration>) -> Self {
        let ws = self
            .workspace_root
            .clone()
            .unwrap_or_else(|| config.workspace_root.clone());
        self.policy_gate = Some(Arc::new(
            crate::policy::effective::EffectivePolicy::standard_with_policy_config(
                &ws,
                Some(&config.app_config.policy),
            ),
        ) as Arc<dyn PolicyGate>);
        self.config = Some(config);
        self
    }

    pub fn with_workspace_root(mut self, root: std::path::PathBuf) -> Self {
        self.workspace_root = Some(root);
        self
    }

    pub fn production(
        pool: SqlitePool,
        storage_root: std::path::PathBuf,
        bus: Arc<BroadcastEventBus>,
    ) -> Self {
        // NOTE: `storage_root` here is the WORKSPACE root (see main.rs).
        // Standalone fallback stack, superseded by `with_runtime` whenever a
        // composed runtime exists. Paths are channel-aware so even the
        // standalone stack cannot cross deployment channels.
        let channel = crate::deployment::DeploymentChannel::current();
        let policy_gate = Arc::new(crate::policy::effective::EffectivePolicy::standard(
            &storage_root,
        ));
        let capabilities = Arc::new(crate::capability::registry::CapabilityRegistry::production(
            &storage_root,
            Some(bus.clone()),
            None,
        ));
        let mission_repo = Arc::new(
            crate::persistence::sqlite::repositories::SqliteMissionRepository::new(pool.clone()),
        );
        let artifacts = Arc::new(crate::persistence::artifacts::FsArtifactStore::new(
            crate::deployment::DeploymentPaths::project_artifacts_dir(&storage_root, channel),
        ));
        let checkpoint_mgr = Arc::new(crate::checkpoint::manager::CheckpointManager::new(
            pool.clone(),
            artifacts.clone(),
            // Canonical staging directory shared with AppRuntime and controller
            // dependencies to enforce a single authoritative checkpoint manager per runtime scope.
            crate::deployment::DeploymentPaths::project_staging_dir(&storage_root, channel),
        ));
        let coordinator = Arc::new(crate::policy::approval::ApprovalCoordinator::new(
            Some(pool.clone()),
            None,
        ));

        Self {
            policy_gate: Some(policy_gate),
            capability_registry: Some(capabilities),
            event_bus: Some(bus),
            pool: Some(pool),
            mission_repo: Some(mission_repo),
            checkpoint_manager: Some(checkpoint_mgr),
            artifact_store: Some(artifacts),
            approval_coordinator: Some(coordinator),
            runtime: None,
            workspace_root: Some(storage_root),
            config: None,
        }
    }

    /// Convert parsed CLI arguments into a canonical RuntimeCommand.
    pub fn parse_command(&self, cli: &Cli) -> Option<RuntimeCommand> {
        let cmd = cli.command.as_ref()?;
        match cmd {
            Commands::Session(s) => match &s.command {
                None | Some(crate::cli::args::SessionCommands::New) => {
                    Some(RuntimeCommand::NewSession)
                }
                Some(crate::cli::args::SessionCommands::List) => Some(RuntimeCommand::ListSessions),
                Some(crate::cli::args::SessionCommands::Show { id }) => {
                    Some(RuntimeCommand::ShowSession { id: id.clone() })
                }
                Some(crate::cli::args::SessionCommands::Resume { id }) => {
                    Some(RuntimeCommand::ResumeSession { id: id.clone() })
                }
            },
            Commands::Mission(m) => match &m.command {
                MissionCommands::Run {
                    prompt,
                    profile,
                    wait_for_approval,
                } => Some(RuntimeCommand::RunMission {
                    prompt: prompt.clone(),
                    profile: profile.clone(),
                    wait_for_approval: *wait_for_approval,
                }),
                MissionCommands::List { all } => Some(RuntimeCommand::ListMissions { all: *all }),
                MissionCommands::Show { id } => {
                    Some(RuntimeCommand::ShowMission { id: id.clone() })
                }
                MissionCommands::Pause { id } => {
                    Some(RuntimeCommand::PauseMission { id: id.clone() })
                }
                MissionCommands::Resume { id } => {
                    Some(RuntimeCommand::ResumeMission { id: id.clone() })
                }
                MissionCommands::Cancel { id, reason } => Some(RuntimeCommand::CancelMission {
                    id: id.clone(),
                    reason: reason.clone(),
                }),
                MissionCommands::Fork { id, prompt } => Some(RuntimeCommand::ForkMission {
                    id: id.clone(),
                    prompt: prompt.clone(),
                }),
            },
            Commands::Task(t) => match &t.command {
                TaskCommands::List { mission_id } => Some(RuntimeCommand::ListTasks {
                    mission_id: mission_id.clone(),
                }),
                TaskCommands::Show { id } => Some(RuntimeCommand::ShowTask { id: id.clone() }),
            },
            Commands::Agent(_) => Some(RuntimeCommand::ListAgents),
            Commands::Capability(_) => Some(RuntimeCommand::ListCapabilities),
            Commands::Policy(p) => match &p.command {
                PolicyCommands::Check { tool, mission_id } => Some(RuntimeCommand::CheckPolicy {
                    tool: tool.clone(),
                    mission_id: mission_id.clone(),
                }),
            },
            Commands::Checkpoint(c) => match &c.command {
                crate::cli::args::CheckpointCommands::List { mission_id } => {
                    Some(RuntimeCommand::ListCheckpoints {
                        mission_id: mission_id.clone(),
                    })
                }
                crate::cli::args::CheckpointCommands::Restore { id } => {
                    Some(RuntimeCommand::RestoreCheckpoint { id: id.clone() })
                }
            },
            Commands::Artifact(a) => match &a.command {
                crate::cli::args::ArtifactCommands::List { mission_id } => {
                    Some(RuntimeCommand::ListArtifacts {
                        mission_id: mission_id.clone(),
                    })
                }
                crate::cli::args::ArtifactCommands::Show { id } => {
                    Some(RuntimeCommand::ShowArtifact { id: id.clone() })
                }
            },
            Commands::Doctor(d) => Some(RuntimeCommand::RunDoctor {
                category: d.category.clone(),
                json: d.json,
            }),
            Commands::Config(c) => match &c.command {
                ConfigCommands::Validate { path } => {
                    Some(RuntimeCommand::ValidateConfig { path: path.clone() })
                }
                ConfigCommands::Get { key } => Some(RuntimeCommand::ConfigGet { key: key.clone() }),
                ConfigCommands::Set { key, value } => Some(RuntimeCommand::ConfigSet {
                    key: key.clone(),
                    value: value.clone(),
                }),
                ConfigCommands::Sources => Some(RuntimeCommand::ConfigSources),
                ConfigCommands::Explain { key } => {
                    Some(RuntimeCommand::ConfigExplain { key: key.clone() })
                }
            },
            Commands::Telemetry(t) => match &t.command {
                crate::cli::args::TelemetryCommands::Inspect {
                    mission_id,
                    summary,
                    spans,
                    metrics,
                    export,
                } => Some(RuntimeCommand::InspectTelemetry {
                    mission_id: mission_id.clone(),
                    summary: *summary,
                    spans: *spans,
                    metrics: *metrics,
                    export: export.clone(),
                }),
            },
            Commands::Eval(e) => match &e.command {
                crate::cli::args::EvalCommands::Run {
                    scenario,
                    all,
                    iterations,
                    output,
                } => Some(RuntimeCommand::RunEval {
                    scenario: scenario.clone(),
                    all: *all,
                    iterations: *iterations,
                    output: output.clone(),
                }),
            },
            Commands::Tui => None,
            Commands::Init(a) => Some(RuntimeCommand::RunInit { force: a.force }),
            Commands::Version(a) => Some(RuntimeCommand::Version { verbose: a.verbose }),
            Commands::Deployment(a) => Some(RuntimeCommand::ShowDeployment { verbose: a.verbose }),
            Commands::Update(a) => Some(RuntimeCommand::Update {
                check_only: a.check,
                manifest: a.manifest.clone(),
                install_dir: a.install_dir.clone(),
                target: a.target.clone(),
            }),
            Commands::Rollback(a) => Some(RuntimeCommand::Rollback {
                install_dir: a.install_dir.clone(),
            }),
        }
    }

    /// Dispatch a canonical RuntimeCommand against runtime services (CLI-04).
    pub async fn dispatch(&self, command: RuntimeCommand) -> Result<CliOutput, CliError> {
        match command {
            RuntimeCommand::Version { verbose } => {
                // Deployment-aware version: channel is part of the output,
                // canonical semver (CARGO_PKG_VERSION) is never forked.
                let ctx = crate::deployment::DeploymentContext::current();
                if verbose {
                    let mut text = ctx.cli_version_string();
                    text.push('\n');
                    text.push_str(&ctx.verbose_report());
                    let mut data = ctx.to_json();
                    data["runtime"] = serde_json::json!("m31a");
                    Ok(CliOutput::success(text, data))
                } else {
                    let text = ctx.cli_version_string();
                    let mut data = ctx.to_json();
                    data["runtime"] = serde_json::json!("m31a");
                    Ok(CliOutput::success(text, data))
                }
            }

            RuntimeCommand::ShowDeployment { verbose } => {
                // Canonical deployment identity + (verbose) filesystem paths.
                // Never exposes secrets; credentials are only named, not read.
                let ctx = crate::deployment::DeploymentContext::current();
                let paths = crate::deployment::DeploymentPaths::current();
                let ws = self
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let mut text = ctx.verbose_report();
                if verbose {
                    text.push_str(&format!(
                        "\nPaths:\n  Config: {}\n  Data: {}\n  Cache: {}\n  State: {}\n  Socket: {}\n  Project DB: {}\n  Project state: {}",
                        paths.config_dir().display(),
                        paths.data_dir().display(),
                        paths.cache_dir().display(),
                        paths.state_dir().display(),
                        paths.socket_path().display(),
                        crate::deployment::DeploymentPaths::project_db_path(&ws, ctx.channel)
                            .display(),
                        crate::deployment::DeploymentPaths::project_state_dir(&ws, ctx.channel)
                            .display(),
                    ));
                }
                let mut data = ctx.to_json();
                data["config_dir"] = serde_json::json!(paths.config_dir());
                data["data_dir"] = serde_json::json!(paths.data_dir());
                data["state_dir"] = serde_json::json!(paths.state_dir());
                Ok(CliOutput::success(text, data))
            }

            RuntimeCommand::Update {
                check_only,
                manifest,
                install_dir,
                target,
            } => {
                // Deterministic update from an explicit manifest file. No
                // network fetch, no model participation: the operator (or CI)
                // supplies immutable release metadata; we discover, verify,
                // and stage atomically. Production never consumes development
                // artifacts via this path.
                let manifest_path = manifest.ok_or_else(|| {
                    CliError::ExecutionFailed(
                        "update requires --manifest <deployment-manifest.json>".to_string(),
                    )
                })?;
                let doc = std::fs::read_to_string(&manifest_path).map_err(|e| {
                    CliError::ExecutionFailed(format!(
                        "cannot read manifest '{}': {e}",
                        manifest_path.display()
                    ))
                })?;
                let parsed = crate::deployment::DeploymentManifest::parse_json(&doc)
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                let ctx = crate::deployment::DeploymentContext::current();
                let target_ref = target.as_deref().unwrap_or(ctx.target.as_str());
                let candidate = crate::deployment::discover_update(
                    &parsed,
                    ctx.channel,
                    target_ref,
                    &ctx.version,
                )
                .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                if check_only {
                    return Ok(CliOutput::success(
                        format!(
                            "Update available: {} {} ({})",
                            candidate.channel.binary_name(),
                            candidate.version,
                            candidate.target
                        ),
                        serde_json::json!({
                            "current_version": ctx.version,
                            "candidate": candidate,
                        }),
                    ));
                }
                // Apply path requires artifact bytes alongside the manifest.
                // In this offline-first implementation the manifest's sibling
                // directory must contain the artifact file; checksum mismatch
                // or missing bytes fail closed with the live binary untouched.
                let manifest_dir = manifest_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let bytes = std::fs::read(manifest_dir.join(&candidate.filename)).map_err(|e| {
                    CliError::ExecutionFailed(format!(
                        "cannot read artifact '{}': {e}",
                        candidate.filename
                    ))
                })?;
                let artifact = crate::deployment::ReleaseArtifact {
                    artifact_id: format!(
                        "{}-{}-{}",
                        ctx.channel.binary_name(),
                        candidate.version,
                        candidate.target
                    ),
                    version: candidate.version.clone(),
                    channel: candidate.channel,
                    target: candidate.target.clone(),
                    format: archive_format(&candidate.filename).to_string(),
                    filename: candidate.filename.clone(),
                    sha256: candidate.sha256.clone(),
                    size: candidate.size,
                    build_id: candidate.build_id.clone(),
                    git_commit: candidate.commit.clone(),
                };
                let dir = install_dir.unwrap_or_else(default_install_dir);
                let installer = crate::deployment::Installer::new(&dir);
                let live = installer
                    .install_bytes(&artifact, &bytes, ctx.channel)
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                Ok(CliOutput::success(
                    format!(
                        "Updated {} to {} ({})",
                        ctx.channel.binary_name(),
                        candidate.version,
                        live.display()
                    ),
                    serde_json::json!({
                        "updated": true,
                        "version": candidate.version,
                        "path": live,
                    }),
                ))
            }

            RuntimeCommand::Rollback { install_dir } => {
                let ctx = crate::deployment::DeploymentContext::current();
                let dir = install_dir.unwrap_or_else(default_install_dir);
                let live = crate::deployment::rollback(&dir, ctx.channel.binary_name())
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                Ok(CliOutput::success(
                    format!(
                        "Rolled back {} ({})",
                        ctx.channel.binary_name(),
                        live.display()
                    ),
                    serde_json::json!({ "rolled_back": true, "path": live }),
                ))
            }

            RuntimeCommand::RunInit { force } => {
                // Explicit initialization entry: idempotent, never fakes success.
                // Bare `m31a` never behaves as `m31a init`; this path only runs
                // when the operator explicitly requests it.
                let pool = self.pool.as_ref().ok_or_else(|| {
                    CliError::ExecutionFailed("Database pool required".to_string())
                })?;
                let ws = self
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                match crate::init::resolve_startup(&ws, pool).await {
                    Err(e) => Err(CliError::ExecutionFailed(format!(
                        "Workspace initialization state is unusable: {e}. Resolve the underlying issue (do not delete state blindly) and retry."
                    ))),
                    Ok(crate::init::StartupDecision::Initialized(instance)) if !force => {
                        Ok(CliOutput::success(
                            format!(
                                "Workspace '{}' is already initialized (instance {}, state {}). Nothing to do.",
                                instance.workspace_root().display(),
                                instance.instance_id(),
                                instance.state().label()
                            ),
                            serde_json::json!({ "initialized": true, "instance_id": instance.instance_id() }),
                        ))
                    }
                    Ok(decision) => {
                        if force && decision.instance().is_initialized() {
                            // Explicit re-entry: rewind BOTH durable
                            // authorities via the canonical helper, then
                            // direct the operator to the interactive wizard.
                            // Completion itself still requires the wizard.
                            crate::init::begin_explicit_reonboarding(&ws, pool)
                                .await
                                .map_err(|e| {
                                    CliError::ExecutionFailed(format!(
                                        "Cannot re-enter onboarding: {e}"
                                    ))
                                })?;
                            return Ok(CliOutput::success(
                                "Workspace re-entered first-run onboarding. Run 'm31a tui' to complete setup.".to_string(),
                                serde_json::json!({ "initialized": false, "reentered": true }),
                            ));
                        }
                        Ok(CliOutput::success(
                            "Workspace is not initialized. Run 'm31a tui' to complete first-run setup.".to_string(),
                            serde_json::json!({ "initialized": false }),
                        ))
                    }
                }
            }

            RuntimeCommand::NewSession => {
                let pool = self.pool.as_ref().ok_or_else(|| {
                    CliError::ExecutionFailed("Database pool required".to_string())
                })?;
                let ws = self
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let repo = crate::interaction::session::SqliteSessionRepository::new(pool.clone());
                let sess = repo
                    .create_session(&ws)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

                Ok(CliOutput::success(
                    format!(
                        "Created session '{}' at {}",
                        sess.id,
                        sess.workspace_root.display()
                    ),
                    serde_json::json!({ "session": sess }),
                ))
            }

            RuntimeCommand::ListSessions => {
                let pool = self.pool.as_ref().ok_or_else(|| {
                    CliError::ExecutionFailed("Database pool required".to_string())
                })?;
                let repo = crate::interaction::session::SqliteSessionRepository::new(pool.clone());
                let list = repo
                    .list_sessions()
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

                let mut text = format!(
                    "{:<38} {:<12} {:<38} {}\n",
                    "SESSION ID", "STATUS", "ACTIVE MISSION", "WORKSPACE"
                );
                text.push_str(&format!("{}\n", "-".repeat(100)));
                for s in &list {
                    text.push_str(&format!(
                        "{:<38} {:<12} {:<38} {}\n",
                        s.id.to_string(),
                        s.status.to_string(),
                        s.active_mission_id
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        s.workspace_root.display()
                    ));
                }

                Ok(CliOutput::success(
                    text,
                    serde_json::json!({ "sessions": list }),
                ))
            }

            RuntimeCommand::ShowSession { id } => {
                let sid = id
                    .parse::<uuid::Uuid>()
                    .map(crate::ids::SessionId::from)
                    .map_err(|e| CliError::NotFound(format!("Invalid session id '{id}': {e}")))?;

                let pool = self.pool.as_ref().ok_or_else(|| {
                    CliError::ExecutionFailed("Database pool required".to_string())
                })?;
                let repo = crate::interaction::session::SqliteSessionRepository::new(pool.clone());
                let sess = repo
                    .get_session(sid)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                    .ok_or_else(|| CliError::NotFound(format!("Session '{id}' not found")))?;
                let history = repo.get_conversation(sid).await.unwrap_or_default();

                let mut out_text = format!(
                    "Session:        {}\n\
                     Workspace:      {}\n\
                     Status:         {}\n\
                     Active Mission: {}\n\
                     Created At:     {}\n\
                     Updated At:     {}\n\n\
                     Conversation History ({} turns):\n",
                    sess.id,
                    sess.workspace_root.display(),
                    sess.status,
                    sess.active_mission_id
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    sess.created_at,
                    sess.updated_at,
                    history.len()
                );

                for turn in &history {
                    out_text.push_str(&format!(
                        "  [{}] {}: {}\n",
                        turn.sequence(),
                        turn.kind_str(),
                        turn.text_content()
                    ));
                }

                Ok(CliOutput::success(
                    out_text,
                    serde_json::json!({
                        "session": sess,
                        "conversation": history
                    }),
                ))
            }

            RuntimeCommand::ResumeSession { id } => {
                let sid = id
                    .parse::<uuid::Uuid>()
                    .map(crate::ids::SessionId::from)
                    .map_err(|e| CliError::NotFound(format!("Invalid session id '{id}': {e}")))?;

                if let Some(ref rt) = self.runtime {
                    let sess = rt
                        .resume_session(sid)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                    let history = rt
                        .session_repo()
                        .get_conversation(sid)
                        .await
                        .unwrap_or_default();
                    let text = format!(
                        "Session '{}' resumed. Workspace: {}. History: {} turn(s).",
                        sess.id,
                        sess.workspace_root.display(),
                        history.len()
                    );
                    Ok(CliOutput::success(
                        text,
                        serde_json::json!({
                            "session_id": sess.id.to_string(),
                            "workspace": sess.workspace_root.to_string_lossy(),
                            "status": sess.status.to_string(),
                            "active_mission": sess.active_mission_id.map(|m| m.to_string()),
                            "turns_count": history.len()
                        }),
                    ))
                } else if let Some(ref pool) = self.pool {
                    let repo =
                        crate::interaction::session::SqliteSessionRepository::new(pool.clone());
                    let sess = repo
                        .get_session(sid)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                        .ok_or_else(|| CliError::NotFound(format!("Session '{id}' not found")))?;
                    repo.update_status(sid, crate::interaction::session::SessionState::Active)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                    Ok(CliOutput::success(
                        format!("Session '{}' resumed.", sess.id),
                        serde_json::json!({ "session_id": sess.id.to_string(), "status": "active" }),
                    ))
                } else {
                    Err(CliError::ExecutionFailed(
                        "No database connection available".to_string(),
                    ))
                }
            }

            RuntimeCommand::RunMission {
                prompt,
                profile,
                wait_for_approval,
            } => {
                // Execution requires a real runtime. A failed runtime
                // assembly is a typed error — NEVER a fabricated
                // `status: "started"` success (Invariant 7).
                let runtime = if let Some(ref rt) = self.runtime {
                    Some(rt.clone())
                } else if let (Some(pool), Some(bus)) = (self.pool.clone(), self.event_bus.clone())
                {
                    let ws = self
                        .workspace_root
                        .clone()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    let rt = crate::runtime::AppRuntime::from_pool_and_workspace(pool, ws, bus)
                        .await
                        .map_err(|e| {
                            CliError::ExecutionFailed(format!(
                                "runtime assembly failed, mission not started: {e}"
                            ))
                        })?;
                    Some(Arc::new(rt))
                } else {
                    None
                };

                if let Some(rt) = runtime {
                    let summary = rt
                        .run_mission(&prompt, profile.as_deref(), wait_for_approval)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                    Ok(CliOutput::success(
                        format!(
                            "Mission '{}' finished with status {}: {}",
                            summary.mission_id, summary.status, prompt
                        ),
                        serde_json::json!({
                            "mission_id": summary.mission_id.to_string(),
                            "prompt": prompt,
                            "profile": profile,
                            "wait_for_approval": wait_for_approval,
                            "status": summary.status,
                            "halt_reason": summary.halt_reason,
                            "tasks_completed": summary.tasks_completed,
                        }),
                    ))
                } else {
                    Err(CliError::ExecutionFailed(
                        "Runtime execution engine is unavailable to execute mission; mission was NOT started".to_string(),
                    ))
                }
            }

            RuntimeCommand::ListMissions { all } => {
                let list: Vec<serde_json::Value> = if let Some(ref repo) = self.mission_repo {
                    let missions = repo
                        .list_all()
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                    missions
                        .into_iter()
                        .map(|m| {
                            serde_json::json!({
                                "id": m.id.to_string(),
                                "objective": m.objective,
                                "state": format!("{:?}", m.status),
                                "created_at": m.created_at.to_rfc3339(),
                            })
                        })
                        .collect()
                } else if let Some(ref pool) = self.pool {
                    let rows = sqlx::query_as::<_, (Vec<u8>, String, String, String)>(
                        "SELECT id, objective, status, created_at FROM missions ORDER BY created_at DESC",
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                    rows.into_iter()
                        .map(|(id_b, obj, status, created_at)| {
                            let mid = MissionId::from_bytes(id_b.try_into().unwrap_or_default());
                            serde_json::json!({
                                "id": mid.to_string(),
                                "objective": obj,
                                "state": status,
                                "created_at": created_at,
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                let text = if list.is_empty() {
                    "Found 0 missions".to_string()
                } else {
                    let mut s = format!("Found {} missions:\n", list.len());
                    for m in &list {
                        let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("-");
                        let state = m.get("state").and_then(|v| v.as_str()).unwrap_or("-");
                        let obj = m.get("objective").and_then(|v| v.as_str()).unwrap_or("-");
                        s.push_str(&format!("  {} [{}] {}\n", id, state, obj));
                    }
                    s.trim_end().to_string()
                };

                Ok(CliOutput::success(
                    text,
                    serde_json::json!({
                        "missions": list,
                        "all": all
                    }),
                ))
            }

            RuntimeCommand::ShowMission { id } => {
                let mid = id
                    .parse::<MissionId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;

                if let Some(ref repo) = self.mission_repo {
                    let mission = repo
                        .get(mid)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                    match mission {
                        Some(m) => Ok(CliOutput::success(
                            format!("Mission {id}: {} ({:?})", m.objective, m.status),
                            serde_json::json!({
                                "mission_id": id,
                                "objective": m.objective,
                                "state": format!("{:?}", m.status),
                                "created_at": m.created_at.to_rfc3339(),
                            }),
                        )),
                        None => Err(CliError::NotFound(format!("Mission '{id}' not found"))),
                    }
                } else if let Some(ref pool) = self.pool {
                    let row = sqlx::query_as::<_, (Vec<u8>, String, String, String)>(
                        "SELECT id, objective, status, created_at FROM missions WHERE id = ?",
                    )
                    .bind(mid.as_bytes().as_slice())
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

                    match row {
                        Some((_, obj, status, created_at)) => Ok(CliOutput::success(
                            format!("Mission {id}: {obj} ({status})"),
                            serde_json::json!({
                                "mission_id": id,
                                "objective": obj,
                                "state": status,
                                "created_at": created_at,
                            }),
                        )),
                        None => Err(CliError::NotFound(format!("Mission '{id}' not found"))),
                    }
                } else {
                    Err(CliError::ExecutionFailed(
                        "No database connection available to query mission".to_string(),
                    ))
                }
            }

            RuntimeCommand::PauseMission { id } => {
                let mid = id
                    .parse::<MissionId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;

                if let Some(ref repo) = self.mission_repo {
                    repo.update_status(mid, crate::state_machine::MissionState::Paused)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else if let Some(ref pool) = self.pool {
                    let repo =
                        crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                            pool.clone(),
                        );
                    repo.update_status(mid, crate::state_machine::MissionState::Paused)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else {
                    return Err(CliError::ExecutionFailed(
                        "No mission repository available to pause mission".to_string(),
                    ));
                }

                if let Some(ref bus) = self.event_bus {
                    let env = EventEnvelope::new(
                        0,
                        Some(mid),
                        None,
                        "cli".to_string(),
                        EventType::MissionPaused {
                            mission_id: mid,
                            reason: "Operator request".to_string(),
                        },
                    );
                    let _ = bus.publish(env).await;
                }
                Ok(CliOutput::success(
                    format!("Mission '{id}' paused"),
                    serde_json::json!({ "mission_id": id, "status": "paused" }),
                ))
            }

            RuntimeCommand::ResumeMission { id } => {
                let mid = id
                    .parse::<MissionId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;

                if let Some(ref repo) = self.mission_repo {
                    repo.update_status(mid, crate::state_machine::MissionState::Executing)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else if let Some(ref pool) = self.pool {
                    let repo =
                        crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                            pool.clone(),
                        );
                    repo.update_status(mid, crate::state_machine::MissionState::Executing)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else {
                    return Err(CliError::ExecutionFailed(
                        "No mission repository available to resume mission".to_string(),
                    ));
                }

                if let Some(ref bus) = self.event_bus {
                    let env = EventEnvelope::new(
                        0,
                        Some(mid),
                        None,
                        "cli".to_string(),
                        EventType::MissionResumed { mission_id: mid },
                    );
                    let _ = bus.publish(env).await;
                }
                Ok(CliOutput::success(
                    format!("Mission '{id}' resumed"),
                    serde_json::json!({ "mission_id": id, "status": "running" }),
                ))
            }

            RuntimeCommand::CancelMission { id, reason } => {
                let mid = id
                    .parse::<MissionId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;
                let cancel_reason = reason.unwrap_or_else(|| "Operator cancelled".to_string());

                if let Some(ref repo) = self.mission_repo {
                    repo.update_status(mid, crate::state_machine::MissionState::Cancelled)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else if let Some(ref pool) = self.pool {
                    let repo =
                        crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                            pool.clone(),
                        );
                    repo.update_status(mid, crate::state_machine::MissionState::Cancelled)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else {
                    return Err(CliError::ExecutionFailed(
                        "No mission repository available to cancel mission".to_string(),
                    ));
                }

                if let Some(ref bus) = self.event_bus {
                    let env = EventEnvelope::new(
                        0,
                        Some(mid),
                        None,
                        "cli".to_string(),
                        EventType::MissionCancelled {
                            mission_id: mid,
                            reason: cancel_reason.clone(),
                        },
                    );
                    let _ = bus.publish(env).await;
                }
                Ok(CliOutput::success(
                    format!("Mission '{id}' cancelled: {cancel_reason}"),
                    serde_json::json!({
                        "mission_id": id,
                        "status": "cancelled",
                        "reason": cancel_reason
                    }),
                ))
            }

            RuntimeCommand::ForkMission { id, prompt } => {
                let new_id = MissionId::new();
                let objective = prompt
                    .clone()
                    .unwrap_or_else(|| format!("Fork of mission {id}"));

                let mission = crate::state::Mission::new(new_id, objective.clone());
                if let Some(ref repo) = self.mission_repo {
                    repo.insert(&mission)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else if let Some(ref pool) = self.pool {
                    let repo =
                        crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                            pool.clone(),
                        );
                    repo.insert(&mission)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else {
                    return Err(CliError::ExecutionFailed(
                        "No mission repository available to fork mission".to_string(),
                    ));
                }

                Ok(CliOutput::success(
                    format!("Forked mission '{id}' to '{new_id}'"),
                    serde_json::json!({
                        "parent_id": id,
                        "fork_id": new_id.to_string(),
                        "prompt": prompt
                    }),
                ))
            }

            RuntimeCommand::ListTasks { mission_id } => {
                let tasks: Vec<serde_json::Value> = if let Some(ref pool) = self.pool {
                    let rows = if let Some(ref mid_str) = mission_id {
                        let mid = mid_str
                            .parse::<MissionId>()
                            .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;
                        sqlx::query_as::<_, (Vec<u8>, Vec<u8>, String, String)>(
                            "SELECT id, mission_id, title, status FROM tasks WHERE mission_id = ?",
                        )
                        .bind(mid.as_bytes().as_slice())
                        .fetch_all(pool)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                    } else {
                        sqlx::query_as::<_, (Vec<u8>, Vec<u8>, String, String)>(
                            "SELECT id, mission_id, title, status FROM tasks",
                        )
                        .fetch_all(pool)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                    };

                    rows.into_iter()
                        .map(|(id_b, mid_b, title, status)| {
                            let tid = TaskId::from_bytes(id_b.try_into().unwrap_or_default());
                            let mid = MissionId::from_bytes(mid_b.try_into().unwrap_or_default());
                            serde_json::json!({
                                "task_id": tid.to_string(),
                                "mission_id": mid.to_string(),
                                "title": title,
                                "status": status,
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                Ok(CliOutput::success(
                    format!("Found {} tasks", tasks.len()),
                    serde_json::json!({ "tasks": tasks, "mission_id": mission_id }),
                ))
            }

            RuntimeCommand::ShowTask { id } => {
                let tid = id
                    .parse::<TaskId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid task id: {e}")))?;

                if let Some(ref pool) = self.pool {
                    let row = sqlx::query_as::<_, (Vec<u8>, Vec<u8>, String, String, Option<String>)>(
                        "SELECT id, mission_id, title, status, failure_reason FROM tasks WHERE id = ?",
                    )
                    .bind(tid.as_bytes().as_slice())
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

                    match row {
                        Some((_, mid_b, title, status, failure)) => {
                            let mid = MissionId::from_bytes(mid_b.try_into().unwrap_or_default());
                            Ok(CliOutput::success(
                                format!("Task {id}: {title} ({status})"),
                                serde_json::json!({
                                    "task_id": id,
                                    "mission_id": mid.to_string(),
                                    "title": title,
                                    "state": status,
                                    "failure_reason": failure,
                                }),
                            ))
                        }
                        None => Err(CliError::NotFound(format!("Task '{id}' not found"))),
                    }
                } else {
                    Ok(CliOutput::success(
                        format!("Task {id}"),
                        serde_json::json!({ "task_id": id, "state": "ready" }),
                    ))
                }
            }

            RuntimeCommand::ListAgents => {
                let builtin_roles: Vec<String> = crate::agent::registry::RoleRegistry::global()
                    .read()
                    .map(|guard| guard.builtin_ids())
                    .unwrap_or_default();
                Ok(CliOutput::success(
                    format!("{} canonical agent roles active", builtin_roles.len()),
                    serde_json::json!({
                        "roles": builtin_roles
                    }),
                ))
            }

            RuntimeCommand::ListCapabilities => {
                // Prefer the runtime's canonical registry when attached.
                let count = self
                    .runtime
                    .as_ref()
                    .map(|rt| rt.capability_registry().list_capabilities().len())
                    .or_else(|| {
                        self.capability_registry
                            .as_ref()
                            .map(|r| r.list_capabilities().len())
                    })
                    .unwrap_or(0);
                Ok(CliOutput::success(
                    format!("{count} capabilities registered"),
                    serde_json::json!({ "capability_count": count }),
                ))
            }

            RuntimeCommand::CheckPolicy { tool, mission_id } => {
                // Identities are explicit: a malformed mission id is a caller
                // error, never a silently fabricated zero identity.
                let mid = match mission_id.as_deref() {
                    Some(m) => m
                        .parse::<MissionId>()
                        .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?,
                    None => MissionId::default(),
                };
                let tid = TaskId::new();

                // Prefer the runtime's canonical policy when attached so a
                // reconfigured runtime cannot leave this path evaluating
                // against a stale gate, preserving a single policy authority per scope.
                let runtime_gate: Option<Arc<dyn PolicyGate>> = self
                    .runtime
                    .as_ref()
                    .map(|rt| rt.policy().clone() as Arc<dyn PolicyGate>);
                let gate = runtime_gate.as_ref().or(self.policy_gate.as_ref());
                if let Some(gate) = gate {
                    let req = PolicyEvaluationRequest {
                        mission_id: mid,
                        task_id: tid,
                        tool_or_action: tool.clone(),
                        context_digest: "cli_check".to_string(),
                    };
                    let decision = gate
                        .evaluate(req)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

                    let (decision_str, code) = match decision {
                        PolicyDecision::Allow => ("allow", 0),
                        PolicyDecision::Ask => ("ask", 0),
                        PolicyDecision::Deny | PolicyDecision::Escalate => ("deny", 4),
                    };

                    Ok(CliOutput::success(
                        format!("Tool '{tool}': {decision_str}"),
                        serde_json::json!({
                            "tool": tool,
                            "decision": decision_str
                        }),
                    )
                    .with_exit_code(code))
                } else {
                    // Fail closed: no policy authority is attached, so no
                    // authorization claim can be made. A default "allow" here
                    // would be a fabricated governance decision.
                    Err(CliError::ExecutionFailed(
                        "No policy authority attached; cannot evaluate tool authorization"
                            .to_string(),
                    ))
                }
            }

            RuntimeCommand::ResolveApproval {
                approval_id,
                decision,
            } => {
                let action = match decision {
                    crate::tui::approval::ApprovalDecision::ApproveOnce => {
                        crate::policy::approval::ApprovalAction::AllowOnce
                    }
                    crate::tui::approval::ApprovalDecision::ApproveAlways => {
                        crate::policy::approval::ApprovalAction::AllowForMission
                    }
                    crate::tui::approval::ApprovalDecision::Reject => {
                        crate::policy::approval::ApprovalAction::Deny {
                            reason: "Rejected by operator in TUI".to_string(),
                        }
                    }
                    crate::tui::approval::ApprovalDecision::Dismiss => {
                        crate::policy::approval::ApprovalAction::Deny {
                            reason: "Dismissed by operator in TUI".to_string(),
                        }
                    }
                    crate::tui::approval::ApprovalDecision::Edit => {
                        return Err(CliError::ExecutionFailed(
                            "Parameter editing is not currently supported".to_string(),
                        ));
                    }
                };

                let action_str = format!("{action:?}");
                if let Some(ref coordinator) = self.approval_coordinator {
                    let req_id = approval_id
                        .parse::<crate::ids::ApprovalRequestId>()
                        .map_err(|e| {
                            CliError::NotFound(format!("Invalid approval request id: {e}"))
                        })?;
                    coordinator
                        .resolve_request(req_id, action.clone(), "operator")
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                } else if let Some(ref pool) = self.pool {
                    let coordinator = ApprovalCoordinator::new(Some(pool.clone()), None);
                    let req_id = approval_id
                        .parse::<crate::ids::ApprovalRequestId>()
                        .map_err(|e| {
                            CliError::NotFound(format!("Invalid approval request id: {e}"))
                        })?;
                    coordinator
                        .resolve_request(req_id, action.clone(), "operator")
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;
                }

                if let Some(ref bus) = self.event_bus {
                    let env = EventEnvelope::new(
                        0,
                        None,
                        None,
                        "cli".to_string(),
                        EventType::ApprovalResolved {
                            request_id: approval_id.clone(),
                            decision: action_str.clone(),
                            resolved_by: "operator".to_string(),
                        },
                    );
                    let _ = bus.publish(env).await;
                }

                Ok(CliOutput::success(
                    format!("Approval request '{approval_id}' resolved: {action_str}"),
                    serde_json::json!({
                        "approval_id": approval_id,
                        "decision": format!("{decision:?}"),
                        "action": action_str,
                    }),
                ))
            }

            RuntimeCommand::ListCheckpoints { mission_id } => {
                let checkpoints: Vec<serde_json::Value> = if let Some(ref pool) = self.pool {
                    let rows = if let Some(ref mid_str) = mission_id {
                        let mid = mid_str
                            .parse::<MissionId>()
                            .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;
                        sqlx::query_as::<_, (Vec<u8>, Vec<u8>, i64, String, i64, String, String)>(
                            "SELECT id, mission_id, sequence, stage, cycle, state_summary, created_at FROM checkpoints WHERE mission_id = ? ORDER BY sequence ASC",
                        )
                        .bind(mid.as_bytes().as_slice())
                        .fetch_all(pool)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                    } else {
                        sqlx::query_as::<_, (Vec<u8>, Vec<u8>, i64, String, i64, String, String)>(
                            "SELECT id, mission_id, sequence, stage, cycle, state_summary, created_at FROM checkpoints ORDER BY created_at ASC",
                        )
                        .fetch_all(pool)
                        .await
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                    };

                    rows.into_iter()
                        .map(|(id_b, mid_b, seq, stage, cycle, summary, created_at)| {
                            let cid = crate::ids::CheckpointId::from_bytes(
                                id_b.try_into().unwrap_or_default(),
                            );
                            let mid = MissionId::from_bytes(mid_b.try_into().unwrap_or_default());
                            serde_json::json!({
                                "checkpoint_id": cid.to_string(),
                                "mission_id": mid.to_string(),
                                "sequence": seq,
                                "stage": stage,
                                "cycle": cycle,
                                "state_summary": summary,
                                "created_at": created_at,
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                Ok(CliOutput::success(
                    format!("Found {} checkpoints", checkpoints.len()),
                    serde_json::json!({ "checkpoints": checkpoints, "mission_id": mission_id }),
                ))
            }

            RuntimeCommand::RestoreCheckpoint { id } => {
                let cid = id
                    .parse::<crate::ids::CheckpointId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid checkpoint id: {e}")))?;

                let mgr = if let Some(ref m) = self.checkpoint_manager {
                    m.clone()
                } else if let Some(ref pool) = self.pool {
                    // Channel-aware storage: production legacy paths,
                    // development isolated siblings (never shared).
                    let ws = self
                        .workspace_root
                        .clone()
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    let channel = crate::deployment::DeploymentChannel::current();
                    let artifacts = self.artifact_store.clone().unwrap_or_else(|| {
                        Arc::new(crate::persistence::artifacts::FsArtifactStore::new(
                            crate::deployment::DeploymentPaths::project_artifacts_dir(&ws, channel),
                        ))
                    });
                    let staging =
                        crate::deployment::DeploymentPaths::project_staging_dir(&ws, channel);
                    Arc::new(crate::checkpoint::manager::CheckpointManager::new(
                        pool.clone(),
                        artifacts,
                        staging,
                    ))
                } else {
                    return Err(CliError::ExecutionFailed(
                        "No checkpoint manager or database connection available".to_string(),
                    ));
                };

                let ws = self
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));

                let res = mgr
                    .restore_checkpoint(cid, &ws)
                    .await
                    .map_err(|e| match e {
                        crate::checkpoint::manager::CheckpointError::NotFound(_) => {
                            CliError::NotFound(format!("Checkpoint '{id}' not found"))
                        }
                        other => CliError::ExecutionFailed(other.to_string()),
                    })?;

                if let Some(ref bus) = self.event_bus {
                    let env = EventEnvelope::new(
                        0,
                        Some(res.mission_id),
                        None,
                        "cli".to_string(),
                        EventType::CheckpointRestored {
                            checkpoint_id: cid,
                            mission_id: res.mission_id,
                        },
                    );
                    let _ = bus.publish(env).await;
                }

                Ok(CliOutput::success(
                    format!(
                        "Restored checkpoint {id} (cycle {}, stage {})",
                        res.cycle, res.stage
                    ),
                    serde_json::json!({
                        "checkpoint_id": id,
                        "mission_id": res.mission_id.to_string(),
                        "sequence": res.sequence,
                        "stage": res.stage,
                        "cycle": res.cycle,
                        "preserved_tasks": res.preserved_tasks,
                        "invalidated_tasks": res.invalidated_tasks,
                        "rescheduled_tasks": res.rescheduled_tasks,
                        "summary": res.summary,
                        "restored": true,
                    }),
                ))
            }

            RuntimeCommand::ListArtifacts { mission_id } => {
                let pool = self.pool.as_ref().ok_or_else(|| {
                    CliError::ExecutionFailed("Database pool required".to_string())
                })?;

                let rows = if let Some(ref mid_str) = mission_id {
                    let uuid = mid_str.parse::<uuid::Uuid>().map_err(|e| {
                        CliError::ExecutionFailed(format!("Invalid mission id: {e}"))
                    })?;
                    sqlx::query_as::<_, (Vec<u8>, String, i64, String)>(
                        "SELECT id, name, size_bytes, logical_path FROM artifacts WHERE mission_id = ? ORDER BY created_at DESC",
                    )
                    .bind(uuid.as_bytes().as_slice())
                    .fetch_all(pool)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                } else {
                    sqlx::query_as::<_, (Vec<u8>, String, i64, String)>(
                        "SELECT id, name, size_bytes, logical_path FROM artifacts ORDER BY created_at DESC LIMIT 50",
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                };

                let mut out_text = String::from(
                    "ARTIFACT ID                            NAME                 SIZE       PATH\n----------------------------------------------------------------------------------------------------\n",
                );
                let mut data_list = Vec::new();
                for (id_b, name, size, path) in rows {
                    let aid =
                        crate::ids::ArtifactId::from_bytes(id_b.try_into().unwrap_or_default());
                    out_text.push_str(&format!("{:<38} {:<20} {:<10} {}\n", aid, name, size, path));
                    data_list.push(serde_json::json!({
                        "id": aid.to_string(),
                        "name": name,
                        "size_bytes": size,
                        "path": path,
                    }));
                }
                if data_list.is_empty() {
                    out_text.push_str("Found 0 artifacts\n");
                }
                Ok(CliOutput::success(
                    out_text,
                    serde_json::Value::Array(data_list),
                ))
            }

            RuntimeCommand::ShowArtifact { id } => {
                let aid = id
                    .parse::<crate::ids::ArtifactId>()
                    .map_err(|e| CliError::NotFound(format!("Invalid artifact id: {e}")))?;

                let path = if let Some(ref store) = self.artifact_store {
                    store.path_for(aid, "json")
                } else {
                    std::path::PathBuf::from(format!("artifacts/{id}.json"))
                };
                let exists = path.exists();
                let size = if exists {
                    std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };

                Ok(CliOutput::success(
                    format!("Artifact {id}: {} ({} bytes)", path.display(), size),
                    serde_json::json!({
                        "artifact_id": id,
                        "path": path.to_string_lossy(),
                        "exists": exists,
                        "size_bytes": size,
                    }),
                ))
            }

            RuntimeCommand::RunDoctor { category, json } => {
                let runner = DoctorRunner::with_default_probes();
                let report = runner.run(category.as_deref()).await;
                let text = if json {
                    serde_json::to_string_pretty(&report.to_json()).unwrap_or_default()
                } else {
                    report.format_text()
                };
                Ok(CliOutput::success(text, report.to_json()))
            }

            RuntimeCommand::ValidateConfig { path } => {
                let ws = self
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let target_path = path.unwrap_or_else(|| ws.join(".m31a").join("config.toml"));
                // Intentional absence is reported explicitly: it validates
                // as documented defaults, but the response says so (rather
                // than claiming a file validated).
                let existed = target_path.exists();
                let content = if existed {
                    std::fs::read_to_string(&target_path).map_err(|e| {
                        CliError::ConfigError(format!(
                            "Failed to read {}: {e}",
                            target_path.display()
                        ))
                    })?
                } else {
                    "".to_string()
                };
                let cfg = parse_and_validate_config(&content)
                    .map_err(|e| CliError::ConfigError(e.to_string()))?;
                Ok(CliOutput::success(
                    if existed {
                        format!("Configuration schema valid ({})", target_path.display())
                    } else {
                        format!(
                            "No configuration file at {}; documented defaults validate",
                            target_path.display()
                        )
                    },
                    serde_json::json!({
                        "valid": true,
                        "exists": existed,
                        "file": target_path.display().to_string(),
                        "concurrency_limit": cfg.runtime.concurrency_limit,
                        "model": cfg.agents.default_model,
                    }),
                ))
            }

            RuntimeCommand::ConfigGet { key } => {
                let resolved_config = match &self.config {
                    Some(cfg) => cfg.clone(),
                    None => {
                        let ws = self
                            .workspace_root
                            .clone()
                            .unwrap_or_else(|| std::path::PathBuf::from("."));
                        Arc::new(crate::config::ResolvedConfiguration::build_fallback(&ws))
                    }
                };

                match resolved_config.provenance.resolve(&key) {
                    Some(resolved) => {
                        let safe_val = if crate::config::is_secret_key(&key) {
                            crate::config::mask_value(&resolved.value)
                        } else {
                            resolved.value
                        };
                        Ok(CliOutput::success(
                            format!("{key} = {} ({})", safe_val, resolved.layer.display_name()),
                            serde_json::json!({
                                "key": key,
                                "value": safe_val,
                                "layer": resolved.layer.display_name(),
                                "precedence": resolved.layer.precedence(),
                                "source_file": resolved.source_file.as_ref().map(|p| p.display().to_string()),
                            }),
                        ))
                    }
                    None => Err(CliError::NotFound(format!(
                        "Configuration key not found: {key}"
                    ))),
                }
            }

            RuntimeCommand::ConfigSet { key, value } => {
                let ws = self
                    .workspace_root
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));

                // Invariant protection: immutable security constraints cannot be mutated
                let resolved_config = match &self.config {
                    Some(cfg) => cfg.clone(),
                    None => Arc::new(crate::config::ResolvedConfiguration::build_fallback(&ws)),
                };
                if let Some(existing) = resolved_config.provenance.resolve(&key)
                    && existing.is_immutable
                {
                    return Err(CliError::ConfigError(format!(
                        "Cannot modify immutable security invariant '{key}'"
                    )));
                }

                let config_dir = ws.join(".m31a");
                let _ = std::fs::create_dir_all(&config_dir);
                let config_file = config_dir.join("config.toml");

                let toml_val: toml::Table = if config_file.exists() {
                    // A present-but-corrupt file is a hard error: parsing it
                    // as empty would silently destroy operator configuration.
                    let content = std::fs::read_to_string(&config_file).map_err(|e| {
                        CliError::ConfigError(format!(
                            "Failed to read {}: {e}",
                            config_file.display()
                        ))
                    })?;
                    content.parse::<toml::Table>().map_err(|e| {
                        CliError::ConfigError(format!(
                            "Existing configuration {} is corrupt: {e}",
                            config_file.display()
                        ))
                    })?
                } else {
                    toml::Table::new()
                };

                let parsed_val: toml::Value = if let Ok(b) = value.parse::<bool>() {
                    toml::Value::Boolean(b)
                } else if let Ok(i) = value.parse::<i64>() {
                    toml::Value::Integer(i)
                } else if let Ok(f) = value.parse::<f64>() {
                    toml::Value::Float(f)
                } else if (value.starts_with('[') && value.ends_with(']'))
                    || (value.starts_with('{') && value.ends_with('}'))
                {
                    value
                        .parse::<toml::Value>()
                        .unwrap_or_else(|_| toml::Value::String(value.clone()))
                } else {
                    toml::Value::String(value.clone())
                };

                let mut candidate_table = toml_val.clone();
                insert_dotted_toml_value(&mut candidate_table, &key, parsed_val.clone())
                    .map_err(CliError::ConfigError)?;

                let serialized = toml::to_string_pretty(&candidate_table)
                    .map_err(|e| CliError::ConfigError(e.to_string()))?;

                let validation_result =
                    crate::config::schema::parse_and_validate_config(&serialized);
                let final_serialized = match validation_result {
                    Ok(_) => serialized,
                    Err(err) => {
                        if !parsed_val.is_str() {
                            let mut fallback_table = toml_val.clone();
                            let str_val = toml::Value::String(value.clone());
                            if insert_dotted_toml_value(&mut fallback_table, &key, str_val).is_ok()
                            {
                                if let Ok(candidate_ser) = toml::to_string_pretty(&fallback_table) {
                                    if crate::config::schema::parse_and_validate_config(
                                        &candidate_ser,
                                    )
                                    .is_ok()
                                    {
                                        candidate_ser
                                    } else {
                                        return Err(CliError::ConfigError(format!(
                                            "Invalid configuration for key '{key}': {err}"
                                        )));
                                    }
                                } else {
                                    return Err(CliError::ConfigError(format!(
                                        "Invalid configuration for key '{key}': {err}"
                                    )));
                                }
                            } else {
                                return Err(CliError::ConfigError(format!(
                                    "Invalid configuration for key '{key}': {err}"
                                )));
                            }
                        } else {
                            return Err(CliError::ConfigError(format!(
                                "Invalid configuration for key '{key}': {err}"
                            )));
                        }
                    }
                };

                std::fs::write(&config_file, final_serialized)
                    .map_err(|e| CliError::ConfigError(e.to_string()))?;

                Ok(CliOutput::success(
                    format!("Configuration updated: {key} = {value}"),
                    serde_json::json!({
                        "key": key,
                        "value": value,
                        "updated": true,
                    }),
                ))
            }

            RuntimeCommand::ConfigSources => {
                let resolved_config = match &self.config {
                    Some(cfg) => cfg.clone(),
                    None => {
                        let ws = self
                            .workspace_root
                            .clone()
                            .unwrap_or_else(|| std::path::PathBuf::from("."));
                        Arc::new(crate::config::ResolvedConfiguration::build_fallback(&ws))
                    }
                };

                let sources = resolved_config.sources();
                let ctx = crate::deployment::DeploymentContext::current();
                let paths = crate::deployment::DeploymentPaths::current();
                let mut text = format!(
                    "Channel: {} ({})\nConfig source: {}\nLoaded Configuration Sources (Precedence Ascending):\n",
                    ctx.channel,
                    ctx.channel.binary_name(),
                    paths.user_config_file().display()
                );
                for s in &sources {
                    text.push_str(&format!("  • {}\n", s));
                }
                Ok(CliOutput::success(
                    text.trim_end().to_string(),
                    serde_json::json!({
                        "sources": sources,
                        "count": sources.len(),
                        "channel": ctx.channel.as_str(),
                        "config_source": paths.user_config_file(),
                    }),
                ))
            }

            RuntimeCommand::ConfigExplain { key } => {
                let resolved_config = match &self.config {
                    Some(cfg) => cfg.clone(),
                    None => {
                        let ws = self
                            .workspace_root
                            .clone()
                            .unwrap_or_else(|| std::path::PathBuf::from("."));
                        Arc::new(crate::config::ResolvedConfiguration::build_fallback(&ws))
                    }
                };

                match resolved_config.explain(&key) {
                    Some(explain) => {
                        let mut text = format!(
                            "Key: {}\nResolved Value: {}\nWinning Tier: {}\nSource File: {}\nImmutable: {}\nOverrides:\n",
                            explain.key,
                            explain.resolved_value,
                            explain.winning_tier,
                            explain
                                .source_file
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "builtin / in-memory".to_string()),
                            explain.is_immutable,
                        );
                        for o in &explain.overrides {
                            text.push_str(&format!(
                                "  - {}: {} (source: {})\n",
                                o.tier_name,
                                o.value,
                                o.source_file
                                    .as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|| "none".to_string())
                            ));
                        }
                        Ok(CliOutput::success(
                            text.trim_end().to_string(),
                            serde_json::to_value(&explain).unwrap_or_default(),
                        ))
                    }
                    None => Err(CliError::NotFound(format!(
                        "Configuration key not found: {key}"
                    ))),
                }
            }

            RuntimeCommand::InspectTelemetry {
                mission_id,
                summary,
                spans,
                metrics,
                export,
            } => {
                let m_id: MissionId = mission_id
                    .parse()
                    .map_err(|e| CliError::NotFound(format!("Invalid mission id: {e}")))?;

                let pool = self.pool.as_ref().ok_or_else(|| {
                    CliError::ExecutionFailed("Database connection pool not available".to_string())
                })?;

                let repo = crate::persistence::sqlite::repositories::SqliteTelemetryRepository::new(
                    pool.clone(),
                );
                let writer = crate::telemetry::stream::NdjsonStreamWriter::default();

                crate::telemetry::inspect_telemetry(
                    &repo,
                    &writer,
                    &m_id,
                    summary,
                    spans,
                    metrics,
                    export.as_deref(),
                )
                .await
                .map(CliOutput::from)
                .map_err(|e| CliError::ExecutionFailed(e.to_string()))
            }

            RuntimeCommand::RunEval {
                scenario,
                all: _,
                iterations: _,
                output,
            } => {
                let runner = crate::eval::runner::EvalRunner::new();
                let scorecard = if let Some(sc_id) = scenario {
                    let res = runner
                        .run_scenario(&sc_id)
                        .await
                        .map_err(CliError::ExecutionFailed)?;
                    crate::eval::scorecard::EvalScorecard::from_results(vec![res])
                } else {
                    runner.run_all().await
                };

                let format = output.as_deref().unwrap_or("markdown");
                let text = if format.eq_ignore_ascii_case("json") {
                    scorecard
                        .to_json()
                        .map_err(|e| CliError::ExecutionFailed(e.to_string()))?
                } else {
                    scorecard.to_markdown()
                };

                let data = serde_json::to_value(&scorecard)
                    .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

                let exit_code =
                    if scorecard.summary.failed > 0 || scorecard.summary.harness_errors > 0 {
                        1
                    } else {
                        0
                    };

                Ok(CliOutput::success(text, data).with_exit_code(exit_code))
            }
        }
    }
}

/// Default installation directory: the running executable's parent directory.
/// Honors platform conventions by never hardcoding `/usr/local/bin`; the
/// operator may override with `--install-dir` for user-local installs
/// without requiring root.
fn default_install_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Archive format inferred from filename (deployment artifact model).
fn archive_format(filename: &str) -> &str {
    if filename.ends_with(".zip") {
        "zip"
    } else {
        "tar.gz"
    }
}

/// Recursively insert a value into a TOML table following a dotted key path (e.g. `agents.default_model`).
pub fn insert_dotted_toml_value(
    root: &mut toml::Table,
    key: &str,
    val: toml::Value,
) -> Result<(), String> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.is_empty() || parts.iter().any(|p| p.trim().is_empty()) {
        return Err(format!("Invalid configuration key path: '{key}'"));
    }

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if i == parts.len() - 1 {
            current.insert(part.to_string(), val);
            return Ok(());
        }

        if !current.contains_key(part) {
            current.insert(part.to_string(), toml::Value::Table(toml::Table::new()));
        }

        let entry = current.get_mut(part).expect("key just checked or inserted");
        match entry {
            toml::Value::Table(tbl) => {
                current = tbl;
            }
            _ => {
                return Err(format!(
                    "Cannot set nested key '{key}': intermediate key '{part}' is not a table"
                ));
            }
        }
    }
    Ok(())
}
