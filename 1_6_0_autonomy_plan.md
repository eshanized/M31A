# M31A v1.6.0 — Autonomy Implementation Plan

Rendering is complete. This plan covers the four autonomy features from the v1.6 roadmap, plus critical infrastructure fixes that enable them.

---

## Status: Existing vs. Needed

| Feature | What Exists | What Needs Building |
|---------|------------|---------------------|
| **Deferred Tools** | Per-tool "remember" cache, concurrent request routing, persistent permissions, risk-level rate limiting, non-interactive blocking | Batch approval queue, session-scoped "approve all", queue depth display, approval UX |
| **Adaptive Context Budgeting** | Cost budget limit (`BudgetLimitUSD`), reactive auto-compaction, AutoDream compression | Proactive 70% warning, adaptive context allocation, phase-aware budgeting |
| **Ghost Mode** | `--prompt` single-turn headless, TUI ghost write picker/output viewer | Full-workflow headless execution, non-interactive permissions, exit codes, progress output |
| **Cross-Session Knowledge** | Ledger with session metadata/stats, keyword extraction | Knowledge extraction from sessions, persistent store, injection into new sessions |

---

## Priority Order (by user value)

| Priority | Feature | Rationale |
|----------|---------|-----------|
| **P0** | Deferred Tools + Emitter Fix | Biggest friction in autonomous mode. C3 is Critical. C2 causes silent data loss. |
| **P1** | Adaptive Context Budgeting | Prevents context overflow in long sessions. M3 is Medium but blocks autonomous runs. |
| **P2** | Ghost Mode | Highest leverage for CI/CD and automation. Requires P0 (permissions) and P1 (context) to work. |
| **P3** | Cross-Session Knowledge | Compounds value over time. Lowest immediate urgency but highest long-term value. |

---

## Wave 1: Deferred Tools + Emitter Reliability

**Objective:** Eliminate the #1 friction point in autonomous mode. Users can batch-approve dangerous tool calls instead of approving each one individually. Workflow event delivery becomes reliable.

**Estimated effort:** 3-4 days

### 1A: Emitter Channel Reliability (C2)

**Affected packages:** `internal/tui/`

**Implementation:**

1. Increase `emitterCh` buffer from 128 to 512 in `app.go` constructor
2. Replace goroutine spawn in `TodoWrite` callback (line 452-456) with bounded retry (3 attempts, drop on failure)
3. Surface `droppedMsgCount` in status bar footer when > 0
4. Add `drainMultipleCmd()` that reads up to 4 messages per tick under load (when channel > 75% full)

**Files modified:**
- `internal/tui/app.go` — channel buffer size, TodoWrite callback
- `internal/tui/app_channel.go` — Emit with bounded retry, drop counter
- `internal/tui/app_view.go` — status bar drop count display
- `internal/tui/repl_footer.go` — drop count badge

**Regression risks:**
- Buffer increase: safe (more memory, fewer drops)
- Bounded retry: safer than current unbounded goroutines
- Drain behavior: must preserve Bubble Tea single-threaded contract (read one msg per tick is the rule; reading multiple is safe if done synchronously)

**Validation:**
- Unit test: emitter drops zero messages at 200 msg burst
- Unit test: bounded retry caps at 3 attempts
- Unit test: drop count surfaces correctly
- Integration test: workflow with 4+ parallel tools delivers all events

### 1B: Batch Permission Approval (C3)

**Affected packages:** `internal/tools/`, `internal/tui/components/`, `internal/tui/`

**Implementation:**

1. **Permission queue with session scope:**
   - Add `ApprovalQueue` struct to `permissions.go` that holds pending tool calls
   - Queue entries: `{requestID, toolName, command, riskLevel, taskID, timestamp}`
   - Queue is session-scoped (lives for the duration of a workflow)

2. **"Approve All" mechanism:**
   - Add `[A] Approve all` to permission modal (reusing existing `A` key but with new semantics)
   - When user selects "Approve All": cache approval for `toolName` + `riskLevel` combination for current task
   - New approval source: `"batch_approval"` — sits between `"rule"` and `"agent_default"` in evaluation pipeline
   - Batch approvals expire when task completes or workflow phase transitions

3. **Queue depth visibility:**
   - Add `QueueDepth` field to `PermissionRequest`
   - Display in permission modal: "3 tools queued behind this one"
   - Add `ToolQueueDepth` to workflow events for sidebar/execute screen display

4. **Scoped batch approval:**
   - Task-scoped: approvals last until current task completes
   - Phase-scoped: optional `/approve-phase` command for phase-level batching
   - Visual indicator: "Batch active: bash (3 remaining)" in footer

**Files modified:**
- `internal/tools/permissions.go` — ApprovalQueue, batch approval logic, new "batch_approval" source
- `internal/tools/permissions.go` — `ensurePermission()` adds batch_approval check before askPermission
- `internal/tools/interface.go` — QueueDepth field on PermissionRequest
- `internal/tui/components/permission.go` — "Approve All" option, queue depth display
- `internal/tui/app_update.go` — handlePermissionResponse with batch logic
- `internal/tui/repl_footer.go` — batch approval status indicator

**Regression risks:**
- Security: batch approvals are session-scoped and task-scoped, not permanent
- Must not weaken existing per-tool "remember" cache
- Must not auto-approve tools the user hasn't explicitly batch-approved
- Existing `interactive=false` mode must continue to block dangerous tools

**Validation:**
- Unit test: batch approval caches by toolName+riskLevel
- Unit test: batch approval expires on task completion
- Unit test: batch approval expires on phase transition
- Unit test: queue depth reported correctly
- Integration test: 8 bash commands in a task → user approves first → remaining 7 auto-approve
- Integration test: new task after batch → permissions reset

### 1C: Approval Queue UI

**Affected packages:** `internal/tui/`

**Implementation:**

1. Add `/pending` slash command that shows pending approval queue
2. Add queue count to sidebar execute screen footer
3. Add batch approval status to footer: "Batch: bash ×3" when active

**Files modified:**
- `internal/tui/commands/commands.go` — register `/pending`
- `internal/tui/sidebar_model.go` — queue count in execute footer
- `internal/tui/repl_footer.go` — batch status indicator

**Regression risks:** Low. Additive display.

**Validation:**
- `/pending` shows correct queue
- Queue count updates in real-time
- Batch status shows active tool and count

---

## Wave 2: Adaptive Context Budgeting

**Objective:** Prevent context overflow in long autonomous sessions. Users see proactive warnings and M31A manages context budget dynamically.

**Estimated effort:** 2-3 days

### 2A: Proactive Context Warning (M3)

**Affected packages:** `internal/tui/`

**Implementation:**

1. Add toast notification at 70% context usage: "Context at 70%. Consider /compress."
2. Add second toast at 85%: "Context at 85%. Compaction recommended."
3. Auto-suggest `/compress` in chat when 70% reached (insert system message)
4. Track whether warning was already shown (avoid repeating every frame)

**Files modified:**
- `internal/tui/repl_stream.go` — add threshold checks alongside existing 95% error
- `internal/tui/repl_footer.go` — context ring already shows color; add toast trigger
- `internal/tui/toast.go` — existing toast system, no changes needed

**Regression risks:** Low. Additive notification.

**Validation:**
- Unit test: toast fires at 70% threshold
- Unit test: toast fires at 85% threshold
- Unit test: warning not repeated within same session
- Manual: verify toast appears during long conversation

### 2B: Adaptive Context Allocation

**Affected packages:** `internal/workflow/`, `pkg/compaction/`

**Implementation:**

1. **Phase-aware context ratios:**
   - Plan phase: keep more conversation history (plan benefits from full context)
   - Execute phase: keep more tool results (execution needs recent outputs)
   - Verify phase: keep more code context (verification needs file content)
   - Add `ContextBudget` config with per-phase ratios

2. **Proactive compaction scheduling:**
   - Before each phase transition, check if context > 60% and trigger compaction
   - During Execute phase, check after every N tool calls (configurable)
   - Use existing `compactor.ShouldCompact()` but call it proactively

3. **Compaction quality improvement:**
   - Extend compaction summary template to preserve: tool usage patterns, decision rationale, task progress
   - Add `tool_summary` section to compaction output: "Used bash 5 times, edit 3 times, grep 8 times"

**Files modified:**
- `internal/workflow/engine.go` — proactive compaction checks in RunPhase
- `internal/config/types.go` — ContextBudget config
- `internal/config/loader.go` — default values
- `pkg/compaction/compaction.go` — extended template

**Regression risks:**
- Compaction already works; changes are to trigger timing and summary quality
- Must not compact too aggressively (lose important context)
- Must not compact too conservatively (still overflow)

**Validation:**
- Unit test: compaction triggers at 60% on phase transition
- Unit test: phase-specific context ratios applied
- Unit test: compaction summary preserves tool patterns
- Integration test: 20-task plan completes without context overflow
- Integration test: context budget visible in sidebar

### 2C: Context Budget Visibility

**Affected packages:** `internal/tui/`

**Implementation:**

1. Add context budget breakdown to sidebar: "Context: 45% conversation, 30% tools, 15% system, 10% code"
2. Add compaction event to sidebar: "Compacted: 12K → 4K tokens (67% reduction)"
3. Add context forecast: "At current rate, overflow in ~8 turns"

**Files modified:**
- `internal/tui/sidebar_model.go` — budget breakdown section
- `internal/tui/repl_state.go` — context metrics tracking

**Regression risks:** Low. Additive display.

**Validation:**
- Budget breakdown shows correct percentages
- Compaction events logged correctly
- Forecast reasonably accurate (within 2 turns)

---

## Wave 3: Ghost Mode (Headless Execution)

**Objective:** M31A can execute full workflows without the TUI. Enables CI/CD integration, batch operations, and automation.

**Estimated effort:** 4-5 days

### 3A: Headless Workflow Engine

**Affected packages:** `cmd/m31a/`, `internal/workflow/`, `internal/tools/`

**Implementation:**

1. **New CLI flags:**
   - `--headless` — run full workflow without TUI
   - `--goal "..."` — the goal to achieve (replaces `--prompt` for full workflow)
   - `--max-turns N` — safety limit on LLM turns (default 50)
   - `--output-format diff|log|json` — what to produce (default: diff)
   - `--permission-policy auto-allow-safe|auto-deny|prompt` — how to handle permissions

2. **Headless mode flow:**
   ```
   main.go --headless --goal "add error handling to main.go"
     → Create engine (same as TUI mode)
     → Set permission policy from flag
     → Run workflow: Initialize → Discuss → Plan → Execute → Verify → Ship
     → Emit progress to stdout (structured, parseable)
     → Exit with code 0 (success) or 1 (failure)
   ```

3. **Non-interactive permission handling:**
   - `auto-allow-safe`: auto-approve tools with risk < dangerous
   - `auto-deny`: deny all tools requiring permission (safe mode)
   - `prompt`: fall back to stdin prompt (for manual use)
   - Configurable per-tool via permission rules

4. **Structured output:**
   - Progress events: `{"phase":"execute","task":3,"total":10,"status":"running"}`
   - Tool events: `{"tool":"bash","command":"go test ./...","status":"success","duration_ms":1200}`
   - Completion: `{"status":"success","commits":["abc123"],"tasks_completed":10,"cost_usd":0.42}`
   - Error: `{"status":"error","phase":"verify","error":"build failed","exit_code":1}`

5. **Result persistence:**
   - Save session to `.m31a/sessions/` (same as TUI mode)
   - Write ledger entry (same as TUI mode)
   - Create git commits (same as TUI mode)
   - Produce diff output to stdout (if `--output-format diff`)

**Files modified:**
- `cmd/m31a/main.go` — new flags, headless workflow runner
- `cmd/m31a/headless.go` — new file: headless mode implementation
- `internal/workflow/engine.go` — headless mode support (no TUI emitter)
- `internal/tools/permissions.go` — permission policy mode
- `internal/tools/dispatcher.go` — headless permission handling

**Regression risks:**
- Must not affect TUI mode at all
- Headless mode must use same engine, same tools, same providers
- Permission policy must be strict by default (auto-deny for dangerous)
- Exit codes must be reliable for CI/CD

**Validation:**
- Unit test: headless mode creates engine correctly
- Unit test: permission policy auto-allows safe tools
- Unit test: permission policy auto-denies dangerous tools
- Unit test: structured output is valid JSON
- Integration test: `--headless --goal "create hello.go"` produces working code
- Integration test: `--headless --permission-policy auto-deny` blocks dangerous tools
- E2E test: exit code 0 on success, 1 on failure

### 3B: Ghost Mode Progress Display

**Affected packages:** `cmd/m31a/`

**Implementation:**

1. **Terminal progress output:**
   ```
   [Initialize] Detecting project type... Go project detected
   [Discuss] 3 clarifying questions (auto-answered in headless mode)
   [Plan] Generated 8-task plan
   [Execute] Task 1/8: Adding error handling to main.go ✓ (1.2s)
   [Execute] Task 2/8: Adding tests... ✓ (3.4s)
   [Verify] Build passed, tests passed
   [Ship] Committed: feat: add error handling to main.go
   ```

2. **Quiet mode:** `--quiet` flag suppresses progress, only shows final result
3. **Verbose mode:** `--verbose` flag shows full LLM responses and tool outputs

**Files modified:**
- `cmd/m31a/headless.go` — progress formatting

**Regression risks:** Low. Output only.

**Validation:**
- Progress output is parseable
- Quiet mode suppresses all progress
- Verbose mode shows full detail

### 3C: Headless Permission Integration

**Affected packages:** `internal/tools/`, `internal/workflow/`

**Implementation:**

1. Wire permission policy from CLI flags into dispatcher
2. Add `PermissionPolicy` type: `AllowSafe | DenyAll | Prompt`
3. In `ensurePermission()`, check policy before asking TUI modal
4. Log all permission decisions in headless mode for audit trail

**Files modified:**
- `internal/tools/permissions.go` — PermissionPolicy type, policy check in ensurePermission
- `internal/tools/dispatcher.go` — policy configuration
- `internal/workflow/engine.go` — pass policy to dispatcher

**Regression risks:**
- Must not affect interactive mode
- Policy must default to DenyAll (safest)

**Validation:**
- Unit test: AllowSafe policy auto-approves safe tools
- Unit test: DenyAll policy blocks all permission-requiring tools
- Unit test: Prompt policy falls back to stdin
- Integration test: headless mode with DenyAll completes safe workflow

---

## Wave 4: Cross-Session Knowledge Carryover

**Objective:** M31A learns from every session and applies that knowledge in future sessions. Makes M31A smarter on each project over time.

**Estimated effort:** 3-4 days

### 4A: Knowledge Extraction

**Affected packages:** `pkg/ledger/`, `pkg/session/`

**Implementation:**

1. **Extraction at Ship phase:**
   - After successful commit, extract from session:
     - Tool usage patterns (which tools were used for which task types)
     - Error patterns (what went wrong, how it was resolved)
     - Convention observations (naming patterns, file organization detected)
     - Architecture notes (dependencies, frameworks, patterns identified)
   - Store as structured YAML in `<project>/.m31a/knowledge/`

2. **Knowledge schema:**
   ```yaml
   session_id: "abc123"
   timestamp: "2026-06-29T10:00:00Z"
   project_type: "go"
   conventions:
     - pattern: "error handling"
       observation: "Uses pkg/errors wrapping"
       confidence: 0.8
   tool_patterns:
     - tool: "bash"
       command_pattern: "go test"
       frequency: 12
       success_rate: 0.92
   failures:
     - error: "undefined: SomeType"
       resolution: "Added missing import"
       tool_used: "edit"
   architecture:
     - "Uses cobra for CLI"
     - "Uses testify for assertions"
     - "Internal packages under internal/"
   ```

3. **Ledger enhancement:**
   - Add `FailureReason` and `ResolutionMethod` fields to ledger entries
   - Extract from self-heal reports what was fixed and how

**Files modified:**
- `pkg/ledger/ledger.go` — enhanced entry schema
- `pkg/session/manager.go` — knowledge extraction at ship
- `pkg/knowledge/` — new package: knowledge store

**Regression risks:**
- Extraction must not slow down ship phase
- Knowledge files must be small (< 10KB per session)
- Must not store sensitive information (API keys, secrets)

**Validation:**
- Unit test: extraction produces valid YAML
- Unit test: extraction captures tool patterns correctly
- Unit test: extraction captures error patterns correctly
- Unit test: knowledge files are under 10KB
- Integration test: full session produces knowledge file

### 4B: Knowledge Storage and Retrieval

**Affected packages:** `pkg/knowledge/` (new), `internal/config/`

**Implementation:**

1. **Knowledge store:**
   - File-based storage: `<project>/.m31a/knowledge/*.yaml`
   - One file per session, named by session ID
   - Indexed by project type, tool patterns, and conventions
   - Retention: keep last 20 sessions, prune older

2. **Retrieval API:**
   - `GetRelevantKnowledge(projectType, goal) []KnowledgeEntry`
   - Fuzzy match on project type and goal keywords
   - Rank by recency and relevance
   - Return top 5 entries

3. **Configuration:**
   ```toml
   [knowledge]
   enabled = true
   max_entries = 20
   extraction_enabled = true
   injection_enabled = true
   ```

**Files modified:**
- `pkg/knowledge/store.go` — new file: knowledge store
- `pkg/knowledge/types.go` — new file: knowledge types
- `pkg/knowledge/retrieval.go` — new file: relevance ranking
- `internal/config/types.go` — KnowledgeConfig
- `internal/config/loader.go` — defaults

**Regression risks:**
- Must not inject knowledge that contradicts current instructions
- Must not store secrets or sensitive data
- Must not slow down session startup significantly

**Validation:**
- Unit test: store persists and loads correctly
- Unit test: retrieval ranks by relevance
- Unit test: retention prunes old entries
- Unit test: no sensitive data stored

### 4C: Knowledge Injection

**Affected packages:** `internal/workflow/`, `internal/tui/`

**Implementation:**

1. **Injection at Initialize phase:**
   - After project detection, load relevant knowledge
   - Inject into system prompt as "Project Learnings" section
   - Format: "From past sessions on similar projects: ..."

2. **Injection at Discuss phase:**
   - When answering questions, reference past patterns
   - "Based on past sessions, this project uses testify for testing."

3. **Knowledge screen:**
   - `/knowledge` command to browse stored knowledge
   - Shows: conventions, tool patterns, recent failures, architecture notes

**Files modified:**
- `internal/workflow/engine.go` — knowledge injection in buildSystemPrompt
- `internal/workflow/initialize.go` — load knowledge after project detection
- `internal/tui/commands/commands.go` — register `/knowledge`
- `internal/tui/knowledge_model.go` — new file: knowledge browser screen

**Regression risks:**
- Knowledge must not override explicit user instructions
- Knowledge must be clearly labeled as "from past sessions"
- Must not inject stale or contradictory knowledge

**Validation:**
- Unit test: knowledge injected into system prompt
- Unit test: knowledge not injected when disabled
- Unit test: `/knowledge` screen shows correct data
- Integration test: second session references past learnings
- Manual: verify knowledge is relevant and accurate

---

## Cross-Cutting Concerns

### Technical Debt (from ROADMAP.md)

The ROADMAP.md identifies these technical debt items targeting v1.6:

| Debt | Severity | Addressed In |
|------|----------|--------------|
| Engine struct decomposition (47+ fields) | Critical | Deferred to v1.7-v1.8 (too large for v1.6) |
| TUI Update dispatch (3544 lines) | High | Deferred to v1.7 (not blocking autonomy) |
| Provider registry race conditions | High | **Should fix in Wave 1** (correctness issue) |
| Documentation drift | Low | Address as part of each wave |

### Testing Strategy

Each wave includes:
- Unit tests for new types and functions
- Integration tests for end-to-end flows
- Regression tests for existing behavior
- Race condition tests with `-race` flag

### Documentation

Each wave updates:
- `ROADMAP.md` — mark completed features
- `CHANGELOG.md` — new version entry
- Inline code comments for new types and interfaces

---

## Wave Dependency Graph

```
Wave 1 (Deferred Tools + Emitter)
  ├── 1A: Emitter Reliability ─────────────┐
  ├── 1B: Batch Permission Approval ───────┤
  └── 1C: Approval Queue UI ───────────────┘
                                            │
Wave 2 (Context Budgeting)                 │
  ├── 2A: Proactive Warning ───────────────┤
  ├── 2B: Adaptive Allocation ─────────────┤
  └── 2C: Budget Visibility ───────────────┘
                                            │
Wave 3 (Ghost Mode)                        │
  ├── 3A: Headless Workflow Engine ─────────┤
  ├── 3B: Progress Display ────────────────┤
  └── 3C: Permission Integration ──────────┤
                                            │
Wave 4 (Knowledge Carryover)               │
  ├── 4A: Knowledge Extraction ────────────┤
  ├── 4B: Storage and Retrieval ───────────┤
  └── 4C: Knowledge Injection ─────────────┘
```

**Dependencies:**
- Wave 2 depends on Wave 1 (context budgeting needs reliable event delivery)
- Wave 3 depends on Wave 1 (ghost mode needs batch permissions)
- Wave 3 depends on Wave 2 (ghost mode needs context budgeting)
- Wave 4 is independent (can start in parallel with Wave 3)

---

## Success Criteria (v1.6)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Batch approval reduces interruptions | Drop from ~3/plan to <1/plan | Measure permission modals per session |
| Context overflow failures | <3% (down from ~20%) | Count context overflow errors in sessions |
| Ghost mode: 10-task plan completion | 80% complete headless | Run 20 headless sessions, measure completion |
| Cross-session knowledge retrieval | >70% relevant conventions surfaced | Manual evaluation of injected knowledge |

---

## Implementation Order

1. **Wave 1A** (Emitter Reliability) — 0.5 days, no dependencies
2. **Wave 1B** (Batch Approval) — 2 days, depends on 1A for reliable delivery
3. **Wave 1C** (Approval Queue UI) — 0.5 days, depends on 1B
4. **Wave 2A** (Proactive Warning) — 0.5 days, no dependencies
5. **Wave 2B** (Adaptive Allocation) — 1.5 days, depends on 2A
6. **Wave 2C** (Budget Visibility) — 0.5 days, depends on 2B
7. **Wave 3A** (Headless Engine) — 2.5 days, depends on 1B + 2B
8. **Wave 3B** (Progress Display) — 1 day, depends on 3A
9. **Wave 3C** (Permission Integration) — 1 day, depends on 3A + 1B
10. **Wave 4A** (Knowledge Extraction) — 1.5 days, independent
11. **Wave 4B** (Storage and Retrieval) — 1 day, depends on 4A
12. **Wave 4C** (Knowledge Injection) — 1.5 days, depends on 4B

**Total estimated effort:** 14-18 days

---

## What NOT to Do

- Do not decompose the Engine struct (v1.7-v1.8 scope)
- Do not rewrite the TUI Update dispatch (v1.7 scope)
- Do not add new workflow phases
- Do not change the permission system's fundamental architecture
- Do not add features not in the v1.6 roadmap
- Do not change appearance or rendering behavior
