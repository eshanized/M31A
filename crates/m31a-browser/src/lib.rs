//! `m31a-browser`: Headless Chromium CDP browser automation, accessibility trees, and profiling engine.
//!
//! Provides Chromium DevTools Protocol (CDP) connectivity, accessibility tree indexing
//! with `@e1`, `@e2`, ... reference allocation, screenshot capturing, and console/network telemetry.

pub mod cdp;
pub mod dom_tree;
pub mod session;

pub use cdp::{CdpError, CdpEvent, CdpRequest, CdpResponse, ConsoleMessage, NetworkRecord};
pub use dom_tree::{AccessibilityNode, AccessibilityTreeBuilder, ElementRef, PageSnapshot};
pub use session::{BrowserConfig, BrowserSession};

/// Typed browser subsystem errors.
#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("Browser unavailable: no Chromium binary found on system")]
    BrowserUnavailable,
    #[error("Chromium process launch failed: {0}")]
    ProcessLaunch(String),
    #[error("CDP handshake failed: {0}")]
    CdpHandshake(String),
    #[error("CDP protocol error: {0}")]
    CdpProtocol(String),
    #[error("Browser transport unavailable")]
    TransportUnavailable,
    #[error("JavaScript evaluation failed: {0}")]
    EvaluationFailed(String),
    #[error("Screenshot capture failed: {0}")]
    ScreenshotFailed(String),
    #[error("Heap inspection failed: {0}")]
    HeapInspectionFailed(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
