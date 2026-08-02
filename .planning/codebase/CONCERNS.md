# Codebase Concerns

**Analysis Date:** 2026-08-03

## Tech Debt

**Empty `pkg/` Directory:**
- Issue: `docs/ARCHITECTURE.md` documents `pkg/` as containing 12+ public packages (autodream, arbitrage, bisect, etc.) but the directory is completely empty
- Files: `pkg/` (empty), `docs/ARCHITECTURE.md` (lines 65-79, 88-93)
- Impact: Documentation is misleading; dependency rule "pkg/ must NOT import internal/" is vacuous
- Fix approach: Either populate `pkg/` with intended packages or update documentation to reflect current structure

**Documentation Drift (15+ items):**
- Issue: Architecture documentation references old directory structure before package reorganization
- Files: `docs/ARCHITECTURE.md` (lines 18-55), `docs/ONBOARDING.md` (line 131), `docs/KEYBINDINGS.md` (lines 131, 156-194)
- Impact: Developers following docs will look in wrong locations; onboarding material is inaccurate
- Fix approach: Regenerate all documentation from current codebase structure

**Secret Detection Heuristic is Trivially Evadable:**
- Issue: Ship preflight only checks for 5 prefix strings (sk-, ghp_, glpat-, xoxb-, AKIA) — easily evaded with base64, environment variables, or different key formats
- Files: `internal/engine/workflow/ship_preflight.go` (lines 66-76)
- Impact: Hardcoded secrets can slip through to production commits
- Fix approach: Add regex patterns for PEM keys, JWT tokens, base64 blobs, and env-var references

**Subagent Prompt Injection Defense is Single-Layer:**
- Issue: Subagent prompt injection protection relies solely on system prompt instructions
- Files: `internal/tools/subagent/loop.go` (lines 397-403)
- Impact: Malicious tool outputs could potentially manipulate subagent behavior
- Fix approach: Add output sanitization or delimiter-based trust zones

**Config Plaintext API Key Fallback:**
- Issue: When OS keychain is unavailable, API keys fall back to plaintext config storage
- Files: `internal/core/config/loader.go` (lines 516-534)
- Impact: API keys stored in plaintext on disk in certain environments
- Fix approach: Warn users when using plaintext fallback; document security implications

## Known Bugs

**WebSearch IP Check Bug (SEC-05):**
- Symptoms: Private IP filtering in websearch checks `ips[0].IP` instead of loop variable `ip.IP`, only first IP is ever validated
- Files: `internal/tools/search/websearch.go` (lines 79, 105)
- Trigger: When DNS returns multiple IPs for a host, only the first is checked
- Workaround: None — all IPs except the first bypass the filter

**Router Initialization Race (BUG-30):**
- Symptoms: Main content area not rendering for router-delegated screens
- Files: `internal/ui/tui/router.go` (lines 46-58), `internal/ui/tui/app_screens.go`
- Trigger: `SwitchTo` called before `Register` during Init() — `activeID` never updated
- Workaround: ScreenREPL works (bypasses router); other screens affected
- Status: Fix identified in ISSUES_1.md but verification needed

## Security Considerations

**Bash Sandbox: No Network Isolation (SEC-01/02):**
- Risk: Bash tool can exfiltrate data via curl, wget, nc, python without network restrictions
- Files: `internal/tools/exec/bash.go` (lines 303-344), `internal/tools/exec/bash_sandbox_linux.go`
- Current mitigation: Dangerous command patterns block some network tools but not all
- Recommendations: Add network namespace isolation (`unshare CLONE_NEWNET`) or block outbound network commands

**WebFetch SSRF: No Redirect IP Re-check (SEC-03):**
- Risk: SSRF attacks via redirects to internal/private IPs after initial validation passes
- Files: `internal/tools/search/webfetch.go` (lines 65-70)
- Current mitigation: Initial URL validation only
- Recommendations: Re-check IPs on every redirect; add DNS pinning

**WebFetch: No DNS Pinning (SEC-04):**
- Risk: DNS rebinding attacks where initial resolution returns public IP but subsequent requests resolve to private IP
- Files: `internal/tools/search/webfetch.go` (lines 246-256)
- Current mitigation: None
- Recommendations: Add custom `DialContext` with DNS pinning (copy pattern from `websearch.go`)

**`M31A_SKIP_LANDLOCK=1` Disables All Sandboxing (SEC-09):**
- Risk: Any process can disable Landlock filesystem restrictions with single env var
- Files: `internal/tools/exec/bash_sandbox_linux.go` (line 26)
- Current mitigation: Only for testing purposes
- Recommendations: Gate behind `debug` build tag or remove in production builds

## Performance Bottlenecks

**Engine.go Complexity (1832 lines):**
- Problem: Core workflow engine is extremely large with 50+ methods
- Files: `internal/engine/workflow/engine.go`
- Cause: Accumulated features without decomposition
- Improvement path: Extract into smaller, focused components (e.g., separate pause/resume, cost tracking, context building)

**Token Estimation EMA Calibration:**
- Problem: Token estimator uses exponential moving average that can drift with unusual usage patterns
- Files: `internal/engine/tokens/estimator.go` (lines 289-415)
- Cause: Dynamic calibration based on actual API responses
- Improvement path: Already addressed with EMA clamping + overhead estimation; monitor for accuracy

**Large File Count (56+ tool files):**
- Problem: Tool implementations spread across many files making discovery harder
- Files: `internal/tools/` (56 non-test files)
- Cause: 18 tools with subdirectories for different categories
- Improvement path: Consider tool registry pattern with auto-discovery

## Fragile Areas

**TUI Router Initialization:**
- Files: `internal/ui/tui/router.go`, `internal/ui/tui/app.go`
- Why fragile: Initialization order dependency between `SwitchTo` and `Register` calls
- Safe modification: Always ensure `Register` is called before `SwitchTo`, or use explicit activation
- Test coverage: Limited — router tests don't cover initialization race conditions

**Provider Model Discovery:**
- Files: `internal/integrations/provider/cache.go`, `internal/integrations/provider/capabilities.go`
- Why fragile: Model lists are dynamic from APIs; hardcoded assumptions break with API changes
- Safe modification: Never hardcode model names; always query API
- Test coverage: Good — cache has singleflight + atomic protection

**Bash Command Detection:**
- Files: `internal/tools/exec/bash.go` (lines 301-358)
- Why fragile: Regex-based pattern matching can be bypassed with obfuscation
- Safe modification: Add new patterns as discovered; consider allowlist approach
- Test coverage: Moderate — test file exists but coverage gaps

## Scaling Limits

**Session Compaction:**
- Current capacity: Automatic compaction when context window fills up
- Limit: Very long sessions may still exceed context after compaction
- Scaling path: Improve compaction algorithms; add manual `/compact` command

**Subagent Depth (2 levels):**
- Current capacity: Subagents can spawn subagents up to depth 2
- Limit: Deeply nested tasks may require more levels
- Scaling path: Make depth configurable per task type

**Tool Concurrency (Dispatcher):**
- Current capacity: Configurable concurrency limits per tool
- Limit: Rate limits from providers may cause unexpected throttling
- Scaling path: Add provider-specific rate limit awareness

## Dependencies at Risk

**Go 1.25.12:**
- Risk: Very recent Go version; may have compatibility issues with older systems
- Impact: Build failures on systems without Go 1.25+
- Migration plan: Consider building with Go 1.24 for wider compatibility

**golang.org/x/sys:**
- Risk: Platform-specific syscall usage (Landlock) may break on kernel updates
- Impact: Sandbox may fail silently on unsupported kernels
- Migration plan: Already handles graceful degradation; add more kernel version checks

**Charmbracelet Libraries:**
- Risk: TUI framework is actively developed; breaking changes possible
- Impact: UI may break on upgrades
- Migration plan: Pin versions; test thoroughly on upgrades

## Missing Critical Features

**MCP (Model Context Protocol) Client:**
- Problem: No support for third-party tool servers via MCP
- Blocks: Cannot use ecosystem of MCP-based tools; competitive disadvantage

**Hooks/Plugin System:**
- Problem: No pre/post tool call hooks for user-defined automation
- Blocks: Cannot wire custom workflows (lint on save, auto-format, etc.)

**Per-File Accept/Reject Review:**
- Problem: Users want file-by-file control before changes land
- Blocks: Trust/transparency for complex changes

**Dry-Run/Preview Mode:**
- Problem: No way to see what would happen before granting permission
- Blocks: Safe experimentation with tool execution

## Test Coverage Gaps

**`internal/tools/search` (2.0% coverage):**
- What's not tested: SSRF protection, webfetch redirect handling, websearch IP filtering
- Files: `internal/tools/search/webfetch.go`, `internal/tools/search/websearch.go`
- Risk: Security-critical code with minimal testing
- Priority: HIGH

**`internal/tools/exec` (0.0% coverage):**
- What's not tested: Bash sandbox, command detection, devserver lifecycle
- Files: `internal/tools/exec/bash.go`, `internal/tools/exec/bash_sandbox_linux.go`
- Risk: Core execution layer untested
- Priority: HIGH

**`internal/tools/fileops` (0.0% coverage):**
- What's not tested: File editing, writing, moving operations
- Files: `internal/tools/fileops/edit.go`, `internal/tools/fileops/filewrite.go`
- Risk: File modification operations untested
- Priority: HIGH

**`internal/ui/tui` (32.4% coverage):**
- What's not tested: Most screen renderers, input handling, streaming
- Files: `internal/ui/tui/app.go`, `internal/ui/tui/app_screens.go`
- Risk: UI layer has limited testing
- Priority: MEDIUM

**`internal/engine/rollback` (53.8% coverage vs 90% target):**
- What's not tested: Edge cases in commit-chain management
- Files: `internal/engine/rollback/` (target is 90% for critical path)
- Risk: Rollback is critical path but below target
- Priority: HIGH

**`internal/tools/ai` (0.0% coverage):**
- What's not tested: AskUserQuestion tool implementation
- Files: `internal/tools/ai/question.go`
- Risk: User interaction tool untested
- Priority: MEDIUM

**`internal/integrations/provider/nvidia` (0.0% coverage):**
- What's not tested: Nvidia NIM provider implementation
- Files: `internal/integrations/provider/nvidia/`
- Risk: Provider integration untested
- Priority: MEDIUM

---

*Concerns audit: 2026-08-03*
