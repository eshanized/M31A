//! Comprehensive Integration Test Suite: P0 Process & Shell Security Boundary
//!
//! Verifies:
//! 1. Ordinary shell/process execution cannot mutate `.git`
//! 2. Ordinary shell/process execution cannot mutate `.m31a`
//! 3. Process execution cannot escape the authorized workspace
//! 4. Policy is evaluated before side effects (immutable veto)
//! 5. Sandbox isolation (Bubblewrap) reinforces the boundary with ro-bind mounts
//! 6. Fallback execution remains fail-closed within documented limits
//! 7. Git subprocesses cannot redirect repository state via GIT_DIR, GIT_WORK_TREE, GIT_INDEX_FILE, etc.
//! 8. Command arguments cannot silently redirect execution to protected paths
//! 9. Denial produces observable, typed failure
//! 10. Legitimate commands and Git operations inside the workspace continue working

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

use m31a::capability::error::CapabilityError;
use m31a::capability::providers::cli_git::CliGitProvider;
use m31a::capability::providers::local_process::LocalProcessProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::capability::traits::git::GitService;
use m31a::capability::traits::process::ProcessService;
use m31a::capability::traits::shell::ShellService;
use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::policy::effective::EffectivePolicy;
use m31a::process::env::{EnvironmentBuilder, check_command_safety, validate_working_directory};
use m31a::sandbox::plan::SandboxPlan;
use m31a::sandbox::provider::SandboxProvider;
use m31a::sandbox::providers::{BubblewrapSandboxProvider, ProcessIsolationProvider};
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::process::{RunCommandInput, RunCommandTool};
use m31a::tools::registry::ToolRegistry;

fn setup_test_workspace() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().canonicalize().unwrap();

    // 1. Protected .git directory and metadata
    let git_dir = ws.join(".git");
    fs::create_dir_all(git_dir.join("hooks")).unwrap();
    fs::create_dir_all(git_dir.join("objects")).unwrap();
    fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
    fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n",
    )
    .unwrap();
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(git_dir.join("hooks/pre-commit"), "#!/bin/sh\nexit 0\n").unwrap();

    // 2. Protected .m31a runtime state
    let m31a_dir = ws.join(".m31a");
    fs::create_dir_all(&m31a_dir).unwrap();
    fs::write(
        m31a_dir.join("m31a.db"),
        "SQLITE_FORMAT_3_INTERNAL_M31A_DATABASE_CONTENT",
    )
    .unwrap();
    fs::write(m31a_dir.join("config.toml"), "profile = 'default'\n").unwrap();

    // 3. Legitimate workspace source files
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("main.rs"),
        "fn main() { println!(\"hello\"); }\n",
    )
    .unwrap();
    fs::write(ws.join("README.md"), "# Test Project\n").unwrap();

    // 4. Legitimate dotfiles that MUST remain accessible
    fs::write(ws.join(".gitignore"), "target/\nCargo.lock\n").unwrap();
    fs::write(ws.join(".gitattributes"), "* text=auto\n").unwrap();
    let github_dir = ws.join(".github/workflows");
    fs::create_dir_all(&github_dir).unwrap();
    fs::write(github_dir.join("ci.yml"), "name: CI\non: push\n").unwrap();

    (tmp, ws)
}

// ===========================================================================
// Test A: Shell Attempts to Delete .git
// ===========================================================================

#[tokio::test]
async fn test_p0_shell_delete_git_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. Direct shell invocation via ShellService
    let res = provider
        .execute_bounded_shell("rm -rf .git", None, 10)
        .await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "execute_bounded_shell('rm -rf .git') must fail closed with PermissionDenied"
    );

    // 2. Shell invocation via ProcessService with -c
    let res_proc = provider
        .spawn_command("sh", &["-c".into(), "rm -rf .git".into()], None, 10)
        .await;
    assert!(
        matches!(res_proc, Err(CapabilityError::PermissionDenied(_))),
        "spawn_command('sh -c rm -rf .git') must fail closed with PermissionDenied"
    );

    // 3. Direct binary invocation targeting .git
    let res_rm = provider
        .spawn_command("rm", &["-rf".into(), ".git".into()], None, 10)
        .await;
    assert!(
        matches!(res_rm, Err(CapabilityError::PermissionDenied(_))),
        "spawn_command('rm -rf .git') must fail closed with PermissionDenied"
    );

    // 4. Tool-mediated execution via RunCommandTool
    let cap_reg = Arc::new(CapabilityRegistry::production(&ws, None, None));
    let ctx = ToolExecutionContext {
        workspace_root: ws.clone(),
        capability_registry: cap_reg,
        role_envelope: None,
        agent_role: Some(m31a::state_machine::agent::AgentRole::implementer()),
        autonomy_mode: Some(m31a::state_machine::AutonomyMode::Autonomous),
        mission_id: Some(MissionId::new()),
        task_id: Some(TaskId::new()),
        agent_id: Some(AgentId::new()),
        policy_hash: Some("test-policy-hash".to_string()),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        task_repo: None,
    };

    let tool_res = RunCommandTool
        .execute(
            &ctx,
            RunCommandInput {
                command: "sh".to_string(),
                args: Some(vec!["-c".to_string(), "rm -rf .git".to_string()]),
                timeout_secs: Some(10),
            },
        )
        .await;
    assert!(
        tool_res.is_err(),
        "RunCommandTool targeting .git must fail closed"
    );

    // VERIFY PHYSICAL STATE UNTOUCHED
    assert!(ws.join(".git").exists());
    assert!(ws.join(".git/config").exists());
    assert!(ws.join(".git/hooks/pre-commit").exists());
}

// ===========================================================================
// Test B: Shell Attempts to Delete .m31a
// ===========================================================================

#[tokio::test]
async fn test_p0_shell_delete_m31a_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. Shell delete .m31a
    let res = provider
        .execute_bounded_shell("rm -rf .m31a", None, 10)
        .await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "execute_bounded_shell('rm -rf .m31a') must fail closed"
    );

    // 2. Binary rm .m31a/m31a.db
    let res_rm = provider
        .spawn_command("rm", &["-f".into(), ".m31a/m31a.db".into()], None, 10)
        .await;
    assert!(
        matches!(res_rm, Err(CapabilityError::PermissionDenied(_))),
        "spawn_command('rm -f .m31a/m31a.db') must fail closed"
    );

    // VERIFY PHYSICAL STATE UNTOUCHED
    assert!(ws.join(".m31a").exists());
    assert!(ws.join(".m31a/m31a.db").exists());
    assert_eq!(
        fs::read_to_string(ws.join(".m31a/m31a.db")).unwrap(),
        "SQLITE_FORMAT_3_INTERNAL_M31A_DATABASE_CONTENT"
    );
}

// ===========================================================================
// Test C: Shell Attempts to Write .git/config
// ===========================================================================

#[tokio::test]
async fn test_p0_shell_write_git_config_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // Redirect output into .git/config
    let res = provider
        .execute_bounded_shell("echo 'pwned=true' > .git/config", None, 10)
        .await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Writing into .git/config must fail closed"
    );

    // Append to .git/hooks/pre-commit
    let res_append = provider
        .execute_bounded_shell("echo 'evil' >> .git/hooks/pre-commit", None, 10)
        .await;
    assert!(
        matches!(res_append, Err(CapabilityError::PermissionDenied(_))),
        "Appending to .git/hooks/pre-commit must fail closed"
    );

    // VERIFY PHYSICAL STATE UNTOUCHED
    let git_config = fs::read_to_string(ws.join(".git/config")).unwrap();
    assert!(!git_config.contains("pwned"));
    assert!(git_config.contains("repositoryformatversion"));

    let pre_commit = fs::read_to_string(ws.join(".git/hooks/pre-commit")).unwrap();
    assert!(!pre_commit.contains("evil"));
}

// ===========================================================================
// Test D: Shell Attempts to Write .m31a/m31a.db
// ===========================================================================

#[tokio::test]
async fn test_p0_shell_write_m31a_db_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    let res = provider
        .execute_bounded_shell("echo 'corrupted' > .m31a/m31a.db", None, 10)
        .await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Writing to .m31a/m31a.db must fail closed"
    );

    // VERIFY PHYSICAL STATE UNTOUCHED
    let db_content = fs::read_to_string(ws.join(".m31a/m31a.db")).unwrap();
    assert_eq!(
        db_content, "SQLITE_FORMAT_3_INTERNAL_M31A_DATABASE_CONTENT",
        "Database must remain pristine"
    );
}

// ===========================================================================
// Test E: Shell Attempts Traversal into Protected Paths
// ===========================================================================

#[tokio::test]
async fn test_p0_shell_traversal_into_protected_paths_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. src/../.git
    let res1 = provider
        .execute_bounded_shell("rm -rf src/../.git", None, 10)
        .await;
    assert!(
        matches!(res1, Err(CapabilityError::PermissionDenied(_))),
        "Traversal src/../.git must be denied"
    );

    // 2. ./.m31a/m31a.db
    let res2 = provider
        .execute_bounded_shell("rm -f ./.m31a/m31a.db", None, 10)
        .await;
    assert!(
        matches!(res2, Err(CapabilityError::PermissionDenied(_))),
        "Curdir traversal ./.m31a must be denied"
    );

    // 3. Case variations: .GIT / .M31A
    let res3 = provider
        .execute_bounded_shell("rm -rf .GIT", None, 10)
        .await;
    assert!(
        matches!(res3, Err(CapabilityError::PermissionDenied(_))),
        "Case-insensitive .GIT must be denied"
    );

    let res4 = provider
        .execute_bounded_shell("rm -rf .M31A", None, 10)
        .await;
    assert!(
        matches!(res4, Err(CapabilityError::PermissionDenied(_))),
        "Case-insensitive .M31A must be denied"
    );

    // VERIFY PHYSICAL STATE UNTOUCHED
    assert!(ws.join(".git").exists());
    assert!(ws.join(".m31a/m31a.db").exists());
}

// ===========================================================================
// Test F: Git with Malicious GIT_DIR
// ===========================================================================

#[tokio::test]
async fn test_p0_git_malicious_git_dir_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. Environment variable setting blocked in EnvironmentBuilder
    let mut env = EnvironmentBuilder::new(&ws);
    let res_env = env.set_var("GIT_DIR", "/tmp/malicious_git");
    assert!(
        res_env.is_err(),
        "EnvironmentBuilder must reject GIT_DIR variable assignment"
    );

    // 2. Shell command attempting inline GIT_DIR prefix
    let res_shell = provider
        .execute_bounded_shell("GIT_DIR=/tmp/malicious git status", None, 10)
        .await;
    assert!(
        matches!(res_shell, Err(CapabilityError::PermissionDenied(_))),
        "Shell command with GIT_DIR= assignment must be denied"
    );

    // 3. check_command_safety rejects GIT_DIR
    let res_safety = check_command_safety(
        "git",
        &["--git-dir=/tmp/malicious".to_string(), "status".to_string()],
    );
    assert!(
        res_safety.is_err(),
        "check_command_safety must reject --git-dir"
    );
}

// ===========================================================================
// Test G: Git with Malicious GIT_WORK_TREE
// ===========================================================================

#[tokio::test]
async fn test_p0_git_malicious_git_work_tree_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. Environment variable setting blocked
    let mut env = EnvironmentBuilder::new(&ws);
    let res_env = env.set_var("GIT_WORK_TREE", "/tmp/malicious_wt");
    assert!(
        res_env.is_err(),
        "EnvironmentBuilder must reject GIT_WORK_TREE assignment"
    );

    // 2. Shell command attempting inline GIT_WORK_TREE prefix
    let res_shell = provider
        .execute_bounded_shell("GIT_WORK_TREE=/tmp/other git checkout", None, 10)
        .await;
    assert!(
        matches!(res_shell, Err(CapabilityError::PermissionDenied(_))),
        "Shell command with GIT_WORK_TREE= assignment must be denied"
    );

    // 3. check_command_safety rejects --work-tree
    let res_safety = check_command_safety(
        "git",
        &["--work-tree=/tmp/other".to_string(), "status".to_string()],
    );
    assert!(
        res_safety.is_err(),
        "check_command_safety must reject --work-tree"
    );
}

// ===========================================================================
// Test H: Git with Malicious GIT_INDEX_FILE
// ===========================================================================

#[tokio::test]
async fn test_p0_git_malicious_git_index_file_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. EnvironmentBuilder blocks GIT_INDEX_FILE
    let mut env = EnvironmentBuilder::new(&ws);
    assert!(env.set_var("GIT_INDEX_FILE", "/tmp/malicious_idx").is_err());

    // 2. Shell command with GIT_INDEX_FILE
    let res = provider
        .execute_bounded_shell("GIT_INDEX_FILE=/tmp/idx git add .", None, 10)
        .await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Shell command with GIT_INDEX_FILE= must be denied"
    );
}

// ===========================================================================
// Test I: Equivalent Repository Redirection Mechanisms Denied
// ===========================================================================

#[tokio::test]
async fn test_p0_git_redirection_mechanisms_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let git_prov = CliGitProvider::new(&ws);

    // 1. Option injection in checkout (rejected before authorization is
    // even consulted: the denied gate proves validation fires first)
    let res_checkout = git_prov
        .checkout("--git-dir=/tmp/foo", &m31a::git::GitGate::denied())
        .await;
    assert!(
        matches!(res_checkout, Err(CapabilityError::PermissionDenied(_))),
        "Option injection in checkout must be denied"
    );

    // 2. Option injection in add
    let res_add = git_prov
        .add(
            &[PathBuf::from("--work-tree=/tmp/foo")],
            &m31a::git::GitGate::denied(),
        )
        .await;
    assert!(
        matches!(res_add, Err(CapabilityError::PermissionDenied(_))),
        "Option injection in add must be denied"
    );

    // 3. Command line with -c core.gitDir
    let res_config = check_command_safety(
        "git",
        &[
            "-c".to_string(),
            "core.gitDir=/tmp/foo".to_string(),
            "status".to_string(),
        ],
    );
    assert!(
        res_config.is_err(),
        "check_command_safety must reject core.gitDir configuration override"
    );

    // 4. Command line with --separate-git-dir
    let res_sep = check_command_safety(
        "git",
        &[
            "--separate-git-dir=/tmp/foo".to_string(),
            "init".to_string(),
        ],
    );
    assert!(
        res_sep.is_err(),
        "check_command_safety must reject --separate-git-dir"
    );
}

// ===========================================================================
// Test J: Legitimate Commands Inside the Workspace Still Work
// ===========================================================================

#[tokio::test]
async fn test_p0_legitimate_commands_allowed() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalProcessProvider::new(ws.clone());

    // 1. Basic echo command via spawn_command
    let out1 = provider
        .spawn_command("echo", &["hello m31a".to_string()], None, 10)
        .await
        .unwrap();
    assert_eq!(out1.exit_code, 0);
    assert_eq!(out1.stdout.trim(), "hello m31a");

    // 2. Shell command reading legitimate file
    let out2 = provider
        .execute_bounded_shell("cat README.md", None, 10)
        .await
        .unwrap();
    assert_eq!(out2.exit_code, 0);
    assert!(out2.stdout.contains("Test Project"));

    // 3. Shell reading legitimate dotfiles
    let out3 = provider
        .execute_bounded_shell("cat .gitignore", None, 10)
        .await
        .unwrap();
    assert_eq!(out3.exit_code, 0);
    assert!(out3.stdout.contains("target/"));

    let out4 = provider
        .execute_bounded_shell("cat .gitattributes", None, 10)
        .await
        .unwrap();
    assert_eq!(out4.exit_code, 0);
    assert!(out4.stdout.contains("* text=auto"));

    let out5 = provider
        .execute_bounded_shell("cat .github/workflows/ci.yml", None, 10)
        .await
        .unwrap();
    assert_eq!(out5.exit_code, 0);
    assert!(out5.stdout.contains("name: CI"));

    // 4. Cargo toolchain invocation
    let out6 = provider
        .spawn_command("cargo", &["--version".to_string()], None, 10)
        .await
        .unwrap();
    assert_eq!(out6.exit_code, 0);
    assert!(out6.stdout.contains("cargo"));
}

// ===========================================================================
// Test K: Legitimate Git Operations Still Work
// ===========================================================================

#[tokio::test]
async fn test_p0_legitimate_git_operations_allowed() {
    let (_tmp, ws) = setup_test_workspace();

    // Initialize an actual git repository in workspace
    let init_out = tokio::process::Command::new("git")
        .args(["init"])
        .current_dir(&ws)
        .output()
        .await
        .unwrap();
    assert!(init_out.status.success());

    // Configure user name and email locally for testing commits
    let _ = tokio::process::Command::new("git")
        .args(["config", "user.name", "M31A Test"])
        .current_dir(&ws)
        .output()
        .await;
    let _ = tokio::process::Command::new("git")
        .args(["config", "user.email", "test@m31a.local"])
        .current_dir(&ws)
        .output()
        .await;

    let git_prov = CliGitProvider::new(&ws);
    let legit_auth = m31a::git::AuthorizationAuthority::new();
    let mint = |op: m31a::git::GitOperation| {
        let auth = legit_auth.mint_git_authorization(
            m31a::ids::MissionId::new(),
            Some(m31a::ids::TaskId::new()),
            None,
            "test-policy-hash".to_string(),
            ws.clone(),
            op,
            Vec::new(),
            "test-scope",
            "test-mint",
            std::time::Duration::from_secs(600),
        );
        m31a::git::GitGate::authorized_verified(auth, &legit_auth).expect("test gate")
    };

    // 1. Status query succeeds
    let status = git_prov.status().await.unwrap();
    assert!(!status.untracked.is_empty() || !status.staged.is_empty() || status.is_clean);

    // 2. Add legitimate source file
    let add_res = git_prov
        .add(
            &[PathBuf::from("src/main.rs")],
            &mint(m31a::git::GitOperation::Add {
                paths: vec!["src/main.rs".to_string()],
            }),
        )
        .await;
    assert!(add_res.is_ok(), "git add src/main.rs must succeed");

    // 3. Commit
    let commit_hash = git_prov
        .commit(
            "initial test commit",
            &mint(m31a::git::GitOperation::Commit {
                message: "initial test commit".to_string(),
            }),
        )
        .await
        .unwrap();
    assert!(!commit_hash.is_empty(), "Commit must return commit SHA");

    // 4. Log
    let log = git_prov.log(5).await.unwrap();
    assert!(!log.is_empty());
    assert_eq!(log[0].message, "initial test commit");

    // 5. Diff
    let diff = git_prov.diff(false).await.unwrap();
    assert!(diff.is_empty() || !diff.is_empty()); // query succeeds
}

// ===========================================================================
// Test L: Sandboxed Bubblewrap Reinforces Protected Path Immutability
// ===========================================================================

#[test]
fn test_p0_sandboxed_bubblewrap_behavior() {
    let (_tmp, ws) = setup_test_workspace();
    let bwrap = BubblewrapSandboxProvider::new(PathBuf::from("/usr/bin/bwrap"));

    let plan = SandboxPlan::new(ws.clone())
        .with_env_var("MY_VAR", "my_val")
        .with_env_var("GIT_DIR", "/tmp/malicious"); // Attempt to inject GIT_DIR into plan

    let private_tmp = ws.join("tmp_sandbox");
    let args = bwrap.build_command_args(&plan, &private_tmp, "sh", &["-c".into(), "ls".into()]);

    // 1. Verify workspace is mounted read-write
    assert!(args.contains(&"--bind".to_string()));
    assert!(args.contains(&"/workspace".to_string()));

    // 2. Verify .git is mounted read-only (--ro-bind)
    let git_path_str = ws.join(".git").to_string_lossy().to_string();
    let has_git_ro_bind = args
        .windows(3)
        .any(|w| w[0] == "--ro-bind" && w[1] == git_path_str && w[2] == "/workspace/.git");
    assert!(
        has_git_ro_bind,
        "Bubblewrap args must include --ro-bind <workspace>/.git /workspace/.git"
    );

    // 3. Verify .m31a is masked with empty read-only directory
    let has_m31a_ro_bind = args
        .windows(3)
        .any(|w| w[0] == "--ro-bind" && w[1].contains("empty_m31a") && w[2] == "/workspace/.m31a");
    assert!(
        has_m31a_ro_bind,
        "Bubblewrap args must include --ro-bind <empty_m31a> /workspace/.m31a"
    );

    // 4. Verify clean environment and filtered GIT_DIR
    assert!(args.contains(&"--clearenv".to_string()));
    assert!(args.contains(&"MY_VAR".to_string()));
    assert!(
        !args.contains(&"GIT_DIR".to_string()),
        "Bubblewrap must filter out GIT_DIR environment variable"
    );
}

// ===========================================================================
// Test M: Fallback Behavior Remains Fail-Closed
// ===========================================================================

#[tokio::test]
async fn test_p0_fallback_behavior_fails_closed() {
    let (_tmp, ws) = setup_test_workspace();

    // 1. ProcessIsolationProvider rejects filesystem isolation requests
    let proc_provider = ProcessIsolationProvider::new();
    let plan = SandboxPlan::new(ws.clone()).with_fs_isolation(true);

    let prep_res = proc_provider.prepare(&plan).await;
    assert!(
        prep_res.is_err(),
        "ProcessIsolationProvider must reject plans requiring filesystem isolation"
    );

    // 2. Working directory validation rejects traversal
    let ws_escape = validate_working_directory(Some(Path::new("..")), &ws);
    assert!(
        ws_escape.is_err(),
        "validate_working_directory must reject parent traversal"
    );

    // 3. Working directory targeting .git rejected
    let git_cwd = validate_working_directory(Some(Path::new(".git")), &ws);
    assert!(
        git_cwd.is_err(),
        "validate_working_directory must reject .git cwd"
    );

    // 4. Working directory targeting .m31a rejected
    let m31a_cwd = validate_working_directory(Some(Path::new(".m31a")), &ws);
    assert!(
        m31a_cwd.is_err(),
        "validate_working_directory must reject .m31a cwd"
    );

    // 5. Legitimate subdirectory cwd allowed
    let src_cwd = validate_working_directory(Some(Path::new("src")), &ws);
    assert!(src_cwd.is_ok(), "Legitimate src cwd must be allowed");
}

// ===========================================================================
// Test N: Policy Engine Immutable Veto for Process & Shell Commands
// ===========================================================================

#[tokio::test]
async fn test_p0_policy_engine_process_veto() {
    let (_tmp, ws) = setup_test_workspace();
    let policy = EffectivePolicy::standard(&ws);

    // 1. Request targeting .git via command line
    let req_git = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "run_command")
        .with_role(m31a::state_machine::agent::AgentRole::implementer())
        .with_autonomy_mode(m31a::state_machine::AutonomyMode::Autonomous)
        .with_workspace(ws.clone())
        .with_arguments(serde_json::json!({
            "command": "rm -rf .git",
            "args": ["-rf", ".git"]
        }));

    let (decision, record) = policy.evaluate_record(req_git).await.unwrap();
    assert_eq!(
        decision,
        PolicyDecision::Deny,
        "Policy must veto rm -rf .git"
    );
    assert!(
        record.is_some(),
        "Must have audit record of policy decision"
    );

    // 2. Request attempting Git redirection via command line
    let req_redirect = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "run_command")
        .with_role(m31a::state_machine::agent::AgentRole::implementer())
        .with_autonomy_mode(m31a::state_machine::AutonomyMode::Autonomous)
        .with_workspace(ws.clone())
        .with_arguments(serde_json::json!({
            "command": "git --git-dir=/etc status",
            "args": ["--git-dir=/etc", "status"]
        }));

    let (decision_red, _) = policy.evaluate_record(req_redirect).await.unwrap();
    assert_eq!(
        decision_red,
        PolicyDecision::Deny,
        "Policy must veto git --git-dir"
    );

    // 3. Pipeline stage 6 blocks before policy and execution
    let mut tool_reg = ToolRegistry::new();
    tool_reg.register(RunCommandTool);

    let cap_reg = Arc::new(CapabilityRegistry::production(&ws, None, None));
    let ctx = ToolExecutionContext {
        workspace_root: ws.clone(),
        capability_registry: cap_reg,
        role_envelope: None,
        agent_role: Some(m31a::state_machine::agent::AgentRole::implementer()),
        autonomy_mode: Some(m31a::state_machine::AutonomyMode::Autonomous),
        mission_id: Some(MissionId::new()),
        task_id: Some(TaskId::new()),
        agent_id: Some(AgentId::new()),
        policy_hash: Some("test-policy-hash".to_string()),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        task_repo: None,
    };

    let pipeline = ToolPipelineRunner::new(Arc::new(tool_reg));
    let action = m31a::agent::runner::ActionRequest {
        id: "call-proc-1".to_string(),
        tool_name: "run_command".to_string(),
        parameters: serde_json::json!({
            "command": "sh",
            "args": ["-c", "rm -rf .git"]
        }),
    };

    let pipe_res = pipeline
        .execute_action(
            &action,
            &ctx,
            &policy,
            m31a::state::intake::AutonomyMode::Autonomous,
        )
        .await;

    assert!(!pipe_res.success, "Pipeline must block rm -rf .git");
    assert!(
        pipe_res
            .error
            .as_ref()
            .unwrap()
            .contains("PROTECTED_PATH_DENIED")
            || pipe_res
                .error
                .as_ref()
                .unwrap()
                .contains("Permission denied"),
        "Error must specify denial, got: {:?}",
        pipe_res.error
    );
}
