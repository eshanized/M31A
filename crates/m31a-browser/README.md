# `m31a-browser`

> Headless Chromium Chrome DevTools Protocol (CDP) automation, accessibility DOM snapshotting, element interaction, and screenshot capture for M31A.

---

## Why Does This Crate Exist?

Modern autonomous coding assistants cannot restrict themselves to backend logic; they must build and verify rich web user interfaces. When an agent writes React, Vue, Svelte, or HTML/CSS, how does it know whether the component actually rendered properly or if a runtime JavaScript error broke the page?

`m31a-browser` provides programmatic browser automation:
- Automatically discovers and launches headless Chromium or Google Chrome instances.
- Communicates directly over WebSocket using the Chrome DevTools Protocol (CDP) without requiring heavyweight external dependencies like Selenium.
- Parses the browser DOM into a token-efficient accessibility tree (`PageSnapshot`) with unique element references (`@e1`, `@e2`).
- Simulates human user interactions: clicking, typing, scrolling, and navigating.
- Captures full-page and element screenshot artifacts.
- Enforces strict SSRF protections: blocks private internal IP addresses and unapproved domains.

---

## Core Concepts & How It Works

### 1. Accessibility DOM Tree (`dom_tree.rs`)
Dumping raw HTML into an LLM context window wastes thousands of tokens on script tags, SVGs, and styling boilerplate.  
`AccessibilityTreeBuilder` extracts the semantic accessibility tree:
- Form elements, buttons, links, inputs, and headings.
- Assigns deterministic reference IDs (e.g., `button "@e4" [Submit Order]`).
- The agent can simply execute `click("@e4")` or `type_text("@e2", "admin")`.

### 2. Chrome DevTools Protocol (CDP) Streaming (`cdp.rs`)
Establishes a low-latency WebSocket connection to Chromium:
- Listens to `Runtime.consoleAPICalled` to capture `console.error` and exceptions.
- Listens to `Network.responseReceived` to detect failed HTTP 4xx/5xx requests.
- Invokes `Page.captureScreenshot` to synthesize PNG artifacts.

### 3. Hardened Security & SSRF Defense (`session.rs`)
To prevent agents from weaponizing the browser against internal networks:
- Restricts navigation to `domain_allowlist` and configured development server ports (e.g. `localhost:3000`, `127.0.0.1:8080`).
- Enforces IP blocklists against private subnets (`169.254.169.254`, `10.0.0.0/8`, etc.).

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/session.rs`](src/session.rs) | `BrowserSession`, `BrowserConfig` | Manages Chromium process lifecycles and page interactions. |
| [`src/cdp.rs`](src/cdp.rs) | `CdpClient`, `ConsoleMessage`, `NetworkRecord` | Low-level WebSocket CDP framing and event dispatcher. |
| [`src/dom_tree.rs`](src/dom_tree.rs) | `PageSnapshot`, `AccessibilityNode` | Semantic accessibility tree extraction and element indexing. |

---

## Practical Usage Example

```rust
use m31a_browser::session::{BrowserConfig, BrowserSession};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = BrowserConfig {
        headless: true,
        viewport_width: 1280,
        viewport_height: 800,
        ..Default::default()
    };

    // 1. Launch or attach to headless Chromium
    let session = BrowserSession::launch(config).await?;

    // 2. Navigate to local development server
    session.navigate("http://localhost:3000").await?;

    // 3. Extract semantic page snapshot
    let snapshot = session.capture_accessibility_tree().await?;
    println!("Title: {}", snapshot.title);
    println!("Interactive elements found: {}", snapshot.nodes.len());

    // 4. Capture screenshot
    let screenshot_png = session.capture_screenshot().await?;
    std::fs::write("screenshot.png", screenshot_png)?;
    println!("Saved screenshot.png");

    session.close().await?;
    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-browser
```
