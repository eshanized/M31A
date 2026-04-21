# UX Comparison Report: OpenCode → M31A
## Actionable Patterns to Elevate M31A's Terminal Experience

**Date**: 2026-06-13
**Scope**: UX patterns from OpenCode that M31A can adopt or adapt

---

## Executive Summary

OpenCode is a mature, production-grade AI coding agent with a polished TUI built on SolidJS/OpenTUI, offering 33+ themes, a command palette, leader-key system, plugin architecture, and extensive configuration layering. M31A is a well-designed Go-native CLI with Bubble Tea, a six-phase workflow engine, and strong differentiators (arbitrage, bisect, ledger). This report identifies **12 high-impact UX patterns** from OpenCode that M31A should adopt, organized by effort and impact.

---

## 1. Command Palette (Ctrl+P) — HIGH IMPACT, LOW EFFORT

### OpenCode Pattern
**File**: `packages/opencode/src/cli/cmd/tui/component/command-palette.tsx`

OpenCode's command palette is a fuzzy-searchable overlay (Ctrl+P) that surfaces all available actions across categories (Session, Agent, System, Provider). It includes:
- Fuzzy search with ranked results
- Categorized results with keybinding hints
- Suggested commands section
- Slash command aliases (`/sessions`, `/models`, `/connect`)

### M31A Current State
M31A has a command palette (`Ctrl+P`) but it's limited to slash command browsing. It lacks:
- Fuzzy search ranking
- Category grouping in results
- Keybinding display per command
- Context-aware suggestions (recently used, most used)

### Recommendation
Enhance M31A's command palette to match OpenCode's:
1. Add **fuzzy search with scoring** (Levenshtein is already used for typo suggestions — extend to search ranking)
2. Add **category headers** in the palette results (Core, Workflow, AI/Model, Git, Settings)
3. Show **keybinding hints** next to each command
4. Track **frequency of use** and sort suggested commands by frecency
5. Add **context-aware filtering** — when in Plan phase, prioritize workflow commands

### Effort: ~1-2 days
### Impact: High — improves discoverability for all users

---

## 2. Theme System with 30+ Built-in Themes — MEDIUM IMPACT, MEDIUM EFFORT

### OpenCode Pattern
**File**: `packages/opencode/src/cli/cmd/tui/context/theme.tsx`

OpenCode ships 33+ themes (catppuccin, dracula, gruvbox, nord, tokyonight, rosepine, solarized, monokai, material, github, etc.) with:
- Full dark/light mode support with system detection
- Theme locking (force dark or light)
- Custom themes via `themes/*.json` in config directories
- Plugin-contributed themes
- Comprehensive color schema: primary, secondary, accent, error, warning, success, info, text, textMuted, background, backgroundPanel, border, diff colors, markdown colors, syntax highlighting colors
- `thinkingOpacity` for inline thinking blocks

### M31A Current State
M31A has dark/light/auto modes with accent color override and border style options (rounded, double, thin, split). It has ~80 theme properties but no built-in theme library.

### Recommendation
1. Ship **10-15 built-in themes** as Go embedded structs (catppuccin, dracula, gruvbox, nord, tokyonight, rosepine, solarized, github-dark, monokai)
2. Add a **`/theme` picker** screen (M31A already has `ThemePicker` screen — populate it with previews)
3. Support **theme files** (`~/.m31a/themes/*.toml`) for user-created themes
4. Add `thinkingOpacity` equivalent — control how prominent thinking blocks appear
5. Add **syntax highlighting color rules** per theme for code blocks

### Effort: ~3-5 days
### Impact: Medium — major polish factor, user retention

---

## 3. Plugin/Extension System with TUI Slots — HIGH IMPACT, HIGH EFFORT

### OpenCode Pattern
**Files**: `packages/plugin/src/index.ts`, `packages/plugin/src/tui.ts`

OpenCode has a dual plugin system:
- **Server plugins**: Hooks for config, tools, auth, providers, chat messages, permissions, shell env
- **TUI plugins**: UI injection via named slots (`home_logo`, `home_prompt`, `sidebar_title`, `sidebar_content`, `session_prompt`, etc.)

TUI slot system:
```typescript
type TuiSlots = {
  app, app_bottom, home_logo, home_prompt, home_prompt_right,
  home_bottom, home_footer, session_prompt, session_prompt_right,
  sidebar_title, sidebar_content, sidebar_footer
}
```

Plugins can add:
- Custom UI components in named locations
- Custom themes
- Custom commands
- Custom tools
- Event handlers

### M31A Current State
M31A has a tool interface (`types.Tool`) and MCP integration for external tools, but no plugin system for UI extension or hook-based behavior modification.

### Recommendation
Design a plugin system for M31A in two phases:

**Phase 1 — Tool Plugins (V1.1)**:
- Define a `Plugin` interface with `Name()`, `Version()`, `Tools()`, `Hooks()`
- Register plugins in config: `[[plugins]] name = "my-plugin" path = "./plugin.so"`
- Use Go plugin system (`.so` files) or embed plugins as Go modules

**Phase 2 — TUI Slots (V1.2)**:
- Define named slots in the Bubble Tea view: `HeaderLeft`, `HeaderRight`, `FooterLeft`, `FooterRight`, `Sidebar`, `StatusBar`
- Plugins register components that render into slots
- Slot rendering is composited in `app_view.go`

### Effort: ~2-3 weeks
### Impact: High — enables community ecosystem

---

## 4. Configuration Layering with Deep Merge — MEDIUM IMPACT, LOW EFFORT

### OpenCode Pattern
**File**: `packages/opencode/src/config/config.ts`

OpenCode loads configuration in strict precedence order:
1. Well-known remote config (from auth URLs)
2. Global config (`~/.config/opencode/opencode.json`)
3. Custom config (`OPENCODE_CONFIG` env var)
4. Project config files (walking up directory tree)
5. `.opencode` directories
6. `OPENCODE_CONFIG_CONTENT` env var
7. Active account/org config
8. Managed config directories
9. macOS managed preferences (`.mobileconfig`)

Key behaviors:
- **Deep merge** with array concatenation for instructions
- **JSONC support** (comments in config files)
- **Graceful degradation** — invalid config skips rather than crashes
- **Unknown key warnings** for typo detection

### M31A Current State
M31A has a 5-layer config system (defaults → global TOML → .env → env vars → project TOML) with variable substitution and validation. It walks up 3 parent dirs for project config.

### Recommendation
1. Add **JSONC/TOML comment support** in project config (already has TOML — this is native)
2. Implement **deep merge** for nested config sections (currently uses overwrite semantics)
3. Add **config inheritance** — project config can `extend` global config: `extends = "~/.m31a/config.toml"`
4. Add **config diff view** — `/settings diff` shows which values differ from defaults
5. Add **config freeze** — export current effective config to a file

### Effort: ~2-3 days
### Impact: Medium — power user workflow improvement

---

## 5. Permission System with Glob Patterns and Risk Levels — ALREADY GOOD, ENHANCE

### OpenCode Pattern
**File**: `packages/opencode/src/config/permission.ts`

OpenCode's permission system:
- Actions: `ask`, `allow`, `deny`
- Targets: `read`, `edit`, `glob`, `grep`, `list`, `bash`, `task`, `external_directory`, `todowrite`, `question`, `webfetch`, `websearch`, `repo_clone`, `repo_overview`, `lsp`, `doom_loop`, `skill`
- Per-pattern rules: `"*.ts": "allow"`, `"*.json": "deny"`

### M31A Current State
M31A already has a solid permission system with glob patterns, risk levels (low/medium/high/destructive), per-agent profiles, and rate limiting.

### Recommendation
M31A's permission system is already comparable or better. Enhancements:
1. Add **per-file-extension rules** like OpenCode: `[[permissions.rules]] pattern = "*.go" action = "allow"`
2. Add **session-scoped allow-always** — "allow for this session" in addition to "allow always"
3. Add **permission audit log** — track all permission decisions for review
4. Add **dry-run mode** — `/permissions dry-run` shows what would be allowed/denied without executing

### Effort: ~1-2 days
### Impact: Low-Medium — incremental improvement

---

## 6. Leader Key System with Which-Key Overlay — ALREADY GOOD, POLISH

### OpenCode Pattern
**File**: `packages/opencode/src/cli/cmd/tui/config/keybind.ts`

OpenCode's leader key system:
- Default leader: `ctrl+x`
- Configurable timeout
- 200+ configurable bindings
- Which-key overlay shows available chords when leader is active
- Categories: App, Session, Model/Agent, Message navigation, Input editing, Dialog navigation, Plugin controls

### M31A Current State
M31A has a leader key system (Ctrl+X, configurable timeout) with two-key chords and a which-key overlay.

### Recommendation
M31A's system is already solid. Enhancements:
1. Add **keybinding customization** via config file (currently hardcoded in `keybindings.go`)
2. Add **keybinding presets** — vim, emacs, custom
3. Add **keybinding conflict detection** at startup
4. Add **keybinding search** — `/keybind search "model"` finds all bindings for model-related actions
5. Export keybinding documentation as `/keybinds` command

### Effort: ~2-3 days
### Impact: Low-Medium — power user quality of life

---

## 7. Session Management with Fork/Branch — HIGH IMPACT, MEDIUM EFFORT

### OpenCode Pattern
**File**: `packages/opencode/src/cli/cmd/run.ts`

OpenCode session features:
- `--continue/-c` resumes last session
- `--session/-s <id>` resumes specific session
- `--fork` creates a branch from existing session
- Session sharing (`--share`)
- Session export
- Session rename, delete
- Session history search
- SQLite-backed persistence

### M31A Current State
M31A has session resume with file-based state (PROJECT.md, TASKS.md, STATE.md) and session archival. Sessions are stored in `~/.m31a/sessions/<id>/`.

### Recommendation
1. Add **session forking** — `/fork` creates a copy of current session with new ID, preserving all state files
2. Add **session search** — `/resume search <query>` filters sessions by goal text
3. Add **session tags** — tag sessions for categorization (`/tag add web`, `/resume tag:web`)
4. Add **session comparison** — `/session diff <id1> <id2>` shows differences between two sessions
5. Add **session export** — `/export` writes session to a shareable Markdown file
6. Add **session sharing** — generate a public URL for session summary (via a hosted service or local server)

### Effort: ~3-5 days
### Impact: High — improves multi-project workflows

---

## 8. Error Handling with Actionable Messages — MEDIUM IMPACT, LOW EFFORT

### OpenCode Pattern
**File**: `packages/opencode/src/cli/error.ts`

OpenCode's error handling:
- `CliError` class with message + exitCode
- `FormatError()` handles 12+ error types with specific messages
- `ProviderModelNotFoundError` suggests similar models
- `ConfigDirectoryTypoError` catches common mistakes
- `ConfigInvalidError` lists all validation issues
- `UICancelledError` silently exits (no error message)

### M31A Current State
M31A has sentinel errors with `UserMessage()` mappings and pattern-matching fallbacks. It's already good.

### Enhancement
1. Add **error suggestions** — "Did you mean `/model`?" for typos
2. Add **error recovery hints** — "Run `/settings` to update your API key" (already partially done)
3. Add **error grouping** — batch similar errors into a single notification
4. Add **error history** — `/errors` shows last N errors with timestamps and context
5. Add **silent exit** for user cancellation (match OpenCode's `UICancelledError` pattern)

### Effort: ~1-2 days
### Impact: Medium — reduces friction

---

## 9. Plugin TUI API with State, Events, and Lifecycle — HIGH IMPACT, HIGH EFFORT

### OpenCode Pattern
**File**: `packages/plugin/src/tui.ts`

OpenCode exposes a rich TUI API to plugins:
```typescript
type TuiPluginApi = {
  app, attention, keys, keymap, mode, route, ui, tuiConfig, kv, state,
  theme, client, event, renderer, slots, plugins, lifecycle
}
```

Features:
- **Route system** — plugins can register custom screens
- **Event bus** — plugins can subscribe to app events
- **KV store** — persistent key-value storage per plugin
- **Lifecycle hooks** — `onActivate`, `onDeactivate`
- **Dialog system** — plugins can open dialogs (alert, confirm, prompt, select)
- **Toast notifications** — plugins can show toasts

### M31A Current State
M31A has no plugin API. Tools implement a simple interface. No UI extension points.

### Recommendation
Design a Go-native plugin API:

```go
type Plugin interface {
    Name() string
    Version() string
    Activate(ctx PluginContext) error
    Deactivate() error
}

type PluginContext struct {
    App     AppStateReader     // Read-only app state
    Events  EventEmitter       // Subscribe/publish events
    UI      UIService          // Open dialogs, show toasts
    KV      KVStore            // Persistent key-value storage
    Config  ConfigReader       // Read config values
    Routes  RouteRegistry      // Register custom screens
}
```

### Effort: ~3-4 weeks
### Impact: High — enables extensibility ecosystem

---

## 10. Theme-Driven Syntax Highlighting in Code Blocks — MEDIUM IMPACT, LOW EFFORT

### OpenCode Pattern
**File**: `packages/plugin/src/tui.ts` (theme colors)

OpenCode themes define syntax highlighting colors:
```typescript
syntaxComment: RGBA
syntaxKeyword: RGBA
syntaxFunction: RGBA
syntaxVariable: RGBA
syntaxString: RGBA
```

These are applied to code blocks rendered via Glamour/Chroma.

### M31A Current State
M31A uses Glamour for markdown rendering and Chroma for syntax highlighting, but theme-driven syntax colors are not configurable per theme.

### Recommendation
1. Add syntax color properties to theme struct:
   ```go
   type Theme struct {
       // ... existing fields ...
       SyntaxComment  string `toml:"syntax_comment"`
       SyntaxKeyword  string `toml:"syntax_keyword"`
       SyntaxFunction string `toml:"syntax_function"`
       SyntaxVariable string `toml:"syntax_variable"`
       SyntaxString   string `toml:"syntax_string"`
       SyntaxNumber   string `toml:"syntax_number"`
   }
   ```
2. Map theme syntax colors to Chroma style on theme load
3. Each built-in theme ships with curated syntax colors

### Effort: ~1-2 days
### Impact: Medium — visual polish

---

## 11. Toast Notification System with Variants and Auto-Dismiss — ALREADY GOOD, POLISH

### OpenCode Pattern
**File**: `packages/opencode/src/cli/cmd/tui/ui/toast.tsx`

OpenCode toasts:
- Variants: `info`, `success`, `warning`, `error`
- Auto-dismiss with configurable duration
- Theme-colored variant icons
- Stack management (prevent overlap)

### M31A Current State
M31A already has a toast system with:
- Types: success, error, warning, info
- Up to 3 visible simultaneously
- Auto-dismiss with progress bar animation
- Slide-in animation frames
- Position: top-right, adapts for narrow terminals

### Recommendation
M31A's toast system is already comparable or better. Minor enhancements:
1. Add **action toasts** — toasts with clickable actions ("Undo" button)
2. Add **persistent toasts** — toasts that don't auto-dismiss until dismissed manually
3. Add **toast history** — `/toasts` shows recent toast notifications
4. Add **toast queuing** — when >3 toasts, queue and show in order

### Effort: ~1 day
### Impact: Low — incremental polish

---

## 12. Help System with Context-Sensitive Hints — ALREADY GOOD, POLISH

### OpenCode Pattern
**File**: `packages/opencode/src/cli/cmd/tui/ui/dialog-help.tsx`

OpenCode's help:
- Simple help dialog explaining command palette
- External docs at opencode.ai/docs
- 20+ language README translations

### M31A Current State
M31A has:
- Scrollable help screen with 8 sections
- Context-sensitive footer hints per screen
- 11 docs in `/docs/` directory
- Comprehensive CONTRIBUTING.md

### Recommendation
M31A's help system is already stronger. Enhancements:
1. Add **interactive tutorial** — first-run walkthrough of key features
2. Add **tooltip system** — hover/long-press on UI elements shows contextual help
3. Add **help search** — `/help search <query>` searches help content
4. Add **help for current screen** — `?` shows help specific to current screen (partially done via context hints)

### Effort: ~2-3 days
### Impact: Low-Medium — improves onboarding

---

## Priority Matrix

| Priority | Pattern | Impact | Effort | Recommendation |
|----------|---------|--------|--------|----------------|
| **P0** | Command Palette Enhancement | High | Low | Implement immediately |
| **P0** | Session Forking | High | Medium | Add to V1.1 roadmap |
| **P1** | Theme System (10+ built-in) | Medium | Medium | Add to V1.1 roadmap |
| **P1** | Error Suggestions | Medium | Low | Implement immediately |
| **P1** | Syntax Highlighting per Theme | Medium | Low | Implement immediately |
| **P2** | Plugin System (Tool Plugins) | High | High | Design for V1.2 |
| **P2** | Plugin TUI API | High | High | Design for V1.2 |
| **P2** | Config Deep Merge | Medium | Low | Implement in V1.1 |
| **P3** | Keybinding Customization | Low-Med | Low | Implement in V1.1 |
| **P3** | Session Search/Tags | High | Medium | Implement in V1.1 |
| **P3** | Toast Actions | Low | Low | Implement in V1.1 |
| **P3** | Help Search | Low-Med | Low | Implement in V1.2 |

---

## Implementation Roadmap

### V1.1 (Next Release)
1. Command Palette enhancement (fuzzy search, categories, keybinding hints)
2. Error suggestions and recovery hints
3. Session forking (`/fork`)
4. Session search (`/resume search`)
5. Config deep merge
6. Keybinding customization via config
7. 10 built-in themes (catppuccin, dracula, gruvbox, nord, tokyonight, rosepine, solarized, github-dark, monokai, material)
8. Theme picker screen population
9. Syntax highlighting per theme

### V1.2
1. Plugin system (tool plugins via Go modules)
2. TUI slot system for UI injection
3. Plugin API (events, KV store, dialogs, toasts)
4. Theme files (`~/.m31a/themes/*.toml`)
5. Interactive tutorial
6. Help search
7. Toast actions and persistence
8. Session export and sharing
9. Permission audit log

---

## Key Takeaway

M31A already has a strong UX foundation — its six-phase workflow, file-based state, and cost arbitrage are unique differentiators. The highest-leverage improvements from OpenCode are:

1. **Command Palette polish** — immediate discoverability win
2. **Theme library** — visual appeal and user retention
3. **Plugin architecture** — community ecosystem enablement
4. **Session forking** — power user workflow improvement

These patterns don't replace M31A's strengths — they amplify them.
