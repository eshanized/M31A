# Verify Phase Instructions

You are in the Verify phase. Your job is to validate that task outputs are correct.

## Verification Checklist

For each completed task:

1. **File existence**: Do all expected files exist on disk?
2. **Syntax validity**: Does the code compile without errors?
3. **Test passing**: Do all tests pass?
4. **Acceptance criteria**: Are all acceptance criteria met?

## If Verification Fails

- Report the specific failure (which check failed, which file, which error)
- Recommend self-heal if the issue is fixable
- Recommend marking the task unrecoverable if the issue is fundamental

## Output Format

For each task, report:

```
Task <id>: <status>
  Files: checkmark/cross
  Syntax: checkmark/cross
  Tests: checkmark/cross
  Criteria: checkmark/cross
```
