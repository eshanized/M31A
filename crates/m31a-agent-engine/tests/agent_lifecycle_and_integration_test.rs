//! Integration tests for agent lifecycle states, unverified commit prevention, and verification-gated integration.

use m31a_execpolicy::ExecPolicy;
use m31a_provider::ProviderClient;
use m31a_storage::db::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::agent_runtime::{AgentRuntime, AgentSpawnRequest};
use m31a_tools::registry::ToolRegistry;
use m31a_trust::{TrustLevel, TrustStore};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

async fn create_test_nim_client(
    responses: Vec<&str>,
) -> (m31a_provider::LocalWireServer, ProviderClient) {
    let server = m31a_provider::LocalWireServer::start()
        .await
        .expect("start local wire server");
    for resp in responses {
        server.enqueue_text(resp);
    }
    let provider = m31a_provider::NimProvider::new(
        server.api_base(),
        "local-test-key".to_string(),
        "meta/llama-3.3-70b-instruct".to_string(),
    );
    (server, ProviderClient::from_provider(provider))
}

fn init_git_repo(path: &std::path::Path) {
    Command::new("git")
        .args(["init", "-b", "master"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@m31a.local"])
        .current_dir(path)
        .output()
        .unwrap();
    std::fs::write(path.join("README.md"), "# Initial Commit\n").unwrap();
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(path)
        .output()
        .unwrap();
}

#[tokio::test]
async fn test_worktree_patch_path_validation() {
    // 1. Directory traversal rejected
    let traversal_patch = "--- a/src/main.rs\n+++ b/../../etc/passwd\n@@ -1,1 +1,1 @@\n+hacked\n";
    let res = WorktreeManager::validate_patch_paths(traversal_patch);
    assert!(res.is_err(), "Expected traversal patch to be rejected");
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Directory traversal attempt rejected")
    );

    // 2. Sensitive path .ssh rejected
    let ssh_patch = "--- a/src/main.rs\n+++ b/.ssh/id_rsa\n@@ -1,1 +1,1 @@\n+key\n";
    let res2 = WorktreeManager::validate_patch_paths(ssh_patch);
    assert!(res2.is_err());
    assert!(
        res2.unwrap_err()
            .to_string()
            .contains("forbidden protected path")
    );

    // 3. Sensitive path .git/hooks rejected
    let git_patch = "--- a/src/main.rs\n+++ b/.git/hooks/pre-commit\n@@ -1,1 +1,1 @@\n+hook\n";
    let res3 = WorktreeManager::validate_patch_paths(git_patch);
    assert!(res3.is_err());
    assert!(
        res3.unwrap_err()
            .to_string()
            .contains("forbidden protected path")
    );

    // 4. Safe path accepted
    let safe_patch = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,1 +1,1 @@\n+// safe\n";
    let res4 = WorktreeManager::validate_patch_paths(safe_patch);
    assert!(res4.is_ok());
    assert_eq!(res4.unwrap(), vec![std::path::PathBuf::from("src/main.rs")]);
}

#[tokio::test]
async fn test_dirty_parent_working_tree_blocks_integration() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let mgr = WorktreeManager::new(WorktreeSettings::new(dir.path()));

    // Make repository dirty
    std::fs::write(dir.path().join("dirty.txt"), "uncommitted changes").unwrap();
    assert!(mgr.is_dirty(dir.path()).unwrap());

    let patch =
        "--- a/README.md\n+++ b/README.md\n@@ -1,1 +1,2 @@\n # Initial Commit\n+# Added line\n";
    let res = mgr.apply_and_commit_worktree_diff(patch, "feat: update", "test-agent", "coder");
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("parent repository working directory contains uncommitted changes")
    );
}

#[tokio::test]
async fn test_subagent_changes_do_not_auto_commit_without_explicit_integration() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let head_before = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let head_before_sha = String::from_utf8_lossy(&head_before.stdout)
        .trim()
        .to_string();

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let (_wire_server, client) =
        create_test_nim_client(vec!["Done with modifications."]).await;
    let worktree_mgr = std::sync::Arc::new(WorktreeManager::new(WorktreeSettings::new(dir.path())));
    let mut trust_store = TrustStore::new();
    trust_store
        .trust(dir.path(), TrustLevel::SessionOnly)
        .unwrap();

    let runtime = m31a_agent_engine::agents::EngineAgentRuntime::new(
        client,
        tools,
        storage,
        ExecPolicy::standard(),
        dir.path().to_path_buf(),
        None,
        128_000,
        3,
    )
    .with_worktree_manager(worktree_mgr)
    .with_trust_store(Arc::new(Mutex::new(trust_store)));

    let spawned = runtime
        .spawn_agent(AgentSpawnRequest {
            task_name: "modify file".to_string(),
            prompt: "Please modify README.md".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: true,
        })
        .await
        .unwrap();

    let wt_path = spawned.worktree_path.expect("worktree should be allocated");

    // Subagent modifies a file in its isolated worktree
    std::fs::write(
        wt_path.join("README.md"),
        "# Initial Commit\n# Child modification\n",
    )
    .unwrap();

    // Wait for agent to complete turn
    let outcomes = runtime
        .wait_agents(std::slice::from_ref(&spawned.agent_id), Some(5000))
        .await
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    // Crucial check: status is IntegrationPending, NOT automatically committed to parent!
    assert_eq!(outcomes[0].status, "integration_pending");

    // Check parent git HEAD has NOT changed
    let head_after = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let head_after_sha = String::from_utf8_lossy(&head_after.stdout)
        .trim()
        .to_string();
    assert_eq!(
        head_before_sha, head_after_sha,
        "Parent HEAD must NOT be auto-committed by child agent completion!"
    );
}

#[tokio::test]
async fn test_cancellation_requested_state_progression() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let (_wire_server, client) = create_test_nim_client(vec![]).await;
    let mut trust_store = TrustStore::new();
    trust_store
        .trust(dir.path(), TrustLevel::SessionOnly)
        .unwrap();

    let runtime = m31a_agent_engine::agents::EngineAgentRuntime::new(
        client,
        tools,
        storage,
        ExecPolicy::standard(),
        dir.path().to_path_buf(),
        None,
        128_000,
        3,
    )
    .with_trust_store(Arc::new(Mutex::new(trust_store)));

    let spawned = runtime
        .spawn_agent(AgentSpawnRequest {
            task_name: "long task".to_string(),
            prompt: "Run forever".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: false,
        })
        .await
        .unwrap();

    // Request closure
    let close_res = runtime.close_agent(&spawned.agent_id).await.unwrap();
    assert!(close_res.contains("cancellation signalled"));

    // Wait for the cancellation to take effect
    let outcomes = runtime
        .wait_agents(std::slice::from_ref(&spawned.agent_id), Some(2000))
        .await
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status, "cancelled");
}

#[tokio::test]
async fn test_review_engine_gates_integration_on_critical_finding() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let (_wire_server, client) = create_test_nim_client(vec!["Done."]).await;
    let worktree_mgr = std::sync::Arc::new(WorktreeManager::new(WorktreeSettings::new(dir.path())));
    let mut trust_store = TrustStore::new();
    trust_store
        .trust(dir.path(), TrustLevel::SessionOnly)
        .unwrap();

    let runtime = m31a_agent_engine::agents::EngineAgentRuntime::new(
        client,
        tools,
        storage,
        ExecPolicy::standard(),
        dir.path().to_path_buf(),
        None,
        128_000,
        3,
    )
    .with_worktree_manager(worktree_mgr)
    .with_trust_store(Arc::new(Mutex::new(trust_store)));

    let spawned = runtime
        .spawn_agent(AgentSpawnRequest {
            task_name: "add sensitive secret".to_string(),
            prompt: "Commit secret".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: true,
        })
        .await
        .unwrap();

    let wt_path = spawned.worktree_path.expect("worktree should be allocated");

    // Subagent commits a hardcoded secret in production file
    std::fs::write(
        wt_path.join("README.md"),
        "# Initial Commit\nlet api_key = \"ghp_123456789012345678901234567890123456\";\n",
    )
    .unwrap();

    // Wait for agent to finish turn
    let outcomes = runtime
        .wait_agents(std::slice::from_ref(&spawned.agent_id), Some(5000))
        .await
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status, "integration_pending");

    // Explicit integration attempt must be REJECTED by Code Review Engine
    let integrate_res = runtime.integrate_agent(&spawned.agent_id, None).await;
    assert!(
        integrate_res.is_err(),
        "Expected integration to be rejected by code review"
    );
    let err_str = integrate_res.unwrap_err().to_string();
    assert!(
        err_str.contains("Integration rejected by Code Review Engine")
            || err_str.contains("Hardcoded secret"),
        "Expected review rejection error, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_recursive_grandchild_spawn_and_depth_limiting() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let (_wire_server, client) = create_test_nim_client(vec![
        "Child response",
        "Grandchild response",
        "Great-grandchild response",
    ])
    .await;
    let mut trust_store = TrustStore::new();
    trust_store
        .trust(dir.path(), TrustLevel::SessionOnly)
        .unwrap();

    let runtime = Arc::new(
        m31a_agent_engine::agents::EngineAgentRuntime::new(
            client,
            tools.clone(),
            storage.clone(),
            ExecPolicy::standard(),
            dir.path().to_path_buf(),
            None,
            128_000,
            3, // Max depth = 3
        )
        .with_trust_store(Arc::new(Mutex::new(trust_store))),
    );
    // Wire runtime into tools
    tools.set_agent_runtime(runtime.clone());

    // 1. Spawn root child (depth 1)
    let child = runtime
        .spawn_agent(AgentSpawnRequest {
            task_name: "level 1 child".to_string(),
            prompt: "Do work at level 1".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: false,
        })
        .await
        .unwrap();

    // 2. Spawn grandchild under child scope (depth 2)
    let grandchild = m31a_tools::agent_runtime::CALLER_AGENT_ID
        .scope(child.agent_id.clone(), async {
            runtime
                .spawn_agent(AgentSpawnRequest {
                    task_name: "level 2 grandchild".to_string(),
                    prompt: "Do work at level 2".to_string(),
                    role_name: "coder".to_string(),
                    model_override: None,
                    depth: 2,
                    max_depth: 3,
                    isolate_worktree: false,
                })
                .await
        })
        .await
        .unwrap();

    // 3. Spawn great-grandchild under grandchild scope (depth 3)
    let great_grandchild = m31a_tools::agent_runtime::CALLER_AGENT_ID
        .scope(grandchild.agent_id.clone(), async {
            runtime
                .spawn_agent(AgentSpawnRequest {
                    task_name: "level 3 great-grandchild".to_string(),
                    prompt: "Do work at level 3".to_string(),
                    role_name: "coder".to_string(),
                    model_override: None,
                    depth: 3,
                    max_depth: 3,
                    isolate_worktree: false,
                })
                .await
        })
        .await
        .unwrap();

    // 4. Attempt to spawn beyond max depth (depth 4 > max_depth 3) -> MUST FAIL
    let over_limit = m31a_tools::agent_runtime::CALLER_AGENT_ID
        .scope(great_grandchild.agent_id.clone(), async {
            runtime
                .spawn_agent(AgentSpawnRequest {
                    task_name: "level 4 over limit".to_string(),
                    prompt: "This must fail".to_string(),
                    role_name: "coder".to_string(),
                    model_override: None,
                    depth: 4,
                    max_depth: 3,
                    isolate_worktree: false,
                })
                .await
        })
        .await;

    assert!(
        over_limit.is_err(),
        "Expected spawn exceeding max_depth to be rejected"
    );
    assert!(
        over_limit
            .unwrap_err()
            .to_string()
            .contains("depth limit 3 exceeded")
    );
}

#[tokio::test]
async fn test_terminate_tree_report_truthfulness() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let (_wire_server, client) = create_test_nim_client(vec![]).await;
    let mut trust_store = TrustStore::new();
    trust_store
        .trust(dir.path(), TrustLevel::SessionOnly)
        .unwrap();

    let runtime = Arc::new(
        m31a_agent_engine::agents::EngineAgentRuntime::new(
            client,
            tools.clone(),
            storage.clone(),
            ExecPolicy::standard(),
            dir.path().to_path_buf(),
            None,
            128_000,
            3,
        )
        .with_trust_store(Arc::new(Mutex::new(trust_store))),
    );
    tools.set_agent_runtime(runtime.clone());

    // Spawn child
    let child = runtime
        .spawn_agent(AgentSpawnRequest {
            task_name: "parent task".to_string(),
            prompt: "Do work".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: false,
        })
        .await
        .unwrap();

    // Spawn grandchild
    let grandchild = m31a_tools::agent_runtime::CALLER_AGENT_ID
        .scope(child.agent_id.clone(), async {
            runtime
                .spawn_agent(AgentSpawnRequest {
                    task_name: "child task".to_string(),
                    prompt: "Do child work".to_string(),
                    role_name: "coder".to_string(),
                    model_override: None,
                    depth: 2,
                    max_depth: 3,
                    isolate_worktree: false,
                })
                .await
        })
        .await
        .unwrap();

    // Terminate the entire tree starting at child
    let report = runtime.terminate_tree(&child.agent_id).await.unwrap();

    assert_eq!(report.root_id, child.agent_id);
    assert!(report.is_complete, "Termination report should be complete");
    assert!(
        report.terminated_agents.contains(&child.agent_id),
        "Child agent should be terminated"
    );
    assert!(
        report.terminated_agents.contains(&grandchild.agent_id),
        "Grandchild agent should be terminated"
    );
    assert!(
        report.timed_out_agents.is_empty(),
        "No agents should have timed out"
    );
    assert!(
        report.failed_agents.is_empty(),
        "No agents should have failed"
    );
}

#[tokio::test]
async fn test_integration_verification_fails_closed_on_unsupported_project() {
    let dir = tempdir().unwrap();
    init_git_repo(dir.path());

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let (_wire_server, client) = create_test_nim_client(vec!["Done."]).await;
    let worktree_mgr = std::sync::Arc::new(WorktreeManager::new(WorktreeSettings::new(dir.path())));
    let mut trust_store = TrustStore::new();
    trust_store
        .trust(dir.path(), TrustLevel::SessionOnly)
        .unwrap();

    let runtime = m31a_agent_engine::agents::EngineAgentRuntime::new(
        client,
        tools,
        storage,
        ExecPolicy::standard(),
        dir.path().to_path_buf(),
        None,
        128_000,
        3,
    )
    .with_worktree_manager(worktree_mgr)
    .with_trust_store(Arc::new(Mutex::new(trust_store)));

    let spawned = runtime
        .spawn_agent(AgentSpawnRequest {
            task_name: "modify unknown project".to_string(),
            prompt: "Modify markdown".to_string(),
            role_name: "coder".to_string(),
            model_override: None,
            depth: 1,
            max_depth: 3,
            isolate_worktree: true,
        })
        .await
        .unwrap();

    let wt_path = spawned.worktree_path.expect("worktree should be allocated");

    // Modify a non-build-manifest file in repository without Cargo.toml
    std::fs::write(
        wt_path.join("README.md"),
        "# Initial Commit\n# Additional notes\n",
    )
    .unwrap();

    let outcomes = runtime
        .wait_agents(std::slice::from_ref(&spawned.agent_id), Some(5000))
        .await
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status, "integration_pending");

    // Integration must fail-closed because repository has no Cargo.toml, package.json, etc.
    let integrate_res = runtime.integrate_agent(&spawned.agent_id, None).await;
    assert!(integrate_res.is_err());
    let err_msg = integrate_res.unwrap_err().to_string();
    assert!(
        err_msg.contains("VerificationIncomplete") && err_msg.contains("unsupported project type"),
        "Expected VerificationIncomplete error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_tool_registry_subagent_tools_fail_closed_without_runtime() {
    let registry = ToolRegistry::new();
    let cwd = std::env::current_dir().unwrap();

    // 1. spawn_agent
    let spawn_res = registry
        .execute(
            "spawn_agent",
            r#"{"task_name": "Test", "prompt": "Test"}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(spawn_res.is_err());
    assert!(
        spawn_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );

    // 2. send_input
    let send_res = registry
        .execute(
            "send_input",
            r#"{"agent_id": "test", "message": "hello"}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(send_res.is_err());
    assert!(
        send_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );

    // 3. wait_agent
    let wait_res = registry
        .execute(
            "wait_agent",
            r#"{"agent_ids": ["test"]}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(wait_res.is_err());
    assert!(
        wait_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );

    // 4. close_agent
    let close_res = registry
        .execute(
            "close_agent",
            r#"{"agent_id": "test"}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(close_res.is_err());
    assert!(
        close_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );

    // 5. terminate_agent_tree
    let term_res = registry
        .execute(
            "terminate_agent_tree",
            r#"{"agent_id": "test"}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(term_res.is_err());
    assert!(
        term_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );

    // 6. integrate_agent
    let int_res = registry
        .execute(
            "integrate_agent",
            r#"{"agent_id": "test"}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(int_res.is_err());
    assert!(
        int_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );

    // 7. list_agents
    let list_res = registry
        .execute(
            "list_agents",
            r#"{}"#,
            &cwd,
            m31a_core::SandboxPolicy::WorkspaceWrite,
        )
        .await;
    assert!(list_res.is_err());
    assert!(
        list_res
            .unwrap_err()
            .to_string()
            .contains("AgentRuntime unavailable")
    );
}
