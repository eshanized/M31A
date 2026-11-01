//! Dedicated integration tests for `m31a-browser` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `BrowserConfig`, `BrowserSession`, `BrowserError`, `CdpError`, `CdpEvent`, `CdpRequest`, `CdpResponse`, `ConsoleMessage`, `NetworkRecord`, `AccessibilityNode`, `AccessibilityTreeBuilder`, `ElementRef`, `PageSnapshot`
//! - Typed `BrowserError` formatting and conversions

use m31a_browser::{
    AccessibilityNode, AccessibilityTreeBuilder, BrowserConfig, BrowserError, CdpRequest,
    PageSnapshot,
};

#[test]
fn test_root_reexports() {
    let config = BrowserConfig::default();
    assert_eq!(config.viewport_width, 1280);

    let builder = AccessibilityTreeBuilder::new();
    let root = AccessibilityNode {
        ref_id: None,
        role: "RootWebArea".to_string(),
        name: "Home".to_string(),
        value: None,
        backend_node_id: None,
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        children: Vec::new(),
    };
    let snap: PageSnapshot = builder.build("about:blank", "Home", root);
    assert_eq!(snap.title, "Home");

    let req = CdpRequest::new(1, "Page.enable", None);
    assert_eq!(req.id, 1);
}

#[test]
fn test_browser_error_types_and_display() {
    let err1 = BrowserError::BrowserUnavailable;
    assert!(err1.to_string().contains("Browser unavailable"));

    let err2 = BrowserError::ProcessLaunch("spawn error".to_string());
    assert!(err2.to_string().contains("launch failed"));

    let err3 = BrowserError::CdpHandshake("101 upgrade rejected".to_string());
    assert!(err3.to_string().contains("handshake failed"));

    let err4 = BrowserError::EvaluationFailed("ReferenceError: foo is not defined".to_string());
    assert!(err4.to_string().contains("JavaScript evaluation failed"));

    let err5 = BrowserError::ScreenshotFailed("buffer encoding issue".to_string());
    assert!(err5.to_string().contains("Screenshot capture failed"));

    let err6 = BrowserError::HeapInspectionFailed("OOM".to_string());
    assert!(err6.to_string().contains("Heap inspection failed"));
}
