//! Integration tests for Transactional Model Activation and Session Resume (`model_activation_transaction_test.rs`).
//!
//! Verifies two-phase commit, TCP connectivity verification using `LocalWireServer`,
//! failure-safety boundaries, and active-turn protection.

use m31a_core::Config;
use m31a_core::types::SessionMetadata;
use m31a_provider::wire_server::LocalWireServer;
use m31a_tui::app::App;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_successful_model_activation_over_tcp() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let config = Config {
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");
    assert_eq!(app.active_model, "nvidia/nemotron-3.5-lightning-30b-a3b");

    // Transactionally switch to another registered model
    let target = "meta/llama-3.3-70b-instruct";
    let res = app.switch_model(target);
    assert!(
        res.is_ok(),
        "Switching to valid model must succeed over TCP"
    );

    // All active state must be atomically committed
    assert_eq!(app.active_model, target);
    assert_eq!(app.bottom_pane.active_model, target);
    assert_eq!(app.nim_client.model(), target);
    assert_eq!(app.config.model, target);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unknown_model_rejection_no_client_constructed() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let initial_model = "nvidia/nemotron-3.5-lightning-30b-a3b";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");

    // Attempting to switch to an unknown model without explicit provider must fail
    let res = app.switch_model("unregistered-org/phantom-model-xyz");
    assert!(res.is_err(), "Unknown model must fail resolution");

    // State must remain strictly unchanged
    assert_eq!(app.active_model, initial_model);
    assert_eq!(app.bottom_pane.active_model, initial_model);
    assert_eq!(app.nim_client.model(), initial_model);

    // Available models must not be polluted with fabricated metadata
    assert!(
        !app.available_models
            .iter()
            .any(|m| m.slug == "unregistered-org/phantom-model-xyz")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unauthenticated_model_rejection_preserves_runtime() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let initial_model = "nvidia/nemotron-3.5-lightning-30b-a3b";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");

    // Simulate missing credential for subsequent switch
    app.config.api_key = None;

    let res = app.switch_model("meta/llama-3.3-70b-instruct");
    assert!(res.is_err(), "Switching without key must fail validation");
    let err_str = res.err().unwrap().to_string();
    assert!(
        err_str.contains("API key") || err_str.contains("Missing API key"),
        "Error was: {err_str}"
    );

    // Runtime state must remain intact
    assert_eq!(app.active_model, initial_model);
    assert_eq!(app.bottom_pane.active_model, initial_model);
    assert_eq!(app.nim_client.model(), initial_model);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_connectivity_failure_preserves_previously_active_model() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let initial_model = "nvidia/nemotron-3.5-lightning-30b-a3b";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");

    // Point endpoint to dead port
    app.config.api_base = "http://127.0.0.1:1".to_string();

    let res = app.switch_model("meta/llama-3.3-70b-instruct");
    assert!(res.is_err(), "Dead endpoint must fail connectivity probe");

    // Previous active model and client must be preserved
    assert_eq!(app.active_model, initial_model);
    assert_eq!(app.bottom_pane.active_model, initial_model);
    assert_eq!(app.nim_client.model(), initial_model);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_active_turn_rejection_prevents_model_switching() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let initial_model = "nvidia/nemotron-3.5-lightning-30b-a3b";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");

    // Simulate an active turn in progress
    let turn_id = "test-turn-123";
    app.turn_state
        .transition_submitted(turn_id)
        .expect("start turn");
    app.current_turn_id = Some(turn_id.to_string());

    // Switch model must be immediately rejected
    let res = app.switch_model("meta/llama-3.3-70b-instruct");
    assert!(res.is_err(), "Must reject switch during active turn");
    assert!(
        res.err()
            .unwrap()
            .to_string()
            .contains("Cannot switch model while a turn or agent execution is active")
    );

    // State preserved
    assert_eq!(app.active_model, initial_model);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_thread_resume_failure_handling() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let initial_model = "nvidia/nemotron-3.5-lightning-30b-a3b";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");
    let original_session_id = app.session_id.clone();

    // Create a thread in storage with an invalid/unsupported model
    let bad_meta = SessionMetadata {
        id: "broken-session-123".to_string(),
        title: "Broken Session".to_string(),
        model: "completely-unknown/bad-model".to_string(),
        cwd: "/tmp".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        extra: std::collections::HashMap::new(),
    };
    app.storage
        .save_thread(&bad_meta, "/tmp/rollout.jsonl", "WorkspaceWrite", "Default")
        .expect("save bad thread");

    // Resume session must fail validation and preserve existing session state
    let res = app.resume_session(&bad_meta.id);
    assert!(res.is_err(), "Resuming invalid model must fail");

    assert_eq!(app.session_id, original_session_id);
    assert_eq!(app.active_model, initial_model);
    assert_eq!(app.bottom_pane.active_model, initial_model);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_healthy_endpoint_with_unavailable_model_fails_before_activation() {
    let wire_server = LocalWireServer::start().await.expect("wire server start");
    let temp = tempdir().unwrap();

    let initial_model = "meta/llama-3.3-70b-instruct";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");
    assert_eq!(app.active_model, initial_model);

    // "nvidia/nonexistent-model" is a valid NIM slug format that resolves to NIM provider,
    // and the endpoint is healthy, but the model is absent from the provider's /models catalog.
    let unavailable_target = "nvidia/nonexistent-model";
    let res = app.switch_model(unavailable_target);
    assert!(
        res.is_err(),
        "Switching to unavailable model on a healthy endpoint must fail before activation"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("not available"),
        "Error must clearly indicate model unavailability: {err_msg}"
    );

    // Active state must be completely preserved
    assert_eq!(app.active_model, initial_model);
    assert_eq!(app.bottom_pane.active_model, initial_model);
    assert_eq!(app.nim_client.model(), initial_model);
    assert_eq!(app.config.model, initial_model);
}
