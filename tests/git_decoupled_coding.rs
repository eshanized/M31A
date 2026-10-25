//! Integration tests verifying that autonomous coding is fully decoupled from Git.
//!
//! Architectural requirements:
//! 1. Ordinary directories without `.git` run missions to completion under default configuration.
//! 2. No `.git` directory is created at any stage of execution.
//! 3. Explicitly required isolation fails closed when `.git` is missing.
//! 4. Contradictory configuration (git disabled + isolation required) fails validation.
//! 5. Git disabled mode allows full autonomous coding execution.
//! 6. Doctor probes report non-mandatory warnings or disabled state rather than blocking failures for missing Git.

use std::sync::Arc;
use tempfile::tempdir;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::config::schema::{AppConfig, GitExecutionIsolation};
use m31a::init::doctor::{DiagnosticStatus, DoctorEngine};
use m31a::runtime::AppRuntime;

#[derive(Clone)]
struct DecoupledTestModel {
    plan_json: String,
}

impl Default for DecoupledTestModel {
    fn default() -> Self {
        let plan_json = serde_json::json!({
            "tasks": [{
                "id": "TASK-01",
                "title": "Survey repository structure",
                "description": "Read-only survey",
                "role": "researcher",
                "depends_on": [],
                "required_capabilities": ["fs.read"]
            }]
        })
        .to_string();
        Self { plan_json }
    }
}

#[async_trait::async_trait]
impl ModelCaller for DecoupledTestModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: self.plan_json.clone(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        let mut text = compiled.system_prompt.clone();
        for m in &compiled.messages {
            text.push_str(&format!("{:?}", m));
        }
        if text.contains("Goal to Decompose") || text.contains("planner") || text.contains("Plan") {
            Ok(ModelProposal::Complete {
                summary: self.plan_json.clone(),
                artifacts: vec![],
            })
        } else {
            Ok(ModelProposal::Complete {
                summary: "Task complete".to_string(),
                artifacts: vec![],
            })
        }
    }

    fn bind_role_tools(
        &self,
        _role: &m31a::state_machine::agent::AgentRole,
        _tools: Vec<serde_json::Value>,
    ) -> Option<std::sync::Arc<dyn ModelCaller>> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

/// A fresh workspace directory with no `.git` runs a mission successfully under default config.
/// No `.git` directory is created.
#[tokio::test]
async fn test_fresh_directory_without_git_runs_to_completion() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    assert!(
        !ws.join(".git").exists(),
        "Initial workspace must not have .git"
    );

    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init on non-git directory")
        .with_model_caller(Arc::new(DecoupledTestModel::default()));

    let res = runtime
        .run_mission("fix typo in README", Some("autonomous"), false)
        .await;

    assert!(
        res.is_ok(),
        "Mission in non-git directory must succeed: {:?}",
        res.err()
    );

    // Verify git was never artificially initialized
    assert!(
        !ws.join(".git").exists(),
        "Core runtime must never fabricate or initialize a .git repository"
    );
}

/// Explicitly required isolation must fail closed when .git is missing.
#[tokio::test]
async fn test_explicit_required_isolation_fails_closed_without_git() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    let dot_m31a = ws.join(".m31a");
    tokio::fs::create_dir_all(&dot_m31a).await.unwrap();
    let toml_str = r#"[git]
execution_isolation = "required"
"#;
    let md = m31a::config::migrate::convert_config_toml_to_markdown(toml_str).unwrap();
    tokio::fs::write(dot_m31a.join("config.md"), md)
        .await
        .unwrap();

    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init")
        .with_model_caller(Arc::new(DecoupledTestModel::default()));

    let res = runtime
        .run_mission("fix typo in README", Some("autonomous"), false)
        .await;

    assert!(
        res.is_err(),
        "Must block execution when isolation is required and .git is missing"
    );
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("isolation required by policy") && err.contains("not a git repository"),
        "Error must explain isolation requirement: {}",
        err
    );
}

/// Contradictory configuration (git.enabled = false + execution_isolation = "required")
/// fails validation.
#[test]
fn test_contradictory_git_configuration_fails_validation() {
    let mut config = AppConfig::default();
    config.git.enabled = false;
    config.git.execution_isolation = GitExecutionIsolation::Required;

    let res = m31a::config::schema::validate_config(&config);
    assert!(res.is_err(), "Contradictory config must fail validation");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("contradictory"),
        "Error message must clearly identify contradiction: {}",
        err_msg
    );
}

/// Disabling Git completely allows normal coding missions in filesystem mode.
#[tokio::test]
async fn test_git_disabled_mode_operates_in_filesystem_mode() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    let dot_m31a = ws.join(".m31a");
    tokio::fs::create_dir_all(&dot_m31a).await.unwrap();
    let toml_str = r#"[git]
enabled = false
execution_isolation = "best_effort"
"#;
    let md = m31a::config::migrate::convert_config_toml_to_markdown(toml_str).unwrap();
    tokio::fs::write(dot_m31a.join("config.md"), md)
        .await
        .unwrap();

    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init with git disabled")
        .with_model_caller(Arc::new(DecoupledTestModel::default()));

    let res = runtime
        .run_mission("fix typo in README", Some("autonomous"), false)
        .await;

    assert!(
        res.is_ok(),
        "Mission with git disabled must succeed in filesystem mode: {:?}",
        res.err()
    );
    assert!(!ws.join(".git").exists(), "No .git should exist");
}

/// Doctor diagnostics on a non-git directory do not produce blocking failures.
#[test]
fn test_doctor_diagnostics_on_non_git_workspace_do_not_block() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    let doctor = DoctorEngine::new();
    let probes = doctor.run_all(&ws);

    // Repository check should be Warn, not Fail, and not mandatory
    let repo_probe = probes.iter().find(|p| p.id == "git_repository").unwrap();
    assert_eq!(repo_probe.status, DiagnosticStatus::Warn);
    assert!(!repo_probe.is_mandatory);

    // No blocking failures
    assert!(
        !doctor.has_blocking_failures(&probes),
        "A directory without git must not have blocking diagnostic failures"
    );
}

/// Doctor diagnostics with git disabled mark probes as Disabled.
#[test]
fn test_doctor_diagnostics_with_git_disabled() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    let doctor = DoctorEngine::new();
    let probes = doctor.run_all_with_git_enabled(&ws, false);

    let git_installed = probes.iter().find(|p| p.id == "git_installed").unwrap();
    assert_eq!(git_installed.status, DiagnosticStatus::Disabled);

    let git_repo = probes.iter().find(|p| p.id == "git_repository").unwrap();
    assert_eq!(git_repo.status, DiagnosticStatus::Disabled);

    assert!(!doctor.has_blocking_failures(&probes));
}

/// When isolation is best_effort on a git repository and worktree execution is bypassed or unavailable,
/// pre-existing untracked files and uncommitted tracked modifications in the primary workspace
/// are strictly protected from being staged or committed. Automatic Git finalization is Skipped.
#[tokio::test]
async fn test_primary_workspace_mutation_safety_untracked_files_preserved() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    // 1. Initialize a real git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&ws)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&ws)
        .output()
        .expect("git config user.name");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&ws)
        .output()
        .expect("git config user.email");

    // Create an initial tracked file and commit it
    let tracked_file = ws.join("tracked.txt");
    tokio::fs::write(&tracked_file, "initial tracked content")
        .await
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&ws)
        .output()
        .expect("git add tracked.txt");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(&ws)
        .output()
        .expect("git commit");

    // 2. Introduce dirty state in primary workspace
    // - Modify tracked file (uncommitted modification)
    tokio::fs::write(&tracked_file, "modified uncommitted tracked content")
        .await
        .unwrap();
    // - Create an untracked file (user work-in-progress)
    let untracked_file = ws.join("untracked_notes.txt");
    tokio::fs::write(&untracked_file, "sensitive operator notes - do not commit")
        .await
        .unwrap();

    // Create a blocker file so worktree directory creation fails, forcing
    // best_effort isolation to downgrade to primary workspace execution.
    let blocker = ws.join("worktree_blocker_file");
    tokio::fs::write(&blocker, "regular file").await.unwrap();

    // Configure best_effort isolation with invalid worktree_dir
    let dot_m31a = ws.join(".m31a");
    tokio::fs::create_dir_all(&dot_m31a).await.unwrap();
    let toml_str = format!(
        r#"[git]
enabled = true
execution_isolation = "best_effort"
auto_commit = true
worktree_dir = "{}"
"#,
        blocker.display()
    );
    let md = m31a::config::migrate::convert_config_toml_to_markdown(&toml_str).unwrap();
    tokio::fs::write(dot_m31a.join("config.md"), md)
        .await
        .unwrap();

    // Initialize runtime and run mission with DecoupledTestModel
    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init on git workspace")
        .with_model_caller(Arc::new(DecoupledTestModel::default()));

    let summary = runtime
        .run_mission(
            "inspect repository without committing user files",
            Some("autonomous"),
            false,
        )
        .await
        .expect("mission must succeed under best_effort fallback");

    // 3. Assertions
    // Pre-existing untracked file must remain untracked and unchanged
    assert_eq!(
        tokio::fs::read_to_string(&untracked_file).await.unwrap(),
        "sensitive operator notes - do not commit"
    );
    // Pre-existing modified tracked file must keep dirty modification
    assert_eq!(
        tokio::fs::read_to_string(&tracked_file).await.unwrap(),
        "modified uncommitted tracked content"
    );

    // Git status should show untracked_notes.txt is still untracked (not staged)
    let git_status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&ws)
        .output()
        .expect("git status");
    let status_str = String::from_utf8_lossy(&git_status.stdout);
    assert!(
        status_str.contains("?? untracked_notes.txt"),
        "Untracked file must remain untracked: {status_str}"
    );

    // Git finalization must be reported as Skipped with the explicit protection reason
    let git_fin = summary
        .git_finalization
        .expect("git finalization summary must be present");
    match git_fin {
        m31a::runtime::GitFinalizationSummary::Skipped { reason } => {
            assert!(
                reason.contains("Automatic Git finalization skipped")
                    && reason.contains("pre-existing workspace files protected"),
                "Expected protection skip reason, got: {reason}"
            );
        }
        other => panic!("Expected Skipped summary on fallback, got: {other:?}"),
    }
}

/// Runtime Git reconfiguration atomically toggles Git capability availability in CapabilityRegistry.
#[tokio::test]
async fn test_runtime_git_reconfiguration_atomic_capability_sync() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init")
        .with_model_caller(Arc::new(DecoupledTestModel::default()));

    // Initial state: default config has git.enabled = true
    let caps = runtime.capability_registry();
    assert!(
        caps.is_available(&m31a::capability::CapabilityFamily::Git),
        "Git capability should be available by default"
    );

    // 1. Reconfigure runtime: disable Git
    let mut disabled_config = (**runtime.config()).clone();
    disabled_config.app_config.git.enabled = false;
    let reconfigured_runtime = runtime.with_config(Arc::new(disabled_config));

    let updated_caps = reconfigured_runtime.capability_registry();
    assert!(
        !updated_caps.is_available(&m31a::capability::CapabilityFamily::Git),
        "Git capability must become unavailable when git.enabled = false"
    );

    // 2. Reconfigure runtime: re-enable Git
    let mut enabled_config = (**reconfigured_runtime.config()).clone();
    enabled_config.app_config.git.enabled = true;
    let re_enabled_runtime = reconfigured_runtime.with_config(Arc::new(enabled_config));

    let re_enabled_caps = re_enabled_runtime.capability_registry();
    assert!(
        re_enabled_caps.is_available(&m31a::capability::CapabilityFamily::Git),
        "Git capability must become available again when git.enabled = true"
    );
}
