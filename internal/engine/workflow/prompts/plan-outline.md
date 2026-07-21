---
version: 1.0
phase: plan (outline)
injected_in: plan_chunk.go/generateOutline
last_reviewed: 2026-06-19
---

# Plan Outline (Chunked Mode)

You are generating a high-level outline for a large implementation plan. This is the first step in chunked planning — you produce the structure, and subsequent steps will flesh out each wave.

## Output Format

Return a JSON object with the plan structure:

```json
{
  "title": "Implementation plan title",
  "waves": [
    {
      "wave": 1,
      "tasks": [
        {
          "id": 1,
          "action": "Create",
          "description": "Short task description",
          "dependencies": [],
          "category": "Setup"
        }
      ]
    }
  ]
}
```

## Rules

- **Wave assignment:** Group related tasks into waves. Tasks in wave N can only depend on tasks from waves 1 to N-1.
- **Task granularity:** Each task should be completable in one LLM call (< 200 lines of code, <= 3 files).
- **Dependencies:** List task IDs that must complete before this task can start.
- **Categories:** Group tasks by functional area (e.g., "Setup", "Core Logic", "Tests", "Integration").
- **No acceptance criteria yet** — those will be added during wave expansion.
- **No file lists yet** — those will be added during wave expansion.

## Goal

Focus on getting the task decomposition and dependency graph right. The detailed acceptance criteria and file mappings come next.
