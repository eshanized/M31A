# Codebase Concerns

**Analysis Date:** 2026-07-04

## Tech Debt

**Engine concurrent field access (partially fixed):**
- Issue: `Engine.perPhaseModels` was unprotected; now protected by `perPhaseModelsMu` (`internal/workflow/engine.go:123`). However, `Engine.provider` and `Engine.modelID` are still written without locks in `SetModel()` (`internal/workflow/engine.go:577-581`), while read concurrently in `RunPhase()`, `buildSystemPrompt()`, and `consumeStream()`.
- Files: `internal/workflow/engine.go:577-581`
- Impact: Data race on every workflow execution between TUI goroutine (SetModel) and workflow goroutine (RunPhase). May cause undefined behavior under `go test -race`.
- Fix approach: Use `atomic.Value` for `provider`/`modelID` or protect with `modelIDMu` consistently.

**Headless workflow mode is a stub:**
- Issue: `runHeadlessWorkflow()` prints "headless workflow mode not yet fully implemented" and returns 1. The `--run` flag is unreachable for full workflows.
- Files: `cmd/m31a/main.go:57-59`
- Impact: Ghost mode (v1.6 roadmap goal) is non-functional. Only `--prompt` single-shot works.
- Fix approach: Implement full workflow execution headless mode with structured output (diff, decision log, verification report).

**Dangerous command blocklist is string-match only:**
- Issue: `checkDangerousCommand()` uses `strings.Contains` on the full command string. Patterns like `rm -r -f /` (separate args) or `rm$'\t'-rf /` (tab obfuscation) bypass `rm -rf /` pattern matching.
- Files: `internal/tools/bash.go:477-490`
- Impact: Defense-in-depth measure is bypassable with trivial shell syntax variations.
- Fix approach: Normalize command arguments (split on whitespace, expand variables) before pattern matching. Or parse into AST-like representation.

**`$()` and backtick patterns block legitimate use:**
- Issue: `dangerousObfuscationPatterns` blocks all `$(` and backtick substitution (`internal/tools/bash.go:470-472`). This prevents commands like `echo $(date)` or `` echo `date` `` which are benign.
- Files: `internal/tools/bash.go:470-472`
- Impact: Overly broad blocking of common shell patterns. Users must use workarounds.
- Fix approach: Only block when combined with dangerous commands, not standalone.

**String concatenation in nested loops:**
- Issue: `planCtx += fmt.Sprintf(...)` in nested loop at `internal/workflow/execute.go:741-746` and `output += fmt.Sprintf(...)` at `internal/tools/codemap.go:119-133` creates O(n^2) allocations.
- Files: `internal/workflow/execute.go:741-746`, `internal/tools/codemap.go:119-133`
- Impact: Memory pressure and CPU waste on large codebases. For a 1000-file codemap, this allocates ~1M intermediate strings.
- Fix approach: Use `strings.Builder` for all string accumulation in loops.

## Known Bugs

**Pending permission counter double-decrement:**
- Symptoms: "N tools behind this one" display shows negative or incorrect counts.
- Files: `internal/tools/permissions.go:377-385`
- Trigger: When the permission channel send fails (channel full/blocked).
- Workaround: None. The counter goes to -2 when the channel is full.

**Decision logger data loss on shutdown race:**
- Symptoms: Decision log entries lost when `Close()` races with pending writes.
- Files: `internal/decision/logger.go:92-117`
- Trigger: Calling `Close()` while entries are still being written. The `select` can pick `<-l.closeCh` while items are buffered in `l.ch`.
- Workaround: Call `Flush()` before `Close()`.

**Diff summary misclassifies files on git error:**
- Symptoms: Ship phase shows incorrect file stats (all files shown as "modified").
- Files: `internal/workflow/diff_summary.go:24`
- Trigger: When `git diff --name-status` fails (e.g., shallow clone, detached HEAD).
- Workaround: None.

**Truncated messages can become nil:**
- Symptoms: Agent loop sends empty context to LLM, producing garbage responses.
- Files: `internal/tui/app_update_commands.go:267`
- Trigger: When `TruncateMessagesForLLM` fails (e.g., context length estimation error).
- Workaround: None.

**Unchecked JSON serialization:**
- Symptoms: Plan output is empty/malformed with no error signal.
- Files: `internal/workflow/plan.go:436`
- Trigger: When tasks contain non-serializable values.
- Workaround: None.

**Filelist nil dereference on sort:**
- Symptoms: Panic when sorting by "modified" time and `DirEntry.Info()` fails.
- Files: `internal/tools/filelist.go:201`
- Trigger: Filesystem permission issues or broken symlinks during directory listing.
- Workaround: None.

**Agent loop iterator leak on error:**
- Symptoms: HTTP connection to LLM provider not closed on early return.
- Files: `internal/tui/streaming/agent_loop.go:203-235`
- Trigger: When `ChatCompletionStream` succeeds but a later error causes early return.
- Workaround: None. Connection leaks until GC closes it.

**Double-close risk on iterator:**
- Symptoms: Potential undefined behavior from closing an already-closed resource.
- Files: `internal/tui/streaming/agent_loop.go:210-217`
- Trigger: Context cancellation goroutine calls `iterator.Close()` while main loop also calls it.
- Workaround: None. Usually safe because `Close()` is idempotent for HTTP connections.

## Security Considerations

**Bash command blocklist bypass:**
- Risk: Destructive commands can bypass the string-matching blocklist using shell syntax variations.
- Files: `internal/tools/bash.go:418-490`
- Current mitigation: Pattern matching on normalized lowercase command string.
- Recommendations: Parse command into tokens before matching. Add more patterns. Consider using a shell parser (like `mvdan/sh`) for robust AST-based analysis.

**HTTPCheck SSRF protection was missing (now fixed):**
- Risk: LLM could be prompted to access internal services, cloud metadata endpoints, or RFC1918 addresses.
- Files: `internal/tools/httpcheck.go:26-50`
- Current mitigation: `newSSRFProtectedTransport()` now provides DNS pinning, private IP blocking, and reserved IP checks.
- Recommendations: Verify the implementation covers all edge cases (IPv6, DNS rebinding on redirects).

**Fragile error string matching for security decisions:**
- Risk: Security-relevant error detection uses `strings.Contains(err.Error(), ...)` which breaks if error messages change.
- Files: `internal/git/git.go:231`, `internal/workflow/intent.go:89`, `internal/tools/webfetch.go:415`
- Current mitigation: Patterns are reasonably stable.
- Recommendations: Use `errors.Is()` with typed sentinel errors where possible.

**Unsafe type assertions on sync.Map values:**
- Risk: Panic if stored value type doesn't match assertion. Multiple `sync.Map.Load` calls followed by unchecked type assertions.
- Files: `internal/tools/webfetch.go:106` (`tcpConn.RemoteAddr().(*net.TCPAddr)` without comma-ok), `internal/tools/dns_cache.go:63,100,113,130`
- Current mitigation: Types are controlled internally, so mismatch is unlikely.
- Recommendations: Use comma-ok type assertions everywhere. Add defensive checks.

**Environment variable leakage risk:**
- Risk: API keys read from environment via `os.Getenv()` are process-wide and visible to any goroutine.
- Files: `internal/config/loader.go:789-820`
- Current mitigation: Keys are passed to keychain storage (`pkg/keychain/`) and not written to disk in plaintext.
- Recommendations: Clear env vars after loading into keychain. Use process-specific keychain access.

## Performance Bottlenecks

**O(n^2) string building in execute phase:**
- Problem: Building `planCtx` via string concatenation in nested loop allocates quadratic intermediate strings.
- Files: `internal/workflow/execute.go:741-746`
- Cause: `planCtx += fmt.Sprintf(...)` pattern.
- Improvement path: Replace with `strings.Builder`.

**O(n^2) codemap output generation:**
- Problem: Building codemap output via string concatenation in nested loop.
- Files: `internal/tools/codemap.go:119-133`
- Cause: `output += fmt.Sprintf(...)` pattern.
- Improvement path: Replace with `strings.Builder`.

**Context.Background() in hot paths:**
- Problem: `context.Background()` used in `buildSystemPrompt()` and compaction contexts, making them uncancellable.
- Files: `internal/workflow/context_builder.go:94`, `internal/workflow/engine.go:811,942`
- Cause: Convenience, but creates contexts that don't respect workflow cancellation.
- Improvement path: Pass parent context through; use `context.WithTimeout` derived from workflow context.

**Global mutex contention in codeintel:**
- Problem: `goModulePathCacheMu` (`internal/codeintel/graph.go:331`) is a package-level mutex serialized across all graph operations.
- Files: `internal/codeintel/graph.go:331`
- Cause: Shared cache for Go module path resolution.
- Improvement path: Use sharded lock or per-package locks.

## Fragile Areas

**Engine state management:**
- Files: `internal/workflow/engine.go`
- Why fragile: Engine holds ~20 mutable fields accessed from multiple goroutines (TUI, workflow, tool dispatcher). Only some have mutex protection. The transition from "Engine owns everything" to "WorkflowState extracted" is incomplete.
- Safe modification: Always hold `modelIDMu` or `perPhaseModelsMu` when accessing model fields. Never mutate `WorkflowState` from goroutines outside the workflow goroutine.
- Test coverage: Good unit coverage but race detector finds issues.

**Dispatcher permission flow:**
- Files: `internal/tools/dispatcher.go`, `internal/tools/permissions.go`
- Why fragile: Complex channel-based permission system with batch approvals, rate limiting, concurrency control, and question routing. Multiple mutex types (`sync.RWMutex`, `sync.Map`, atomic counters) interact.
- Safe modification: Always hold `mu` for tool map access; `batchMu` for batch approvals. Never call callbacks under lock.
- Test coverage: Thorough, including concurrency tests.

**TUI-Bubble Tea state:**
- Files: `internal/tui/app.go`, `internal/tui/sidebar_model.go`
- Why fragile: Bubble Tea is strictly single-threaded. All state mutations go through `Update()`. Goroutines communicate via channels (`ch <- Msg`). Any mutation outside `Update()` is a bug.
- Safe modification: Use `tea.Cmd` for async operations. Never mutate `AppState` from goroutines.
- Test coverage: Channel tests and integration tests exist.

**Workflow state machine:**
- Files: `internal/workflow/state_machine.go`, `internal/workflow/engine.go`
- Why fragile: Seven-phase workflow with checkpoint/resume. State transitions must be atomic. Compaction, intent classification, and context building all interact with state.
- Safe modification: Always use `transitionMu` for phase transitions. Don't call compaction during transitions.
- Test coverage: State machine tests exist but complex interactions are hard to cover.

## Scaling Limits

**Subagent concurrency:**
- Current capacity: Max 50 total subagents (`MaxTotalSubagents = 50`), max depth 2.
- Limit: Each subagent runs a full LLM conversation loop. 50 concurrent subagents = 50 simultaneous API calls + 50 tool dispatchers.
- Scaling path: Profile memory usage with many subagents. Consider connection pooling for API calls.

**Tool execution concurrency:**
- Current capacity: Max 8 concurrent tool executions (`MaxConcurrentTools`), rate limit 10/sec normal, 2/sec dangerous.
- Limit: LLM API rate limits and network I/O.
- Scaling path: Already well-bounded. No immediate scaling needed.

**Session storage:**
- Current capacity: Sessions stored as flat JSON files in `.m31a/` directory. Single manager with file locking.
- Limit: File locking serializes all session operations. Large sessions (>1000 messages) may have slow load/save.
- Scaling path: Consider SQLite or BoltDB for session storage if performance becomes an issue.

## Dependencies at Risk

**charmbracelet/bubbletea:**
- Risk: Core TUI framework. Major version updates can break rendering.
- Impact: Entire TUI layer depends on it.
- Migration plan: No immediate alternative. Monitor for breaking changes.

**charmbracelet/glamour:**
- Risk: Markdown rendering library. Used for rendering LLM responses and help text.
- Impact: Rendering quality affects UX.
- Migration plan: Could replace with simpler renderer if needed.

**odvcencio/gotreesitter:**
- Risk: Tree-sitter Go bindings. CGO dependency (build constraint `CGO_ENABLED=0` excludes it).
- Impact: Code intelligence features degrade gracefully without it (falls back to regex parsing).
- Migration plan: Already handled via build tags.

**pkoukk/tiktoken-go:**
- Risk: Token counting library. Used for context window management.
- Impact: Incorrect token counts could cause context overflow or premature truncation.
- Migration plan: Could use alternative tokenizer if accuracy issues found.

## Missing Critical Features

**Ghost mode (headless workflow execution):**
- Problem: `runHeadlessWorkflow()` is a stub. Full workflow cannot run without TUI.
- Blocks: CI/CD integration, batch operations, automation scripts, any non-interactive use.

**Hunk-level rollback:**
- Problem: Rollback is file-level only (`pkg/rollback/`). No ability to revert specific changes within a file.
- Blocks: Surgical undo of partial file changes. Users must revert entire files.

**Behavioral verification:**
- Problem: Acceptance criteria checking is text-substring based (`internal/workflow/engine_verify.go`). Cannot execute commands or hit endpoints to verify behavior.
- Blocks: Runtime correctness verification. Only syntactic checks possible.

## Test Coverage Gaps

**E2E test gaps:**
- What's not tested: Full workflow execution with real LLM (skipped without API keys). Ship phase with real git operations. Compaction during active streaming.
- Files: `e2e_test.go` (skipped in CI), `internal/workflow/ship_test.go`
- Risk: Regression in end-to-end workflow flow.
- Priority: High

**Race condition detection:**
- What's not tested: Full concurrent access patterns between TUI and workflow goroutines under load.
- Files: All files in `internal/workflow/`, `internal/tui/`
- Risk: Data races only caught by `go test -race`, not in normal test runs.
- Priority: High

**Security edge cases:**
- What's not tested: Bash command blocklist bypass patterns (tab obfuscation, split args). HTTPCheck SSRF with IPv6 addresses. DNS rebinding with fast TTL changes.
- Files: `internal/tools/bash.go:418-490`, `internal/tools/httpcheck.go`, `internal/tools/websearch.go`
- Risk: Security bypass in production.
- Priority: Medium

**Error path coverage:**
- What's not tested: Behavior when session persistence fails (LoadProject returns error). Behavior when git operations fail mid-workflow. Behavior when LLM provider returns malformed JSON.
- Files: `internal/workflow/ship.go:251`, `internal/workflow/diff_summary.go:24`, `internal/workflow/plan.go:436`
- Risk: Silent data loss or incorrect behavior on error paths.
- Priority: Medium

**Performance regression:**
- What's not tested: Memory allocation patterns for large codebases (codemap, context building). Token estimation accuracy for long conversations.
- Files: `internal/tools/codemap.go`, `internal/workflow/context_builder.go`, `internal/tokens/`
- Risk: OOM or context overflow on large projects.
- Priority: Low

---

*Concerns audit: 2026-07-04*
