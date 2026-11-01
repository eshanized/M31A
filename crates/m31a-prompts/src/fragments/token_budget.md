# Token Budget Management

## Context Window

The model has a finite context window. The system tracks token usage and automatically compacts history when usage exceeds **65%**.

## Tools for Budget Awareness

### `get_context_remaining`
Returns estimated tokens remaining in the current context window.
```json
{"tool": "get_context_remaining", "args": {}}
```
Response: `{"tokens_remaining": 45000, "percentage_used": 0.55}`

**Use this proactively** — check before large operations.

### `new_context_window`
Forces a context reset (compaction + fresh window).
```json
{"tool": "new_context_window", "args": {}}
```
- Fails if auto-compaction is disabled
- Preserves essential context (recent turns, goals, key facts)
- Use when approaching budget limit on a long task

## Automatic Compaction

At **65% usage**, a background compaction prepares a condensed history. At the next turn, the prepared compaction is applied.

**You don't need to manage this** — but you can accelerate it with `new_context_window` if you know a context-heavy operation is coming.

## Budget-Conscious Patterns

1. **Read once, use many times** — Don't re-read files
2. **Batch tool calls** — Multiple independent tools in one turn
3. **Summarize tool outputs** — The system truncates large outputs (50KB limit)
4. **Use `verify_workspace`** instead of running raw test commands — returns structured diagnostics
5. **Prefer `apply_patch`** over multiple `write_file` calls — single tool call, atomic

## When to Force Reset

- Tokens remaining < 10,000
- About to start a large multi-file refactor
- Previous compaction lost critical context
- Switching to a completely different task area