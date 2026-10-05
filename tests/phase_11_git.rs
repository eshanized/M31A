//! Phase 11 Git subsystem integration tests (GST-01–GST-04).

#[path = "common/git_auth.rs"]
mod git_auth;

use git_auth::TestGitAuth;
use m31a::git::{WorktreeConfig, WorktreeManager};
use m31a::ids::MissionId;
use std::process::Command;
use tempfile::TempDir;

/// Helper to initialize a pristine git repository for tests.
fn setup_test_repo() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let repo_path = dir.path();

    // git init
    let status = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(status.success());

    // config user.name and user.email
    Command::new("git")
        .args(["config", "user.name", "M31A Test"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "test@m31a.dev"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.email failed");

    // initial file and commit
    std::fs::write(repo_path.join("README.md"), "# Test Repo\n").unwrap();
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(repo_path)
        .status()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .status()
        .expect("git commit failed");

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .expect("git rev-parse HEAD failed");
    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();

    (dir, head)
}

#[tokio::test]
async fn test_git_operations_in_worktree() {
    let (repo_dir, base_head) = setup_test_repo();
    let repo_path = repo_dir.path();

    let config = WorktreeConfig::new(repo_path);
    let manager = WorktreeManager::new(config);

    let mission_id = MissionId::new();
    let test_auth = TestGitAuth::new().with_mission(mission_id);
    let wt_path_str = repo_path
        .join(".m31a/worktrees")
        .join(mission_id.to_string())
        .to_string_lossy()
        .to_string();
    let wt_branch = format!("m31a/{mission_id}");
    let worktree = manager
        .create_worktree(
            &mission_id,
            None,
            &test_auth.worktree_add_gate(repo_path, &wt_path_str, &wt_branch),
        )
        .await
        .expect("create_worktree should succeed");

    // Assert worktree path follows .m31a/worktrees/<mission_id>
    let expected_rel_path = format!(".m31a/worktrees/{mission_id}");
    assert!(worktree.path.ends_with(&expected_rel_path));
    assert!(worktree.path.exists());

    // Assert branch is m31a/<mission_id>
    assert_eq!(worktree.branch, format!("m31a/{mission_id}"));
    assert_eq!(worktree.base_commit, base_head);

    // Verify main working tree does not have the worktree branch checked out
    let main_branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let main_branch = String::from_utf8_lossy(&main_branch_output.stdout);
    assert_eq!(main_branch.trim(), "main");

    // Modify a file in the worktree
    let worktree_file = worktree.path.join("agent_work.txt");
    std::fs::write(&worktree_file, "Worktree modification\n").unwrap();

    // Verify file is NOT in main repo
    assert!(!repo_path.join("agent_work.txt").exists());

    // Run git commands inside worktree
    let raw_gate = test_auth.raw_gate(&worktree.path);
    let status_out = manager
        .run_git_in_worktree(&worktree, &["status", "--porcelain"], &raw_gate)
        .await
        .expect("git status should succeed");
    assert!(status_out.contains("agent_work.txt"));

    manager
        .run_git_in_worktree(&worktree, &["add", "agent_work.txt"], &raw_gate)
        .await
        .expect("git add should succeed");

    manager
        .run_git_in_worktree(
            &worktree,
            &["commit", "-m", "feat: agent work in worktree"],
            &raw_gate,
        )
        .await
        .expect("git commit should succeed");

    // Still not in main repo
    assert!(!repo_path.join("agent_work.txt").exists());

    // Check list_worktrees
    let worktrees = manager.list_worktrees().await.unwrap();
    assert!(worktrees.iter().any(|p| p == &worktree.path));

    // Remove worktree
    manager
        .remove_worktree(
            &worktree,
            true,
            &test_auth.worktree_remove_gate(repo_path, true, Some(worktree.branch.clone())),
        )
        .await
        .expect("remove_worktree should succeed");

    assert!(!worktree.path.exists());
}

#[tokio::test]
async fn test_commit_attribution_and_trailers() {
    use m31a::git::attribution::GitAttributionStore;
    use m31a::git::trailers::CommitTrailers;
    use m31a::ids::{MissionId, TaskId};
    use m31a::state_machine::agent::AgentRole;
    use sqlx::sqlite::SqlitePoolOptions;

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let trailers = CommitTrailers::new(
        mission_id,
        task_id,
        AgentRole::implementer(),
        "claude-sonnet-3-5",
    )
    .with_verification("run-12345");

    let formatted = trailers.format_trailers();
    assert!(formatted.contains(&format!("M31A-Mission: {mission_id}")));
    assert!(formatted.contains(&format!("M31A-Task: {task_id}")));
    assert!(formatted.contains("M31A-Agent: implementer"));
    assert!(formatted.contains("M31A-Model: claude-sonnet-3-5"));
    assert!(formatted.contains("M31A-Verification: run-12345"));

    // Test embed
    let commit_msg = "feat: implement feature x";
    let embedded = CommitTrailers::embed_trailers(commit_msg, &trailers).unwrap();
    assert!(embedded.starts_with("feat: implement feature x\n\n"));

    // Test parse round-trip
    let parsed = CommitTrailers::parse_trailers(&embedded).unwrap();
    assert_eq!(parsed, trailers);

    // Test SQLite attribution store
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("in-memory db");

    let migration_sql = include_str!("../migrations/010_git_attribution.sql");
    sqlx::raw_sql(migration_sql)
        .execute(&pool)
        .await
        .expect("migration 010 should apply");

    let store = GitAttributionStore::new(pool);
    let commit_hash = "abc123def456";
    let record = store
        .record_commit(commit_hash, &trailers, None)
        .await
        .expect("record commit");

    assert_eq!(record.commit_hash, commit_hash);
    assert_eq!(record.mission_id, mission_id.to_string());
    assert_eq!(record.task_id, task_id.to_string());
    assert_eq!(record.agent_role, "implementer");
    assert_eq!(record.model_id, "claude-sonnet-3-5");
    assert_eq!(record.verification_run_id, Some("run-12345".to_string()));

    // Query find_by_commit
    let found = store.find_by_commit(commit_hash).await.unwrap().unwrap();
    assert_eq!(found, record);

    // Query find_by_task
    let task_records = store.find_by_task(&task_id).await.unwrap();
    assert_eq!(task_records.len(), 1);
    assert_eq!(task_records[0].commit_hash, commit_hash);

    // Query find_by_mission
    let mission_records = store.find_by_mission(&mission_id).await.unwrap();
    assert_eq!(mission_records.len(), 1);

    // Query find_by_model
    let model_records = store.find_by_model("claude-sonnet-3-5").await.unwrap();
    assert_eq!(model_records.len(), 1);
}

#[tokio::test]
async fn test_remote_and_destructive_policy_enforcement() {
    use m31a::git::GitOperation;
    use m31a::git::stash::PrivateStashManager;
    use m31a::ids::MissionId;
    use m31a::kernel::seams::policy::PolicyDecision;

    // 1. Safe operations
    let status_op = GitOperation::Status;
    assert!(!status_op.is_remote());
    assert!(!status_op.is_destructive());
    assert_eq!(status_op.default_policy_decision(), PolicyDecision::Allow);
    assert!(status_op.enforce_policy(false).is_ok());

    // 2. Remote push operation defaults to Ask
    let push_op = GitOperation::Push {
        remote: "origin".into(),
        branch: "feature".into(),
        force: false,
    };
    assert!(push_op.is_remote());
    assert_eq!(push_op.default_policy_decision(), PolicyDecision::Ask);
    // Unapproved push fails closed
    assert!(push_op.enforce_policy(false).is_err());
    // Approved push succeeds
    assert!(push_op.enforce_policy(true).is_ok());

    // 3. Destructive reset defaults to Deny
    let reset_op = GitOperation::HardReset {
        target: "HEAD~1".into(),
    };
    assert!(reset_op.is_destructive());
    assert_eq!(reset_op.default_policy_decision(), PolicyDecision::Deny);
    assert!(reset_op.enforce_policy(true).is_err());

    // 4. Private Stash isolation
    let (repo_dir, _head) = setup_test_repo();
    let repo_path = repo_dir.path();
    let stash_mgr = PrivateStashManager::new(repo_path);
    let mission_id = MissionId::new();
    let stash_auth = TestGitAuth::new().with_mission(mission_id);

    // Modify a file
    let dirty_file = repo_path.join("README.md");
    std::fs::write(&dirty_file, "# Modified for stash test\n").unwrap();

    // Save to private stash
    let sha = stash_mgr
        .save(
            &mission_id,
            Some("task wip"),
            &stash_auth.stash_save_gate(&mission_id, repo_path),
        )
        .await
        .expect("stash save should succeed")
        .expect("should produce commit sha");
    assert!(!sha.is_empty());

    // Working directory should now be reset to clean
    let content_after = std::fs::read_to_string(&dirty_file).unwrap();
    assert_eq!(content_after, "# Test Repo\n");

    // Standard git stash list should be EMPTY
    let standard_stash_out = Command::new("git")
        .args(["stash", "list"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let standard_stash = String::from_utf8_lossy(&standard_stash_out.stdout);
    assert!(
        standard_stash.trim().is_empty(),
        "standard git stash must remain empty"
    );

    // has_stash should be true
    assert!(stash_mgr.has_stash(&mission_id).await.unwrap());

    // Pop private stash
    stash_mgr
        .pop(
            &mission_id,
            &stash_auth.stash_pop_gate(&mission_id, repo_path),
        )
        .await
        .expect("stash pop should succeed");

    // Content should be restored
    let content_restored = std::fs::read_to_string(&dirty_file).unwrap();
    assert_eq!(content_restored, "# Modified for stash test\n");

    // has_stash should now be false
    assert!(!stash_mgr.has_stash(&mission_id).await.unwrap());
}

#[tokio::test]
async fn test_drift_detection_and_invalidation() {
    use futures::StreamExt;
    use m31a::events::bus::{BroadcastEventBus, EventBus, EventFilter};
    use m31a::events::types::EventType;
    use m31a::git::drift::{DriftStatus, TreeHashDriftDetector};
    use m31a::git::integration::{
        IntegrationState, MergeStrategy, WorktreeIntegrationStateMachine,
    };
    use m31a::git::worktree::{WorktreeConfig, WorktreeManager};
    use m31a::ids::MissionId;
    use std::sync::Arc;
    use std::time::Duration;

    let (repo_dir, _head) = setup_test_repo();
    let repo_path = repo_dir.path();

    let event_bus = Arc::new(BroadcastEventBus::new(64));
    let mut rx = event_bus.subscribe(EventFilter::all()).await;

    let detector = TreeHashDriftDetector::new().with_event_bus(event_bus.clone());
    let mission_id = MissionId::new();

    // 1. Initial snapshot is clean
    let snapshot = detector
        .record_snapshot(repo_path)
        .await
        .expect("record snapshot");

    let status = detector
        .check_drift(&mission_id, repo_path, &snapshot)
        .await
        .expect("check drift");
    assert_eq!(status, DriftStatus::Clean);

    // 2. Introduce drift by modifying a file
    let dirty_file = repo_path.join("drift_test.txt");
    std::fs::write(&dirty_file, "external modification\n").unwrap();

    let drift_status = detector
        .check_drift(&mission_id, repo_path, &snapshot)
        .await
        .expect("check drift after modification");

    match drift_status {
        DriftStatus::Drifted {
            expected_hash,
            actual_hash,
            modified_files,
        } => {
            assert_eq!(expected_hash, snapshot.tree_hash);
            assert_ne!(actual_hash, expected_hash);
            assert!(modified_files.iter().any(|f| f.contains("drift_test.txt")));
        }
        DriftStatus::Clean => panic!("Expected repository drift to be detected!"),
    }

    // Verify event was emitted
    let mut received_drift_event = false;
    while let Ok(Some(Ok(envelope))) =
        tokio::time::timeout(Duration::from_millis(100), rx.next()).await
    {
        if matches!(
            envelope.event_type,
            EventType::RepositoryDriftDetected {
                mission_id: m,
                ..
            } if m == mission_id
        ) {
            received_drift_event = true;
            break;
        }
    }
    assert!(
        received_drift_event,
        "RepositoryDriftDetected event should be emitted"
    );

    // Clean up drift file
    let _ = std::fs::remove_file(&dirty_file);

    // 3. Test WorktreeIntegrationStateMachine with clean merge
    let wt_config = WorktreeConfig::new(repo_path);
    let wt_manager = WorktreeManager::new(wt_config);

    let mission_1 = MissionId::new();
    let auth_1 = TestGitAuth::new().with_mission(mission_1);
    let wt1_path_str = repo_path
        .join(".m31a/worktrees")
        .join(mission_1.to_string())
        .to_string_lossy()
        .to_string();
    let wt1_branch = format!("m31a/{mission_1}");
    let worktree_1 = wt_manager
        .create_worktree(
            &mission_1,
            None,
            &auth_1.worktree_add_gate(repo_path, &wt1_path_str, &wt1_branch),
        )
        .await
        .expect("create worktree 1");

    // Add a file in worktree 1
    std::fs::write(worktree_1.path.join("feature.txt"), "feature content\n").unwrap();
    let raw_1 = auth_1.raw_gate(&worktree_1.path);
    wt_manager
        .run_git_in_worktree(&worktree_1, &["add", "feature.txt"], &raw_1)
        .await
        .unwrap();
    wt_manager
        .run_git_in_worktree(
            &worktree_1,
            &["commit", "-m", "feat: add feature.txt"],
            &raw_1,
        )
        .await
        .unwrap();

    let mut state_machine = WorktreeIntegrationStateMachine::new(repo_path);
    assert_eq!(state_machine.state(), IntegrationState::MissionCompleted);

    let report = state_machine
        .integrate(
            &worktree_1,
            "main",
            MergeStrategy::PreferFastForward,
            &auth_1.integrate_gate(&worktree_1.branch, "main", repo_path),
        )
        .await
        .expect("integration should succeed");

    assert_eq!(report.state, IntegrationState::Integrated);
    assert!(report.merge_commit.is_some());
    assert_eq!(state_machine.state(), IntegrationState::Integrated);

    // 4. Test WorktreeIntegrationStateMachine with conflicting merge
    // Create conflict on main
    let conflict_file = repo_path.join("conflict.txt");
    std::fs::write(&conflict_file, "main version\n").unwrap();
    Command::new("git")
        .args(["add", "conflict.txt"])
        .current_dir(repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "conflict on main"])
        .current_dir(repo_path)
        .status()
        .unwrap();

    // Create worktree 2 based on HEAD before conflict.txt
    let mission_2 = MissionId::new();
    let auth_2 = TestGitAuth::new().with_mission(mission_2);
    let wt2_path_str = repo_path
        .join(".m31a/worktrees")
        .join(mission_2.to_string())
        .to_string_lossy()
        .to_string();
    let wt2_branch = format!("m31a/{mission_2}");
    let worktree_2 = wt_manager
        .create_worktree(
            &mission_2,
            None,
            &auth_2.worktree_add_gate(repo_path, &wt2_path_str, &wt2_branch),
        )
        .await
        .expect("create worktree 2");

    std::fs::write(worktree_2.path.join("conflict.txt"), "branch version\n").unwrap();
    let raw_2 = auth_2.raw_gate(&worktree_2.path);
    wt_manager
        .run_git_in_worktree(&worktree_2, &["add", "conflict.txt"], &raw_2)
        .await
        .unwrap();
    wt_manager
        .run_git_in_worktree(&worktree_2, &["commit", "-m", "conflict on branch"], &raw_2)
        .await
        .unwrap();

    // In main, modify conflict.txt again so branch and main diverge on same lines
    std::fs::write(&conflict_file, "main divergent version\n").unwrap();
    Command::new("git")
        .args(["commit", "-am", "diverge main"])
        .current_dir(repo_path)
        .status()
        .unwrap();

    let mut state_machine_conflict = WorktreeIntegrationStateMachine::new(repo_path);
    let conflict_report = state_machine_conflict
        .integrate(
            &worktree_2,
            "main",
            MergeStrategy::PreferFastForward,
            &auth_2.integrate_gate(&worktree_2.branch, "main", repo_path),
        )
        .await
        .expect("integrate call should return report");

    assert_eq!(
        conflict_report.state,
        IntegrationState::IntegrationConflict,
        "Should transition to IntegrationConflict"
    );
    assert!(
        conflict_report
            .conflict_files
            .iter()
            .any(|f| f.contains("conflict.txt")),
        "conflict_files should include conflict.txt"
    );
    assert_eq!(
        state_machine_conflict.state(),
        IntegrationState::IntegrationConflict
    );

    // Main repo should NOT be in a conflicted merge state
    let status_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let main_status = String::from_utf8_lossy(&status_out.stdout);
    assert!(
        !main_status.contains("UU"),
        "Main repository must not have unresolved merge conflicts"
    );
}

#[tokio::test]
async fn test_centralized_git_service_methods() {
    use m31a::capability::providers::CliGitProvider;
    use m31a::capability::traits::git::GitService;
    use m31a::git::trailers::CommitTrailers;
    use m31a::ids::{MissionId, TaskId};
    use m31a::state_machine::agent::AgentRole;

    let (repo_dir, _base_head) = setup_test_repo();
    let repo_path = repo_dir.path();
    let git = CliGitProvider::new(repo_path);

    // 1. Initial status is clean
    let status = git.status_porcelain().await.expect("status porcelain");
    assert!(status.trim().is_empty());

    // 2. Write new file and verify status
    std::fs::write(repo_path.join("file1.txt"), "hello world\n").unwrap();
    std::fs::write(repo_path.join(".m31a_ignored.txt"), "secret\n").unwrap();

    let status_dirty = git.status_porcelain().await.expect("status porcelain");
    assert!(status_dirty.contains("file1.txt"));

    // 3. Stage all and then reset ignored file
    let svc_auth = TestGitAuth::new();
    git.add_all(&svc_auth.add_all_gate(repo_path))
        .await
        .expect("add all");
    git.reset(
        &[".m31a_ignored.txt"],
        &svc_auth.unstage_gate(&[".m31a_ignored.txt"], repo_path),
    )
    .await
    .expect("reset");

    // 4. Commit with trailers
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let trailers = CommitTrailers::new(mission_id, task_id, AgentRole::implementer(), "meta/test");
    let full_msg = m31a::git::trailers::CommitTrailers::embed_trailers("Add file1.txt", &trailers)
        .expect("embed trailers");
    let commit_hash = git
        .commit_with_trailers(
            "Add file1.txt",
            &trailers,
            &svc_auth.commit_gate(&full_msg, repo_path),
        )
        .await
        .expect("commit with trailers");
    assert!(!commit_hash.is_empty());

    // 5. Diff range
    let head_diff = git.diff_range("HEAD~1..HEAD").await.expect("diff range");
    assert!(head_diff.contains("file1.txt"));
    assert!(head_diff.contains("+hello world"));

    // 6. Modify tracked file and restore HEAD
    std::fs::write(repo_path.join("file1.txt"), "corrupted content\n").unwrap();
    assert!(
        git.diff_range("HEAD")
            .await
            .unwrap()
            .contains("corrupted content")
    );

    git.restore_head("file1.txt", &svc_auth.restore_gate("file1.txt", repo_path))
        .await
        .expect("restore head");
    let restored = std::fs::read_to_string(repo_path.join("file1.txt")).unwrap();
    assert_eq!(restored, "hello world\n");
}
