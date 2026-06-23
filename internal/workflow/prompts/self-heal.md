---
version: 1.2
phase: execute (heal loop), verify (heal after failed verification)
injected_in: execute.go/healTask, engine.go/HealTask
last_reviewed: 2026-06-14
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

   **Root cause taxonomy** (check in order):
   - **Wrong type**: a type, field, or method was referenced that doesn't exist
     → Error patterns: "undefined: X", "X is not a type", "cannot use X as Y"
   - **Missing import**: a package was used but not imported
     → Error patterns: "undefined: X" where X is from another package, "use of undeclared name"
   - **Interface mismatch**: a method was defined with the wrong signature
     → Error patterns: "does not implement", "wrong type for X method"
   - **Stale reference**: a renamed/deleted symbol is still referenced
     → Error patterns: "X redeclared", "undefined: X" after rename, "X undeclared"
   - **State dependency**: the code assumes prior state that wasn't established
     → Error patterns: nil pointer dereference, missing map key, index out of range
   - **Environment**: a command/binary is unavailable in the current environment
     → Error patterns: "exec: not found", "command not found", exit code 127
   - **Missing symbol**: a function/type is referenced but not defined in scope
     → Error patterns: "undefined: X", "use of undeclared name X"
   - **Wrong method signature**: correct type, wrong parameter count or types
     → Error patterns: "not enough arguments", "too many arguments", "cannot use X as Y"
   - **Stale import path**: package moved or renamed; import path no longer valid
     → Error patterns: "cannot find package", "no required module provides package"
   - **Circular dependency**: code creates an import cycle between packages
     → Error patterns: "import cycle not allowed"
   - **Platform issue**: OS-specific code running on wrong platform (e.g., Windows paths on Linux)
     → Error patterns: build succeeds but fails at runtime, path separator issues
   - **Version mismatch**: using an API that doesn't exist in the installed package version
     → Error patterns: "X.Y undefined", "not enough arguments in call to"

4. **Plan the fix**. What file(s) need to change? What is the correct code?

5. **Apply the fix**. Use **Edit** for targeted changes (preferred). Use FileWrite only
   when the entire file content needs to be replaced. Use Bash to rebuild and retest.

6. **Verify the fix**. Run build and test commands. If they pass, the heal succeeded.

## Heal Context Rules

This is a **self-heal call**, not the initial execution. You have different information:
- You see the **exact error message** from the failed attempt.
- You see the **current file state** (post-previous-attempt).
- You know the **previous approach failed**.

You MUST:
1. **Read the error message carefully** — it tells you exactly what went wrong.
2. **Check if the error is in the file you just wrote, or in a different file** — compiler errors often cascade.
3. **Try a fundamentally different approach** — don't repeat the same edit. If Edit failed, try FileWrite. If the import was wrong, fix the import. If the type is wrong, change the call site.
4. **Don't add new code to "work around" the error** — fix the actual root cause.

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
