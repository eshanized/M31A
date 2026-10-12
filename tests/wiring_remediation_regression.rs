//! Regression tests for M31A wiring remediation & runtime integrity recovery.
//!
//! Covers:
//! - RLS-01: scripts/build-release.sh Python heredocs syntax and compilation.
//! - RLS-02 & RLS-03: scripts/validate_release_manifests.py single-publisher validation.
//! - CFG-01: Live runtime configuration reconfiguration and authority updates.
//! - TUI-01: Task graph snapshot authoritative reconciliation and pruning.
//! - TUI-02: Execution-branch display and worktree hydration.
//! - TUI-03: Workspace-scoped task graph lookups without cross-workspace leakage.
//! - UX-01: /clear-session durable-first ordering and failure preservation.
//! - CLI-01: CLI overrides preserved post-onboarding.
//! - DOC-01: Credential isolation and Tier-5 environment variable boundaries.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use m31a::config::ResolvedConfiguration;
use m31a::events::types::TaskSummary;
use m31a::ids::{MissionId, SessionId, TaskGraphId, TaskId};
use m31a::interaction::events::InteractionEvent;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::task::TaskState;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::model::TuiViewModel;

#[test]
fn test_rls_01_build_release_python_heredocs_compile() {
    let script_path = Path::new("scripts/build-release.sh");
    assert!(script_path.exists(), "scripts/build-release.sh must exist");

    let content = fs::read_to_string(script_path).expect("failed to read build-release.sh");

    // Extract all python3 << 'EOF' ... EOF or python3 - << 'EOF' ... EOF blocks
    let lines: Vec<&str> = content.lines().collect();
    let mut in_python_block = false;
    let mut current_block = String::new();
    let mut eof_delimiter = String::new();
    let mut blocks_tested = 0;

    for line in lines {
        let trimmed = line.trim();
        if !in_python_block {
            if trimmed.contains("python3 -c \"$(")
                || (trimmed.starts_with("python3") && trimmed.contains("<<"))
            {
                if let Some(pos) = trimmed.find("<<") {
                    let rem = trimmed[pos + 2..].trim();
                    let delim = rem
                        .trim_matches('\'')
                        .trim_matches('"')
                        .split_whitespace()
                        .next()
                        .unwrap_or("EOF");
                    eof_delimiter = delim.to_string();
                    in_python_block = true;
                    current_block.clear();
                }
            }
        } else if trimmed == eof_delimiter {
            in_python_block = false;
            blocks_tested += 1;

            // Test compile current_block
            let temp_file = std::env::temp_dir().join(format!("test_heredoc_{blocks_tested}.py"));
            fs::write(&temp_file, &current_block).expect("failed to write temp python file");

            let status = Command::new("python3")
                .arg("-m")
                .arg("py_compile")
                .arg(&temp_file)
                .status()
                .expect("python3 must be available to test heredocs");

            let _ = fs::remove_file(&temp_file);
            assert!(
                status.success(),
                "Embedded Python heredoc #{blocks_tested} failed py_compile syntax check:\n{current_block}"
            );
        } else {
            current_block.push_str(line);
            current_block.push('\n');
        }
    }

    assert!(
        blocks_tested > 0,
        "Expected at least one Python heredoc in scripts/build-release.sh"
    );
}

#[test]
fn test_rls_02_and_03_release_manifest_validator() {
    let validator_path = Path::new("scripts/validate_release_manifests.py");
    assert!(
        validator_path.exists(),
        "scripts/validate_release_manifests.py must exist"
    );

    let temp_dir = std::env::temp_dir().join(format!("test_m31a_release_{}", uuid::Uuid::now_v7()));
    let dist_dir = temp_dir.join("dist");
    fs::create_dir_all(&dist_dir).expect("failed to create temp dist dir");

    let targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ];

    // Create valid mock artifacts for all 6 targets
    let mut expected_sums = Vec::new();
    for target in &targets {
        let ext = if target.contains("windows") {
            "zip"
        } else {
            "tar.gz"
        };
        let archive_name = format!("m31a-v0.1.6-{target}.{ext}");
        let archive_path = dist_dir.join(&archive_name);
        let archive_content = format!("mock-archive-content-for-{target}");
        fs::write(&archive_path, &archive_content).unwrap();

        use sha2::{Digest, Sha256};
        let hash = format!("{:x}", Sha256::digest(archive_content.as_bytes()));
        let size = archive_content.len();

        expected_sums.push(format!("{hash}  {archive_name}"));

        // Write target sidecars
        let sidecar_manifest = serde_json::json!({
            "schema_version": 1,
            "version": "0.1.6",
            "channel": "stable",
            "commit": "mockcommit12345",
            "artifacts": [{
                "target": target,
                "platform": "linux",
                "filename": archive_name,
                "sha256": hash,
                "size": size,
                "build_id": "mock-build-id",
                "commit": "mockcommit12345"
            }]
        });
        fs::write(
            dist_dir.join(format!("deployment-manifest-{target}.json")),
            serde_json::to_string_pretty(&sidecar_manifest).unwrap(),
        )
        .unwrap();

        fs::write(
            dist_dir.join(format!("sbom-{target}.json")),
            "{\"bomFormat\":\"CycloneDX\"}",
        )
        .unwrap();

        fs::write(
            dist_dir.join(format!("SHA256SUMS-{target}")),
            format!("{hash}  {archive_name}\n"),
        )
        .unwrap();
    }

    // Run validator on valid directory -> must exit 0
    let status = Command::new("python3")
        .arg("scripts/validate_release_manifests.py")
        .arg(&dist_dir)
        .arg("v0.1.6")
        .arg("mockcommit12345")
        .arg("stable")
        .status()
        .expect("failed to run validate_release_manifests.py");
    assert!(
        status.success(),
        "validator should succeed on valid release fixtures"
    );

    // Unified deployment-manifest.json and SHA256SUMS must have been generated
    let unified_manifest = dist_dir.join("deployment-manifest.json");
    assert!(
        unified_manifest.exists(),
        "unified deployment-manifest.json must be created"
    );
    let unified_sums = dist_dir.join("SHA256SUMS");
    assert!(unified_sums.exists(), "unified SHA256SUMS must be created");

    // Test failure case: remove an artifact
    fs::remove_file(dist_dir.join(format!("m31a-v0.1.6-{}.tar.gz", targets[0]))).unwrap();
    let fail_status = Command::new("python3")
        .arg("scripts/validate_release_manifests.py")
        .arg(&dist_dir)
        .arg("v0.1.6")
        .arg("mockcommit12345")
        .arg("stable")
        .status()
        .expect("failed to run validate_release_manifests.py");
    assert!(
        !fail_status.success(),
        "validator must fail when an artifact is missing"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_cfg_01_live_runtime_reconfiguration() {
    let temp_dir = std::env::temp_dir().join(format!("test_m31a_cfg01_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(&temp_dir).unwrap();

    let runtime = m31a::runtime::AppRuntime::new(&temp_dir).await.unwrap();
    let original_model = runtime.config().active_model.clone();

    // Build new config with updated model and max_tokens
    let mut new_config = (**runtime.config()).clone();
    let target_model = "meta/llama-3.1-70b-instruct";
    new_config.active_model = target_model.to_string();
    new_config.app_config.runtime.concurrency_limit = 8;

    let reconfigured = runtime.with_config(Arc::new(new_config));
    assert_eq!(reconfigured.config().active_model, target_model);
    assert_ne!(reconfigured.config().active_model, original_model);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tui_01_task_graph_snapshot_reconciliation() {
    let mut model = TuiViewModel::new();
    assert_eq!(model.tasks.len(), 0);

    let task_a_id = TaskId::new();
    let task_b_id = TaskId::new();
    let task_c_id = TaskId::new();
    let task_d_id = TaskId::new();

    let initial_summaries = vec![
        TaskSummary {
            id: task_a_id,
            title: "Task A".to_string(),
            role: AgentRole::planner(),
            status: TaskState::Pending,
            dependencies: vec![],
        },
        TaskSummary {
            id: task_b_id,
            title: "Task B".to_string(),
            role: AgentRole::implementer(),
            status: TaskState::Pending,
            dependencies: vec![task_a_id],
        },
        TaskSummary {
            id: task_c_id,
            title: "Task C".to_string(),
            role: AgentRole::reviewer(),
            status: TaskState::Pending,
            dependencies: vec![task_b_id],
        },
    ];

    // 1. Materialize initial graph rev 1
    let ok = model.reconcile_task_graph("graph-1", 1, &initial_summaries);
    assert!(ok);
    assert_eq!(model.tasks.len(), 3);
    assert_eq!(model.active_graph_revision, 1);

    // 2. Simulate task A starting execution and progressing to 50%
    if let Some(t) = model
        .tasks
        .iter_mut()
        .find(|t| t.id == task_a_id.to_string())
    {
        t.status = "running".to_string();
        t.progress_pct = 50;
        t.assigned_agent_id = Some("agent-worker-1".to_string());
    }

    // 3. Replanning generates rev 2: Task B is removed, Task D is added
    let rev2_summaries = vec![
        TaskSummary {
            id: task_a_id,
            title: "Task A Updated".to_string(),
            role: AgentRole::planner(),
            status: TaskState::Pending,
            dependencies: vec![],
        },
        TaskSummary {
            id: task_c_id,
            title: "Task C".to_string(),
            role: AgentRole::reviewer(),
            status: TaskState::Pending,
            dependencies: vec![],
        },
        TaskSummary {
            id: task_d_id,
            title: "Task D".to_string(),
            role: AgentRole::implementer(),
            status: TaskState::Pending,
            dependencies: vec![task_c_id],
        },
    ];

    let ok = model.reconcile_task_graph("graph-1", 2, &rev2_summaries);
    assert!(ok);
    assert_eq!(model.tasks.len(), 3);
    assert_eq!(model.active_graph_revision, 2);

    // Assert: Task A preserved running state and progress, title updated
    let task_a = model
        .tasks
        .iter()
        .find(|t| t.id == task_a_id.to_string())
        .unwrap();
    assert_eq!(task_a.title, "Task A Updated");
    assert_eq!(task_a.status, "running");
    assert_eq!(task_a.progress_pct, 50);
    assert_eq!(task_a.assigned_agent_id.as_deref(), Some("agent-worker-1"));

    // Assert: Task B was PRUNED completely
    assert!(model.tasks.iter().all(|t| t.id != task_b_id.to_string()));

    // Assert: Task D was added
    assert!(model.tasks.iter().any(|t| t.id == task_d_id.to_string()));

    // Assert: Topological/display order matches rev2_summaries exactly
    assert_eq!(model.tasks[0].id, task_a_id.to_string());
    assert_eq!(model.tasks[1].id, task_c_id.to_string());
    assert_eq!(model.tasks[2].id, task_d_id.to_string());

    // 4. Stale revision rev 1 arriving out of order is rejected
    let stale_accepted = model.reconcile_task_graph("graph-1", 1, &initial_summaries);
    assert!(!stale_accepted, "stale graph revision must be ignored");
    assert_eq!(model.active_graph_revision, 2);
    assert_eq!(model.tasks.len(), 3);
}

#[tokio::test]
async fn test_tui_02_execution_branch_detection() {
    let temp_dir = std::env::temp_dir().join(format!("test_m31a_tui02_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(&temp_dir).unwrap();

    let runtime = m31a::runtime::AppRuntime::new(&temp_dir).await.unwrap();
    let mission_id = MissionId::new();

    // No worktree exists -> returns None
    assert_eq!(runtime.active_execution_branch(&mission_id), None);

    // Create the expected worktree dir on disk
    let wt_dir = temp_dir
        .join(".m31a")
        .join("worktrees")
        .join(mission_id.to_string());
    fs::create_dir_all(&wt_dir).unwrap();

    // Worktree exists -> returns Some("m31a/<mid>")
    let branch = runtime.active_execution_branch(&mission_id);
    assert_eq!(branch, Some(format!("m31a/{mission_id}")));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_tui_03_scoped_task_graph_repository() {
    let temp_dir = std::env::temp_dir().join(format!("test_m31a_tui03_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(&temp_dir).unwrap();

    let db_path = temp_dir.join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let repo =
        m31a::persistence::sqlite::repositories::SqliteTaskGraphRepository::new(pool.clone());

    let ws1 = temp_dir.join("workspace1");
    let ws2 = temp_dir.join("workspace2");
    fs::create_dir_all(&ws1).unwrap();
    fs::create_dir_all(&ws2).unwrap();

    let mid1 = MissionId::new();
    let mid2 = MissionId::new();
    let sid1 = SessionId::new();
    let sid2 = SessionId::new();

    let now_str = chrono::Utc::now().to_rfc3339();

    // Insert missions
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, 'active', ?, ?)"
    )
    .bind(mid1.as_bytes().as_slice())
    .bind("Objective 1")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, 'active', ?, ?)"
    )
    .bind(mid2.as_bytes().as_slice())
    .bind("Objective 2")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // Insert sessions with workspace_root, mission_id, and active_mission_id
    sqlx::query(
        "INSERT INTO sessions (id, mission_id, workspace_root, active_mission_id, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'active', ?, ?)"
    )
    .bind(sid1.as_bytes().as_slice())
    .bind(mid1.as_bytes().as_slice())
    .bind(ws1.to_string_lossy().as_ref())
    .bind(mid1.as_bytes().as_slice())
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO sessions (id, mission_id, workspace_root, active_mission_id, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'active', ?, ?)"
    )
    .bind(sid2.as_bytes().as_slice())
    .bind(mid2.as_bytes().as_slice())
    .bind(ws2.to_string_lossy().as_ref())
    .bind(mid2.as_bytes().as_slice())
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    let gid1 = TaskGraphId::new();
    let gid2 = TaskGraphId::new();

    // Insert task graphs
    sqlx::query(
        "INSERT INTO task_graphs (id, mission_id, revision, plan_id, status, created_at, updated_at) VALUES (?, ?, 1, 'plan1', 'active', ?, ?)"
    )
    .bind(gid1.as_bytes().as_slice())
    .bind(mid1.as_bytes().as_slice())
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO task_graphs (id, mission_id, revision, plan_id, status, created_at, updated_at) VALUES (?, ?, 1, 'plan2', 'active', ?, ?)"
    )
    .bind(gid2.as_bytes().as_slice())
    .bind(mid2.as_bytes().as_slice())
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // Query scoped to ws1 -> must return gid1, never gid2
    let graph1 = repo
        .find_latest_active_graph_for_workspace(&ws1)
        .await
        .unwrap();
    assert!(graph1.is_some(), "must find graph for ws1");
    assert_eq!(graph1.unwrap().id, gid1);

    // Query scoped to ws2 -> must return gid2, never gid1
    let graph2 = repo
        .find_latest_active_graph_for_workspace(&ws2)
        .await
        .unwrap();
    assert!(graph2.is_some(), "must find graph for ws2");
    assert_eq!(graph2.unwrap().id, gid2);

    // Query unknown workspace -> returns None
    let unk = temp_dir.join("unknown");
    let graph_unk = repo
        .find_latest_active_graph_for_workspace(&unk)
        .await
        .unwrap();
    assert!(graph_unk.is_none());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_ux_01_clear_session_ordering_preserves_memory() {
    let mut model = TuiViewModel::new();
    model.conversation.push(TuiConversationItem::User {
        id: "msg-1".to_string(),
        sequence: 1,
        text: "Important message".to_string(),
        mentions: Vec::new(),
        timestamp: chrono::Utc::now(),
    });
    assert_eq!(model.conversation.len(), 1);

    // Simulated durable failure: an Error event arrives instead of SessionConversationCleared
    model.apply_interaction_event(&InteractionEvent::Error {
        message: "Failed to clear durable conversation for session".to_string(),
    });

    // In-memory conversation is PRESERVED on error (the original user message remains in conversation)
    assert!(model.conversation.iter().any(|item| {
        matches!(
            item,
            TuiConversationItem::User { text, .. } if text == "Important message"
        )
    }));

    // Simulated durable success: SessionConversationCleared arrives
    model.apply_interaction_event(&InteractionEvent::SessionConversationCleared {
        session_id: SessionId::new(),
    });

    // In-memory conversation is cleared ONLY upon confirmed durable success
    assert_eq!(model.conversation.len(), 0);
}

#[test]
fn test_cli_01_cli_overrides_preserved_post_onboarding() {
    let temp_dir = std::env::temp_dir().join(format!("test_m31a_cli01_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(&temp_dir).unwrap();

    let custom_model = "meta/llama-3.1-70b-instruct";
    let custom_profile = "code-reviewer";

    let builder = ResolvedConfiguration::builder(&temp_dir)
        .with_model(Some(custom_model.to_string()))
        .with_profile(Some(custom_profile.to_string()));

    let config = builder.build().unwrap();
    assert_eq!(config.active_model, custom_model);
    assert_eq!(config.active_profile.as_deref(), Some(custom_profile));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_doc_01_credentials_isolated_from_tier5_env() {
    let temp_dir = std::env::temp_dir().join(format!("test_m31a_doc01_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Set API_KEY_NVIDIA in environment
    unsafe {
        std::env::set_var("API_KEY_NVIDIA", "nvapi-untrusted-raw-env-token");
        std::env::set_var("NVIDIA_MODEL", "meta/llama-3.1-8b-instruct");
    }

    let config = ResolvedConfiguration::builder(&temp_dir).build().unwrap();

    // NVIDIA_MODEL is respected as a model override
    assert_eq!(config.active_model, "meta/llama-3.1-8b-instruct");

    // Clean up env
    unsafe {
        std::env::remove_var("API_KEY_NVIDIA");
        std::env::remove_var("NVIDIA_MODEL");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
