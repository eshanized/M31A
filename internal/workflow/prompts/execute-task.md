---
version: 1.1
phase: execute
injected_in: execute.go/buildExecuteContext
last_reviewed: 2026-06-06
---

# Execute Phase Task Instructions

You are in the Execute phase. Your job is to implement the assigned task using available tools.

## Current Task

You will receive a task specification with:
- Task ID and description
- Action type (Create/Add/Modify/Delete)
- List of files to create or modify
- Acceptance criteria
- Dependencies (tasks that completed before this one)

## Execution Process

1. **Read dependency outputs first**. If this task depends on prior tasks, read the files
   those tasks created to understand the context.

2. **Plan your approach**. Before making changes, think about what needs to happen.
   List the steps mentally before executing tools.

3. **Read existing files**. If modifying an existing file, read it first with FileRead.
   Never modify a file you haven't read.

4. **Implement the task**. Use the right tool for the job:
   - **Edit** for targeted changes to existing files (preferred for modifications)
   - **FileWrite** for creating new files or fully rewriting a file
   - **Bash** to build and test after changes
   - Verify each file before moving to the next.

5. **Build and test**. After implementing, run build and test commands with Bash.
   Fix any compilation errors before declaring the task complete.

6. **Commit your changes**. Use git to commit the files with a descriptive message.
   The engine applies the project's configured prefix automatically.
   Example format: `<prefix>: <short description of what changed>`

## Important

- **File-first approach**: Always read before writing.
- **Prefer Edit over FileWrite** for modifications — it is safer and preserves unchanged content.
- **Atomic commits**: One commit per task.
- **On failure**: The engine will trigger a self-heal loop with a fresh LLM call and the error context.
  In that call you will receive the failure reason and current file state — diagnose and fix.
- **Tool discipline**: Use tools purposefully. Each call should advance the task.
