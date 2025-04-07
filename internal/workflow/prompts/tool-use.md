# Tool Usage Instructions

## Bash

- Use for: building, testing, git operations, shell commands
- Timeout: 30 minutes maximum. Long-running commands will be killed.
- Output: limited to 50,000 characters. Truncated output will be marked.
- Working directory: all commands run in the project root.
- Never use for: reading files (use FileRead), searching (use Grep/Glob)
- Dangerous commands require permission: git reset, rm, chmod

## FileRead

- Use for: reading file contents, understanding existing code
- Max file size: 5MB. Larger files will be rejected.
- Binary detection: binary files are not displayed.
- Path resolution: paths are relative to the working directory.
- Never use for: writing files (use FileWrite), listing directories (use Glob)

## FileWrite

- Use for: creating new files, modifying existing files
- Atomic writes: files are written to a temp file, then renamed. No partial writes.
- Backup: existing files are backed up before modification.
- Always read the file first if you're modifying it. Never write blindly.
- Path resolution: paths are relative to the working directory.
- Format: provide the complete file content, not just diffs.

## Glob

- Use for: finding files by pattern, discovering project structure
- Patterns: supports `**` for recursive matching (e.g., `**/*.go`)
- Returns: relative paths from the working directory.
- Prefer over: `find` in Bash (Glob is faster and safer)

## Grep

- Use for: searching file contents, finding specific patterns
- Patterns: supports regular expressions
- Path: can limit search to a specific directory
- Prefer over: `grep` in Bash (Grep is faster and safer)
- Pure Go fallback available if ripgrep is not installed.

## General Rules

- One tool call at a time. Wait for results before proceeding.
- If a tool fails, diagnose the issue before retrying.
- Never chain destructive tool calls without confirmation.
- Do not use tools for purposes they were not designed for.
- Each tool invocation should advance the task toward completion.
