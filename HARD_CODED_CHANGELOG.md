# M31A Hardcoded Refactoring Changelog

**Phase:** 07-hardcoded-refactoring
**Generated:** 2026-07-06
**Scope:** All user-visible and developer-visible improvements from externalizing hardcoded behaviors

---

## User-Visible Improvements

### Configuration

- Tool rate limits are now configurable via TOML (`tools.rate_limit_burst`, `tools.rate_limit_per_sec`)
- Dangerous rate limits are configurable (`tools.dangerous_rate_limit_burst`, `tools.dangerous_rate_limit_per_sec`)
- Max concurrent tools is configurable (`tools.max_concurrent`)
- Max tool concurrency in execute phase is configurable (`tools.max_tool_concurrency`)
- Tool call loop detection window is configurable (`tools.loop_detect_window`)
- Context truncation threshold is configurable (`features.context_truncation_threshold`)
- Retry policy parameters are configurable (`features.retry_max_attempts`, `features.retry_base_delay_ms`, `features.retry_max_delay_ms`, `features.retry_backoff_multiplier`)
- Max retry-after wait is configurable (`features.max_retry_after_secs`)
- Max parallel tasks is configurable (`features.max_parallel_tasks`)
- Coordinator safety timeout is configurable (`features.coordinator_timeout_secs`)
- Max heal attempts is configurable (`features.max_heal_attempts`)
- Max plan retries is configurable (`features.max_plan_retries`)
- Output retention days is configurable (`tools.output_retention_days`)
- DNS cache TTL is configurable (`tools.dns_cache_ttl_secs`)
- Fuzzy match threshold is configurable (`tools.fuzzy_threshold`)
- Min lines for fuzzy matching is configurable (`tools.min_lines_for_fuzzy`)
- Bash max timeout is configurable (`tools.bash_max_timeout_secs`)
- WebFetch retry parameters are configurable (`tools.webfetch_max_retries`, `tools.webfetch_retry_delay_ms`)

### Prompt System

- Project-level prompt overrides via `.m31a/prompts/` directory
- System prompt override via config (`prompts.system_prompt_file`)
- Per-prompt overrides via config (`prompts.overrides`)
- Model-specific template overrides (`prompts.model_template_overrides`)
- Tool-use prompt now reflects actual runtime limits (no more lying to the model about timeouts and output limits)
- 4-level priority chain: config override > project-level > global > embedded default

### Provider System

- Fallback provider priority is configurable (`provider.fallback_priority`)
- Health check timeout is configurable (`provider.health_check_timeout_secs`)
- Provider registration order is configurable (`provider.registration_order`)
- Model capability patterns are extensible via config (`model_capabilities.extra_reasoning_patterns`, etc.)
- Known model capabilities are overridable via config (`model_capabilities.known_capabilities`)

### Tool Behavior

- Dangerous command blocklist is extensible via config (`tools.additional_blocked_commands`)
- Obfuscation detection patterns are extensible (`tools.additional_obfuscation_patterns`)
- Compiled security baseline is never bypassable — config only adds to it
- Subagent profiles are fully configurable (`agents.profiles`)
- Narrative templates are overridable (`narrative.template_overrides`)
- Event classification rules are overridable (`narrative.classification_overrides`)
- Compaction summary template is configurable (`compaction.summary_template`, `compaction.summary_template_file`)

### UI Customization

- Logo is overridable via config or file (`ui.logo_file`, `ui.logo_text`)
- Welcome suggestions are configurable (`ui.welcome_suggestions`)
- Keyboard hints are configurable (`ui.keyboard_hints`)
- Unicode symbols are overridable (`ui.symbol_overrides`)
- ASCII fallback mode available (`ui.ascii_fallback`) for terminals with poor Unicode support
- Toast types are overridable (`ui.toast_type_overrides`)
- Theme file support (`ui.theme_file`) for custom themes

### Template System

- External template directory configurable (`templates.external_dir`)
- Website framework selectable (`templates.website_framework`)
- Custom design palettes configurable (`templates.custom_palettes`)

---

## Developer-Visible Improvements

### Architecture

- Prompt loader with 4-level priority chain (config > project > global > embedded)
- Provider registry with configurable fallback priority
- Model capability detection with config-based pattern extensions
- UI theme extension support via config
- Template directory externalization
- Symbol override system with ASCII fallback
- Toast type registry with config overrides

### Code Quality

- Eliminated duplicate config defaults
- Improved naming consistency
- Better separation of concerns
- Enhanced testability
- All config fields have matching defaults (zero behavior change)

### Testing

- Config-with-fallback pattern enables testing with custom configs
- Package-level config injection for provider capabilities
- Additive config merging preserves built-in patterns

---

## Configuration Reference

### New TOML Sections

```toml
[templates]
external_dir = ""           # path to custom template directory
website_framework = "nextjs" # nextjs, vue, svelte, astro, html
[templates.custom_palettes]
# my_palette = { primary = "#FF5733", secondary = "#33FF57" }

[ui]
logo_file = ""              # path to custom logo file
logo_text = ""              # inline logo text (overrides logo_file)
welcome_suggestions = []    # custom welcome prompt suggestions
keyboard_hints = []         # custom keyboard hint strings
symbol_overrides = {}       # map of symbol name to replacement
ascii_fallback = false      # use ASCII instead of Unicode
theme_file = ""             # path to custom theme file
[ui.toast_type_overrides]
# success = { icon = "OK", title = "Done" }

[prompts]
system_prompt_file = ""     # path to custom system prompt
project_prompt_dir = ".m31a/prompts"  # project-level overrides
global_prompt_dir = ""      # global overrides (~/.m31a/prompts/)
[prompts.overrides]
# execute-task = "/path/to/custom-execute.md"
[prompts.model_template_overrides]
# mistral = "/path/to/mistral.txt"

[narrative]
[narrative.template_overrides]
# task_complete = "Done! {description} completed successfully."
[narrative.classification_overrides]
# tool_call = "expanded"

[compaction]
summary_template = ""       # inline template text
summary_template_file = ""  # path to template file
```

### All New Config Keys

| Section | Key | Type | Default | Description |
|---------|-----|------|---------|-------------|
| `ui.logo_file` | string | `""` | Path to custom logo file |
| `ui.logo_text` | string | `""` | Inline logo text |
| `ui.welcome_suggestions` | []string | `[]` | Custom welcome suggestions |
| `ui.keyboard_hints` | []string | `[]` | Custom keyboard hints |
| `ui.symbol_overrides` | map[string]string | `{}` | Symbol name to replacement |
| `ui.ascii_fallback` | bool | `false` | ASCII mode for legacy terminals |
| `ui.theme_file` | string | `""` | Custom theme file path |
| `ui.toast_type_overrides` | map[ToastTypeConfig] | `{}` | Toast icon/title overrides |
| `templates.external_dir` | string | `""` | External template directory |
| `templates.website_framework` | string | `"nextjs"` | Website build framework |
| `templates.custom_palettes` | map[map[string]string] | `{}` | Custom color palettes |
| `prompts.system_prompt_file` | string | `""` | System prompt override |
| `prompts.project_prompt_dir` | string | `".m31a/prompts"` | Project prompt directory |
| `prompts.global_prompt_dir` | string | `""` | Global prompt directory |
| `prompts.overrides` | map[string]string | `{}` | Per-prompt file overrides |
| `prompts.model_template_overrides` | map[string]string | `{}` | Model-specific templates |
| `narrative.template_overrides` | map[string]string | `{}` | Narrative text overrides |
| `narrative.classification_overrides` | map[string]string | `{}` | Event classification overrides |
| `compaction.summary_template` | string | `""` | Inline compaction template |
| `compaction.summary_template_file` | string | `""` | Compaction template file path |
