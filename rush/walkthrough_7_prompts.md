# Walkthrough — Master Prompt System Redesign

## Summary

Replaced the hardcoded 4-line `const systemPrompt` in `engine.go` with a file-based prompt system using `//go:embed` and phase-specific composition. 7 prompt files loaded at compile time, composed per-phase at runtime.

---

## Files Created

| File | Lines | Purpose |
|------|-------|---------|
| `internal/workflow/prompts/base.md` | 58 | Core identity, workflow phases, principles, code style, communication |
| `internal/workflow/prompts/tool-use.md` | 50 | Detailed Bash/FileRead/FileWrite/Glob/Grep instructions |
| `internal/workflow/prompts/plan-format.md` | 51 | Plan phase JSON schema, field descriptions, rules, example |
| `internal/workflow/prompts/execute-task.md` | 40 | Execute phase: read deps, plan, implement, build, commit |
| `internal/workflow/prompts/discuss-questions.md` | 26 | Discuss phase: ask 2-4 clarifying questions, format guidelines |
| `internal/workflow/prompts/self-heal.md` | 31 | Self-heal: diagnostic process, when to admit defeat |
| `internal/workflow/prompts/verify-checklist.md` | 30 | Verify phase: file existence, syntax, tests, criteria checklist |
| **Total** | **286** | |

## Files Modified

| File | Lines Changed | Description |
|------|---------------|-------------|
| `internal/workflow/engine.go` | +62/-4 | Added embed directive, PromptRegistry, LoadPrompts(), buildSystemPrompt(), prompts field, NewEngine update |
| `internal/workflow/discuss.go` | +1/-1 | buildDiscussContext uses buildSystemPrompt(e.prompts.Discuss) |
| `internal/workflow/plan.go` | +1/-1 | buildPlanContext uses buildSystemPrompt(e.prompts.ToolUse, e.prompts.PlanFormat) |
| `internal/workflow/execute.go` | +2/-2 | buildExecuteContext + healTask use buildSystemPrompt |
| `internal/workflow/engine_test.go` | +93/-2 | Fixed ContextPruning test, added 3 new tests |

---

## Prompt Composition Matrix

| Phase | System Prompt Composition | LLM Call? |
|-------|--------------------------|-----------|
| Initialize | (no LLM call) | No |
| Discuss | `base.md` + `discuss-questions.md` | Yes |
| Plan | `base.md` + `tool-use.md` + `plan-format.md` | Yes |
| Execute | `base.md` + `tool-use.md` + `execute-task.md` | Yes |
| Heal | `base.md` + `self-heal.md` | Yes (via healTask) |
| Verify | (no direct LLM) | No |
| Ship | (no LLM call) | No |

---

## Prompt Content Verification

### `base.md` (58 lines)
- Identity: "M31A, a terminal AI coding assistant"
- 6 workflow phases listed
- Core principles: clean code, read before write, atomic commits, self-heal
- Tool philosophy summary
- Code style: gofmt, go vet, small functions, error handling
- Communication: concise, show reasoning, report results

### `tool-use.md` (50 lines)
- Bash: timeout, output limit, working dir, dangerous commands
- FileRead: max size, binary detection, path resolution
- FileWrite: atomic writes, backup, read-first rule
- Glob: patterns, recursive matching, prefer over find
- Grep: regex, path limiting, prefer over grep
- General rules: one call at a time, diagnose failures

### `plan-format.md` (51 lines)
- JSON array only output, no markdown fences
- Task schema: id, action, description, dependencies, files, acceptance_criteria
- 6 rules: no cycles, no self-refs, valid deps, unique IDs, non-empty fields, dependency order
- Full JSON example with 2 tasks

### `execute-task.md` (40 lines)
- 6-step execution process
- Read deps first, plan approach, read existing files
- Implement with FileWrite, build with Bash
- Commit with `feat(task <id>): <description>`
- File-first, atomic commits, self-heal awareness

### `discuss-questions.md` (26 lines)
- Ask 2-4 clarifying questions
- Specific, actionable, "how/what" not "yes/no"
- Number sequentially
- Skip if goal is trivial

### `self-heal.md` (31 lines)
- 6-step diagnostic process
- Read error, check file state, identify root cause
- Plan fix, apply fix, verify fix
- Admit defeat after 2 failed attempts
- Bisect context if available

### `verify-checklist.md` (30 lines)
- 4 checks: file existence, syntax, tests, acceptance criteria
- Report specific failures
- Recommend self-heal or mark unrecoverable
- Output format with checkmark/cross per check

---

## Build Integrity

```
go mod tidy          — OK (no changes needed)
go build -o m31a     — OK (CGO_ENABLED=0, embed directive resolved)
go vet ./...         — OK (no issues)
```

---

## Test Results

```
go test -race -count=1 ./...  — ALL PASS

Internal workflow package: 19 tests pass
  - 16 existing tests (all pass, 1 updated assertion)
  - 3 new tests:
    - TestPromptRegistry_LoadPrompts
    - TestEngine_BuildSystemPrompt
    - TestEngine_PhasePromptComposition
```

---

## Deviations from Plan

None. Implementation followed the approved plan exactly.
