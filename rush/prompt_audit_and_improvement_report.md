# M31A — Hardcoded LLM Prompt Audit & Improvement Report

> **Scope:** All Go source files and embedded Markdown files in the M31A codebase that contain
> text ultimately sent to an LLM, or text that shapes LLM behaviour (tool descriptions, parameter
> schemas, user-message templates). Test-only fixtures are flagged separately.

---

## 1. Taxonomy of Prompt Locations

The codebase uses **three distinct mechanisms** to inject text into LLM context:

| # | Mechanism | Location | Managed? |
|---|-----------|----------|----------|
| A | Embedded `.md` files (`//go:embed`) | `internal/workflow/prompts/` | ✅ Centralized |
| B | Inline Go string literals / `fmt.Sprintf` templates | `internal/workflow/*.go` | ❌ Scattered |
| C | Tool `Description()` + `ParameterSchema()` methods | `internal/tools/*.go` | ❌ Hardcoded |

---

## 2. Category A — Embedded Markdown Prompts (`internal/workflow/prompts/`)

All seven files are loaded via `//go:embed prompts/*.md` in
`engine.go` (lines 26–60) and registered into a `PromptRegistry` struct.
`buildSystemPrompt()` (line 466) concatenates `Base` + optional extras with an `---` separator.

### 2.1 `base.md` — Universal System Prompt
**File:** `internal/workflow/prompts/base.md`
**Injected in:** Every phase (always the first item in `buildSystemPrompt`)
**Content summary:**

- Identity: "You are M31A, a terminal AI coding assistant."
- Describes 6-phase workflow by name only (no detail)
- Core principles: 7 bullet rules (read-before-write, atomic commits, self-heal, etc.)
- Tool philosophy: describes 5 tools (Bash, FileRead, FileWrite, Glob, Grep) — **but does NOT mention Edit, WebFetch, TodoWrite, AskUserQuestion**
- Code style: Go conventions, function size, error handling, comments
- Communication: conciseness, reasoning visibility

**Issues:**
- Tool list (`Bash, FileRead, FileWrite, Glob, Grep`) is **stale** — 4 additional tools are registered but not listed.
- Phase descriptions in base are placeholder-level; no hint of what context is expected in each phase.
- Hardcodes Go-only guidance even though the engine is language-agnostic by design.

---

### 2.2 `tool-use.md` — Tool Reference Card
**File:** `internal/workflow/prompts/tool-use.md`
**Injected in:** Plan phase + Execute phase (via `buildSystemPrompt(e.prompts.ToolUse, ...)`)
**Content summary:**

Describes 5 tools (Bash, FileRead, FileWrite, Glob, Grep) with:
- Use-cases, constraints (timeout, size limits), danger notes
- "General Rules" — one tool at a time, diagnose before retrying, no chaining destructive calls

**Issues:**
- **Missing tools:** `Edit`, `WebFetch`, `TodoWrite`, `AskUserQuestion` are registered but not described.
- Timeout value `30 minutes` in text; actual constant is `BashTimeout = 1800s` (30 min) — consistent, but fragile if the constant changes without updating this file.
- FileWrite description says "provides complete file content, not just diffs" — contradicts the presence of the `Edit` tool (targeted diffs).
- "One tool call at a time" — this is actually enforced by the sequential dispatcher, but stating it as a soft rule may cause the LLM to under-utilise chaining opportunities.

---

### 2.3 `plan-format.md` — Plan Phase Schema
**File:** `internal/workflow/prompts/plan-format.md`
**Injected in:** Plan phase (`buildSystemPrompt(e.prompts.ToolUse, e.prompts.PlanFormat)`)
**Content summary:**

- Strict instruction: output ONLY a JSON array, no markdown fences, no extra text.
- Task schema: `id`, `action`, `description`, `dependencies`, `files`, `acceptance_criteria`.
- Dependency rules (no cycles, no self-ref, all IDs valid).
- Example JSON block.

**Issues:**
- The schema in the prompt is **not derived from code** — it is typed by hand. If `internal/types/task.go` evolves, this file must be updated manually.
- The `action` enum (`Create/Add/Modify/Delete`) is duplicated in:
  1. `plan-format.md`
  2. `buildPlanContext()` inline string (line 170 of `plan.go`) — **identical instruction appears twice in the same message**.
- No mention of the `status` field (set by engine post-parse); could confuse a model that reads prior TASKS.md.

---

### 2.4 `discuss-questions.md` — Discuss Phase Persona
**File:** `internal/workflow/prompts/discuss-questions.md`
**Injected in:** Discuss phase (`buildSystemPrompt(e.prompts.Discuss)`)
**Content summary:**

- Instructions to ask 2–4 numbered clarifying questions.
- Format examples showing numbered question style.
- "When to Skip" section.

**Issues:**
- The user message in `buildDiscussContext()` (line 128 of `discuss.go`) **repeats** the same instruction:
  `"Ask 2-4 clarifying questions to understand the requirements better. Number each question. Be specific and concise."`
  This duplication could confuse the model about which authority to follow.
- No guidance on how to handle existing Q&A from `PROJECT.md` (answers may already exist).

---

### 2.5 `execute-task.md` — Execute Phase Task Instructions
**File:** `internal/workflow/prompts/execute-task.md`
**Injected in:** Execute phase (`buildSystemPrompt(e.prompts.ToolUse, e.prompts.ExecuteTask)`)
**Content summary:**

- Read dependency outputs first.
- Plan before acting.
- Read existing files.
- Implement, build, test.
- Commit with `feat(task <id>): <description>` format.
- Key reminders (file-first, atomic commits, self-heal awareness, tool discipline).

**Issues:**
- Commit format `feat(task <id>): <description>` is **hardcoded here** but the actual prefix is read from `cfg.Git.CommitPrefix` (defaults to `"feat"`) and used in `execute.go` line 273. These can diverge.
- No mention of `Edit` tool as an option for targeted changes (only FileWrite is implied by "One file at a time").
- "Self-heal awareness" section says "you'll get a chance to fix it" without explaining what the LLM should do differently in a heal loop (the `self-heal.md` prompt handles that, but the model doesn't know it will be re-invoked).

---

### 2.6 `self-heal.md` — Self-Heal Loop Instructions
**File:** `internal/workflow/prompts/self-heal.md`
**Injected in:** `healTask()` in `execute.go` (line 338) and `HealTask()` in `engine.go` (line 337)
**Content summary:**

- 6-step diagnostic process (read error, check file state, identify root cause, plan fix, apply, verify).
- "When to Admit Defeat" — after 2 failed attempts, say so explicitly.
- "Bisect Context" section (optional).

**Issues:**
- "After 2 failed heal attempts" — the actual constant is `MaxHealAttempts` (from `internal/types`). The prompt hardcodes `2`, which will break silently if the constant changes.
- The heal user-message is composed inline in `execute.go` line 339:
  `fmt.Sprintf("Task %d failed: %s\n\nCurrent file state:\n%s\n\nFix the issue and use tools to apply the fix.", ...)`
  This inline instruction partially overlaps with `self-heal.md` and is not part of the prompt registry.
- No guidance on which tools to prefer for healing (should use `Edit` for targeted fixes).

---

### 2.7 `verify-checklist.md` — DEAD PROMPT (Never Injected)
**File:** `internal/workflow/prompts/verify-checklist.md`
**Injected in:** **NOWHERE**

> **WARNING:** `verify-checklist.md` is **never injected** into any LLM request. The `runVerify()` function
> in `verify.go` performs verification programmatically (file stat, go build, go test) and only
> calls `healTask()` with the `self-heal.md` prompt. This file is a dead artifact.

---

## 3. Category B — Inline Go String Literals (Hardcoded Prompt Fragments)

These are strings scattered in Go source that become part of LLM message content.

### 3.1 `discuss.go` — `buildDiscussContext()` (Lines 107, 128)

```go
// Line 107 — MEMORY.md injection label
Content: "Memory from previous sessions:\n" + string(mem)

// Line 128 — duplicate instruction
Content: ctx + "\n\nAsk 2-4 clarifying questions to understand the requirements better. Number each question. Be specific and concise."
```

**Problems:**
- `"Memory from previous sessions:"` — a prompt-shaping string not in any prompt file.
- The clarifying question instruction **duplicates** `discuss-questions.md`.

---

### 3.2 `plan.go` — `buildPlanContext()` (Lines 130, 136, 147–150, 156–167, 170)

```go
// Line 130 — context header
ctx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s\n\n", goal, projectType, framework)

// Line 136 — memory injection label
ctx += "## Cross-Session Memory\n" + string(mem) + "\n\n"

// Line 142 — file schema label
ctx += "Existing files:\n" + fileSchema + "\n\n"

// Line 147–150 — discuss Q&A injection
ctx += "User answers from Discuss phase:\n"
ctx += fmt.Sprintf("- Q: %s → A: %s\n", q, a)

// Line 156 — retry preamble
ctx += "## Previous Attempt Failed\n"
// Line 167 — retry instruction
ctx += "Please fix the issues above and return a corrected JSON task array.\n\n"

// Line 170 — CRITICAL: schema instruction duplicating plan-format.md
ctx += "Generate a task list to accomplish the goal. Return a JSON array of tasks. " +
    "Each task must have: id (int), action (string: Add/Modify/Delete/Create), description (string), " +
    "dependencies (array of int, empty if none), files (array of string), acceptance_criteria (array of string). " +
    "Do not include any text outside the JSON array."
```

**Problems:**
- Line 170 is a **complete re-statement** of `plan-format.md`'s schema in a user message. Both arrive in the same request.
- All section headers (`## Cross-Session Memory`, `## Previous Attempt Failed`, etc.) are untestable, unversioned strings buried in Go code.

---

### 3.3 `execute.go` — `buildExecuteContext()` (Lines 308, 316, 320–323)

```go
// Line 308 — project context block
projectCtx = fmt.Sprintf("## Project Context\nGoal: %s\nType: %s\nFramework: %s\n\n",
    project.Goal, project.ProjectType, project.Framework)

// Line 316 — task list label
Content: projectCtx + "Task list:\n" + taskSummary

// Lines 320–323 — current task spec (BUG: task.ID appears twice)
taskSpec := fmt.Sprintf("Execute task %d: %d\nAction: %s\nDescription: %s\nFiles: %v\nDependencies: %v",
    task.ID, task.ID, task.Action, task.Description, task.Files, task.Dependencies)
taskSpec += "\nAcceptance criteria: " + strings.Join(task.AcceptanceCriteria, "; ")
```

**Problems:**
- All prompt-shaping labels are unmanaged Go strings.
- **Line 320 bug:** `"Execute task %d: %d"` passes `task.ID` twice (second `%d` should not be there).
- No explanation of what `Files: [...]` or `Dependencies: [...]` means to the model.

---

### 3.4 `execute.go` — `healTask()` (Line 339)

```go
{Role: "user", Content: fmt.Sprintf(
    "Task %d failed: %s\n\nCurrent file state:\n%s\n\nFix the issue and use tools to apply the fix.",
    task.ID, failure, e.readTaskFiles(task.Files)
)}
```

**Problems:**
- A full user-turn prompt constructed inline with no file-based management.
- Partially overlaps with `self-heal.md` (which is in the system prompt).
- Too vague given the elaborate guidance in `self-heal.md`.

---

### 3.5 `verify.go` — `tryBisectHeal()` (Line 208)

```go
failure := fmt.Sprintf(
    "bisect identified commit %s as introducing the failure:\n%s\n\nVerification errors: %v",
    bisectResult.OffendingCommit.ShortHash, bisectResult.Diff, verifyResult.Errors,
)
```

**Problems:**
- Inline bisect context template; prompt-critical but ungoverned.
- The model receives a raw git diff and must infer how to interpret it.

---

### 3.6 `engine.go` — `HealTask()` (Line 324)

```go
failure := fmt.Sprintf("manual heal requested for task %d", task.ID)
```

**Problems:**
- Gives the LLM no actionable information about why the task failed.
- Manual heals triggered from the TUI pass this as the entire failure context.

---

## 4. Category C — Tool Descriptions & Parameter Schemas

All tools implement `Description() string` and `ParameterSchema() string`. These are sent to the LLM via `buildToolDefinitions()` in `engine.go` and directly influence how the model invokes tools.

| Tool | Description | Schema Issues |
|------|-------------|---------------|
| **Bash** | "Execute a shell command with output capping, timeout, and working directory support." | Complete |
| **FileRead** | "Read a file's contents with path safety checks." | Complete |
| **FileWrite** | "Write content to a file atomically with backup and path safety." | No read-first guidance |
| **Edit** | "Make targeted edits to a file using line-range replacement or smart string matching." | `start_line`/`end_line` params exist in code **but are absent from schema** |
| **Glob** | "List files matching a glob pattern. Supports ** for recursive matching." | Complete |
| **Grep** | "Search file contents using regex patterns. Uses ripgrep when available, falls back to pure-Go." | `max_results` param exists in code **but is absent from schema** |
| **TodoWrite** | "Write the complete todo list to TODO.md in the session directory." | Complete |
| **AskUserQuestion** | "Ask the user a question and wait for their answer." | `allow_custom` and `timeout` params exist **but are absent from schema** |

> **IMPORTANT:** The `Edit` tool's `start_line`/`end_line` parameters and the `Grep` tool's `max_results`
> parameter are implemented but **absent from JSON Schema**. The model can never discover or use these features,
> making them effectively dead code from the LLM's perspective.

---

## 5. Test Fixtures With Inline Prompts

```go
// pkg/autodream/autodream_test.go, line 441
{Role: "system", Content: "You are a helpful assistant.", ...}
```

Test-only; does not affect production. Noted because it uses a generic persona instead of the actual `base.md` — making the test less representative.

---

## 6. Summary Table — All Hardcoded Prompt Locations

| ID | File | Lines | Type | Description |
|----|------|-------|------|-------------|
| P1 | `prompts/base.md` | 1–59 | Embedded | Universal system identity + principles |
| P2 | `prompts/tool-use.md` | 1–51 | Embedded | Tool reference (5 tools, missing 4) |
| P3 | `prompts/plan-format.md` | 1–52 | Embedded | Plan output schema |
| P4 | `prompts/discuss-questions.md` | 1–27 | Embedded | Discuss question guidance |
| P5 | `prompts/execute-task.md` | 1–41 | Embedded | Execute phase task instructions |
| P6 | `prompts/self-heal.md` | 1–32 | Embedded | Self-heal diagnostic loop |
| P7 | `prompts/verify-checklist.md` | 1–31 | Embedded | **DEAD — never injected** |
| I1 | `discuss.go:107` | inline | Category B | `"Memory from previous sessions:\n"` |
| I2 | `discuss.go:128` | inline | Category B | Duplicate discuss instruction |
| I3 | `plan.go:130` | inline | Category B | Goal/ProjectType/Framework header |
| I4 | `plan.go:136` | inline | Category B | `"## Cross-Session Memory\n"` |
| I5 | `plan.go:142` | inline | Category B | `"Existing files:\n"` |
| I6 | `plan.go:147` | inline | Category B | `"User answers from Discuss phase:\n"` |
| I7 | `plan.go:156` | inline | Category B | `"## Previous Attempt Failed\n"` |
| I8 | `plan.go:167` | inline | Category B | `"Please fix the issues above..."` |
| I9 | `plan.go:170` | inline | Category B | **Full schema instruction (duplicates plan-format.md)** |
| I10 | `execute.go:308` | inline | Category B | `"## Project Context\n..."` |
| I11 | `execute.go:316` | inline | Category B | `"Task list:\n"` |
| I12 | `execute.go:320` | inline | Category B | **Task spec template — bug: `%d: %d`** |
| I13 | `execute.go:339` | inline | Category B | Heal user-turn prompt |
| I14 | `verify.go:208` | inline | Category B | Bisect context prompt |
| I15 | `engine.go:324` | inline | Category B | Manual heal failure string (uninformative) |
| T1 | `tools/bash.go:29,37` | method | Category C | Description + schema |
| T2 | `tools/fileread.go:29,37` | method | Category C | Description + schema |
| T3 | `tools/filewrite.go:35,43` | method | Category C | Description + schema |
| T4 | `tools/edit.go:30,38` | method | Category C | Description + schema (**missing `start_line`/`end_line`**) |
| T5 | `tools/glob.go:29,37` | method | Category C | Description + schema |
| T6 | `tools/grep.go:37,45` | method | Category C | Description + schema (**missing `max_results`**) |
| T7 | `tools/todo.go:37,45` | method | Category C | Description + schema |
| T8 | `tools/question.go:42,50` | method | Category C | Description + schema (**missing `allow_custom`, `timeout`**) |

---

## 7. Improvement Recommendations for `internal/workflow/prompts/`

### 7.1 Remove or Inject `verify-checklist.md`

`verify-checklist.md` is never used. Options:
- **Option A (recommended):** Delete the file — verification is programmatic. No LLM prompt needed.
- **Option B:** Add injection in `runVerify()` if LLM-assisted verification is planned. Register `VerifyChecklist` in message building.

---

### 7.2 Add Missing Tools to `base.md` and `tool-use.md`

The registered tool set is: `Bash, FileRead, FileWrite, Edit, Glob, Grep, TodoWrite, WebFetch, AskUserQuestion`.

**Add to `base.md` — Tool Philosophy section:**
```markdown
- **Edit**: Make targeted string or line-range replacements in existing files (prefer over FileWrite for modifications)
- **WebFetch**: Fetch a URL and return its content as text (no JS execution)
- **TodoWrite**: Write a structured TODO list to the session directory
- **AskUserQuestion**: Pause and ask the user a question (interactive sessions only — never in automated execution)
```

**Add to `tool-use.md`:**
```markdown
## Edit
- Use for: targeted modifications to existing files (replace a string, swap a line range)
- Prefer over FileWrite when changing < 30% of a file
- Strategies: exact-match → line-trimmed → whitespace-normalized → fuzzy-anchor
- Supports start_line + end_line for precise line-range replacement

## WebFetch
- Use for: fetching documentation, APIs, or external resources
- Returns text/markdown content; does not execute JavaScript
- Timeout: 30s

## TodoWrite
- Use for: writing a structured TODO list to track task progress
- Parameters: todos (array of {content, status, priority})

## AskUserQuestion
- Use for: pausing execution to ask the user a clarifying question
- NEVER use in automated task execution
- Parameters: question (required), header, options, allow_custom, timeout
```

---

### 7.3 Eliminate Duplicate Instructions (I2 and I9)

**I2 — `discuss.go:128`:** The `discuss-questions.md` system prompt already provides the instruction. Remove from user message:
```go
// Before:
Content: ctx + "\n\nAsk 2-4 clarifying questions..."

// After (system prompt already handles it):
Content: ctx
```

**I9 — `plan.go:170`:** The `plan-format.md` system prompt already states the full JSON schema. Remove duplicated instruction from user message. Only keep retry-specific feedback:
```go
// Remove line 170 entirely (or only add on retry attempts)
if len(existingTasks) > 0 {
    ctx += "Please fix the errors above and return a corrected JSON task array.\n\n"
}
```

---

### 7.4 Move Inline User-Turn Templates to Prompt Files

Create two new files in `prompts/`:

**`prompts/heal-context.md`** (template for `healTask()` user message):
```markdown
# Self-Heal Request

Task {{.TaskID}} failed with the following error:

{{.FailureReason}}

## Current File State
{{.FileContents}}

Review the error, diagnose the root cause, and apply a fix using your available tools.
Follow the self-healing diagnostic process from your instructions above.
```

**`prompts/plan-context.md`** (template for plan phase user message):
```markdown
## Goal
{{.Goal}}

## Project
- Type: {{.ProjectType}}
- Framework: {{.Framework}}

{{if .Memory}}## Cross-Session Memory
{{.Memory}}
{{end}}
{{if .FileSchema}}## Existing Files
{{.FileSchema}}
{{end}}
{{if .Answers}}## Requirements from Discuss Phase
{{range .Answers}}- Q: {{.Question}} → A: {{.Answer}}
{{end}}{{end}}
{{if .RetryErrors}}## Previous Attempt Failed — Fix Required
Errors:
{{range .RetryErrors}}- {{.}}
{{end}}
{{end}}
```

Use `text/template` in Go to render these, keeping all prompt text in `.md` files.

---

### 7.5 Fix `execute-task.md` Commit Format Drift

Replace hardcoded `feat(task <id>): <description>` with a config-neutral statement:
```markdown
## Commit Format
Commit your changes after completing each task using a descriptive message.
The engine will apply the project's configured commit prefix automatically.
Example: `<prefix>: <task description>`
```

---

### 7.6 Fix `self-heal.md` — Replace Hardcoded Attempt Count

Replace:
```markdown
After 2 failed heal attempts, the task is marked unrecoverable.
```
With:
```markdown
After the maximum number of heal attempts is exhausted, the task is marked unrecoverable.
If you cannot diagnose the issue, say so explicitly:
"I cannot determine the root cause. The task may be unrecoverable."
```

---

### 7.7 Fix the `execute.go:320` Bug

```go
// Bug: task.ID appears twice
taskSpec := fmt.Sprintf("Execute task %d: %d\nAction: ...")

// Fix:
taskSpec := fmt.Sprintf("Execute task %d\nAction: %s\nDescription: %s\nFiles: %v\nDependencies: %v",
    task.ID, task.Action, task.Description, task.Files, task.Dependencies)
```

---

### 7.8 Fix Tool Schema Gaps

**Edit tool** — expose `start_line`/`end_line`:
```go
func (t *Edit) ParameterSchema() string {
    return `{
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File path to edit"},
            "old_string": {"type": "string", "description": "Exact string to find and replace"},
            "new_string": {"type": "string", "description": "Replacement string"},
            "start_line": {"type": "integer", "description": "Start line for line-range edit (1-indexed, requires end_line)"},
            "end_line": {"type": "integer", "description": "End line for line-range edit (1-indexed, inclusive)"}
        },
        "required": ["path", "new_string"]
    }`
}
```

**Grep tool** — expose `max_results`:
```json
"max_results": {"type": "integer", "description": "Maximum number of matches to return (default 100)"}
```

**AskUserQuestion tool** — expose `allow_custom` and `timeout`:
```json
"allow_custom": {"type": "boolean", "description": "Allow user to type a custom answer (default true)"},
"timeout": {"type": "integer", "description": "Seconds to wait for user answer (default 300)"}
```

---

### 7.9 Add Prompt Versioning Frontmatter

Add YAML frontmatter to each `.md` prompt file:
```markdown
---
version: 1.0
phase: discuss
injected_in: discuss.go/buildDiscussContext
last_reviewed: 2026-06-06
---
```

This enables CI checks for stale prompts and lets the team track prompt evolution alongside code changes.

---

## 8. Priority Matrix (Ranked)

| Priority | Item | Impact | Effort |
|----------|------|--------|--------|
| CRITICAL | Fix `execute.go:320` bug (`%d: %d`) | Task execution context corrupted | Low |
| CRITICAL | Expose `Edit` `start_line`/`end_line` in schema | Model can never use line-range edits | Low |
| CRITICAL | Remove or inject `verify-checklist.md` | Dead code / confusion | Low |
| HIGH | Add missing tools to `base.md` + `tool-use.md` | Model uses wrong/no tools | Medium |
| HIGH | Remove duplicate schema in `plan.go:170` | Conflicting instructions to LLM | Low |
| HIGH | Remove duplicate discuss instruction in `discuss.go:128` | Conflicting instructions | Low |
| MEDIUM | Extract heal user-turn to `prompts/heal-context.md` | Maintainability | Medium |
| MEDIUM | Extract plan context to `prompts/plan-context.md` | Maintainability | Medium |
| MEDIUM | Fix `self-heal.md` hardcoded attempt count | Silent break when constant changes | Low |
| MEDIUM | Fix `execute-task.md` commit format | Prevent config drift | Low |
| MEDIUM | Improve manual heal message (I15) | Model gets no failure context | Low |
| LOW | Add `max_results` to Grep schema | Model can tune result counts | Low |
| LOW | Add `allow_custom`/`timeout` to AskUserQuestion schema | Schema completeness | Low |
| LOW | Add prompt frontmatter versioning | Governance | Medium |

---

*Report generated: 2026-06-06. Codebase: `github.com/eshanized/M31A`.*
