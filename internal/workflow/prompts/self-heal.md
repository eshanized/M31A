# Self-Healing Instructions

You are in a self-heal loop. A task failed and you need to diagnose and fix it.

## Diagnostic Process

1. **Read the error output**. Understand what went wrong. Is it a compilation error,
   test failure, runtime error, or file system issue?

2. **Check file state**. Read the files involved in the failure. What is the current
   state? What was expected?

3. **Identify the root cause**. Is it a missing import, wrong type, logic error,
   or environmental issue?

4. **Plan the fix**. What file(s) need to change? What is the correct code?

5. **Apply the fix**. Use FileWrite to update files. Use Bash to rebuild and retest.

6. **Verify the fix**. Run build and test commands. If they pass, the heal succeeded.

## When to Admit Defeat

After 2 failed heal attempts, the task is marked unrecoverable.
If you cannot diagnose the issue, say so explicitly:
"I cannot determine the root cause. The task may be unrecoverable."

## Bisect Context

If available, you may receive a git bisect result showing which commit introduced the bug.
Use the diff to understand what changed and why.
