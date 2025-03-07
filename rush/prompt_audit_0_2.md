# ROLE

You are the **CTO and Principal Architect** conducting a comprehensive audit of M31A's
implementation across Phases 0, 1, and 2. You have deep knowledge of the project spec
(Adrenaline/idea.md), reference doc (Adrenaline/REFERENCE.md), roadmap (Adrenaline/ROADMAP.md),
and all walkthroughs produced so far.

Your job is to identify EVERY deviation, missing deliverable, architectural inconsistency,
and potential downstream breakage BEFORE Phase 3 begins. You are not implementing code —
you are auditing what exists against what was specified.

---

# SCOPE

Audit these completed phases:
- **Phase 0**: Foundation & Documentation (walkthrough_0.md)
- **Phase 1**: Provider Abstraction Layer (walkthrough_1.md)
- **Phase 2**: TUI Foundation (walkthrough_2.md)

Cross-reference against:
- `Adrenaline/idea.md` — Full V1 specification
- `Adrenaline/REFERENCE.md` — Condensed overview
- `Adrenaline/ROADMAP.md` — Implementation roadmap with acceptance criteria
- `rush/prompt_0.md`, `rush/prompt_1.md`, `rush/prompt_2.md` — Original prompts
- `walkthrough_0.md`, `walkthrough_1.md`, `walkthrough_2.md` — Actual outputs

---

# AUDIT METHODOLOGY

For each phase, systematically check:

1. **Deliverable completeness** — Every file listed in the prompt's DELIVERABLES section must exist.
2. **Spec compliance** — Every requirement from idea.md sections relevant to that phase must be met.
3. **Interface consistency** — Types defined in Phase 0 must match what Phase 1 and 2 actually use.
4. **Walkthrough honesty** — Walkthroughs must accurately reflect what was built, not what was promised.
5. **Build integrity** — Binary compiles, tests pass, vet clean, no race conditions.
6. **Architectural soundness** — Package boundaries respected, no circular deps, no leaks between layers.
7. **Deviation impact** — Each deviation must be assessed: does it block Phase 3, require fixing, or is it acceptable?

---

# AUDIT AREAS

## Area 1: Project Structure & Build System

Check:
- Does the directory structure match the roadmap's package layout?
- Does `go.mod` have the correct module path and Go version?
- Does the Makefile have all required targets (build, test, lint, vet, clean, release)?
- Does `.gitignore` cover all required patterns?
- Does CI workflow (`.github/workflows/ci.yml`) match the roadmap spec (lint, test, build matrix, release job)?
- Does `.goreleaser.yaml` cover all platforms/arch combos?
- Does `.golangci.yml` have the required linters enabled?
- Is LICENSE unmodified MIT text?
- Is the binary statically linked with CGO_ENABLED=0?
- Is the binary stripped (`-s -w` ldflags)?

## Area 2: Types & Interfaces (Phase 0)

Check against `internal/types/types.go` and `internal/provider/interface.go`:
- Does `LLMProvider` interface match the spec (Name, FetchModels, ChatCompletionStream, EstimateCost, HealthCheck, GetModel)?
- Does `Tool` interface match the spec (Name, Description, RiskLevel, Execute)?
- Does `Message` struct have all required fields (Role, Content, Segments, ToolCalls, Usage, CreatedAt)?
- Does `MessageSegment` have Type, Content, DurationMs, Visible?
- Does `Task` struct have all fields (ID, Description, Action, Dependencies, Files, AcceptanceCriteria, Status, HealsAttempted, PredictedFiles, CommitHash)?
- Does `ModelInfo` have all fields (ID, Provider, Name, Description, ContextLength, Pricing, Architecture, TopProvider, Capabilities)?
- Does `Config` struct match the config.toml schema from the spec?
- Are all sentinel errors defined in `internal/errors/errors.go`?
- Are all constants defined in `internal/types/constants.go`?
- Are there any types that were promised in the spec but missing?

## Area 3: Provider Layer (Phase 1)

Check against `internal/provider/`:
- Does OpenRouter client implement all LLMProvider methods?
- Does Zen client implement all LLMProvider methods?
- Does the model cache have TTL (5min) and stale fallback (24h)?
- Does the SSE parser handle multi-line data, event types, [DONE] sentinel?
- Does reasoning normalization cover all 4 model families (deepseek, openai-o, anthropic, qwen)?
- Does the provider registry support Register, Active, SetActive, Get, List?
- Does auto-fallback trigger on 429 and 503?
- Does HealthCheck classify live/slow/offline correctly?
- Are all 40 tests passing with -race?
- Is `EstimateCost` a known issue (always returns 0 because interface lacks model ID)?
- Does FetchModels check cache before hitting API?

## Area 4: TUI Foundation (Phase 2)

Check against `internal/tui/`:
- Does AppState implement tea.Model correctly (Init, Update, View)?
- Does screen routing work (FirstRun → REPL, placeholder screens)?
- Does the theme system have both dark and light palettes matching spec hex values?
- Does the header render as a single line with: brand, provider badge, model name, context bar, health status?
- **CRITICAL: Is the provider badge `[OR]`/`[ZEN]` as specified, or `[OP]`/`[ZE]` as implemented?**
- Does the context bar color-shift at 80% and 95% thresholds?
- Does the REPL have 4 regions (header, messages, input, status)?
- Does the input use bubbles/textarea with placeholder, enter=send, shift+enter=newline?
- Does the health check ticker run at 60s interval with adaptive backoff?
- Does the first-run screen validate API keys via HealthCheck before transitioning?
- Does the first-run screen have the 6-state flow (Welcome → ProviderSelect → KeyInput → Validating → KeychainPrompt → Complete)?
- Are all 110+ tests passing with -race?
- Is the binary stripped?
- Does Go version match CI requirement (1.22)?

## Area 5: Cross-Phase Consistency

Check:
- Do Phase 1 and Phase 2 use the same types from `internal/types/`? No duplicate definitions?
- Does `internal/provider/interface.go` from Phase 0 match what Phase 1 actually implemented?
- Does the `ChatRequest` type live in `internal/provider/` (where Phase 1 moved it) and is it used consistently?
- Are there any import cycles between packages?
- Does the logger from Phase 0 get used anywhere in Phase 1 or 2?
- Are sentinel errors used consistently (not replaced with fmt.Errorf)?
- Is there any code in Phase 1 or 2 that violates the "no implementation code in Phase 0" boundary?

## Area 6: Documentation

Check:
- Does `AGENTS.md` contain all required sections (Project Overview, Build Commands, Architecture Rules, Package Layout, Prohibitions, File Paths, Dependencies)?
- Does `docs/ARCHITECTURE.md` cover dependency graph, data flows, threading, context pruning?
- Does `docs/INTERFACES.md` mirror the actual Go interfaces from `internal/` files?
- Does `docs/TYPES.md` document env vars, constants, errors, enums?
- Do `.planning/` artifacts exist (PROJECT.md, REQUIREMENTS.md, STATE.md, CONTEXT.md)?
- Is `.planning/REQUIREMENTS.md` fully expanded (all 26 + 6 acceptance criteria, no shortcuts)?
- Is `opencode.json` correct (instruction file chain)?

## Area 7: Walkthrough Accuracy

Check each walkthrough:
- Do the checkboxes accurately reflect what was built?
- Is the build output real (not placeholder text)?
- Are deviations honestly reported?
- Are there any items marked [x] that are actually incomplete or partially implemented?
- Do the walkthroughs reference actual file paths that exist?

## Area 8: Spec Drift Analysis

Compare the cumulative state of the codebase against the roadmap's acceptance criteria (idea.md §15 and §17):

For each of the 26 core + 6 differentiator criteria, assess:
- **Implemented** — Fully working
- **Partial** — Some pieces exist, others missing
- **Not started** — Belongs to a future phase
- **Deviation** — Implemented differently than spec

Focus on criteria that Phases 0-2 should have addressed:
1. Binary builds (CGO_ENABLED=0)
2. Dual-provider discovery
3. Provider selector (deferred to Phase 7)
4. Provider switching
5. Inline thinking (deferred to Phase 3)
6. Streaming UX (deferred to Phase 3)
7. Tool cards (deferred to Phase 3)
8-26. Various future phases

---

# OUTPUT FORMAT

Produce a single markdown document with this structure:

```markdown
# M31A — Phase 0-2 Audit Report

## Executive Summary
[2-3 sentences: overall health, readiness for Phase 3, critical findings]

## Audit Scorecard

| Area | Status | Notes |
|------|--------|-------|
| Project Structure & Build | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Types & Interfaces | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Provider Layer | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| TUI Foundation | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Cross-Phase Consistency | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Documentation | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Walkthrough Accuracy | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |
| Spec Drift | ✅ PASS / ⚠️ WARN / ❌ FAIL | [brief note] |

## Critical Findings (Must Fix Before Phase 3)

### [Finding 1: Title]
- **Area**: [which audit area]
- **Severity**: CRITICAL
- **Description**: [what's wrong]
- **Spec Reference**: [idea.md section or roadmap task]
- **Impact**: [what breaks if not fixed]
- **Fix**: [specific action to take]

[Continue for all CRITICAL findings]

## Warnings (Should Fix, Can Parallelize with Phase 3)

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
| 1 | 1 | EstimateCost always returns 0 | Cost display shows $0.00 | Known, deferred |
| 2 | 1 | FetchModels always hits API (no cache check) | Extra API calls | Minor |
| 3 | 2 | Provider badge [OP]/[ZE] instead of [OR]/[ZEN] | User-facing, spec violation | Must fix |
[Continue for all deviations found across walkthroughs]

## Acceptance Criteria Status

For each of the 32 acceptance criteria (26 core + 6 differentiator):

| # | Criterion | Status | Phase |
|---|-----------|--------|-------|
| 1 | Binary builds | ✅ Implemented | Phase 0 |
| 2 | Dual-provider discovery | ✅ Implemented | Phase 1 |
| 3 | Provider selector | ⏳ Not started | Phase 7 |
[Continue for all 32]

## Phase 3 Readiness

[Clear GO / NO-GO verdict with conditions]

**GO conditions met:** [list]
**NO-GO conditions:** [list any blockers]

## Recommended Phase 3 Prompt Adjustments

[Based on audit findings, suggest any changes to the Phase 3 prompt to account for
deviations, missing pieces, or architectural changes discovered during audit.]
```

---

# EXECUTION INSTRUCTIONS

1. Read ALL source files:
   - `Adrenaline/idea.md`
   - `Adrenaline/REFERENCE.md`
   - `Adrenaline/ROADMAP.md`
   - `walkthrough_0.md`, `walkthrough_1.md`, `walkthrough_2.md`
   - All Go files in `internal/` and `pkg/`
   - All config files (`.github/`, `Makefile`, `.gitignore`, etc.)
   - All docs/ files
   - All `.planning/` files

2. For each audit area, verify by actually reading the files — do NOT assume the walkthrough is accurate.

3. Be thorough but pragmatic. A deviation is only worth flagging if it has real impact.
   Cosmetic differences (variable names, file organization within a package) are not deviations.

4. Produce the audit report as `rush/audit_0_2.md`.

5. Do NOT modify any code files. This is a read-only audit.

---

# HARD CONSTRAINTS

- Do NOT modify any code files
- Do NOT modify any walkthrough files
- Do NOT skip audit areas
- Do NOT assume walkthrough accuracy — verify by reading source files
- Do NOT flag cosmetic differences as deviations
- Do NOT flag future-phase features as missing (only flag what the current phase should have delivered)
- The audit report must be produced as `rush/audit_0_2.md`
