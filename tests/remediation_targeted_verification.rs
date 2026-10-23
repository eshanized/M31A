//! Targeted Security, Reliability, Durability, and Architecture Remediation Tests.
//!
//! Validates:
//! 1. P1: Credential and sensitive file access path closure through shell commands.
//! 2. P1: Scheduler durable transitions consistency and error handling.
//! 3. P1: Supervisor bounded operation deadline across descendant pipe inheritance and timeouts.
//! 4. P2: Mutation deduplication fence claim acquisition, concurrency rejection, staleness recovery, and outcome commit.
//! 5. P2: Atomic projection file persistence with atomic replacement.
//! 6. P3: Action fingerprint canonical JSON serialization and hashing.

use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;

use m31a::agent::runner::compute_action_fingerprint;
use m31a::ids::{ExecutionRunId, TaskId};
use m31a::kernel::invariants::{is_protected_or_sensitive_path, is_sensitive_or_credential_path};
use m31a::persistence::sqlite::repositories::{SqliteExecutionRunRepository, SqliteTaskRepository};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::pipeline::dedup::{
    DEFAULT_CLAIM_STALENESS_SECS, FenceVerdict, MutationDedupFence, canonical_json,
    fingerprint_mutation, is_fenced_mutating_tool,
};
use m31a::policy::matcher::extract_shell_tokens;
use m31a::process::env::check_command_safety;
use m31a::process::supervisor::{ProcessError, ProcessSupervisor};
use m31a::state::execution_run::ExecutionRun;
use m31a::state::task::{BlockingReason, Task, TaskResult};
use m31a::state_machine::TaskState;
use m31a::storage::StorageLayout;
use sqlx::SqlitePool;

// ========================================================================
// 1. P1: Credential and Sensitive File Access Protection Tests
// ========================================================================

#[test]
fn test_sensitive_path_detection_invariants() {
    // Credential and sensitive files
    assert!(is_sensitive_or_credential_path(Path::new(".env")));
    assert!(is_sensitive_or_credential_path(Path::new("./.env")));
    assert!(is_sensitive_or_credential_path(Path::new(".env.local")));
    assert!(is_sensitive_or_credential_path(Path::new(
        ".env.production"
    )));
    assert!(is_sensitive_or_credential_path(Path::new("sub/dir/.env")));
    assert!(is_sensitive_or_credential_path(Path::new(".ssh/id_rsa")));
    assert!(is_sensitive_or_credential_path(Path::new(
        ".aws/credentials"
    )));
    assert!(is_sensitive_or_credential_path(Path::new(
        ".gnupg/secring.gpg"
    )));
    assert!(is_sensitive_or_credential_path(Path::new("id_ed25519")));
    assert!(is_sensitive_or_credential_path(Path::new(
        "credentials.json"
    )));
    assert!(is_sensitive_or_credential_path(Path::new("token.secret")));
    assert!(is_sensitive_or_credential_path(Path::new("/etc/shadow")));
    assert!(is_sensitive_or_credential_path(Path::new("/etc/sudoers")));

    // Both protected (.git/.m31a) and sensitive
    assert!(is_protected_or_sensitive_path(Path::new(".git/config")));
    assert!(is_protected_or_sensitive_path(Path::new(".m31a/state.db")));
    assert!(is_protected_or_sensitive_path(Path::new(".env")));

    // Legitimate files MUST NOT be falsely flagged
    assert!(!is_sensitive_or_credential_path(Path::new("src/main.rs")));
    assert!(!is_sensitive_or_credential_path(Path::new("Cargo.toml")));
    assert!(!is_sensitive_or_credential_path(Path::new("README.md")));
    assert!(!is_sensitive_or_credential_path(Path::new(".gitignore")));
    assert!(!is_sensitive_or_credential_path(Path::new(
        ".github/workflows/ci.yml"
    )));
}

#[test]
fn test_shell_token_extraction_and_redirection_parsing() {
    let tokens = extract_shell_tokens("cat .env");
    assert!(tokens.contains(&".env".to_string()));

    let tokens = extract_shell_tokens("cat './.env'");
    assert!(tokens.contains(&"./.env".to_string()));

    let tokens = extract_shell_tokens("cat \".env\"");
    assert!(tokens.contains(&".env".to_string()));

    let tokens = extract_shell_tokens("grep -i secret < .env.production");
    assert!(tokens.contains(&".env.production".to_string()));

    let tokens = extract_shell_tokens("head -n 5 .aws/credentials | base64");
    assert!(tokens.contains(&".aws/credentials".to_string()));

    let tokens = extract_shell_tokens("sh -c 'cat id_rsa'");
    assert!(tokens.contains(&"id_rsa".to_string()));
}

#[test]
fn test_process_safety_checks_deny_credential_access_before_spawn() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path().canonicalize().unwrap();

    let env_file = ws.join(".env");
    fs::write(&env_file, "SENTINEL_SECRET=super_secret_value_12345\n").unwrap();

    let normal_file = ws.join("notes.txt");
    fs::write(&normal_file, "normal workspace contents\n").unwrap();

    // 1. Prohibited commands: must be rejected before spawn
    assert!(check_command_safety("cat", &[".env".to_string()]).is_err());
    assert!(check_command_safety("cat", &["./.env".to_string()]).is_err());
    assert!(check_command_safety("cat", &[".env.local".to_string()]).is_err());
    assert!(check_command_safety("cat", &[".ssh/id_rsa".to_string()]).is_err());
    assert!(check_command_safety("cat", &[".aws/credentials".to_string()]).is_err());
    assert!(check_command_safety("grep", &["secret".to_string(), ".env".to_string()]).is_err());

    // Shell string safety check
    assert!(check_command_safety("sh", &["-c".to_string(), "cat .env".to_string()]).is_err());
    assert!(check_command_safety("sh", &["-c".to_string(), "cat < .env".to_string()]).is_err());
    assert!(check_command_safety("sh", &["-c".to_string(), "cat './.env'".to_string()]).is_err());
    assert!(check_command_safety("sh", &["-c".to_string(), "cat \".env\"".to_string()]).is_err());

    // 2. Allowed control cases: must succeed safety validation
    assert!(check_command_safety("cat", &["notes.txt".to_string()]).is_ok());
    assert!(check_command_safety("ls", &["-la".to_string()]).is_ok());
    assert!(check_command_safety("echo", &["hello world".to_string()]).is_ok());
    assert!(check_command_safety("cargo", &["check".to_string()]).is_ok());
}

// ========================================================================
// 2. P1: Scheduler Durable Transitions Consistency Tests
// ========================================================================

#[tokio::test]
async fn test_scheduler_grouped_atomic_transitions_durability() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();

    let run_repo = SqliteExecutionRunRepository::new(pool.clone());
    let task_repo = SqliteTaskRepository::new(pool.clone());

    let run_id = ExecutionRunId::new();
    let run = ExecutionRun::new(run_id, "Scheduler Consistency Test".to_string());
    run_repo.insert(&run).await.unwrap();

    // Create 3 tasks: t1 -> t2, t1 -> t3
    let t1_id = TaskId::new();
    let mut t1 = Task::new(t1_id, run_id, "Task 1 (root)".to_string());
    t1.status = TaskState::Running;

    let t2_id = TaskId::new();
    let mut t2 = Task::new(t2_id, run_id, "Task 2 (dependent on t1)".to_string());
    t2.status = TaskState::Blocked;
    t2.blocking_reason = Some(BlockingReason::PolicyDenied {
        reason: "Waiting for dependency".to_string(),
    });

    let t3_id = TaskId::new();
    let mut t3 = Task::new(t3_id, run_id, "Task 3 (dependent on t1)".to_string());
    t3.status = TaskState::Blocked;
    t3.blocking_reason = Some(BlockingReason::PolicyDenied {
        reason: "Waiting for dependency".to_string(),
    });

    task_repo.insert(&t1).await.unwrap();
    task_repo.insert(&t2).await.unwrap();
    task_repo.insert(&t3).await.unwrap();

    // 1. Atomically mark t1 succeeded and dependents (t2, t3) ready
    let result = TaskResult::new("Success evidence");
    task_repo
        .mark_succeeded_and_dependents_ready(t1_id, Some(&result), &[t2_id, t3_id])
        .await
        .unwrap();

    let fetched_t1 = task_repo.get(t1_id).await.unwrap().unwrap();
    assert_eq!(fetched_t1.status, TaskState::Succeeded);
    assert_eq!(fetched_t1.result.unwrap().summary, "Success evidence");

    let fetched_t2 = task_repo.get(t2_id).await.unwrap().unwrap();
    assert_eq!(fetched_t2.status, TaskState::Ready);
    assert!(fetched_t2.blocking_reason.is_none());

    let fetched_t3 = task_repo.get(t3_id).await.unwrap().unwrap();
    assert_eq!(fetched_t3.status, TaskState::Ready);

    // 2. Test failure and dependent blocking
    let t4_id = TaskId::new();
    let mut t4 = Task::new(t4_id, run_id, "Task 4 (failing)".to_string());
    t4.status = TaskState::Running;

    let t5_id = TaskId::new();
    let mut t5 = Task::new(t5_id, run_id, "Task 5 (dependent on t4)".to_string());
    t5.status = TaskState::Pending;

    task_repo.insert(&t4).await.unwrap();
    task_repo.insert(&t5).await.unwrap();

    let now = chrono::Utc::now();
    let fail_reason = BlockingReason::PrerequisiteFailed {
        failed_task_id: t4_id,
        error_summary: "Compilation failed".to_string(),
    };
    task_repo
        .mark_failed_and_dependents_blocked(t4_id, now, &[t5_id], &fail_reason)
        .await
        .unwrap();

    let fetched_t4 = task_repo.get(t4_id).await.unwrap().unwrap();
    assert_eq!(fetched_t4.status, TaskState::Failed);

    let fetched_t5 = task_repo.get(t5_id).await.unwrap().unwrap();
    assert_eq!(fetched_t5.status, TaskState::Blocked);
    assert_eq!(fetched_t5.blocking_reason, Some(fail_reason));

    // 3. Test cancellation and dependent blocking
    let t6_id = TaskId::new();
    let mut t6 = Task::new(t6_id, run_id, "Task 6 (cancelling)".to_string());
    t6.status = TaskState::Ready;

    let t7_id = TaskId::new();
    let mut t7 = Task::new(t7_id, run_id, "Task 7 (dependent on t6)".to_string());
    t7.status = TaskState::Pending;

    task_repo.insert(&t6).await.unwrap();
    task_repo.insert(&t7).await.unwrap();

    let cancel_reason = BlockingReason::PrerequisiteFailed {
        failed_task_id: t6_id,
        error_summary: "Task cancelled".to_string(),
    };
    task_repo
        .mark_cancelled_and_dependents_blocked(t6_id, now, &[t7_id], &cancel_reason)
        .await
        .unwrap();

    let fetched_t6 = task_repo.get(t6_id).await.unwrap().unwrap();
    assert_eq!(fetched_t6.status, TaskState::Cancelled);

    let fetched_t7 = task_repo.get(t7_id).await.unwrap().unwrap();
    assert_eq!(fetched_t7.status, TaskState::Blocked);
}

// ========================================================================
// 3. P1: Supervisor Absolute Operation Deadline & Pipe Draining Tests
// ========================================================================

#[tokio::test]
async fn test_supervisor_bounds_descendant_pipe_inheritance() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path().to_path_buf();

    // Set a short deadline: 1 second
    let supervisor =
        ProcessSupervisor::new(Duration::from_millis(1000), Duration::from_millis(200));

    let start = std::time::Instant::now();

    // Launch a process that exits immediately but spawns a long-running descendant holding stdout open
    #[cfg(unix)]
    let args = vec!["-c".to_string(), "(sleep 15 >&1) & exit 0".to_string()];
    #[cfg(unix)]
    let result = supervisor
        .run_command("sh", &args, &ws, None, None, None)
        .await;

    #[cfg(windows)]
    let args = vec![
        "/c".to_string(),
        "start /b ping -n 15 127.0.0.1 > nul".to_string(),
    ];
    #[cfg(windows)]
    let result = supervisor
        .run_command("cmd", &args, &ws, None, None, None)
        .await;

    let elapsed = start.elapsed();

    // The supervisor MUST NOT hang for 15 seconds. It must return within bounded time (< 4 seconds)
    assert!(
        elapsed < Duration::from_secs(4),
        "Supervisor hung for {:?}; must enforce bounded operation deadline",
        elapsed
    );

    // Result should be ok (child exited 0) or TimedOut (if drain timed out and killed tree)
    match result {
        Ok(proc_res) => {
            assert_eq!(proc_res.exit_code, 0);
        }
        Err(ProcessError::TimedOut(_)) => {
            // Also acceptable: pipe drain timed out and supervisor aborted cleanly
        }
        Err(other) => {
            panic!("Unexpected supervisor error: {:?}", other);
        }
    }
}

#[tokio::test]
async fn test_supervisor_enforces_timeout_and_reaps() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path().to_path_buf();

    let supervisor = ProcessSupervisor::new(Duration::from_millis(300), Duration::from_millis(100));

    let start = std::time::Instant::now();

    #[cfg(unix)]
    let args = vec!["5".to_string()];
    #[cfg(unix)]
    let result = supervisor
        .run_command("sleep", &args, &ws, None, None, None)
        .await;

    #[cfg(windows)]
    let args = vec!["5".to_string()];
    #[cfg(windows)]
    let result = supervisor
        .run_command("timeout", &args, &ws, None, None, None)
        .await;

    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "Supervisor timeout exceeded: {:?}",
        elapsed
    );

    match result {
        Err(ProcessError::TimedOut(_)) => {}
        Ok(res) => panic!(
            "Expected timeout error, got successful exit {}",
            res.exit_code
        ),
        Err(other) => panic!("Expected timeout, got {:?}", other),
    }
}

// ========================================================================
// 4. P2: Mutation Deduplication Fence Tests
// ========================================================================

#[tokio::test]
async fn test_mutation_dedup_fence_concurrency_and_staleness_recovery() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();

    let fence = MutationDedupFence::new(pool.clone());
    let task_id = TaskId::new();
    let tool_name = "write_file";
    let params = serde_json::json!({
        "path": "test.txt",
        "content": "hello world"
    });
    let fp = fingerprint_mutation(&task_id, tool_name, &params);

    assert!(is_fenced_mutating_tool(tool_name));
    assert!(!is_fenced_mutating_tool("run_command"));

    // 1. Initial claim acquisition succeeds
    let verdict1 = fence
        .acquire_claim(&task_id, tool_name, &fp, None)
        .await
        .unwrap();
    assert_eq!(verdict1, FenceVerdict::Proceed);

    // 2. Concurrent identical claim on active in_progress row is rejected
    let verdict2 = fence
        .acquire_claim(&task_id, tool_name, &fp, None)
        .await
        .unwrap();
    assert_eq!(verdict2, FenceVerdict::ConcurrentInProgress);

    // 3. Commit outcome as failure -> permits retry
    fence
        .commit_outcome(&task_id, tool_name, &fp, false, None, None)
        .await
        .unwrap();

    let verdict3 = fence
        .acquire_claim(&task_id, tool_name, &fp, None)
        .await
        .unwrap();
    assert_eq!(verdict3, FenceVerdict::Proceed);

    // 4. Commit outcome as success -> suppresses duplicates
    let output_hash = "abc123hash";
    fence
        .commit_outcome(&task_id, tool_name, &fp, true, Some(output_hash), None)
        .await
        .unwrap();

    let verdict4 = fence
        .acquire_claim(&task_id, tool_name, &fp, None)
        .await
        .unwrap();
    assert_eq!(
        verdict4,
        FenceVerdict::DuplicateSuppressed {
            output_hash: Some(output_hash.to_string())
        }
    );

    // 5. Test staleness recovery: simulate a crashed worker by manually setting
    // updated_at to 10 minutes ago
    let stale_task = TaskId::new();
    let stale_fp = fingerprint_mutation(&stale_task, tool_name, &params);

    // Acquire claim
    let _ = fence
        .acquire_claim(&stale_task, tool_name, &stale_fp, None)
        .await
        .unwrap();

    // Backdate updated_at by 10 minutes
    let old_time = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        "UPDATE tool_mutation_fence SET updated_at = ? WHERE task_id = ? AND tool_name = ? AND fingerprint = ?",
    )
    .bind(&old_time)
    .bind(stale_task.as_bytes().as_slice())
    .bind(tool_name)
    .bind(&stale_fp)
    .execute(&pool)
    .await
    .unwrap();

    // Next claim with DEFAULT_CLAIM_STALENESS_SECS (5 mins) should recover the stale claim and Proceed
    let verdict_recovered = fence
        .acquire_claim(
            &stale_task,
            tool_name,
            &stale_fp,
            Some(DEFAULT_CLAIM_STALENESS_SECS),
        )
        .await
        .unwrap();
    assert_eq!(verdict_recovered, FenceVerdict::Proceed);
}

// ========================================================================
// 5. P2: Atomic Projection File Persistence Tests
// ========================================================================

#[test]
fn test_atomic_projection_file_persistence() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path().canonicalize().unwrap();

    // Create .m31a/project boundary
    fs::create_dir_all(ws.join(".m31a/project")).unwrap();

    // Write initial projection
    let initial_data = "{\"mission_id\":\"m1\",\"state\":\"Genesis\"}";
    let target_path = StorageLayout::write_projection_file(
        &ws,
        ".m31a/project",
        "mission_projection.json",
        initial_data,
    )
    .unwrap();

    assert_eq!(fs::read_to_string(&target_path).unwrap(), initial_data);

    // Ensure no temp files remained in directory
    let entries: Vec<_> = fs::read_dir(ws.join(".m31a/project"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries, vec!["mission_projection.json"]);

    // Atomically replace with new projection
    let updated_data = "{\"mission_id\":\"m1\",\"state\":\"Execution\"}";
    StorageLayout::write_projection_file(
        &ws,
        ".m31a/project",
        "mission_projection.json",
        updated_data,
    )
    .unwrap();

    assert_eq!(fs::read_to_string(&target_path).unwrap(), updated_data);

    let entries_after: Vec<_> = fs::read_dir(ws.join(".m31a/project"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries_after, vec!["mission_projection.json"]);
}

// ========================================================================
// 6. P3: Canonical Action Fingerprint Serialization Tests
// ========================================================================

#[test]
fn test_canonical_action_fingerprint_serialization() {
    // 1. Key ordering invariance
    let val_a = serde_json::json!({
        "path": "src/lib.rs",
        "action": "edit",
        "line": 42
    });
    let val_b = serde_json::json!({
        "line": 42,
        "action": "edit",
        "path": "src/lib.rs"
    });

    let fp_a = compute_action_fingerprint("edit_file", &val_a);
    let fp_b = compute_action_fingerprint("edit_file", &val_b);
    assert_eq!(
        fp_a, fp_b,
        "Fingerprints must match regardless of key order"
    );

    // 2. String escaping and special characters
    let val_special = serde_json::json!({
        "content": "line1\nline2\t\"quoted text\" and \\backslashes\\",
        "nested": {
            "key with \"quotes\"": "value"
        }
    });
    let canonical_str = canonical_json(&val_special);
    assert!(canonical_str.contains("\\\"quoted text\\\""));
    assert!(canonical_str.contains("\\\\backslashes\\\\"));
    assert!(canonical_str.contains("key with \\\"quotes\\\""));

    let fp_special1 = compute_action_fingerprint("write_file", &val_special);
    let fp_special2 = compute_action_fingerprint("write_file", &val_special);
    assert_eq!(fp_special1, fp_special2);

    // 3. Typed distinction: number vs string
    let val_num = serde_json::json!({ "id": 123 });
    let val_str = serde_json::json!({ "id": "123" });
    assert_ne!(
        compute_action_fingerprint("tool", &val_num),
        compute_action_fingerprint("tool", &val_str),
        "Numbers and numeric strings must not produce identical fingerprints"
    );

    // 4. Array order preservation
    let arr_1 = serde_json::json!([1, 2, 3]);
    let arr_2 = serde_json::json!([3, 2, 1]);
    assert_ne!(
        compute_action_fingerprint("tool", &arr_1),
        compute_action_fingerprint("tool", &arr_2),
        "Array ordering must be preserved in canonical fingerprints"
    );
}
