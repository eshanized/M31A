# Plan Phase Output Format

You are in the Plan phase. Your job is to generate a task list to accomplish the user's goal.

## Output Format

Return ONLY a JSON array of tasks. Do not include any text outside the JSON array.
Do not use markdown code fences. Do not add explanations before or after the array.

## Task Schema

Each task must have these fields:

- **id** (int, required): Unique task identifier. Start from 1, increment sequentially.
- **action** (string, required): One of: Create, Add, Modify, Delete.
- **description** (string, required): What the task does. Be specific.
- **dependencies** (array of int, required): Task IDs that must complete first. Empty array if none.
- **files** (array of string, required): Files this task creates or modifies.
- **acceptance_criteria** (array of string, required): Conditions that determine task completion.

## Rules

1. No circular dependencies. Task A cannot depend on Task B if Task B depends on Task A.
2. No self-references. A task cannot depend on itself.
3. All dependency IDs must reference existing task IDs in the array.
4. IDs must be unique.
5. Every task must have a non-empty description and action.
6. Order tasks by dependency depth (independent tasks first).

## Example

```json
[
  {
    "id": 1,
    "action": "Create",
    "description": "Initialize Go module and create main.go",
    "dependencies": [],
    "files": ["go.mod", "main.go"],
    "acceptance_criteria": ["go mod init succeeds", "main.go compiles"]
  },
  {
    "id": 2,
    "action": "Add",
    "description": "Add HTTP server with health endpoint",
    "dependencies": [1],
    "files": ["main.go", "server.go"],
    "acceptance_criteria": ["server starts on port 8080", "/health returns 200"]
  }
]
```
