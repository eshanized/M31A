//! Integration tests for tool registry and execution harness.
//!
//! Tests built-in tools (file ops, git, search, patch), input repair, and parallel dispatching.

use m31a_core::SandboxPolicy;
use m31a_tools::ToolRegistry;
use tempfile::tempdir;

#[tokio::test]
async fn test_filesystem_tools_lifecycle() {
    let tmp = tempdir().unwrap();
    let cwd = tmp.path();
    let registry = ToolRegistry::new();

    // 1. write_file
    let write_args = serde_json::json!({
        "path": "src/hello.rs",
        "content": "pub fn hello() -> &'static str { \"world\" }\n"
    });
    let write_res = registry
        .execute(
            "write_file",
            &write_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .unwrap();
    assert!(
        write_res.contains("success")
            || write_res.contains("written")
            || write_res.contains("src/hello.rs")
    );
    assert!(cwd.join("src/hello.rs").exists());

    // 2. read_file
    let read_args = serde_json::json!({
        "path": "src/hello.rs"
    });
    let read_res = registry
        .execute(
            "read_file",
            &read_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .unwrap();
    assert!(read_res.contains("pub fn hello()"));

    // 3. list_dir
    let list_args = serde_json::json!({
        "path": "src"
    });
    let list_res = registry
        .execute(
            "list_dir",
            &list_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .unwrap();
    assert!(list_res.contains("hello.rs"));

    // 4. file_search
    let search_args = serde_json::json!({
        "pattern": "*.rs"
    });
    let search_res = registry
        .execute(
            "file_search",
            &search_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .unwrap();
    assert!(search_res.contains("src/hello.rs"));
}

#[tokio::test]
async fn test_apply_patch_tool_integration() {
    let tmp = tempdir().unwrap();
    let cwd = tmp.path();
    let registry = ToolRegistry::new();

    // Create an initial file
    let initial_file = cwd.join("config.txt");
    std::fs::write(&initial_file, "setting_a = 10\nsetting_b = 20\n").unwrap();

    // V4A Patch to update config.txt and add new_file.txt
    let patch_text = r#"*** Begin Patch
*** Update File: config.txt
@@
-setting_a = 10
+setting_a = 99
 setting_b = 20
*** Add File: new_file.txt
+alpha
+beta
*** End Patch
"#;

    let patch_args = serde_json::json!({
        "patch": patch_text
    });

    let patch_res = registry
        .execute(
            "apply_patch",
            &patch_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .unwrap();

    assert!(
        patch_res.to_lowercase().contains("modified")
            || patch_res.contains("config.txt")
            || patch_res.to_lowercase().contains("applied")
            || patch_res.to_lowercase().contains("created")
    );

    let updated_config = std::fs::read_to_string(&initial_file).unwrap();
    assert!(updated_config.contains("setting_a = 99"));

    let new_file_path = cwd.join("new_file.txt");
    assert!(new_file_path.exists());
    let new_content = std::fs::read_to_string(&new_file_path).unwrap();
    assert!(new_content.contains("alpha\nbeta"));
}

#[tokio::test]
async fn test_exec_command_tool_integration() {
    let tmp = tempdir().unwrap();
    let cwd = tmp.path();
    let registry = ToolRegistry::new();

    let exec_args = serde_json::json!({
        "command": "echo 'm31a-real-exec-output'"
    });

    let exec_res = registry
        .execute(
            "exec_command",
            &exec_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .unwrap();

    assert!(exec_res.contains("m31a-real-exec-output"));
}

#[tokio::test]
async fn test_readonly_sandbox_policy_enforcement() {
    let tmp = tempdir().unwrap();
    let cwd = tmp.path();
    let registry = ToolRegistry::new();

    let write_args = serde_json::json!({
        "path": "blocked.txt",
        "content": "should fail"
    });

    let res = registry
        .execute(
            "write_file",
            &write_args.to_string(),
            cwd,
            SandboxPolicy::ReadOnly,
        )
        .await;

    // ReadOnly policy must deny write operations
    assert!(
        res.is_err(),
        "Write should be denied under ReadOnly sandbox policy"
    );

    let deploy_res = registry
        .execute(
            "cloud_deploy",
            r#"{"target": "vercel"}"#,
            cwd,
            SandboxPolicy::ReadOnly,
        )
        .await;
    assert!(
        deploy_res.is_err(),
        "cloud_deploy should be denied under ReadOnly sandbox policy"
    );

    let migrate_res = registry
        .execute(
            "supabase_migrate",
            r#"{"sql": "SELECT 1;"}"#,
            cwd,
            SandboxPolicy::ReadOnly,
        )
        .await;
    assert!(
        migrate_res.is_err(),
        "supabase_migrate should be denied under ReadOnly sandbox policy"
    );

    let copy_res = registry
        .execute(
            "multi_env_copy_file",
            r#"{"source_environment_id": "env1", "source_path": "/tmp/a", "target_environment_id": "env2", "target_path": "/tmp/b"}"#,
            cwd,
            SandboxPolicy::ReadOnly,
        )
        .await;
    assert!(
        copy_res.is_err(),
        "multi_env_copy_file should be denied under ReadOnly sandbox policy"
    );

    let image_res = registry
        .execute(
            "imagegen",
            r#"{"prompt": "A sunset"}"#,
            cwd,
            SandboxPolicy::ReadOnly,
        )
        .await;
    assert!(
        image_res.is_err(),
        "imagegen should be denied under ReadOnly sandbox policy"
    );
}
