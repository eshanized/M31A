# ROLE

You are the **CTO and Principal Architect** conducting a focused audit of M31A's
implementation across Phase 3 (Message Rendering Pipeline) and Phase 4 (Tool System).
You have deep knowledge of the project spec (Adrenaline/idea.md), reference doc
(Adrenaline/REFERENCE.md), roadmap (Adrenaline/ROADMAP.md), and all walkthroughs.

Your job is to verify that Phases 3 and 4 were implemented correctly, identify any
deviations or inconsistencies, and confirm readiness for Phase 5. You are NOT auditing
Phases 0-2 (those were audited previously) — focus ONLY on Phase 3 and Phase 4.

---

# SCOPE

Audit these completed phases:
- **Phase 3**: Message Rendering Pipeline (walkthrough_3.md)
- **Phase 4**: Tool System (walkthrough_4.md)

Cross-reference against:
- `Adrenaline/idea.md` — Full V1 specification (sections 5, 6, 8, 13 relevant to these phases)
- `Adrenaline/REFERENCE.md` — Condensed overview
- `Adrenaline/ROADMAP.md` — Phase 3 and Phase 4 task descriptions
- `rush/prompt_3.md`, `rush/prompt_4.md` — Original prompts
- `walkthrough_3.md`, `walkthrough_4.md` — Actual outputs
- `rush/audit_0_2.md` — Previous audit report (for context on existing patterns)

---

# AUDIT METHODOLOGY

For each phase, systematically check:

1. **Deliverable completeness** — Every file listed in the prompt's DELIVERABLES section must exist.
2. **Spec compliance** — Every requirement from idea.md sections relevant to that phase must be met.
3. **Interface consistency** — Types defined in Phase 0 must match what Phase 3/4 actually use.
4. **Walkthrough honesty** — Walkthroughs must accurately reflect what was built, not what was promised.
5. **Build integrity** — Binary compiles, tests pass, vet clean, no race conditions.
6. **Architectural soundness** — Package boundaries respected, no circular deps, no leaks between layers.
7. **Deviation impact** — Each deviation assessed: blocks Phase 5, requires fixing, or acceptable?

---

# AUDIT AREAS

## Area 1: Message Rendering Pipeline (Phase 3)

### P3.1 — Streaming Token Renderer
- [ ] StreamMsg, StreamDoneMsg, StreamErrorMsg defined in streaming.go
- [ ] StartStreamCmd spawns goroutine, calls iterator.Next(), sends chunks via channel
- [ ] Chunks sent as StreamMsg (not directly mutating state)
- [ ] StreamDoneMsg emits final assembled message with all segments + usage
- [ ] StreamErrorMsg on network/provider/context errors
- [ ] Context cancellation closes iterator cleanly
- [ ] StreamTickCmd emits at ~10fps

### P3.2 — Inline Thinking/Reasoning Blocks
- [ ] ThinkingBlock with segment, theme, expanded, startedAt
- [ ] Expanded render: header + border + content in TextSecondary
- [ ] Collapsed render: header only with [▼]
- [ ] Toggle works correctly
- [ ] Duration format: N.Ns (<10s), NN.Ns (<60s), Mm Ns (>=60s)
- [ ] Finalized segments use DurationMs from segment
- [ ] Header truncates with ... when too wide

### P3.3 — Rich Tool Cards
- [ ] ToolCard with toolName, input, output, state, theme
- [ ] Per-tool colors: Bash=Warning, FileRead=Thinking, FileWrite=Brand, Glob=TextSecondary, Grep=Thinking
- [ ] Status badges: [..] running, [OK] success, [ERR] error
- [ ] Auto-collapse > 20 lines
- [ ] Output cap at MaxToolOutputChars (10,000) with truncation notice
- [ ] Binary detection: [binary file, N bytes]
- [ ] Toggle collapses/expands
- [ ] Card layout: header → input → output → status

### P3.4 — Markdown Rendering
- [ ] MessageRenderer uses Glamour TermRenderer
- [ ] Dark/light theme styles matched
- [ ] Renderer cached
- [ ] Width = terminal width - padding
- [ ] User messages: right-aligned, plain text, max 70% width

### P3.5 — Permission Modal
- [ ] PermissionModal with request, theme, elapsed, timeout
- [ ] Centered overlay with dimmed background
- [ ] Tool name + risk color (Dangerous/Destructive = red)
- [ ] Command preview in bordered box
- [ ] Key bindings: Y/Enter=Allow, A=AllowAlways, N/Esc=Deny, E=Exit
- [ ] Auto-deny countdown (N:NN format)
- [ ] Allow/AllowAlways/Deny return PermissionResponse

### P3.6 — REPL Integration
- [ ] ReplModel extended with streaming state fields
- [ ] Update() handles StreamMsg, StreamDoneMsg, StreamErrorMsg, TickMsg
- [ ] View() uses msgRenderer.RenderMessage()
- [ ] During streaming: renders in-progress message
- [ ] Thinking blocks update with live duration
- [ ] Tool cards inline between segments
- [ ] Textarea disabled during streaming, enabled on completion
- [ ] ctrl+c cancels streaming
- [ ] Auto-scroll to bottom

## Area 2: Tool System (Phase 4)

### P4.1 — Bash Tool
- [ ] Bash struct with workDir, output buffer, mutex
- [ ] exec.CommandContext with bash -c (or cmd /C on Windows)
- [ ] 30-min timeout (types.BashTimeout)
- [ ] Signal forwarding on cancellation (process group)
- [ ] stdout/stderr read concurrently
- [ ] Output cap at BashOutputLimit (50,000)
- [ ] Binary detection (null byte scan)
- [ ] Error handling: not found, timeout, non-zero exit, permission denied

### P4.2 — FileRead Tool
- [ ] Path resolution + workDir check
- [ ] Symlink resolution (EvalSymlinks)
- [ ] Size check (stat, reject > 5MB)
- [ ] Binary detection (null bytes + DetectContentType)
- [ ] Text: full content; Binary: [binary file, mime, N bytes]
- [ ] Error handling: not found, permission denied, too large

### P4.3 — FileWrite Tool
- [ ] Path resolution + workDir check
- [ ] Backup before overwrite (backupDir/sanitized.timestamp)
- [ ] Atomic write: temp file → fsync → rename
- [ ] Directory creation (MkdirAll)
- [ ] Binary content rejection
- [ ] Cleanup on failure (defer remove temp)

### P4.4 — Glob Tool
- [ ] doublestar.Glob for ** support
- [ ] Sorted output with size + modified
- [ ] Table format
- [ ] 1,000 result hard limit
- [ ] rg integration (when .gitignore present)
- [ ] Fallback to doublestar

### P4.5 — Grep Tool
- [ ] rg check at construction (hasRg flag)
- [ ] rg execution: --json --line-number --max-count
- [ ] Pure-Go fallback: Walk + regexp + Scanner
- [ ] Glob filter via doublestar.Match
- [ ] Binary file skipping
- [ ] .gitignore support (automatic with rg, manual in fallback)
- [ ] Output format: file:line: match_text
- [ ] Max results cap

### P4.6 — Dispatcher
- [ ] tools map + permissions map + request/response channels
- [ ] Register: adds to map, panics on duplicate
- [ ] Execute: lookup by name, check risk level
- [ ] Safe/Medium: immediate execution
- [ ] Dangerous/Destructive: check remembered permissions, then request
- [ ] Permission flow: send to requestCh, wait on responseCh
- [ ] ErrPermissionDenied on denial
- [ ] ApprovePermission sends response

## Area 3: Cross-Phase Consistency (Phase 3 ↔ Phase 4)

Check:
- Do tool cards (Phase 3) render ToolCall/ToolResult types that match what tools (Phase 4) produce?
- Does the permission modal (Phase 3) integrate with the dispatcher's PermissionRequest/PermissionResponse (Phase 4)?
- Are sentinel errors from `internal/errors/` used consistently across both phases?
- Does the streaming.go integration support tool call deltas that tools produce?
- Are there any type mismatches between Phase 3's component APIs and Phase 4's tool outputs?
- Does `types.ToolInput` match the parameter format tools expect?
- Does `types.ToolResult` match what the dispatcher returns and what tool cards render?

## Area 4: Spec Drift Analysis

Compare against idea.md sections relevant to these phases:

**Phase 3 (idea.md §5 — Inline Thinking & Reasoning, §6.2 — Tool Call Rendering, §8.1 — REPL Screen):**
- Inline thinking blocks with collapsible rendering
- Tool cards with color-coded badges
- Streaming token-by-token without flicker
- Permission modal with timeout

**Phase 4 (idea.md §6.1 — Implemented Tools, §6.3 — Subagent Safety):**
- 5 V1 tools: Bash, FileRead, FileWrite, Glob, Grep
- Risk levels: Safe, Medium, Dangerous, Destructive
- Permission gate for Dangerous/Destructive tools
- Atomic writes, binary detection, path safety

For each relevant spec item, assess:
- **Implemented** — Fully working
- **Partial** — Some pieces exist, others missing
- **Not started** — Belongs to future phase
- **Deviation** — Implemented differently than spec

## Area 5: Walkthrough Accuracy

For each walkthrough (3 and 4):
- Do the checkboxes accurately reflect what was built?
- Is the test output real (not placeholder)?
- Are deviations honestly reported?
- Are there items marked [x] that are actually incomplete?
- Do walkthrough file paths actually exist?

## Area 6: Build & Test Integrity

Check:
- `go build ./...` passes
- `go vet ./...` passes
- `go test -race ./internal/tui/...` passes (Phase 3)
- `go test -race ./internal/tools/...` passes (Phase 4)
- Binary is statically linked (CGO_ENABLED=0)
- No new external dependencies beyond what's approved in AGENTS.md
- `go.mod` still at `go 1.22`

---

# OUTPUT FORMAT

Produce a single markdown document with this structure:

```markdown
# M31A — Phase 3-4 Audit Report

## Executive Summary
[2-3 sentences: overall health, readiness for Phase 5, critical findings]

## Audit Scorecard

| Area | Status | Notes |
|------|--------|-------|
| Message Rendering Pipeline | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Tool System | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Cross-Phase Consistency | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Spec Drift | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Walkthrough Accuracy | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Build & Test Integrity | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |

## Critical Findings (Must Fix Before Phase 5)

### [Finding 1: Title]
- **Area**: [which audit area]
- **Severity**: CRITICAL
- **Description**: [what's wrong]
- **Spec Reference**: [idea.md section or roadmap task]
- **Impact**: [what breaks if not fixed]
- **Fix**: [specific action]

[Continue for all CRITICAL findings]

## Warnings (Should Fix, Can Parallelize with Phase 5)

### [Warning 1: Title]
- **Area**: [which audit area]
- **Severity**: MEDIUM
- **Description**: [what's suboptimal]
- **Spec Reference**: [idea.md section or roadmap task]
- **Impact**: [what could go wrong]
- **Fix**: [specific action]

[Continue for all MEDIUM findings]

## Informational (No Action Required)

### [Note 1: Title]
- **Area**: [which audit area]
- **Severity**: LOW
- **Description**: [observation, improvement suggestion]

[Continue for all LOW findings]

## Deviation Register

| # | Phase | Deviation | Impact | Status |
|---|-------|-----------|--------|--------|
| 1 | 3 | [deviation description] | [impact] | [must fix / acceptable / deferred] |
| 2 | 4 | [deviation description] | [impact] | [must fix / acceptable / deferred] |
[Continue for all deviations]

## Cross-Phase Type Consistency

| Type | Phase 3 Usage | Phase 4 Usage | Consistent? |
|------|--------------|---------------|-------------|
| types.ToolCall | [how used] | [how used] | ✅/❌ |
| types.ToolResult | [how used] | [how used] | ✅/❌ |
| types.ToolInput | [how used] | [how used] | ✅/❌ |
| types.RiskLevel | [how used] | [how used] | ✅/❌ |
| tools.PermissionRequest | [how used] | [how used] | ✅/❌ |
| tools.PermissionResponse | [how used] | [how used] | ✅/❌ |

## Walkthrough Accuracy Check

| Walkthrough | Claim | Actual | Accurate? |
|-------------|-------|--------|-----------|
| walkthrough_3.md | [claim] | [actual] | ✅/❌ |
| walkthrough_4.md | [claim] | [actual] | ✅/❌ |

## Phase 5 Readiness

[Clear GO / NO-GO verdict with conditions]

**GO conditions met:** [list]
**NO-GO conditions:** [list any blockers]

## Recommended Phase 5 Prompt Adjustments

[Based on audit findings, suggest any changes to the Phase 5 prompt to account for
deviations, missing pieces, or architectural changes discovered during audit.]
```

---

# EXECUTION INSTRUCTIONS

1. Read ALL relevant source files:
   - `Adrenaline/idea.md` (sections 5, 6, 8, 13)
   - `Adrenaline/REFERENCE.md`
   - `Adrenaline/ROADMAP.md` (Phase 3, Phase 4 sections)
   - `walkthrough_3.md`, `walkthrough_4.md`
   - All Go files in `internal/tui/` and `internal/tui/components/`
   - All Go files in `internal/tools/`
   - `internal/types/types.go`, `internal/types/constants.go`
   - `internal/tools/interface.go`
   - `internal/errors/errors.go`
   - `go.mod`, `AGENTS.md`

2. For each audit area, verify by actually reading the files — do NOT assume the walkthrough is accurate.

3. Be thorough but pragmatic. A deviation is only worth flagging if it has real impact.

4. Produce the audit report as `rush/audit_3_4.md`.

5. Do NOT modify any code files. This is a read-only audit.

---

# HARD CONSTRAINTS

- Do NOT modify any code files
- Do NOT modify any walkthrough files
- Do NOT skip audit areas
- Do NOT assume walkthrough accuracy — verify by reading source files
- Do NOT flag cosmetic differences as deviations
- Do NOT flag future-phase features as missing
- The audit report must be produced as `rush/audit_3_4.md`
- Focus ONLY on Phase 3 and Phase 4 — do NOT re-audit Phases 0-2
