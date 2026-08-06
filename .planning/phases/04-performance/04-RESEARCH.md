# Phase 4: Performance - Research

**Date:** 2026-08-06
**Status:** Research complete

## 1. Startup Optimization (Sub-500ms Target)

### Current Startup Sequence (`cmd/m31a/main.go`)

The current `run()` function performs these operations eagerly at startup:

1. `flag.Parse()` — negligible
2. `config.LoadDotEnv()` — file I/O (~1-5ms)
3. `log.NewLogger()` — minimal
4. `config.Load(configPath)` — TOML parsing (~5-20ms)
5. `keychain.New()` — D-Bus/CLI call (~50-500ms on Linux, <10ms on macOS)
6. `cfg.ResolveAPIKeys(kc)` — keychain lookup (~50-200ms)
7. Provider registration loop — 3 providers, each `tui.RegisterProvider()` (~50-100ms each)
8. `tools.DefaultDispatcher()` — channel initialization + goroutines (~1-5ms)
9. Subagent manager setup — minimal
10. `tea.NewProgram()` + `p.Run()` — Bubble Tea init

**Bottleneck breakdown:**
- Keychain (step 5-6): 100-700ms — D-Bus on Linux is slow, especially in containers
- Provider registration (step 7): 150-300ms — three sequential HTTP health checks
- Config load (step 4): 5-20ms — negligible

### Lazy Loading Strategy (D-02, D-03)

**What to defer:**
- Provider registration → defer until first LLM call
- Keychain init → defer until API key needed
- Config validation → surface at first use (D-03)
- Subagent manager → defer until Agent tool invoked
- Dispatcher rate limiter goroutines → start on first `Execute()` call

**What to keep eager:**
- Flag parsing (must be instant for `--help`, `--version`)
- Logger init (needed for all error paths)
- Config file loading (needed for flag values like `--model`)
- Bubble Tea program creation (first interactive element)

**Implementation pattern:**
```go
// Lazy provider registration — create registry, but don't register until needed
type lazyRegistry struct {
    once    sync.Once
    cfg     *config.Config
    version string
    inner   *provider.Registry
}

func (r *lazyRegistry) ActiveProvider() provider.LLMProvider {
    r.once.Do(func() {
        r.inner = provider.NewRegistry()
        // register providers here
    })
    return r.inner.ActiveProvider()
}
```

**Measurement approach:**
- Add `time.Since(start)` at key checkpoints in `run()`
- Log startup phases with `slog.Debug` so `--debug` shows timing
- Benchmark: `BenchmarkStartup` compiles binary and times `--version` (fastest path)
- Separate benchmark: `BenchmarkStartupInteractive` times to first TUI render

### Startup Profiling Technique

```go
// In main.go, early in run()
if os.Getenv("M31A_STARTUP_TRACE") != "1" {
    // skip
} else {
    defer func() {
        f, _ := os.Create("/tmp/m31a-startup.pprof")
        pprof.WriteHeapProfile(f)
        f.Close()
    }()
}
```

Also consider `runtime/trace` for goroutine scheduling analysis during startup.

## 2. sync.Pool Best Practices (D-05, D-06, D-07)

### Hot Paths Identified

| Hot Path | Current Allocation | Pool Target |
|----------|-------------------|-------------|
| SSE parser buffer | 1MB per connection (`sse.go:26`) | `sync.Pool` for `[]byte` buffers |
| Tool dispatch JSON unmarshal | New `ToolInput` per call | Pool for `ToolInput` structs |
| LLM streaming chunks | New `toolCallBuilder` per tool call | Pool for builders |
| Dev server ring buffer | 256KB per server | Shared pool with total limit |
| `consumeStream` string builder | New `strings.Builder` per stream | Pool for builders |

### SSE Buffer Pool (D-06)

**Current code** (`sse.go:26`):
```go
scanner.Buffer(make([]byte, 0, sseMaxLineLength), sseMaxLineLength)
```

**Proposed pool:**
```go
var sseBufferPool = sync.Pool{
    New: func() any {
        buf := make([]byte, 0, sseMaxLineLength)
        return &buf
    },
}

func NewSSEParserWithContext(resp *http.Response, ctx context.Context) *SSEParser {
    bufPtr := sseBufferPool.Get().(*[]byte)
    scanner := bufio.NewScanner(resp.Body)
    scanner.Buffer(*bufPtr, sseMaxLineLength)
    // ... store bufPtr in parser for Put on Close
}
```

**Reset strategy:** Do NOT zero the buffer — `bufio.Scanner` overwrites as it reads. Just return to pool on `Close()`. The buffer size (1MB) is fixed, so no stale-data risk.

**Pool sizing:** No explicit size limit needed — `sync.Pool` handles GC pressure automatically. On high concurrency (8 tools max), pool will hold ~8 buffers.

### Dev Server Shared Buffer Pool (D-07)

**Current code** (`devserver.go:22-26`):
```go
const maxLogBytes = 256 * 1024 // 256KB per server
```

**Proposed approach:**
- Shared `sync.Pool` for `ringBuffer` objects
- Add a global `atomic.Int64` tracking total allocated bytes
- Cap total at configurable limit (e.g., 4MB)
- When cap reached, new servers get smaller buffers or block

```go
var (
    devServerBufferPool = sync.Pool{
        New: func() any {
            if totalBytes.Load() > maxTotalDevServerBytes {
                return nil // signal: use smaller buffer
            }
            return NewRingBuffer(maxLogBytes)
        },
    }
    totalBytes atomic.Int64
)
```

### Allocation Profiling (D-08)

**Step 1:** Add `-memprofile` to test suite first:
```bash
go test -memprofile=mem.out ./...
go tool pprof -alloc_objects mem.out
```

**Step 2:** Use `runtime.MemStats` in benchmarks:
```go
var m1, m2 runtime.MemStats
runtime.GC()
runtime.ReadMemStats(&m1)
// ... operation ...
runtime.ReadMemStats(&m2)
fmt.Printf("allocs: %d, bytes: %d\n", m2.Mallocs-m1.Mallocs, m2.TotalAlloc-m1.TotalAlloc)
```

**Step 3:** `go tool pprof -alloc_space` shows cumulative allocations (best for finding hot paths).

### sync.Pool Reset Strategy

For structs with embedded slices (like `toolCallBuilder`):
```go
var builderPool = sync.Pool{
    New: func() any {
        return &toolCallBuilder{}
    },
}

// Get from pool
b := builderPool.Get().(*toolCallBuilder)
b.id = ""
b.name = ""
b.arguments.Reset()

// Return to pool (don't zero — Reset() above handles it)
defer builderPool.Put(b)
```

**Key rule:** Always `Reset()` or clear fields before returning to pool. For byte slices, no reset needed if caller overwrites.

## 3. Task Runner Extension for Parallel Operations (D-09, D-10, D-11)

### Current Task Runner (`taskrunner/runner.go`)

**Capabilities:**
- Topological sort via Kahn's algorithm → dependency-aware execution groups
- Bounded parallelism per group via semaphore (`MaxParallel`)
- Per-task timeout, retry with backoff, skip-on-dep-failure
- Thread-safe status tracking with `sync.RWMutex`

**Extension points for Phase 4:**

1. **Parallel indexing** (CodeIntel): Register indexing tasks in the runner, let it manage parallelism
2. **Parallel verification**: Run independent verification checks concurrently
3. **Parallel analysis**: Code analysis tools (CodeMap, CodeComplexity) can run in parallel

### How to Extend

**Pattern:** Wrap existing operations as `ExecuteFunc` closures:

```go
// Parallel code intelligence build
indexTask := types.Task{
    ID:          1,
    Description: "Build code intelligence index",
    Dependencies: []int{}, // runs first
}
verifyTasks := []types.Task{
    {ID: 2, Description: "Verify syntax", Dependencies: []int{1}},
    {ID: 3, Description: "Verify tests", Dependencies: []int{1}},
    {ID: 4, Description: "Verify type safety", Dependencies: []int{1}},
}
// Tasks 2, 3, 4 run in parallel after task 1 completes
```

**Semaphore pattern (D-10):** Already implemented in task runner via `sem := make(chan struct{}, r.maxParallel())`. Matches dispatcher.go pattern.

**Key constraint:** Task state mutations must be thread-safe. The runner's `sync.RWMutex` protects `status` and `results` maps. Individual task `ExecuteFunc` implementations must handle their own concurrency.

### Parallel Operations Inventory

| Operation | Current | Parallelizable | Dependencies |
|-----------|---------|---------------|--------------|
| CodeIntel Build | Sequential file parse | Yes | None |
| File verification (syntax, tests, types) | Sequential | Yes | After CodeIntel |
| Tool execution (from LLM) | Already parallel via dispatcher | Already done | N/A |
| Dev server health checks | Sequential port polls | Yes | None |
| Config validation | Sequential | Minimal benefit | N/A |
| Session resume | Sequential file reads | Minor benefit | N/A |

## 4. Benchmark Design & pprof (D-13, D-14, D-15, D-16)

### Existing Benchmarks

The codebase already has 42 benchmark functions across:
- `internal/tools/edit_benchmark_test.go` — Edit tool strategies
- `internal/tools/codeanalysis/codecomplexity_benchmark_test.go` — Code analysis
- `internal/engine/narrative/benchmarks_test.go` — Narrative engine
- `internal/ui/tui/app_channel_bench_test.go` — Channel emitter
- `internal/ui/tui/components/glamour_bench_test.go` — Markdown rendering
- `internal/ui/tui/theme/bench_alloc_test.go` — Style allocation
- `internal/integrations/codeintel/bench_test.go` — Code intelligence (most comprehensive)

### New Benchmarks Needed (D-14)

**Startup benchmarks:**
```go
func BenchmarkStartup(b *testing.B) {
    for i := 0; i < b.N; i++ {
        // Build binary, run with --version, measure time
    }
}

func BenchmarkStartupToFirstRender(b *testing.B) {
    // Measures binary start to first TUI Init() call
}
```

**SSE parsing benchmarks:**
```go
func BenchmarkSSEParser(b *testing.B) {
    // Benchmark SSE event parsing with various payload sizes
}

func BenchmarkSSEParserWithPooledBuffer(b *testing.B) {
    // Compare with sync.Pool approach
}
```

**Tool dispatch benchmarks:**
```go
func BenchmarkToolDispatch(b *testing.B) {
    // Benchmark full CallTool path (permission + rate limit + execute)
}

func BenchmarkJSONUnmarshal(b *testing.B) {
    // Benchmark ToolInput unmarshal specifically
}
```

**Streaming benchmarks:**
```go
func BenchmarkConsumeStream(b *testing.B) {
    // Benchmark stream consumption with mock iterator
}

func BenchmarkToolCallBuilder(b *testing.B) {
    // Benchmark tool call accumulation from chunks
}
```

### CI Benchmark Regression (D-15)

**Approach:** Use `benchstat` for comparison:
```bash
# Baseline
go test -bench=. -benchmem -count=5 ./... > old.txt
# After changes
go test -bench=. -benchmem -count=5 ./... > new.txt
# Compare
benchstat old.txt new.txt
```

**50% regression threshold** (D-15): Lenient — tolerates CI noise. Implementation:
```go
// In benchmark test
func BenchmarkStartup(b *testing.B) {
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        // measure startup
    }
    // benchstat handles threshold externally
}
```

**CI integration:** Add to Makefile:
```makefile
## bench-compare  — Run benchmarks and compare against baseline
bench-compare:
    @go test -bench=. -benchmem -count=5 -run=^$$ ./... > /tmp/new.txt
    @benchstat $(BENCH_BASELINE) /tmp/new.txt
```

**Threshold enforcement:** `benchstat -delta 50%` flag enforces the 50% threshold.

### pprof Integration (D-16)

**Already implemented** in `cmd/m31a/main.go:244-269`:
- `startPprofServer()` starts on `localhost:6060`
- Gated behind `--debug` flag or `M31A_DEBUG=1`
- Uses `net/http/pprof` blank import for handler registration

**Enhancements needed:**
1. Add memory profiling endpoint logging
2. Add `go tool pprof` commands to Makefile profile target (already exists)
3. Document pprof usage in README/AGENTS.md

**Available pprof endpoints:**
```
http://localhost:6060/debug/pprof/           — Index
http://localhost:6060/debug/pprof/profile    — CPU profile (30s)
http://localhost:6060/debug/pprof/heap       — Heap profile
http://localhost:6060/debug/pprof/goroutine  — Goroutine dump
http://localhost:6060/debug/pprof/allocs     — Allocation profile
```

## 5. Specific Refactoring Targets

### 5.1 Rate Limiter (dispatcher.go:95-139)

**Current:** Two goroutines with `time.Ticker` refilling channel-based token buckets.
**Overhead:** Goroutine scheduling + ticker overhead under high tool concurrency.

**Proposed:** Replace with `golang.org/x/time/rate` (already in go.sum via indirect deps):
```go
import "golang.org/x/time/rate"

type Dispatcher struct {
    rateLimiter      *rate.Limiter
    dangerousLimiter *rate.Limiter
    // ...
}

func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall) (types.ToolResult, error) {
    if err := d.rateLimiter.Wait(ctx); err != nil {
        return types.ToolResult{}, err
    }
    // ...
}
```

**Benefits:** No goroutines, no ticker, no channel overhead. Token bucket implemented in pure Go with mutex.

**Trade-off:** Current channel-based approach is simpler to reason about. `x/time/rate` adds a dependency but it's in `golang.org/x/` (stable, well-maintained).

### 5.2 CodeIntel Build Time (codeintel.go)

**Current:** 30-second timeout for full tree-sitter parse of all source files.
**Already optimized:** Incremental builds via `buildIncremental()`, disk cache via `SaveCache()`/`LoadCache()`.

**Further optimizations:**
1. **Per-file lazy parsing:** Don't build full index upfront; parse files on demand
2. **Parallel file parsing:** Current `parseFiles()` is sequential — use goroutines
3. **Cache warming:** Background goroutine pre-parses likely-needed files during idle time

**Parallel file parsing** (fits D-09):
```go
func (idx *Indexer) parseFilesParallel(ctx context.Context, paths []string) ([]*FileInfo, error) {
    var (
        results = make([]*FileInfo, len(paths))
        wg      sync.WaitGroup
        sem     = make(chan struct{}, runtime.NumCPU())
    )
    for i, relPath := range paths {
        wg.Add(1)
        sem <- struct{}{}
        go func(i int, relPath string) {
            defer wg.Done()
            defer func() { <-sem }()
            // parse file
            results[i] = parsed
        }(i, relPath)
    }
    wg.Wait()
    return results, nil
}
```

### 5.3 SSE Buffer Allocation (sse.go:26)

**Current:** `make([]byte, 0, sseMaxLineLength)` — 1MB per connection.
**Pooled approach:** As described in Section 2 above.

**Additional optimization:** Use variable buffer sizes based on typical response sizes:
```go
var sseBufferPool = sync.Pool{
    New: func() any {
        // Start with 64KB, let it grow if needed
        buf := make([]byte, 0, 64*1024)
        return &buf
    },
}
```

But the scanner's `Buffer()` method caps at `sseMaxLineLength`, so the pool should use that size for consistency.

### 5.4 Dev Server Ring Buffer (devserver.go:22-26)

**Current:** 256KB per server, no total limit.
**Proposed:** Shared pool with global memory cap as described in Section 2.

**Alternative:** Use `bytes.Buffer` with `Grow()` hint instead of custom `ringBuffer`:
```go
type ringBuffer struct {
    mu       sync.Mutex
    buf      bytes.Buffer
    maxBytes int
    lines    int
}
```

But the current implementation is already efficient — the main improvement is the shared pool.

### 5.5 Streaming String Builder (engine_streaming.go)

**Current:** `consumeStream` creates new `strings.Builder` per call.
**Pool opportunity:** Low priority — `strings.Builder` is lightweight and GC handles it well.

**Higher-impact:** The `toolCallBuilder` map in `consumeStreamWithTools` creates new structs per tool call. Pool these:
```go
var toolCallBuilderPool = sync.Pool{
    New: func() any {
        return &toolCallBuilder{}
    },
}
```

## 6. Implementation Priority Order

Based on impact vs. effort:

1. **SSE buffer pool** (D-06) — High impact, low effort, isolated change
2. **Rate limiter refactor** — Medium impact, low effort, replace goroutines
3. **Lazy provider registration** (D-02) — High impact, medium effort
4. **Parallel CodeIntel parsing** (D-09) — Medium impact, medium effort
5. **Tool call builder pool** (D-05) — Low impact, low effort
6. **Startup benchmarks** (D-14) — Medium impact, low effort, enables measurement
7. **CI benchmark regression** (D-13, D-15) — High impact, medium effort
8. **Dev server buffer pool** (D-07) — Low impact, medium effort
9. **Lazy keychain init** (D-02) — Medium impact, medium effort
10. **pprof documentation** (D-16) — Low impact, low effort

## 7. Risk Assessment

| Change | Risk | Mitigation |
|--------|------|------------|
| sync.Pool for SSE buffers | Low — buffer reuse is safe | Don't zero buffers; reset on Close() |
| Lazy provider registration | Medium — error timing changes | D-03 accepts this; surface at first use |
| Rate limiter refactor | Low — `x/time/rate` is battle-tested | Keep channel-based as fallback |
| Parallel CodeIntel | Medium — concurrent file access | Use `sync.RWMutex` on shared state |
| Task runner extension | Low — existing infrastructure | Follow established ExecuteFunc pattern |
| pprof in production | Low — gated behind --debug | Already implemented correctly |

## 8. Testing Strategy

**Unit tests:**
- `sync.Pool` roundtrip: get → use → put → get → verify reuse
- Lazy init: verify provider not registered until first call
- Rate limiter: verify burst behavior matches spec
- Parallel task runner: verify all tasks complete, status correct

**Benchmarks:**
- Before/after for each change using `benchstat`
- `BenchmarkStartup` as the north star metric (sub-500ms target)
- `BenchmarkSSEParser` with/without pool
- `BenchmarkToolDispatch` with/without rate limiter change

**Integration tests:**
- Full workflow execution with performance assertions
- pprof server accessibility in debug mode
- CI regression detection with 50% threshold

---

*Research: 2026-08-06*
*Phase: 4-Performance*
