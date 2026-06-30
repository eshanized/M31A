# Tools Reference

M31 Autonomous provides 18 built-in tools for the LLM to interact with your codebase and the outside world.

---

## Built-in Tools

### Bash

Executes shell commands with timeout and output capture.

- **Risk level:** dangerous
- **Permission:** Always prompts (unless explicitly allowed)
- **Timeout:** 30 minutes (configurable)
- **Grace kill:** 5 seconds (`tools.bash_kill_grace_secs`)
- **Output limit:** 50,000 characters

**Dangerous command blocking:** Commands matching blocklist patterns (e.g., `rm -rf /`, `dd if=/dev/zero`) are denied at the tool boundary.

---

### FileRead

Reads files from the local filesystem.

- **Risk level:** safe
- **Supports:** Byte/line-level offsets, binary detection
- **Image/PDF:** Rendered as attachments

---

### FileWrite

Creates new files or overwrites existing files.

- **Risk level:** medium
- **Atomic writes:** Temp file + rename pattern
- **Backup:** Auto-backup enabled by default (`features.auto_backup = true`)
- **Max backups:** 10 per file (`tools.max_backups_per_file`)

---

### Edit

Modifies files using 7-strategy cascading replacement.

- **Risk level:** medium
- **Strategies:** Exact → line-trimmed → whitespace-normalized → indent-normalized → line-skip fuzzy → fuzzy anchor → Levenshtein similarity
- **Fuzzy threshold:** 0.7 similarity
- **Conflict detection:** Multiple match detection with `replaceAll` option

---

### Glob

File pattern matching across the codebase.

- **Risk level:** safe
- **Max results:** 1000 (`tools.max_glob_results`)
- **Fallback:** Retries with `*.<ext>` pattern if no matches
- **Ripgrep:** Uses ripgrep for performance when available

---

### Grep

Content search with regular expressions.

- **Risk level:** safe
- **Max results:** 100 (`tools.max_grep_results`)
- **Features:** Full regex, file type filtering, gitignore-aware
- **ReDoS protection:** Regex complexity limits

---

### WebFetch

Fetches and converts web content to markdown.

- **Risk level:** safe
- **SSRF protection:** Blocks private, loopback, and link-local IPs
- **DNS pinning:** Resolves DNS once, pins IP (5 min TTL)
- **Redirect limit:** 5 (`tools.webfetch_max_redirects`)
- **Output formats:** markdown, text, HTML

---

### WebSearch

Searches the web using SearXNG (privacy-respecting meta-search engine).

- **Risk level:** safe
- **Max results:** 10
- **SearXNG instance:** `tools.websearch_base_url` (default: `https://search.sagibo.net`)
- **No API key required:** Zero telemetry
- **DNS cache:** Prevents TOCTOU rebinding attacks

---

### CodeMap

Code intelligence interface for understanding code relationships.

- **Risk level:** safe
- **Operations:** upstream, downstream, define, references, relevant, symbols
- **Languages:** Go, TypeScript, Python, Rust

---

### CodeComplexity

Classifies codebase complexity to inform model selection.

- **Risk level:** safe
- **Levels:** Simple (<10K lines), Moderate (10K–50K), Complex (50K+)
- **Languages:** All 4 supported languages

---

### FileDelete

Deletes files with backup.

- **Risk level:** destructive
- **Backup:** Creates backup before deletion
- **Containment:** Validates path is within workspace

---

### FileMove

Moves or renames files.

- **Risk level:** medium
- **Containment:** Validates source and destination within workspace
- **Backup:** Creates backup of destination if exists

---

### FileList

Tree-style directory listing.

- **Risk level:** safe
- **Features:** Expandable directories, file type indicators
- **Skip dirs:** Respects `tools.skip_dirs` configuration

---

### TodoWrite

Manages TODO.md with sidebar notification.

- **Risk level:** safe
- **Features:** Add/remove/update TODO items
- **Sidebar:** Updates todo sidebar in real-time

---

### TodoRead

Parses and reads TODO.md.

- **Risk level:** safe
- **Output:** Parsed TODO items with status

---

### DevServer

Manages dev server lifecycle.

- **Risk level:** dangerous
- **Operations:** start, stop, restart, logs, port-check
- **Features:** Crash monitoring, automatic restart
- **Port checking:** Validates port availability before start

---

### HTTPCheck

Makes HTTP requests with status/body/JSON path validation.

- **Risk level:** safe
- **Features:** Status code check, body validation, JSON path queries
- **Use case:** Smoke testing, API validation

---

### AskUserQuestion

Interactive user question with timeout and custom options.

- **Risk level:** safe
- **Features:** Timeout support, custom option buttons
- **Integration:** Permission modal, discuss phase

---

## Tool Execution Pipeline

Every tool call goes through:

1. **Concurrency semaphore** — Max 8 concurrent executions
2. **Rate limiter** — Token bucket (20 burst / 10 sustained)
3. **Risk-level rate limiter** — Dangerous tools: 5 burst / 2 sustained
4. **Permission check** — Rule evaluation → agent default → risk-level fallback
5. **Tool execution**
6. **Output bounding** — 2000 lines / 51200 bytes (configurable)

---

## Subagent System

The LLM can spawn child agents that run in isolated git worktrees:

- **Max concurrent:** 8 subagents
- **Max nesting depth:** 2 levels
- **Per-subagent budgets:** 50 tools, 50K tokens, 25 turns
- **Profiles:** build, plan, general, explore, security, review
- **Custom profiles:** Configurable via `[agents.profiles]`

---

## Tool List

| # | Tool | Risk | Category |
|---|------|------|----------|
| 1 | Bash | dangerous | Execution |
| 2 | FileRead | safe | File |
| 3 | FileWrite | medium | File |
| 4 | Edit | medium | File |
| 5 | Glob | safe | Search |
| 6 | Grep | safe | Search |
| 7 | WebFetch | safe | Web |
| 8 | WebSearch | safe | Web |
| 9 | CodeMap | safe | Code Intelligence |
| 10 | CodeComplexity | safe | Code Intelligence |
| 11 | FileDelete | destructive | File |
| 12 | FileMove | medium | File |
| 13 | FileList | safe | File |
| 14 | TodoWrite | safe | Task |
| 15 | TodoRead | safe | Task |
| 16 | DevServer | dangerous | Runtime |
| 17 | HTTPCheck | safe | Web |
| 18 | AskUserQuestion | safe | Interactive |

---

## Permission System

Each tool execution goes through permission checking:

1. **Pattern matching** — Checks tool arguments against `[permissions.rules]`
2. **Risk assessment** — Classifies as safe/medium/dangerous/destructive
3. **Action** — `allow`, `deny`, or `prompt` (ask user)
4. **Per-agent profiles** — Different agents can have different permission rules

Config example:
```toml
[permissions]
default_mode = "prompt"

[[permissions.rules]]
tool = "Bash"
pattern = "rm -rf"
risk_level = "destructive"
action = "deny"

[permissions.agents.build]
default_action = "allow"
```

---

## Runtime Configuration

| Config Key | Default | Description |
|------------|---------|-------------|
| `tools.max_glob_results` | 1000 | Max glob search results |
| `tools.max_grep_results` | 100 | Max grep search results |
| `tools.bash_kill_grace_secs` | 5 | Grace period before killing bash |
| `tools.max_backups_per_file` | 10 | Max backup copies per file |
| `tools.webfetch_max_redirects` | 5 | Max web fetch redirects |
| `tools.webfetch_user_agent` | "M31A/dev" | User-Agent for web fetches |
| `tools.websearch_base_url` | "https://search.sagibo.net" | SearXNG instance URL |
| `tools.websearch_enabled` | true | Enable/disable web search tool |
| `tools.output_max_lines` | 2000 | Max tool output lines |
| `tools.output_max_bytes` | 51200 | Max tool output bytes |
