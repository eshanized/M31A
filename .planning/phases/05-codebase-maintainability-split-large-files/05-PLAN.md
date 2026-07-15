---
phase: 05-codebase-maintainability-split-large-files
plan: 01
type: execute
wave: 1
depends_on: []
files_modified: []
autonomous: true
requirements: [REQ-01, REQ-02, REQ-03, REQ-04, REQ-05, REQ-06]
user_setup: []

must_haves:
  truths:
    - All critical path files (>500 lines) are split into focused modules
    - The project builds successfully after each file split
    - All tests pass after each file split
    - Code formatting is correct (gofmt, goimports)
    - No import cycles are introduced
    - pkg/ must NOT import internal/ constraint is maintained
    - Each split file is committed separately
  artifacts:
    - Split files with descriptive names based on content
    - Updated test files to match new file locations
    - Clean import statements via goimports
    - One commit per file split
  key_links:
    - goimports -w after each split
    - make check after each split
    - One commit per file split
---

<objective>
Split the most critical large Go files (>500 lines) into smaller, focused files organized by functionality. Start with low-risk leaf packages, then medium-risk tool and TUI packages, then high-risk workflow and entry point files. Each split must be verified with `make check` and committed separately.

Purpose: Large files reduce code readability and make maintenance difficult. Splitting by functional responsibility creates focused modules that are easier to understand, test, and modify. Starting with low-risk files builds confidence before tackling critical path components.

Output: Critical path files split into focused modules, each verified by automated testing and committed individually.
</objective>

<execution_context>
@/home/snigdha/.config/opencode/gsd-core/workflows/execute-plan.md
@/home/snigdha/.config/opencode/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/05-codebase-maintainability-split-large-files/05-CONTEXT.md
@.planning/phases/05-codebase-maintainability-split-large-files/05-RESEARCH.md
@.planning/codebase/STRUCTURE.md
@.planning/codebase/ARCHITECTURE.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Split low-risk leaf packages (helpers, transition, app_input)</name>
  <files>internal/tui/helpers.go, internal/tui/transition.go, internal/tui/app_input.go, internal/tui/helpers_*.go, internal/tui/transition_*.go, internal/tui/app_input_*.go</files>
  <action>
Per D-01, D-03, D-09: Split three low-risk files by functionality. These are leaf packages with minimal dependents.

**File 1: internal/tui/helpers.go (402 lines)**
1. Read helpers.go to identify functional groups (string helpers, file helpers, UI helpers)
2. Create: helpers_string.go, helpers_file.go, helpers_ui.go
3. Move functions to appropriate files

**File 2: internal/tui/transition.go (404 lines)**
1. Read transition.go to identify transition types (screen, phase, state)
2. Create: transition_screen.go, transition_phase.go, transition_state.go
3. Move transition functions to appropriate files

**File 3: internal/tui/app_input.go (474 lines)**
1. Read app_input.go to identify input categories (keyboard, mouse, paste)
2. Create: app_input_keyboard.go, app_input_mouse.go, app_input_paste.go
3. Move input handlers to appropriate files

After each file split:
- Run goimports -w internal/tui/
- Run make check
- Commit with: refactor(tui): split [filename] by functionality per D-03
  </action>
  <verify>
    <automated>make check</automated>
  </verify>
  <done>
    - helpers.go is under 200 lines
    - transition.go is under 200 lines
    - app_input.go is under 200 lines
    - All new files are under 200 lines
    - make check passes after each split
    - Three commits created (one per file)
  </done>
</task>

<task type="auto">
  <name>Task 2: Split medium-risk tool package files (dispatcher, permissions, edit)</name>
  <files>internal/tools/dispatcher.go, internal/tools/permissions.go, internal/tools/edit.go, internal/tools/dispatcher_*.go, internal/tools/permissions_*.go, internal/tools/edit_*.go</files>
  <action>
Per D-01, D-03, D-09: Split three medium-risk tool package files by functionality.

**File 1: internal/tools/dispatcher.go (492 lines)**
1. Read dispatcher.go to identify concerns (core dispatch, rate limiting, concurrency)
2. Create: dispatcher_rate_limit.go, dispatcher_concurrency.go
3. Move rate limiting and concurrency logic to new files

**File 2: internal/tools/permissions.go (691 lines)**
1. Read permissions.go to identify permission types (rules, approval, checking)
2. Create: permissions_rules.go, permissions_approval.go, permissions_checker.go
3. Move permission logic to appropriate files

**File 3: internal/tools/edit.go (821 lines)**
1. Read edit.go to identify edit strategies (replace, insert, delete, diff)
2. Create: edit_replace.go, edit_insert.go, edit_delete.go, edit_diff.go
3. Move edit strategies to appropriate files

After each file split:
- Run goimports -w internal/tools/
- Run make check
- Commit with: refactor(tools): split [filename] by functionality per D-03
  </action>
  <verify>
    <automated>make check</automated>
  </verify>
  <done>
    - dispatcher.go is under 200 lines
    - permissions.go is under 200 lines
    - edit.go is under 200 lines
    - All new files are under 200 lines
    - make check passes after each split
    - Three commits created (one per file)
  </done>
</task>

<task type="auto">
  <name>Task 3: Split medium-risk TUI package files (settings_model, sidebar_model, app)</name>
  <files>internal/tui/settings_model.go, internal/tui/sidebar_model.go, internal/tui/app.go, internal/tui/settings_*.go, internal/tui/sidebar_*.go, internal/tui/app_*.go</files>
  <action>
Per D-01, D-03, D-09: Split three medium-to-high risk TUI package files by functionality.

**File 1: internal/tui/settings_model.go (901 lines)**
1. Read settings_model.go to identify tab/function groups
2. Create: settings_update.go, settings_view.go, settings_tabs.go
3. Move Update(), View(), and tab rendering to appropriate files

**File 2: internal/tui/sidebar_model.go (876 lines)**
1. Read sidebar_model.go to identify model/view separation
2. Create: sidebar_update.go, sidebar_view.go, sidebar_render.go
3. Move rendering logic to new files

**File 3: internal/tui/app.go (773 lines)**
1. Read app.go to identify app lifecycle concerns
2. Create: app_lifecycle.go, app_state.go (if not exists)
3. Move initialization and lifecycle methods

After each file split:
- Run goimports -w internal/tui/
- Run make check
- Commit with: refactor(tui): split [filename] by functionality per D-03
  </action>
  <verify>
    <automated>make check</automated>
  </verify>
  <done>
    - settings_model.go is under 200 lines
    - sidebar_model.go is under 200 lines
    - app.go is under 200 lines
    - All new files are under 200 lines
    - make check passes after each split
    - Three commits created (one per file)
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| pkg/ → internal/ | Module boundary must not be violated during splits |
| Test files → source files | Tests must be updated when source files are split |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-05-01 | Tampering | Import statements | medium | mitigate | Run goimports -w after each split to fix imports |
| T-05-02 | Repudiation | Split changes | low | accept | One commit per split provides audit trail |
| T-05-03 | Information Disclosure | Code organization | low | accept | Splitting improves readability, not security |
| T-05-04 | Denial of Service | Build system | medium | mitigate | Run make check after each split to catch issues |
| T-05-05 | Elevation of Privilege | Module boundary | high | mitigate | Verify pkg/ does not import internal/ after splits |
</threat_model>

<verification>
- After each task: make check must pass
- After all tasks: wc -l on all .go files must show <200 lines
- go list ./... must show no import cycles
- git log must show one commit per split
</verification>

<success_criteria>
- Critical path files are split into focused modules
- Project builds successfully
- All tests pass with race detector
- Code formatting is correct
- No import cycles introduced
- pkg/ → internal/ constraint maintained
- Each split committed separately
</success_criteria>

<output>
Create `.planning/phases/05-codebase-maintainability-split-large-files/05-01-SUMMARY.md` when done
</output>
