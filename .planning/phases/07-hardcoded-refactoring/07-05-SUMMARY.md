---
phase: 07-hardcoded-refactoring
plan: 05
subsystem: ui
tags: [logo, welcome, unicode, symbols, toast, templates, config, documentation]

# Dependency graph
requires:
  - phase: 07-01
    provides: "Extended Config struct with UIConfig fields"
  - phase: 07-03
    provides: "Prompt override system with 4-level priority chain"
provides:
  - "Logo overridable via config (ui.logo_file, ui.logo_text)"
  - "Welcome suggestions configurable (ui.welcome_suggestions)"
  - "Keyboard hints configurable (ui.keyboard_hints)"
  - "Unicode symbol overrides (ui.symbol_overrides, ui.ascii_fallback)"
  - "Toast type overrides (ui.toast_type_overrides)"
  - "Template directory configurable (templates.external_dir, templates.website_framework)"
  - "Design palettes configurable (templates.custom_palettes)"
  - "HARD_CODED_REFACTOR_PLAN.md with validated findings"
  - "HARD_CODED_CHANGELOG.md with all improvements documented"
  - "UPDATED_HARDCODED_AUDIT.md classifying all 142 findings"
affects: [ui, config, documentation]

# Tech tracking
tech-stack:
  added: []
  patterns: [config-with-fallback, variadic-optional-param, symbol-override-map, ascii-fallback]

key-files:
  created:
    - HARD_CODED_REFACTOR_PLAN.md
    - HARD_CODED_CHANGELOG.md
    - UPDATED_HARDCODED_AUDIT.md
  modified:
    - internal/config/types.go
    - internal/config/loader.go
    - internal/tui/components/logo.go
    - internal/tui/repl_welcome.go
    - internal/tui/toast.go
    - internal/tui/theme/unicode.go
    - internal/tui/home_model.go
    - internal/tui/home_view.go
    - internal/tui/firstrun_view.go
    - internal/tui/app_nav.go
    - internal/tui/app_view.go
    - internal/tui/app_state.go

key-decisions:
  - "Used variadic params for logo functions to maintain backward compatibility"
  - "resolveLogoText handles text > file > embedded priority in one place"
  - "Symbol override map covers all 30+ Unicode symbols with ASCII fallbacks"
  - "Toast overrides use package-level var for zero-allocation rendering"
  - "All 142 audit findings classified into Fixed/Configurable/Preserved/NotApplicable"

patterns-established:
  - "Config-with-fallback: consumer checks cfg.Field before using config value, falls back to constant"
  - "Symbol override map: named symbols overridable via config with ASCII fallback for legacy terminals"
  - "Variadic optional param: backward-compatible function extensions without breaking callers"

requirements-completed: [HARD-14, HARD-15]

# Metrics
duration: 10min
completed: 2026-07-06
---

# Phase 07 Plan 05: UI & Template Externalization Summary

**Logo, welcome suggestions, Unicode symbols, toast types, and template directories externalized via TOML config with zero behavior change on defaults, plus three documentation deliverables classifying all 142 audit findings**

## Performance

- **Duration:** 10 min
- **Started:** 2026-07-05T22:15:43Z
- **Completed:** 2026-07-05T22:25:37Z
- **Tasks:** 2
- **Files modified:** 12 (+ 3 created)

## Accomplishments
- Logo overridable via config (text or file) with priority: inline > file > embedded default
- Welcome suggestions and keyboard hints configurable from TOML
- Unicode symbols overridable via config map with ASCII fallback for legacy terminals
- Toast types overridable via config (icon and title per type)
- Template directory externalized (external_dir, website_framework, custom_palettes)
- All 142 audit findings classified: 67 Fixed, 36 Already Configurable, 17 Preserved, 22 Not Applicable
- HARD_CODED_REFACTOR_PLAN.md created with validated findings and implementation order
- HARD_CODED_CHANGELOG.md created with all improvements documented
- UPDATED_HARDCODED_AUDIT.md created with every finding classified

## Task Commits

Each task was committed atomically:

1. **Task 1: UI Externalization — Logo, Welcome, Symbols, Theme** - `67e54c86` (feat)
2. **Task 2: Website Template Directory Config and Documentation Deliverables** - `280a1fb9` (feat)

## Files Created/Modified
- `internal/config/types.go` - Added LogoFile, LogoText, WelcomeSuggestions, KeyboardHints, SymbolOverrides, ASCIIFallback, ThemeFile, ToastTypeOverrides to UIConfig; added ToastTypeConfig and TemplateConfig structs
- `internal/config/loader.go` - Added defaults for new UI and template config fields; added "templates" to knownConfigKeys
- `internal/tui/components/logo.go` - RenderLogo/RenderBigLogo accept optional customLogoText via variadic param
- `internal/tui/repl_welcome.go` - Added resolveLogoText (text > file > embedded), config-aware renderKeyboardHints, config-aware welcomePrompts
- `internal/tui/toast.go` - Added toastOverrides var, SetToastOverrides, getToastConfig with override support
- `internal/tui/theme/unicode.go` - Added unicodeSymbols/asciiSymbols maps, GetSymbol function with override and ASCII fallback
- `internal/tui/home_model.go` - Added cfg field and SetConfig method for logo customization
- `internal/tui/home_view.go` - Passes logo config to RenderBigLogo
- `internal/tui/firstrun_view.go` - Passes logo config to RenderBigLogo
- `internal/tui/app_nav.go` - Wires config to HomeModel on creation
- `internal/tui/app_view.go` - Wires config to HomeModel on creation
- `internal/tui/app_state.go` - Wires toast overrides from config
- `HARD_CODED_REFACTOR_PLAN.md` - Validated findings with implementation order
- `HARD_CODED_CHANGELOG.md` - All user-visible and developer-visible improvements
- `UPDATED_HARDCODED_AUDIT.md` - All 142 findings classified

## Decisions Made
- Used variadic params for RenderLogo/RenderBigLogo to maintain backward compatibility with existing callers
- resolveLogoText handles text > file > embedded priority in one reusable function
- Symbol override map covers all 30+ Unicode symbols with corresponding ASCII fallbacks
- Toast overrides use package-level var for zero-allocation rendering path
- All 142 audit findings classified into 4 categories with clear rationale

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added config field to HomeModel for logo customization**
- **Found during:** Task 1 (Logo config override)
- **Issue:** HomeModel had no config field, so logo config couldn't be passed to renderHome
- **Fix:** Added cfg field to HomeModel, SetConfig method, wired at all 3 creation sites
- **Files modified:** internal/tui/home_model.go, internal/tui/app_nav.go, internal/tui/app_view.go
- **Verification:** go build and go vet pass
- **Committed in:** 67e54c86 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Auto-fix necessary for logo config to flow to HomeModel. No scope creep.

## Issues Encountered
- Pre-existing lint warnings in internal/tools/webfetch.go (unused constants from plan 07-01) — out of scope

## Known Stubs
None - all config fields have working defaults and embedded values remain as fallback.

## Threat Flags
| Flag | File | Description |
|------|------|-------------|
| T-07-14 | internal/tui/components/logo.go | Custom logo/text is cosmetic only — users control their own display |
| T-07-15 | internal/config/types.go | External template dir could contain malicious content — rendered by AI, not executed as code |
| T-07-16 | internal/config/types.go | Custom theme/palette could have poor contrast — user can revert via config |

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All UI elements configurable via TOML
- Template system externalized for custom frameworks
- Three documentation deliverables complete
- Zero behavior change when no config overrides are set
- Phase 07-hardcoded-refactoring complete

---
*Phase: 07-hardcoded-refactoring*
*Completed: 2026-07-06*

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Task 1 commit (67e54c86): FOUND
- Task 2 commit (280a1fb9): FOUND
- HARD_CODED_REFACTOR_PLAN.md: FOUND
- HARD_CODED_CHANGELOG.md: FOUND
- UPDATED_HARDCODED_AUDIT.md: FOUND
- Shared files (STATE.md, ROADMAP.md) not modified: CONFIRMED
