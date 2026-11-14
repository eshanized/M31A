# M31A — TUI Transcript Rendering Architecture Specification

**Status:** IMPLEMENTED, VERIFIED & RELEASE-READY  
**Target:** `crates/m31a-tui/src/transcript_view.rs`, `crates/m31a-tui/src/renderer.rs`, `crates/m31a-tui/src/mouse.rs`  
**Classification:** TUI Systems Architecture & Rendering Specification  
**Audit Invariant:** $\text{measure\_cell\_rendered\_height}(cell, W) \equiv \text{render\_cell\_visual\_rows\_static}(cell, W).\text{len}()$  

---

## 1. Architectural Overview & Design Principles

In terminal UI virtualizers, rendering accuracy requires that the measurement algorithm calculating viewport dimensions matches the projection algorithm rendering terminal rows.

M31A implements a unified visual-row pipeline that guarantees:
1. **Accurate Line Wrapping:** Paragraph wrapping and terminal boundary constraints are evaluated identically in measurement and rendering.
2. **Unified Virtualization:** Multi-byte UTF-8, ANSI escape sequences, CJK fullwidth characters, emoji sequences, and long tokens (file paths, git hashes, UUIDs) are calculated with Unicode-standard column widths.
3. **Smooth Viewport Virtualization:**
   - Assistant responses, tool cards, and reasoning blocks fit viewports without hidden trailing rows.
   - Elimination of phantom blank rows inside reasoning bubbles.
   - Stable auto-scrolling during continuous streaming token generation.
   - Precise mouse hit-testing for copying, expanding, and folding blocks.

---

## 2. Unified Visual Row Pipeline

The core architectural invariant is **canonical visual-row symmetry**:

$$\forall \text{ cell } c \in \text{TranscriptCell}, \forall W \in \mathbb{N}^+: \quad \text{measure\_cell\_rendered\_height}(c, W) = |\text{render\_cell\_visual\_rows\_static}(c, W)|$$

The single visual-row function is the sole authoritative source of truth:

```text
                 TranscriptCell
                       │
                       ▼
       render_cell_visual_rows_static(cell, width)
                       │
         ┌─────────────┴─────────────┐
         ▼                           ▼
    Measurement                  Rendering
         │                           │
         ▼                           ▼
 Visual row count            Actual terminal rows
         │                           │
         └─────────────┬─────────────┘
                       ▼
       Zero Coordinate Mismatch Guarantee
```

This single projection governs:
- Cell height measurement
- Total transcript height (`TranscriptVirtualizer::total_line_count`)
- Cache entry sizing
- Visible-window computation (`calculate_visible_window`)
- Scroll offset clamping and auto-scroll bottom detection
- Virtualized rendering buffer population
- Text copying and search indexing (`get_line_as_string`)
- Mouse hit-testing and toggle interaction (`find_cell_and_local_line`, `toggle_cell_at_line`)

---

## 3. Terminal Wrapping & Unicode Specifications

Implemented in `crates/m31a-tui/src/transcript_view.rs`:

```rust
pub fn wrap_line_to_visual_rows(line: &str, max_width: usize) -> Vec<String>
```

### 3.1 Unbreakable Tokens & Grapheme Clustering
When a word or token (e.g. SHA-256 hash `d2a84f...`, deep file paths, base64 blobs) exceeds `max_width`:
- The tokenizer does not truncate or cause line overflow.
- Graphemes are sliced cluster-by-cluster according to `unicode_segmentation` and `unicode_width::UnicodeWidthChar`.
- Grapheme clusters are never severed midway, preventing terminal corruption.

### 3.2 CJK Fullwidth Characters
- Standard Asian ideographs (`测`, `試`, `한`, `日`) measure display width 2 via `unicode-width`.
- Line budget arithmetic counts display cells, not byte length or `char` count.
- Prevents premature line breaks or overflow past the right border.

### 3.3 Style Preservation Across Row Splits
When Ratatui spans spanning multiple words are broken across terminal visual rows:
- The styling (foreground, background, modifiers) is propagated to each resulting visual row fragment.
- Explicit newline boundaries (`\n`) are preserved.

---

## 4. Transcript Virtualizer Cache Lifecycle

`TranscriptVirtualizer` manages a dimension-aware cache:
1. **Width Sensitivity:** Every cached measurement is tied to terminal `width`. Any resize invalidates the cache entries immediately.
2. **Expansion / Collapse Invalidation:** Toggling tool call outputs, reasoning chains, or exploration groups invalidates only the affected cells (`invalidate_cell(index)`), recalculating visual row count on demand without rebuilding the entire transcript.
3. **Streaming Token Invalidation:** When streaming tokens arrive, only the active trailing cell's cache entry is invalidated, ensuring steady 60fps rendering without lag.
4. **Scroll Clamping:** `chat_widget.scroll_controller.clamp_offset()` dynamically constrains the scroll offset to $[0, \text{total\_lines} - \text{viewport\_height}]$.

---

## 5. Verification & Test Evidence

Verified against Ratatui's real `TestBackend` in both unit and integration tests:

1. **`test_transcript_wrapping_and_measurement_across_varying_widths`** (`tests/tui_regression_test.rs`):
   - Executed across terminal widths 40, 60, 80, 100, and 120.
   - For every width, `measure_cell_rendered_height` matches `render_cell_visual_rows_static.len()` exactly with delta = 0.
2. **`test_unbreakable_tokens_wrap_predictably`** (`tests/tui_regression_test.rs`):
   - Evaluated 120-char continuous hash on an 80-column terminal.
   - Wrapped cleanly into exactly 2 visual rows without panics or clipping.
3. **`test_multibyte_cjk_width_handling`** (`tests/tui_regression_test.rs`):
   - 10 double-width CJK characters verified to occupy 20 display columns, wrapping into 2 rows on a 15-width container without buffer overflow.
4. **`canonical_visual_rows_match_rendered_buffer_at_every_width`** (`src/transcript_view.rs`):
   - Compares production renderer output cell-by-cell against the canonical projection using Ratatui `TestBackend`.
5. **PTY End-to-End Suite** (`tests/pty_end_to_end_test.rs`):
   - 9 full terminal tests executed through `libc::openpty` without glitches, flickering, or offset drifting.
