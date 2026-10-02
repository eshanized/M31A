//! Integration tests proving isolation policy enforcement at the authority boundary (Findings F & G).
//!
//! Non-negotiable invariants:
//! 1. required + worktree unavailable (missing .git) -> execution blocked (fails closed)
//! 2. best_effort + worktree unavailable -> explicit compatibility fallback
//! 3. required + worktree available -> executes in isolated worktree

use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::events::bus::BroadcastEventBus;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;

struct IsolationTestModel {
    plan_json: String,
}

impl Default for IsolationTestModel {
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
impl ModelCaller for IsolationTestModel {
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
}

fn init_git_repo(path: &std::path::Path) {
    let _ = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(path)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.name", "Security Tester"])
        .current_dir(path)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "security@m31a.local"])
        .current_dir(path)
        .status();
    std::fs::write(path.join("README.md"), "# Test\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(path)
        .status();
}

#[tokio::test]
async fn test_isolation_required_blocks_when_git_missing() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    // Default configuration has execution_isolation = "required"
    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init")
        .with_model_caller(Arc::new(IsolationTestModel::default()));

    let res = runtime
        .run_mission("Test unisolated execution", Some("autonomous"), false)
        .await;

    assert!(
        res.is_err(),
        "Execution must fail closed when isolation is required and git is missing"
    );
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("isolation required by policy")
            && err_str.contains("not a git repository"),
        "Error must explicitly explain isolation policy failure: {}",
        err_str
    );
}

#[tokio::test]
async fn test_isolation_best_effort_allows_fallback_when_git_missing() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();

    let db_path = ws.join("m31a.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(1024));

    let dot_m31a = ws.join(".m31a");
    tokio::fs::create_dir_all(&dot_m31a).await.unwrap();
    tokio::fs::write(
        dot_m31a.join("config.toml"),
        r#"[git]
execution_isolation = "best_effort"
"#,
    )
    .await
    .unwrap();

    let resolved_cfg = Arc::new(
        m31a::config::resolved::ResolvedConfigBuilder::new(&ws)
            .build()
            .unwrap(),
    );

    let runtime = AppRuntime::from_pool_workspace_and_config(pool, ws, bus, resolved_cfg)
        .await
        .unwrap()
        .with_model_caller(Arc::new(IsolationTestModel::default()));

    // Under best_effort, failure to create worktree does NOT block execution at Step 3
    let res = runtime
        .run_mission("fix typo in README", Some("autonomous"), false)
        .await;

    // The mission proceeds past isolation check to mission execution
    assert!(
        res.is_ok(),
        "Best effort fallback must allow execution in primary workspace: {:?}",
        res.err()
    );
}

#[tokio::test]
async fn test_isolation_required_succeeds_in_isolated_worktree() {
    let temp = tempdir().unwrap();
    let ws = temp.path().to_path_buf();
    init_git_repo(&ws);

    let runtime = AppRuntime::new(&ws)
        .await
        .expect("runtime init")
        .with_model_caller(Arc::new(IsolationTestModel::default()));

    let res = runtime
        .run_mission("fix typo in README", Some("autonomous"), false)
        .await;

    assert!(
        res.is_ok(),
        "Execution must succeed when worktree isolation is created: {:?}",
        res.err()
    );
}
