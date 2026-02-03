---
version: 1.1
phase: plan, execute
injected_in: engine.go/buildSystemPrompt (Plan + Execute phases)
last_reviewed: 2026-06-06
---

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
- Never use for: writing files (use FileWrite or Edit), listing directories (use Glob)

## FileWrite

- Use for: creating brand-new files, or fully rewriting a file from scratch
- Atomic writes: files are written to a temp file, then renamed. No partial writes.
- Backup: existing files are backed up before modification.
- **Prefer Edit over FileWrite when modifying an existing file** — FileWrite replaces the entire content.
- Path resolution: paths are relative to the working directory.
- Format: provide the complete file content, not just diffs.

## Edit

- Use for: targeted modifications to existing files — string replacement or line-range replacement
- **Prefer this over FileWrite when changing less than ~80% of a file**
- Two modes:
  - **String mode**: provide `old_string` (exact text to find) and `new_string` (replacement)
  - **Line-range mode**: provide `start_line`, `end_line`, and `new_string` (replaces those lines)
- Cascading match strategies: exact → line-trimmed → whitespace-normalized → fuzzy-anchor
- Always read the file first so you know the exact text to replace.

## Glob

- Use for: finding files by pattern, discovering project structure
- Patterns: supports `**` for recursive matching (e.g., `**/*.go`)
- Returns: relative paths from the working directory.
- Prefer over: `find` in Bash (Glob is faster and safer)

## Grep

- Use for: searching file contents, finding specific patterns
- Patterns: supports regular expressions
- Path: can limit search to a specific directory or file
- Include: can filter by file glob (e.g. `*.go`)
- Max results: configurable via `max_results` parameter (default 100)
- Prefer over: `grep` in Bash (Grep is faster and safer)
- Pure Go fallback available if ripgrep is not installed.

## WebFetch

- Use for: fetching documentation, API references, or external resources by URL
- Returns: text/markdown content of the page (no JavaScript execution)
- Timeout: 30 seconds
- Do not use for: downloading binary files or authenticating to services

## TodoWrite

- Use for: writing a structured TODO list to track task progress in the session
- Call after starting or completing significant steps to keep the list current
- Parameters: `todos` — array of `{content, status, priority}` objects
- Status values: `pending`, `in_progress`, `completed`, `cancelled`
- Priority values: `high`, `medium`, `low`

## AskUserQuestion

- Use for: pausing execution to ask the user a clarifying question
- **NEVER use in automated task execution flows** — only in interactive sessions
- Parameters: `question` (required), `header`, `options`, `allow_custom`, `timeout`
- The tool blocks until the user answers or the timeout expires

## FileList

- Use for: listing files and directories in a path with metadata (size, type)
- Parameters: `path` (directory to list, default ".")
- Returns: file names, sizes, and whether each entry is a directory
- Prefer over: `ls` in Bash for structured directory listings

## FileDelete

- Use for: safely deleting a file with automatic backup before removal
- Parameters: `path` (required) — the file to delete
- Backup: the file is backed up before deletion for recovery
- Never use for: deleting directories (use Bash `rm -r` instead)

## FileMove

- Use for: renaming or moving a file to a new path
- Parameters: `source` (required), `destination` (required)
- Creates parent directories for the destination if they don't exist
- Prefer over: `mv` in Bash for single-file moves

## General Rules

- If a tool fails, diagnose the issue before retrying.
- Never chain destructive tool calls without confirmation.
- Do not use tools for purposes they were not designed for.
- Each tool invocation should advance the task toward completion.
