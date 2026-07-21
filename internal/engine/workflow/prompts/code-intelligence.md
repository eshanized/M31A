---
version: 1.0
phase: plan, execute, autonomous
injected_in: engine.go/buildSystemPrompt (Plan + Execute), app_update_commands.go (autonomous)
last_reviewed: 2026-06-13
---

# Code Intelligence

Code intelligence context is **automatically injected** into your task context by the engine.
You do NOT need to call CodeMap for the files listed in your task — the relevant files,
dependencies, and type definitions are already provided in your context.

## When to Call CodeMap (Only When Needed)

Only call CodeMap when you need **additional exploration** beyond what's auto-injected:
- You need to find a symbol that isn't in the auto-injected context.
- You need to trace a dependency chain deeper than what's shown.
- You're working on a file not listed in the current task.

**Do NOT call CodeMap for:**
- Files already listed in your task specification.
- Files already shown in the "Recommended Files" or "Dependencies" sections of your context.
- Types or functions already defined in the auto-injected context.

## CodeMap Modes

| Mode | Query | Returns |
|------|-------|---------|
| `upstream` | file path | Files this file imports (dependencies) |
| `downstream` | file path | Files that import this file (dependents) |
| `define` | symbol name | Where the symbol is defined |
| `references` | symbol name | Where the symbol is used |
| `relevant` | task description | Files ranked by relevance to the task |
| `symbols` | file path | All symbols exported/defined in the file |

## Workflow

1. **Receive a task** — note the files to modify and the task description.
2. **Query intelligence** — use CodeMap to find dependencies, definitions, and relevant files.
3. **Read recommended files** — the intelligence layer returns scored results; read the top ones.
4. **Write correct code** — with full context from steps 2-3, your code will have correct types,
   imports, and consistent patterns.

## Important

- **Trust the intelligence layer.** Files it recommends are structurally related to your task.
- **Read before writing.** Even with CodeMap results, you must read the actual file content.
- **Combine with Grep/Glob.** CodeMap provides structural context; use Grep for content search
  and Glob for file discovery when CodeMap results are insufficient.
- **Do not skip CodeMap.** Calling it takes milliseconds and prevents entire categories of errors:
  wrong types, missing imports, broken interfaces, duplicated logic.
