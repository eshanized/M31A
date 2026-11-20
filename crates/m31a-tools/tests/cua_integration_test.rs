use m31a_core::SandboxPolicy;
use m31a_tools::ToolRegistry;
use tempfile::tempdir;

#[tokio::test]
async fn test_cua_tools_execution_via_registry() {
    unsafe {
        std::env::set_var("M31A_SIMULATED_CUA", "1");
    }
    let tmp = tempdir().unwrap();
    let cwd = tmp.path();
    let registry = ToolRegistry::new();

    // 1. cua_list_apps
    let list_res = registry
        .execute(
            "cua_list_apps",
            "{}",
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .expect("Failed to execute cua_list_apps");

    assert!(list_res.contains("M31A Text Editor"));
    assert!(list_res.contains("Calculator"));

    // 2. cua_get_state
    let get_state_args = serde_json::json!({
        "app_id": "app_editor",
        "capture_background": true
    });
    let state_res = registry
        .execute(
            "cua_get_state",
            &get_state_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .expect("Failed to execute cua_get_state");

    let state_json: serde_json::Value = serde_json::from_str(&state_res).unwrap();
    let state_id = state_json["state_id"].as_str().unwrap().to_string();
    assert_eq!(state_json["app_name"], "M31A Text Editor");
    assert_eq!(state_json["element_count"], 6);

    // 3. cua_action (mutating textfield)
    let action_args = serde_json::json!({
        "app_id": "app_editor",
        "target_index": 4,
        "action_type": "set_value",
        "value": "fn verified_by_m31a() {}",
        "state_id": state_id
    });
    let action_res = registry
        .execute(
            "cua_action",
            &action_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .expect("Failed to execute cua_action");

    let action_json: serde_json::Value = serde_json::from_str(&action_res).unwrap();
    assert_eq!(action_json["success"], true);
    assert_eq!(action_json["action_sent"], true);
    let new_state_id = action_json["new_state_id"].as_str().unwrap().to_string();
    assert_ne!(new_state_id, state_id);

    // 4. cua_key
    let key_args = serde_json::json!({
        "app_id": "app_editor",
        "keys": " // added key",
        "state_id": new_state_id
    });
    let key_res = registry
        .execute(
            "cua_key",
            &key_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .expect("Failed to execute cua_key");

    let key_json: serde_json::Value = serde_json::from_str(&key_res).unwrap();
    assert_eq!(key_json["success"], true);

    // 5. cua_mouse
    let mouse_args = serde_json::json!({
        "action": "click",
        "x": 100,
        "y": 200,
        "button": "left"
    });
    let mouse_res = registry
        .execute(
            "cua_mouse",
            &mouse_args.to_string(),
            cwd,
            SandboxPolicy::WorkspaceWrite,
        )
        .await
        .expect("Failed to execute cua_mouse");

    let mouse_json: serde_json::Value = serde_json::from_str(&mouse_res).unwrap();
    assert_eq!(mouse_json["success"], true);

    // 6. Read-Only sandbox policy gating: cua_action should be rejected in ReadOnly mode
    let ro_err = registry
        .execute(
            "cua_action",
            &action_args.to_string(),
            cwd,
            SandboxPolicy::ReadOnly,
        )
        .await
        .unwrap_err();

    assert!(ro_err.to_string().contains("Sandbox policy is Read-Only"));
}
