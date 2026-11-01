# Multi-Agent Collaboration

## Agent Roles

| Role | Specialization | Best For |
|------|---------------|----------|
| `architect` | System design, API contracts, data models | Planning, high-level design |
| `coder` | Implementation, bug fixes, refactoring | Writing code, tests |
| `reviewer` | Code review, security audit, quality | Reviewing changes, finding issues |
| `explorer` | Codebase analysis, dependency mapping | Understanding unfamiliar code |
| `custom` | User-defined via system prompt | Specialized tasks |

## Spawning Agents

```json
{
  "tool": "spawn_agent",
  "args": {
    "task_name": "Implement user authentication",
    "prompt": "Create JWT-based auth with refresh tokens...",
    "role": "coder",
    "model": "nvidia/nemotron-3.5-lightning-30b-a3b"
  }
}
```

- **Depth limit**: Maximum 1 level of subagents (configurable)
- **Returns**: `agent_id` (e.g., `agent_coder_a1b2c3d4`) and nickname

## Communicating with Agents

### `send_input` — Steer a running agent
```json
{"tool": "send_input", "args": {"agent_id": "agent_coder_a1b2c3d4", "message": "Use bcrypt not argon2", "interrupt": false}}
```
- `interrupt: true` — Stops current execution immediately

### `wait_agent` — Block until completion
```json
{"tool": "wait_agent", "args": {"agent_ids": ["agent_coder_a1b2c3d4"], "timeout_ms": 300000}}
```
- Default timeout: 30 seconds
- Max timeout: 1 hour
- Returns each agent's status and output

### `close_agent` — Terminate cleanly
```json
{"tool": "close_agent", "args": {"agent_id": "agent_coder_a1b2c3d4"}}
```

### `terminate_agent_tree` — Cascade terminate
```json
{"tool": "terminate_agent_tree", "args": {"agent_id": "agent_architect_x9y8z7"}}
```
Terminates the agent and ALL its descendants.

## Message Board (Inter-Agent Communication)

For persistent, structured communication across agents:

```json
// Create channel
{"tool": "create_channel", "args": {"name": "auth-design", "topic": "JWT token format discussion"}}

// Post message
{"tool": "post_message", "args": {"channel": "auth-design", "message": "Proposing RS256 with 15min expiry", "author": "architect"}}

// Read thread
{"tool": "read_thread", "args": {"thread_id": "post_abc123"}}
```

- Channels persist across sessions
- Secrets/keys auto-sanitized in posts
- Subscriptions: `subscribe_channel` for notifications

## Worktree Isolation

Each agent can get its own git worktree:
```json
{"tool": "create_worktree", "args": {"base_ref": "main"}}
```
- Isolates experimental changes
- `list_worktrees`, `remove_worktree` for management

## Best Practices

1. **Spawn with clear, scoped tasks** — "Fix bug X" not "Work on the codebase"
2. **Use roles appropriately** — Don't make a coder do architecture
3. **Wait with timeouts** — Prevents hanging on stuck agents
4. **Use message board for coordination** — Not `send_input` for everything
5. **Clean up** — `close_agent` or `terminate_agent_tree` when done
6. **Monitor depth** — Respect the 1-level limit