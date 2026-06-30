# Seven-Phase Workflow

M31 Autonomous routes every prompt through a seven-phase workflow, giving you cost-optimized, high-quality responses with full traceability.

---

## Workflow Modes

| Mode | Phases | Use Case |
|------|--------|----------|
| **auto** (default) | Adaptive | Classifies intent and chooses appropriate phases |
| **full** | All 7 | Complete workflow for complex tasks |
| **fast** | Skip Plan | When requirements are already clear |
| **direct** | Init → Execute → Ship | Quick changes with no planning overhead |

Set via config: `features.workflow_mode = "auto"` or `/workflow mode:full`.

---

## Phase Transitions

```
Idle       → Initialize
Initialize → Discuss | Execute | Idle
Discuss    → Plan | Execute | Idle
Plan       → Execute | Plan (retry) | Discuss | Idle
Execute    → Verify | Ship | Idle
Verify     → Runtime | Ship | Execute (heal) | Idle
Runtime    → Ship | Execute | Idle
Ship       → Idle
```

---

## Phase 1: Initialize

**Purpose:** Detect project type, build code intelligence index, gather environment context.

**Package:** `internal/workflow/initialize.go`

**What happens:**
- Detects project language (Go, Node, Rust, Python) and build system
- Initializes git repository if not present
- Builds code intelligence index (import graphs, symbol lookup, relevance scoring)
- Optional deep project analysis (`init_deep_analysis = true`)
- Optional environment preflight checks (`init_preflight = true`)

**Inputs:** User goal or prompt

**Outputs:** `PhaseResult` with project state, code index, analysis results

**Failure handling:** Falls back to basic mode if deep analysis fails. Git init failure is non-fatal.

**Resume behavior:** Checkpoint saved with project state and analysis results.

---

## Phase 2: Discuss

**Purpose:** LLM asks clarifying questions to gather requirements before planning.

**Package:** `internal/workflow/discuss.go`

**What happens:**
- LLM generates clarifying questions based on the goal
- Questions presented one at a time in the TUI with timeout
- Built-in quality scoring detects yes/no, vague, or duplicate questions
- Answer completeness check ensures sufficient context gathered
- Follow-up questions generated if initial answers are incomplete

**Inputs:** Goal, project context, code index

**Outputs:** `PhaseResult` with clarified requirements, Q&A history

**Configuration:**
- `discuss_quality_check = true` — Filter low-quality questions
- `discuss_completeness = true` — Score answer completeness
- `ui.discuss_timeout = 300` — Q&A timeout in seconds

**Failure handling:** If quality check fails, questions are regenerated. Timeout skips remaining questions.

**Resume behavior:** Checkpoint saved with Q&A state and answers.

---

## Phase 3: Plan

**Purpose:** Break down the task into structured plan with dependencies, file predictions, and acceptance criteria.

**Package:** `internal/workflow/plan.go`

**What happens:**
- Optional pre-plan research step (`plan_research = true`)
- Structured task plan with `[NEW]`/`[MODIFY]` file predictions
- Plan checker runs revision loops (max 3 iterations)
- Coverage gates verify goal phrase coverage
- Security gate checks for auth/crypto file awareness
- Gap analysis identifies missing files/goals
- Large plans auto-chunk for reliability (`plan_chunked = true`)

**Inputs:** Clarified requirements, project context, code index

**Outputs:** `PhaseResult` with plan markdown, task list with dependencies

**Configuration:**
- `plan_research = true` — Pre-plan web research
- `plan_check = true` — LLM-based plan quality review
- `plan_security_gate = true` — Security file awareness check
- `plan_coverage_gate = true` — Goal phrase coverage check
- `plan_gap_analysis = true` — Missing file detection
- `plan_chunked = true` — Auto-chunk large plans

**Failure handling:** Plan checker can trigger revision loops (up to 3 iterations). If quality remains low, falls back to user feedback.

**Resume behavior:** Checkpoint saved with plan content, version, and refinement state.

---

## Phase 4: Execute

**Purpose:** Implement tasks using tools, with self-healing and quality gates.

**Package:** `internal/workflow/execute.go`

**What happens:**
- Pre-flight validation checks dependencies (`execute_preflight = true`)
- Tasks executed in dependency order (Kahn's topological sort)
- Bounded parallelism (4 concurrent tasks via semaphore)
- LLM makes tool calls via 18 built-in tools, sees results, iterates
- Tool-call loop detection stops repeated tool sequences (`execute_loop_detect = true`)
- Per-task quality gates verify acceptance criteria (`execute_quality_gate = true`)
- Failed tasks trigger self-healing (error output fed back to LLM, up to 2 retries)

**Inputs:** Plan with tasks and dependencies

**Outputs:** `PhaseResult` with task statuses, tool call history, diff summaries

**Failure handling:**
- Self-healing retries (up to 2 attempts per task)
- Loop detection breaks infinite tool call sequences
- Git bisect fallback to find offending commit
- Tasks with unrecoverable errors marked as skipped

**Resume behavior:** Checkpoint saved with task statuses, tool call state, and live output.

---

## Phase 5: Verify

**Purpose:** Run build and test suite, verify correctness, scan for security issues.

**Package:** `internal/workflow/verify.go`

**What happens:**
- Runs configured build command (`verify.build_command` or auto-detect)
- Runs configured test command (`verify.test_command` or auto-detect)
- Self-healing on failure (up to 2 retries)
- Git bisect fallback when test failure location is unknown
- Security file scanning for hardcoded secrets
- Verification report with pass rate (`verify_report = true`)

**Inputs:** Executed code, test configuration

**Outputs:** `PhaseResult` with test results, verification report

**Configuration:**
- `verify.build_command = ""` — Build command (auto-detect if empty)
- `verify.test_command = ""` — Test command (auto-detect if empty)
- `verify_report = true` — Generate verification report

**Failure handling:** Self-healing retries, git bisect fallback, security scan.

**Resume behavior:** Checkpoint saved with verification results.

---

## Phase 6: Runtime

**Purpose:** Dev server lifecycle management, HTTP smoke tests, route discovery.

**Package:** `internal/workflow/runtime.go`

**What happens:**
- Starts dev server using configured tool
- Runs HTTP smoke tests against discovered routes
- Validates expected responses
- Discovers available routes

**Inputs:** Executed code, dev server configuration

**Outputs:** `PhaseResult` with smoke test results, route discovery

**Failure handling:** Dev server crash detection and restart. Smoke test failures logged.

**Resume behavior:** Checkpoint saved with runtime state.

---

## Phase 7: Ship

**Purpose:** Final commit, changelog generation, ledger entry, session metrics.

**Package:** `internal/workflow/ship.go`

**What happens:**
- Pre-ship checklist (`ship_preflight = true`):
  - TODO/FIXME detection
  - Debug statement scanning
  - Hardcoded secret detection
- Git commit with configured prefix
- Auto-generated changelog (`ship_changelog = true`)
- Ledger entry for cross-session learning
- Session metrics recording

**Inputs:** Verified code, plan context

**Outputs:** `PhaseResult` with commit hash, changelog, ledger entry

**Configuration:**
- `git.commit_prefix = "feat"` — Commit message prefix
- `git.ship_prefix = "chore"` — Ship commit prefix
- `ship_preflight = true` — Pre-ship checklist
- `ship_changelog = true` — Auto-generated changelog

**Failure handling:** Pre-ship failures prevent commit. Changelog generation failure is non-fatal.

**Resume behavior:** Final checkpoint saved with commit hash and metrics.

---

## Quality Gates

### Plan Quality
- **Granularity:** Task size validation
- **Security:** Auth/crypto file awareness
- **Gap Analysis:** Missing files/goals detection
- **Requirements Coverage:** Goal phrase coverage

### Discuss Quality
- **Question Quality:** Yes/no, vague, duplicate detection (Jaccard similarity)
- **Answer Completeness:** 0-100 score

### Execute Quality
- **Pre-flight:** Dependency validation
- **Loop Detection:** Repeated tool call signature detection
- **Per-task:** Acceptance criteria verification

### Ship Quality
- **Pre-ship:** TODO/FIXME, debug statements, hardcoded secrets
- **Post-ship:** Commit validation

---

## Self-Healing

Failed tasks get error output fed back to the LLM with enhanced context:
- Git diff of changes
- Code intelligence context
- Acceptance criteria

Up to 2 retries per task. Git bisect used as fallback to find offending commit when tests fail.
