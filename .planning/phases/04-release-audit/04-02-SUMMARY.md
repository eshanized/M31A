---
phase: 04-release-audit
plan: 02
subsystem: workflow engine, tool subsystem, subagent manager
tags: [security, concurrency, testing, correctness]
requires:
  - 04-01
provides:
  - C2
  - C3
  - H1
  - H2
  - H3
  - H4
tech_stack_added: []
tech_stack_patterns: [mutex-protection, delimiter-wrapping, worktree-isolation-default]
key_files_modified:
  - internal/workflow/engine.go
  - internal/tools/bash.go
  - internal/tools/dispatcher.go
  - internal/tools/subagent/manager.go
  - internal/tools/subagent/events.go
  - internal/tools/subagent/events_test.go
  - internal/tools/subagent/manager_test.go
  - internal/tools/dispatcher_test.go
  - internal/tools/extra_test.go
  - internal/tools/coverage_boost_test.go
  - pkg/bisect/bisect_test.go
  - internal/tools/bash_security_test.go
  - internal/workflow/engine_race_test.go
decisions:
  - "Default subagent isolation changed from IsolationDefault to IsolationWorktree"
  - "Degraded mode emits EventSpawnFailed instead of failing on worktree creation"
  - "Tool output wrapped in <tool_output> XML delimiters for LLM boundary clarity"
  - "Command substitution detection expanded to include $() and backticks"
metrics:
  duration: "3h"
  completed: "2026-07-14"
  tasks_completed: 5
  files_modified: 13
status: complete
---

# Phase 04 Plan 02: Security, Concurrency, and Test Suite Fixes Summary

Fix CRITICAL data race (C3), test suite timeouts (C2), and HIGH security bypasses (H1-H4) in the workflow engine and tool subsystem.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | C3: Protect e.provider with mutex | 7be5b3b6 | internal/workflow/engine.go, internal/workflow/engine_race_test.go |
| 2 | C2: Fix test suite timeouts | fbd38b2c | internal/tools/coverage_boost_test.go, pkg/bisect/bisect_test.go |
| 3 | H1-H2: Expand command blocklist + prompt injection defense | f35077bd | internal/tools/bash.go, internal/tools/bash_security_test.go |
| 4 | H4: Add tool output delimiters | bc140eb5 | internal/tools/dispatcher.go, internal/tools/dispatcher_test.go, internal/tools/extra_test.go |
| 5 | H3-H4: Default subagent isolation + degraded mode | 5d769003 | internal/tools/subagent/manager.go, internal/tools/subagent/events.go, internal/tools/subagent/events_test.go, internal/tools/subagent/manager_test.go |

## Key Changes

### Task 1: Data Race Fix (C3)
- Moved `e.provider = p` inside `modelIDMu.Lock()` in SetModel
- Added `providerAndModel()` accessor for atomic reads
- Updated all 6 `e.provider` reader sites to use `providerAndModel()`
- Added 3 concurrency tests: TestSetModel_Concurrent, TestSetModel_Consistency, TestSetModel_NilGuard

### Task 2: Test Suite Timeouts (C2)
- Added `testing.Short()` skips to 7 bisect tests making real git operations
- Added `testing.Short()` skips to 6 HTTPCheck tests making real DNS lookups
- Tests complete within 60s under `-short` flag

### Task 3: Command Blocklist + Prompt Injection (H1, H2)
- Expanded blocklist: mkfs.ext4, mkfs.xfs, fdisk, wipefs, shred, nc, ncat
- Expanded `containsVariableExpansion` to detect `$()` command substitution and backticks
- Added security tests for expanded blocklist, command substitution, and chain detection

### Task 4: Tool Output Delimiters (H4)
- Added `wrapToolOutput()` function wrapping output in `<tool_output>` XML delimiters
- Updated dispatcher to wrap all tool output
- Fixed 5 tests expecting raw output

### Task 5: Subagent Isolation + Degraded Mode (H3, H4)
- Changed default isolation from `IsolationDefault` to `IsolationWorktree`
- Added `EventSpawnFailed` event type
- On worktree creation failure, log warning and emit event instead of failing
- Subagent continues in degraded mode using parent workdir
- Added 2 tests: default isolation verification, degraded mode event emission

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing fmt import in manager_test.go**
- **Found during:** Task 5
- **Issue:** Test file used `fmt.Errorf` without importing `fmt`
- **Fix:** Added `fmt` to import block
- **Files modified:** internal/tools/subagent/manager_test.go
- **Commit:** 5d769003

**2. [Rule 1 - Bug] Fixed duplicate isolation defaulting**
- **Found during:** Task 5
- **Issue:** Spawn() had two isolation defaults — original at line 107 and new at line 140. The original overrode the new.
- **Fix:** Changed original default from IsolationDefault to IsolationWorktree, removed duplicate
- **Files modified:** internal/tools/subagent/manager.go
- **Commit:** 5d769003

## Self-Check

- [x] engine_race_test.go: 3 concurrency tests
- [x] coverage_boost_test.go: 6 HTTPCheck tests with -short skips
- [x] bisect_test.go: 7 tests with -short skips
- [x] bash_security_test.go: 3 new security test functions
- [x] dispatcher_test.go: Updated for tool output wrapping
- [x] extra_test.go: Updated for tool output wrapping
- [x] events.go: EventSpawnFailed added
- [x] events_test.go: Updated event type count to 9
- [x] manager_test.go: 2 new tests + failWorktreeProvider mock
- [x] All tests pass under `-short`
