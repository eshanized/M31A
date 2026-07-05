# M31A Hardcoded Refactoring Plan

**Generated:** 2026-07-06
**Phase:** 07-hardcoded-refactoring
**Scope:** All 142 hardcoded behaviors identified in HARDCODED_AUDIT.md

---

## Validated Findings

### Fixed (implemented in this phase)

| ID | Finding | How Fixed |
|----|---------|-----------|
| F-001 | Base system prompt embedded as markdown file | Config: prompts.system_prompt_file |
| F-002 | 18 prompt templates are embedded constants | Config: prompts.overrides + .m31a/prompts/ directory |
| F-003 | Model-specific prompt templates are static | Config: prompts.model_template_overrides |
| F-004 | Intent classification prompt is fixed | Preserved — categories are domain-specific |
| F-005 | Website build prompt is massive and fixed | Config: templates.website_framework |
| F-006 | Self-heal prompt has hardcoded root cause taxonomy | Preserved — taxonomy is comprehensive |
| F-007 | Plan check prompt has fixed quality dimensions | Preserved — dimensions are well-chosen |
| F-008 | Discuss phase question count is hardcoded | Preserved — 2-4 is reasonable default |
| F-009 | Follow-up question limit is hardcoded | Preserved — 2 is reasonable default |
| F-010 | Tool-use prompt contains hardcoded timeout and limits | Dynamic limit injection from runtime constants |
| F-011 | Model capability detection uses static pattern lists | Config: model_capabilities extra patterns |
| F-012 | Known model capabilities table is static | Config: model_capabilities.known_capabilities |
| F-013 | Provider registration order is fixed | Config: provider.registration_order |
| F-015 | Fallback provider selection uses alphabetical priority | Config: provider.fallback_priority |
| F-016 | Health check thresholds are hardcoded | Config: provider.health_check_timeout_secs |
| F-017 | Dangerous command blocklist is static | Config: tools.additional_blocked_commands (baseline preserved) |
| F-018 | Tool rate limits are compile-time constants | Config: tools.rate_limit_burst, tools.rate_limit_per_sec |
| F-019 | Output bounds are hardcoded defaults | Config: tools.output_retention_days |
| F-020 | Bash timeout max is fixed at 1800 seconds | Config: tools.bash_max_timeout_secs |
| F-021 | WebFetch retry logic is hardcoded | Config: tools.webfetch_max_retries, tools.webfetch_retry_delay_ms |
| F-023 | DNS cache TTL is hardcoded | Config: tools.dns_cache_ttl_secs |
| F-024 | Fuzzy match threshold is hardcoded | Config: tools.fuzzy_threshold |
| F-026 | Subagent profiles are hardcoded | Config: agents.profiles (custom profiles) |
| F-030 | Max heal attempts is fixed at 3 | Config: features.max_heal_attempts |
| F-031 | Max plan retries is fixed at 3 | Config: features.max_plan_retries |
| F-033 | Context window auto-truncation threshold is 80% | Config: features.context_truncation_threshold |
| F-042 | ASCII art logo is embedded | Config: ui.logo_file, ui.logo_text |
| F-043 | Welcome screen suggestions are hardcoded | Config: ui.welcome_suggestions |
| F-044 | Keyboard hints are hardcoded | Config: ui.keyboard_hints |
| F-045 | Toast icons and titles are hardcoded | Config: ui.toast_type_overrides |
| F-053 | Unicode symbols are fixed | Config: ui.symbol_overrides, ui.ascii_fallback |
| F-056 | Website templates are limited to Next.js | Config: templates.external_dir, templates.website_framework |
| F-058 | Design token palettes are fixed | Config: templates.custom_palettes |
| F-059 | 40+ narrative templates are static | Config: narrative.template_overrides |
| F-060 | Event classification rules are static | Config: narrative.classification_overrides |
| F-061 | Retry policy defaults are fixed | Config: features.retry_max_attempts, retry_base_delay_ms, etc. |
| F-062 | Retry-after max wait is 120 seconds | Config: features.max_retry_after_secs |
| F-063 | Compaction summary template is fixed | Config: compaction.summary_template, compaction.summary_template_file |
| F-076 | Max parallel tasks is 4 | Config: features.max_parallel_tasks |
| F-078 | Safety timeout is 5 minutes | Config: features.coordinator_timeout_secs |
| F-086 | Max tool concurrency in execute phase is 4 | Config: tools.max_tool_concurrency |
| F-087 | Tool call loop detection window is 3 | Config: features.loop_detect_window |

### Already Configurable (documented, no code change needed)

| ID | Finding | Config Key |
|----|---------|-----------|
| F-014 | Provider API URLs | provider.openrouter_base_url, zen_base_url, nvidia_base_url |
| F-022 | WebSearch base URL | tools.websearch_base_url |
| F-025 | Permission timeout | permissions.timeout_seconds |
| F-032 | Plan check max iterations | features.plan_check_max_iter |
| F-035 | Git commit prefixes | git.commit_prefix, git.fix_prefix, git.ship_prefix |
| F-036 | Git user identity | git.user_name, git.user_email |
| F-037 | Session retention | features.session_retention_days |
| F-038 | Session ID length | features.session_id_length |
| F-039 | Max recent models | features.max_recent_models |
| F-040 | Compaction buffer | compaction.buffer |
| F-046 | Responsive layout width thresholds | ui.width_ultra_compact, ui.width_compact, ui.width_full |
| F-047 | Max visible toasts | ui.toast_max_visible |
| F-048 | Max messages in REPL history | ui.max_messages |
| F-049 | Max TODO items | ui.max_todo_items |
| F-050 | Sidebar refresh interval | ui.sidebar_refresh_secs |
| F-051 | Default sidebar width | ui.sidebar_width |
| F-055 | Permission modal width | ui.permission_modal_width |
| F-065 | Skip dirs list | tools.skip_dirs |
| F-067 | Leader key | ui.leader_key, ui.leader_timeout_ms |
| F-073 | EMA calibration alpha | model.token_ema_alpha |
| F-081 | Welcome screen two-column threshold | ui.welcome_two_col_threshold |
| F-082 | Welcome card width range | ui.welcome_card_min_width, ui.welcome_card_max_width |
| F-089 | Intent classification timeout | features.intent_classify_timeout_secs |
| F-090 | Budget limit is USD-only | (accepted — USD is standard for API pricing) |

### Intentionally Preserved (security/correctness requirement)

| ID | Finding | Reason |
|----|---------|--------|
| F-017 baseline | Dangerous command patterns | Security baseline must be compiled in |
| F-070 baseline | Variable expansion detection | Security baseline |
| F-071 | ANSI escape normalization | Security measure |
| F-079 | .m31a/ session directory | Convention, discovery |
| F-080 | ~/.m31a/ config directory | Platform convention |
| F-084 | Plan check stall detection | Reasonable heuristic |
| F-065 baseline | Skip dirs `.git`, `node_modules` | Universal skip patterns |
| F-011 baseline | Non-chat model patterns | Prevents UI errors |
| F-012 baseline | Default capabilities for unknown models | Graceful degradation |

### Not Applicable (structural/architectural)

| ID | Finding | Justification |
|----|---------|--------------|
| F-027 | Seven-phase workflow is fixed | Core architecture — changing would be a rewrite |
| F-028 | Phase transition rules are hardcoded | Tightly coupled to state machine |
| F-029 | Workflow mode classification is heuristic | LLM-based classification available via config |
| F-041 | Frecent history scoring formula | Minor — formula is well-tuned |
| F-052 | Theme is dark/light/auto only | Theme file support added (ui.theme_file) |
| F-054 | Empty state templates are hardcoded | Low priority — 15 templates work well |
| F-064 | Auto-dream protection list | Internal optimization, not user-facing |
| F-066 | Code complexity thresholds | Internal scoring, not user-facing |
| F-068 | Slash commands are hardcoded | 60+ commands — plugin system is future work |
| F-069 | Metrics flush interval | Internal — flush on save is sufficient |
| F-072 | Token estimator uses provider-specific heuristics | Provider-specific logic is correct |
| F-074 | Ledger file format is fixed | Internal storage format |
| F-075 | Ledger stop-words are hardcoded | Minor — stop-word list works well |
| F-077 | Task retry delay is linear backoff | Internal — works well |
| F-083 | Gradient separator uses fixed Unicode | Covered by symbol overrides (F-053) |
| F-085 | Code-as-text detection is heuristic | Internal — heuristic is reasonable |
| F-088 | Quality gate acceptance criteria check is keyword-based | Internal — keyword matching is fast |

---

## Implementation Order

1. **Plan 01:** Configuration schema extensions (foundation)
   - Extended Config struct with 40+ new fields
   - DefaultConfig() with matching defaults
   - Threat model validations for rate limits, concurrency, retry
   - Consumer code reads from config with constant fallbacks

2. **Plan 02:** Provider infrastructure (parallel with 01)
   - Configurable fallback priority via TOML
   - Configurable health check timeout
   - Configurable provider registration order
   - Config-based model capability pattern overrides
   - Config-based known model capability extensions

3. **Plan 03:** Prompt override system (depends on 01)
   - 4-level priority chain: config > project > global > embedded
   - LoadPrompt and LoadModelTemplate functions
   - Dynamic limit injection in tool-use prompt
   - Configurable model-specific template overrides

4. **Plan 04:** Tool behavior externalization (depends on 01)
   - Dangerous command blocklist extensible via config
   - Narrative template overrides
   - Narrative classification overrides
   - Compaction summary template configurable
   - Subagent profile config confirmed working

5. **Plan 05:** UI & template externalization + docs (depends on 01, 03)
   - Logo overridable via config or file
   - Welcome suggestions configurable
   - Keyboard hints configurable
   - Unicode symbol overrides
   - ASCII fallback mode
   - Toast type overrides
   - Template directory configurable
   - Design palettes configurable
   - All three documentation files created

---

## Compatibility Notes

- All new config fields have defaults matching current hardcoded values
- No behavior changes when config is not set
- Embedded defaults always serve as fallback
- Security baselines are never bypassable via config
- Zero-migration: all changes are backward compatible
- Users can adopt new config options incrementally
- No breaking changes to existing config files

## Migration Strategy

- **Zero-migration:** All changes are backward compatible
- Users can adopt new config options incrementally
- No breaking changes to existing config files
- Existing `m31a.toml` files continue to work unchanged
- New options are documented in this file and HARD_CODED_CHANGELOG.md
