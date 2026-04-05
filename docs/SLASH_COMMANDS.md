# Slash Commands

M31A provides slash commands for runtime actions. Commands are parsed at input and executed before the LLM prompt.

---

## Command List

| Command | Description | Usage |
|---------|-------------|-------|
| `/session` | Session management | `/session list`, `/session create <name>`, `/session delete <id>`, `/session switch <id>`, `/session save`, `/session checkpoint` |
| `/model` | List and select models | `/model`, `/model <id>` to select |
| `/tools` | List and toggle tools | `/tools`, `/tools <name>` to toggle |
| `/config` | View/alter config | `/config`, `/config <key> <value>` |
| `/history` | View session history | `/history`, `/history <n>` for last N messages |
| `/export` | Export session | `/export`, `/export <format>` (json/md) |
| `/agent` | Agent configuration | `/agent list`, `/agent <name>` to select |
| `/ghost` | Ghost write files | `/ghost` enters ghost picker mode |
| `/bisect` | Compare model responses | `/bisect <model1> <model2> <prompt>` |
| `/tui` | Toggle TUI mode | `/tui on`, `/tui off` |
| `/help` | Show help | `/help` |
| `/keychain` | API key management | `/keychain set`, `/keychain get`, `/keychain delete` |
| `/dream` | Toggle prompt enhancement | `/dream on`, `/dream off` |
| `/flush` | Clear screen and reset | `/flush` |
| `/quit` | Quit application | `/quit` |
| `/exit` | Alias for quit | `/exit` |
| `//` | Literal slash passthrough | `//command` sends `/command` as prompt |
| `!` | Bash command passthrough | `!ls -la` runs `ls -la` |

---

## Command Details

### `/session`
Session management commands. Sessions are stored as JSON in `~/.m31a/sessions/`.

- `/session list` — List all sessions with timestamps
- `/session create <name>` — Create a new session
- `/session delete <id>` — Delete a session
- `/session switch <id>` — Switch active session
- `/session save` — Save current session
- `/session checkpoint` — Create a checkpoint snapshot

### `/model`
Model selection and browsing.

- `/model` — List all available models with capabilities
- `/model <id>` — Select a specific model by ID (supports fuzzy matching)

### `/tools`
Tool management.

- `/tools` — List all registered tools and their enabled/disabled status
- `/tools <name>` — Toggle a tool on/off

### `/config`
Runtime configuration inspection and modification.

- `/config` — Display current configuration
- `/config <key> <value>` — Set a config value (e.g., `/config model.default gpt-4o`)

### `/history`
View conversation history for the current session.

- `/history` — Show full history
- `/history <n>` — Show last N messages

### `/export`
Export session data.

- `/export` — Export current session in default format
- `/export json` — Export as JSON
- `/export md` — Export as Markdown

### `/agent`
Agent profile management. Agents define model + system prompt + tool configurations.

- `/agent list` — List available agent profiles
- `/agent <name>` — Switch to an agent profile

### `/ghost`
Ghost write — generates files from prompts using the LLM. Activates ghost picker mode where you select files to write.

### `/bisect`
Compare responses from different models side by side.

- `/bisect <model1> <model2> <prompt>` — Sends the same prompt to two models and shows a diff

### `/keychain`
Manage API keys in the OS keychain.

- `/keychain set` — Prompt to set an API key
- `/keychain get` — Show current key status
- `/keychain delete` — Remove a stored key

### `/dream`
Toggle DREAM prompt enhancement mode (auto-dream).

- `/dream on` — Enable prompt enhancement
- `/dream off` — Disable prompt enhancement

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
