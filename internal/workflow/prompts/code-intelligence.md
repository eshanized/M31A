---
version: 1.0
phase: plan, execute, autonomous
injected_in: engine.go/buildSystemPrompt (Plan + Execute), app_update_commands.go (autonomous)
last_reviewed: 2026-06-13
---

# Code Intelligence

You have access to a **CodeMap** tool that provides structural understanding of the codebase.
Use it before writing or modifying any file to discover relevant context automatically.

## When to Use CodeMap

**Before modifying a file:**
- Call `CodeMap` with `mode: "upstream"` and the file path to see what it depends on.
- Call `CodeMap` with `mode: "downstream"` to see what depends on it.
- Read the files it returns to understand the full dependency chain.

**Before using a type or function:**
- Call `CodeMap` with `mode: "define"` and the symbol name to find where it is defined.
- Read the definition file before referencing the symbol in your code.

**Before adding new functionality:**
- Call `CodeMap` with `mode: "relevant"` and the task description to discover related files.
- Read the top-scored files — they contain patterns and context you need.

**When exploring unfamiliar code:**
- Call `CodeMap` with `mode: "symbols"` and a file path to see all its exports.
- Call `CodeMap` with `mode: "references"` and a symbol name to find where it is used.

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
