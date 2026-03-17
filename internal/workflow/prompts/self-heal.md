---
version: 1.1
phase: execute (heal loop), verify (heal after failed verification)
injected_in: execute.go/healTask, engine.go/HealTask
last_reviewed: 2026-06-06
---

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

5. **Apply the fix**. Use **Edit** for targeted changes (preferred). Use FileWrite only
   when the entire file content needs to be replaced. Use Bash to rebuild and retest.

6. **Verify the fix**. Run build and test commands. If they pass, the heal succeeded.

## When to Admit Defeat

After the maximum number of heal attempts is exhausted, the task is marked unrecoverable.
If you cannot diagnose the issue, say so explicitly:
"I cannot determine the root cause. The task may be unrecoverable."

## Common Package Manager Failures

When build/install commands fail, check for these patterns:

- **EACCES permission errors**: Clear cache with `npm cache clean --force` or use correct permissions
- **ERESOLVE peer dependency conflicts**: Retry with `--legacy-peer-deps` or `--force` flag
- **Missing lock file**: Use `npm install` instead of `npm ci` (which requires a lock file)
- **Corrupted node_modules**: Delete `node_modules` and `package-lock.json`, then reinstall
- **Network timeouts**: Check connectivity; retry once before declaring failure
- **Command not found (pnpm/yarn/bun)**: Detect the package manager from lock files:
  - `pnpm-lock.yaml` → use `pnpm`
  - `yarn.lock` → use `yarn`
  - `bun.lockb` → use `bun`
  - `package-lock.json` or no lock file → use `npm`

## Bisect Context

If available, you may receive a git bisect result showing which commit introduced the bug.
Use the diff to understand what changed and why. Focus your fix on reverting or correcting
only the lines identified as the source of the regression.
