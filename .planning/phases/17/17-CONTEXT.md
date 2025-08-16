# Phase 17: Post-Phase-16 Audit Fixes — Context

**Gathered:** 2026-06-03
**Status:** Ready for planning
**Source:** Codebase audit performed after Phase 16 completion

<domain>

## Phase Boundary

This phase fixes 36 issues found by a comprehensive codebase audit
performed on 2026-06-03 after Phase 16 (UX Polish) completion. The
audit read all Go files across `cmd/`, `internal/`, and `pkg/` and
ran `go build`, `go vet`, and `go test` to identify build, test,
correctness, security, and code quality issues.

The fixes are organized into 4 plans across 4 waves:

- **Wave 1** — Critical build/test issues (1 plan, 4 issues)
- **Wave 2** — High severity correctness bugs (1 plan, 9 issues)
- **Wave 1–2 overlap** — Some High issues can run parallel to Critical
- **Wave 3** — Medium severity issues (1 plan, 12 issues)
- **Wave 4** — Low severity polish (1 plan, 11 issues)

</domain>

<decisions>

## Implementation Decisions

### Critical (4)

#### C-1 — go.mod Go version mismatch
- **File:** `go.mod:3`, `internal/workflow/workflow_test.go:124,133,153,166,183`
- **Decision:** `testing.Context()` requires Go 1.24+ but `go.mod` declares `go 1.22`. Either bump `go.mod` to `go 1.24` or replace `t.Context()` with `context.Background()` in tests.
- **Test:** `go vet ./...` must pass with zero errors.

#### C-2 — ModelCache.Refresh singleflight/mutex race
- **File:** `internal/provider/cache.go:46-54`
- **Decision:** `Refresh()` holds `c.mu.Lock()` while entering `singleflight.Do`, which also calls `Set()` that takes `c.mu.Lock()`. Since Go mutexes are not reentrant, concurrent callers can deadlock. Fix: don't hold the outer lock during singleflight, or use a separate `refreshing` flag with atomic operations.
- **Test:** `TestModelCache_Refresh_Concurrent` — spawn 10 goroutines calling `Refresh` simultaneously; assert no deadlock and all get the same result.

#### C-3 — Edit.atomicWrite uses predictable temp filename
- **File:** `internal/tools/edit.go:196`
- **Decision:** `tmpPath := targetPath + ".m31a_edit_tmp"` is predictable and can cause races. Use `crypto/rand` for temp file name like `FileWrite` does.
- **Test:** `TestEdit_AtomicWrite_RandomTempName` — verify temp file name contains random bytes.

#### C-4 — Zen client test failures (3 tests)
- **File:** `internal/provider/zen/client_test.go:390,446,468`
- **Decision:** Three tests fail because error messages don't match expectations:
  1. `TestChatCompletionStream_PaymentError`: Error "no credits: Invalid API key" doesn't contain "payment" or "billing"
  2. `TestChatCompletionStream_ContextError`: Body "context window exceeded" doesn't match `isContextExceeded` patterns
  3. `TestChatCompletionStream_UnexpectedStatus`: `sanitizeProviderError(502)` returns "Provider gateway error" without "502"
  Fix: Update `isContextExceeded` to match "context window exceeded", update error messages to include relevant keywords, or fix tests to match actual behavior.
- **Test:** All three tests must pass.

### High (9)

#### H-1 — isContextExceeded doesn't match "context window exceeded"
- **Files:** `internal/provider/openrouter/client.go:278-281`, `internal/provider/zen/client.go:259-262`
- **Decision:** Add `"context window exceeded"` to the patterns in `isContextExceeded`. This is the actual error text from OpenRouter.
- **Test:** `TestIsContextExceeded_ContextWindowExceeded` — verify the pattern matches.

#### H-2 — Grep.rgWithRG truncation message shows wrong count
- **File:** `internal/tools/grep.go:185`
- **Decision:** When truncated, `count-maxResults` is always 0 because the loop exits when `count >= maxResults`. Track the actual number of skipped matches or use a different calculation.
- **Test:** `TestGrep_RG_TruncationMessage` — create >100 matches, verify message shows correct overflow count.

#### H-3 — Grep.grepPureGo truncation message also wrong
- **File:** `internal/tools/grep.go:280`
- **Decision:** `len(results)-maxResults` is always 0 because `results` is capped at `maxResults`. Same fix as H-2.
- **Test:** Same test as H-2 covers both paths.

#### H-4 — WebFetch creates new http.Client per request
- **File:** `internal/tools/webfetch.go:178`
- **Decision:** Share an `http.Client` across calls (store as a field on `WebFetch` struct). This enables connection reuse and TLS session resumption.
- **Test:** `TestWebFetch_SharedClient` — verify the same client is reused across multiple calls.

#### H-5 — SSEParser doesn't handle trailing whitespace in [DONE]
- **File:** `internal/provider/sse.go:57`
- **Decision:** Trim whitespace from payload before comparing to "[DONE]". Some providers send trailing whitespace.
- **Test:** `TestSSEParser_DoneWithWhitespace` — send "data: [DONE] \n" and verify it returns EOF.

#### H-6 — Glob rg output not sorted before truncation
- **File:** `internal/tools/glob.go:120-121`
- **Decision:** Add `--sort path` to the rg command in `globWithRG` to ensure deterministic output before truncation.
- **Test:** `TestGlob_RG_Sorted` — verify output is sorted.

#### H-7 — Engine.consumeStream doesn't handle non-EOF errors before EOF
- **File:** `internal/workflow/engine.go:484-501`
- **Decision:** Check error before appending chunk delta. If both chunk and error are non-nil, log the partial content and return the error.
- **Test:** `TestConsumeStream_ErrorBeforeEOF` — mock stream that returns chunk then error; verify partial content is handled.

#### H-8 — Engine.verifyTask runs go test without context timeout
- **File:** `internal/workflow/engine.go:1306`
- **Decision:** Use `exec.CommandContext(ctx, ...)` with a timeout derived from the parent context. Default to 5 minutes if no context deadline.
- **Test:** `TestVerifyTask_ContextTimeout` — verify command respects context cancellation.

#### H-9 — ReplModel.SetProvider calls FetchModels synchronously
- **File:** `internal/tui/repl.go:704-744`
- **Decision:** Return a `tea.Cmd` that performs the fetch asynchronously and updates state on completion. This prevents blocking the TUI event loop.
- **Test:** Manual test — switch providers and verify TUI remains responsive.

### Medium (12)

#### M-1 — Grep.rgWithRG stderr not captured
- **File:** `internal/tools/grep.go:127-188`
- **Decision:** Capture stderr from rg and include it in error messages when rg fails for non-exit-code reasons.
- **Test:** `TestGrep_RG_StderrCaptured` — mock rg to write to stderr; verify error message includes stderr content.

#### M-2 — Edit.atomicWrite doesn't fsync
- **File:** `internal/tools/edit.go:196-203`
- **Decision:** Add `tmpFile.Sync()` before `tmpFile.Close()` like `FileWrite` does.
- **Test:** `TestEdit_AtomicWrite_Fsync` — verify fsync is called (can check file metadata timing).

#### M-3 — WebFetch.htmlToMarkdown doesn't handle self-closing tags
- **File:** `internal/tools/webfetch.go:299-314`
- **Decision:** Add handling for self-closing tags like `<br/>`, `<img ... />`, `<hr/>`.
- **Test:** `TestHtmlToMarkdown_SelfClosingTags` — verify self-closing tags are stripped correctly.

#### M-4 — WebFetch SSRF bypass via DNS rebinding (TOCTOU)
- **File:** `internal/tools/webfetch.go:92-108`
- **Decision:** Use a custom `DialContext` that checks the resolved IP at connection time, not just before. This prevents TOCTOU races where DNS could be rebinned between check and connect.
- **Test:** `TestWebFetch_DialContext_IPPinned` — verify the custom DialContext is used.

#### M-5 — config.Load doesn't handle Agents section in merge
- **File:** `internal/config/loader.go:118-249`
- **Decision:** Add `Agents` section handling to `mergeConfig`. Copy agent profiles from overlay to base.
- **Test:** `TestMergeConfig_Agents` — verify agents section is merged correctly.

#### M-6 — TodoWrite uses sessionsDir but other tools use workDir
- **File:** `internal/tools/todo.go:108`
- **Decision:** This is intentional (TodoWrite writes to session directory). No change needed, but document the behavior.
- **Test:** No test needed.

#### M-7 — AskUserQuestion channel send non-blocking
- **File:** `internal/tools/question.go:94-100`
- **Decision:** The non-blocking select with "question channel full" error is intentional to prevent blocking when TUI is busy. No change needed.
- **Test:** No test needed.

#### M-8 — Dispatcher.askPermissionWithAgentDefault has no timeout
- **File:** `internal/tools/permissions.go:163-196`
- **Decision:** Add configurable timeout like `askPermission` has. Use `d.permissionTimeout`.
- **Test:** `TestAskPermissionWithAgentDefault_Timeout` — verify timeout fires.

#### M-9 — Dispatcher.askPermissionFallback has no timeout
- **File:** `internal/tools/permissions.go:198-236`
- **Decision:** Add configurable timeout like `askPermission` has. Use `d.permissionTimeout`.
- **Test:** `TestAskPermissionFallback_Timeout` — verify timeout fires.

#### M-10 — config.Loader.save clears API keys before marshaling
- **File:** `internal/config/loader.go:490-491`
- **Decision:** Copy the struct before clearing keys, or clear keys only in the TOML output, not on the pointer receiver.
- **Test:** `TestConfigSave_PreservesKeys` — verify API keys are preserved in memory after save.

#### M-11 — Grep.grepPureGo opens each file twice
- **File:** `internal/tools/grep.go:228-252`
- **Decision:** Read binary check + scan in one pass. Open once, read header for binary detection, then continue scanning if not binary.
- **Test:** `TestGrep_PureGo_SingleOpen` — verify file is opened only once (can check with strace or mock).

#### M-12 — SSEParser scanner.Buffer initial size
- **File:** `internal/provider/sse.go:21`
- **Decision:** Minor optimization — pre-allocate buffer to expected SSE event size. Low priority, skip if time-constrained.
- **Test:** No test needed.

### Low (11)

#### L-1 — openrouter/sanitizeProviderError HTML stripping O(n²)
- **File:** `internal/provider/openrouter/client.go:377-384`
- **Decision:** Use a single-pass HTML stripper instead of restarting from beginning each iteration.
- **Test:** No test needed (performance only).

#### L-2 — FileWrite.Execute uses "input" variable name
- **File:** `internal/tools/filewrite.go:128`
- **Decision:** Rename to `fileContent` or `existingContent` for clarity.
- **Test:** No test needed.

#### L-3 — loadGitignore only reads root .gitignore
- **File:** `internal/tools/grep.go:286-299`
- **Decision:** Walk up the directory tree to find nested .gitignore files. This is complex; defer to a future improvement.
- **Test:** No test needed.

#### L-4 — matchesGitignore matches against full path
- **File:** `internal/tools/grep.go:302-309`
- **Decision:** Convert patterns to match against relative paths, not full paths.
- **Test:** `TestMatchesGitignore_RelativePath` — verify patterns match relative paths.

#### L-5 — config.atomicWrite doesn't set file permissions
- **File:** `internal/config/loader.go:565`
- **Decision:** Use `os.OpenFile(path, os.O_CREATE|os.O_WRONLY, 0600)` for security-sensitive config files.
- **Test:** `TestAtomicWrite_Permissions` — verify config file has 0600 permissions.

#### L-6 — Registry.SetActive wraps sentinel error misleadingly
- **File:** `internal/provider/registry.go:46-49`
- **Decision:** Use a more descriptive error message or create a new sentinel error for "provider not found".
- **Test:** No test needed.

#### L-7 — FileRead.Execute doesn't use ctx for I/O
- **File:** `internal/tools/fileread.go:102`
- **Decision:** Use `io.Reader` that respects context cancellation. Low priority.
- **Test:** No test needed.

#### L-8 — FileWrite.Execute doesn't use ctx for I/O
- **File:** `internal/tools/filewrite.go:163`
- **Decision:** Same as L-7.
- **Test:** No test needed.

#### L-9 — Grep.grepPureGo doesn't close file on error
- **File:** `internal/tools/grep.go:228-244`
- **Decision:** Use `defer f.Close()` after the first open.
- **Test:** No test needed.

#### L-10 — Bash.Execute waitCh timeout not tied to context
- **File:** `internal/tools/bash.go:177-179`
- **Decision:** The 30-second timeout should be tied to the command's context, not a fixed value.
- **Test:** No test needed.

#### L-11 — Config.Save clears API keys on pointer receiver
- **File:** `internal/config/loader.go:490-491`
- **Decision:** Same fix as M-10 — copy the struct before clearing.
- **Test:** Same test as M-10.

</decisions>

<canonical_refs>

## Canonical References

- `go.mod` — Go version declaration
- `internal/provider/cache.go` — ModelCache implementation
- `internal/tools/edit.go` — Edit tool atomic write
- `internal/provider/zen/client_test.go` — Failing tests
- `internal/provider/openrouter/client.go` — isContextExceeded patterns
- `internal/tools/grep.go` — Grep truncation messages
- `internal/tools/webfetch.go` — WebFetch SSRF and client creation
- `internal/provider/sse.go` — SSE parser
- `internal/tui/repl.go` — SetProvider synchronous fetch
- `internal/workflow/engine.go` — consumeStream and verifyTask

</canonical_refs>

---

*Phase: 17-post-phase-16-audit-fixes*
*Context gathered: 2026-06-03 via codebase audit*
