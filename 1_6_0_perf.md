# M31A v1.6.0 — Performance & Perception Initiative

## 1. Benchmark Plan

### 1.1 Startup Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkStartup_Cold` | Full startup from process entry to first `View()` | Run binary, measure wall time from `main()` to `tea.Program.Run()` | < 300ms |
| `BenchmarkStartup_ConfigLoad` | 6-layer config cascade | `config.Load()` with typical config | < 20ms |
| `BenchmarkStartup_ProviderReg` | 3 provider registrations | `provider.NewRegistry()` + 3x `RegisterProvider()` | < 50ms |
| `BenchmarkStartup_SessionList` | Resume session list load | `sessionManager.ListSessions()` | < 10ms |
| `BenchmarkStartup_WorktreeSweep` | `subagent.Sweep()` | `git worktree prune` with timeout | Background (0ms blocking) |

### 1.2 Rendering Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkView_Idle` | Full View() with no streaming | Render REPL screen, no activity | < 5ms |
| `BenchmarkView_Streaming` | Full View() during streaming | Render REPL with streaming content | < 10ms |
| `BenchmarkView_WithSidebar` | View() with sidebar on Full breakpoint | 80+ col terminal | < 8ms |
| `BenchmarkView_PermissionModal` | View() with permission overlay | Dimmed REPL + modal | < 10ms |
| `BenchmarkRenderPage` | `layout.RenderPage()` composition | Header + content + footer | < 1ms |
| `BenchmarkRenderSidebar` | `SidebarModel.View()` | Full sidebar with all sections | < 3ms |
| `BenchmarkRenderFooter` | `BuildFooter()` | Status bar with all zones | < 0.5ms |

### 1.3 Streaming Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkStreamTick` | Single streaming tick (StreamMsg + render) | Simulate one SSE chunk + render | < 5ms |
| `BenchmarkStreamIncremental` | Incremental render during streaming | Append to cached content | < 3ms |
| `BenchmarkStreamFullRender` | Full message re-render | All messages re-rendered | < 15ms |
| `BenchmarkStreamTickRate` | Actual ticks per second | Count ticks processed in 1s | >= 8fps |

### 1.4 Keypress and Interaction Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkKeypress_Simple` | Character input to textarea | Send `tea.KeyMsg{'a'}` | < 2ms |
| `BenchmarkKeypress_Command` | Slash command execution | Send `/help` | < 10ms |
| `BenchmarkScroll_Wheel` | Mouse wheel scroll event | Send `tea.MouseMsg{WheelUp}` | < 2ms |
| `BenchmarkAutocomplete` | @mention or slash completion | Trigger autocomplete overlay | < 5ms |
| `BenchmarkViewport_Scroll` | Viewport scroll to bottom | 1000 messages, scroll to end | < 5ms |

### 1.5 Content Rendering Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkGlamour_Render` | Glamour markdown render | 1000-char markdown chunk | < 5ms |
| `BenchmarkGlamour_RenderCached` | Glamour with cached renderer | Same width, reuse renderer | < 3ms |
| `BenchmarkSyntax_Highlight` | Chroma syntax highlight | 50-line Go code block | < 5ms |
| `BenchmarkSyntax_RenderBlock` | Full code block with line numbers | 50-line block | < 8ms |
| `BenchmarkToolCard_New` | `NewToolCard` allocation | Bash tool with 100-line output | < 5ms |
| `BenchmarkToolCard_Render` | `ToolCard.Render()` | Block mode with output | < 3ms |
| `BenchmarkDiff_Render` | Diff content rendering | 200-line unified diff | < 10ms |

### 1.6 Allocation Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkAlloc_StyleCache` | `BuildSemanticStyles` vs cached | Compare per-frame cost | 0 allocs (cached) |
| `BenchmarkAlloc_SSELines` | SSE parser line slice reuse | Reuse `lines[:0]` pattern | 0 allocs per event |
| `BenchmarkAlloc_ModelCache` | `Models()` snapshot | 300 models | 0 allocs (cached) |
| `BenchmarkAlloc_ScrollSplit` | `overlayScrollbar` split/join | 100-line viewport | 0 allocs (in-place) |
| `BenchmarkAlloc_ToolCard` | ToolCard with cached styles | Reuse SemanticStyles | 0 style allocs |

### 1.7 Concurrency Benchmarks

| Benchmark | What | How | Target |
|-----------|------|-----|--------|
| `BenchmarkChannel_Emitter` | Channel send throughput | 1000 sends to 128 buffer | < 1ms total |
| `BenchmarkChannel_Stream` | Stream channel throughput | 1000 sends to 64 buffer | < 1ms total |
| `BenchmarkMutex_Dispatcher` | RWMutex under concurrent reads | 8 goroutines reading tools | < 100us p99 |
| `BenchmarkMutex_Registry` | RWMutex under concurrent reads | 8 goroutines reading providers | < 50us p99 |

---

## 2. Profiling Plan

### 2.1 CPU Profiling

**When to capture:**
- During streaming (10fps rendering loop)
- During startup (first 2 seconds)
- During scroll (rapid mouse wheel)
- During permission modal display

**How:**
```go
import "runtime/pprof"

f, _ := os.Create("/tmp/m31a_cpu.prof")
pprof.StartCPUProfile(f)
defer pprof.StopCPUProfile()
```

**Key targets:**
- `View()` call stack — see where time is spent per frame
- `renderFrameWithTheme()` — header/footer/sidebar composition
- `renderStreamingContent()` — streaming render path
- `Glamour.Render()` — markdown rendering
- `chroma.Tokenise()` — syntax highlighting
- `lipgloss.*.Render()` — style rendering

### 2.2 Memory Profiling

**When to capture:**
- After 60 seconds of streaming (peak allocation)
- After 10 tool executions (tool card churn)
- After session save (JSON serialization)

**How:**
```go
import "runtime/pprof"

f, _ := os.Create("/tmp/m31a_mem.prof")
pprof.WriteHeapProfile(f)
f.Close()
```

**Key targets:**
- `BuildSemanticStyles` — style object count
- `strings.Split` — string allocation count
- `Glamour.Render` — markdown allocation count
- `chroma.Tokenise` — token allocation count
- `json.Marshal` — serialization allocation count
- `ToolCard` — per-card allocation count

### 2.3 Goroutine Profiling

**When to capture:**
- Steady state (idle REPL)
- During streaming
- During parallel tool execution
- During subagent activity

**How:** `runtime.NumGoroutine()` at each state.

**Key targets:**
- Confirm 7-8 steady-state goroutines
- Confirm stream goroutine lifecycle (1 per stream, exits on EOF)
- Confirm no goroutine buildup during parallel tools
- Confirm subagent goroutine count <= 8 (semaphore limit)

### 2.4 GC Profiling

**When to capture:**
- During streaming (10 seconds)
- During idle (10 seconds)

**How:**
```go
import "runtime/debug"

debug.SetGCPercent(-1) // disable GC
// ... run workload ...
var m1 runtime.MemStats
runtime.ReadMemStats(&m1)
// ... wait ...
var m2 runtime.MemStats
runtime.ReadMemStats(&m2)
// Compare m1.HeapAlloc, m1.NumGC, m2.NumGC
```

**Key targets:**
- GC pause duration (should be < 1ms)
- GC frequency (should be < 1 per second during streaming)
- Heap growth rate (should be stable, not growing)

### 2.5 Latency Profiling

**Interactive latency measurement:**
```go
// In Update():
start := time.Now()
// ... process message ...
duration := time.Since(start)
if duration > 16*time.Millisecond {
    slog.Warn("slow update", "duration", duration, "msg", fmt.Sprintf("%T", msg))
}
```

**Key targets:**
- `Update()` duration per message type
- `View()` duration per frame
- Channel send/receive latency

---

## 3. Bottleneck Ranking

### Tier 1: Critical (Blocks smooth streaming)

| # | Bottleneck | Evidence | Impact | Current Cost |
|---|-----------|----------|--------|--------------|
| **B1** | `BuildSemanticStyles()` per-frame | Called 5-8x per View(), each allocating ~30 lipgloss.Style objects. During streaming at 10fps = **1500-2400 allocs/second** | GC pressure, streaming micro-stutters | ~30 lipgloss.Style allocs x 5-8 calls x 10fps |
| **B2** | Glamour re-renders ALL messages | Every `renderMessages()` call re-renders every message through Glamour. 10 messages x ~25-55 allocs = **250-550 allocs per render** | Streaming frame drops, O(n) message count | Full markdown parse+render per message per frame |
| **B3** | `overlayScrollbar` splits full viewport | `strings.Split(rendered, "\n")` on every View() call = **N+1 string allocs** where N = viewport height (~30-100 rows) | Per-frame string churn | ~100 string allocs per View() |
| **B4** | `compositeOverlays` splits full viewport | Another `strings.Split` + `strings.Join` per View() when overlays are visible | Per-frame string churn | ~200 string allocs per View() |
| **B5** | Emitter channel drops messages | 128 buffer with 500ms timeout. Heavy parallel tool execution fills buffer in ~16s of burst | Lost workflow events, incomplete progress display | Silent message loss |

### Tier 2: High (Measurable allocation pressure)

| # | Bottleneck | Evidence | Impact | Current Cost |
|---|-----------|----------|--------|--------------|
| **B6** | SSE parser `lines` slice re-allocation | `var lines []string` declared inside infinite loop at `sse.go:49` — re-allocated from nil on every SSE event | 50-200 small allocs per stream | ~48 bytes x 200 events |
| **B7** | Model cache `Models()` snapshot | `make(map[string]*types.ModelInfo, len(c.models))` + loop copy per call. 300+ models = **~16.8KB per call**, 10-30x/sec | Heap churn during model selector | ~168KB/sec of map allocations |
| **B8** | AutoDream defensive copy chain | `SetMessages()` + `Messages()` = 2-3 full slice copies per sync. 200 messages x 80 bytes = **~48KB per sync** | GC pressure during workflow transitions | ~48-96KB per phase transition |
| **B9** | `RenderCodeBlock` calls `BuildSemanticStyles` twice | Once in function body (line 65), once inside `renderCodeLines` (line 106) | Wasted style allocs per code block | ~60 wasted lipgloss.Style allocs per code block |
| **B10** | Session JSON `json.Marshal` per save | No buffer pool. Full `[]byte` allocation per save. 200 messages = **~200KB JSON** | GC pressure from short-lived byte slices | ~200KB per save x 10-50 saves/session |

### Tier 3: Medium (Optimization opportunities)

| # | Bottleneck | Evidence | Impact | Current Cost |
|---|-----------|----------|--------|--------------|
| **B11** | WebFetch `htmlToMarkdown` redundant lowercase | `strings.ToLower(rawHTML)` recomputed 9 times on lines 465-489 | O(9N) string copies for N-byte HTML | ~900KB for 100KB page |
| **B12** | Edit tool intermediate slice allocs | 7-strategy cascade creates 3-4 intermediate `[]string` slices per strategy | GC pressure per edit | ~96KB worst-case garbage |
| **B13** | Grep per-file scanner allocation | `bufio.NewScanner` + 4KB buffer allocated per file, not reused | Memory churn during grep | ~4KB x N files |
| **B14** | Grep regex compilation per execute | `regexp.Compile` per pure-Go grep, not cached | CPU + allocs per grep | ~200-2000 bytes per call |
| **B15** | Codeintel file hash (SHA-256) | Full file read + SHA-256 per file per incremental check | I/O + CPU per file | O(file size) per file |

### Tier 4: Low (Fine-tuning)

| # | Bottleneck | Evidence | Impact | Current Cost |
|---|-----------|----------|--------|--------------|
| **B16** | DNS cache eviction bubble sort | O(N^2) sort on 1024 entries in `dns_cache.go:148` | Rare latency spike | ~1M comparisons worst case |
| **B17** | `SkipDirsMap()` copies map per call | Full map copy in `types/constants.go:146` on every file traversal | Small alloc per traversal | ~128 bytes per call |
| **B18** | `toTOMLKey` per-field allocation | Allocates `[]byte` per field name in config merge | Small allocs during config load | ~80 bytes x 80 fields |
| **B19** | tiktoken Encode buffer | No pooling for `[]int` result from `Encode()` | Small allocs per token estimate | ~4KB per call |
| **B20** | `isBinaryContent()` per tool card | Scans first 1024 bytes on every `NewToolCard` | Unnecessary scan for non-binary output | 1024 bytes per card |

---

## 4. Optimization Roadmap

### Phase 1: Zero-Risk Foundation (Week 1)

These changes are safe, isolated, and measurable.

#### O1: Thread StyleCache Through Rendering Pipeline

**What:** Replace all `theme.BuildSemanticStyles(t)` calls with `m.styleCache.SemanticStyles` (already exists in `theme/cache.go:107`).

**Files to change:**
- `repl_view.go:325,349` — wave separator, slash suggestions
- `repl_scrollbar.go:32` — overlay scrollbar
- `repl_footer.go:51,193,253` — status bar
- `repl_state.go:481,493` — streaming content
- `components/toolcard.go:115` — NewToolCard
- `components/syntax.go:65,106` — RenderCodeBlock (twice)
- `components/permission.go:217` — highlight command
- `layout/page.go:94,171,223` — BuildHeader, renderContextMeter, BuildFooter

**Approach:**
1. Add `styleCache *theme.StyleCache` field to `ReplModel`, `AppState`
2. Pass cache through render functions instead of `theme.Theme`
3. Replace `theme.BuildSemanticStyles(t)` with `cache.SemanticStyles`

**Expected improvement:** Eliminate ~1500-2400 lipgloss.Style allocs/second during streaming. Reduce View() allocation count by ~60%.

**Implementation cost:** 2-3 days. Thread cache through ~12 call sites.

**Regression risk:** Low. Cache is rebuilt on theme change. Style values are identical.

**Success metric:** `go test -bench=BenchmarkAlloc_StyleCache -benchmem` shows 0 allocs.

---

#### O2: Cache Glamour-Rendered Message Output

**What:** Cache the rendered output of each message segment. Only re-render when width changes or content changes.

**Files to change:**
- `components/message.go:420-451` — `renderContentSegment()`
- `components/message.go:299-418` — `renderAssistantMessage()`

**Approach:**
1. Add `renderedCache map[string]string` to `MessageRenderer` (key = segment content hash)
2. On `renderContentSegment()`, check cache before calling `Glamour.Render()`
3. Invalidate cache on `SetWidth()` (resize)

**Expected improvement:** Reduce per-frame render cost from O(N*M) to O(N) where N = messages, M = segments. During streaming with 10 messages, saves ~250-550 allocs per render.

**Implementation cost:** 1-2 days. Cache map + invalidation logic.

**Regression risk:** Low. Cache is keyed on content. Width change invalidates all.

**Success metric:** `BenchmarkView_Idle` shows < 2ms with 10 messages.

---

#### O3: Eliminate Overlay Scrollbar String Split

**What:** Replace `strings.Split(rendered, "\n")` + `strings.Join` with in-place line counting and targeted insertion.

**Files to change:**
- `repl_scrollbar.go:22-96` — `overlayScrollbar()`

**Approach:**
1. Use `strings.Count(rendered, "\n")` for line count (already done at line 29)
2. Use `strings.Index` loop to find line boundaries for the target row
3. Replace only the target row's rightmost column with the scrollbar character
4. No full split/join needed

**Expected improvement:** Eliminate ~100 string allocs per View() call.

**Implementation cost:** 1 day. Rewrite `overlayScrollbar` with index-based insertion.

**Regression risk:** Low. Same visual output, fewer allocations.

**Success metric:** `BenchmarkAlloc_ScrollSplit` shows 0 allocs.

---

#### O4: Eliminate Redundant `BuildSemanticStyles` in `RenderCodeBlock`

**What:** `RenderCodeBlock` calls `BuildSemanticStyles` twice (once directly, once inside `renderCodeLines`). Pass the cache instead.

**Files to change:**
- `components/syntax.go:61-95` — `RenderCodeBlock()`
- `components/syntax.go:97-122` — `renderCodeLines()`

**Approach:** Pass `SemanticStyles` as parameter instead of calling `BuildSemanticStyles(t)`.

**Expected improvement:** Eliminate ~60 wasted style allocs per code block.

**Implementation cost:** 0.5 day. Parameter threading.

**Regression risk:** Low. Same styles, fewer allocations.

**Success metric:** Single `BuildSemanticStyles` call per code block (verified via allocation count).

---

#### O5: SSE Parser Line Slice Reuse

**What:** Move `var lines []string` outside the loop and reuse with `lines[:0]`.

**Files to change:**
- `provider/sse.go:48-49` — `Next()` method

**Approach:**
```go
// Before (inside loop):
for {
    var lines []string  // re-allocated every iteration
    ...
}

// After:
var lines []string
for {
    lines = lines[:0]  // reuse backing array
    ...
}
```

**Expected improvement:** Eliminate 50-200 small allocs per LLM stream.

**Implementation cost:** 0.5 day. Single variable relocation.

**Regression risk:** Low. Same behavior, fewer allocations.

**Success metric:** `BenchmarkAlloc_SSELines` shows 0 allocs per event.

---

### Phase 2: Targeted Wins (Week 2)

These require slightly more care but deliver significant impact.

#### O6: Cache Model List Snapshot

**What:** Cache the `Models()` map copy. Invalidate when `Set()` is called.

**Files to change:**
- `provider/cache.go:119-130` — `Models()`

**Approach:**
1. Add `cachedSnapshot map[string]*types.ModelInfo` and `snapshotValid bool` to `ModelCache`
2. In `Models()`, return cached snapshot if valid
3. In `Set()`, set `snapshotValid = false`

**Expected improvement:** Eliminate ~16.8KB map copy per render (10-30x/sec).

**Implementation cost:** 1 day. Cache invalidation logic.

**Regression risk:** Low. Snapshot is read-only reference.

**Success metric:** `BenchmarkAlloc_ModelCache` shows 0 allocs per call.

---

#### O7: Batch Emitter Drain

**What:** Read multiple messages per tick when the channel is under pressure.

**Files to change:**
- `app.go:460-474` — `drainEmitterCmd()`
- `app_channel.go:19-34` — `channelEmitter`

**Approach:**
1. In `drainEmitterCmd()`, drain up to 16 messages per tick (batch)
2. Return a `BatchDrainMsg` containing all drained messages
3. Process all messages in a single `Update()` call

**Expected improvement:** 16x drain throughput under load. Reduce message drop probability.

**Implementation cost:** 2 days. Batch message type + processing loop.

**Regression risk:** Medium. Must preserve Bubble Tea single-threaded contract. All messages processed in one Update().

**Success metric:** No dropped messages during 8-parallel-tool stress test.

---

#### O8: Move Worktree Sweep to Background

**What:** Move `subagent.Sweep()` from blocking startup to background goroutine.

**Files to change:**
- `cmd/m31a/main.go:340-344` — `subagent.Sweep()`

**Approach:** Launch sweep in goroutine after TUI starts. Show spinner in sidebar if sweep is running.

**Expected improvement:** Eliminate up to 30s startup blocking.

**Implementation cost:** 0.5 day. Goroutine launch + optional status indicator.

**Regression risk:** Low. Sweep is best-effort cleanup.

**Success metric:** Startup time < 500ms (previously could be 30s).

---

#### O9: Buffer Pool for Session JSON

**What:** Use `sync.Pool` for `json.Marshal` byte slices.

**Files to change:**
- `pkg/session/manager.go` — `SaveSession()`, `SaveMessages()`

**Approach:**
```go
var bufPool = sync.Pool{
    New: func() any { return new(bytes.Buffer) },
}
```

**Expected improvement:** Reduce session save allocation from ~200KB to near-zero (reused buffers).

**Implementation cost:** 1 day. Pool creation + Get/Put pattern.

**Regression risk:** Low. Pool is transparent to callers.

**Success metric:** `go tool allocs -test=TestSessionSave` shows reduced allocs.

---

#### O10: Lazy `isBinaryContent` Check

**What:** Only check for binary content if output contains null bytes or control characters.

**Files to change:**
- `components/toolcard.go:145-160` — `isBinaryContent()`

**Approach:** Quick null-byte scan of first 1024 bytes before full binary detection.

**Expected improvement:** Skip full binary scan for 90%+ of tool outputs (text).

**Implementation cost:** 0.5 day. Fast-path check.

**Regression risk:** Low. Same binary detection, faster for non-binary.

**Success metric:** `BenchmarkToolCard_New` shows < 2ms for text output.

---

### Phase 3: Architectural Improvements (Week 3)

These require more design work but deliver lasting impact.

#### O11: Single-Pass Viewport Rendering

**What:** Combine `overlayScrollbar` and `compositeOverlays` into a single pass over viewport lines.

**Files to change:**
- `repl_scrollbar.go` — `overlayScrollbar()`
- `repl_view.go:52-91` — `compositeOverlays()`
- `repl_view.go:273-315` — `compositeOverlaysTop()`

**Approach:**
1. Build viewport content line-by-line in a single pass
2. Apply scrollbar, overlays, and top compositing during line construction
3. No post-hoc string splitting

**Expected improvement:** Eliminate ~300 string allocs per View() call.

**Implementation cost:** 3-5 days. Refactor viewport rendering to line-builder pattern.

**Regression risk:** Medium. Must preserve exact visual output.

**Success metric:** `BenchmarkView_Idle` shows < 3ms with 0 string allocs from overlays.

---

#### O12: Incremental Message Rendering

**What:** Track which messages changed and only re-render those.

**Files to change:**
- `repl_state.go:385-443` — `renderMessages()`
- `components/message.go` — `MessageRenderer`

**Approach:**
1. Add `dirtyMessages map[int]bool` to `ReplModel`
2. On `UpdateLiveTool()`, mark only the changed message index as dirty
3. In `renderMessages()`, only re-render dirty messages, concatenate with cached output

**Expected improvement:** Reduce full render cost from O(N) to O(dirty). During tool execution with 50 messages, only 1 message re-renders.

**Implementation cost:** 3-4 days. Dirty tracking + cache invalidation.

**Regression risk:** Medium. Must correctly handle all mutation paths.

**Success metric:** `BenchmarkView_Streaming` shows constant time regardless of message count.

---

#### O13: Permission Modal Quick-Path

**What:** Avoid re-rendering full REPL background when showing permission modal.

**Files to change:**
- `app_view.go:199-211` — `renderDimmedModal()`

**Approach:** Cache the last REPL frame as a string. Use cached frame as dimmed background instead of re-rendering.

**Expected improvement:** Reduce permission modal render from ~10ms to ~2ms.

**Implementation cost:** 1 day. Frame caching.

**Regression risk:** Low. Background is static during modal.

**Success metric:** Permission modal appears in < 5ms from request.

---

#### O14: WebFetch Single-Pass HTML Conversion

**What:** Replace the 15-stage `htmlToMarkdown` pipeline with a single-pass converter.

**Files to change:**
- `tools/webfetch.go:429-501` — `htmlToMarkdown()`

**Approach:**
1. Pre-compute `lower := strings.ToLower(rawHTML)` once
2. Pass through all tag replacements in a single pass
3. Use `strings.Builder` with pre-grown capacity

**Expected improvement:** Reduce WebFetch memory from ~1.5MB to ~200KB for 100KB pages.

**Implementation cost:** 2-3 days. Rewrite htmlToMarkdown as single-pass.

**Regression risk:** Medium. Must preserve identical markdown output.

**Success metric:** `go tool allocs -test=TestWebFetch` shows 3x reduction.

---

### Phase 4: Advanced (Week 4, if time permits)

#### O15: Compile Chroma Lexers

Cache compiled Chroma lexers by language instead of looking up per call.

#### O16: JSON Serialization Pool

Extend buffer pooling from sessions to all `json.Marshal` calls.

#### O17: Codeintel Scanner Reuse

Pool `bufio.Scanner` instances across file parses.

#### O18: Regex Cache for Grep

Cache compiled regexes by pattern string.

---

## 5. Expected Measurable Gains

### Per-Phase Expected Improvements

| Phase | Optimization | Before | After | Improvement |
|-------|-------------|--------|-------|-------------|
| **1** | O1: StyleCache threading | 1500-2400 allocs/sec | 0 allocs/sec | **100% elimination** |
| **1** | O2: Glamour output cache | 250-550 allocs per render | ~50 allocs (dirty only) | **80-90% reduction** |
| **1** | O3: Scrollbar in-place | 100 string allocs/view | 0 string allocs/view | **100% elimination** |
| **1** | O4: CodeBlock double-style | 60 allocs per code block | 0 redundant allocs | **100% elimination** |
| **1** | O5: SSE line reuse | 50-200 allocs per stream | 0 allocs per stream | **100% elimination** |
| **2** | O6: Model cache snapshot | 16.8KB per render | 0KB per render | **100% elimination** |
| **2** | O7: Batch emitter drain | Drops at 128 msgs | No drops at 2048 msgs | **16x headroom** |
| **2** | O8: Background sweep | 0-30s startup | < 500ms startup | **60x improvement** |
| **2** | O9: Session buffer pool | 200KB per save | ~0KB per save | **~100% reduction** |
| **2** | O10: Lazy binary check | 1024 bytes per card | ~10 bytes per card (null scan) | **99% reduction** |
| **3** | O11: Single-pass viewport | 300 string allocs/view | 0 string allocs/view | **100% elimination** |
| **3** | O12: Incremental messages | O(N) per render | O(1) per render | **Linear to constant** |
| **3** | O13: Modal quick-path | ~10ms modal render | ~2ms modal render | **5x improvement** |
| **3** | O14: HTML single-pass | ~1.5MB for 100KB | ~200KB for 100KB | **7.5x reduction** |

### Aggregate Expected Gains

| Metric | Current (Estimated) | After Optimization | Improvement |
|--------|---------------------|-------------------|-------------|
| **View() allocations (idle)** | ~500-800 per frame | ~50-100 per frame | **80-90% reduction** |
| **View() allocations (streaming)** | ~1500-3000 per frame | ~100-200 per frame | **90-95% reduction** |
| **View() latency (idle)** | ~5-8ms | ~2-3ms | **50-60% reduction** |
| **View() latency (streaming)** | ~8-15ms | ~3-5ms | **60-70% reduction** |
| **GC frequency (streaming)** | ~2-5/sec | ~0-1/sec | **75-100% reduction** |
| **GC pause (streaming)** | ~1-2ms | < 0.5ms | **60-75% reduction** |
| **Startup time** | 200-500ms (30s worst) | < 500ms (always) | **Consistent, bounded** |
| **Memory at steady state** | Growing with messages | Stable | **Bounded** |
| **Emitter message loss** | Drops under load | No drops | **Eliminated** |

---

## 6. Validation Strategy

### 6.1 Benchmark-Driven Development

Every optimization must:
1. Have a benchmark BEFORE implementation (establish baseline)
2. Implement the optimization
3. Re-run benchmark to verify improvement
4. Run `go test -race ./...` to verify correctness

**Command sequence:**
```bash
# Baseline
go test -bench=BenchmarkView_Idle -benchmem -count=5 > /tmp/bench_before.txt

# Implement optimization
# ...

# Verify improvement
go test -bench=BenchmarkView_Idle -benchmem -count=5 > /tmp/bench_after.txt

# Compare
benchstat /tmp/bench_before.txt /tmp/bench_after.txt

# Verify correctness
go test -race ./...
```

### 6.2 Allocation Budget

After all Phase 1-2 optimizations:

| Component | Budget | Verification |
|-----------|--------|-------------|
| View() per frame | < 100 allocs | `BenchmarkView_Idle -benchmem` |
| Streaming tick | < 200 allocs | `BenchmarkStreamTick -benchmem` |
| ToolCard creation | < 20 allocs | `BenchmarkToolCard_New -benchmem` |
| Scrollbar render | 0 allocs | `BenchmarkAlloc_ScrollSplit -benchmem` |
| Session save | < 10 allocs | `go tool allocs -test=TestSessionSave` |

### 6.3 Latency Budget

| Interaction | Budget | Measurement |
|-------------|--------|-------------|
| Keypress to cursor move | < 2ms | `time.Since(start)` in Update() |
| View() idle | < 3ms | Benchmark |
| View() streaming | < 5ms | Benchmark |
| Permission modal appear | < 5ms | `time.Since(requestTime)` |
| Scroll wheel response | < 2ms | Benchmark |
| Slash command execution | < 10ms | Benchmark |

### 6.4 Memory Budget

| Metric | Budget | Measurement |
|--------|--------|-------------|
| Steady-state heap | < 50MB | `runtime.ReadMemStats` |
| Heap growth rate | < 1MB/min | MemStats over 10 minutes |
| GC pause | < 1ms | `runtime.ReadMemStats.PauseTotalNs` / NumGC |
| Goroutine count (steady) | 7-8 | `runtime.NumGoroutine()` |
| Goroutine count (streaming) | < 15 | `runtime.NumGoroutine()` during stream |
| Goroutine count (parallel tools) | < 20 | `runtime.NumGoroutine()` during execute |

### 6.5 Stress Tests

| Test | What | Expected |
|------|------|----------|
| **Rapid scroll** | 1000 mouse wheel events in 1s | No frame drops, < 5ms per event |
| **Streaming + scroll** | Stream LLM response while scrolling | Smooth streaming, no stutter |
| **Parallel tools** | 8 tools executing simultaneously | No emitter drops, < 10ms View() |
| **Large conversation** | 500 messages, scroll to top | < 5ms scroll, < 10ms View() |
| **Rapid resize** | 50 resize events in 1s | No crash, < 10ms per resize |
| **Permission spam** | 20 permission requests in 1s | All processed, no deadlock |
| **Config reload** | 10 config saves in 1s | Debounced, no panic |
| **Subagent burst** | Spawn 8 subagents simultaneously | All start, < 20 goroutines total |

### 6.6 Regression Gates

Before merging any optimization:
1. `go test -race ./...` passes
2. `go vet ./...` passes
3. All existing benchmarks pass or improve
4. New benchmarks added for the optimized path
5. Manual testing: stream 1000 tokens, scroll, type, permission modal
6. Visual regression: no change in rendered output

### 6.7 Continuous Monitoring

After optimization, add runtime telemetry (configurable, off by default):
```go
type Metrics struct {
    ViewDuration    time.Duration  // p50, p99 of View() calls
    UpdateDuration  time.Duration  // p50, p99 of Update() calls
    AllocsPerFrame  int64          // average allocations per View()
    GCPressure      float64        // GC cycles per second
    EmitterDrops    int64          // total dropped messages
    GoroutinePeak   int            // max goroutines observed
}
```

Expose via `/metrics` slash command for debugging.
