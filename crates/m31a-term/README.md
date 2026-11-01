# `m31a-term`

Terminal capability detection, OSC 8 hyperlinks, LaTeX math rendering, Mermaid diagram helpers, and patchmark tail healing for **M31A**.

---

## Overview

`m31a-term` contains terminal rendering utilities that enhance the developer experience. It formats clickable file links using OSC 8 escape sequences, renders inline LaTeX math expressions using Unicode subscripts/superscripts, renders ASCII Mermaid diagrams, and heals truncated markdown blocks during streaming.

---

## User-Facing Capabilities

- **Clickable File & Symbol Hyperlinks**: Generates clickable OSC 8 links in supported terminals (`file:///...`), allowing users to click file paths or symbol names to open them directly in their editor.
- **LaTeX Math Rendering**: Renders mathematical formulas and subscripts directly in ASCII/Unicode terminal buffers.
- **Tail Healing (`heal_streaming_markdown`)**: Prevents malformed markdown tags or unclosed code fences from breaking terminal formatting during mid-stream token arrival.

---

## Developer Guide & Architecture

### Key Modules

- [`hyperlinks`](src/hyperlinks.rs): `format_clickable_path`, `format_file_hyperlink`, and `strip_hyperlinks`.
- [`detection`](src/detection.rs): `TerminalCapabilities` detecting 24-bit TrueColor, Unicode support, and terminal emulators.
- [`patchmark`](src/patchmark.rs): `heal_streaming_markdown` guaranteeing balanced code fences during live token arrival.
- [`latex`](src/latex.rs): Unicode subscript/superscript translation and 2D ASCII math rendering.

---

## Testing

```bash
cargo test -p m31a-term
```
