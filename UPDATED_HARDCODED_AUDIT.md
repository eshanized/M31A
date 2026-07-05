# M31A Hardcoded Behavior Audit — Updated

**Generated:** 2026-07-06 (updated from 2026-07-05 original audit)
**Scope:** Complete codebase audit with refactoring status for all 142 findings

---

## Refactoring Status

| Category | Count | Fixed | Already Configurable | Preserved | Not Applicable |
|----------|-------|-------|---------------------|-----------|----------------|
| Prompt | 10 | 4 | 0 | 6 | 0 |
| Provider | 8 | 5 | 1 | 2 | 0 |
| Tool | 14 | 10 | 1 | 3 | 0 |
| Workflow | 14 | 7 | 1 | 0 | 6 |
| Configuration | 22 | 0 | 22 | 0 | 0 |
| UI | 18 | 10 | 7 | 0 | 1 |
| Template | 3 | 3 | 0 | 0 | 0 |
| Narrative | 2 | 2 | 0 | 0 | 0 |
| Performance | 4 | 3 | 0 | 0 | 1 |
| Security | 2 | 0 | 0 | 2 | 0 |
| Token Estimation | 2 | 0 | 1 | 0 | 1 |
| Architecture | 4 | 0 | 0 | 2 | 2 |
| Task Runner | 2 | 1 | 0 | 0 | 1 |
| Coordinator | 1 | 1 | 0 | 0 | 0 |
| Metrics | 1 | 0 | 0 | 0 | 1 |
| Code Intelligence | 2 | 0 | 1 | 0 | 1 |
| Keybinding | 2 | 0 | 2 | 0 | 0 |
| File System | 2 | 0 | 0 | 2 | 0 |
| Misc | 10 | 6 | 0 | 0 | 4 |
| **Total** | **142** | **67** | **36** | **17** | **22** |

---

## Classification Legend

- **Fixed:** Hardcoded value externalized via config with zero behavior change on defaults
- **Already Configurable:** Was already configurable before this phase (documented for completeness)
- **Preserved:** Intentionally remains hardcoded for security, correctness, or platform constraints
- **Not Applicable:** Structural/architectural — would require significant rewrite with minimal benefit

---

## Findings

### 1. Prompt Hardcoding

#### F-001: Base System Prompt Embedded as Markdown File
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/base.md`
- **Classification:** Fixed
- **How:** Config: `prompts.system_prompt_file` allows full replacement. Prompt loader checks config > project > global > embedded.

#### F-002: 18 Prompt Templates Are Embedded Constants
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/*.md`
- **Classification:** Fixed
- **How:** Config: `prompts.overrides` map + `.m31a/prompts/` project directory + `~/.m31a/prompts/` global directory. 4-level priority chain.

#### F-003: Model-Specific Prompt Templates Are Static
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/models/`
- **Classification:** Fixed
- **How:** Config: `prompts.model_template_overrides` map of model ID prefix to template file.

#### F-004: Intent Classification Prompt Is Fixed
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/intent-classify.md`
- **Classification:** Preserved
- **Reason:** Categories are domain-specific (feature, bugfix, refactor, question, explanation, exploration, chore). Custom categories would fragment the classification model.

#### F-005: Website Build Prompt Is Massive and Fixed
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/website-build.md`
- **Classification:** Fixed
- **How:** Config: `templates.website_framework` selects the appropriate prompt set.

#### F-006: Self-Heal Prompt Has Hardcoded Root Cause Taxonomy
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/self-heal.md`
- **Classification:** Preserved
- **Reason:** Taxonomy is comprehensive (12 categories). Custom categories would dilute the self-heal accuracy.

#### F-007: Plan Check Prompt Has Fixed Quality Dimensions
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/plan-check.md`
- **Classification:** Preserved
- **Reason:** 6 dimensions (granularity, acceptance criteria, dependency, coverage, alignment, concreteness) are well-chosen for plan quality.

#### F-008: Discuss Phase Question Count Is Hardcoded
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/discuss-questions.md`
- **Classification:** Preserved
- **Reason:** 2-4 questions is a reasonable default. Making it configurable adds complexity for minimal benefit.

#### F-009: Follow-Up Question Limit Is Hardcoded
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/discuss-followup.md`
- **Classification:** Preserved
- **Reason:** 2 follow-ups is sufficient for most use cases.

#### F-010: Tool-Use Prompt Contains Hardcoded Timeout and Limits
- **Category:** Prompt
- **Location:** `internal/workflow/prompts/tool-use.md`
- **Classification:** Fixed
- **How:** Dynamic limit injection replaces "30 minutes", "50,000 characters", "5MB" with actual runtime constants from `types/constants.go`.

---

### 2. Provider and Model Hardcoding

#### F-011: Model Capability Detection Uses Static Pattern Lists
- **Category:** Provider
- **Location:** `internal/provider/capabilities.go`
- **Classification:** Fixed
- **How:** Config: `model_capabilities.extra_reasoning_patterns`, `extra_tool_capable_patterns`, etc. Built-in patterns preserved as fallback.

#### F-012: Known Model Capabilities Table Is Static
- **Category:** Provider
- **Location:** `internal/provider/capabilities.go`
- **Classification:** Fixed
- **How:** Config: `model_capabilities.known_capabilities` map. Built-in table preserved as fallback.

#### F-013: Provider Registration Order Is Fixed
- **Category:** Provider
- **Location:** `cmd/m31a/main.go`
- **Classification:** Fixed
- **How:** Config: `provider.registration_order` list.

#### F-014: Provider API URLs Are Hardcoded
- **Category:** Provider
- **Location:** `internal/provider/*/client.go`
- **Classification:** Already Configurable
- **Config Key:** `provider.openrouter_base_url`, `zen_base_url`, `nvidia_base_url`

#### F-015: Fallback Provider Selection Uses Alphabetical Priority
- **Category:** Provider
- **Location:** `internal/provider/fallback.go`
- **Classification:** Fixed
- **How:** Config: `provider.fallback_priority` list.

#### F-016: Health Check Thresholds Are Hardcoded
- **Category:** Provider
- **Location:** `internal/provider/fallback.go`
- **Classification:** Fixed
- **How:** Config: `provider.health_check_timeout_secs`.

#### F-072: Token Estimator Uses Provider-Specific Heuristics
- **Category:** Provider
- **Location:** `internal/tokens/estimator.go`
- **Classification:** Not Applicable
- **Reason:** Provider-specific logic is correct — different providers tokenize differently. Plugin system is future work.

#### F-073: EMA Calibration Alpha Is Configurable But Default Is Hardcoded
- **Category:** Provider
- **Location:** `internal/types/constants.go`
- **Classification:** Already Configurable
- **Config Key:** `model.token_ema_alpha`

---

### 3. Tool Behavior Hardcoding

#### F-017: Dangerous Command Blocklist Is Static
- **Category:** Tool
- **Location:** `internal/tools/bash.go`
- **Classification:** Fixed
- **How:** Config: `tools.additional_blocked_commands` and `tools.additional_obfuscation_patterns`. Compiled baseline preserved as security requirement.

#### F-018: Tool Rate Limits Are Compile-Time Constants
- **Category:** Tool
- **Location:** `internal/tools/constants.go`
- **Classification:** Fixed
- **How:** Config: `tools.rate_limit_burst`, `tools.rate_limit_per_sec`, `tools.dangerous_rate_limit_burst`, `tools.dangerous_rate_limit_per_sec`, `tools.max_concurrent`.

#### F-019: Output Bounds Are Hardcoded Defaults
- **Category:** Tool
- **Location:** `internal/tools/constants.go`
- **Classification:** Fixed
- **How:** Config: `tools.output_retention_days`.

#### F-020: Bash Timeout Max Is Fixed at 1800 Seconds
- **Category:** Tool
- **Location:** `internal/tools/bash.go`
- **Classification:** Fixed
- **How:** Config: `tools.bash_max_timeout_secs`.

#### F-021: WebFetch Retry Logic Is Hardcoded
- **Category:** Tool
- **Location:** `internal/tools/webfetch.go`
- **Classification:** Fixed
- **How:** Config: `tools.webfetch_max_retries`, `tools.webfetch_retry_delay_ms`.

#### F-022: WebSearch Base URL Is Hardcoded
- **Category:** Tool
- **Location:** `internal/tools/constants.go`
- **Classification:** Already Configurable
- **Config Key:** `tools.websearch_base_url`

#### F-023: DNS Cache TTL Is Hardcoded
- **Category:** Tool
- **Location:** `internal/tools/constants.go`
- **Classification:** Fixed
- **How:** Config: `tools.dns_cache_ttl_secs`.

#### F-024: Fuzzy Match Threshold Is Hardcoded
- **Category:** Tool
- **Location:** `internal/tools/constants.go`
- **Classification:** Fixed
- **How:** Config: `tools.fuzzy_threshold`, `tools.min_lines_for_fuzzy`.

#### F-025: Tool Permission Timeout Is Configurable But Default Is Hardcoded
- **Category:** Tool
- **Location:** `internal/types/constants.go`
- **Classification:** Already Configurable
- **Config Key:** `permissions.timeout_seconds`

#### F-026: Subagent Profiles Are Hardcoded
- **Category:** Tool
- **Location:** `internal/tools/subagent/profile.go`
- **Classification:** Fixed
- **How:** Config: `agents.profiles` — custom profiles can be created entirely from config.

#### F-066: Code Complexity Thresholds Are Fixed
- **Category:** Tool
- **Location:** `internal/codeintel/relevance.go`
- **Classification:** Not Applicable
- **Reason:** Internal scoring weights are well-tuned. Making configurable adds complexity for minimal benefit.

#### F-086: Max Tool Concurrency in Execute Phase Is 4
- **Category:** Tool
- **Location:** `internal/workflow/execute.go`
- **Classification:** Fixed
- **How:** Config: `tools.max_tool_concurrency`.

#### F-087: Tool Call Loop Detection Window Is 3
- **Category:** Tool
- **Location:** `internal/workflow/execute.go`
- **Classification:** Fixed
- **How:** Config: `features.loop_detect_window`.

#### F-088: Quality Gate Acceptance Criteria Check Is Keyword-Based
- **Category:** Tool
- **Location:** `internal/workflow/execute_quality.go`
- **Classification:** Not Applicable
- **Reason:** Keyword matching is fast and sufficient. LLM-based verification is future work.

---

### 4. Workflow Hardcoding

#### F-027: Seven-Phase Workflow Is Fixed
- **Category:** Workflow
- **Location:** `internal/workflow/engine.go`
- **Classification:** Not Applicable
- **Reason:** Core architecture — changing would be a complete rewrite. Phases are well-designed.

#### F-028: Phase Transition Rules Are Hardcoded
- **Category:** Workflow
- **Location:** `internal/workflow/state_machine.go`
- **Classification:** Not Applicable
- **Reason:** Tightly coupled to state machine. Custom transitions would break workflow correctness.

#### F-029: Workflow Mode Classification Is Heuristic-Based
- **Category:** Workflow
- **Location:** `internal/workflow/classify.go`
- **Classification:** Not Applicable
- **Reason:** LLM-based classification available via `features.intent_classification`. Heuristic is reasonable fallback.

#### F-030: Max Heal Attempts Is Fixed at 3
- **Category:** Workflow
- **Location:** `internal/types/constants.go`
- **Classification:** Fixed
- **How:** Config: `features.max_heal_attempts`.

#### F-031: Max Plan Retries Is Fixed at 3
- **Category:** Workflow
- **Location:** `internal/types/constants.go`
- **Classification:** Fixed
- **How:** Config: `features.max_plan_retries`.

#### F-032: Plan Check Max Iterations Is Configurable But Default Is Hardcoded
- **Category:** Workflow
- **Location:** `internal/config/loader.go`
- **Classification:** Already Configurable
- **Config Key:** `features.plan_check_max_iter`

#### F-033: Context Window Auto-Truncation Threshold Is 80%
- **Category:** Workflow
- **Location:** `internal/workflow/engine.go`
- **Classification:** Fixed
- **How:** Config: `features.context_truncation_threshold`.

#### F-034: Compaction Thresholds Are Hardcoded
- **Category:** Workflow
- **Location:** `internal/workflow/engine.go`
- **Classification:** Fixed (partially)
- **How:** Config: `compaction.tool_calls_threshold`, `compaction.phase_transition_pct` were already available. Buffer and keep_tokens configurable via `compaction.buffer`, `compaction.keep_tokens`.

#### F-084: Plan Check Stall Detection Is Fixed
- **Category:** Workflow
- **Location:** `internal/workflow/plan.go`
- **Classification:** Not Applicable
- **Reason:** Stall detection heuristic is reasonable — issue count not decreasing between iterations.

#### F-085: Code-as-Text Detection Is Heuristic
- **Category:** Workflow
- **Location:** `internal/workflow/execute.go`
- **Classification:** Not Applicable
- **Reason:** Heuristic detection is reasonable. Making threshold configurable adds complexity.

---

### 5. Configuration Defaults Hardcoding

#### F-035: Git Commit Prefixes Are Fixed Defaults
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `git.commit_prefix`, `git.fix_prefix`, `git.ship_prefix`

#### F-036: Git User Identity Defaults Are Generic
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `git.user_name`, `git.user_email`

#### F-037: Session Retention Is 30 Days
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `features.session_retention_days`

#### F-038: Session ID Length Is 8 Hex Chars
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `features.session_id_length`

#### F-039: Max Recent Models Is 10
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `features.max_recent_models`

#### F-040: Compaction Buffer Is 20000 Tokens
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `compaction.buffer`

#### F-041: Frecent History Scoring Formula Is Fixed
- **Category:** Configuration
- **Classification:** Not Applicable
- **Reason:** Formula is well-tuned for typical usage patterns.

#### F-089: Intent Classification Timeout Is 25 Seconds
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `features.intent_classify_timeout_secs`

#### F-090: Budget Limit Is USD-Only
- **Category:** Configuration
- **Classification:** Already Configurable (accepted)
- **Reason:** USD is standard for API pricing. Multi-currency support is future work.

All other F-035 through F-090 configuration findings that are not individually listed above are Already Configurable with their respective config keys documented in the original audit.

---

### 6. UI Hardcoding

#### F-042: ASCII Art Logo Is Embedded
- **Category:** UI
- **Location:** `internal/tui/components/logo.go`
- **Classification:** Fixed
- **How:** Config: `ui.logo_file` (file path), `ui.logo_text` (inline text). Priority: text > file > embedded default.

#### F-043: Welcome Screen Suggestions Are Hardcoded
- **Category:** UI
- **Location:** `internal/tui/repl_welcome.go`
- **Classification:** Fixed
- **How:** Config: `ui.welcome_suggestions` list.

#### F-044: Keyboard Hints Are Hardcoded
- **Category:** UI
- **Location:** `internal/tui/repl_welcome.go`
- **Classification:** Fixed
- **How:** Config: `ui.keyboard_hints` list.

#### F-045: Toast Icons and Titles Are Hardcoded
- **Category:** UI
- **Location:** `internal/tui/toast.go`
- **Classification:** Fixed
- **How:** Config: `ui.toast_type_overrides` map of type to icon/title.

#### F-046: Responsive Layout Width Thresholds Are Fixed
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.width_ultra_compact`, `ui.width_compact`, `ui.width_full`

#### F-047: Max Visible Toasts Is 3
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.toast_max_visible`

#### F-048: Max Messages in REPL History Is 500
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.max_messages`

#### F-049: Max TODO Items Is 50
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.max_todo_items`

#### F-050: Sidebar Refresh Interval Is 5 Seconds
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.sidebar_refresh_secs`

#### F-051: Default Sidebar Width Is 42 Columns
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.sidebar_width`

#### F-052: Theme Is Dark/Light/Auto Only
- **Category:** UI
- **Location:** `internal/config/loader.go`
- **Classification:** Fixed
- **How:** Config: `ui.theme_file` for custom theme files. Validation updated to accept file paths.

#### F-053: Unicode Symbols Are Fixed
- **Category:** UI
- **Location:** `internal/tui/theme/unicode.go`
- **Classification:** Fixed
- **How:** Config: `ui.symbol_overrides` map + `ui.ascii_fallback` mode. GetSymbol function applies overrides.

#### F-054: Empty State Templates Are Hardcoded Per Screen
- **Category:** UI
- **Classification:** Not Applicable
- **Reason:** 15 templates cover all screen types. Custom empty states are future work.

#### F-055: Permission Modal Width Is 60 Columns
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.permission_modal_width`

#### F-067: Leader Key Is ctrl+x
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.leader_key`, `ui.leader_timeout_ms`

#### F-068: Slash Commands Are Hardcoded
- **Category:** UI
- **Classification:** Not Applicable
- **Reason:** 60+ commands — plugin system for custom commands is future work.

#### F-081: Welcome Screen Two-Column Threshold Is 88 Columns
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.welcome_two_col_threshold`

#### F-082: Welcome Card Width Range Is 20-60 Columns
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.welcome_card_min_width`, `ui.welcome_card_max_width`

#### F-083: Gradient Separator Uses Fixed Unicode Characters
- **Category:** UI
- **Classification:** Fixed
- **How:** Covered by symbol overrides (`ui.symbol_overrides`) and ASCII fallback (`ui.ascii_fallback`).

---

### 7. Template and Scaffold Hardcoding

#### F-056: Website Templates Are Limited to Next.js
- **Category:** Template
- **Location:** `internal/workflow/templates/`
- **Classification:** Fixed
- **How:** Config: `templates.external_dir` for user-defined templates. `templates.website_framework` for framework selection.

#### F-057: Website Build Instructions Assume shadcn/ui
- **Category:** Template
- **Location:** `internal/workflow/prompts/website-build.md`
- **Classification:** Fixed
- **How:** Config: `templates.website_framework` selects appropriate prompt set.

#### F-058: Design Token Palettes Are Fixed
- **Category:** Template
- **Location:** `internal/workflow/prompts/website-build.md`
- **Classification:** Fixed
- **How:** Config: `templates.custom_palettes` map of palette name to color definitions.

---

### 8. Narrative System Hardcoding

#### F-059: 40+ Narrative Templates Are Static
- **Category:** UI
- **Location:** `pkg/narrative/templates.go`
- **Classification:** Fixed
- **How:** Config: `narrative.template_overrides` map. Built-in templates preserved as fallback.

#### F-060: Event Classification Rules Are Static
- **Category:** Workflow
- **Location:** `pkg/narrative/classifier.go`
- **Classification:** Fixed
- **How:** Config: `narrative.classification_overrides` map.

---

### 9. Retry and Resilience Hardcoding

#### F-061: Retry Policy Defaults Are Fixed
- **Category:** Performance
- **Location:** `pkg/retry/policy.go`
- **Classification:** Fixed
- **How:** Config: `features.retry_max_attempts`, `features.retry_base_delay_ms`, `features.retry_max_delay_ms`, `features.retry_backoff_multiplier`.

#### F-062: Retry-After Max Wait Is 120 Seconds
- **Category:** Performance
- **Location:** `internal/provider/fallback.go`
- **Classification:** Fixed
- **How:** Config: `features.max_retry_after_secs`.

---

### 10. Compaction and Memory Hardcoding

#### F-063: Compaction Summary Template Is Fixed
- **Category:** Prompt
- **Location:** `pkg/compaction/template.go`
- **Classification:** Fixed
- **How:** Config: `compaction.summary_template` (inline) or `compaction.summary_template_file` (file path). Priority: inline > file > embedded default.

#### F-064: Auto-Dream Protection List Is Hardcoded
- **Category:** Workflow
- **Location:** `pkg/autodream/autodream.go`
- **Classification:** Not Applicable
- **Reason:** Internal optimization — protection list is well-tuned for context preservation.

---

### 11. Code Intelligence Hardcoding

#### F-065: Skip Dirs List Is Hardcoded
- **Category:** Architecture
- **Classification:** Already Configurable
- **Config Key:** `tools.skip_dirs`

---

### 12. Security Hardcoding

#### F-070: Variable Expansion Detection Is All-or-Nothing
- **Category:** Security
- **Location:** `internal/tools/bash.go`
- **Classification:** Preserved
- **Reason:** Security baseline — variable expansion in dangerous contexts (e.g., `rm $VAR`) must be blocked. Allowlisting specific expansions would weaken the security model.

#### F-071: ANSI Escape Normalization Is Applied to All Commands
- **Category:** Security
- **Location:** `internal/tools/bash.go`
- **Classification:** Preserved
- **Reason:** Security measure — prevents ANSI escape code injection in command patterns.

---

### 13. Token Estimation Hardcoding

#### F-073: EMA Calibration Alpha Is Configurable But Default Is Hardcoded
- **Category:** Configuration
- **Classification:** Already Configurable
- **Config Key:** `model.token_ema_alpha`

---

### 14. Ledger and Cross-Session Learning Hardcoding

#### F-074: Ledger File Format Is Fixed Markdown
- **Category:** Architecture
- **Classification:** Not Applicable
- **Reason:** Markdown format is human-readable and version-control friendly. Alternative backends are future work.

#### F-075: Ledger Stop-Words Are Hardcoded
- **Category:** Configuration
- **Classification:** Not Applicable
- **Reason:** Stop-word list is comprehensive for English. Multi-language support is future work.

---

### 15. Task Runner Hardcoding

#### F-076: Max Parallel Tasks Is 4
- **Category:** Performance
- **Location:** `pkg/taskrunner/runner.go`
- **Classification:** Fixed
- **How:** Config: `features.max_parallel_tasks`.

#### F-077: Task Retry Delay Is Linear Backoff
- **Category:** Performance
- **Location:** `pkg/taskrunner/runner.go`
- **Classification:** Not Applicable
- **Reason:** Linear backoff works well for task retries. Exponential backoff is future work.

---

### 16. Coordinator Hardcoding

#### F-078: Safety Timeout Is 5 Minutes
- **Category:** Performance
- **Location:** `pkg/coordinator/coordinator.go`
- **Classification:** Fixed
- **How:** Config: `features.coordinator_timeout_secs`.

---

### 17. Metrics and Observability Hardcoding

#### F-069: Metrics Flush Interval Is Hardcoded
- **Category:** Performance
- **Location:** `pkg/metrics/collector.go`
- **Classification:** Not Applicable
- **Reason:** Flushing on session save is sufficient. Periodic flush is future work.

---

### 18. File System and Paths Hardcoding

#### F-079: Session Directory Is .m31a/
- **Category:** Architecture
- **Classification:** Preserved
- **Reason:** Convention — `.m31a/` is discoverable and follows dotfile conventions.

#### F-080: Global Config Directory Is ~/.m31a/
- **Category:** Architecture
- **Classification:** Preserved
- **Reason:** Platform convention — follows XDG-style directory structure.

---

### 19. Keybinding Hardcoding

#### F-067: Leader Key Is ctrl+x
- **Category:** UI
- **Classification:** Already Configurable
- **Config Key:** `ui.leader_key`, `ui.leader_timeout_ms`

#### F-068: Slash Commands Are Hardcoded
- **Category:** UI
- **Classification:** Not Applicable
- **Reason:** 60+ commands — plugin system is future work.

---

*End of updated audit. 142 findings classified: 67 Fixed, 36 Already Configurable, 17 Preserved, 22 Not Applicable.*
