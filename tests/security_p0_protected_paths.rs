//! Comprehensive P0 Security Verification Suite: Protected Path Boundary Remediation.
//!
//! Verifies that protected repository and runtime state (`.git` and `.m31a`) cannot be
//! reached, read, written, modified, or traversed through ordinary capability execution,
//! model-facing tools, pipeline stages, policy evaluations, or symlink aliases.
//!
//! Validates 12 specific threat vectors:
//! 1. Direct `.git` and `.m31a` access
//! 2. `../` directory traversal escapes into protected paths
//! 3. Absolute path aliases
//! 4. Relative path aliases (`./`, `@`, etc.)
//! 5. Redundant path components and separators (`/./`, `//`)
//! 6. Symlink aliases (direct, nested, and ancestors of new files)
//! 7. Nested protected paths (`submodule/.git`, `vendor/.m31a`)
//! 8. Tool-mediated access (read_file, write_file, edit_file, apply_patch, delete_file, list_files, glob, grep)
//! 9. Directory listing and recursive filtering (ensuring .git/.m31a are never enumerated or leaked)
//! 10. Pipeline ResourceScopeStage fail-closed behavior before execution
//! 11. Policy engine immutable veto at Layer 0 (BuiltInSafety)
//! 12. Allowed legitimate workspace operations (.gitignore, .gitattributes, .github, source code)

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> std::io::Result<()> {
    let orig = original.as_ref();
    let lk = link.as_ref();
    if orig.is_dir() {
        std::os::windows::fs::symlink_dir(orig, lk)
    } else {
        std::os::windows::fs::symlink_file(orig, lk)
    }
}
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;

use m31a::capability::error::CapabilityError;
use m31a::capability::providers::LocalFileSystemProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::capability::traits::fs::FileSystemService;
use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::policy::effective::EffectivePolicy;
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::error::ToolError;
use m31a::tools::fs::{
    ApplyPatchInput, ApplyPatchTool, EditFileInput, EditFileTool, GlobInput, GlobTool, GrepInput,
    GrepTool, ListFilesInput, ListFilesTool, ReadFileInput, ReadFileTool, WriteFileInput,
    WriteFileTool,
};
use m31a::tools::registry::ToolRegistry;

/// Helper to set up a realistic mock workspace containing legitimate files,
/// dummy `.git` metadata, and dummy `.m31a` runtime data.
fn setup_test_workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().expect("failed to create temp dir");
    let ws = dir.path().join("workspace");
    fs::create_dir_all(&ws).expect("failed to create workspace");

    // Legitimate workspace files
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(ws.join("src/main.rs"), "fn main() { println!(\"hello\"); }").unwrap();
    fs::write(ws.join("README.md"), "# Test Project").unwrap();
    fs::write(ws.join(".gitignore"), "target/\n.m31a/\n").unwrap();
    fs::write(ws.join(".gitattributes"), "* text=auto\n").unwrap();

    fs::create_dir_all(ws.join(".github/workflows")).unwrap();
    fs::write(
        ws.join(".github/workflows/ci.yml"),
        "name: CI\non: [push]\n",
    )
    .unwrap();

    // Protected .git directory
    fs::create_dir_all(ws.join(".git/hooks")).unwrap();
    fs::create_dir_all(ws.join(".git/objects")).unwrap();
    fs::write(
        ws.join(".git/config"),
        "[core]\n\trepositoryformatversion = 0\n",
    )
    .unwrap();
    fs::write(ws.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

    // Protected .m31a directory
    fs::create_dir_all(ws.join(".m31a/artifacts")).unwrap();
    fs::write(ws.join(".m31a/m31a.db"), "SQLITE_MOCK_DB_BYTES").unwrap();
    fs::write(ws.join(".m31a/config.toml"), "profile = \"default\"\n").unwrap();

    (dir, ws)
}

// ===========================================================================
// Test 1: Direct .git and .m31a Access Denied
// ===========================================================================

#[tokio::test]
async fn test_p0_direct_protected_paths_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // Direct access to .git
    let res = provider.resolve_and_verify(Path::new(".git"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Must reject direct access to .git, got: {res:?}"
    );

    let res = provider.resolve_and_verify(Path::new(".git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Must reject direct access to .git/config, got: {res:?}"
    );

    let res = provider.resolve_and_verify(Path::new(".git/HEAD"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Must reject direct access to .git/HEAD, got: {res:?}"
    );

    // Direct access to .m31a
    let res = provider.resolve_and_verify(Path::new(".m31a"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Must reject direct access to .m31a, got: {res:?}"
    );

    let res = provider.resolve_and_verify(Path::new(".m31a/m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Must reject direct access to .m31a/m31a.db, got: {res:?}"
    );

    let res = provider.resolve_and_verify(Path::new(".m31a/config.toml"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Must reject direct access to .m31a/config.toml, got: {res:?}"
    );

    // Verify operations fail-closed and no read occurs
    assert!(
        provider
            .read_file(Path::new(".git/config"), None, None)
            .await
            .is_err()
    );
    assert!(
        provider
            .read_file(Path::new(".m31a/m31a.db"), None, None)
            .await
            .is_err()
    );

    // Verify state unchanged
    let git_config = fs::read_to_string(ws.join(".git/config")).unwrap();
    assert!(git_config.contains("repositoryformatversion = 0"));
}

// ===========================================================================
// Test 2: Directory Traversal into Protected Paths Denied
// ===========================================================================

#[tokio::test]
async fn test_p0_traversal_into_protected_paths_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // From src/ to .git
    let res = provider.resolve_and_verify(Path::new("src/../.git"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Traversal into .git must be denied"
    );

    let res = provider.resolve_and_verify(Path::new("src/../.git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Traversal into .git/config must be denied"
    );

    // Deep traversal
    fs::create_dir_all(ws.join("a/b/c")).unwrap();
    let res = provider.resolve_and_verify(Path::new("a/b/c/../../../.m31a/m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Deep traversal into .m31a must be denied"
    );

    // Traversal write attempt fails-closed
    let write_res = provider
        .write_file(
            Path::new("src/../.git/hooks/pre-commit"),
            b"malicious script",
        )
        .await;
    assert!(
        write_res.is_err(),
        "Write via traversal into .git must fail"
    );

    // Assert file was NOT created
    assert!(
        !ws.join(".git/hooks/pre-commit").exists(),
        "Side effect must not have occurred on disk"
    );
}

// ===========================================================================
// Test 3: Absolute and Relative Path Aliases Denied
// ===========================================================================

#[tokio::test]
async fn test_p0_absolute_and_relative_path_aliases_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // Absolute paths within workspace targeting .git and .m31a
    let abs_git = ws.join(".git");
    let res = provider.resolve_and_verify(&abs_git);
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Absolute path alias to .git must be denied"
    );

    let abs_m31a_db = ws.join(".m31a/m31a.db");
    let res = provider.resolve_and_verify(&abs_m31a_db);
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Absolute path alias to .m31a/m31a.db must be denied"
    );

    // Relative aliases with ./ and @
    let res = provider.resolve_and_verify(Path::new("./.git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "./.git/config must be denied"
    );

    let res = provider.resolve_and_verify(Path::new("@.git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "@.git/config must be denied"
    );

    let res = provider.resolve_and_verify(Path::new("@/.m31a/m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "@/.m31a/m31a.db must be denied"
    );
}

// ===========================================================================
// Test 4: Redundant Path Components and Separators Denied
// ===========================================================================

#[tokio::test]
async fn test_p0_redundant_path_components_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // Embedded . and redundant slashes
    let res = provider.resolve_and_verify(Path::new(".git/././config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        ".git/././config must be denied"
    );

    let res = provider.resolve_and_verify(Path::new(".m31a/././m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        ".m31a/././m31a.db must be denied"
    );

    let res = provider.resolve_and_verify(Path::new("src/./../../.git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Redundant traversal into .git must be denied"
    );

    // Redundant slashes
    let res = provider.resolve_and_verify(Path::new(".git//config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        ".git//config must be denied"
    );
}

// ===========================================================================
// Test 5: Symlink Aliases Denied (Direct, Nested, and Ancestor)
// ===========================================================================

#[tokio::test]
async fn test_p0_symlink_aliases_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // 1. Direct symlink to .git
    let sym_git = ws.join("symlink_git");
    if let Err(e) = symlink(ws.join(".git"), &sym_git) {
        #[cfg(windows)]
        {
            eprintln!("Skipping symlink test on Windows (privilege not held): {e}");
            return;
        }
        #[cfg(not(windows))]
        panic!("Failed to create symlink: {e}");
    }

    let res = provider.resolve_and_verify(Path::new("symlink_git"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Symlink directly to .git must be denied"
    );

    let res = provider.resolve_and_verify(Path::new("symlink_git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Path through symlink to .git/config must be denied"
    );

    // 2. Direct symlink to .m31a
    let sym_m31a = ws.join("symlink_m31a");
    symlink(ws.join(".m31a"), &sym_m31a).unwrap();

    let res = provider.resolve_and_verify(Path::new("symlink_m31a/m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Path through symlink to .m31a/m31a.db must be denied"
    );

    // 3. Nested symlink inside subdirectory pointing to .git
    fs::create_dir_all(ws.join("sub")).unwrap();
    let nested_sym = ws.join("sub/leak_git");
    symlink(ws.join(".git/config"), &nested_sym).unwrap();

    let res = provider.resolve_and_verify(Path::new("sub/leak_git"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Nested symlink to .git/config must be denied"
    );

    // 4. Attempt to write a non-existent file through a symlink to .git
    let write_res = provider
        .write_file(Path::new("symlink_git/injected_file.txt"), b"INJECTION")
        .await;
    assert!(
        write_res.is_err(),
        "Write through symlink into .git must fail"
    );

    assert!(
        !ws.join(".git/injected_file.txt").exists(),
        "Injected file through symlink must NOT exist on disk"
    );
}

// ===========================================================================
// Test 6: Nested Protected Paths Denied
// ===========================================================================

#[tokio::test]
async fn test_p0_nested_protected_paths_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // Nested submodule .git
    let nested_git = ws.join("modules/submod/.git");
    fs::create_dir_all(&nested_git).unwrap();
    fs::write(nested_git.join("config"), "submodule config").unwrap();

    let res = provider.resolve_and_verify(Path::new("modules/submod/.git/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Nested submodule .git must be denied"
    );

    // Nested .m31a
    let nested_m31a = ws.join("packages/pkg_a/.m31a");
    fs::create_dir_all(&nested_m31a).unwrap();
    fs::write(nested_m31a.join("m31a.db"), "nested db").unwrap();

    let res = provider.resolve_and_verify(Path::new("packages/pkg_a/.m31a/m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Nested package .m31a must be denied"
    );
}

// ===========================================================================
// Test 7: Platform Path Edge Cases (Backslashes and Case Sensitivity)
// ===========================================================================

#[tokio::test]
async fn test_p0_platform_edge_cases_denied() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // Windows-style backslashes
    let res = provider.resolve_and_verify(Path::new(".git\\config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Backslash .git\\config must be denied"
    );

    let res = provider.resolve_and_verify(Path::new(".m31a\\m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Backslash .m31a\\m31a.db must be denied"
    );

    // Case-insensitive variants
    let res = provider.resolve_and_verify(Path::new(".GIT/config"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Case-variant .GIT/config must be denied"
    );

    let res = provider.resolve_and_verify(Path::new(".M31A/m31a.db"));
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Case-variant .M31A/m31a.db must be denied"
    );
}

// ===========================================================================
// Test 8: Directory Listing and Recursive Filtering
// ===========================================================================

#[tokio::test]
async fn test_p0_directory_listing_and_filtering() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // 1. Direct listing of .git must fail
    let res = provider.list_files(Path::new(".git"), false).await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Direct list_files on .git must be denied"
    );

    // 2. Direct listing of .m31a must fail
    let res = provider.list_files(Path::new(".m31a"), false).await;
    assert!(
        matches!(res, Err(CapabilityError::PermissionDenied(_))),
        "Direct list_files on .m31a must be denied"
    );

    // 3. Recursive listing of workspace root must NEVER include .git or .m31a
    let all_files = provider.list_files(Path::new("."), true).await.unwrap();

    for f in &all_files {
        for c in f.components() {
            let name = c.as_os_str().to_string_lossy();
            assert_ne!(
                name,
                ".git",
                "list_files leaked .git entry: {}",
                f.display()
            );
            assert_ne!(
                name,
                ".m31a",
                "list_files leaked .m31a entry: {}",
                f.display()
            );
        }
    }

    // Must include legitimate files
    assert!(all_files.contains(&PathBuf::from("src/main.rs")));
    assert!(all_files.contains(&PathBuf::from("README.md")));
    assert!(all_files.contains(&PathBuf::from(".gitignore")));
    assert!(all_files.contains(&PathBuf::from(".gitattributes")));
    assert!(all_files.contains(&PathBuf::from(".github/workflows/ci.yml")));
}

// ===========================================================================
// Test 9: Tool-Mediated Access Fails Closed
// ===========================================================================

#[tokio::test]
async fn test_p0_tool_mediated_access_fails_closed() {
    let (_tmp, ws) = setup_test_workspace();
    let cap_reg = Arc::new(CapabilityRegistry::production(&ws, None, None));
    let mut tool_reg = ToolRegistry::new();
    tool_reg.register(ReadFileTool);
    tool_reg.register(WriteFileTool);
    tool_reg.register(EditFileTool);
    tool_reg.register(ApplyPatchTool);
    tool_reg.register(ListFilesTool);
    tool_reg.register(GlobTool);
    tool_reg.register(GrepTool);

    let ctx = ToolExecutionContext {
        workspace_root: ws.clone(),
        capability_registry: cap_reg.clone(),
        role_envelope: None,
        agent_role: Some(m31a::state_machine::agent::AgentRole::implementer()),
        autonomy_mode: Some(m31a::state_machine::AutonomyMode::Autonomous),
        mission_id: Some(MissionId::new()),
        task_id: Some(TaskId::new()),
        agent_id: Some(AgentId::new()),
        policy_hash: Some("test-policy-hash".to_string()),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
    };

    // 1. ReadFileTool
    let res = ReadFileTool
        .execute(
            &ctx,
            ReadFileInput {
                path: ".git/config".to_string(),
                offset: None,
                limit: None,
            },
        )
        .await;
    assert!(
        matches!(res, Err(ToolError::PermissionDenied { .. })),
        "ReadFileTool on .git/config must fail with PermissionDenied"
    );

    // 2. WriteFileTool
    let res = WriteFileTool
        .execute(
            &ctx,
            WriteFileInput {
                path: ".m31a/evil.txt".to_string(),
                content: "malicious".to_string(),
            },
        )
        .await;
    assert!(
        matches!(res, Err(ToolError::PermissionDenied { .. })),
        "WriteFileTool on .m31a/evil.txt must fail with PermissionDenied"
    );
    assert!(
        !ws.join(".m31a/evil.txt").exists(),
        "evil.txt must not exist"
    );

    // 3. EditFileTool
    let res = EditFileTool
        .execute(
            &ctx,
            EditFileInput {
                path: ".git/config".to_string(),
                old_content: "repositoryformatversion".to_string(),
                new_content: "tampered".to_string(),
                ..Default::default()
            },
        )
        .await;
    assert!(
        matches!(res, Err(ToolError::PermissionDenied { .. })),
        "EditFileTool on .git/config must fail with PermissionDenied"
    );

    // 4. ApplyPatchTool
    let patch =
        "--- a/.git/config\n+++ b/.git/config\n@@ -1,2 +1,2 @@\n-[core]\n+[core_tampered]\n";
    let res = ApplyPatchTool
        .execute(
            &ctx,
            ApplyPatchInput {
                path: ".git/config".to_string(),
                patch: patch.to_string(),
            },
        )
        .await;
    assert!(
        matches!(res, Err(ToolError::PermissionDenied { .. })),
        "ApplyPatchTool on .git/config must fail with PermissionDenied"
    );

    // 5. ListFilesTool
    let res = ListFilesTool
        .execute(
            &ctx,
            ListFilesInput {
                path: Some(".git".to_string()),
                recursive: Some(false),
            },
        )
        .await;
    assert!(
        matches!(res, Err(ToolError::PermissionDenied { .. })),
        "ListFilesTool on .git must fail with PermissionDenied"
    );

    // 6. GlobTool matching .git
    let glob_res = GlobTool
        .execute(
            &ctx,
            GlobInput {
                pattern: "**/*".to_string(),
                path: None,
            },
        )
        .await
        .unwrap();

    for m in &glob_res.matches {
        let path = Path::new(m);
        for c in path.components() {
            let name = c.as_os_str().to_string_lossy();
            assert_ne!(name, ".git", "Glob match leaked protected path: {m}");
            assert_ne!(name, ".m31a", "Glob match leaked protected path: {m}");
        }
    }

    // 7. GrepTool searching in workspace
    let grep_res = GrepTool
        .execute(
            &ctx,
            GrepInput {
                pattern: "repositoryformatversion".to_string(),
                path: None,
                case_insensitive: Some(false),
            },
        )
        .await
        .unwrap();

    // Grep must find 0 matches because .git/config was not searched
    assert_eq!(
        grep_res.matches.len(),
        0,
        "Grep must not match into .git/config"
    );
}

// ===========================================================================
// Test 10: Pipeline ResourceScopeStage Blocks Before Policy Evaluation
// ===========================================================================

#[tokio::test]
async fn test_p0_pipeline_resource_scope_stage_blocks_before_execution() {
    let (_tmp, ws) = setup_test_workspace();
    let cap_reg = Arc::new(CapabilityRegistry::production(&ws, None, None));
    let mut tool_reg = ToolRegistry::new();
    tool_reg.register(ReadFileTool);
    tool_reg.register(WriteFileTool);

    let ctx = ToolExecutionContext {
        workspace_root: ws.clone(),
        capability_registry: cap_reg.clone(),
        role_envelope: None,
        agent_role: Some(m31a::state_machine::agent::AgentRole::implementer()),
        autonomy_mode: Some(m31a::state_machine::AutonomyMode::Autonomous),
        mission_id: Some(MissionId::new()),
        task_id: Some(TaskId::new()),
        agent_id: Some(AgentId::new()),
        policy_hash: Some("test-policy-hash".to_string()),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
    };

    let pipeline = m31a::pipeline::runner::ToolPipelineRunner::new(Arc::new(tool_reg));
    let policy = EffectivePolicy::standard(&ws);

    // Execute action targeting .git/config
    let action = m31a::agent::runner::ActionRequest {
        id: "call-1".to_string(),
        tool_name: "read_file".to_string(),
        parameters: serde_json::json!({ "path": ".git/config" }),
    };

    let result = pipeline
        .execute_action(
            &action,
            &ctx,
            &policy,
            m31a::state::intake::AutonomyMode::Autonomous,
        )
        .await;

    assert!(!result.success, "Action targeting .git must fail");
    assert!(
        result
            .error
            .as_ref()
            .unwrap()
            .contains("PROTECTED_PATH_DENIED")
            || result.error.as_ref().unwrap().contains("Permission denied"),
        "Error must specify protected path denial, got: {:?}",
        result.error
    );

    // Execute action targeting .m31a/m31a.db via write_file
    let action_write = m31a::agent::runner::ActionRequest {
        id: "call-2".to_string(),
        tool_name: "write_file".to_string(),
        parameters: serde_json::json!({ "path": ".m31a/m31a.db", "content": "corrupted" }),
    };

    let result_write = pipeline
        .execute_action(
            &action_write,
            &ctx,
            &policy,
            m31a::state::intake::AutonomyMode::Autonomous,
        )
        .await;

    assert!(!result_write.success, "Write targeting .m31a must fail");
    let db_content = fs::read_to_string(ws.join(".m31a/m31a.db")).unwrap();
    assert_eq!(
        db_content, "SQLITE_MOCK_DB_BYTES",
        "Database must remain completely untouched"
    );
}

// ===========================================================================
// Test 11: Policy Engine Immutable Veto at Layer 0 (BuiltInSafety)
// ===========================================================================

#[tokio::test]
async fn test_p0_policy_engine_immutable_veto() {
    let (_tmp, ws) = setup_test_workspace();
    let policy = EffectivePolicy::standard(&ws);

    // 1. Direct request with .git/config
    let req_git = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "read_file")
        .with_role(m31a::state_machine::agent::AgentRole::implementer())
        .with_autonomy_mode(m31a::state_machine::AutonomyMode::Autonomous)
        .with_workspace(ws.clone())
        .with_arguments(serde_json::json!({ "path": ".git/config" }));

    let (decision, record) = policy.evaluate_record(req_git).await.unwrap();
    assert_eq!(
        decision,
        PolicyDecision::Deny,
        "Policy must veto .git/config"
    );
    let record = record.unwrap();
    assert_eq!(
        record.matched_rule_id.as_deref(),
        Some("veto-protected-runtime-paths"),
        "Must match veto-protected-runtime-paths rule"
    );

    // 2. Direct request with .m31a/m31a.db
    let req_m31a = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "write_file")
        .with_role(m31a::state_machine::agent::AgentRole::implementer())
        .with_autonomy_mode(m31a::state_machine::AutonomyMode::Autonomous)
        .with_workspace(ws.clone())
        .with_arguments(serde_json::json!({ "path": ".m31a/m31a.db" }));

    let (decision_m31a, record_m31a) = policy.evaluate_record(req_m31a).await.unwrap();
    assert_eq!(
        decision_m31a,
        PolicyDecision::Deny,
        "Policy must veto .m31a/m31a.db"
    );
    assert_eq!(
        record_m31a.unwrap().matched_rule_id.as_deref(),
        Some("veto-protected-runtime-paths")
    );
}

// ===========================================================================
// Test 12: Legitimate Workspace Operations Allowed
// ===========================================================================

#[tokio::test]
async fn test_p0_legitimate_workspace_operations_allowed() {
    let (_tmp, ws) = setup_test_workspace();
    let provider = LocalFileSystemProvider::new(&ws).unwrap();

    // 1. Read existing code file
    let content = provider
        .read_file(Path::new("src/main.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&content),
        "fn main() { println!(\"hello\"); }"
    );

    // 2. Write new source file
    let written = provider
        .write_file(
            Path::new("src/lib.rs"),
            b"pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )
        .await
        .unwrap();
    assert!(written > 0);
    assert!(ws.join("src/lib.rs").exists());

    // 3. Edit source file
    provider
        .edit_file(Path::new("src/main.rs"), "hello", "world")
        .await
        .unwrap();
    let updated = provider
        .read_file(Path::new("src/main.rs"), None, None)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&updated).contains("world"));

    // 4. Dotfiles that are NOT protected (.gitignore, .gitattributes, .github)
    let gitignore = provider
        .read_file(Path::new(".gitignore"), None, None)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&gitignore).contains("target/"));

    let gitattrib = provider
        .read_file(Path::new(".gitattributes"), None, None)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&gitattrib).contains("* text=auto"));

    let ci_yaml = provider
        .read_file(Path::new(".github/workflows/ci.yml"), None, None)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&ci_yaml).contains("name: CI"));

    // 5. Delete legitimate file
    provider.delete_file(Path::new("src/lib.rs")).await.unwrap();
    assert!(!ws.join("src/lib.rs").exists());
}
