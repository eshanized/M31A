# `m31a-term`

> Terminal raw mode handling, ANSI/OSC escape sequence processing, OSC 8 hyperlinks, and terminal-native Mermaid diagram rendering (`merman`) for M31A.

---

## Why Does This Crate Exist?

Modern terminals are capable of rich visuals: truecolor 24-bit color palettes, clickable hyperlinks (OSC 8), Unicode box-drawing art, and alternate screen buffers. However, terminal capabilities vary wildly across environments (Alacritty, iTerm2, WezTerm, Kitty, GNOME Terminal, Windows Terminal, and headless CI runners).

`m31a-term` provides low-level terminal rendering and compatibility infrastructure:
- **Terminal Capability Detection (`detection.rs`)**: Detects truecolor support, kitty graphics protocols, bracketed paste, and OSC 8 hyperlink compatibility.
- **Terminal Mermaid Engine (`merman.rs`)**: Parses and renders Mermaid flowcharts and sequence diagrams directly into ASCII/Unicode box-drawing art inside the terminal without headless browser dependencies!
- **Clickable Hyperlinks (`hyperlinks.rs`)**: Formats file paths and URLs into standard OSC 8 clickable terminal links (`\x1b]8;;url\x1b\\text\x1b]8;;\x1b\\`).
- **ANSI Stripping (`strip.rs`)**: High-speed zero-allocation removal of ANSI color codes when piping output to plain text or log files.

---

## Core Concepts & How It Works

### 1. Terminal Mermaid Engine (`merman.rs`)
When an agent outputs a ```mermaid diagram, rather than showing raw syntax or failing, `m31a-term` computes a 2D topological graph layout and renders it using Unicode box characters (`┌`, `─`, `┐`, `│`, `└`, `┘`, `▲`, `▼`, `►`).

### 2. OSC 8 Hyperlink Injection (`hyperlinks.rs`)
Wraps file references (`src/main.rs:42`) into clickable terminal hyperlinks so developers can click directly in their terminal to jump to that line in their IDE.

### 3. Terminal Capability Profiling (`detection.rs`)
Inspects `$TERM`, `$COLORTERM`, `$TERM_PROGRAM`, and sends device query escape sequences to dynamically adjust rendering fidelity.

---

## Key Modules & Types

| Module | Key Structs / Functions | Purpose |
| :--- | :--- | :--- |
| [`src/merman.rs`](src/merman.rs) | `MermanEngine`, `DiagramKind`, `FlowNode`, `FlowEdge` | Renders Mermaid flowcharts to Unicode terminal art. |
| [`src/hyperlinks.rs`](src/hyperlinks.rs) | `format_osc8_link`, `file_hyperlink` | Generates clickable OSC 8 terminal hyperlink escape sequences. |
| [`src/detection.rs`](src/detection.rs) | `TerminalCapabilities`, `detect_capabilities` | Profiles truecolor, unicode width, and hyperlink support. |
| [`src/strip.rs`](src/strip.rs) | `strip_ansi`, `visible_width` | Strips ANSI escapes and calculates grapheme visual column widths. |

---

## Practical Usage Example

```rust
use m31a_term::hyperlinks::format_osc8_link;
use m31a_term::merman::MermanEngine;

fn main() -> anyhow::Result<()> {
    // 1. Generate an OSC 8 clickable terminal hyperlink
    let link = format_osc8_link("file:///home/developer/src/main.rs#L42", "src/main.rs:42");
    println!("Clickable link: {}", link);

    // 2. Render a Mermaid flowchart directly to terminal Unicode art
    let mermaid_code = r#"
graph TD
    A[Start] --> B{Build Passed?}
    B -->|Yes| C[Deploy]
    B -->|No| D[Auto-Heal]
"#;

    if let Ok(diagram) = MermanEngine::render(mermaid_code) {
        println!("Rendered Diagram:\n{}", diagram);
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-term
```
