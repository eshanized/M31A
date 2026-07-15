# DISCUSSION-LOG.md — Phase 8: Investigate and Fix TUI Blank Screens

## Session: 2026-07-16

### Phase Context
Investigate why `m31a` TUI shows blank screens in real terminal. All 30+ screens affected.

### Gray Areas Discussed

#### 1. TTY Requirement
**Question**: The 'could not open TTY' error happens when running in non-TTY environments (scripts, CI). How should we prioritize?
**Options**: Real terminal first / Headless testing / Both
**Selected**: Real terminal first
**Notes**: Bubble Tea requires TTY for alt-screen mode. Tests pass because they mock dimensions. CI can use virtual TTY (gotty, expect) later.

#### 2. WindowSizeMsg Timing
**Question**: Bubble Tea calls View() with 0×0 dims before first WindowSizeMsg. Our View() returns "" for 0 dims (correct). But maybe the first WindowSizeMsg isn't arriving?
**Options**: Expected behavior / Fix initial render / Alt-screen issue
**Selected**: Expected behavior
**Notes**: This is standard Bubble Tea pattern. Verify WindowSizeMsg arrives in real terminal with debug logging if needed.

#### 3. Theme/Color — Lipgloss Compatibility
**Question**: Theme uses dark mode only with brand colors. Could foreground/background colors be identical making text invisible? How to verify?
**Options**: Audit theme colors / High-contrast test / Lipgloss compatibility
**Selected**: Lipgloss compatibility
**Rationale**: Colors look correct (#0F1117 bg, #E2E4E9 fg) but lipgloss ANSI sequences may misrender on some terminals. Audit theme/cache styles.

#### 4. Screenable Implementation
**Question**: Router delegates View() to active Screenable. If Screenable not registered or active is nil, View() returns "". Which screens might be missing registration?
**Options**: Audit Screenable impl / Router active screen / Screen switch default
**Selected**: Audit Screenable impl
**Finding**: ReplModel missing SetDimensions (uses direct width/height fields). Need full audit of all 30+ screens.

#### 5. Dimension Calculation
**Question**: contentDimensions() computes content W/H minus chrome (2 rows) and sidebar. Could edge cases produce ≤0 content area?
**Options**: Expected behavior / Fix initial render / Alt-screen issue
**Selected**: Expected behavior
**Action**: Add guards in contentDimensions() for negative/zero content area. Check UltraNarrow threshold (40 cols).

### Decisions Made
1. **TTY**: Real terminal first priority
2. **WindowSizeMsg**: Accept standard Bubble Tea behavior
3. **Theme**: Audit lipgloss color output
4. **Screenable**: Full audit of all 30+ screens
5. **Dimensions**: Add edge case guards

### Deferred
- CI-compatible headless TUI testing
- Light/auto theme support
- Windows ARM64 target
EOF