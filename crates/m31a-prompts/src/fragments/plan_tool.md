# Plan Tool Usage

## Purpose

`update_plan` tracks multi-step task progress. It provides structure for complex work and enables the user to see your progress.

## When to Use

- Tasks with 3+ distinct steps
- Long-running operations where progress visibility matters
- When you need to coordinate multiple tools in sequence

## Schema

```json
{
  "plan": [
    {"step": "Descriptive step name", "status": "pending|in_progress|completed"},
    ...
  ],
  "explanation": "Optional context about current state"
}
```

## Rules

1. **Exactly one `in_progress` step** — The validator rejects multiple
2. **Update after milestones** — Don't wait until the end
3. **Keep steps granular** — "Implement feature" is too vague; "Implement user auth module" is better
4. **Explanation is optional but recommended** — Helps user understand context

## Example Flow

```json
// Start
{"plan": [{"step": "Analyze codebase structure", "status": "in_progress"}, {"step": "Design data model", "status": "pending"}, {"step": "Implement API endpoints", "status": "pending"}], "explanation": "Starting exploration"}

// After analysis
{"plan": [{"step": "Analyze codebase structure", "status": "completed"}, {"step": "Design data model", "status": "in_progress"}, {"step": "Implement API endpoints", "status": "pending"}], "explanation": "Found 3 key modules"}

// Continue...
```

## Anti-Patterns to Avoid

- ❌ Never having a plan for complex tasks
- ❌ Multiple `in_progress` steps (will error)
- ❌ Updating plan only at the very end
- ❌ Vague step names like "Do work"