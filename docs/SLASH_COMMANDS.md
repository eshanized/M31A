# Slash Command Reference

M31A provides 16 slash commands for session management, provider control, workflow operations, and configuration.

## Session Commands

### `/help`

List all available commands with descriptions.

**Arguments:** None

**Example:**
```
/help
```

### `/clear`

Clear the current conversation context. Messages remain in session history.

**Arguments:** None

**Example:**
```
/clear
```

### `/status`

Show current session information including session ID, provider, model, phase, and message count.

**Arguments:** None

**Example:**
```
/status
```

### `/sessions`

List all recent sessions with their provider, model, and message count. Corrupted sessions are marked with `[!CORRUPT]`.

**Arguments:** None

**Example:**
```
/sessions
```

### `/undo`

Show details of the latest checkpoint including phase, timestamp, message count, and task count.

**Arguments:** None

**Example:**
```
/undo
```

### `/quit`

Exit the application. On exit, session state is saved.

**Arguments:** None

**Example:**
```
/quit
```

### `/reset`

Return to the first-run setup screen.

**Arguments:** None

**Example:**
```
/reset
```

## Provider Commands

### `/provider [name]`

Show the active provider or switch to a different one.

**Arguments:**
- No args: Show current provider and available alternatives
- `name`: Switch to the specified provider

**Example:**
```
/provider
/provider zen
```

### `/fallback [name]`

Show fallback provider status or manually switch providers.

**Arguments:**
- No args: Show active provider and available fallbacks
- `name`: Switch to the specified provider

**Example:**
```
/fallback
/fallback openrouter
```

### `/model [name|--selector]`

View model information or open the interactive model selector.

**Arguments:**
- No args: Show current default model
- `name`: Look up and display model details
- `--selector`: Open the fuzzy-searchable model selector screen

**Example:**
```
/model
/model anthropic/claude-sonnet-4-20250514
/model --selector
```

### `/models`

List all cached models from the active provider with context length and pricing.

**Arguments:** None

**Example:**
```
/models
```

## Workflow Commands

### `/phase [name]`

Show the current workflow phase or transition to a new phase.

**Arguments:**
- No args: Show current phase
- `name`: Transition to the specified phase

**Valid phases:** `idle`, `initialize`, `discuss`, `plan`, `execute`, `verify`, `ship`

**Example:**
```
/phase
/phase plan
```

### `/goal [text]`

Show or set the session goal.

**Arguments:**
- No args: Show current goal
- `text`: Set the session goal

**Example:**
```
/goal
/goal Add user authentication to the API
```

## Utility Commands

### `/ledger [stats|<type>]`

Show recent session entries from the cross-session learning ledger.

**Subcommands:**
- No args: Show last 5 session entries
- `stats`: Show aggregate statistics (total sessions, avg tasks, avg cost, avg duration, top frameworks, top failures)
- `<type>`: Filter entries by project type (e.g., `go`, `python`, `nodejs`)

**Example:**
```
/ledger
/ledger stats
/ledger go
```

### `/rollback [<hash>]`

Show the commit chain or reset to a specific commit.

**Subcommands:**
- No args: Show last 10 commits with HEAD marker
- `<hash>`: Soft reset to the specified commit (preserves uncommitted changes)
- `--hard <hash>`: Hard reset to the specified commit (discards uncommitted changes)

**Example:**
```
/rollback
/rollback abc1234
/rollback --hard abc1234
```

### `/compress`

Trigger AutoDream context consolidation. Compresses conversation history when it exceeds the context threshold.

**Arguments:** None

**Example:**
```
/compress
```

### `/config [key value]`

Show all configuration or set a specific config value.

**Arguments:**
- No args: Display full config
- `key value`: Set a config value using dot-notation keys

**Supported keys:**
- `ui.theme` — Theme name (e.g., `dark`, `light`)
- `model.default` — Default model ID
- `ui.compact_mode` — Enable/disable compact mode (boolean)
- `ui.show_token_usage` — Show token usage in header (boolean)
- `provider.auto_fallback` — Enable auto-fallback on errors (boolean)

**Example:**
```
/config
/config ui.theme light
/config provider.auto_fallback true
```
