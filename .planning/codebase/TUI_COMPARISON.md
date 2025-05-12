# OpenCode TUI vs M31A TUI — Comprehensive Comparison

## Architecture

| Aspect | OpenCode | M31A |
|---|---|---|
| **Language** | TypeScript / SolidJS | Go / Bubble Tea |
| **Rendering** | @opentui framework, 60fps targeted | charmbracelet (lipgloss + bubbles + glamour) |
| **Navigation** | Route-based (`Home` / `Session` routes via `<Switch>/<Match>`) | Screen-based enum (`ScreenREPL`, `ScreenSettings`, etc.) with 10 screens |
| **State model** | SolidJS signals (`createSignal`, `createMemo`, `createEffect`) | Bubble Tea single-threaded `Update()` + `tea.Cmd`/`tea.Msg` |
| **Extensibility** | Plugin slots (`sidebar_content`, `home_bottom`, etc.) with `TuiPluginRuntime.Slot` | None — monolithic |

## Screen Layout

### OpenCode Session View

```
┌──────────────────────────────────────────────────────────────┐
│  [row, flexGrow=1]                                           │
│  ┌────────────────────────────┐  ┌────────────────────────┐  │
│  │ <scrollbox> flexGrow=1     │  │ Sidebar (42px, cond.) │  │
│  │                            │  │                        │  │
│  │  [UserMessage]             │  │ [Session title]       │  │
│  │  [AssistantMessage]        │  │ [Workspace]           │  │
│  │    - TextPart (markdown)   │  │ [Share URL]           │  │
│  │    - ToolPart (inline/block)│ │ [plugin: sidebar_     │  │
│  │    - ReasoningPart         │  │  content]             │  │
│  │                            │  │                        │  │
│  │  [PermissionPrompt]        │  │ [OpenCode version]    │  │
│  │  [QuestionPrompt]          │  └────────────────────────┘  │
│  │  [Prompt] ← pinned bottom  │                              │
│  │  [Toast]                   │                              │
│  └────────────────────────────┘                              │
└──────────────────────────────────────────────────────────────┘
```

- Sidebar is conditionally visible: hidden for subagents, auto-shown when width > 120, manual toggle
- Content area: `dimensions.width - (sidebarVisible ? 42 : 0) - 4`
- Messages scroll with `stickyScroll="bottom"` (auto-scrolls to latest)
- Permission prompts and input are `flexShrink={0}` — always visible, never scrolled

### M31A REPL Screen

```
┌─────────────────────────────────────┐
│ [Header]  [OR]  model  ctx  [LIVE]  │  ← 1 line, always visible
├─────────────────────────────────────┤
│                                     │
│  [Viewport — scrollable]            │  ← fills remaining height
│   - Welcome screen (empty)          │
│   - User messages (right bubbles)   │
│   - Assistant messages (markdown)   │
│   - Thinking blocks                 │
│   - Tool cards                      │
│                                     │
├─────────────────────────────────────┤
│ [Ready]              Last: 15:04:05 │  ← 1 line, always visible
├─────────────────────────────────────┤
│ ┌─────────────────────────────────┐ │
│ │ Type a message, /command...     │ │  ← 3 lines, textarea
│ │                                 │ │
│ │                                 │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

- Three vertical sections: header (1 line), viewport (remaining), status bar (1 line), textarea (3 lines)
- No sidebar
- No conditional layout based on terminal width (only a 40x10 minimum check)
- Viewport height = `totalHeight - 2(header) - 3(input)`

## Message Display

### User Messages

| Aspect | OpenCode | M31A |
|---|---|---|
| **Alignment** | Left-aligned with left border in agent color | Right-aligned bubble (max 70% width) |
| **Styling** | Left border box, agent color, `QUEUED` badge for pending | Rounded border, elevated surface bg (`#242424`), primary text |
| **Attachments** | File badges with MIME type (`txt`, `img`, `pdf`, `dir`) with colored backgrounds | None |
| **Compaction** | Shows compaction separator when context was compacted | None |
| **Timestamps** | Toggleable per-message timestamps | None |
| **Revert markers** | "X message reverted" banner with diff summary | None |

### Assistant Messages

| Aspect | OpenCode | M31A |
|---|---|---|
| **Content rendering** | PART_MAPPING dispatch: `text`→TextPart, `tool`→ToolPart, `reasoning`→ReasoningPart | Glamour markdown renderer for content segments |
| **Footer line** | `▣ {mode} · {model} · {duration} · interrupted` per message | None |
| **Subagent hint** | Shows `childShortcut + " view subagents"` when task tool exists | None |
| **Error display** | Red left-border error box (non-abort errors) | None |
| **Markdown** | `<markdown>` with syntax styling, conceal, streaming modes | Glamour with dark/light style + word wrap |

## Tool Rendering

### OpenCode — Dual Inline/Block Pattern

Each tool component (Shell, Write, Edit, Glob, Grep, Read, WebFetch, WebSearch, Task, ApplyPatch, TodoWrite, Question, Skill, GenericTool) follows:

- **InlineTool**: Single-line with icon + pending text + completion state. Shown while running or when no detailed output.
- **BlockTool**: Expandable block with left border, title, formatted content. Shown when output/diagnostics available.

Tool-specific rendering:
- **Edit / ApplyPatch**: `<diff>` element with unified or split view (auto-selected when `contentWidth > 120`)
- **Shell**: Command + output, expand/collapse for large output (max 10 lines * width chars)
- **GenericTool**: Hidden by default, requires `showGenericToolOutput` toggle; falls back to inline icon

### M31A — Single ToolCard Pattern

```
┌──────────────────────────────────────┐
│  [ Bash ]                            │  ← tool name badge (colored)
│  $ git status                        │  ← humanized input
│  On branch main...                   │  ← output (expanded only)
│   OK  Completed in 1.23s             │  ← status footer
└──────────────────────────────────────┘
```

| Aspect | OpenCode | M31A |
|---|---|---|
| **Display pattern** | Inline when running, Block when complete | Single card, always shown |
| **Collapse logic** | User-controlled + large output | Auto-collapse: >20 lines, binary, or truncated |
| **Diff view** | Split/unified diff for Edit/Patch | None |
| **Per-tool rendering** | Specialized component per tool type | Generic card, humanized input text only |
| **Running state** | Spinner in inline tool | `[..] Running...` with spinner in card footer |
| **Completion state** | InlineTool with completion indicator | `OK` badge + duration |
| **Output sanitization** | N/A (framework-level) | `SanitizeOutput()` strips ANSI + control chars |

## Thinking / Reasoning Display

| Aspect | OpenCode | M31A |
|---|---|---|
| **Default state** | Collapsed (configurable via `"hide"` mode) | Collapsed |
| **Collapsed view** | Header: `[+/-] Thought: {title} · {duration}` with spinner while running | `[▼] Thinking ({duration})` |
| **Expanded view** | Full markdown body in muted fg, rendered as `<code filetype="markdown">` | Header + `─` separator + content with 1-char padding in secondary text |
| **Toggle** | Per-block expand/collapse via `+/-` icon | Global `t`/`T` key toggles ALL blocks |
| **Duration format** | `{duration}` | `3.2s` / `12.5s` / `1m 23s` |

## Input Area

### OpenCode Prompt

```
┌──────────────────────────────────────────────────┐
│ │ [textarea, minHeight=1, maxHeight=floor(h/3)] │ │
│ │                                                │ │
│ │ [Agent] · [Model] [Provider] [· Variant]      │ │
│ └────────────────────────────────────────────────┘ │
│ ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀ │
│ [Spinner + retry]           [Editor · Usage · /?] │
│ [Autocomplete dropdown]                           │
└──────────────────────────────────────────────────┘
```

- Left border color dynamically matches agent color
- Shell mode: triggered by `!` at cursor position 0, changes border to `theme.primary`
- Metadata row: Agent, Model, Provider, Variant with fade-in animations
- Status bar below: spinner/retry status, editor file context, usage (tokens + cost), shortcut hints
- Autocomplete dropdown for slash commands, file paths
- Extmark/part system for inline badges (file attachments, agent switches)
- Max height: `max(6, floor(height/3))`

### M31A REPL Input

```
┌─────────────────────────────────────┐
│ Type a message, /command, goal...   │
│                                     │
│                                     │
└─────────────────────────────────────┘
```

- Fixed 3-line `bubbles/textarea`
- No metadata row (agent/model shown in header only)
- No autocomplete dropdown
- No shell mode
- No extmark/part system
- Input history via Up/Down arrows
- No character limit, no newline insertion (Enter submits)
- Placeholder: `"Type a message, /command, or goal..."`

## Header / Status

### OpenCode — No traditional header

OpenCode does not have a top status bar. Provider/model/agent info is shown in the prompt's metadata row. Health status is shown in the footer status indicators.

### M31A — Header + Status Bar

**Header (1 line):**
```
[M31A]  [OR]  claude-sonnet-4-202...  12.5K/128K ctx  [LIVE]
```
- Brand badge, provider badge, model name (truncated to 20 chars), context usage with color coding, health status

**Status Bar (1 line):**
```
[Ready]                    Last activity: 15:04:05
```
- Left: current operation in model badge, or "Ready"
- Right: last activity timestamp

## Sidebar / File Tree

| Aspect | OpenCode | M31A |
|---|---|---|
| **Exists** | Yes — 42px fixed width, conditionally visible | No |
| **Content** | Session title, workspace, share URL, plugin slots, version footer | N/A |
| **File tree** | `feature-plugins/sidebar/files.tsx` — "Modified Files" list with additions/deletions | N/A |
| **Visibility** | Auto-shows when width > 120, manual toggle, hidden for subagents | N/A |
| **Plugin slots** | `sidebar_title`, `sidebar_content`, `sidebar_footer` | N/A |
| **Narrow terminal** | Becomes absolute-positioned overlay with semi-transparent bg | N/A |

## Dialog System

### OpenCode

- Stack-based dialog system via `DialogProvider` / `useDialog()`
- `dialog.replace(<Component />)` — replaces entire stack
- Backdrop: full-screen, semi-transparent black, z-index 3000
- Content panel: `theme.backgroundPanel`, variable width (60/88/116 for medium/large/xlarge)
- Dismissal: Escape, Ctrl+C, or mouse click on backdrop
- Mode stack: `"modal"` mode pushed during dialogs, isolating keybindings
- Mouse copy-on-select support
- Dialog types: Alert, Confirm, Timeline, Fork, SessionRename, RetryAction, ExportOptions, ProviderConnect, WorkspaceUnavailable, Skill, Stash, SessionSwitcher (two-pane with search, pin, delete, rename)

### M31A

- Screen-based modals: `ScreenPermission`, `ScreenPlan`, `ScreenModelSelector`, `ScreenSettings`
- No overlay/backdrop pattern — entire screen switches
- Permission modal: y/a/n/e keyboard actions
- No dialog stacking
- No mouse support
- No z-index layering

## Keyboard Bindings

### OpenCode — 170+ bindings, centralized config

| Category | Count | Examples |
|---|---|---|
| Application | 8 | `ctrl+c` exit, `ctrl+p` command palette, `ctrl+x` leader |
| Session | 20+ | `ctrl+n` new, `ctrl+l` list, `ctrl+g` timeline, `escape` interrupt |
| Navigation | 10+ | Page up/down, half-page, first/last, next/previous message |
| Model/Agent | 12+ | `ctrl+m` list, `f2` cycle, `tab` switch agent |
| Prompt/Input | 37+ | Full readline-style editing (word forward/back, delete word, undo/redo) |
| Dialog | 6+ | Navigate, submit, page up/down |
| Autocomplete | 4+ | Prev/next, select, complete |
| Which-key | 10+ | Toggle, layout, group navigation |
| Diff viewer | 12+ | Toggle split/unified, expand/collapse, file navigation |

Key features:
- Leader key (`ctrl+x`) with configurable timeout
- Mode stack for context-sensitive bindings
- Slash commands: `/sessions`, `/resume`, `/continue`, `/skills`, `/warp`, `/editor`
- Command palette (`ctrl+p`) with fuzzy search

### M31A — ~15 bindings, scattered across files

| Category | Keys |
|---|---|
| Global | `Ctrl+C` (cancel stream / quit), `x` (dismiss banner) |
| REPL | `Enter` submit, `Up/Down` history, `PgUp/PgDown` scroll, `t/T` toggle thinking, `Esc` clear input |
| Permission | `y/Y` allow once, `a/A` allow always, `n/N` deny, `e/E` exit |
| Slash commands | `/settings`, `/resume`, `/models`, `/phase`, `/workflow` |

Key differences:
- No leader key
- No command palette
- No word-level editing commands
- No model/agent cycling via keyboard
- No session management shortcuts
- No which-key system

## Plugin Architecture

### OpenCode

- `TuiPluginRuntime.Slot` with named extension points:
  - `app`, `app_bottom`
  - `home_logo`, `home_prompt`, `home_prompt_right`, `home_bottom`, `home_footer`
  - `sidebar_title`, `sidebar_content`, `sidebar_footer`
  - `session` plugin slots
- Slot modes: `single_winner`, `replace`
- Plugins receive `TuiPluginApi` with access to: `state.session.diff()`, `ui.dialog`, `keymap`, `route`, `theme`, `toast`, etc.
- Internal plugins: session switcher, sidebar files, etc.

### M31A

- No plugin system
- All features hardcoded

## Visual Hierarchy & Styling

### OpenCode

| Token | Usage |
|---|---|
| `theme.text` | Primary text |
| `theme.textMuted` | Secondary/dimmed text |
| `theme.background` | Terminal background |
| `theme.backgroundPanel` | Dialog/sidebar backgrounds |
| `theme.backgroundElement` | Prompt container |
| `theme.border` | Borders |
| `theme.borderSubtle` | Subtle borders |
| `theme.primary` | Accent/brand color |
| `theme.success` | Green (live, OK) |
| `theme.error` | Red (errors) |
| `theme.warning` | Yellow (warnings, variant badges) |
| `theme.diffAdded` | Green diff stats |
| `theme.diffRemoved` | Red diff stats |
| `theme.accent` | Accent highlights |
| Agent colors | Per-agent dynamic coloring for borders, badges, highlights |

### M31A

| Token | Hex | Usage |
|---|---|---|
| Background | `#0D0D0D` | Terminal background |
| Surface | `#1A1A1A` | Assistant bubbles, input |
| SurfaceElevated | `#242424` | User bubbles, tool cards, modals |
| Border | `#2E2E2E` | Border lines |
| Brand | `#D77757` | Header, badges, progress |
| TextPrimary | `#E8EAED` | Main text |
| TextSecondary | `#9AA0A6` | Subtle text |
| Thinking | `#8AB4F8` | Thinking blocks, spinner |
| Success | `#81C995` | OK badges, live |
| Error | `#F28B82` | Errors, offline |
| Warning | `#FDD663` | Warnings, slow status |
| CodeBG | `#2D2D2D` | Thinking block bg |

Both support dark/light themes. OpenCode uses theme tokens with dynamic agent coloring. M31A uses fixed hex values with brand coral.

## Spacing & Layout Mechanics

| Aspect | OpenCode | M31A |
|---|---|---|
| **Layout model** | Flexbox with `flexGrow`, `flexShrink`, `gap`, `padding` | `lipgloss.JoinVertical`/`JoinHorizontal` with fixed dimensions |
| **Padding** | Explicit `paddingLeft`, `paddingTop`, `gap` values | Lipgloss padding via `.Padding(0, 1)` etc. |
| **Borders** | Border props on `<box>` elements (`border={["left"]}`) | Lipgloss `.Border()`, `.BorderStyle()` |
| **Scrolling** | `<scrollbox>` with `stickyScroll`, `stickyStart` | `bubbles/viewport` with manual scroll position |
| **Z-index** | Explicit `zIndex` prop (3000 for dialogs) | Screen switching (no layering) |
| **Positioning** | `relative` / `absolute` positioning | Always flow layout |
| **Minimum size** | No hard minimum — adapts to any size | 40x10 minimum |

## TUI Visibility Issues Identified

### Critical

1. **No sidebar / file context**: M31A has no file tree or modified files view. Users cannot see what files changed during a session without leaving the TUI.

2. **No command palette**: OpenCode's `ctrl+p` command palette provides discoverable access to 170+ commands. M31A users must remember slash commands or key bindings.

3. **No model/agent cycling**: OpenCode can cycle models (`f2`) and agents (`tab`) without leaving the prompt. M31A requires navigating to `/models` screen.

4. **No session management in-TUI**: OpenCode has a full session switcher dialog with search, pin, delete, rename, and preview. M31A has a basic `/resume` screen.

5. **Tool rendering lacks granularity**: M31A's single ToolCard pattern cannot show diffs (Edit/Patch), split output, or per-tool specialized rendering. OpenCode has 14 specialized tool components.

### Moderate

6. **No thinking per-block toggle**: M31A's `t`/`T` toggles ALL thinking blocks globally. OpenCode allows per-block expand/collapse.

7. **No input metadata**: M31A's prompt shows no model, provider, agent, or variant info inline — only in the header where it can be truncated.

8. **No usage display**: M31A shows context tokens in header but no cost estimate. OpenCode shows both in the prompt status bar.

9. **No autocomplete**: M31A's textarea has no slash-command or file-path autocomplete dropdown.

10. **No shell mode**: OpenCode's `!` prefix switches to shell-mode prompt with different styling and placeholders.

11. **No which-key system**: OpenCode has a which-key overlay (`ctrl+alt+k`) showing available commands. M31A has no keybinding hints.

12. **No leader key**: OpenCode's `ctrl+x` leader key enables chorded shortcuts. M31A has no leader concept.

13. **No message footer**: OpenCode shows `▣ {mode} · {model} · {duration}` per message. M31A has no per-message metadata.

14. **No error display per message**: OpenCode shows red left-border error boxes on failed messages. M31A has no inline error display.

15. **No revert/timeline markers**: OpenCode shows revert banners with diff summaries. M31A has no undo/redo UI.

### Minor

16. **No file attachment badges**: OpenCode shows MIME-type badges for attached files. M31A has no attachment display.

17. **No compaction indicator**: OpenCode shows a separator when context was compacted. M31A silently compacts.

18. **No timestamps toggle**: OpenCode can show per-message timestamps. M31A has none.

19. **No subagent navigation**: OpenCode shows subagent hints and child session navigation. M31A has no subagent awareness.

20. **No terminal title management**: OpenCode sets terminal window title based on session. M31A does not.

21. **No mouse support**: OpenCode supports mouse click-to-dismiss dialogs, click-to-expand, copy-on-select. M31A is keyboard-only.

22. **No plugin extensibility**: OpenCode's plugin slots allow third-party UI extensions. M31A cannot be extended.
