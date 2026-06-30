# Slash Commands

M31 Autonomous provides slash commands for runtime actions. Commands are parsed from input and executed before the LLM prompt.

---

## Command List

### Core

| Command | Description |
|---------|-------------|
| `/help` | List available commands |
| `/clear` | Clear conversation messages (with confirmation) |
| `/status` | Show session info |
| `/reset` | Reset to factory state (deletes config, keys, sessions) |
| `/quit` | Exit the application |
| `/exit` | Exit the application (alias for /quit) |
| `/chat` | Start a new chat session (clear messages) |
| `/flush` | Clear screen and reset view |
| `/search` | Search conversation messages |
| `/about` | Show version and system info |
| `/undo` | Show latest checkpoint info |
| `/history` | Open chat history browser |
| `/prompt-history` | Show recent prompt history |
| `/health` | Show system health status |
| `/tools` | List available tools |
| `/copy-error` | Copy last error to clipboard |

### AI & Model

| Command | Description |
|---------|-------------|
| `/compress` | Trigger context consolidation |
| `/memory` | Manage context memory (view/pause/resume/revert) |
| `/dream` | Alias for `/memory` |
| `/optimize` | Suggest cheaper model alternatives |
| `/model` | Show or switch model |
| `/fallback` | Switch provider / show fallback status |
| `/provider` | Show or switch provider |

### Config & Settings

| Command | Description |
|---------|-------------|
| `/settings` | Open settings editor |
| `/config` | Open full config editor (all sections, editable) |
| `/cost` | Toggle cost display |
| `/log` | Show recent log entries |
| `/key` | Show API key status |
| `/keychain` | Alias for `/key` |
| `/tokens` | Estimate token count |

### Session

| Command | Description |
|---------|-------------|
| `/sessions` | List recent sessions |
| `/export` | Export session to file |
| `/fork` | Fork current session |
| `/prev` | Switch to previous session |
| `/next` | Switch to next session |
| `/save` | Save current session |
| `/goal` | Set or show session goal |
| `/resume` | Open session browser |
| `/ledger` | Show learning ledger |

### Workflow

| Command | Description |
|---------|-------------|
| `/new` | Start a new workflow |
| `/workflow` | Workflow control |
| `/plan` | Alias for `/phase plan` |
| `/refine` | Refine the current plan with feedback |
| `/execute` | Alias for `/phase execute` |
| `/verify` | Alias for `/phase verify` |
| `/runtime` | Alias for `/phase runtime` |
| `/ship` | Alias for `/phase ship` |
| `/phase` | Show or transition phase |
| `/pause` | Pause workflow |
| `/resume-task` | Resume workflow |
| `/pending` | Show pending permission queue and batch approvals |
| `/agent-mode` | Toggle autonomous agent mode |
| `/metrics` | Open session analytics |
| `/dashboard` | Open workflow dashboard |
| `/notifications` | Open notification center |
| `/files` | Open file explorer |
| `/ghost` | Ghost write files |

### Git

| Command | Description |
|---------|-------------|
| `/diff` | Show git diff |
| `/rollback` | Browse or reset to commit |
| `/bisect` | Git bisect info |

### Subagents

| Command | Description |
|---------|-------------|
| `/agent` | Spawn a parallel subagent (or list active) |
| `/agent-cancel` | Cancel a running subagent |

### Analysis

| Command | Description |
|---------|-------------|
| `/complexity` | Show codebase complexity report |
| `/decisions` | Show decision log for current session |

---

## Special Syntax

### Literal Slash (`//`)

Prefix a prompt with `//` to send it literally without slash command parsing:
```
//model should be optimized
```

### Bang Passthrough (`!`)

Prefix a prompt with `!` to execute it as a bash command:
```
!ls -la src/
```

Output is captured and displayed inline.

---

## Command Palette

Press `Ctrl+P` to open the command palette. All registered slash commands appear with fuzzy search and category grouping.

## Leader Keys

Press `Ctrl+X` followed by a key for quick navigation:

| Shortcut | Action |
|----------|--------|
| `Ctrl+X s` | Settings |
| `Ctrl+X h` | Help |
| `Ctrl+X l` | Ledger |
| `Ctrl+X k` | Rollback |
| `Ctrl+X d` | Dashboard |
| `Ctrl+X !` | Notifications |
| `Ctrl+X f` | File explorer |
| `Ctrl+X a` | Subagents panel |
| `Ctrl+X c` | Config viewer |
| `Ctrl+X i` | Session detail |
| `Ctrl+X o` | Tool output |
| `Ctrl+X g` | Home screen |
| `Ctrl+X b` | Toggle sidebar (REPL only) |
| `Ctrl+X n` | New session (REPL only) |
| `Ctrl+X r` | Session list (REPL only) |
| `Ctrl+X m` | Select model (REPL only) |
| `Ctrl+X [` | Widen sidebar (REPL only) |
| `Ctrl+X ]` | Narrow sidebar (REPL only) |

---

## Dynamic Commands

Skills discovered from `~/.m31a/` and project directories are registered as slash commands at startup.
