---
phase: 07-hardcoded-refactoring
plan: 04
subsystem: tools
tags: [config, bash, dangerous-commands, subagent-profiles, narrative, compaction, templates]

# Dependency graph
requires:
  - phase: 07-01
    provides: "Config struct with ToolsConfig, AgentsConfig, CompactionConfig sections"
provides:
  - "Configurable dangerous command blocklist extensions via tools.additional_blocked_commands"
  - "Configurable obfuscation pattern extensions via tools.additional_obfuscation_patterns"
  - "Narrative template overrides via narrative.template_overrides"
  - "Narrative classification overrides via narrative.classification_overrides"
  - "Compaction summary template override via compaction.summary_template and summary_template_file"
affects: [narrative, compaction, tools, config]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Config-only override pattern with compiled baseline preservation"]

key-files:
  created: []
  modified:
    - internal/config/types.go
    - internal/config/loader.go
    - internal/tools/bash.go
    - internal/tools/defaults.go
    - internal/tools/subagent/profile.go
    - pkg/narrative/templates.go
    - pkg/narrative/classifier.go
    - pkg/narrative/engine.go
    - pkg/compaction/template.go
    - pkg/compaction/compaction.go
    - internal/tui/narrative_emitter.go
    - internal/tui/app.go
    - internal/workflow/engine.go

key-decisions:
  - "Config patterns only ADD to compiled security baseline, never remove"
  - "Narrative template overrides preserve category/priority/display from defaults"
  - "Classification overrides support string values: narrative, grouped, hidden, expanded"
  - "Compaction template priority: inline > file > embedded default"
  - "Subagent profile config already existed (AgentsConfig.Profiles) - no changes needed"

patterns-established:
  - "Compiled baseline preservation: security-critical defaults remain in source, config only extends"
  - "Config override pattern: NewXWithOverrides() constructors accept map[string]string from config"

requirements-completed: [HARD-11, HARD-12, HARD-13]

# Metrics
duration: 8min
completed: 2026-07-05
---

# Phase 07 Plan 04: Tool Behavior Externalization Summary

**Configurable dangerous command blocklist, narrative template overrides, and compaction template config with compiled security baselines preserved as fallback**

## Performance

- **Duration:** 8 min
- **Started:** 2026-07-05T22:03:39Z
- **Completed:** 2026-07-05T22:11:47Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- Dangerous command blocklist extensible via config (AdditionalBlockedCommands, AdditionalObfuscationPatterns) with compiled baseline never bypassable
- Narrative templates overridable via config (TemplateOverrides map)
- Event classification rules overridable via config (ClassificationOverrides map)
- Compaction summary template configurable via inline text or file path
- Subagent profile config already existed (AgentsConfig.Profiles) - confirmed working

## Task Commits

Each task was committed atomically:

1. **Task 1: Dangerous Command Blocklist Extension and Subagent Profile Config** - `f0f2a238` (feat)
2. **Task 2: Narrative Template Overrides and Compaction Template Config** - `41c2eed6` (feat)

## Files Created/Modified
- `internal/config/types.go` - Added AdditionalBlockedCommands, AdditionalObfuscationPatterns to ToolsConfig; NarrativeConfig struct; SummaryTemplate/SummaryTemplateFile to CompactionConfig
- `internal/config/loader.go` - Added defaults for new config fields; added "narrative" to known config keys
- `internal/tools/bash.go` - Updated Bash struct to hold additional patterns; update checkDangerousCommand to check user patterns after compiled baseline
- `internal/tools/defaults.go` - Pass config patterns to NewBash constructor
- `pkg/narrative/templates.go` - Added NewTemplateResolverWithOverrides for config-based template overrides
- `pkg/narrative/classifier.go` - Added NewClassifierWithOverrides for config-based classification overrides
- `pkg/narrative/engine.go` - Added EngineConfigWithOverrides for config-aware narrative engine
- `pkg/compaction/template.go` - Renamed summaryTemplate to defaultSummaryTemplate; added TemplateWithConfig with inline > file > default priority
- `pkg/compaction/compaction.go` - Added SummaryTemplate and SummaryTemplateFile to Config; updated generateSummary to use TemplateWithConfig
- `internal/tui/narrative_emitter.go` - Updated newNarrativeEmitter to accept config and apply narrative overrides
- `internal/tui/app.go` - Pass config to newNarrativeEmitter
- `internal/workflow/engine.go` - Updated compactionConfig to pass SummaryTemplate and SummaryTemplateFile

## Decisions Made
- Config patterns only ADD to compiled security baseline, never remove (security requirement)
- Narrative template overrides preserve category/priority/display from defaults (only text changes)
- Classification overrides support string values mapped to Classification constants
- Compaction template priority: inline > file > embedded default
- Subagent profile config already existed from previous work - no changes needed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated all NewBash callers to pass additional parameters**
- **Found during:** Task 1 (Dangerous Command Blocklist Extension)
- **Issue:** NewBash signature changed from 2 to 4 parameters, breaking all callers
- **Fix:** Updated all test files and production callers to pass nil, nil for additional patterns
- **Files modified:** internal/tools/bash_test.go, bash_security_test.go, coverage_boost_test.go, tooldefs_test.go, toolinput_test.go, tools_test.go, internal/workflow/engine_test.go
- **Verification:** go build ./... succeeds, go test passes
- **Committed in:** f0f2a238 (Task 1 commit)

**2. [Rule 2 - Missing Critical] Added "narrative" to knownConfigKeys**
- **Found during:** Task 2 (Narrative Template Overrides)
- **Issue:** New top-level config section "narrative" would trigger unknown key warning
- **Fix:** Added "narrative": true to knownConfigKeys map
- **Files modified:** internal/config/loader.go
- **Verification:** No warnings on config load
- **Committed in:** 41c2eed6 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
- Pre-existing lint warnings in internal/tools/webfetch.go (unused constants from plan 07-01) - out of scope
- Pre-existing test timeout in TestHTTPCheck_Execute_NotExpectedContent (DNS resolution hang) - out of scope

## Known Stubs
None - all config fields have proper defaults and the embedded templates remain as fallback.

## Threat Flags
None - new config surface follows existing patterns. Compiled security baseline for dangerous commands is never bypassable via config.

## Next Phase Readiness
- All hardcoded tool behaviors externalized via config
- Embedded defaults preserved as fallback for all externalized content
- Ready for remaining hardcoded refactoring plans

---
*Phase: 07-hardcoded-refactoring*
*Completed: 2026-07-05*
