# M31A — TUI Model Selector Architecture Specification

**Status:** IMPLEMENTED, VERIFIED & RELEASE-READY  
**Target:** `crates/m31a-tui/src/model_selector.rs`, `crates/m31a-tui/src/bottom_pane.rs`, `crates/m31a-tui/src/modals.rs`, `crates/m31a-tui/src/screens.rs`, `crates/m31a-tui/src/app.rs`  
**Classification:** TUI Systems Architecture & Interaction Specification  
**Viewport Invariant:** $1 \text{ Selectable Model Entry} \equiv 1 \text{ Terminal Row}$; $\text{Visible Entries} \le \text{inner\_height}$  

---

## 1. Architectural Overview & Design Principles

The M31A Terminal User Interface provides a unified, stateful model selection system across three cohesive interaction surfaces:
1. **BottomPane Popup (`BottomPaneMode::ModelSelectorPopup`):** Floating inline autocomplete popup in the prompt composer.
2. **Modal Picker (`ModalState::ModelPicker`):** Centered floating dialog launched via `/model` or `Ctrl+M` / `F2`.
3. **Dedicated Models Screen (`Screen::Models`):** Full-screen catalog with detailed inspector reached via `F3` or Tab cycling.

### Core Viewport & Interaction Invariants
- **Strict Row Mapping:** Exactly 1 selectable model entry maps to 1 terminal row; visible entries never exceed container inner height.
- **Persistent State:** Search queries, selected indices, and scroll offsets persist cleanly across modal open/close cycles via `ModelListState`.
- **Direct Coordinate Mapping:** Mouse click hit-testing directly maps terminal row coordinates to model list items with zero vertical offset divergence.
- **Rich Keyboard Navigation:** Full support for `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`, and real-time substring filtering.
- **Split-View Catalog Screen:** Two-pane layout with model list on the left and comprehensive capability inspector (context length, cost, provider, tags) on the right.

---

## 2. Canonical Stateful Component: `ModelListState`

All model interfaces now exclusively use `ModelListState` from `crates/m31a-tui/src/model_selector.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ModelListState {
    pub models: Vec<ModelInfo>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub search_query: String,
    pub active_model: String,
}
```

### 2.1 Viewport Clamping & Navigation Math
- **`set_viewport_height(inner_height)`:** Dynamically sets the available rows and clamps `scroll_offset`.
- **`ensure_selected_visible()`:** Enforces that `selected_index` is always within $[\text{scroll\_offset}, \text{scroll\_offset} + \text{viewport\_height} - 1]$.
- **`select_next()` / `select_prev()`:** Circular/bounded navigation with automatic viewport adjustment.
- **`select_page_down()` / `select_page_up()`:** Shifts selection by `viewport_height` rows.
- **`select_home()` / `select_end()`:** Jumps directly to top (`offset=0`) or bottom (`offset=total-viewport`).
- **`set_search_query(q)`:** Real-time substring filter across `slug`, `display_name`, and `description`.

---

## 3. 1-Row Compact Representation & Ratatui Scrollbar

`render_model_list` enforces the 1 terminal row per model invariant:

```text
┌─ Select Model [1/85] ──────────────────────────────────────────┐
│ ▸ ● Llama 3.3 70B (meta/llama-3.3-70b-instruct) [128k] [T]    ▲│
│   ○ DeepSeek R1 (deepseek-ai/deepseek-r1) [64k] [R] [T]       █│
│   ○ Mistral Large (mistralai/mistral-large-2407) [128k] [T]   ││
│   ○ Nemotron 70B (nvidia/llama-3.1-nemotron-70b) [128k] [T]   ▼│
└────────────────────────────────────────────────────────────────┘
```

Each entry is rendered as a single Ratatui `Line`:
- Selection indicator (`▸ ` or `  `)
- Active model indicator (`● ` or `  `)
- Display name (bold on selection)
- Slug in muted parentheses
- Context window badge (`[128k]`)
- Capability badges:
  - `[R]` Reasoning (yellow)
  - `[T]` Function Tools (green)
  - `[V]` Vision (cyan)

### 3.1 Real Ratatui Vertical Scrollbar
A genuine `ratatui::widgets::Scrollbar` with `ScrollbarOrientation::VerticalRight`, arrows (`▲`, `▼`), track (`│`), and thumb (`█`) is rendered along the inner right margin whenever `total_filtered > inner_height`.

---

## 4. The Three Unified Touchpoints

```text
                         ┌───────────────────────┐
                         │   Model Catalog       │
                         │   (85 NIM Models)     │
                         └───────────┬───────────┘
                                     │
      ┌──────────────────────────────┼──────────────────────────────┐
      ▼                              ▼                              ▼
┌──────────────┐              ┌──────────────┐              ┌──────────────┐
│  BottomPane  │              │    Modal     │              │    Models    │
│    Popup     │              │    Picker    │              │  Two-Pane    │
└──────┬───────┘              └──────┬───────┘              └──────┬───────┘
       │                             │                             │
       │ (1-row items)               │ (Persistent ModalState)     │ (Catalogue Left +
       │ (ModelListState)            │ (Full keyboard & search)    │  Detail Inspector Right)
       │                             │                             │
       └─────────────────────────────┼─────────────────────────────┘
                                     │
                                     ▼
                        ┌──────────────────────────┐
                        │   app.switch_model(&slug)│
                        │  (Transactional Result)  │
                        └──────────────────────────┘
```

### 4.1 BottomPane Model Popup
Anchored directly above the composer with `popup_height = 12.min(...)`. Delegates scrolling, keyboard navigation, and mouse clicks to `ModelListState`.

### 4.2 Modal Model Picker
- Stored as `ModalState::ModelPicker { state: ModelListState }`.
- Persists user search query and selection across frames.
- Full keyboard support: `Up`/`Down`, `PgUp`/`PgDn`, `Home`/`End`, typing to filter, `Backspace` to un-filter, `Esc` to close, `Enter` to activate.
- Exact mouse hit-testing via `ModelSelectorGeometry`:
  ```rust
  let geom = ModelSelectorGeometry::from_area(picker_area);
  if let Some(visible_idx) = geom.row_to_visible_index(Position::new(column, row)) {
      let absolute_idx = state.scroll_offset + visible_idx;
      // ...
  }
  ```
  Outer border clicks, headers, and scrollbar tracks are cleanly excluded.

### 4.3 Dedicated Models Screen (Two-Pane Architecture)
In `crates/m31a-tui/src/screens.rs`:
- **Left Pane (55%):** Full catalogue list with live filter bar (`/` key), Ratatui scrollbar, active indicators, and capability badges.
- **Right Pane (45%):** Detail Inspector (`render_model_details`):
  - **Model Specification:** Model Name, Slug, Originator/Publisher, Context Window & Max Output Tokens (explicitly showing `Unknown` if unspecified, never fabricated defaults), Default Temperature & Sampling, Tool Calling Mode, Modality Support, Description (wrapped).
  - **Live Health & Connectivity:** Distinct runtime section displaying probe status (`Probed` with actual latency ms, `Not Probed`, or `Unavailable`). Static catalog metadata is strictly decoupled from live network health.
  - Interactive Actions Bar (`[Enter] Activate  |  [/] Filter  |  [Esc] Chat`)

---

## 5. Transactional Model Activation Contract (Two-Phase Commit)

`App::switch_model(&mut self, slug: &str) -> Result<String>` guarantees strict transactional integrity:
1. **Turn In-Flight Check:** If an agent turn or background execution is active (`is_running == true`), model switching is immediately rejected to prevent mid-turn runtime divergence.
2. **Pure Resolution:** Resolves model identity via `ProviderType::resolve` without mutating `available_models` or `active_model`. Unknown models or ambiguous endpoints fail with typed errors. No metadata fabrication.
3. **Configuration & Credential Validation:** Invokes `validate_provider_config` across all supported providers. Rejects missing or whitespace-only keys for remote providers.
4. **Candidate Client Construction:** Builds candidate `ProviderClient` in complete isolation.
5. **Bounded Connectivity Probe:** Executes a 5-second timeout TCP connectivity check via a dedicated background thread (`check_connection()`). If unreachable or unauthorized, the operation aborts.
6. **Replacement Runtime Preparation:** Instantiates `EngineAgentRuntime` and `GuardianReviewer` bound to the candidate client before touching application state.
7. **Atomic Disk Persistence:** Serializes candidate configuration to a unique temporary file (`.config.toml.tmp.<uuid>`), sets `0600` permissions, fsyncs the file, atomically renames to `config.toml`, and fsyncs parent directory.
8. **Single-Commit State Mutation:** Atomically commits replacement runtime, updating `active_model`, `nim_client`, `bottom_pane.active_model`, `models_screen_state`, and re-injecting runtime into `ToolRegistry`.
9. **Failure Safety:** If any step fails, the previous active model, client, tools, and UI states remain completely unmutated. Modal views remain open and display actionable error messages.

---

## 6. Verification & Test Evidence

1. **`test_model_list_rendering_single_row_invariant_and_scrollbar`** (`tests/tui_regression_test.rs`):
   - Renders 50 models into a 70x12 terminal with `inner_height = 10`.
   - Asserts buffer rows 1 to 10 contain exactly 10 distinct models without overflow.
   - Asserts vertical Ratatui Scrollbar symbols (`▲`, `█`, `│`) render on rightmost column.
   - Asserts `render_model_details` populates specification fields in the right pane.
2. **`test_large_model_catalog_scrolling_and_navigation`** (`tests/tui_regression_test.rs`):
   - 150-model catalog navigation: `PageDown`, `End`, `Home`, and search filtering.
3. **`test_modal_model_picker_keyboard_interaction_lifecycle`** (`tests/tui_regression_test.rs`):
   - Tests opening `Ctrl+M`, `Down`, `Up`, `PageDown`, `Home`, character search typing, `Backspace`, and `Esc`.
4. **`test_transactional_model_switching_updates_agent_runtime`** (`tests/tui_regression_test.rs`):
   - Verifies successful activation updates both `active_model` and `nim_client.model()`.
   - Verifies missing credentials fail honestly and preserve existing active model.
5. **`test_pty_model_picker_selection_and_switch`** (`tests/pty_end_to_end_test.rs`):
   - Real PTY test verifying model picker selection and switch in an interactive terminal session.
