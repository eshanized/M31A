# REPL Coupling Trace Investigation

**Phase:** 3.5 (Investigation Only — No Migration)
**Goal:** Complete mutation and read trace for ReplModel to determine migration safety
**Output:** `docs/audits/repl-coupling-trace.md`

---

## Context

REPL is the last screen to migrate in Phase 3 and has the highest coupling of anything migrated so far. Per `docs/audits/screen-inventory.md`:

> "AppState handlers directly mutate replModel fields (streaming, thinking, messages, toolCards, lastUsage, lastCost). Sidebar calls StartTokenBurn(), AddToolCallStart(), CompleteToolCall()."

Unlike Execute (one-way coupling to Plan) or ChatHistory (already fixed), REPL has genuine bidirectional coupling with Sidebar and is read by multiple other models at View()-time.

---

## Plan

### Task 1: Mutation Trace (Part A)

**Goal:** Find every place that writes to a field on `m.replModel` or calls a setter on it.

**Files to trace:**
- `internal/tui/app_handlers.go` — streaming, thinking, thinkingStartAt, lastStatus, activeProvider
- `internal/tui/handler_stream.go` — delegates to replModel stream handlers
- `internal/tui/handler_tool.go` — tool card creation/updates
- `internal/tui/handler_modal.go` — AddMessage(), handleThinkingToggle()
- `internal/tui/handler_navigation.go` — textarea.SetValue(), textarea.Focus()
- `internal/tui/app_input.go` — width, height (content dimensions)
- `internal/tui/app_session.go` — session restore reads Messages()
- `internal/tui/app_agent.go` — activeProvider sync, Messages() for session save
- `internal/tui/app.go` — lastStatus, workflowPhase, workflowPhaseIndex, totalPhases
- `internal/tui/helpers.go` — SetProvider(), SetDispatcher(), SetSessionID(), ClearMessages(), AddMessage()
- `internal/tui/app_view.go` — width, height via syncReplSize(), SetSidebarWidth()

**Classification for each site:**
- **SAFE:** Called from Update()-path handler with value passed via message
- **UNSAFE:** Reads live engine/goroutine state directly
- **UNCLEAR:** Cannot determine without deeper trace

**Special attention:**
- Streaming chunk data — most likely place for goroutine boundary crossing
- Any site touching `streamCh`, `streamContent`, `streamSegments`

### Task 2: Read Trace (Part B)

**Goal:** Enumerate every place other code reads `m.replModel` fields directly.

**Known sites (from existing audits):**
- `buildHeaderInfo()` (app_view.go:302-367) — lastUsage.TotalTokens, activeModel
- `buildFooterInfo()` (app_view.go:370-478) — cwd, thinking, streaming, lastUsage, lastCost
- `syncReplSize()` (app_view.go:985-998) — width, height
- `updateSidebarUsage()` (app_view.go:1009-1036) — lastUsage, activeModel, lastCost
- `autoDream` (app.go:687-697) — Messages(), SetMessages()

**Additional sites to check:**
- Sidebar-related handlers
- Anything reading `.streaming`, `.thinking` for conditional UI
- View-path reads in renderDimmedModal, renderREPLContent

**Classification:**
- Confirm each is a same-thread, Update()-or-View()-path read of TUI-internal state
- Confirm none crosses into workflow engine's goroutine

**Screenable migration safety:**
- Confirm reads would not break once replModel is wrapped as Screenable
- Verify dual-reference pattern (concrete field + router registration) holds

### Task 3: Sidebar Coupling Trace (Part C)

**Goal:** Trace bidirectional REPL-Sidebar coupling.

**Sidebar → REPL (known):**
- `StartTokenBurn()` — called by Sidebar
- `AddToolCallStart()` — called by Sidebar
- `CompleteToolCall()` — called by Sidebar

**REPL → Sidebar (to verify):**
- Check if REPL calls into Sidebar directly anywhere
- screen-inventory.md only documented Sidebar→REPL direction
- Confirm no REPL→Sidebar direction exists

**Migration impact:**
- Confirm Sidebar is NOT being migrated (not in Screen enum per PHASE3_PLAN.md)
- Confirm Sidebar's coupling to replModel is unaffected by REPL's Router status

### Task 4: Write Audit Document

**Output:** `docs/audits/repl-coupling-trace.md`

**Structure:**
```markdown
# REPL Coupling Trace

## Part A: Mutation Sites
[Table of all mutation sites with SAFE/UNSAFE/UNCLEAR classification]

## Part B: Read Sites
[Table of all read sites with thread-safety classification]

## Part C: Sidebar Coupling
[Bidirectional coupling analysis]

## Migration Readiness
[Summary: what's safe, what needs attention, recommended approach]
```

---

## Verification

After completing the trace:
1. All mutation sites documented with classification
2. All read sites documented with thread-safety confirmation
3. Bidirectional Sidebar coupling confirmed
4. Migration readiness assessment written
5. No code changes made (investigation only)

---

## Files Created/Modified

- `docs/audits/repl-coupling-trace.md` (new — the investigation output)

**No migration in this task.** Even if everything comes back clean, do not migrate REPL.
