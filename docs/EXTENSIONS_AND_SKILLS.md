# Extensions, Skills & Model Context Protocol (MCP)

**M31A** provides an extensible architecture allowing developers to teach agents repository-specific workflows, inject specialized domain instructions, and connect external tools via the **Model Context Protocol (MCP)**.

---

## 1. The Skills Subsystem

Skills are specialized packages of instructions, scripts, and context files that extend M31A's domain knowledge. They are dynamically discovered, parsed, and injected into the model prompt when relevant.

### Discovery Locations

M31A searches for skills in the following directories (in priority order):

1. **Workspace Skills**: `<workspace-root>/.m31a/skills/`
2. **Legacy Workspace Skills**: `<workspace-root>/.codex/skills/`
3. **User Global Skills**: `~/.m31a/skills/`
4. **Legacy Global Skills**: `~/.codex/skills/`

---

## 2. Anatomy of a Skill

Each skill resides in its own directory containing a mandatory `SKILL.md` file:

```
my-database-skill/
├── SKILL.md            # Mandatory: YAML frontmatter & markdown guidance
├── scripts/            # Optional: Shell/Python scripts the agent can execute
├── references/         # Optional: Additional API references or documentation
└── examples/           # Optional: Code patterns and reference implementations
```

### `SKILL.md` Format

The `SKILL.md` file begins with a YAML frontmatter block defining the skill's name and activation criteria, followed by structured instructions:

```markdown
---
name: database-migrations
description: Guidelines for generating and testing PostgreSQL migrations using Diesel or SQLx
---

# Database Migration Guidelines

When adding or altering database tables in this repository:

1. Always place new migration files in `migrations/YYYYMMDDHHMMSS_name/`.
2. Ensure both `up.sql` and `down.sql` are implemented and symmetric.
3. Verify migrations against the local test database:
   ```bash
   sqlx migrate run --database-url "$TEST_DATABASE_URL"
   ```
4. Never modify existing migration files that have already been applied to production.
```

### Dynamic Activation & Tool Injection

When M31A loads skills:
- The YAML frontmatter (`name`, `description`) is indexed into the agent's available capabilities catalog.
- When a user prompt matches a skill's description, the full instruction block is injected into the model's system context.
- Agents can reference scripts in the skill's `scripts/` directory using the `exec_command` tool.

---

## 3. Model Context Protocol (MCP) Integration

M31A natively integrates with the **Model Context Protocol (MCP)**, allowing external MCP servers to register custom tools, resources, and prompts dynamically.

### Configuring MCP Servers

Add external MCP servers to your `~/.m31a/config.toml` or `.m31a/config.toml`:

```toml
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/docs"]

[mcp_servers.sqlite]
command = "uvx"
args = ["mcp-server-sqlite", "--db-path", "analytics.db"]

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_..." }
```

### Managing MCP Servers via CLI

You can inspect, add, and remove MCP servers without editing configuration files manually:

```bash
# List all active MCP servers and registered tools
m31a mcp list

# Add a new MCP server
m31a mcp add postgres uvx mcp-server-postgres --url "postgresql://localhost/mydb"

# Remove an MCP server
m31a mcp remove postgres
```

### Lazy Tool Discovery & Invocation

To prevent bloating the context window with hundreds of tool schemas:
- M31A indexes tool names and high-level descriptions from MCP servers.
- When an agent requires an external tool, it searches the catalog via `tool_search`, resolves the exact JSON schema, and calls the MCP tool over stdio or HTTP.
