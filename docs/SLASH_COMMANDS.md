# Slash Commands

M31A provides slash commands for runtime actions. Commands are parsed from input and executed before the LLM prompt.

---

## Command List

### Core

| Command | Description | Usage |
|---------|-------------|-------|
| `/help` | Show keybinding help | `/help` |
| `/chat` | Start a new chat session (clear messages) | `/chat` |
| `/clear` | Clear conversation messages | `/clear` (with confirmation) |
| `/flush` | Clear screen and reset view | `/flush` |
| `/search` | Search conversation messages | `/search <query>` |
| `/status` | Show session info | `/status` |
| `/about` | Show version and system info | `/about` |
| `/history` | Open chat history browser | `/history` |
| `/prompt-history` | Show recent prompt history | `/prompt-history` |
| `/health` | Show system health status | `/health` |
| `/tools` | List available tools | `/tools` |
| `/copy-error` | Copy last error to clipboard | `/copy-error` |
| `/undo` | Show latest checkpoint info | `/undo` |
| `/reset` | Reset to first-run screen | `/reset` (with confirmation) |
| `/quit` | Exit the application | `/quit` |
| `/exit` | Exit the application (alias) | `/exit` |

### Session

| Command | Description | Usage |
|---------|-------------|-------|
| `/sessions` | List recent sessions | `/sessions` |
| `/resume` | Open session browser | `/resume` |
| `/save` | Save current session | `/save` |
| `/export` | Export session to file | `/export [markdown\|json] [path]` |
| `/goal` | Set or show session goal | `/goal [text]` |
| `/ledger` | Show learning ledger | `/ledger` |

### Workflow

| Command | Description | Usage |
|---------|-------------|-------|
| `/new` | Start a new workflow | `/new` |
| `/workflow` | Show workflow status | `/workflow` |
| `/plan` | Start plan phase | `/plan` |
| `/refine` | Refine the current plan | `/refine` |
| `/execute` | Start execute phase | `/execute` |
| `/verify` | Start verify phase | `/verify` |
| `/ship` | Start ship phase | `/ship` |
| `/phase` | Show or transition phase | `/phase [discuss\|plan\|execute\|verify\|ship]` |
| `/pause` | Pause workflow | `/pause` |
| `/resume-task` | Resume paused workflow | `/resume-task` |
| `/agent-mode` | Toggle autonomous agent mode | `/agent-mode [on\|off]` |
| `/metrics` | Open session analytics | `/metrics` |
| `/dashboard` | Open workflow dashboard | `/dashboard` |

### AI & Model

| Command | Description | Usage |
|---------|-------------|-------|
| `/model` | Show or switch model | `/model` |
| `/models` | List all cached models | `/models` |
| `/provider` | Show or switch provider | `/provider [name]` |
| `/fallback` | Show fallback status | `/fallback [provider]` |
| `/optimize` | Suggest cheaper model alternatives | `/optimize` |
| `/compress` | Trigger context consolidation | `/compress` |
| `/memory` | Manage context memory | `/memory [view\|pause\|resume\|revert]` |
| `/dream` | Alias for `/memory` | `/dream [view\|pause\|resume\|revert]` |
| `/tokens` | Estimate token count | `/tokens` |
| `/cost` | Toggle cost display | `/cost` |

### Git

| Command | Description | Usage |
|---------|-------------|-------|
| `/diff` | Show git diff | `/diff [ref] [ref]` |
| `/rollback` | Browse or reset to commit | `/rollback [hash]` |
| `/bisect` | Git bisect info | `/bisect [good] [bad]` |

### Settings & System

| Command | Description | Usage |
|---------|-------------|-------|
| `/settings` | Open settings editor | `/settings` |
| `/config` | Open full config editor | `/config` |
| `/theme` | Switch theme | `/theme [dark\|light\|auto\|palette]` |
| `/themes` | Open theme picker | `/themes` |
| `/key` | Show API key status | `/key` |
| `/keychain` | Alias for `/key` | `/keychain` |
| `/log` | Show recent log entries | `/log [n]` |
| `/files` | Open file explorer | `/files` |
| `/ghost` | Ghost write files | `/ghost` |

### Subagents

| Command | Description | Usage |
|---------|-------------|-------|
| `/agent` | Spawn or list subagents | `/agent [desc]: [prompt]` |
| `/agent-cancel` | Cancel a running subagent | `/agent-cancel <id\|all>` |

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

Press `ctrl+p` to open the command palette. All registered slash commands appear in the palette with fuzzy search.

## Quick Actions

Press `ctrl+q` to open the quick actions overlay for fast access to common commands.

## Leader Keys

Press `ctrl+x` followed by a key for quick navigation:

| Shortcut | Action |
|----------|--------|
| `ctrl+x h` | Help screen |
| `ctrl+x s` | Settings |
| `ctrl+x b` | Toggle sidebar |
| `ctrl+x p` | Theme picker |
| `ctrl+x m` | Model selector |
| `ctrl+x r` | Resume session |
| `ctrl+x n` | New workflow |
| `ctrl+x l` | Learning ledger |
| `ctrl+x k` | Rollback browser |
| `ctrl+x t` | Toggle theme |
| `ctrl+x d` | Dashboard |
| `ctrl+x f` | File explorer |
| `ctrl+x !` | Notifications |
