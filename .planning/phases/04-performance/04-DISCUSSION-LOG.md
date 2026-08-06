# Phase 4: Performance - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-06
**Phase:** 04-performance
**Areas discussed:** Startup optimization, Memory strategy, Parallelism scope, Performance measurement

---

## Startup optimization

| Option | Description | Selected |
|--------|-------------|----------|
| Under 500ms | Feels instant. May require lazy-loading providers and deferring non-essential init. | ✓ |
| Under 1 second | Fast enough for most users. Allows some eager loading. | |
| Under 2 seconds | Acceptable. Current behavior is probably close to this already. | |
| You decide | Measure current startup, then optimize the biggest wins. | |

**User's choice:** Under 500ms
**Notes:** Aggressive target requiring lazy everything.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Lazy everything | Defer provider registration, config validation, and keychain lookup until first LLM call. | ✓ |
| Lazy providers only | Load config eagerly (fail fast on bad config), but defer provider registration and keychain. | |
| You decide | Profile current startup, identify the slowest paths, and defer only those. | |

**User's choice:** Lazy everything
**Notes:** Config validation deferred to first use.

---

| Option | Description | Selected |
|--------|-------------|----------|
| At first use | User types a prompt, then gets an error about bad config. | ✓ |
| Async warning | Show a non-blocking warning in the TUI status bar after startup. | |
| You decide | Let the agent choose the UX based on what feels natural in the TUI. | |

**User's choice:** At first use
**Notes:** Error surfaces when user first interacts with a feature needing config.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Pure lazy | Nothing loaded until needed. Simplest implementation. | ✓ |
| Background warm-up | After TUI renders, spawn a goroutine to pre-fetch models list and warm caches. | |
| You decide | Let the agent pick based on profiling results. | |

**User's choice:** Pure lazy
**Notes:** No background warm-up goroutines.

---

## Memory strategy

| Option | Description | Selected |
|--------|-------------|----------|
| sync.Pool | Standard Go pattern for reducing GC pressure on frequently allocated objects. | ✓ |
| Struct field reuse | Reset and reuse fields on existing structs instead of allocating new ones. | |
| You decide | Profile allocation hot spots first, then apply the right strategy per site. | |

**User's choice:** sync.Pool
**Notes:** For hot paths: SSE parsing, tool dispatch, LLM streaming.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Pooled buffers | Use sync.Pool for SSE buffers. Reuse across connections. | ✓ |
| Smaller default + grow | Start with 64KB, grow on demand. | |
| You decide | Let the agent choose based on profiling typical SSE event sizes. | |

**User's choice:** Pooled buffers
**Notes:** Replace 1MB per-connection allocation with pooled buffers.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Shared pool | All dev servers share a single buffer pool with a total memory limit. | ✓ |
| Per-server cap | Keep per-server buffers but enforce a max total across all servers. | |
| You decide | Let the agent pick based on how many dev servers typically run concurrently. | |

**User's choice:** Shared pool
**Notes:** Fair allocation across servers with total memory bound.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Measure first | Add -memprofile to test suite, identify top allocation sites, then optimize. | ✓ |
| Optimize now | Apply sync.Pool to known hot paths now, add profiling later. | |
| You decide | Let the agent choose the order based on what's fastest to implement. | |

**User's choice:** Measure first
**Notes:** Profile before optimizing for targeted improvements.

---

## Parallelism scope

| Option | Description | Selected |
|--------|-------------|----------|
| All at once | Indexing, analysis, verification, and independent tasks all get concurrent execution. | ✓ |
| Incremental | Start with lowest-risk parallelism and add indexing/analysis later. | |
| You decide | Profile which operations are actually bottlenecked by sequential execution first. | |

**User's choice:** All at once
**Notes:** Maximum parallelism across all operations.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Semaphore-based | Use existing semaphore pattern from dispatcher.go. | ✓ |
| Worker pool | Fixed-size goroutine pool with work-stealing. | |
| You decide | Let the agent choose based on operation characteristics. | |

**User's choice:** Semaphore-based
**Notes:** Consistent with codebase conventions.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse task runner | Extend existing task runner for new parallel operations. | ✓ |
| Separate concurrency | Each operation gets its own goroutine management. | |
| You decide | Let the agent decide per-operation based on whether they fit the task runner model. | |

**User's choice:** Reuse task runner
**Notes:** Consistent, tested, dependency-aware scheduling.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Fail fast | If any parallel operation fails, cancel the rest and report the failure. | |
| Collect all errors | Let other operations complete, collect all failures, report together. | ✓ |
| You decide | Let the agent choose per-operation based on whether partial results are useful. | |

**User's choice:** Collect all errors
**Notes:** User sees all issues at once, more resilient.

---

## Performance measurement

| Option | Description | Selected |
|--------|-------------|----------|
| CI benchmarks | Go benchmarks in CI with regression detection. Fails PR if performance degrades. | |
| Manual profiling | pprof endpoints available for manual investigation. | |
| Both | CI benchmarks for regression detection + pprof endpoints for deep investigation. | ✓ |
| You decide | Let the agent choose based on what's practical for this phase. | |

**User's choice:** Both
**Notes:** Comprehensive measurement approach.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Startup + hot paths | Benchmark startup time, SSE parsing, tool dispatch, LLM streaming. | |
| Comprehensive | All major operations: startup, streaming, dispatch, indexing, compaction, session resume. | ✓ |
| You decide | Let the agent pick based on profiling results from earlier in the phase. | |

**User's choice:** Comprehensive
**Notes:** Complete performance picture across all operations.

---

| Option | Description | Selected |
|--------|-------------|----------|
| 10% regression | Strict — catches small regressions early. May cause flaky failures. | |
| 25% regression | Moderate — catches significant regressions, tolerates CI noise. | |
| 50% regression | Lenient — only catches major regressions. Low false positive rate. | ✓ |
| You decide | Let the agent choose based on typical CI variance. | |

**User's choice:** 50% regression
**Notes:** Lenient threshold, tolerates CI noise.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Debug flag | Start pprof HTTP server only when --debug or M31A_DEBUG=true is set. | ✓ |
| On-demand via signal | Start pprof on SIGUSR1 (Unix). | |
| You decide | Let the agent pick based on typical debugging workflows. | |

**User's choice:** Debug flag
**Notes:** Zero overhead in production, explicit opt-in.

---

## the agent's Discretion

- sync.Pool sizing and reset strategies per hot path
- Which operations to parallelize first within the task runner
- Benchmark function naming and organization
- pprof HTTP server port and shutdown behavior

## Deferred Ideas

None — discussion stayed within phase scope
