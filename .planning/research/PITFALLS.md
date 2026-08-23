# Domain Pitfalls

**Domain:** Agentic coding runtime architectural migration
**Researched:** 2026-08-23

---

## Critical Pitfalls

Mistakes that cause rewrites, data loss, or fundamental architectural failures.

### Pitfall 1: Distributed State Ownership (Split-Brain Architecture)
**What goes wrong:** Multiple components independently own overlapping slices of the same engineering run state (Engine fields, WorkflowState, SessionManager files, Recovery JSON, TaskRunner state, TUI state, Git state). No single authoritative source exists. Changes in one representation don't propagate to others, causing silent state drift.

**Why it happens:** Capabilities are added incrementally to a central workflow object. Each new subsystem (context, compaction, code intelligence, recovery, metrics) gets its own state fields on the Engine or a parallel WorkflowState object. Persistence is added as an afterthought via flat files rather than a unified store.

**Consequences:**
- TUI shows "Executing" but persisted session says "Planning"
- Task runner marks task "Done" but session JSON says "Pending"
- Recovery restores phase X but StateMachine is already at phase Y
- Context snapshots represent stale source while repository has changed
- Bugs become state convergence bugs (hard to reproduce, harder to fix)

**Prevention:**
- Establish **one authoritative runtime model** (`EngineeringRun`) as the single source of truth
- Every stateful subsystem gets exactly one owner; everything else is a projection/cache
- Use SQLite as authoritative local store with explicit transactions for state changes
- TUI consumes runtime events/state — never owns lifecycle logic
- Enforce: "A projection cannot become authoritative merely because it is convenient"

**Detection:** Search for duplicate representations of the same concept (phase, task status, messages, plan version) across Engine, WorkflowState, session files, recovery files, TUI models.

**Phase mapping:** Phase 1 (Core Domain Model), Phase 2 (Persistence Layer), Phase 7 (TUI Foundation)

---

### Pitfall 2: Git as Full-Worktree Owner Instead of Scoped File Operator
**What goes wrong:** Git wrapper exposes `AddAll()` and `Commit()` as public API. Production code calls these, staging and committing ALL worktree changes including user's unrelated modifications. Fallback paths (e.g., Ship phase) silently fall back to `AddAll()` when scoped staging fails.

**Why it happens:** Git abstraction designed for convenience, not safety. No concept of "agent-owned files" vs "user-owned files" in the Git layer. Sensitive file checks only block known secret patterns, not user source files.

**Consequences:** Silent destruction of user's uncommitted work. User's staged changes, unstaged edits, and untracked files all get committed under agent's commit message. Rollback stashes user changes mixed with agent changes. Bisect operates on user's primary worktree.

**Prevention:**
- Remove or privatize `Commit()` and `AddAll()` — no production caller should have access
- All Git mutations must use `CommitWithFiles(paths...)` with explicitly tracked agent files
- Track agent-modified files in session; only reset those files on rollback (`git checkout <commit> -- <files>`)
- Run bisect in isolated worktree (`git worktree add`), never on user's primary worktree
- Validate all LLM-provided file paths through central `ValidateTaskFiles()` before any Git operation

**Detection:** Grep for `AddAll()`, `Commit()` (not `CommitWithFiles`), `StashPush` without file scoping.

**Phase mapping:** Phase 4 (Execution Engine), Phase 8 (Git Intelligence), Phase 9 (Migration Strategy)

---

### Pitfall 3: State Machine Validation Bypass for Checkpoint/Recovery
**What goes wrong:** `StateMachine.SetPhase()` allows arbitrary phase transitions without validation. Four production callers (checkpoint load, recovery, rollback, rollback-to-checkpoint) bypass the transition graph. A stale/corrupted recovery file can set phase to `Ship` from `Idle`, skipping all intermediate phases and their safety checks.

**Why it happens:** State machine treated as data store (setter/getter) rather than validated transition system. Checkpoint/restore assumes data is always valid. No session ID validation on restore.

**Consequences:** Engine enters semantically invalid states (Ship without Execute). Phase-specific invariants (task state, plan existence, git baseline) not validated on restore. Cross-session corruption when recovery file from workflow A loads during workflow B startup.

**Prevention:**
- Replace `SetPhase()` with `RestorePhase(phase, expectedFrom)` that validates transition
- Store `fromPhase` in checkpoint; validate on restore
- Add session ID to recovery state; reject cross-session restores
- Validate restored phase has corresponding tasks in session manager
- Log structured warnings when validation is bypassed (last resort only)

**Detection:** Search for `SetPhase()` calls outside of `Transition()`. Check if recovery/checkpoint load validates session ID and phase reachability.

**Phase mapping:** Phase 2 (Persistence Layer), Phase 4 (Execution Engine), Phase 9 (Migration Strategy)

---

### Pitfall 4: Verification Threshold Semantics Allow Broken Code to Ship
**What goes wrong:** Product decision to treat partial failure as success via `VerifySuccessThreshold = 0.90`. If 10% of tasks fail (tests fail, syntax errors, missing files), verification still returns `Success: true`. Failed tasks logged as "warnings" but Ship phase proceeds.

**Why it happens:** Threshold introduced as pragmatic compromise for "mostly working" runs. No user consent required for shipping known-broken code. Downstream phases don't know verification had failures.

**Consequences:** Broken code ships with test failures, syntax errors, missing files. Core correctness guarantee violated. Adversarial review and verification evidence model undermined.

**Prevention:**
- Remove threshold entirely: `allOK = len(failedTasks) == 0`
- If partial completion is a product requirement, add explicit opt-in config flag `allow_partial_verify` defaulting to `false`
- Require explicit user confirmation in TUI before Ship when partial verify is enabled
- Verification must be evidence-backed, not confidence-backed

**Detection:** Search for success rate calculations that override failure counts. Check if Ship phase inspects individual task verification results.

**Phase mapping:** Phase 5 (Verification Engine), Phase 9 (Migration Strategy)

---

### Pitfall 5: Permission System Coupled to TUI (Headless Mode Broken)
**What goes wrong:** Dispatcher permission flow requires TUI listener on a channel. Headless mode (`--goal`, `--prompt`) creates dispatcher but no permission listener exists. `ensurePermission()` blocks forever on unbuffered channel or returns `ErrPermissionDenied` if buffer full. Dangerous tools either hang or silently fail.

**Why it happens:** Permission system designed exclusively for interactive TUI. No concept of "execution context" (interactive vs headless vs CI). Permission policy is static config, not context-aware.

**Consequences:** Headless workflows non-functional for tool-using operations. No user consent obtained for destructive operations in headless mode. Documented CLI feature doesn't work.

**Prevention:**
- Extract `PermissionPolicy` interface: `Decide(ctx, tool, risk, input) -> Decision`
- Implementations: `InteractivePolicy` (TUI), `HeadlessDenyPolicy` (deny dangerous), `HeadlessAllowPolicy` (allow with audit), `CIPolicy` (predefined rules)
- Dispatcher accepts `PermissionPolicy` at construction, not hardcoded TUI channel
- Add `--permission-mode=auto|deny|allow` flag for headless mode

**Detection:** Check if dispatcher has hardcoded channel sends for permission requests. Verify headless mode tests exist for tool execution.

**Phase mapping:** Phase 3 (Tool/Permission System), Phase 9 (Migration Strategy)

---

### Pitfall 6: Incomplete Task State Restoration on Recovery/Resume
**What goes wrong:** Checkpoint saves partial state (phase, goal, plan version, messages) but NOT task execution state (Status, HealsAttempted, acceptance results). On resume, engine reinitializes TaskRunner with fresh tasks — completed tasks re-execute, failed tasks lose heal count, status diverges between TUI and engine.

**Why it happens:** Task state treated as "derived" (recomputed from plan) but actually has independent lifecycle: heal attempts, status transitions, acceptance criteria results. These are NOT derivable from plan alone.

**Consequences:** Duplicate work, lost heal attempts, incorrect phase transitions, task status divergence between TUI and engine. Session resume fundamentally broken.

**Prevention:**
- Checkpoint must include complete workflow state: tasks, heal counts, status, acceptance results
- `LoadCheckpointData()` must reinitialize TaskRunner with restored task states
- Session ID in recovery state prevents cross-workflow corruption
- On resume: load tasks from session manager BEFORE transitioning state machine

**Detection:** Verify checkpoint struct includes task array. Check resume path loads tasks and reinitializes runner.

**Phase mapping:** Phase 2 (Persistence Layer), Phase 4 (Execution Engine), Phase 9 (Migration Strategy)

---

### Pitfall 7: Inconsistent Path Validation — Scattered, Not Centralized
**What goes wrong:** No centralized path validation layer. Each tool handles paths differently:
- `FileWrite` uses `ResolveAndContainPath()` ✓
- `Bash.Execute()` validates workdir with `filepath.Rel()` but NO symlink resolution ✗
- `verifyTask()` uses `filepath.Join()` without containment check ✗
- `CommitWithFiles()` passes paths directly to `git add --` ✗
LLM-controlled `task.Files` flows through multiple systems without a single validation gate.

**Why it happens:** Path validation added per-tool as needed. No architectural boundary enforcing "all LLM paths validated once at entry."

**Consequences:** Path traversal via git commit (staging files outside workdir), verification reads, or bash workdir escape via symlinks. Supply chain risk from model output.

**Prevention:**
- Central `ValidateTaskFiles(task.Files, workDir)` called at task creation (Plan phase) AND before every use (Execute, Verify, Ship)
- Resolve symlinks on BOTH project root and target before `filepath.Rel` comparison (`filepath.EvalSymlinks`)
- Reject tasks with invalid paths at plan validation time
- All file operations trust validated paths; no re-validation needed downstream

**Detection:** Search for `filepath.Join(workDir, ...)` without `ResolveAndContainPath`. Check if `task.Files` validated at plan parse time.

**Phase mapping:** Phase 1 (Core Domain Model), Phase 3 (Tool/Permission System), Phase 4 (Execution Engine)

---

### Pitfall 8: Shell Command Validation via Regex Heuristics
**What goes wrong:** `containsVariableExpansion()` uses regex `\$[A-Za-z_]` catching `$VAR` but missing `${VAR}`, `${VAR:-default}`, `$((arithmetic))`, `${#array}`. `checkCommandChaining()` splits on `; & |` but doesn't recursively parse nested `$()` inside arguments. String-based parsing cannot capture POSIX shell grammar (context-sensitive: quotes, escapes, nested expansions).

**Why it happens:** Regex/string parsing seems sufficient for "obvious" injections. Shell grammar complexity underestimated.

**Consequences:** Potential command injection if LLM output includes expansions evaluating to attacker-controlled values. Command substitution in arguments (`echo $(id)`) bypasses checks.

**Prevention:**
- Use proper shell parser: `mvdan.cc/sh/v3/syntax` + `mvdan.cc/sh/v3/parser`
- Walk AST, reject dangerous nodes: CommandSubst (`$(...)` or `` `...` ``), ArithmExpr (`$((...))`), dangerous ParamExp flags, redirections to `/dev/tcp`, `/dev/udp`
- Or run commands with `bash -c` where expansions disabled (if compatible with use cases)
- Complete deny-list of ALL shell metacharacters as fallback: `$`, `` ` ``, `{`, `}`, `(`, `)`, `[`, `]`, `*`, `?`, `~`

**Detection:** Test bash tool with `${PATH:-/malicious}`, `echo $(id)`, `cmd arg $(id) arg2`.

**Phase mapping:** Phase 3 (Tool/Permission System)

---

### Pitfall 9: Event-Driven Architecture Without Durable Event Store
**What goes wrong:** System claims to be "event-driven" but events exist only in-memory (emitter channel) or as transient TUI messages. No durable event log enables recovery, replay, audit, or debugging. Events lost on crash/restart. Current state not derivable from events.

**Why it happens:** Event vocabulary defined (CONTEXT_M31A.md §11) but implementation uses Bubble Tea `tea.Msg` channel for UI updates only. No persistence layer for events.

**Consequences:** 
- No recovery from crash mid-run (only partial checkpoint)
- No replay for debugging
- No audit trail of agent decisions
- No deterministic reconstruction from event history
- TUI state becomes de facto source of truth

**Prevention:**
- Implement append-only event store in SQLite (`events.db` per CONTEXT_M31A.md §12)
- Every state transition emits durable event with monotonic ordering
- Current state derivable from event log (event sourcing)
- TUI subscribes to event projections, not direct Engine fields
- Event vocabulary from CONTEXT_M31A.md §11 as minimum set

**Detection:** Check if events persist across process restart. Verify event store schema matches vocabulary.

**Phase mapping:** Phase 2 (Persistence Layer), Phase 4 (Execution Engine), Phase 7 (TUI Foundation)

---

### Pitfall 10: Provider-Agnostic Interface Leaking Provider-Specific Details
**What goes wrong:** `LLMProvider` interface assumes OpenAI-compatible chat completion. NVIDIA client overrides `doChatStream` heavily. Provider-specific request options (`chat_template_kwargs` for NVIDIA coding-agent compatibility, reasoning enable/disable) scattered in generic code instead of owned by provider adapter.

**Why it happens:** First provider (OpenRouter) sets implicit contract. Subsequent providers (Zen, NVIDIA) forced into same shape. Provider-specific features (reasoning, tool calling formats, context window metadata) not modeled in interface.

**Consequences:** 
- NVIDIA-specific logic leaks into generic agent code
- New provider with different API shape requires interface changes
- Model capability detection uses heuristic string matching on IDs instead of API metadata
- Context window management uses estimates instead of provider-reported limits

**Prevention:**
- `LLMProvider` interface models capabilities abstractly: `SupportsTools()`, `SupportsReasoning()`, `GetContextWindow()`, `GetModelMetadata()`
- Provider adapter owns ALL provider-specific request/response translation
- `chat_template_kwargs` and similar options encapsulated in provider implementation
- Capability detection uses provider API metadata first, heuristics only as fallback
- Model routing abstraction built but v1 uses single model — no premature complexity

**Detection:** Search for provider-specific fields (`chat_template_kwargs`, `enable_thinking`) outside provider package. Check if capability detection uses string matching on model IDs.

**Phase mapping:** Phase 3 (LLM Provider Abstraction), Phase 9 (Migration Strategy)

---

### Pitfall 11: Context Engineering as Prompt Manipulation Instead of Decision Support
**What goes wrong:** Context architecture is primarily a conversation-size management mechanism (estimate tokens → compact → truncate tool output → truncate old messages → reject). Answers "how to stop prompt from getting too big" rather than "what does the model need for this engineering decision?"

**Why it happens:** Context built by `ContextBuilder` directly referencing `WorkflowState` fields, configuration, tokens, callbacks into Engine. Multiple layers (WorkflowState.Messages, ContextBuilder, ContextRegistry, ContextSnapshot map, Compactor, TokenEstimator) manipulate same conversation data.

**Consequences:**
- Model receives irrelevant context (noise) and misses relevant context (signal)
- Compaction re-processes entire history (not incremental)
- No attribution: can't answer "what did the model know when it made this decision?"
- Context snapshots not tied to run/task/agent identity

**Prevention:**
- Context Engine consumes authoritative run/task model (not WorkflowState fields)
- `ContextRequest` → Run → Task → Plan slice → Repository intelligence → Evidence → Memory → Git state → Policy → Relevance → Budget → `ContextSnapshot`
- Every model decision gets attributed `ContextSnapshot` with: RunID, TaskID, AgentID, Decision, Model, TokenBudget, TokenUsed, Sources[], Hash
- Incremental compaction: only compact new messages since last compaction point
- Provider `context_window` from API metadata, not estimates

**Detection:** Check if ContextBuilder inspects Engine/WorkflowState fields directly. Verify ContextSnapshot exists with attribution fields.

**Phase mapping:** Phase 3 (LLM Provider Abstraction), Phase 4 (Execution Engine), Phase 5 (Verification Engine)

---

### Pitfall 12: Task Graph as Markdown Plan Instead of Executable IR
**What goes wrong:** Plan produced as Markdown prose. Executor interprets Markdown differently than planner wrote it. No strongly-typed planning IR with explicit dependencies, preconditions, acceptance criteria, verification strategies. LLM produces semantic intent; system should compile to executable TaskGraph.

**Why it happens:** Markdown is human-readable and LLM-friendly. Parsing Markdown back to structured tasks is error-prone but seems easier than building IR.

**Consequences:**
- Planner writes nice-looking plan that executor interprets differently
- Dependencies implicit in prose, not explicit in IR
- Acceptance criteria not machine-checkable
- Verification strategies not linked to tasks
- Risk classifications and checkpoints not enforceable

**Prevention:**
- Strongly-typed `TaskGraph` IR (Objectives, Requirements, Constraints, Decisions, Tasks, Dependencies, Preconditions, AcceptanceCriteria, VerificationSteps, RiskLevel, Checkpoints)
- Human-readable Markdown generated FROM IR (projection)
- Runtime executes IR, not Markdown
- Plan validation checks IR consistency before execution

**Detection:** Check if executor parses Markdown to extract tasks. Verify TaskGraph struct exists with all required fields.

**Phase mapping:** Phase 1 (Core Domain Model), Phase 4 (Execution Engine), Phase 5 (Verification Engine)

---

### Pitfall 13: Verification as Model Confidence Instead of Evidence
**What goes wrong:** Verification relies on model saying "done" or confidence scores. No evidence-backed verdicts. Verifier can silently mutate production code to make failing verification pass. No adversarial review loop.

**Why it happens:** Verification implemented as "run tests and check exit code" without evidence model. Verifier agent shares context with implementer, reproducing same assumptions.

**Consequences:** 
- Agent declaring success ≠ verification
- Failed criteria marked as passed without evidence
- No proof-carrying changes
- Adversarial review not independent (same context/assumptions)

**Prevention:**
- Verification levels (Structural, Unit, Integration, Behavioral, Architectural, Human UAT) with explicit evidence requirements
- `criterion → strategy → command/test → evidence → verdict` chain for every acceptance criterion
- Verifier MUST NOT mutate production code; repair transitions back to planning/execution explicitly
- Adversarial review uses independent model/context when practical
- Proof-carrying changes: every task produces durable change record with intent, plan, diff, commands, tests, evidence, risks, verdict

**Detection:** Check if verification only checks exit codes. Verify evidence model exists with provenance (file, command, test, agent).

**Phase mapping:** Phase 5 (Verification Engine), Phase 4 (Execution Engine)

---

### Pitfall 14: TUI as State Owner Instead of Projection
**What goes wrong:** TUI `AppState` holds workflow phase, task states, agent states, tool states, run state — effectively a second copy of runtime state. TUI mutates this state directly from goroutines (violating Bubble Tea). UI state diverges from runtime state.

**Why it happens:** Bubble Tea Elm architecture misunderstood. TUI built to "manage" workflow rather than "display" it. `MsgEmitter` channel from Engine to TUI carries partial updates; TUI reconstructs state from messages.

**Consequences:**
- Race conditions, lost updates, UI desync (mutating AppState from goroutines)
- TUI state becomes de facto source of truth
- Recovery can't rely on TUI state
- Hard to test runtime without TUI

**Prevention:**
- TUI owns ONLY transient presentation state: focus, selected rows, open overlays, scroll offsets, local draft input, terminal dimensions, cached view projections
- TUI subscribes to normalized application events (session.created, run.started, task.completed, tool.failed, checkpoint.required, verification.completed)
- Events carry monotonic ordering; stale events never move rendered state backwards
- Runtime state authoritative; TUI is read-only projection
- All state mutations in `Update()` only; goroutines send `tea.Msg` via channels

**Detection:** Search for `AppState` fields that mirror Engine/WorkflowState fields. Check for direct `AppState` mutation from goroutines.

**Phase mapping:** Phase 7 (TUI Foundation), Phase 2 (Persistence Layer), Phase 9 (Migration Strategy)

---

### Pitfall 15: Capability-Based Permissions Implemented as Role-Based Allow/Deny
**What goes wrong:** Permission system uses simple "allow/ask/deny" per tool with batch approval and TTL. No capability scoping (filesystem scope, git scope, shell scope, database scope). No risk classes per capability. No workspace containment validation integrated with permission evaluation.

**Why it happens:** Permission system evolved from "allow shell yes/no" to rule-based but kept tool-centric model. Capability model (CONTEXT_M31A.md §23) designed but not implemented.

**Consequences:**
- Overly broad permissions (e.g., "allow shell" = full filesystem access)
- No least-privilege for specific operations
- Symlink escapes not caught by permission layer
- Secret files accessible if tool allowed
- MCP/plugin tools not treated as separate trust domains

**Prevention:**
- Capability model: `capability: filesystem.write, scope: repository, risk: medium, conditions: [deny .env, deny *.pem, deny ~/.ssh/*, deny provider credential files]`
- Git capability: `allowed: [create branch, stage files, commit], requires_approval: [push, force-push, reset --hard, rebase shared branch]`
- Database capability: `requires: checkpoint = migration`
- Permission evaluation at EXECUTION TIME, not only when tools presented to model
- Central `ValidateTaskFiles()` gates all file operations before permission check

**Detection:** Check if permission rules reference capabilities/scopes or just tool names. Verify execution-time evaluation.

**Phase mapping:** Phase 3 (Tool/Permission System), Phase 8 (Git Intelligence)

---

### Pitfall 16: Recovery as Ad-Hoc State Restore Instead of Bounded Recovery Service
**What goes wrong:** Self-healing directly invoked from execute path, mixing recovery policy with execution logic. Recovery doesn't produce attributable `FailureRecord`, `RecoveryAttempt`, new `ContextSnapshot`. Executor decides how recovery works; recovery system doesn't decide final verification.

**Why it happens:** Self-heal feature added as execute-phase helper. Coupled to execute workflow, not independent service.

**Consequences:**
- Recovery logic tangled with execution logic
- No bound on recovery attempts (infinite loops possible)
- Recovery attempts not auditable
- Verification after recovery uses same context as failed attempt

**Prevention:**
- Extract `RecoveryService` with explicit contract
- `Executor → Failure → RecoveryService → Repair attempt → Verifier → Pass/Fail`
- Every recovery attempt produces attributable records
- Recovery bounded by policy (max attempts, escalation path)
- Verifier remains authoritative for verification (independent of recovery)

**Detection:** Check if execute code directly calls healing/verification/commit logic. Verify recovery produces durable records.

**Phase mapping:** Phase 4 (Execution Engine), Phase 5 (Verification Engine), Phase 9 (Migration Strategy)

---

### Pitfall 17: SQLite Migration as Schema Port Instead of Ownership Transfer
**What goes wrong:** Moving from flat files to SQLite by creating tables mirroring JSON structures. Flat files remain as "backup" or "projection" but still written by old code paths. No clear ownership transfer — both systems write, creating split-brain at persistence layer.

**Why it happens:** Incremental migration seems safer. Flat file code paths not deleted because "might need them."

**Consequences:** 
- Two persistence systems, neither authoritative
- Race conditions between SQLite and flat file writes
- Recovery loads from wrong store
- Migration never completes; technical debt doubles

**Prevention:**
- Introduce storage abstraction interfaces FIRST (keep flat file implementation temporarily)
- Add SQLite as authoritative backend behind same interfaces
- Switch all consumers to interfaces
- ONLY THEN delete flat file implementation
- Mark flat files explicitly as projections/artifacts (human-readable), not authoritative state
- Use explicit transactions for state changes: `TaskStarted`, `TaskCompleted`, `VerificationPassed`, `RecoveryStarted`, `RunPaused`, `RunCompleted`

**Detection:** Check if both SQLite and flat file writers exist for same entity. Verify interfaces hide backend.

**Phase mapping:** Phase 2 (Persistence Layer), Phase 9 (Migration Strategy)

---

### Pitfall 18: Model Capability Detection via Heuristics Instead of API Metadata
**What goes wrong:** `ParseModelCapabilities()` uses string matching on model IDs (e.g., "r1" → reasoning, "vision" → multimodal). Provider APIs return capability metadata (modality, tool support, context window) but it's ignored. New models break capability detection.

**Why it happens:** Quick heuristic works for known models. Provider API metadata parsing deferred. Model metadata not extended to include capabilities.

**Consequences:** 
- Models incorrectly classified → wrong tool calling format sent → tool calls fail
- Context overflow (estimates vs actual `context_window`)
- Reasoning not enabled for models that support it
- Multimodal capabilities missed

**Prevention:**
- Extend provider model metadata to include capabilities from API
- Update `ParseModelCapabilities` to use API data first, heuristics second
- Store `context_window` from provider metadata; use for token budget with safety margin (90%)
- Test with unknown model IDs to verify fallback behavior

**Detection:** Search for string matching on model IDs for capabilities. Check if provider model struct includes capability fields.

**Phase mapping:** Phase 3 (LLM Provider Abstraction), Phase 9 (Migration Strategy)

---

### Pitfall 19: Emitter Channel Backpressure Missing — Workflow Stalls Under Load
**What goes wrong:** Workflow emitter channel (Engine → TUI) has fixed capacity. `drainAdaptiveCmd()` only batches at 25% capacity. Under high-frequency tool calls (>100 ops/sec), channel fills, workflow goroutines block on send. No backpressure signal to workflow engine to slow tool dispatch.

**Why it happens:** Channel sized for typical load. Backpressure not considered because "TUI should keep up." No mechanism for TUI to signal "slow down."

**Consequences:** Workflow stalls, memory growth, potential deadlock if workflow holds locks while sending. TUI unresponsive during heavy execution.

**Prevention:**
- Add `context.Context` with timeout to emitter sends
- Implement backpressure: when channel > 75% full, signal workflow to pause tool dispatch via context cancellation or rate limiter
- Drop non-critical messages (debug/trace) under pressure; keep critical (task state, checkpoints, errors)
- Monitor channel depth in diagnostics

**Detection:** Load test with high-frequency tool calls. Monitor channel capacity metrics.

**Phase mapping:** Phase 7 (TUI Foundation), Phase 4 (Execution Engine)

---

### Pitfall 20: Bisect/Rollback Operating on User's Primary Worktree
**What goes wrong:** `Bisect.Run()` runs `git bisect` directly on user's repo. If interrupted (crash, Ctrl+C), repo left in bisect mode. `Rollback.SoftReset/HardReset` stashes ALL uncommitted changes including user's. User's work mixed with agent's rollback; potential loss on `HardReset`.

**Why it happens:** Git operations implemented as convenience methods on repo. No isolation boundary between agent operations and user workspace.

**Consequences:** User's repo corrupted (stuck in bisect mode). User's uncommitted work lost or mixed with agent changes. Manual recovery required.

**Prevention:**
- Run bisect in temporary worktree (`git worktree add`)
- Track agent-modified files explicitly in session
- Rollback uses `git checkout <commit> -- <agent-files>` for selective revert
- User's other changes left untouched
- Signal handler forces `bisect reset` on all exit paths

**Detection:** Check if bisect/rollback operate on primary worktree. Verify worktree isolation for destructive Git ops.

**Phase mapping:** Phase 8 (Git Intelligence), Phase 4 (Execution Engine)

---

## Moderate Pitfalls

### Pitfall 21: Context Compaction Not Incremental
**What goes wrong:** Compaction re-processes entire conversation history rather than incrementally. `Compact()` takes full message list each time. O(n²) behavior as session grows.

**Prevention:** Implement incremental compaction — only compact new messages since last compaction point. Store compaction checkpoint in session.

**Phase mapping:** Phase 3 (LLM Provider Abstraction)

---

### Pitfall 22: Token Estimation Allows Context Overflow
**What goes wrong:** Token estimator uses EMA (exponential moving average) based on historical averages, not worst-case. May allow requests exceeding model's context window. Provider `context_window` from metadata not used.

**Prevention:** Add safety margin (90% of context window). Validate against provider's actual `context_window` from model metadata. Use provider API limits, not estimates.

**Phase mapping:** Phase 3 (LLM Provider Abstraction)

---

### Pitfall 23: API Keys Fall Back to Plaintext Config File
**What goes wrong:** When keychain unavailable, API keys persisted to TOML config file in plaintext with only warning log. Config file may be committed accidentally or exposed.

**Prevention:** 
- Add config option to disable plaintext fallback entirely (fail hard if keychain unavailable)
- Encrypt config file keys at rest (age/sops)
- Prominent TUI warning when keys stored in plaintext
- Never write provider credentials to `.m31a/` artifacts

**Phase mapping:** Phase 2 (Persistence Layer), Phase 3 (LLM Provider Abstraction)

---

### Pitfall 24: God Object Workflow Engine by Dependency Accumulation
**What goes wrong:** `Engine` holds 30+ fields mixing workflow orchestration, state management, LLM prompting, tool dispatch, Git, metrics, hooks, code-intel, compaction, context registry, session persistence, recovery, pause/cancel channels. Violates single responsibility; coupling makes testing difficult; changes cascade.

**Prevention:** Extract domain services with explicit contracts:
- `TaskScheduler` (dependency resolution, execution waves)
- `AgentRuntime` (agent lifecycle, contracts, handoffs)
- `ContextEngine` (context assembly, compaction, snapshots)
- `ModelGateway` (provider routing, streaming, fallbacks)
- `ToolRuntime` (dispatcher, permissions, rate limits)
- `VerificationService` (evidence, verdicts, adversarial review)
- `RecoveryService` (failure handling, repair, bounds)
- `RunStore` (persistence, events, checkpoints)

**Phase mapping:** Phase 1 (Core Domain Model), Phase 4 (Execution Engine), Phase 9 (Migration Strategy)

---

### Pitfall 25: Cross-Workflow Recovery Corruption (Session ID Missing)
**What goes wrong:** Recovery file from workflow A loaded during workflow B startup (same project directory). Engine restores A's phase, plan, messages — completely wrong state for B.

**Prevention:** Session ID in recovery state. On `Recover()`, verify `state.SessionID == engine.sessionID`. Generate new session ID per workflow run.

**Phase mapping:** Phase 2 (Persistence Layer), Phase 9 (Migration Strategy)

---

## Minor Pitfalls

### Pitfall 26: Config Watcher Goroutine Error Handling
**What goes wrong:** `startConfigWatcher()` goroutine panic recovery exists but `sendReload()` can block indefinitely if receiver slow. On shutdown, stop channel closed but goroutine may be blocked in send.

**Prevention:** Make `sendReload` non-blocking with drop policy, or use `select` with `ctx.Done()`.

**Phase mapping:** Phase 7 (TUI Foundation)

---

### Pitfall 27: Todo Sync Errors Silently Ignored
**What goes wrong:** `SyncTodoFromTasks()` returns error but all callers ignore it (`_ = ...`). TODO.md diverges from actual task state without notice.

**Prevention:** Log error on sync failure. Make TODO sync best-effort with explicit warning toast.

**Phase mapping:** Phase 4 (Execution Engine), Phase 7 (TUI Foundation)

---

### Pitfall 28: Rollback Stash Uses Single Stash Entry
**What goes wrong:** `stashIfDirty()` uses fixed stash message "rollback-auto-stash". Concurrent rollbacks or user stashes overwrite each other.

**Prevention:** Use unique stash ref per rollback (include timestamp/run ID). Or better: track agent files and use `git checkout` instead of stash.

**Phase mapping:** Phase 8 (Git Intelligence)

---

### Pitfall 29: Git Diff in Verify Uses Wrong Baseline
**What goes wrong:** Verify phase checks `git diff --name-only HEAD` (unstaged only). Files committed during Execute won't appear. Committed modifications flagged as "task may not have made expected changes."

**Prevention:** Use `git diff --name-only <sessionStartHash>..HEAD` to capture all changes since session baseline.

**Phase mapping:** Phase 5 (Verification Engine), Phase 8 (Git Intelligence)

---

### Pitfall 30: No Protection Against Nested Git Repos/Submodules
**What goes wrong:** Workspace detection assumes single Git repo. Submodules or nested repos cause incorrect root detection, wrong file containment, Git operations on wrong repo.

**Prevention:** Workspace detector must handle multi-repo workspaces as first-class case. Validate Git root per file operation. Reject operations spanning multiple repos unless explicitly configured.

**Phase mapping:** Phase 8 (Git Intelligence), Phase 1 (Core Domain Model)

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|----------------|------------|
| **Phase 1: Core Domain Model** | Creating mega-struct with all project info; not establishing clear ownership boundaries | Define explicit domain objects (Project, Repository, Workspace, Session, Run, Intent, Requirement, Decision, Plan, Task, TaskGraph, Agent, Tool, Capability, PermissionPolicy, Artifact, Verification, Checkpoint, Event) with single owners |
| **Phase 2: Persistence Layer** | Keeping flat files as "backup" while adding SQLite — split-brain at persistence layer | Storage abstraction interfaces FIRST; SQLite as authoritative backend; flat files as projections only; explicit transactions for state changes |
| **Phase 3: LLM Provider Abstraction** | Leaking provider-specific details (chat_template_kwargs, reasoning) into generic code; heuristic capability detection | Provider adapter owns ALL provider-specific translation; capability metadata from API; context_window from provider |
| **Phase 4: Execution Engine** | God object recreation; task state not restored on resume; LLM file paths unvalidated | Extract domain services; checkpoint includes complete task state; central ValidateTaskFiles() at plan creation and before every use |
| **Phase 5: Verification Engine** | Threshold-based success; model confidence as verification; verifier mutates code | All-or-nothing verification; evidence-backed verdicts; verifier independent; proof-carrying changes |
| **Phase 6: Runtime** | HTTP 2xx check only; no semantic validation | Define runtime verification contracts per service type; evidence required |
| **Phase 7: TUI Foundation** | TUI owns runtime state; mutates from goroutines; no event subscription | TUI = projection only; subscribes to normalized events; all mutations in Update() |
| **Phase 8: Git Intelligence** | Git as full-worktree owner; bisect/rollback on primary worktree; no agent-file tracking | Scoped Git operations only; worktree isolation for destructive ops; agent-file tracking |
| **Phase 9: Code Intelligence** | Re-implementing parsers per language; intelligence as cache not authoritative | Orchestrate Tree-sitter + LSP + native tooling; intelligence as projection from SQLite |
| **Phase 10: Migration Strategy** | Big-bang rewrite; preserving old architecture for "backward compatibility" | Incremental ownership transfer; interfaces first; delete old code paths after consumers migrate; architectural reset — known problems MUST NOT be preserved |

---

## Sources

- **AUDIT_REPORT.md** — Forensic audit with 17 confirmed findings (3 P0, 8 P1, 4 P2, 2 P3)
- **AUDIT_ROOT_CAUSES.md** — 7 root cause clusters with architectural analysis
- **AUDIT_VALIDATION_SUMMARY.md** — Independent validation reproducing all critical findings
- **R1_REMEDIATION_SUMMARY.md** — Git safety boundary fixes (Commit() privatized, AddAll fallback removed)
- **R2_REMEDIATION_SUMMARY.md** — State machine validation, session ID recovery fixes
- **M31A_ARCHITECTURE_RESET.md** — Authoritative problem diagnosis: distributed state ownership
- **CONTEXT_M31A.md** — Canonical architecture (6 planes, domain model, event model, safety model)
- **UI-SPEC.md** — TUI specification (36 screens, component library, projection architecture)
- **.planning/codebase/ARCHITECTURE.md** — Current codebase architecture analysis
- **.planning/codebase/CONCERNS.md** — Known tech debt, bugs, security issues, test gaps
- **.planning/codebase/INTEGRATIONS.md** — External integrations audit
- **.planning/PROJECT.md** — Project requirements and constraints

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| State Ownership | HIGH | Confirmed by architecture reset doc, audit root causes, codebase analysis |
| Git Safety | HIGH | P0 reproduced, R1 fixes applied and validated |
| State Machine | HIGH | P0 reproduced, R2 fixes applied and validated |
| Verification Threshold | HIGH | P0 confirmed in source code |
| Permissions/TUI Coupling | HIGH | P1 confirmed in source code |
| Task State Recovery | HIGH | P1 confirmed, NEW-001/NEW-003 discovered in validation |
| Path Validation | HIGH | Multiple P1s from audit, inconsistent patterns confirmed |
| Shell Safety | MEDIUM | PARTIALLY_CORRECT — some expansions caught, nested missed |
| Event Sourcing | MEDIUM | Vocabulary defined in CONTEXT_M31A.md but not implemented |
| Provider Abstraction | HIGH | Leaked NVIDIA specifics confirmed in codebase |
| Context Engineering | HIGH | Architecture reset doc explicitly identifies this |
| Task Graph IR | HIGH | CONTEXT_M31A.md requires it; current uses Markdown |
| Verification Evidence | HIGH | Audit shows exit-code-only; no evidence model |
| TUI Projection | HIGH | Architecture reset doc + UI-SPEC.md both mandate this |
| Capability Permissions | MEDIUM | Designed in CONTEXT_M31A.md but not implemented |
| Recovery Service | MEDIUM | Self-heal exists but architecturally coupled |
| SQLite Migration | HIGH | Architecture reset doc mandates ownership transfer approach |
| Capability Detection | HIGH | Heuristic-only confirmed in capabilities.go |

---

## Gaps to Address

- **E2E workflow with real API keys** — No credentials in audit environment; cannot validate full integration
- **Cross-compilation targets** — Goreleaser not tested; windows/arm64 excluded but listed
- **Concurrent sessions** — Multiple M31A instances in same project not tested
- **Subagent worktree isolation** — Parallel agent isolation not validated
- **Windows path handling** — Path validation on Windows not verified
- **Large session performance** — Memory/context growth under 1000+ messages unknown
- **Network partition behavior** — Provider timeouts, retries, partial streams not tested
- **MCP/plugin trust domains** — Extension points exist but no implementation to validate