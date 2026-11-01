//! Live integration tests for TUI Application Model Activation targeting NVIDIA Nemotron 3 Ultra.
//!
//! Gated by:
//! - Cargo feature: `live-nvidia-tests`
//! - Environment variable: `M31A_RUN_LIVE_NVIDIA_TESTS=1`

use m31a_core::Config;
use m31a_core::types::ChatMessage;
use m31a_provider::live_test_support::{
    NVIDIA_NEMOTRON_3_ULTRA, NVIDIA_NIM_BASE_URL, load_live_test_api_key,
};
use m31a_tui::app::App;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Live NVIDIA NIM integration test requiring M31A_RUN_LIVE_NVIDIA_TESTS=1"]
async fn test_real_live_application_model_activation_and_inference() {
    let api_key = load_live_test_api_key();
    let temp = tempdir().expect("tempdir");

    // Initialize TUI App with working meta model
    let initial_model = "meta/llama-3.3-70b-instruct";
    let config = Config {
        model: initial_model.to_string(),
        api_base: NVIDIA_NIM_BASE_URL.to_string(),
        api_key: Some(api_key.clone()),
        reasoning_effort: Some("medium".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");
    assert_eq!(app.active_model, initial_model);

    // 1. Transactionally switch to Nemotron 3 Ultra using real provider configuration
    let start = std::time::Instant::now();
    let res = app.switch_model(NVIDIA_NEMOTRON_3_ULTRA);
    assert!(
        res.is_ok(),
        "Model activation for Nemotron Ultra must succeed: {:?}",
        res.err()
    );
    println!(
        "Model activation succeeded in {} ms",
        start.elapsed().as_millis()
    );

    // 2. Verify all UI and runtime states committed
    assert_eq!(app.active_model, NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.bottom_pane.active_model, NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.nim_client.model(), NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.config.model, NVIDIA_NEMOTRON_3_ULTRA);

    // Configure explicit, bounded smoke-test request profile on the active client
    app.nim_client = app
        .nim_client
        .clone()
        .with_max_tokens(Some(512))
        .with_reasoning_effort(Some("medium".to_string()))
        .with_temperature(Some(1.0));

    // 3. Crucial: Verify that activation leads to real inference through the active client
    let start_inference = std::time::Instant::now();
    let (response, usage) = app
        .nim_client
        .stream_chat(
            &[ChatMessage::user("Say the word 'ACTIVATED' only.")],
            None,
            None,
            None,
        )
        .await
        .expect("Real inference through newly activated client must succeed");

    println!(
        "Live inference through active client completed in {} ms",
        start_inference.elapsed().as_millis()
    );

    // Verify user-facing final content ONLY (do not concatenate reasoning)
    let content = response.content.as_deref().unwrap_or("");
    assert!(
        content.to_uppercase().contains("ACTIVATED"),
        "Expected user-facing content containing 'ACTIVATED', got: {content}"
    );
    assert!(usage.total_tokens > 0 || usage.prompt_tokens > 0);

    // 4. Verify failure boundary: switching to an unknown model fails and preserves active model
    let bad_switch = app.switch_model("unregistered-org/nonexistent-model-xyz");
    assert!(bad_switch.is_err(), "Unknown model switch must fail");
    assert_eq!(app.active_model, NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.nim_client.model(), NVIDIA_NEMOTRON_3_ULTRA);

    // 5. Verify failure boundary: healthy endpoint with unavailable model fails before activation
    let unavailable_target = "nvidia/nonexistent-nemotron-model-999b";
    let unavail_switch = app.switch_model(unavailable_target);
    assert!(
        unavail_switch.is_err(),
        "Switching to unavailable model on healthy endpoint must fail"
    );
    assert_eq!(app.active_model, NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.nim_client.model(), NVIDIA_NEMOTRON_3_ULTRA);

    // 6. Verify failure boundary: switching without credentials preserves active runtime
    app.config.api_key = None;
    let unauth_switch = app.switch_model("meta/llama-3.3-70b-instruct");
    assert!(unauth_switch.is_err(), "Unauthenticated switch must fail");
    assert_eq!(app.active_model, NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.nim_client.model(), NVIDIA_NEMOTRON_3_ULTRA);

    // 7. Verify failure boundary: dead endpoint preserves active model
    app.config.api_key = Some(api_key);
    app.config.api_base = "http://127.0.0.1:1".to_string();
    let dead_switch = app.switch_model("meta/llama-3.3-70b-instruct");
    assert!(dead_switch.is_err(), "Dead endpoint probe must fail");
    assert_eq!(app.active_model, NVIDIA_NEMOTRON_3_ULTRA);
    assert_eq!(app.nim_client.model(), NVIDIA_NEMOTRON_3_ULTRA);
}
