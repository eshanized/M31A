# Keyboard Shortcuts

Default keybindings for M31A's TUI. All keybindings are shown dynamically
in the help screen (? key) and are sourced from the keybinding registry.

---

## Core Shortcuts

| Key | Action | Context |
|-----|--------|---------|
| `Enter` | Submit prompt | REPL |
| `Esc` | Cancel / go back | Any |
| `Ctrl+C` | Quit (requires confirmation if active) | Any |
| `?` | Toggle help screen | Any |
| `Ctrl+P` | Open command palette | Any |
| `Tab` | Next autocomplete suggestion | Input with suggestions |
| `Shift+Tab` | Previous autocomplete suggestion | Input with suggestions |
| `Up` | Previous suggestion / scroll up | Suggestions, history |
| `Down` | Next suggestion / scroll down | Suggestions, history |

---

## REPL Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Ctrl+J` | Insert newline (multi-line) |
| `Shift+Enter` | Insert newline (multi-line) |
| `Up` / `Down` | Navigate command history |
| `Tab` | Complete slash/mention suggestion |
| `/` | Slash command autocomplete |
| `@` | File mention autocomplete |
| `Ctrl+L` / `End` | Scroll to bottom |
| `Ctrl+U` / `PgUp` | Scroll page up |
| `Ctrl+D` / `PgDn` | Scroll page down |
| `j` / `k` | Scroll line down/up (when input empty) |
| `Ctrl+Y` | Copy last assistant message |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+X` | Leader key prefix |

---

## Leader Key Sequences

Default leader key: `Ctrl+X`

Press and release the leader key, then press the chord key within 1 second.

### Global (any screen)

| Sequence | Action |
|----------|--------|
| `Ctrl+X` `s` | Open settings |
| `Ctrl+X` `h` | Open help |
| `Ctrl+X` `l` | Open ledger |
| `Ctrl+X` `k` | Open rollback |
| `Ctrl+X` `d` | Open dashboard |
| `Ctrl+X` `!` | Open notifications |
| `Ctrl+X` `f` | Open file explorer |
| `Ctrl+X` `a` | Toggle subagents panel |
| `Ctrl+X` `c` | Open config viewer |
| `Ctrl+X` `i` | Open session detail |
| `Ctrl+X` `o` | Open tool output |
| `Ctrl+X` `g` | Open home screen |

### REPL Only

| Sequence | Action |
|----------|--------|
| `Ctrl+X` `b` | Toggle sidebar |
| `Ctrl+X` `n` | New session |
| `Ctrl+X` `r` | Session list |
| `Ctrl+X` `m` | Select model |
| `Ctrl+X` `[` | Widen sidebar |
| `Ctrl+X` `]` | Narrow sidebar |

---

## Command Palette

| Key | Action |
|-----|--------|
| `Ctrl+P` | Open/close command palette |
| `Up` / `Down` | Navigate commands |
| `Enter` | Execute selected command |
| `Esc` | Close palette |
| Type to filter | Fuzzy search commands |

---

## Text Input

| Key | Action |
|-----|--------|
| `Left` / `Right` | Move cursor |
| `Backspace` | Delete character before cursor |
| `Ctrl+W` | Delete word before cursor |
| `Ctrl+U` | Delete line before cursor |
| `Ctrl+A` | Move to beginning of line |
| `Ctrl+E` | Move to end of line |
| `Ctrl+K` | Delete from cursor to end of line |

---

## View Mode (scrolling through responses)

| Key | Action |
|-----|--------|
| `j` | Scroll down one line |
| `k` | Scroll up one line |
| `g` | Scroll to top |
| `G` | Scroll to bottom |
| `d` | Scroll down half page |
| `u` | Scroll up half page |
| `PgUp` | Scroll up one page |
| `PgDown` | Scroll down one page |
| `Home` | Scroll to top |
| `End` | Scroll to bottom |

---

## Permission Modal

| Key | Action |
|-----|--------|
| `y` | Allow once |
| `a` | Allow for session |
| `b` | Approve all |
| `n` | Deny |
| `Enter` | Deny (safe default) |
| `Esc` | Deny (safe default) |

---

## Execute Screen

| Key | Action | Context |
|-----|--------|---------|
| `s` | Skip current task | Paused |
| `c` | Cancel current task | Paused |
| `x` | Cancel entire group | Paused |

---

## Empty State Navigation

| Key | Action |
|-----|--------|
| `j` / `Down` | Focus next action |
| `k` / `Up` | Focus previous action |
| `Enter` | Activate focused action |
| `Esc` | Close empty state |

---

## Slash Commands

| Command | Description |
|---------|-------------|
| `/new` | Start new workflow |
| `/goal` | Set session goal |
| `/plan` | Start plan phase |
| `/execute` | Start execute phase |
| `/verify` | Start verify phase |
| `/ship` | Start ship phase |
| `/pause` | Pause workflow |
| `/resume-task` | Resume workflow |
| `/metrics` | Session analytics |
| `/chat` | Start new chat session |
| `/sessions` | List recent sessions |
| `/resume` | Open session browser |
| `/save` | Save session |
| `/clear` | Clear conversation |
| `/search` | Search messages |
| `/flush` | Clear screen, reset view |
| `/history` | Chat history browser |
| `/status` | Show session info |
| `/model` | Show or switch model |
| `/provider` | Show or switch provider |
| `/fallback` | Provider fallback status |
| `/optimize` | Suggest cheaper model |
| `/compress` | Compress context |
| `/memory` | Manage context memory |
| `/tokens` | Estimate token count |
| `/cost` | Toggle cost display |
| `/diff` | Show git diff |
| `/rollback` | Browse commits |
| `/bisect` | Git bisect |
| `/settings` | Open settings editor |
| `/config` | Show or set config |
| `/health` | System health |
| `/tools` | List available tools |
| `/log` | Recent log entries |
| `/key` | API key status |
| `/keychain` | API key status (alias) |
| `/dream` | Context memory (alias) |
| `/ledger` | Learning ledger |
| `/about` | Version & system info |
| `/reset` | Reset to first-run |
| `/quit` | Exit application |
| `/exit` | Exit (alias) |
| `/getting-started` | Show getting-started guide |
| `/quick` | Toggle quick mode for simple tasks |
| `/skip` | Skip to a workflow phase |
| `/refine` | Refine the current plan with feedback |
| `/pending` | Show pending permission queue and batch approvals |
| `/agent` | Spawn a parallel subagent (or list active) |
| `/agent-cancel` | Cancel a running subagent |
| `/complexity` | Show codebase complexity report |
| `/decisions` | Show decision log for current session |
| `/agent-mode` | Toggle autonomous agent mode |

---

## Customizing Keybindings

Keybindings can be customized in `~/.m31a/config.toml`:

```toml
[ui]
leader_key = "ctrl+x"
leader_timeout_ms = 1000
```

The leader timeout controls how long (in milliseconds) you have to press
the chord key after pressing the leader key. Default is 1000ms (1 second).
