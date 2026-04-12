# Tools Reference

Tools allow M31A's LLM to interact with the outside world — read files, search code, run commands, and fetch web content.

---

## Built-in Tools

### Bash (`internal/tools/execute.go`)
Executes shell commands with timeout and output capture.

- **Timeout configurable** per execution
- **Grace kill period** (`bash_kill_grace_secs`, default 5s)
- **Permission system** checks before destructive operations
- Output collected to string (stdout + stderr)

### Glob (`internal/tools/`)
File pattern matching across the codebase.

- Supports standard glob patterns (`**/*.go`, `src/**/*.ts`)
- If no matches, retries with `*.<ext>` pattern
- Configurable max results (`max_glob_results`, default 1000)

### Grep (`internal/tools/`)
Content search with regular expressions.

- Full regex support
- File type filtering with `include` parameter
- Configurable max results (`max_grep_results`, default 100)

### Read
Read files from the local filesystem.

- Line-offset and limit support for partial reads
- Auto-truncates lines > 2000 chars
- Support for images and PDFs (rendered as attachments)

### Write / Edit
Modify files on disk.

- **Write** — Creates new files or overwrites existing
- **Edit** — Exact string replacement (with conflict detection for multiple matches)
- Both use atomic writes (temp file + rename)
- Auto-backup enabled by default (`auto_backup = true`)
- Max backups per file: 5

### WebFetch
Fetch and convert web content to markdown.

- Configurable max redirects (`webfetch_max_redirects`, default 3)
- Custom User-Agent header
- HTTP → HTTPS auto-upgrade
- Returns content as markdown, text, or HTML

### WebSearch
Real-time web search.

- Supports live crawling mode
- Configurable result count and search depth
- Domain filtering available

---

## External Tool Integrations

### MCP (Model Context Protocol) `internal/tools/mcp_client.go`
Connects to external MCP servers over stdio transport using JSON-RPC.

- Tool discovery via MCP's `tools/list`
- Tool execution via `tools/call`
- Configured in `[tools]` section of config

### Toolhouse `internal/tools/toolhouse.go`
Cloud-based tool execution platform integration.

- Execute cloud-hosted tools
- Offloads compute to Toolhouse infrastructure

### Brave Search `internal/tools/brave.go`
Web search via Brave Search API.

- Requires Brave Search API key in config
- Returns structured search results
- Used as alternative search backend

---

## Tool Registry

All tools are registered in `internal/tools/registry.go`. Tools can be:

- **Listed** — `/tools` shows all registered tools with status
- **Toggled** — `/tools <name>` enables/disables a tool
- **Executed** — LLM selects tools automatically based on the task

---

## Permission System

Each tool execution goes through a permission check:

1. **Pattern matching** — Checks tool arguments against `[permissions.rules]`
2. **Risk assessment** — Classifies as low/medium/high/destructive
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
