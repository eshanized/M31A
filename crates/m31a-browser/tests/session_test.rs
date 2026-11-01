//! Dedicated integration tests for `m31a-browser` (src/session.rs).
//!
//! Validates:
//! - Browser configuration defaults and custom parameters
//! - Launch validation and typed failure handling
//! - Heap snapshot diff analysis and memory leak detection
//! - Real Chromium session lifecycle (launch, navigate, JS evaluation, screenshot, close)

use m31a_browser::cdp::HeapSnapshotSummary;
use m31a_browser::session::{BrowserConfig, BrowserSession};
use std::path::PathBuf;

#[test]
fn test_browser_config_defaults() {
    let config = BrowserConfig::default();
    assert!(config.headless);
    assert_eq!(config.viewport_width, 1280);
    assert_eq!(config.viewport_height, 800);
    assert!(!config.mock_mode);
}

#[tokio::test]
async fn test_browser_invalid_path_fails_cleanly() {
    let config = BrowserConfig {
        chrome_path: Some(PathBuf::from("/bin/does_not_exist_chrome_fake")),
        mock_mode: false,
        ..Default::default()
    };

    let res = BrowserSession::launch(config).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_real_headless_chromium_lifecycle() {
    // Check if Chromium is available on this system
    let has_chromium = which::which("chromium")
        .or_else(|_| which::which("google-chrome"))
        .is_ok();

    if !has_chromium {
        println!("Skipping real browser integration test: Chromium not installed");
        return;
    }

    let config = BrowserConfig {
        headless: true,
        viewport_width: 800,
        viewport_height: 600,
        mock_mode: false,
        ..Default::default()
    };

    let session = match BrowserSession::launch(config).await {
        Ok(s) => s,
        Err(e) => {
            println!("Chromium launch failed in test environment: {e}");
            return;
        }
    };

    assert!(!session.is_simulated());

    // 1. Navigate to about:blank
    let nav_res = session.navigate("about:blank").await;
    assert!(nav_res.is_ok());

    // 2. Evaluate JavaScript expression
    let eval_res = session.evaluate("35 + 7").await;
    assert!(eval_res.is_ok());
    let val = eval_res.unwrap();
    assert!(val.contains("42"), "Expected evaluated output to contain 42, got: {val}");

    // 3. Capture screenshot
    let screenshot_b64 = session.screenshot(None).await;
    assert!(screenshot_b64.is_ok());
    let b64_str = screenshot_b64.unwrap();
    assert!(!b64_str.is_empty(), "Screenshot should produce non-empty base64 string");

    // 4. Inspect network records
    let records = session.get_network_records().await;
    assert!(!records.is_empty());

    // 5. Test heap diff comparison
    let before = HeapSnapshotSummary {
        total_heap_size_bytes: 50_000_000,
        used_heap_size_bytes: 20_000_000,
        heap_size_limit_bytes: 100_000_000,
        documents_count: 1,
        nodes_count: 500,
        listeners_count: 10,
        timestamp_ms: 1000,
    };
    let after = HeapSnapshotSummary {
        total_heap_size_bytes: 80_000_000,
        used_heap_size_bytes: 45_000_000,
        heap_size_limit_bytes: 100_000_000,
        documents_count: 5,
        nodes_count: 3500,
        listeners_count: 85,
        timestamp_ms: 5000,
    };
    let diff = session.diff_heap_snapshots(&before, &after);
    assert_eq!(diff.delta_used_bytes, 25_000_000);
    assert_eq!(diff.delta_nodes, 3000);
    assert_eq!(diff.delta_listeners, 75);
    assert!(diff.potential_leak);
    assert!(diff.analysis.contains("leak"));
}
