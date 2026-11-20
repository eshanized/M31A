use m31a_cua::{
    CuaError, CuaSessionManager, ElementAction, ElementRole, MouseAction, SimulatedBackend,
};
use std::sync::Arc;

#[tokio::test]
async fn test_list_apps() {
    let backend = Arc::new(SimulatedBackend::default());
    let session = CuaSessionManager::new(backend);

    let apps = session.list_apps().await.expect("Failed to list apps");
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].name, "M31A Text Editor");
    assert!(apps[0].is_active);
    assert_eq!(apps[1].name, "Calculator");
}

#[tokio::test]
async fn test_get_app_state_and_tree_traversal() {
    let backend = Arc::new(SimulatedBackend::default());
    let session = CuaSessionManager::new(backend);

    let state = session
        .get_app_state(Some("app_editor"), true)
        .await
        .expect("Failed to get editor state");

    assert_eq!(state.app_name, "M31A Text Editor");
    assert_eq!(state.state_id, "state_editor_v1");
    assert_eq!(state.element_count, 6);

    // Verify root is Window
    assert_eq!(state.root.role, ElementRole::Window);
    assert_eq!(state.root.children.len(), 3);

    // Verify TextArea exists at index 4
    let text_area = state.root.find_by_index(4).expect("TextArea not found");
    assert_eq!(text_area.role, ElementRole::TextArea);
    assert!(text_area.value.as_ref().unwrap().contains("println!"));

    // Verify Button exists at index 5
    let save_btn = state.root.find_by_index(5).expect("Button not found");
    assert_eq!(save_btn.role, ElementRole::Button);
    assert_eq!(save_btn.name, "Save Document");
}

#[tokio::test]
async fn test_perform_action_and_state_id_mutation() {
    let backend = Arc::new(SimulatedBackend::default());
    let session = CuaSessionManager::new(backend);

    let initial_state = session
        .get_app_state(Some("app_editor"), false)
        .await
        .unwrap();

    let initial_state_id = initial_state.state_id.clone();

    // Set value of text area (index 4)
    let new_text = "fn m31a_super_agent() {\n    // verified\n}".to_string();
    let action_res = session
        .perform_action(
            "app_editor",
            4,
            ElementAction::SetValue(new_text.clone()),
            &initial_state_id,
        )
        .await
        .expect("Action failed");

    assert!(action_res.success);
    assert!(action_res.action_sent);
    let new_state_id = action_res.new_state_id.expect("Expected new state id");
    assert_ne!(new_state_id, initial_state_id);

    // Fetch state again and verify the text mutated
    let updated_state = session
        .get_app_state(Some("app_editor"), false)
        .await
        .unwrap();
    let text_area = updated_state.root.find_by_index(4).unwrap();
    assert_eq!(text_area.value.as_ref().unwrap(), &new_text);
}

#[tokio::test]
async fn test_state_id_mismatch_prevents_stale_actions() {
    let backend = Arc::new(SimulatedBackend::default());
    let session = CuaSessionManager::new(backend);

    let initial_state = session
        .get_app_state(Some("app_editor"), false)
        .await
        .unwrap();

    // Advance state with an action
    let res1 = session
        .perform_action(
            "app_editor",
            5,
            ElementAction::Click,
            &initial_state.state_id,
        )
        .await
        .unwrap();

    assert!(res1.success);

    // Try to perform action with stale initial_state_id
    let err = session
        .perform_action(
            "app_editor",
            4,
            ElementAction::SetValue("bad edit".to_string()),
            &initial_state.state_id,
        )
        .await
        .unwrap_err();

    match err {
        CuaError::StateMismatch { expected, actual } => {
            assert_eq!(expected, initial_state.state_id);
            assert_eq!(actual, res1.new_state_id.unwrap());
        }
        other => panic!("Expected StateMismatch error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_anti_replay_blocks_duplicate_dispatch() {
    let backend = Arc::new(SimulatedBackend::default());
    let session = CuaSessionManager::new(backend);

    let state = session
        .get_app_state(Some("app_editor"), false)
        .await
        .unwrap();

    // Dispatch click
    let res = session
        .perform_action("app_editor", 5, ElementAction::Click, &state.state_id)
        .await
        .unwrap();
    assert!(res.success);

    // Immediately dispatch exact duplicate with same state_id
    let err = session
        .perform_action("app_editor", 5, ElementAction::Click, &state.state_id)
        .await
        .unwrap_err();

    match err {
        CuaError::ReplayBlocked(_) => {}
        other => panic!("Expected ReplayBlocked error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_send_keys_and_mouse_action() {
    let backend = Arc::new(SimulatedBackend::default());
    let session = CuaSessionManager::new(backend);

    let state = session
        .get_app_state(Some("app_editor"), false)
        .await
        .unwrap();

    let keys_res = session
        .send_keys("app_editor", "\n// appended via CUA keys", &state.state_id)
        .await
        .unwrap();
    assert!(keys_res.success);

    let mouse_res = session
        .mouse_action(
            MouseAction::Click {
                button: "left".to_string(),
            },
            500,
            300,
        )
        .await
        .unwrap();
    assert!(mouse_res.success);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_linux_native_backend_real_process_enumeration() {
    use m31a_cua::LinuxNativeBackend;

    let backend = Arc::new(LinuxNativeBackend::new());
    let session = CuaSessionManager::new(backend);

    let apps = session.list_apps().await.expect("Native list_apps failed");
    assert!(
        !apps.is_empty(),
        "Must discover at least one desktop or shell process"
    );

    let active_app = apps
        .iter()
        .find(|a| a.is_active)
        .or_else(|| apps.first())
        .unwrap();
    let state = session
        .get_app_state(Some(&active_app.id), false)
        .await
        .expect("Native get_app_state failed");

    assert!(!state.app_name.is_empty());
    assert!(!state.state_id.is_empty());
    assert_eq!(state.root.role, ElementRole::Window);
}
