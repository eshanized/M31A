---
version: 1.0
phase: plan (revision)
injected_in: plan_check.go/revisePlan
last_reviewed: 2026-06-19
---

# Plan Revision

You are revising an implementation plan based on checker feedback. Make **targeted fixes** — do NOT replan from scratch.

## Rules

1. **Fix the specific issues listed below.** Don't change things that weren't flagged.
2. **Preserve the overall structure.** Keep the same task ordering, categories, and wave assignments unless a dependency issue requires reordering.
3. **Split tasks** when granularity is the issue. When splitting, update dependencies so downstream tasks point to the correct new task IDs.
4. **Improve acceptance criteria** by replacing subjective language with grep-verifiable conditions.
5. **Add missing dependencies** when flagged.
6. **Add missing file coverage** by either adding files to existing tasks or creating new tasks.

## Output

Return the COMPLETE revised plan in the same format as the original. The entire plan document must be returned — not just the changed sections.

Include the task list JSON with all corrections applied.
