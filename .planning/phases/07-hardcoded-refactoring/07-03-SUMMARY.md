---
phase: 07-hardcoded-refactoring
plan: 03
subsystem: prompts
tags: [prompts, overrides, config, dynamic-limits, tool-use]

# Dependency graph
requires:
  - phase: 07-01
    provides: "Extended Config struct with ToolsConfig fields (BashMaxTimeoutSecs, etc.)"
provides:
  - "PromptConfig struct with 4-level override chain (config > project > global > embedded)"
  - "internal/workflow/prompts/loader.go with LoadPrompt and LoadModelTemplate"
  - "Dynamic limit injection in tool-use prompt replacing hardcoded 30min/50000/5MB"
  - "Configurable model-specific template overrides"
affects: [07-04, 07-05]

# Tech tracking
tech-stack:
  added: []
  patterns: [prompt-override-chain, dynamic-limit-injection, layered-fallback]

key-files:
  created:
    - internal/workflow/prompts/loader.go
  modified:
    - internal/config/types.go
    - internal/config/loader.go
    - internal/workflow/prompt_builder.go
    - internal/workflow/prompt_templates.go
    - internal/workflow/context_builder.go
    - internal/workflow/engine.go
    - internal/tui/app_update_commands.go

key-decisions:
  - "4-level priority chain: config override > project-level > global > embedded (preserves zero-risk backward compatibility)"
  - "Dynamic limit injection replaces hardcoded values in tool-use prompt to match actual runtime constants"
  - "Nil config handled gracefully in engine and context builder for test compatibility"

patterns-established:
  - "Prompt override chain: LoadPrompt checks config.Overrides[name] > projectPromptDir/name.md > globalPromptDir/name.md > embedded"
  - "Dynamic limit injection: strings.ReplaceAll at load time to reflect actual runtime constants"

requirements-completed: [HARD-08, HARD-09, HARD-10]

# Metrics
duration: 10min
completed: 2026-07-06
---

# Phase 07 Plan 03: Prompt Override System Summary

**Prompt loader with 4-level override chain (config > project > global > embedded) and dynamic limit injection replacing hardcoded tool-use values**

## Performance

- **Duration:** 10 min
- **Started:** 2026-07-05T21:52:17Z
- **Completed:** 2026-07-06T03:02:22Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- Created `internal/workflow/prompts/loader.go` with `LoadPrompt` implementing 4-level priority chain (config override > project-level `.m31a/prompts/` > global `~/.m31a/prompts/` > embedded defaults)
- Added `PromptConfig` struct to `Config` with `system_prompt_file`, `project_prompt_dir`, `global_prompt_dir`, `overrides`, `model_template_overrides` fields
- Integrated loader into `prompt_builder.go` and `prompt_templates.go`, replacing direct `fs.ReadFile` calls with override-aware loading
- Added dynamic limit injection (F-010) that replaces hardcoded "30 minutes", "50,000 characters", "5MB" in tool-use prompt with actual runtime constants from `types/constants.go`

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Prompt Loader with Override Mechanism** - `b4353aed` (feat)
2. **Task 2: Integrate Loader Into Prompt Builder and Inject Dynamic Limits** - `04799afa` (feat)
3. **Lint fix: remove unused promptNames map** - `0aec574b` (fix)

## Files Created/Modified
- `internal/workflow/prompts/loader.go` - New: LoadPrompt (4-level priority) and LoadModelTemplate with override support
- `internal/config/types.go` - Added PromptConfig struct with 5 fields for prompt overrides
- `internal/config/loader.go` - Added DefaultConfig() entries for Prompts, registered "prompts" in knownConfigKeys
- `internal/workflow/prompt_builder.go` - LoadPrompts now uses prompts.LoadPrompt with override chain, added injectToolUseLimits
- `internal/workflow/prompt_templates.go` - SelectTemplate now delegates to prompts.LoadModelTemplate
- `internal/workflow/context_builder.go` - SelectTemplate call updated with config, nil config handled
- `internal/workflow/engine.go` - NewPromptBuilder called with Config.Prompts and WorkDir, nil config handled
- `internal/tui/app_update_commands.go` - LoadPrompts calls updated with config
- `internal/workflow/prompt_builder_test.go` - Updated for new function signatures
- `internal/workflow/prompt_templates_test.go` - Updated for new function signatures
- `internal/workflow/engine_test.go` - Updated for new function signatures
- `internal/workflow/website_build_test.go` - Updated for new function signatures
- `internal/workflow/context_builder_test.go` - Updated for new function signatures

## Decisions Made
- 4-level priority chain preserves zero-risk backward compatibility: embedded prompts always remain as fallback
- Dynamic limit injection uses string replacement at load time (simpler than Go templates, matches existing pattern)
- Nil config handled gracefully in engine and context builder to maintain test compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Handle nil Config in NewEngineFromOptions**
- **Found during:** Task 2 (Integration)
- **Issue:** Test setup passes nil config to NewEngine, causing nil pointer dereference when accessing Config.Prompts
- **Fix:** Added nil check for opts.Config before accessing Prompts field
- **Files modified:** internal/workflow/engine.go
- **Verification:** go build and go vet pass, all tests pass
- **Committed in:** 04799afa (Task 2 commit)

**2. [Rule 1 - Bug] Handle nil Config in ContextBuilder.BuildSystemPrompt**
- **Found during:** Task 2 (Integration)
- **Issue:** Test setup creates ContextBuilder with nil config, causing nil pointer dereference in SelectTemplate call
- **Fix:** Added nil check for cb.cfg before accessing Prompts field
- **Files modified:** internal/workflow/context_builder.go
- **Verification:** go build and go vet pass, all tests pass
- **Committed in:** 04799afa (Task 2 commit)

**3. [Rule 1 - Bug] Remove unused promptNames map**
- **Found during:** Lint verification
- **Issue:** plan code included an unused `promptNames` map that golangci-lint flagged
- **Fix:** Removed the unused variable
- **Files modified:** internal/workflow/prompt_builder.go
- **Verification:** make lint passes (remaining issues are pre-existing in webfetch.go)
- **Committed in:** 0aec574b (lint fix commit)

---

**Total deviations:** 3 auto-fixed (3 bugs: 2 nil-pointer, 1 unused variable)
**Impact on plan:** All auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
- Pre-existing lint warnings in `internal/tools/webfetch.go` (unused constants from plan 07-01) — out of scope, not modified

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Prompt override system complete and ready for use
- Users can create `.m31a/prompts/` directory in their project to override any prompt
- Config-level overrides available via `[prompts]` section in m31a.toml
- Dynamic limit injection ensures tool-use prompt always reflects actual runtime limits
- Zero behavior change when no overrides are set (embedded defaults used)

---
*Phase: 07-hardcoded-refactoring*
*Completed: 2026-07-06*

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Task 1 commit (b4353aed): FOUND
- Task 2 commit (04799afa): FOUND
- Lint fix commit (0aec574b): FOUND
- Shared files (STATE.md, ROADMAP.md) not modified: CONFIRMED
