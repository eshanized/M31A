# M31A Developer Experience Audit

## Executive Summary

| Metric | Score |
|--------|-------|
| **Overall DX Score** | 6.5 / 10 |
| **Vibe Coding Score** | 5.5 / 10 |
| **Adoption Readiness** | 5 / 10 |
| **Enterprise Readiness** | 4 / 10 |
| **Daily Driver Score** | 5.5 / 10 |

### Biggest Strengths

- **Exceptional tool safety infrastructure**: SSRF protection, atomic writes, permission system with rate limiting, and dangerous command blocking are best-in-class among terminal AI agents.
- **Code intelligence across 4 languages**: Import graph, symbol indexing, and relevance scoring is a unique differentiator no competitor matches.
- **Context management is sophisticated**: Three-tier agent loop pruning, EMA-calibrated token estimation, AutoDream consolidation, and LLM-powered compaction show deep engineering.
- **Cross-session learning ledger**: The commit-chain rollback, bisect integration, and ledger system for learning across sessions is genuinely novel.
- **Streaming pipeline is clean**: Goroutine ownership model, context-aware cancellation, and panic recovery follow Bubble Tea conventions precisely.
- **33-screen TUI with 11 themes**: The scope of the UI is ambitious and visually polished.

### Biggest Weaknesses

- **Bash tool blocks legitimate shell usage**: The obfuscation blocklist catches `$(`, `${`, and backticks, which are normal shell syntax. The LLM will frequently fail on routine commands.
- **`--goal` headless mode is not implemented**: A headline feature listed in the README is a stub that returns an error.
- **No dedicated git tool**: The most common operation in coding agent workflows requires raw Bash, exposing git to the dangerous-command blocker.
- **Two parallel settings systems with no explanation**: The Settings screen and Config editor are different views of the same data with different field sets and no indication they overlap.
- **Permission modal has invisible timeout**: 10-minute auto-deny with no visible countdown for the first 5 minutes. Users who walk away lose their approval silently.
- **Workflow phases auto-advance without user confirmation**: Discuss->Plan, Execute->Verify transitions happen without asking, making users feel like passengers.
- **Installer has a URL construction bug**: Case mismatch (`M31A` vs `m31a`) and `v`-prefix handling means downloads will 404.

### Would You Recommend M31A Today?

**Not yet for daily use.** M31A has exceptional infrastructure and ambitious architecture, but critical usability bugs (Bash blocking, installer failure, unimplemented features) and workflow friction (auto-advancing phases, no execution control) would frustrate a developer within the first hour. The foundation is strong enough that fixing 15-20 specific issues would make it competitive with Claude Code and Cursor.

---

## Critical Problems

### C1. Bash Tool Blocks Legitimate Shell Syntax
**Severity:** Critical | **File:** `internal/tools/bash.go:478-481`

The obfuscation blocklist catches `$(`, `${`, and backticks -- standard shell syntax used millions of times daily. Commands like `echo $(date)`, `cd ${DIR}`, and `echo $HOME` are blocked. The LLM receives "blocked dangerous command" with no way to understand why `echo $(whoami)` is dangerous.

**Impact:** The LLM will fail on routine shell operations hundreds of times per session. Every developer who uses M31A will hit this immediately.

**Fix:** Remove `$(`, `${`, and backtick from the obfuscation blocklist. These patterns are already handled by the `containsVariableExpansion()` check for actual injection attempts. The obfuscation list should only catch multi-character sequences that require multiple bash features to be dangerous together.

### C2. Installer Downloads Will 404
**Severity:** Critical | **File:** `install.sh:52-113`

The installer constructs URLs as `M31A_1.0.0_linux_amd64.tar.gz` (uppercase project name, `v` stripped), but goreleaser produces `m31a_v1.0.0_linux_amd64.tar.gz` (lowercase, `v` included). Every `curl | bash` installation will fail with a 404.

**Impact:** New users cannot install M31A via the documented one-liner.

**Fix:** Use lowercase project name and keep the `v` prefix in the URL template.

### C3. `--goal` Headless Mode Is Not Implemented
**Severity:** Critical | **File:** `cmd/m31a/main.go:57-59`

The `runHeadlessWorkflow` function is a stub that prints "not yet fully implemented" and returns error code 1. This feature is listed in the README as a key capability.

**Impact:** CI/CD pipelines and automation workflows cannot use M31A's full workflow engine. Users who try `m31a --goal "..."` get a dead end.

### C4. Permission Modal Timeout Is Invisible
**Severity:** Critical | **File:** `internal/tui/components/permission.go:141-142`

The 10-minute auto-deny timeout is only shown when less than 5 minutes remain. For the first 5 minutes, there is zero indication that the modal will auto-deny. A user who walks away returns to find their approval was silently denied.

**Impact:** Security-sensitive decisions are made without full information. Users lose trust in the system.

---

## High Priority Problems

### H1. No Dedicated Git Tool
**Severity:** High | **File:** `internal/tools/bash.go`

Git is the most common operation in coding agent workflows. Without a dedicated tool, the LLM must remember correct git CLI syntax, and git commands are exposed to the dangerous-command blocker. There is no structured output parsing, no branch safety checks, and no integration with the rollback system beyond what Bash provides.

**Fix:** Implement a `Git` tool with structured operations: `git add`, `git commit`, `git diff`, `git log`, `git branch`, `git checkout`, `git stash`. Parse output into structured types the LLM can reason about.

### H2. Bash Tool Lacks `workdir` Parameter
**Severity:** High | **File:** `internal/tools/bash.go:53-69`

The Bash tool only accepts `command` and `timeout`. The working directory is fixed at construction time. The LLM must compose fragile `cd /path && command` chains, which increases failure rates and interacts poorly with the obfuscation blocklist.

**Fix:** Add an optional `workdir` parameter to the Bash tool schema.

### H3. Two Parallel Settings Systems
**Severity:** High | **Files:** `internal/tui/settings_model.go`, `internal/tui/config_model_*.go`

The Settings screen exposes ~17 fields across 6 tabs. The Config editor exposes ~60+ fields across 10 sections. Users accessing "Settings" vs "Config" get different views of the same data with no explanation. The Config editor omits "nvidia" from provider choices despite it being a valid provider.

**Fix:** Unify into a single settings experience. Remove the Settings screen or make it a simplified view that explicitly notes "showing common options -- see Config editor for advanced settings."

### H4. No Input Validation in Settings/Config Editors
**Severity:** High | **Files:** `internal/tui/settings_model.go:440`, `internal/tui/config_model_*.go`

When the user types "abc" into a numeric field, `strconv.Atoi` returns an error that is silently swallowed. The user thinks the value was saved but it was not. No toast, no error message, no visual feedback.

**Fix:** Show inline validation errors when a value fails parsing. Highlight the field in error color and display a toast.

### H5. Workflow Phases Auto-Advance Without User Confirmation
**Severity:** High | **File:** `internal/tui/app_update_phase.go:141-184`

Discuss->Plan, Execute->Verify, and Verify->Runtime transitions happen automatically. The user never gets a chance to review results between phases. The Discuss->Plan transition is especially problematic because the user's answers are saved silently to PROJECT.md and the plan is generated without showing how answers were interpreted.

**Fix:** Add a transition screen between each phase that shows what was accomplished and asks "Proceed to [next phase]? [y/n/r]".

### H6. No Pause/Stop During Execution
**Severity:** High | **File:** `internal/workflow/execute.go:194-687`

The execution loop has no user-facing pause or skip mechanism. The only cancellation is `ctrl+c` which cancels the entire workflow. The user cannot skip a specific task, pause execution to review, or provide mid-execution feedback.

**Fix:** Add pause/resume to the execution loop. Show a "pause" button in the execute screen. When paused, allow the user to review completed tasks, skip pending tasks, or cancel specific tasks.

### H7. Verification Checks Are Shallow
**Severity:** High | **File:** `internal/workflow/engine_verify.go:230-472`

The verify phase checks file existence, git diff, placeholder detection, basic compilation, and test execution. It does not run linters or type-checkers beyond basic compilation. Placeholder detection only checks exact strings like "TODO" and "FIXME" -- it cannot detect empty function bodies, stub implementations, or logic errors. The "suspiciously small" check triggers only for files under 20 bytes.

**Fix:** Add configurable verify commands (build_command, test_command, lint_command). Improve placeholder detection with heuristics for empty function bodies. Increase the "suspiciously small" threshold.

### H8. No Running Cost/Time Display During Workflow
**Severity:** High | **File:** `internal/workflow/engine.go:677-689`

The cost tracker accumulates costs but only checks against a budget limit. It never displays running cost to the user during the workflow. The user has no idea how much a workflow is costing until it completes (if at all).

**Fix:** Show a running cost counter in the workflow sidebar. Update it after each LLM call. Show estimated remaining cost based on current phase.

### H9. Persistent Permissions Are Read-Only
**Severity:** High | **File:** `internal/tools/persistent_permissions.go`

The `PersistentPermissions` struct has a `Load` method but no `Save` method. The "Allow for session" (`A`) option saves to in-memory state but these decisions are lost on restart. The persistent file exists but is never written to by the permission system.

**Fix:** Implement `Save()` on `PersistentPermissions`. After the user selects "Allow for session," persist the rule to `~/.m31a/permissions.json`.

### H10. Workflow Mode Auto-Determination Is Hidden
**Severity:** High | **File:** `internal/tui/app_update_phase.go:17-53`

The intent classification determines the workflow mode (Full/Fast/Direct) before the Discuss phase. The user never sees this classification or the resulting mode choice. The workflow silently skips phases without the user understanding why.

**Fix:** Show the intent classification result and selected mode before starting the workflow. Let the user override it.

---

## Medium Priority Problems

### M1. Home Screen Is a Dead End
The Home screen (logo + input + tips) has no way to return to it from the REPL. Once the user starts a session, the Home screen is unreachable. The tips shown on Home (e.g., `/help getting-started`) are never visible again.

### M2. Discuss Phase One-at-a-Time Questions Are Tedious
The DiscussModel presents questions one at a time with no "answer all at once" or "review and edit all answers" mode. For 3-4 questions, this is fine. But there is no way to see all questions before answering.

### M3. DiscussCompleteness Score Is Hidden from User
The completeness score appears only as a sidebar todo item ("Answer completeness: 30%"). There is no prompt to the user saying "Your answers are incomplete, would you like to answer follow-up questions?"

### M4. Follow-Up Questions Never Wired in TUI
`GenerateFollowUpsIfNeeded()` exists in the engine but is never called after `DiscussCompleteMsg` in the TUI handler. The feature is dead code from the user's perspective.

### M5. Error Messages Mix Developer and User Language
Some messages use technical terms ("exit code 1", "HTTP 429", "bufio.Scanner: token too long", "50K char cap"). Others are user-friendly. There is no consistent voice.

### M6. No Retry-After Countdown Visible to User
When rate limited, the error banner says "retry in a moment" with no time estimate. The `Retry-After` header value is parsed but never surfaced.

### M7. Error Banners Have No Interactive Actions
Errors appear as informational dead ends. There is no "press R to retry" or "press S to switch provider" mechanism.

### M8. Double Error Display
Stream errors during an agent loop show both an error banner AND a potential toast notification. Two error-related messages simultaneously is noisy.

### M9. Rate Limiting Is Invisible
The rate limiter blocks tool execution by waiting on a channel. From the user's perspective, tools just appear to hang or fail without explanation.

### M10. FileRead Dual Semantics for `limit`/`offset`/`max_lines` Are Confusing
The `limit` parameter changes meaning depending on whether `offset` is set (bytes vs lines). Three parameters interact in non-obvious ways.

### M11. Tool Descriptions Are WHAT-Focused Rather Than WHEN-Focused
Descriptions say what each tool does but not when to use it. Claude Code's descriptions are more prescriptive about WHEN to use each tool, which helps the LLM choose correctly.

### M12. Agent Tool Has No Cancellation or Progress Query
Once spawned, a subagent cannot be cancelled, queried for intermediate progress, or given additional instructions. The parent is blind to intermediate state.

### M13. SSE Stream Timeout Produces Opaque Errors
The watchdog timer closes the connection after 30 seconds, but the error message to the user is "connection reset by peer" instead of "stream timed out after 30 seconds."

### M14. `/fallback` Command Suggested But Does Not Exist
When all providers are unhealthy, the error banner suggests `/fallback` which is not a documented command.

### M15. Provider Name Displayed Twice in Error Banners
`renderErrorBanner` adds the provider suffix, then `makeErrorBannerMsg` adds it again, producing "openrouter (openrouter)."

### M16. No JSON Output Mode for Headless
CI/CD pipelines need structured output (JSON with model, tokens_used, latency_ms). Currently only raw text is produced.

### M17. Session Resume Screen Shows At Most 1 Session
The single-session-per-project model means the resume screen's search, scroll, and filter features are dead weight. The designed-for-browsing UI is overkill for one item.

### M18. No Confirmation Before Resuming a Session
Pressing Enter on a session immediately navigates to the REPL with no "Resume session X?" confirmation.

### M19. No Keychain Availability Indicator
There is no way for the user to check whether their keychain is working from within the TUI. The first-run wizard has a toggle but does not test the keychain.

### M20. Ship Phase Makes Unnecessary LLM Call for "Demonstration"
After everything is done, the Ship phase calls the LLM to generate a walkthrough document. This adds latency when the user just wants to ship and leave. There is no opt-out.

---

## Low Priority Problems

### L1. Toast Auto-Dismiss Timing Inconsistency
The `Toast` struct comment says default is 5 seconds, but the actual default is 3 seconds.

### L2. Toast Actions Are Decorative
Toast action buttons render but have no keyboard handler to trigger them.

### L3. NormalizeCommand Compiles Regex on Every Call
`regexp.MustCompile` is called inside a function that runs on every Bash tool invocation.

### L4. Session Age Display Is Imprecise
A 59-minute session shows "59m" but a 61-minute session jumps to "1h". No relative date or absolute timestamp option.

### L5. Duplicate `/model` Entry in Default Help Sections
The help screen lists `/model` twice with different descriptions.

### L6. Footer Hints Disappear Below 60 Columns
New users on narrow terminals have zero guidance.

### L7. No Mouse Documentation
Mouse support (scroll, click, drag) is undocumented in KEYBINDINGS.md and the help screen.

### L8. Many Screens Lack Footer Hints
FileExplorer, ToolDetail, Decisions, and Diff screens have no footer hints.

### L9. `popScreen()` Falls Back to REPL Unconditionally
Pressing Esc repeatedly from Home lands in the REPL, not back where the user started.

### L10. Edit Fuzzy Match Is Effectively Unreachable
The fuzzy strategy has a confidence range of 0.5-0.8 but the default threshold is 0.8, making it unreachable in practice.

---

## Missing Features

### MF1. No Ghost Mode (Listed in Roadmap)
The README mentions "Ghost mode -- fully headless runs that produce a structured diff without touching the TUI" as a roadmap item. This would be a major DX win for automation.

### MF2. No Deferred Tools (Listed in Roadmap)
"Deferred tools -- queue tool calls that require human approval for batch review" would solve the permission modal fatigue problem.

### MF3. No `--json` Output Mode
CI/CD integration requires structured output. `--json` flag that produces `{model, tokens, latency, result}` would make M31A usable in pipelines.

### MF4. No Config Schema or Validation API
There is no TOML schema or `m31a config validate` command. Users discover invalid config only when things break.

### MF5. No `m31a config init` Command
New users must manually create `~/.m31a/config.toml` or rely on the first-run wizard. A `config init` command that generates a template with comments would improve onboarding.

### MF6. No Session Branching
Users cannot maintain multiple work streams in the same repo (e.g., "fix bug A" and "implement feature B"). The single-session model is too restrictive.

### MF7. No Progress Bar During Workflow Execution
The sidebar tracks tasks but there is no integrated progress bar or task checklist in the main content area during execution.

### MF8. No Image Analysis Tool
Neither Claude Code nor Cursor have this, but it would be a differentiator for frontend work (screenshots, design mockups).

### MF9. No Lint/Format Auto-Fix Tool
A dedicated tool that auto-detects the formatter (gofmt, prettier, rustfmt, black) and returns structured diff output would reduce LLM burden.

---

## Workflow Friction

### WF1. Seven Phases Is Too Many for Simple Tasks
For "fix this typo" or "add a comment," going through Initialize->Discuss->Plan->Execute->Verify->Runtime->Ship is absurd. The `direct` mode exists but is not the default and the user must know to use it.

### WF2. Discuss Phase Feels Like Bureaucracy
For experienced developers who know exactly what they want, the Discuss phase is an obstacle. The questions are often obvious ("What should the API endpoint be?" when the user already said "add /api/users").

### WF3. Plan Phase Cannot Be Skipped Easily
The `fast` mode skips Plan, but the user must know about workflow modes before starting. There is no "skip to execution" option once a workflow begins.

### WF4. Verify Phase Runs Automatically After Execute
The user cannot review execution results before verification starts. If the execution produced wrong output, verification runs on bad code.

### WF5. Runtime Phase Is Surprising
The "Runtime" phase starts a dev server and runs HTTP smoke tests. This is unexpected behavior for a phase called "Runtime." Users expect execution monitoring, not server lifecycle management.

### WF6. Ship Phase Forces a Commit
The Ship phase always commits. There is no "review changes without committing" option. Users who want to inspect the diff before committing must cancel the workflow.

---

## UX Problems

### UX1. 33 Screens Creates Navigation Complexity
With 33 screens, the navigation graph is unwieldy. The breadcrumb system helps but is empty for overlay screens (Permission, ModelSelector).

### UX2. The "s" Key to Skip Provider Setup Leads to Confusing State
Pressing `s` on provider selection produces a model picker with no provider and a bare text input with no guidance. The user has no idea what model ID is valid.

### UX3. Done Screen Shows No Summary of Configuration
The first-run wizard's Done screen shows instructions but no summary of what was configured (which provider, which model).

### UX4. First-Visit Tour Is Plumbed But Not Auto-Shown
The `firstVisit` flag exists but the auto-tour trigger is not wired up. The getting-started tour must be manually triggered with `/getting-started`.

### UX5. Queue Depth Indicator Creates Pressure
Showing "N tools queued behind this one" in the permission modal pressures users to approve quickly. This is a dark pattern for security-sensitive prompts.

### UX6. No Risk Labels on Permission Requests
Risk level is communicated only through border color. Users with color vision deficiencies cannot distinguish safe/dangerous/destructive requests.

### UX7. "Approve All" and "Allow Always" Are Ambiguously Named
Both options are shown with identical styling. The semantic difference (per-command vs per-tool+risk-level) is not explained.

### UX8. Permission Queue Depth Pressure
Multiple queued tool approvals create urgency that discourages careful review.

---

## CLI/TUI Problems

### CLI1. Exit Codes Are Undifferentiated
Both "no provider" and "streaming error" return exit code 1. CI/CD cannot distinguish configuration errors from runtime failures.

### CLI2. Headless Stream Errors Produce Exit 0
If `stream.Next()` returns an error, the loop breaks silently and the caller gets exit code 0 despite the error.

### CLI3. No Verbose/Debug Mode for Installer
No `--dry-run` or verbose mode for debugging installation failures.

### CLI4. No Post-Install Smoke Test
The installer does not verify the installed binary actually runs (`m31a --version`).

### CLI5. Terminal Too Narrow Message Is Unhelpful
Below 40 columns, the TUI shows "Terminal too narrow" but no suggestion to resize or use a different terminal.

### CLI6. Resize Re-render Is Not Debounced
Dragging the terminal edge triggers 30+ model re-renders per pixel, causing frame drops.

---

## Permission Problems

### PR1. Permission Modal Timeout Is Invisible for First 5 Minutes
No countdown, no warning bar, no indication that auto-deny is approaching.

### PR2. "Allow for Session" Scope Is Unclear
The modal says "Allow for session" but does not clarify what "session" means. In reality, it remembers the exact command string, not the tool.

### PR3. Sandbox Failure Is Silently Swallowed
If bash sandboxing fails, the user has no indication that security restrictions were lifted.

### PR4. "Approve All" Can Approve Destructive Operations
Batch approval for a tool+risk-level can inadvertently approve future destructive operations the user has not seen.

---

## Context Problems

### CTX1. Pruning Does Not Reduce Token Estimates
Tool results are pruned by setting `SkipForLLM=true` but the estimator still counts them. The 70%/80%/95% thresholds are based on total tokens, not useful tokens, causing premature compression.

### CTX2. AutoDream Summary Is Extractive, Not Abstractive
The consolidation picks representative snippets but does not use an LLM to summarize. Important details from early messages are lost.

### CTX3. Critical Context Detection Is Fragile
`containsCriticalContext` uses case-insensitive string matching on 8 hardcoded markers. A message saying "I have no plan" would be incorrectly protected.

### CTX4. No Error History View
Errors appear in the REPL but there is no dedicated error log. Transient errors that auto-dismiss are gone forever.

---

## Performance Problems

### PRF1. No Resize Debounce at AppState Level
Every `WindowSizeMsg` triggers the full `handleWindowResize()` with all 30+ model updates. Only the REPL model has debounce.

### PRF2. NormalizeCommand Compiles Regex Every Call
`regexp.MustCompile` inside a hot path creates unnecessary allocations.

### PRF3. Model Fetch on First Launch Is Blocking
The first-run wizard fetches models from the API before showing the model browser. If the API is slow, the user waits without feedback.

---

## Reliability Problems

### REL1. Workflow State Silently Lost on Corrupted Session
If `session.json` is corrupted, workflow state silently resets to idle with no error message.

### REL2. Messages Stored Separately from Metadata
If one of `session.json` or `messages.json` is corrupted, the session partially loads with no indication of data loss.

### REL3. Bisect Heal Can Produce Wrong Results
If the verify check is flaky, `git bisect` produces incorrect results with no retry mechanism.

### REL4. Compaction Has No Retry on API Failure
If the LLM call for compaction fails, compaction fails entirely with no fallback.

---

## Error Handling Problems

### ERR1. Bash Exit Codes Shown Without Translation
The user sees "exit code 1" with no indication of what failed. Common codes (126=permission denied, 127=command not found) should be translated.

### ERR2. Default Error Path Loses All Context
Unrecognized errors produce "An unexpected error occurred -- check the logs" with no provider or model info. Users cannot debug.

### ERR3. "Check the Logs" Is Unhelpful in TUI Context
Many error messages say "check the logs" but the log file path is not shown and users may not know where to find it.

### ERR4. Rate Limiting Errors Are Invisible
The rate limiter silently blocks tool execution. When context is cancelled, the user sees "Request cancelled" instead of understanding that rate limiting caused the failure.

### ERR5. Stream Timeout Produces Opaque Connection Errors
The SSE watchdog closes the connection after 30 seconds, but the error is "connection reset by peer" instead of "stream timed out."

---

## Onboarding Problems

### ONB1. First-Run Wizard Skips to Confusing State
The `s` key to skip provider setup produces a model picker with no provider and no guidance.

### ONB2. API Key Validation Is Format-Only
The wizard checks string prefix patterns but does not attempt an API call. Users discover invalid keys only when they try to use M31A.

### ONB3. Model Browser Depends on Successful API Call
If model fetch fails, the user sees "Enter a model ID manually" but has no way to know valid IDs.

### ONB4. No Post-Setup Summary
The Done screen shows instructions but no confirmation of what was configured.

### ONB5. Getting-Started Tour Is Not Auto-Shown
The `firstVisit` flag exists but the auto-tour trigger is not wired up.

---

## Documentation Problems

### DOC1. KEYBINDINGS.md Does Not Document Mouse Support
Mouse capabilities (scroll, click, drag) are undocumented.

### DOC2. No Config Schema Documentation
The config reference exists but there is no TOML schema or generated documentation from code.

### DOC3. `/fallback` Command Referenced But Does Not Exist
Error messages suggest `/fallback` which is not a real command.

### DOC4. Tool Descriptions Lack Usage Guidance
Descriptions say WHAT each tool does but not WHEN to use it.

---

## Developer Trust Issues

### TR1. Silent Sandbox Degradation
If bash sandboxing fails, the user has no indication that security restrictions were lifted.

### TR2. Silent Keychain Fallback
When keychain save fails, API keys are written to plaintext config with only a `slog.Warn` (invisible without debug mode).

### TR3. No Audit Trail for Permission Decisions
Permission decisions are not logged. There is no way to review what was approved and when.

### TR4. Persistent Permissions Don't Persist
The "Allow for session" option loses all decisions on restart.

---

## Vibe Coding Issues

### VC1. Too Many Phases for "Just Do It" Tasks
For vibe coding (build something in one evening), the seven-phase workflow is too rigid. The `direct` mode exists but is not discoverable.

### VC2. Discuss Phase Interrupts Flow
Experienced developers who know what they want are forced to answer questions before they can start building.

### VC3. No "Just Build It" Mode
There is no one-command mode that skips all ceremony and just executes. `--prompt` is close but is single-shot with no workflow.

### VC4. Permission Modals Break Flow
Every tool call requires approval (unless "allow always" is set). For vibe coding, this is a constant interruption.

### VC5. No Visual Feedback During Long Operations
During execution, the user sees scrolling tool calls but no overall progress indicator or estimated time remaining.

---

## Competitive Comparison

### What M31A Does Better

| Feature | M31A | Claude Code | Cursor |
|---------|------|-------------|--------|
| SSRF protection | Yes | No | N/A |
| Code intelligence (4 languages) | Yes | No | Yes (limited) |
| Cross-session learning | Yes | No | No |
| Commit rollback chain | Yes | No | No |
| Git bisect integration | Yes | No | No |
| Model arbitrage | Yes | No | No |
| Per-phase model assignment | Yes | No | No |
| 18 built-in tools | Yes | ~12 | ~10 |
| Zero telemetry | Yes | No | No |
| Static binary | Yes | No | No |
| Parallel subagents | Yes | Yes (Task) | No |
| Permission system depth | Excellent | Basic | Basic |

### What Competitors Do Better

| Feature | Claude Code | Cursor | OpenCode |
|---------|-------------|--------|----------|
| Installation | `npm install -g` | App store | `go install` |
| First-launch experience | Instant | Instant | Good |
| Git integration | Dedicated tool | Panel | Bash |
| Permission flow | Simple approve | N/A | Simple |
| Error messages | Clear, actionable | Clear | Decent |
| Context window management | Automatic | Automatic | Manual |
| Stream timeout UX | Shows timeout | Shows timeout | Shows timeout |
| Progress indicators | Excellent | Excellent | Good |
| Pause/stop execution | Yes | Yes | No |
| JSON output for CI | Yes | No | No |

### What Users Will Notice Immediately

1. **Bash blocking of normal commands** -- will be the first thing any developer hits
2. **No git tool** -- forces raw Bash for the most common operation
3. **Seven-phase workflow for simple tasks** -- feels like bureaucracy
4. **Permission modal pressure** -- queue depth indicator creates urgency
5. **No execution control** -- cannot pause, skip, or modify during execution
6. **Error messages are sometimes cryptic** -- "exit code 1" with no context
7. **Settings confusion** -- two different settings screens with different fields

---

## Top 100 Improvements

| # | Title | Severity | Why Developers Care | Suggested Solution | Expected Impact | Complexity | Priority |
|---|-------|----------|--------------------|--------------------|----------------|------------|----------|
| 1 | Fix Bash obfuscation blocklist | Critical | Blocks normal shell usage | Remove `$(`, `${`, backtick from blocklist | Eliminates most false positives | Low | P0 |
| 2 | Fix installer URL construction | Critical | `curl \| bash` fails with 404 | Match goreleaser template exactly | Enables installation | Low | P0 |
| 3 | Implement `--goal` headless mode | Critical | Headline feature is a stub | Complete the implementation | Enables CI/CD usage | High | P0 |
| 4 | Fix permission modal timeout visibility | Critical | 10-min auto-deny is invisible | Show countdown bar from start | Builds user trust | Low | P0 |
| 5 | Add `workdir` parameter to Bash tool | High | Forces fragile `cd` chains | Add optional workdir to schema | Reduces command failures | Low | P1 |
| 6 | Implement dedicated Git tool | High | Git is most common operation | Add structured git operations | Improves reliability | Medium | P1 |
| 7 | Unify settings and config editor | High | Two systems confuse users | Merge into single experience | Reduces confusion | Medium | P1 |
| 8 | Add input validation to settings | High | Invalid values silently discarded | Show inline validation errors | Prevents silent failures | Low | P1 |
| 9 | Add phase transition confirmation | High | Auto-advance removes user control | Show transition screen between phases | Restores user agency | Medium | P1 |
| 10 | Add pause/stop to execution | High | No control during workflow | Add pause button in execute screen | Enables mid-execution review | Medium | P1 |
| 11 | Add running cost display | High | Users don't know costs | Show cost counter in sidebar | Enables cost awareness | Low | P1 |
| 12 | Implement persistent permission save | High | "Allow for session" is lost on restart | Add Save() to PersistentPermissions | Remembers user decisions | Low | P1 |
| 13 | Show workflow mode to user | High | Mode auto-determination is hidden | Display classification before start | Builds transparency | Low | P1 |
| 14 | Translate bash exit codes | High | "exit code 1" is unhelpful | Map common codes to descriptions | Improves error messages | Low | P1 |
| 15 | Add human-readable stream timeout | High | "connection reset" is opaque | Catch watchdog timeout specifically | Clarifies streaming errors | Low | P1 |
| 16 | Fix provider name double-display | Medium | "openrouter (openrouter)" is wrong | Remove duplicate provider suffix | Fixes visual bug | Low | P2 |
| 17 | Make error messages consistent | Medium | Mixed developer/user language | Define voice guidelines | Improves readability | Medium | P2 |
| 18 | Add retry-after countdown | Medium | "retry in a moment" is vague | Show Retry-After value to user | Sets expectations | Low | P2 |
| 19 | Add interactive error actions | Medium | Errors are dead ends | Add "R to retry" / "S to switch" | Enables quick recovery | Medium | P2 |
| 20 | Fix FileRead dual semantics | Medium | `limit` changes meaning | Separate byte/line modes | Reduces confusion | Medium | P2 |
| 21 | Improve tool descriptions | Medium | WHAT not WHEN guidance | Add usage context to descriptions | Better LLM tool selection | Low | P2 |
| 22 | Add agent cancellation/progress | Medium | Subagents are fire-and-forget | Add cancel/status methods | Enables control | Medium | P2 |
| 23 | Fix SSE timeout error message | Medium | Watchdog produces opaque errors | Classify timeout errors | Improves debugging | Low | P2 |
| 24 | Fix `/fallback` command reference | Medium | Error suggests non-existent command | Use real command name | Prevents confusion | Low | P2 |
| 25 | Add JSON output mode | Medium | CI/CD needs structured output | Add `--json` flag | Enables pipeline integration | Medium | P2 |
| 26 | Improve session resume UX | Medium | Search/scroll dead for 1 session | Add session count to UI | Manages expectations | Low | P2 |
| 27 | Add session resume confirmation | Medium | Accidental resume is instant | Show confirmation dialog | Prevents mistakes | Low | P2 |
| 28 | Add keychain health check | Medium | Keychain failure is silent | Add health check in settings | Builds trust | Low | P2 |
| 29 | Skip Ship LLM call option | Medium | Demo generation adds latency | Add opt-out flag | Saves time | Low | P2 |
| 30 | Reduce Discuss phase friction | Medium | One-at-a-time is tedious | Add "answer all" mode | Speeds up flow | Medium | P2 |
| 31 | Show DiscussCompleteness to user | Medium | Score is hidden | Show score in discuss UI | Informs decisions | Low | P2 |
| 32 | Wire follow-up questions in TUI | Medium | Feature is dead code | Connect to discuss handler | Completes feature | Low | P2 |
| 33 | Add risk labels to permissions | Medium | Color-only is inaccessible | Add text risk badges | Improves accessibility | Low | P2 |
| 34 | Explain permission action scope | Medium | "Allow for session" is ambiguous | Add scope description | Prevents surprises | Low | P2 |
| 35 | Remove permission queue pressure | Medium | "N tools queued" creates urgency | Remove or rephrase queue indicator | Reduces pressure | Low | P2 |
| 36 | Fix double error display | Medium | Banner + toast is noisy | Deduplicate error messages | Reduces noise | Low | P2 |
| 37 | Make rate limiting visible | Medium | Tools just hang | Show rate limit status | Explains delays | Medium | P2 |
| 38 | Add error history view | Medium | Transient errors are lost | Add error log screen | Enables debugging | Medium | P2 |
| 39 | Improve verify checks | Medium | Shallow verification | Add configurable commands | Catches more bugs | Medium | P2 |
| 40 | Show plan cost/time estimates | Medium | No preview before approval | Populate estimates before plan review | Informs decisions | Medium | P2 |
| 41 | Add back-navigation after plan approval | Medium | Cannot go back to plan | Add back option during execute | Restores flexibility | Medium | P2 |
| 42 | Improve execute screen interactivity | Medium | Screen is purely passive | Add task-level controls | Enables intervention | High | P2 |
| 43 | Show self-heal details | Medium | Heal info is hidden | Display HealReport in UI | Improves transparency | Low | P2 |
| 44 | Fix context pruning token estimates | Medium | Pruning doesn't reduce estimates | Count only non-pruned tokens | Prevents premature compression | Low | P2 |
| 45 | Add resize debounce | Medium | Frame drops on resize | Debounce at AppState level | Smooths resize | Low | P2 |
| 46 | Document mouse support | Medium | Mouse users have no guidance | Add to KEYBINDINGS.md and help | Enables mouse usage | Low | P2 |
| 47 | Add footer hints to all screens | Medium | Some screens lack hints | Add cases for all 33 screens | Improves discoverability | Low | P2 |
| 48 | Fix Home screen dead end | Medium | Cannot return to Home | Add navigation option | Restores navigation | Low | P2 |
| 49 | Wire first-visit tour | Medium | Tour is plumbed but not triggered | Auto-show on first visit | Improves onboarding | Low | P2 |
| 50 | Add config validation command | Medium | No pre-flight check | Add `m31a config validate` | Catches errors early | Low | P2 |
| 51 | Make Runtime phase name clearer | Medium | "Runtime" is misleading | Rename to "Smoke Test" or similar | Sets correct expectations | Low | P2 |
| 52 | Add post-install smoke test | Low | Installer doesn't verify binary | Run `m31a --version` after install | Catches broken installs | Low | P3 |
| 53 | Fix installer typo | Low | `$BIN_name` is wrong variable | Use `$BIN_NAME` | Fixes success message | Low | P3 |
| 54 | Fix toast timing documentation | Low | Comment says 5s, actual is 3s | Update comment | Prevents confusion | Low | P3 |
| 55 | Make toast actions keyboard-accessible | Low | Action buttons are decorative | Add key handler | Enables interaction | Low | P3 |
| 56 | Fix normalizeCommand regex compilation | Low | Unnecessary allocation | Make regex a package var | Minor perf win | Low | P3 |
| 57 | Improve session age display | Low | "59m" jumps to "1h" | Add relative dates | Better precision | Low | P3 |
| 58 | Fix duplicate /model in help | Low | Copy-paste bug | Remove duplicate | Cleans up UI | Low | P3 |
| 59 | Show footer hints below 60 cols | Low | Narrow terminals have no hints | Add minimal hint bar | Helps narrow terminals | Low | P3 |
| 60 | Add scroll acceleration | Low | 1:1 line scroll is slow for long convos | Add shift+scroll for page | Speeds navigation | Low | P3 |
| 61 | Add half-page scroll to ScrollState | Low | Inconsistent scroll behavior | Add HalfPageUp/Down methods | Consistency | Low | P3 |
| 62 | Add config schema generation | Low | No TOML schema exists | Generate from code | Enables validation | Medium | P3 |
| 63 | Add `m31a config init` command | Low | Users must manually create config | Generate template with comments | Improves onboarding | Low | P3 |
| 64 | Add session branching | Low | Single-session is restrictive | Support multiple sessions per project | Enables parallel work | High | P3 |
| 65 | Add progress bar to execute screen | Low | Progress is only in sidebar | Add integrated progress bar | Improves visibility | Medium | P3 |
| 66 | Add lint/format auto-fix tool | Low | No dedicated formatter | Implement auto-detect tool | Reduces LLM burden | Medium | P3 |
| 67 | Improve install.sh retry logic | Low | No retry on download failure | Add retry with backoff | Handles flaky networks | Low | P3 |
| 68 | Add install.sh sudo elevation | Low | `/usr/local/bin` may need root | Detect and suggest sudo | Prevents permission errors | Low | P3 |
| 69 | Add armv7l support | Low | 32-bit ARM not supported | Add architecture detection | Broader platform support | Low | P3 |
| 70 | Fix VirtualViewport ANSI handling | Low | Rune-by-rune breaks ANSI | Use string-based composition | Prevents rendering bugs | Medium | P3 |
| 71 | Add min/max constraints to Box | Low | Content can shrink to 0 | Add MinWidth/MaxWidth | Prevents artifacts | Low | P3 |
| 72 | Add tab overflow scrolling | Low | Tab bar overflows silently | Add scroll indicator | Handles narrow terminals | Low | P3 |
| 73 | Fix config editor nvidia omission | Low | Config editor omits nvidia | Add nvidia to choices | Consistency | Low | P3 |
| 74 | Fix first-run wizard step numbering | Low | Dots and bar show different counts | Align step numbering | Visual consistency | Low | P3 |
| 75 | Add Done screen configuration summary | Low | No recap of what was configured | Show provider+model summary | Confirms setup | Low | P3 |
| 76 | Test keychain before showing toggle | Low | Keychain may not work | Run health check | Prevents false confidence | Low | P3 |
| 77 | Fix mixed line ending handling | Low | Edge case in Edit tool | Track per-line endings | Correctness | Medium | P3 |
| 78 | Improve `containsCriticalContext` | Low | String matching is fragile | Use NLP or regex patterns | Reduces false positives | Medium | P3 |
| 79 | Add compaction retry logic | Low | LLM failure means no compaction | Add retry with backoff | Improves reliability | Low | P3 |
| 80 | Fix agent loop pruning token estimates | Low | Pruning doesn't reduce estimates | Skip pruned in Estimator | Prevents premature compression | Low | P3 |
| 81 | Add half-page scroll to all screens | Low | Inconsistent scroll behavior | Standardize across screens | Consistency | Medium | P3 |
| 82 | Add keyboard shortcuts to tab bar | Low | Tab switching requires [] keys | Show number keys on tabs | Faster navigation | Low | P3 |
| 83 | Fix screen stack pruning | Low | Stale screens accumulate | Prune on phase completion | Cleaner navigation | Low | P3 |
| 84 | Fix cycle_model_forward/backward | Low | Both open same selector | Implement actual cycling | Matches expectation | Medium | P3 |
| 85 | Add scrollbar width tolerance fix | Low | Extra column captures accidental clicks | Reduce hit area | Prevents accidental scrolls | Low | P3 |
| 86 | Add WebFetch proper HTML parser | Low | Custom parser is fragile | Use golang.org/x/net/html | Better HTML handling | Medium | P3 |
| 87 | Add session schema migration | Low | No migration infrastructure | Add version checking | Future-proofs sessions | Medium | P3 |
| 88 | Add rate limit Retry-After to UI | Low | Wait is invisible | Show countdown | Sets expectations | Low | P3 |
| 89 | Fix permission modal timeout to show earlier | Low | Only shows in last 5 minutes | Show from start | Builds awareness | Low | P3 |
| 90 | Add `--verbose` flag to headless | Low | No debug output in CI | Add stderr progress | Enables debugging | Low | P3 |
| 91 | Fix context warning overlap | Low | Warning + auto-fix + error overlap | Coordinate messaging | Reduces confusion | Medium | P3 |
| 92 | Add slow provider indication | Low | Degraded provider is silent | Show "slow provider" badge | Informs user | Low | P3 |
| 93 | Add model browser offline fallback | Low | Model fetch failure blocks wizard | Cache last successful list | Enables offline setup | Low | P3 |
| 94 | Add workflow phase descriptions | Low | Users don't know what phases do | Show tooltip or description | Improves understanding | Low | P3 |
| 95 | Add cost estimate before plan approval | Low | No cost preview | Estimate from plan complexity | Informs decisions | Medium | P3 |
| 96 | Fix verify report display | Low | Report is on disk only | Show in verify screen | Improves visibility | Low | P3 |
| 97 | Add auto-resume prompt on launch | Low | User must manually navigate | Show "Resume? [y/n]" | Saves time | Low | P3 |
| 98 | Add error classification to all errors | Low | Some errors are generic | Classify all error types | Improves messaging | Medium | P3 |
| 99 | Fix mixed developer/user error voice | Low | Inconsistent error language | Define voice guidelines | Professional feel | Low | P3 |
| 100 | Add `m31a doctor` diagnostic command | Low | No health check tool | Check keychain, provider, config | Catches issues early | Medium | P3 |

---

## Final Verdict

### Would experienced developers switch to M31A?

**Not today.** The infrastructure is exceptional -- SSRF protection, atomic writes, permission system depth, code intelligence, and cross-session learning are genuinely best-in-class. But the experience layer has too many friction points that would frustrate a developer within the first hour:

1. **The Bash tool blocks normal shell usage** -- this alone is a dealbreaker. A developer who cannot run `echo $(date)` or `cd ${DIR}` will abandon M31A immediately.

2. **The installer is broken** -- the documented one-liner will 404. New users cannot even install the tool.

3. **The workflow is too rigid for most tasks** -- seven phases for a simple refactor is bureaucracy, not engineering. The `direct` mode exists but is not discoverable.

4. **No execution control** -- developers cannot pause, skip, or modify during execution. This makes them feel like passengers, not drivers.

5. **The permission system creates pressure** -- queue depth indicators and invisible timeouts undermine trust rather than building it.

### What must change before v1.0?

**Non-negotiable (must fix):**
1. Fix the Bash obfuscation blocklist
2. Fix the installer URL construction
3. Implement `--goal` headless mode or remove it from the README
4. Fix permission modal timeout visibility
5. Add `workdir` parameter to Bash tool

**Should fix (strongly recommended):**
6. Implement a dedicated Git tool
7. Unify settings and config editor
8. Add input validation to settings
9. Add phase transition confirmations
10. Add pause/stop to execution

**Nice to have (improve adoption):**
11. Add JSON output mode
12. Document mouse support
13. Add `m31a config validate`
14. Improve error message consistency
15. Add running cost display

M31A has the architecture of a 9/10 tool but the experience of a 6/10 tool. The gap between infrastructure quality and user-facing polish is the core problem. Fix the 10 critical/high issues listed above and M31A becomes genuinely competitive with Claude Code and Cursor for terminal-native AI coding.
