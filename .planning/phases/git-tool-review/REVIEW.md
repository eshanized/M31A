---
phase: git-tool-review
reviewed: 2026-07-14T12:00:00Z
depth: standard
files_reviewed: 1
files_reviewed_list:
  - internal/tools/git.go
findings:
  critical: 1
  warning: 5
  info: 3
  total: 9
status: issues_found
---

# Phase git-tool-review: Code Review Report

**Reviewed:** 2026-07-14T12:00:00Z
**Depth:** standard
**Files Reviewed:** 1
**Status:** issues_found

## Summary

Reviewed `internal/tools/git.go` (554 lines) — a dedicated Git tool for the AI coding assistant that wraps git commands with structured output parsing. The code is generally well-structured with a clean interface implementation (`types.Tool`), proper use of `exec.CommandContext` for safe command execution, and good separation of concerns across git sub-operations.

Key findings: one critical argument injection vector via unvalidated `args` splitting, a logic bug where `git log` flags aren't properly overridden, inconsistent error handling patterns, and several quality issues including the `WriteString(fmt.Sprintf(...))` anti-pattern flagged in the review request.

---

## Critical Issues

### CR-01: Argument Injection via Unvalidated `args` Parameter Splitting

**File:** `internal/tools/git.go:147,220,255,341,352`

**Issue:** The `args` parameter from LLM input is split via `strings.Fields()` and appended directly to git command arguments without any validation or allowlisting. While `exec.CommandContext` prevents shell injection (no shell interpretation), the LLM (or a prompt injection attack) can pass arbitrary git flags.

**Attack vectors:**
- `git checkout` with `--force` — could discard uncommitted work
- `git checkout` with `--orphan` — creates broken branch state
- `git add` with `--patch` / `-p` — triggers interactive mode, hangs the tool
- `git diff` with `--no-index /etc/passwd` — reads files outside the repository
- `git commit` with `--amend` — modifies previous commit silently
- `git branch -D` (uppercase) — force-deletes unmerged branches (the tool only handles `-d`/`--delete`)

**Fix:**
```go
// Add an allowlist of safe flags per operation
var safeAddFlags = map[string]bool{
    ".": true, "-A": true, "-u": true, "-p": true,
}

// Validate each arg against the allowlist before appending
for _, arg := range strings.Fields(args) {
    if !safeAddFlags[arg] && !strings.HasPrefix(arg, "-") {
        // treat as path
        cmdArgs = append(cmdArgs, arg)
    } else if safeAddFlags[arg] {
        cmdArgs = append(cmdArgs, arg)
    } else {
        return gitResult{Success: false, Operation: "add",
            Error: fmt.Sprintf("unsupported flag: %s", arg)}, nil
    }
}
```

Alternatively, for a tool designed for an AI assistant, document the security boundary clearly and rely on the permission system (`RiskLevel() = RiskDangerous` triggers user approval via the dispatcher).

---

## Warnings

### WR-01: `gitLog` Double-Appends Flags Instead of Overriding Default `-20`

**File:** `internal/tools/git.go:249-256`

**Issue:** The comment says "Override if user provides their own count" but the code *appends* user args after the hardcoded `-20`. When the user passes `-5`, the actual command becomes `git log --oneline --decorate -20 -5` — git silently uses the *last* `-N` value, so this happens to work by accident. However, passing `--oneline` again results in duplicate flags, and passing `--format=...` alongside the hardcoded `--oneline` produces conflicting format output.

```go
// Current (buggy):
cmdArgs = append(cmdArgs, "log", "--oneline", "--decorate", "-20")
if args != "" {
    cmdArgs = append(cmdArgs, strings.Fields(args)...)  // appends, doesn't override
}
```

**Fix:**
```go
// Build base args, then let user args replace defaults
cmdArgs = []string{"log", "--oneline", "--decorate", "-20"}
if args != "" {
    userFields := strings.Fields(args)
    // Check if user provided their own -N count
    hasCount := false
    for _, f := range userFields {
        if strings.HasPrefix(f, "-") && f != "-" {
            if _, err := strconv.Atoi(f[1:]); err == nil {
                hasCount = true
                break
            }
        }
    }
    if hasCount {
        // Remove the default -20
        cmdArgs = cmdArgs[:len(cmdArgs)-1]
    }
    cmdArgs = append(cmdArgs, userFields...)
}
```

### WR-02: Inconsistent Error Handling — `gitResult.Error` vs Returned `error`

**File:** `internal/tools/git.go:115-117,152-158,198-204`

**Issue:** The `Execute` method checks `if err != nil` at line 115 and returns early. However, *all* git sub-operations (gitAdd, gitCommit, etc.) return errors embedded in `gitResult.Error` with a nil Go error — meaning the `if err != nil` check at line 115 never triggers for git failures. The error is instead set on `result.Error` and flows through to `ToolResult.Error` at line 128.

This works but creates a confusing dual-error pattern: callers must check both the returned `error` AND `result.Error`. The `Execute` method at lines 119-122 also has a subtle behavior: if `result.Error != ""`, it overwrites `output` with the error text, so the LLM sees the error in the `Output` field too.

**Fix:** Either return errors as Go errors consistently (removing the `gitResult.Error` field) or document the convention clearly with a comment at the type definition:

```go
// gitResult carries both structured output and error state.
// Errors from git commands are stored in Error (not returned as Go error)
// so they can be presented as structured tool output to the LLM.
```

### WR-03: `extractCommitMessage` Fails on Multi-Word Unquoted Messages

**File:** `internal/tools/git.go:527-553`

**Issue:** When the user passes `-m fix bug in login` (no quotes), `strings.Fields()` splits it into `["-m", "fix", "bug", "in", "login"]`. The function only captures `fields[i+1]` = `"fix"`, discarding the rest of the message. The LLM is likely to send messages like `-m "fix the login page"` (quoted) but there's no validation or error message when the unquoted variant silently truncates.

**Fix:**
```go
if f == "-m" && i+1 < len(fields) {
    msg := fields[i+1]
    // Strip surrounding quotes
    if len(msg) >= 2 {
        if (msg[0] == '"' && msg[len(msg)-1] == '"') ||
            (msg[0] == '\'' && msg[len(msg)-1] == '\'') {
            msg = msg[1 : len(msg)-1]
            return msg
        }
    }
    // No quotes — collect remaining fields as the message
    return strings.Join(fields[i+1:], " ")
}
```

### WR-04: `gitBranch` Delete Ignores Extra Flags and Multiple Branch Names

**File:** `internal/tools/git.go:300-321`

**Issue:** When `fields[0]` is `--delete`, only `fields[1]` is used as the branch name. Extra fields (e.g., `--delete feature-a feature-b`) are silently ignored. Additionally, `-D` (force delete) is not recognized — only `-d` and `--delete` are handled, so a user passing `-D branchname` falls through to the "create branch" path and runs `git branch -D` which git interprets as force-delete (happens to work accidentally, but is unintended).

**Fix:**
```go
if fields[0] == "-d" || fields[0] == "-D" || fields[0] == "--delete" {
    // Use --delete explicitly for clarity
    for _, name := range fields[1:] {
        output, err := g.runGit(ctx, "branch", "--delete", name)
        // ...
    }
}
```

### WR-05: `gitStash push` Hardcodes Message, Ignores User-Provided `-m`

**File:** `internal/tools/git.go:393-406`

**Issue:** The stash push always uses `-m "tool-stash"` regardless of any user-provided message in args. If the LLM passes `push -m "WIP: refactor"`, the `-m` and message in `fields[1:]` are completely ignored.

**Fix:**
```go
if fields[0] == "push" || fields[0] == "save" {
    cmdArgs := []string{"stash", "push"}
    // Look for -m flag in remaining fields
    foundMsg := false
    for i := 1; i < len(fields); i++ {
        if fields[i] == "-m" && i+1 < len(fields) {
            cmdArgs = append(cmdArgs, "-m", fields[i+1])
            foundMsg = true
            i++ // skip message
        }
    }
    if !foundMsg {
        cmdArgs = append(cmdArgs, "-m", "tool-stash")
    }
    output, err := g.runGit(ctx, cmdArgs...)
    // ...
}
```

---

## Info

### IN-01: `WriteString(fmt.Sprintf(...))` Anti-Pattern (Staticcheck QF1012 Candidate)

**File:** `internal/tools/git.go:485,487,490,492,496,498,502,504,516`

**Issue:** Multiple instances of `sb.WriteString(fmt.Sprintf(...))` where `fmt.Fprintf(&sb, ...)` would be more idiomatic and avoids an intermediate string allocation. While staticcheck's QF1012 isn't enabled in this project's `.golangci.yml`, the pattern is a recognized Go anti-pattern.

Affected lines:
- Line 485: `sb.WriteString(fmt.Sprintf("\nStaged (%d):\n", len(staged)))`
- Line 487: `sb.WriteString(fmt.Sprintf("  + %s\n", f))`
- Line 490: `sb.WriteString(fmt.Sprintf("\nModified (%d):\n", len(modified)))`
- Line 492: `sb.WriteString(fmt.Sprintf("  ~ %s\n", f))`
- Line 496: `sb.WriteString(fmt.Sprintf("\nDeleted (%d):\n", len(deleted)))`
- Line 498: `sb.WriteString(fmt.Sprintf("  - %s\n", f))`
- Line 502: `sb.WriteString(fmt.Sprintf("\nUntracked (%d):\n", len(untracked)))`
- Line 504: `sb.WriteString(fmt.Sprintf("  ? %s\n", f))`
- Line 516: `sb.WriteString(fmt.Sprintf("\nOn branch: %s", branch))`

**Fix:**
```go
// Before:
sb.WriteString(fmt.Sprintf("\nStaged (%d):\n", len(staged)))

// After:
fmt.Fprintf(&sb, "\nStaged (%d):\n", len(staged))
```

### IN-02: No Test File for `git.go`

**File:** `internal/tools/git_test.go` (missing)

**Issue:** There are no unit tests for the Git tool. All other tools in the package have test coverage. The `extractCommitMessage` function is particularly testable and has edge cases (empty args, missing `-m`, unquoted multi-word messages) that should be covered.

### IN-03: `gitStatus` Parsing Omits Renamed/Copied/Unmerged Status Codes

**File:** `internal/tools/git.go:469-478`

**Issue:** The porcelain status parser handles `??` (untracked), `D` (deleted), and staged/modified, but doesn't handle:
- `R` (renamed) — files show as "modified" instead of "renamed"
- `C` (copied) — files show as "modified" instead of "copied"
- `U` (unmerged) — merge conflicts are not surfaced

This is acceptable for v1 but should be noted for future improvement.

---

_Reviewed: 2026-07-14T12:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
