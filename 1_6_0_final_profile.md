# M31A v1.6.0 — Final Rendering Profile

Wave 1 + Wave 2 complete. O1, O2, O4, O8 implemented.

---

## 1. Benchmark Summary

All measurements on Intel i3-1215U (12th Gen), Linux, amd64.

### Message Rendering (components package)

| Scenario | Time | Allocs | Bytes |
|----------|------|--------|-------|
| Idle view (10 msgs, cached) | **2.1 ms/frame** | **5,119** | **1.3 MB** |
| Streaming (100 ticks) | 0.3–0.5 ms/tick | ~500 | ~13 KB |
| Scroll (50 msgs, full) | 2.6 ms/render | 2,600 | 550 KB |
| 20 messages (1 pass) | 0.95 ms | 2,840 | 473 KB |
| 50 messages (1 pass) | 6.6 ms | 7,100 | 1.2 MB |
| 100 messages (1 pass) | 8.9 ms | 14,200 | 2.4 MB |
| Code block (single) | **0.67 ms/render** | **2,814** | **120 KB** |
| Markdown (single) | 0.60 ms/render | 2,500 | 100 KB |
| Resize (1000 changes) | 242 μs/resize | 500 | ~6 KB |

### Layout Rendering (layout package)

| Scenario | Time | Notes |
|----------|------|-------|
| Full frame (layout only) | **0.09 ms/frame** | Header + footer + JoinVertical |
| BuildHeader | 0.05 ms | lipgloss calls + renderContextMeter |
| BuildFooter | 0.02 ms | lipgloss calls |
| JoinVertical (compose) | 0.02 ms | lipgloss internal |

### Combined Frame Budget

| Component | ms/frame | % of 16ms budget |
|-----------|----------|-------------------|
| Message rendering (10 msgs) | 2.1 ms | 13.1% |
| Layout rendering | 0.09 ms | 0.6% |
| Bubble Tea event loop | ~0.1 ms | 0.6% |
| **Total per frame** | **~2.3 ms** | **14.4%** |
| **Headroom** | **~13.7 ms** | **85.6%** |

---

## 2. CPU Profile — Hotspot Ranking

### Message Render Path (2.1 ms per 10-msg frame)

| # | Hotspot | Flat CPU | Cum CPU | Owner | Category |
|---|---------|----------|---------|-------|----------|
| 1 | `ansi.stringWidth` → `Transition` | 60 ms (18%) | 110 ms (33%) | charmbracelet/x/ansi | **Lipgloss dep** |
| 2 | `lipgloss.Style.Render` | 30 ms (9%) | 170 ms (52%) | lipgloss | **Lipgloss** |
| 3 | `lipgloss.JoinVertical` | 0 | 60 ms (18%) | lipgloss | **Lipgloss** |
| 4 | `lipgloss.getLines` | 0 | 60 ms (18%) | lipgloss | **Lipgloss** |
| 5 | `lipgloss.alignTextHorizontal` | 0 | 40 ms (12%) | lipgloss | **Lipgloss** |
| 6 | `cellbuf.Wrap` | 10 ms (3%) | 40 ms (12%) | charmbracelet/x/cellbuf | **Lipgloss dep** |
| 7 | `lipgloss.Style.applyBorder` | 0 | 30 ms (9%) | lipgloss | **Lipgloss** |
| 8 | `uniseg.propertySearch` | 30 ms (15%) | 30 ms (15%) | rivo/uniseg | **Lipgloss dep** |
| 9 | `uniseg.FirstGraphemeCluster` | 20 ms (6%) | 50 ms (15%) | rivo/uniseg | **Lipgloss dep** |
| 10 | `lipgloss.Style.getAsBool` | 30 ms (9%) | 30 ms (9%) | lipgloss | **Lipgloss** |
| 11 | `StripANSI` | 10 ms (3%) | 10 ms (3%) | **M31A** | **M31A** |
| 12 | `renderContentSegment` (cache hit) | 10 ms (3%) | 10 ms (3%) | **M31A** | **M31A** |
| 13 | `runtime.gcBgMarkWorker` | 0 | 20 ms (10%) | Go runtime | **Runtime** |
| 14 | `runtime.mallocgc` | 0 | 20 ms (6%) | Go runtime | **Runtime** |

### Layout Render Path (0.09 ms per frame)

| # | Hotspot | Flat CPU | Cum CPU | Owner | Category |
|---|---------|----------|---------|-------|----------|
| 1 | `BuildHeader` | 20 ms (10%) | 90 ms (45%) | **M31A** (calling lipgloss) | **M31A+Lipgloss** |
| 2 | `BuildFooter` | 20 ms (10%) | 30 ms (15%) | **M31A** (calling lipgloss) | **M31A+Lipgloss** |
| 3 | `JoinVertical` | 0 | 40 ms (20%) | lipgloss | **Lipgloss** |
| 4 | `renderContextMeter` | 20 ms (10%) | 20 ms (10%) | **M31A** (calling uniseg) | **M31A+uniseg** |
| 5 | `uniseg.propertySearch` | 30 ms (15%) | 30 ms (15%) | rivo/uniseg | **Lipgloss dep** |
| 6 | `lipgloss.Style.Render` | 0 | 30 ms (15%) | lipgloss | **Lipgloss** |
| 7 | `runtime.gcBgMarkWorker` | 0 | 20 ms (10%) | Go runtime | **Runtime** |

---

## 3. Allocation Profile

### Message Rendering (5,119 allocs per 10-msg frame)

| # | Source | Allocs/frame | Category | Avoidable? |
|---|--------|-------------|----------|------------|
| 1 | `strings.genSplit` (from `StripANSI`) | 311 | **M31A** | **Yes — regex-free ANSI stripper** |
| 2 | `lipgloss.Style.Render` (internal) | 469 | Lipgloss | No |
| 3 | `ansi.ReadStyleColor` | 98 | Lipgloss dep | No |
| 4 | `cellbuf.Wrap` | 168 | Lipgloss dep | No |
| 5 | `strings.(*Builder).WriteString` | 132 | Lipgloss (Go stdlib) | No |
| 6 | `regexp.ReplaceAllString` (from `StripANSI`) | 157 | **M31A** | **Yes — regex-free ANSI stripper** |
| 7 | `bytes.growSlice` | 50 | Lipgloss (Go stdlib) | No |
| 8 | `termenv.Style.Foreground` | 33 | Lipgloss dep | No |
| 9 | `lipgloss.JoinHorizontal` | 22 | Lipgloss | No |
| 10 | `bytes.(*Buffer).grow` | 58 | Lipgloss (Go stdlib) | No |
| | **M31A-owned total** | **468 (9.1%)** | | |
| | **Lipgloss+deps total** | **4,232 (82.7%)** | | |
| | **Go runtime/GC** | **419 (8.2%)** | | |

### Layout Rendering (22,000 allocs per frame, 2000 simulated)

| # | Source | Allocs | Category |
|---|--------|--------|----------|
| 1 | `strings.(*Builder).WriteString` | 30,593 | Lipgloss internal |
| 2 | `BuildFooter` | 36,864 | **M31A** (calling lipgloss) |
| 3 | `termenv.Profile.Color` | 32,768 | Lipgloss dep |
| 4 | `BuildHeader` | 36,409 | **M31A** (calling lipgloss) |
| 5 | `renderProvBadge` | 32,768 | **M31A** (calling termenv) |
| 6 | `shortProviderName` | 32,768 | **M31A** (trivial) |

The `renderProvBadge` → `termenv.Profile.Color` path creates 32K allocs per 2000 frames = 16 allocs/frame. This is termenv's color profile lookup.

---

## 4. Rendering Ownership

### What percentage of CPU/allocs does each layer own?

```
Message Render (2.1 ms/frame, 5119 allocs)
├── Lipgloss + deps (ansi, cellbuf, uniseg, termenv)
│   CPU:  ~1.8 ms  (86%)
│   Alloc: ~4,232  (83%)
│   Components: Style.Render, JoinVertical, getLines,
│               alignTextHorizontal, stringWidth,
│               propertySearch, Wrap, applyBorder
│
├── M31A application code
│   CPU:  ~0.06 ms (3%)
│   Alloc: ~468    (9%)
│   Components: StripANSI (3 regex calls),
│               renderAssistantMessage (orchestration),
│               renderContentSegment (cache-hit path)
│
├── Go runtime (GC, malloc)
│   CPU:  ~0.2 ms  (10%)
│   Alloc: ~419    (8%)
│
└── Glamour (cached)
    CPU:  0 ms     (0%)
    Alloc: 0       (0%)

Layout Render (0.09 ms/frame, 22K allocs)
├── Lipgloss + deps
│   CPU:  ~0.04 ms (44%)
│   Alloc: ~60K+   (most)
│   Components: JoinVertical, Style.Render, Width,
│               getLines, uniseg.propertySearch
│
├── M31A application code
│   CPU:  ~0.05 ms (56%)
│   Alloc: ~100K+  (dominant by count)
│   Components: BuildHeader, BuildFooter,
│               renderContextMeter, renderProvBadge
│   NOTE: 99% of this time is spent inside lipgloss calls
│
└── Go runtime
    CPU:  ~0.01 ms (10%)
    Alloc: small
```

### Summary

| Owner | CPU (message) | CPU (layout) | Allocs (message) | Allocs (layout) |
|-------|--------------|--------------|-------------------|-----------------|
| **Lipgloss + deps** | 86% | 44% | 83% | ~70% |
| **M31A code** | 3% | 56%* | 9% | ~25% |
| **Go runtime/GC** | 10% | 10% | 8% | ~5% |
| **Glamour (cached)** | 0% | 0% | 0% | 0% |

*M31A's 56% in layout is almost entirely spent calling into lipgloss. M31A's own logic (string concat, margin values) is <5% of the 56%.

---

## 5. Hotspot-by-Hotspot Decision

### Hotspots that are Lipgloss (cannot modify)

| Hotspot | Decision | Reason |
|---------|----------|--------|
| `lipgloss.Style.Render` | **Ignore** | External library. No faster alternative. Would require full renderer rewrite. |
| `lipgloss.JoinVertical` | **Ignore** | External library. Core composition function. |
| `lipgloss.getLines` | **Ignore** | External library. Called internally by JoinVertical/Render. |
| `lipgloss.alignTextHorizontal` | **Ignore** | External library. Margins/alignment essential for layout. |
| `cellbuf.Wrap` | **Ignore** | External library. Word-wrap is fundamental. |
| `ansi.stringWidth` / `uniseg.propertySearch` | **Ignore** | External dependency (grapheme cluster detection). Deeply embedded in every string measurement. |
| `lipgloss.Style.applyBorder` | **Ignore** | External library. Borders essential for tool cards. |
| `lipgloss.Style.getAsBool/Int` | **Ignore** | External library. Property access, minimal overhead. |
| `termenv.Profile.Color` | **Ignore** | External dependency. Color profile detection. One-time per style. |
| `renderProvBadge` → `termenv.Profile.Color` | **Monitor** | 16 allocs/frame from termenv. Could be cached but saves only 0.3% of total allocs. |

### Hotspots that are M31A-owned

| Hotspot | Decision | Reason |
|---------|----------|--------|
| `StripANSI` (3 regex calls, 468 allocs/frame) | **Monitor** | Saves ~9% of frame allocs if replaced. But: saves only ~0.05ms CPU. Regex is correct and well-tested. Replacement risk (edge-case ANSI sequences) > gain. Not 10% of user-perceived responsiveness. |
| `BuildHeader` / `BuildFooter` (orchestration) | **Ignore** | These are orchestration functions that call lipgloss. Reducing their call count would mean caching rendered strings, but the content changes on every frame (spinner, context ring, time). |
| `renderContextMeter` (calls lipgloss.Width) | **Ignore** | Marginal. Uses cached styles. Width call is inherent to lipgloss. |
| `renderAssistantMessage` (orchestration) | **Ignore** | Already optimal: cache check → render segments → join. No unnecessary work. |
| `renderContentSegment` (cache-hit path) | **Ignore** | Cache hit = 1 map lookup + string append. ~3 μs per segment. Already optimal. |

### Hotspots that are Go runtime

| Hotspot | Decision | Reason |
|---------|----------|--------|
| `runtime.gcBgMarkWorker` | **Ignore** | GC is inherent to Go. Running at 10% of 2.1ms = 0.2ms. Well under budget. Reducing allocs (above) would reduce GC pressure, but not independently optimizable. |
| `runtime.mallocgc` | **Ignore** | Memory allocation is inherent. Cannot optimize without changing language. |

### Hotspots that are already optimized

| Hotspot | Decision | Reason |
|---------|----------|--------|
| Glamour rendering (O2 cache) | **Done** | Cache hit = 0 allocs, 0 CPU. 39x speedup on padded renders. |
| StyleCache (O1 threading) | **Done** | 0.18 ns/lookup vs 25 μs for BuildSemanticStyles. |
| Background worktree sweep (O8) | **Done** | 30s startup blocking → 0ms. |
| CodeBlock double-style (O4) | **Done** | Subsumed by O1. |

---

## 6. Has M31A Reached the Optimization Stop Point?

### Yes.

**Reasoning:**

1. **86% of CPU in the hot path belongs to Lipgloss.** This is not a weakness — it is the correct architecture. Lipgloss is the rendering engine. M31A's job is to assemble the correct inputs. Lipgloss's job is to produce styled terminal output. Optimizing Lipgloss internals would mean maintaining a fork of a widely-used library.

2. **83% of allocations belong to Lipgloss + deps.** These are internal buffers, grapheme cluster tables, style resolution caches, and word-wrap state. They are fundamental to the rendering process and cannot be eliminated without removing features (borders, margins, word-wrap, ANSI parsing).

3. **M31A's own code contributes 3% of CPU and 9% of allocs.** The only candidate (StripANSI regex replacement) would save ~468 allocs/frame and ~0.05ms CPU. This is 9% of allocs but 2.4% of frame time. It does not meet the 10% threshold for user-perceived responsiveness.

4. **Frame time is 2.3 ms against a 16 ms budget.** There is 85.6% headroom. Even doubling M31A's own code cost would not approach the budget limit. The rendering pipeline is not the bottleneck for this application.

5. **All previously identified hotspots have been addressed.** O1 eliminated 1356 allocs/frame. O2 eliminated glamour re-rendering (39x speedup). O4 eliminated double-styling. O8 eliminated 30s startup blocking. The remaining work was profiled and rejected (O11: 30 allocs, 0.48 μs — not worth 3-5 days).

6. **No change would improve user-perceived responsiveness by 10%.** The terminal refresh rate (limited by the terminal emulator, typically 60 Hz) is the true ceiling. At 2.3 ms/frame, M31A is rendering 435 frames/second. The terminal can display 60. The bottleneck is the terminal, not the application.

### What remains (for completeness, not for action)

| Item | Savings | Why not now |
|------|---------|-------------|
| Replace StripANSI regex with byte-scan | ~468 allocs/frame, ~0.05ms | 2.4% CPU gain. Regex is correct and well-tested. Risk of edge-case bugs. |
| Cache `termenv.Profile.Color` in renderProvBadge | ~16 allocs/frame | 0.3% of total allocs. Negligible. |
| Pool `bytes.Buffer` in lipgloss JoinVertical | Would require lipgloss fork | External library. Not our code. |

None of these meet the 10% threshold.

---

## 7. Recommendation

**Do not proceed with Wave 3.**

The rendering pipeline is sound. The optimization floor has been reached. Further work should focus on features, reliability, or UX — not rendering micro-optimization.

The final performance profile:

```
Frame budget:     16.0 ms
Used:              2.3 ms (14.4%)
Headroom:         13.7 ms (85.6%)
Allocs/frame:     5,119
  Lipgloss:       4,232 (83%)
  M31A:             468 (9%)
  Runtime:          419 (8%)
Owner of CPU:     Lipgloss (86%)
Owner of allocs:  Lipgloss (83%)
M31A contribution: 3% CPU, 9% allocs
```

**The best engineering trade-off for a production terminal application is to stop here.**
