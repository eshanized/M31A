# REQUIREMENTS.md — M31A

## Phase 1: Bug Fixes (Critical + High)

### BUG-04: C4 — tools/websearch.go:62 index out of bounds on DNS resolution
- **Severity:** CRITICAL
- **File:** `internal/tools/websearch.go:62`
- **Requirement:** Add `len(ips) == 0` guard before indexing
- **Acceptance:** No panic when DNS returns empty slice

### BUG-05: C5 — tools/webfetch.go:91 index out of bounds on DNS resolution
- **Severity:** CRITICAL
- **File:** `internal/tools/webfetch.go:91`
- **Requirement:** Add `len(ips) == 0` guard before indexing
- **Acceptance:** No panic when DNS returns empty slice

### BUG-06: C6 — tools/dispatcher.go:463 unsafe type assertion on sync.Map value
- **Severity:** CRITICAL
- **File:** `internal/tools/dispatcher.go:463`
- **Requirement:** Use comma-ok type assertion
- **Acceptance:** No panic on type mismatch

### BUG-07: C7 — tools/subagent/manager.go:265 unsafe type assertion on sync.Map value
- **Severity:** CRITICAL
- **File:** `internal/tools/subagent/manager.go:265` (and lines 237, 246, 292)
- **Requirement:** Use comma-ok type assertion
- **Acceptance:** No panic on type mismatch

### BUG-08: H1 — Dispatcher.SetCollector writes without lock
- **Severity:** HIGH
- **File:** `internal/tools/dispatcher.go:190-191`
- **Requirement:** Move `d.collector = c` inside `d.mu.Lock()`
- **Acceptance:** No data race between SetCollector and Execute

### BUG-09: H2 — DevServer.restartServer TOCTOU double lock/unlock
- **Severity:** HIGH
- **File:** `internal/tools/devserver.go:283-298`
- **Requirement:** Fix TOCTOU by holding lock across entire operation
- **Acceptance:** No race condition in restartServer

### BUG-10: H3 — tools/dns_cache.go:84-90 non-atomic counter + TOCTOU eviction
- **Severity:** HIGH
- **File:** `internal/tools/dns_cache.go:84-90`
- **Requirement:** Fix non-atomic counter and TOCTOU eviction
- **Acceptance:** Cache eviction is atomic and doesn't exceed maxSize

### BUG-12: H5 — permissions.go:377-385 double decrement of pendingPermCount
- **Severity:** HIGH
- **File:** `internal/tools/permissions.go:377-385`
- **Requirement:** Fix double decrement by removing deferred -1
- **Acceptance:** pendingPermCount doesn't go negative

### BUG-14: H7 — workflow/diff_summary.go:24 unchecked git diff error
- **Severity:** HIGH
- **File:** `internal/workflow/diff_summary.go:24`
- **Requirement:** Check error from `git diff`
- **Acceptance:** Diff summary handles git errors gracefully

### BUG-15: H8 — tui/app_update_commands.go:267 unchecked TruncateMessagesForLLM
- **Severity:** HIGH
- **File:** `internal/tui/app_update_commands.go:267`
- **Requirement:** Check error from `TruncateMessagesForLLM`
- **Acceptance:** Agent loop handles truncation errors

### BUG-16: H9 — workflow/plan.go:436 unchecked json.MarshalIndent
- **Severity:** HIGH
- **File:** `internal/workflow/plan.go:436`
- **Requirement:** Check error from `json.MarshalIndent`
- **Acceptance:** Plan output handles serialization errors

### BUG-17: H10 — tools/httpcheck.go:115-173 SSRF vulnerability
- **Severity:** HIGH
- **File:** `internal/tools/httpcheck.go:115-173`
- **Requirement:** Add SSRF protection by reusing WebFetch's DNS-pinning transport
- **Acceptance:** HTTPCheck blocks private IP and metadata requests

### BUG-19: H12 — tools/dns_cache.go:63,100,113,130 unsafe type assertions on sync.Map
- **Severity:** HIGH
- **File:** `internal/tools/dns_cache.go:63,100,113,130`
- **Requirement:** Use comma-ok type assertions
- **Acceptance:** No panic on type mismatch

---

## Already Fixed (No Action Required)

### BUG-01: C1 — Engine.perPhaseModels concurrent map read/write
- **Status:** FIXED with sync.RWMutex

### BUG-02: C2 — Engine.activePhase/modelID/workflowMode unprotected concurrent access
- **Status:** FIXED with mutex

### BUG-03: C3 — pkg/session/manager.go:350 nil pointer dereference on os.Stat failure
- **Status:** FIXED with nil check

### BUG-11: H4 — decision/logger.go:92-117 data loss on shutdown race
- **Status:** FIXED with proper drain logic

### BUG-13: H6 — tui/app_view.go:898 nil pointer dereference on m.replModel
- **Status:** FIXED with nil check

---

## Phase 4: Technical Debt Elimination

### TECH-01: Workflow Engine Refactoring
- **Category:** Architecture
- **Scope:** `internal/workflow/engine.go` and related files
- **Requirement:** Extract dedicated components from Engine (1600+ lines, 40+ fields)
- **Components to Extract:** PromptBuilder, ContextBuilder, CostTracker, PhaseCoordinator, WorkflowCache, StreamManager, RetryCoordinator, ReceiptWriter, DecisionRecorder, StateMachine, PhaseExecutor
- **Acceptance:** Engine reduced to orchestration only, all components tested, behavior preserved

### TECH-02: TUI Update Refactoring
- **Category:** Architecture
- **Scope:** `internal/tui/` directory
- **Requirement:** Replace giant Update() switch with modular dispatch
- **Architecture:** Dispatcher with MessageHandler interface, one handler per concern
- **Handlers:** handleStream, handleTool, handleWorkflow, handleConfig, handleSidebar, handleRuntime, handleShip, handleVerify, handleModal, handleInput, handleNavigation
- **Acceptance:** Bubble Tea semantics identical, all message types handled, tests pass

### TECH-03: Reflection-Based Config Merge Removal
- **Category:** Safety
- **Scope:** Configuration merging code
- **Requirement:** Remove reflection recursion, implement type-safe merge
- **Approach:** Generated merge code or explicit merge functions
- **Acceptance:** Zero reflection, compile-time safety, all configs merge correctly

### TECH-04: Edit Tool Simplification
- **Category:** Maintainability
- **Scope:** `internal/tools/edit.go` and related files
- **Requirement:** Reduce seven matching strategies to five: Exact, Trimmed, Normalized, Anchor, Optional fuzzy
- **Features:** Confidence scoring for every replacement, configurable thresholds, no silent incorrect replacements
- **Acceptance:** All strategies work, confidence exposed, thresholds configurable

### TECH-05: Token Estimator Replacement
- **Category:** Dependencies
- **Scope:** Token counting/estimation code
- **Requirement:** Replace aging dependency with maintained implementation
- **Providers:** OpenAI, Anthropic, Gemini, OpenRouter, Qwen, DeepSeek, Mistral, Llama
- **Fallback:** Always emit diagnostics, never silently degrade
- **Acceptance:** Accurate estimation for all providers, diagnostics on fallback

### TECH-06: Model Capability Detection
- **Category:** Runtime
- **Scope:** Provider model handling
- **Requirement:** Remove hardcoded provider lists, implement runtime capability detection
- **Features:** Health checks, metadata driven, automatic adaptation
- **Acceptance:** Minimal hardcoded values, capabilities detected at runtime

### TECH-07: Additional Debt Discovery and Cleanup
- **Category:** Code Quality
- **Scope:** Entire repository
- **Requirement:** Search and remove: long files, large structs, large interfaces, duplicate logic, dead code, hidden coupling, magic constants, deep nesting, complex switch statements, long functions, circular dependencies
- **Acceptance:** No unnecessary duplication remains

### TECH-08: Complexity Reduction
- **Category:** Code Quality
- **Scope:** All packages
- **Requirement:** Reduce cyclomatic and cognitive complexity, reduce coupling, increase cohesion
- **Approach:** Composition, dependency injection, immutable value objects, focused packages
- **Acceptance:** Measurable complexity reduction, improved cohesion

### TECH-09: Concurrency Review
- **Category:** Safety
- **Scope:** All concurrent code
- **Requirement:** Review every goroutine, channel, mutex, atomic, sync.Map, context cancellation, timeout, retry, race possibility
- **Acceptance:** Deterministic behavior, proper shutdown ordering, resource cleanup

### TECH-10: Memory Review
- **Category:** Performance
- **Scope:** All memory allocations
- **Requirement:** Review allocations, escape analysis, object reuse, pooling
- **Acceptance:** No leaks, optimized allocations

### TECH-11: Performance Review
- **Category:** Performance
- **Scope:** All performance-critical paths
- **Requirement:** Review filesystem, git, network, streaming, provider selection, prompt generation, context creation, tree sitter, search, workflow, model cache, permission lookup, metrics, rendering, startup, shutdown
- **Acceptance:** Performance improved or maintained without sacrificing readability

### TECH-12: Error Handling Standardization
- **Category:** Reliability
- **Scope:** All error paths
- **Requirement:** Standardize error paths, prefer sentinel errors, preserve wrapping, improve context, never panic
- **Acceptance:** Every error actionable, consistent error handling

### TECH-13: Logging Review
- **Category:** Observability
- **Scope:** All logging code
- **Requirement:** Review every log for structured, consistent, useful, non-duplicated, safe logging
- **Acceptance:** No secret leakage, consistent logging

### TECH-14: Testing Improvement
- **Category:** Quality
- **Scope:** All test files
- **Requirement:** Preserve behavior, add tests where missing, improve unit/integration/benchmark/race/edge case/property tests
- **Acceptance:** No regression, improved coverage

### TECH-15: Documentation Update
- **Category:** Documentation
- **Scope:** All exported code and architecture
- **Requirement:** Update comments where architecture changes, keep exported documentation accurate, remove obsolete comments
- **Acceptance:** Accurate documentation, no obsolete comments