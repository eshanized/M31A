# Phase 24: TUI Redesign — Research

## Current Codebase State

### Theme System (`internal/tui/theme/`)
- **76+ color tokens** across `Theme` struct (colors.go)
- **Both dark and light themes** exist with full token sets
- **`Mode` enum**: Dark, Light, Auto (termenv detection)
- **`Manager`**: wraps current theme, handles mode cycling
- Colors: Brand=#D77757 (violet) dark / #C45C3A light; Surface=#1A1A1A dark / #F8F9FA light
- Box drawing: `SplitBorder` exists; UserBubble/AssistantBubble use `RoundedBorder`; Modal uses `DoubleBorder`
- Tool label map: Bash=Warning, FileRead=Thinking, FileWrite=Brand, Glob=TextSecondary, Grep=Thinking

### Existing Screen Files (72 files in internal/tui/)
| Screen | File | Pattern | Issues |
|--------|------|---------|--------|
| FirstRun | `firstrun.go` | ASCII logo + list + textinput | Static, no flow animation, 24-col feature cards |
| REPL | `repl.go` + `repl_view.go` + `repl_stream.go` + `repl_thinking.go` | viewport + textarea + spinner | 937 lines; no role gutters, no timestamp bars |
| ModelSelector | `modelselector.go` + `modelselector_view.go` + `modelselector_list.go` | bubbles/list + search | No sparklines, no cost viz, no comparison |
| Settings | `settings.go` | 6-tab layout + inline editing | Unstyled tabs, cramped fields, no descriptions |
| Resume | `resume.go` | bubbles/list + preview | Plain preview, minimal filter chips |
| Plan | `plan.go` | task list + strings.Builder | No timeline, no file impact viz |
| Execute | `execute.go` | task list + spinner + metrics | Good segmented progress bar, plain task rows |
| Verify | `verify.go` | checklist + strings.Builder | Basic heal confirmation |
| Ship | `ship.go` | commit review | Same pattern |
| Diff | `diff.go` | custom renderer | Already has custom renderer |

### Shared Chrome
| Component | File | Current |
|-----------|------|---------|
| Header | `header.go` (header_test.go) | FNV cache, minimal: `M31A [OR] ModelName --/-- ctx [LIVE]` |
| StatusBar | `statusbar.go` | Left/right padding, sparse content |
| Sidebar | `sidebar.go` | Git status, 42-col fixed |
| CommandPalette | `cmdpalette.go` | Standard |
| Spinner | spinner_test.go | `bubbles/spinner` |

### Key Architecture Constraints
- `bubbletea` single-threaded: state mutations through `Update()` only
- `lipgloss` for all styling
- `bubbles` components: viewport, textarea, spinner, list
- `glamour` for markdown rendering
- Frame budget: 16ms for `View()`
- `types.Message` carries Segments, ToolCalls, Usage, Content
- `types.StreamChunk` has Type ("content"/"thinking"), Delta, ThinkingDuration

### File Impact Summary
| Package | Files to Modify | Files to Create |
|---------|----------------|-----------------|
| `internal/tui/theme/` | colors.go, theme.go | sparkline.go, borders.go |
| `internal/tui/` | repl.go, repl_view.go, firstrun.go, modelselector.go, settings.go, resume.go, plan.go, execute.go, verify.go, ship.go, diff.go | starfield.go, sparkline.go, gitstrip.go |
| `internal/tui/components/` | message_renderer.go, tool_card.go | timestamp_bar.go, file_impact.go, session_card.go, dependency_graph.go, live_metrics.go |
| `internal/tui/screens/` | All screen files | Redesigned layouts |

## Implementation Approach

### Wave 1: Theme Enhancement + Shared Primitives
Add new border/drawing primitives to theme, sparkline component, starfield renderer, timestamp bar, git status strip. These are pure additions — no existing code changes, zero regression risk.

### Wave 2: REPL + FirstRun Redesign
The two highest-impact screens. REPL is the primary interface; FirstRun is the onboarding experience. Both get the full visual overhaul from the proposal.

### Wave 3: Workflow Screens (Plan, Execute, Verify, Ship)
These share the task list pattern. Redesign together for consistency. Plan gets the most work (dependency graph, file impact). Execute gets the live metrics bar.

### Wave 4: Utility Screens (ModelSelector, Settings, Resume)
Model Observatory gets sparklines and cost bars. Control Tower gets icon tabs and two-column layout. Session Vault gets timeline view.

### Wave 5: Integration Testing
Wire all redesigned screens into app routing. Test graceful degradation at 80/120/160/200 cols. Verify 16ms frame budget. Race detector pass.

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Frame budget exceeded with complex layouts | HIGH | Profile View() early; cache renders; cap tool output |
| Theme color mismatch between old and new | MEDIUM | Side-by-side diff of Dark()/Light() outputs |
| bubbles/viewport scroll behavior broken | MEDIUM | Keep viewport.Model as underlying scroll engine |
| Cross-platform rendering differences | LOW | lipgloss handles this; test on CI matrix |
| Existing tests break | MEDIUM | Run `go test -race ./...` after each wave |

## Validation Architecture
- Unit tests for each new component (sparkline, starfield, etc.)
- Integration test for REPL streaming with redesigned rendering
- Theme cycle test (dark → light → auto → dark) with all screens
- Frame budget benchmark: View() < 16ms at 120 cols
- Graceful degradation test: all screens render at 80, 120, 160, 200 cols

## RESEARCH COMPLETE
