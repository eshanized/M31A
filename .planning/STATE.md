---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: Release
status: Phase 12 complete
last_updated: "2026-06-01T00:38:39.691Z"
progress:
  total_phases: 9
  completed_phases: 6
  total_plans: 42
  completed_plans: 34
  percent: 67
---

# M31A — Current State

## Active Phase

Phase 12 complete — UX & Editor Experience Adaptations

## Status

Phase 12 complete — Model variants/favorites, shell mode, prompt history, diff viewer, @filepath auto-include

## Completed Phases

- Phase 0 — Foundation & Documentation
- Phase 1 — Provider Abstraction Layer
- Phase 2 — TUI Foundation
- Phase 3 — Message Rendering Pipeline
- Phase 4 — Tool System
- Phase 5 — Session State & Configuration
- Phase 6 — Workflow Engine
- Phase 7 — Signature Features
- Phase 10 — Provider & Message Layer Adaptations
- Phase 11 — Session & Config Adaptations
- Phase 12 — UX & Editor Experience Adaptations

## Completed Plans (Phase 10)

- 10-01: Structured Tool Calls from SSE
- 10-02: Undo/Redo Session Management
- 10-03: Auto Context Compaction

## Completed Plans (Phase 11)

- 11-01: Session Forking — ParentID/ChildrenIDs, ForkSession, /fork /prev /next commands
- 11-02: Multi-Layer Configuration — project-level m31a.toml, validation, var substitution
- 11-03: Permission Ruleset Completion — glob matching, per-agent profiles

## Completed Plans (Phase 12)

- 12-01: Model Variants & Favorites System
- 12-02: Shell Mode — ! prefix REPL command execution
- 12-03: Prompt History with Frecency Ranking
- 12-04: Diff Viewer Screen — lipgloss-styled git diff
- 12-05: Editor Context Auto-Include — @filepath syntax

## Key Decisions Made

### Phase 12 Decisions

- ModelInfo.Variant is `*string` — nil by default, values: "thinking", "fast", "extended", "vision"
- RecentModelsData stored at `~/.m31a/recent_models.json` with atomic writes; max 10 recent entries
- Ctrl+M/Ctrl+Shift+M bindings registered in CtxREPL (not global) — global fallback keeps ctrl+x m chord
- Shell mode (!) bypasses LLM entirely — commands execute via dispatcher without permission modal (PermissionRule globs still apply)
- Shell results display as assistant messages but don't count as conversation turns for LLM context
- FrecentHistory: score = frequency / (hours_since_last_use + 1); max 1000 entries with frecency-based eviction on save
- Arrow-up frecency navigation activates when textarea has content; falls back to in-memory session history when empty
- DiffModel uses lipgloss styling: green (#81C995) for additions, red (#F28B82) for deletions, brand (#D77757) for hunk headers
- /diff routes to full-screen diff viewer by default; --stat outputs inline; <commit> accepts any git ref
- @filepath regex requires a path separator (`/`) — prevents matching @user, @mention, email addresses
- File expansion limited to 100KB; binary detection via 512-byte mime sniff

### Phase 11 Decisions

- SiblingSessions includes the current session in its return list for consistent index-based prev/next navigation
- ForkSession uses retry loop (max 10) for ID collision avoidance
- Corrupt children are silently skipped in ListChildren for resilience
- Session switching clears REPL messages and reloads from the loaded session
- Deep copy on fork: parent messages and project state are copied via marshal/unmarshal to prevent aliasing bugs

### Phase 7 Decisions

- Slash commands implemented as pure functions returning CommandResult (not direct AppState mutation)
- CommandContext carries Registry, SessionManager, Git, Ledger, Rollback, AutoDream references
- AutoDream preserves: system messages, last 5 messages, tool call messages, first message (goal)
- Summary prefix: [AutoDream Context Summary], token estimation: words × 1.3
- LEDGER.md uses Markdown table format with 9 columns (user-approved deviation from original spec)
- Model selector uses bubbles/list with textinput for search; P cycles provider filter
- Settings uses *config.Config + path string for save (not config.Loader — no such type exists)
- Fallback banner dismisses on 'x' key, auto-dismiss after 15s, or on user input
- Model cost display uses standard usage estimate (100K in + 50K out, not raw per-M rates)
- Model item FilterValue includes name, ID, description, provider
- Settings tabs: General, Provider, Model, Permissions, Features, Ledger (6 total)

### Earlier Phases (preserved)

- Sequential task execution in V1 (no concurrency)
- Only OpenRouter and Zen as LLM gateways
- CGO_ENABLED=0 static binary
- API keys always masked as "••••••••" in settings View(), never displayed in plaintext
- SSE parser uses line-by-line bufio.Scanner (not custom split on \n\n)

## Blockers

None
