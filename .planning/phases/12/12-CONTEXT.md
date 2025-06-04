# Phase 12: UX & Editor Experience Adaptations — Context

**Gathered:** 2026-06-01
**Status:** Ready for planning
**Source:** `rush/opencode_adaptation_report.md` (adaptation items 8, 11, 12, 13, 14)

<domain>
## Phase Boundary

Phase 12 implements five OpenCode adaptations focused on user experience improvements — model management, shell mode, prompt history, diff viewing, and editor context integration.

**Depends on:** Phase 10 (Provider & Message Layer Adaptations)

**Duration:** 2.5 weeks | **Complexity:** 6/10 | **Milestone:** Model variants and favorites persist; shell mode bypasses LLM; prompt history persists with frecency; diff viewer renders styled output; `@file` syntax includes file content

### Adaptations

1. **Model Variants & Favorites** (adoption #8) — Variant field on ModelInfo, recent/favorite lists, keyboard cycling, per-agent models.

2. **Shell Mode** (adoption #12) — `!` prefix in REPL bypasses LLM for direct command execution.

3. **Prompt History with Frecency Ranking** (adoption #11) — Persistent prompt storage with frequency/recency scoring, arrow-up/down navigation.

4. **Diff Viewing Enhancement** (adoption #13) — Dedicated diff viewer with syntax highlighting, split/unified format.

5. **Editor Context Auto-Include** (adoption #14) — `@filepath` syntax in REPL auto-includes file contents.
</domain>

<decisions>
## Implementation Decisions

### Model Variants & Favorites

- Add `Variant *string` field to `ModelInfo` in `internal/types/types.go` — nil by default, set to "thinking" / "fast" / "extended" etc.
- Recent models: store last 10 model IDs in `~/.m31a/recent_models.json` as JSON array
- Favorites: store user-pinned model IDs in same file under `favorites` key
- `Ctrl+M` cycles forward through recent list; `Ctrl+Shift+M` cycles backward
- Per-agent model assignment: `[model.agents.{agent_name}]` in config.toml with `default` field
- Model selector UI (07-07) updated to show variant badge, pin/unpin favorites

### Shell Mode

- In `repl.go` `Update()` on `submitMsg`: if input starts with `!`, strip prefix and execute via `tools.Bash`
- Return result as assistant message with content `$ {command}\n{output}`
- No permission modal for shell commands (user explicitly requested execution)
- Risk: follows `PermissionRule` for the Bash tool if glob patterns match
- Shell mode bypasses LLM and message history — command and output appear in chat but don't count as a conversation turn

### Prompt History with Frecency

- `~/.m31a/prompt_history.json` format: `{"entries": [{"text": "...", "last_used": "ISO8601", "frequency": 5, "first_used": "ISO8601"}]}`
- On each submit, upsert the prompt text: if exists, bump frequency and update last_used; if new, append with frequency=1
- Frecency score: `score = frequency / (hours_since_last_use + 1)` — higher is better
- Arrow-up navigates through history; shows top 10 frecency-ranked matches matching current input prefix
- `internal/tui/history.go` — FrecentHistory struct with Load(), Save(), Upsert(), Search(prefix string, limit int)
- Max entries: 1000; oldest entries evicted by frecency score on save

### Diff Viewing Enhancement

- `internal/tui/diff.go` — DiffModel struct implementing tea.Model with Init/Update/View
- Renders `git diff` output with lipgloss syntax highlighting (green for additions, red for deletions)
- Keys: `s` toggle split/unified, `d` toggle diff format, arrows scroll
- `/diff` command enhanced to accept `--staged`, `--stat`, `<commit>` args
- Uses `internal/git/git.go` Diff/DiffStaged methods

### Editor Context Auto-Include

- Before sending a chat request, scan input for `@filepath` patterns
- `@./path/to/file.go` or `@path/to/file.go` — resolve relative to cwd
- Replace `@filepath` with file contents: `--- filepath ---\n{content}\n---`
- If file not found, leave `@filepath` as-is (no error) or show warning inline
- File size limit: 100KB (below the 5MB MaxFileSize for FileRead tool) to avoid context bloat
- Binary files show `@filepath [binary, N bytes]` instead of content
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Files to Modify/Create

- `internal/types/types.go` — ModelInfo (add Variant), Message (add RevertedTo)
- `internal/tui/modelselector.go` — variant badge, favorites, recent list
- `internal/tui/keybindings.go` — Ctrl+M, Ctrl+Shift+M model cycling
- `internal/tui/repl.go` — ! prefix, @filepath syntax
- `internal/tui/history.go` — FrecentHistory (create)
- `internal/tui/diff.go` — DiffModel (create)
- `internal/tui/commands.go` — enhance /diff
- `internal/config/types.go` — per-agent model assignments
- `pkg/session/manager.go` — recent/favorite model persistence
- `internal/config/loader.go` — model assignments config

### Existing Code to Study

- `internal/tui/repl.go` — input handling, submitMsg processing
- `internal/tui/commands.go` — handleDiff at line 891
- `internal/tui/modelselector.go` — ModelSelectorModel, search, detail pane
- `internal/tui/keybindings.go` — existing key bindings
- `internal/git/git.go` — Diff(), DiffStaged() methods
- `internal/types/types.go` — ModelInfo struct
- `pkg/session/manager.go` — existing persistence patterns
- `docs/INTERFACES.md` — All interface definitions
</canonical_refs>

<specifics>
## Wave Structure

**Wave 1** (no dependencies):
- Plan 01: Model Variants & Favorites System
- Plan 02: Shell Mode

**Wave 2** (no dependencies between them):
- Plan 03: Prompt History with Frecency
- Plan 04: Diff Viewer Screen
- Plan 05: Editor Context Auto-Include

### Key Patterns to Follow

- JSON file persistence with atomic writes for history and model lists
- CommandResult pattern for TUI state transitions (already established in Phase 7)
- Lipgloss for all visual styling; glamour for markdown rendering
- Bubble Tea model pattern (Init/Update/View) for new TUI models
- File size limits to prevent memory issues
</specifics>

<deferred>
## Deferred Ideas

- Autocomplete dropdown in REPL for prompt history
- Tab-completion for @file paths
- Interactive diff staging (partial commit)
- Syntax highlighting via tree-sitter or chroma (uses glamour which wraps chroma)
- Full plugin-based TUI slot system
</deferred>

---

*Phase: 12-ux-editor-experience-adaptations*
*Context gathered: 2026-06-01 via OpenCode Adaptation Report*
