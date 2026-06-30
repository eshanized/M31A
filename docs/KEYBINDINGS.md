# Keyboard Shortcuts

Default keybindings for M31 Autonomous's TUI.

---

## Core Shortcuts

| Key | Action | Context |
|-----|--------|---------|
| `Enter` | Submit prompt | REPL |
| `Esc` | Cancel / go back | Any |
| `Ctrl+C` | Quit (requires confirmation if active) | Any |
| `q` | Quit when idle | REPL |
| `Tab` | Next autocomplete suggestion | Input with suggestions |
| `Shift+Tab` | Previous autocomplete suggestion | Input with suggestions |
| `Up` | Previous suggestion / scroll up | Suggestions, history |
| `Down` | Next suggestion / scroll down | Suggestions, history |
| `Ctrl+S` | Save session checkpoint | REPL (session active) |

---

## Leader Key Sequences

Default leader key: `Ctrl+X`

Press and release the leader key, then press the chord key within 2 seconds.

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
| `Ctrl+Q` | Open quick actions |

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
| `y` | Allow (this time) |
| `a` | Allow always |
| `n` | Deny |
| `e` | Exit |

---

## Customizing Keybindings

Keybindings can be customized in `~/.m31a/config.toml`:

```toml
[ui]
leader_key = "ctrl+x"
leader_timeout_ms = 2000
```
