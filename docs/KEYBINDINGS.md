# Keyboard Shortcuts

Default keybindings for M31 Autonomous's TUI.

---

## Core Shortcuts

| Key | Action | Context |
|-----|--------|---------|
| `Enter` | Submit prompt | Ready state |
| `Esc` | Cancel/abort/go back | Processing, Streaming, Paused, pickers |
| `Ctrl+C` | Quit | Any state (requires confirmation if active) |
| `q` | Quit when idle | Ready state |
| `Tab` | Next autocomplete suggestion | Input with suggestions |
| `Shift+Tab` | Previous autocomplete suggestion | Input with suggestions |
| `Up` | Previous suggestion / scroll up | Suggestions list, history |
| `Down` | Next suggestion / scroll down | Suggestions list, history |
| `Ctrl+S` | Save session checkpoint | Ready state (session active) |
| `Ctrl+Z` | Toggle zen mode | Ready state |

---

## Leader Key Sequences

Default leader key: `Ctrl+X`

Press and release the leader key, then press the chord key within the timeout (default 1 second).

| Sequence | Action |
|----------|--------|
| `Ctrl+X` `c` | Compact mode toggle |
| `Ctrl+X` `t` | Theme toggle (dark/light) |
| `Ctrl+X` `s` | Save session |
| `Ctrl+X` `h` | Show help |
| `Ctrl+X` `q` | Quit |

---

## Navigation

| Key | Action |
|-----|--------|
| `PgUp` | Scroll up one page |
| `PgDown` | Scroll down one page |
| `Home` | Scroll to top |
| `End` | Scroll to bottom |

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

## View Mode (Scrolling through responses)

| Key | Action |
|-----|--------|
| `j` | Scroll down one line |
| `k` | Scroll up one line |
| `g` | Scroll to top |
| `G` | Scroll to bottom |
| `d` | Scroll down half page |
| `u` | Scroll up half page |

---

## Customizing Keybindings

Keybindings can be customized in `~/.m31a/config.toml`:

```toml
[ui]
leader_key = "ctrl+x"
leader_timeout_ms = 1000
zen_mode_key = "ctrl+z"
```

Notable: The leader key timeout is configurable for users who prefer slower or faster chord sequences.
