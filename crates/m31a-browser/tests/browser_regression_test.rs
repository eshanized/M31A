use m31a_browser::{BrowserConfig, BrowserSession};
use std::path::PathBuf;

#[tokio::test]
async fn test_browser_launch_failure_produces_typed_error() {
    // Non-existent Chrome path must fail honestly with typed error, never simulated browser
    let config = BrowserConfig {
        chrome_path: Some(PathBuf::from(
            "/nonexistent/bin/chrome_that_does_not_exist_at_all",
        )),
        mock_mode: false,
        ..Default::default()
    };

    let res = BrowserSession::launch(config).await;
    assert!(
        res.is_err(),
        "Launch with invalid binary must return an error"
    );
    let err = res.err().unwrap();
    assert!(
        err.to_string().contains("Chromium")
            || err.to_string().contains("launch")
            || err.to_string().contains("No such file"),
        "Error must describe launch failure honestly, got: {err}"
    );
}
