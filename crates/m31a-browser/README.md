# `m31a-browser`

Headless Chromium Chrome DevTools Protocol (CDP) browser automation, accessibility trees, and visual profiling engine for **M31A**.

---

## Overview

`m31a-browser` controls headless Chromium instances via CDP over asynchronous WebSockets. It powers M31A's **Visual TDD** capabilities by inspecting live web applications, rendering accessibility trees with `@e1`, `@e2` element references, asserting computed CSS styles, taking screenshots, and intercepting console/network failures.

---

## User-Facing Capabilities

- **Visual TDD Validation (`m31a vibe --visual-url <URL>` or `/visual <URL>`)**: Automatically navigates to a running web frontend, checks element visibility and text, inspects layout, and asserts zero console/network errors.
- **Visual Regression Screenshot Capture**: Captures viewport screenshots when UI tests fail, providing visual artifacts for debugging.

---

## Developer Guide & Architecture

### Key Modules

- [`cdp`](src/cdp.rs): Chromium DevTools Protocol message serialization, WebSocket client, `ConsoleMessage` loggers, and `NetworkRecord` trackers.
- [`dom_tree`](src/dom_tree.rs): `AccessibilityTreeBuilder` constructing concise accessibility trees and mapping DOM nodes to short `@e1` element references.
- [`session`](src/session.rs): `BrowserSession` managing child Chromium processes, temporary profile directories, viewport dimensions, and page navigation.

### Programmatic Usage

```rust
use m31a_browser::session::{BrowserConfig, BrowserSession};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = BrowserSession::launch(BrowserConfig::default()).await?;
    session.navigate("http://localhost:3000").await?;
    let snapshot = session.capture_accessibility_snapshot().await?;
    println!("Accessibility Elements: {}", snapshot.nodes.len());
    session.close().await?;
    Ok(())
}
```

---

## Testing

```bash
cargo test -p m31a-browser
```
