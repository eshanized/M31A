# Logic Bug Audit

**Analysis Date:** 2025-01-20
**Files inventoried:** 835  **Files read:** 32  **Coverage:** 3.8%

## Coverage Ledger

[ ] .env.example
[ ] .github/FUNDING.yml
[ ] .github/ISSUE_TEMPLATE/bug_report.md
[ ] .github/ISSUE_TEMPLATE/feature_request.md
[ ] .github/PULL_REQUEST_TEMPLATE.md
[ ] .github/dependabot.yml
[ ] .github/workflows/ci.yml
[ ] .gitignore
[ ] .golangci.yml
[ ] .goreleaser.yaml
[ ] .planning/codebase/ARCHITECTURE.md
[ ] .planning/codebase/CONCERNS.md
[ ] .planning/codebase/CONVENTIONS.md
[ ] .planning/codebase/INTEGRATIONS.md
[ ] .planning/codebase/STACK.md
[ ] .planning/codebase/STRUCTURE.md
[ ] .planning/codebase/TESTING.md
[ ] AGENTS.md
[ ] AUDIT_REPORT.md
[ ] CHANGELOG.md
[ ] CODE_OF_CONDUCT.md
[ ] CONTRIBUTING.md
[ ] LICENSE
[ ] M31A.wiki
[ ] Makefile
[ ] README.md
[ ] SECURITY.md
[ ] TESTING.md
[ ] cmd/m31a/.gitignore
[x] cmd/m31a/main.go
[ ] cmd/m31a/main_test.go
[ ] cmd/m31a/usage.go
[ ] cmd/m31a/usage_test.go
[ ] docs/ARCHITECTURE.md
[ ] docs/CONFIG.md
[ ] docs/INTERFACES.md
[ ] docs/KEYBINDINGS.md
[ ] docs/ONBOARDING.md
[ ] docs/PROVIDERS.md
[ ] docs/QUICKSTART.md
[ ] docs/SCREENS.md
[ ] docs/SLASH_COMMANDS.md
[ ] docs/TOOLS.md
[ ] docs/TROUBLESHOOTING.md
[ ] docs/TYPES.md
[ ] docs/WORKFLOW.md
[ ] docs/archive/DECOMPOSITION_PLAN.md
[ ] docs/archive/DX_AUDIT.md
[ ] docs/archive/FUNCTIONAL_REGRESSION_REPORT.md
[ ] docs/archive/HIGH_PRIORITY_VERIFICATION_REPORT.md
[ ] docs/archive/REGRESSION_ANALYSIS_REPORT.md
[ ] docs/archive/RELEASE_AUDIT_RESOLUTION.md
[ ] docs/archive/RELEASE_AUDIT_V1.md
[ ] go.mod
[ ] go.sum
[ ] install.sh
[ ] internal/core/config/config_extra_test.go
[x] internal/core/config/config_validate.go
[ ] internal/core/config/extra_test.go
[ ] internal/core/config/fallback_priority_test.go
[ ] internal/core/config/instructions.go
[ ] internal/core/config/integration_test.go
[ ] internal/core/config/known_keys_test.go
[x] internal/core/config/loader.go
[ ] internal/core/config/loader_test.go
[x] internal/core/config/merge.go
[ ] internal/core/config/merge_test.go
[ ] internal/core/config/project_context.go
[ ] internal/core/config/project_context_test.go
[ ] internal/core/config/types.go
[ ] internal/core/config/types_test.go
[x] internal/core/errors/errors.go
[x] internal/core/errors/errors_test.go
[x] internal/core/types/constants.go
[x] internal/core/types/filelock_windows.go
[x] internal/core/types/fileutil.go
[x] internal/core/types/fileutil_fcntl_test.go
[x] internal/core/types/fileutil_windows.go
[x] internal/core/types/git.go
[x] internal/core/types/plan.go
[x] internal/core/types/toolcall.go
[x] internal/core/types/types.go
[ ] internal/engine/bisect/bisect.go
[ ] internal/engine/bisect/bisect_test.go
[ ] internal/engine/bisect/doc.go
[ ] internal/engine/bisect/doc_test.go
[ ] internal/engine/bisect/exec.go
[ ] internal/engine/bisect/extra_test.go
[ ] internal/engine/compaction/compaction.go
[ ] internal/engine/compaction/compaction_extra_test.go
[ ] internal/engine/compaction/compaction_test.go
[ ] internal/engine/compaction/serialize.go
[ ] internal/engine/compaction/template.go
[ ] internal/engine/coordinator/coordinator.go
[ ] internal/engine/coordinator/coordinator_test.go
[ ] internal/engine/decision/decision_test.go
[ ] internal/engine/decision/logger.go
[ ] internal/engine/decision/query.go
[ ] internal/engine/decision/receipt.go
[ ] internal/engine/decision/redact.go
[ ] internal/engine/narrative/benchmarks_test.go
[ ] internal/engine/narrative/bridge.go
[ ] internal/engine/narrative/bridge_test.go
[ ] internal/engine/narrative/classifier.go
[ ] internal/engine/narrative/classifier_test.go
[ ] internal/engine/narrative/engine.go
[ ] internal/engine/narrative/engine_test.go
[ ] internal/engine/narrative/event.go
[ ] internal/engine/narrative/grouper.go
[ ] internal/engine/narrative/grouper_test.go
[ ] internal/engine/narrative/templates.go
[ ] internal/engine/narrative/templates_test.go
[ ] internal/engine/narrative/timing.go
[ ] internal/engine/narrative/timing_test.go
[ ] internal/engine/narrative/types.go
[ ] internal/engine/narrative/types_test.go
[ ] internal/engine/rollback/doc.go
[ ] internal/engine/rollback/doc_test.go
[ ] internal/engine/rollback/rollback.go
[ ] internal/engine/rollback/rollback_test.go
[ ] internal/engine/session/checkpoint.go
[ ] internal/engine/session/checkpoint_test.go
[ ] internal/engine/session/doc.go
[ ] internal/engine/session/doc_test.go
[ ] internal/engine/session/fileutil.go
[ ] internal/engine/session/fileutil_windows.go
[ ] internal/engine/session/manager.go
[ ] internal/engine/session/manager_extra_test.go
[ ] internal/engine/session/manager_race_test.go
[ ] internal/engine/session/manager_test.go
[ ] internal/engine/session/planning.go
[ ] internal/engine/session/planning_extra_test.go
[ ] internal/engine/session/planning_test.go
[ ] internal/engine/session/session.go
[ ] internal/engine/session/session_info.go
[ ] internal/engine/session/session_info_test.go
[ ] internal/engine/session/session_resumedat_test.go
[ ] internal/engine/session/session_test.go
[ ] internal/engine/taskrunner/doc.go
[ ] internal/engine/taskrunner/doc_test.go
[ ] internal/engine/taskrunner/runner.go
[ ] internal/engine/taskrunner/runner_test.go
[ ] internal/engine/tokens/context_warning_test.go
[ ] internal/engine/tokens/estimator.go
[ ] internal/engine/tokens/estimator_test.go
[ ] internal/engine/workflow/agent_switch.go
[ ] internal/engine/workflow/agent_switch_test.go
[ ] internal/engine/workflow/bisect_rollback_test.go
[ ] internal/engine/workflow/classify.go
[ ] internal/engine/workflow/classify_test.go
[ ] internal/engine/workflow/context_builder.go
[ ] internal/engine/workflow/context_builder_test.go
[ ] internal/engine/workflow/cost_tracker.go
[ ] internal/engine/workflow/cost_tracker_test.go
[x] internal/engine/workflow/coverage_boost_test.go
[ ] internal/engine/workflow/coverage_gates.go
[ ] internal/engine/workflow/coverage_gates_extra_test.go
[ ] internal/engine/workflow/coverage_gates_test.go
[ ] internal/engine/workflow/diff_summary.go
[ ] internal/engine/workflow/diff_summary_test.go
[ ] internal/engine/workflow/discuss.go
[ ] internal/engine/workflow/discuss_check.go
[ ] internal/engine/workflow/discuss_check_test.go
[ ] internal/engine/workflow/discuss_test.go
[x] internal/engine/workflow/engine.go
[ ] internal/engine/workflow/engine_event_methods.go
[ ] internal/engine/workflow/engine_extra_test.go
[ ] internal/engine/workflow/engine_messages.go
[ ] internal/engine/workflow/engine_messages_test.go
[ ] internal/engine/workflow/engine_parse.go
[ ] internal/engine/workflow/engine_parse_extra_test.go
[ ] internal/engine/workflow/engine_parse_test.go
[ ] internal/engine/workflow/engine_race_test.go
[ ] internal/engine/workflow/engine_test.go
[ ] internal/engine/workflow/engine_verify.go
[ ] internal/engine/workflow/engine_verify_test.go
[ ] internal/engine/workflow/engine_wiring_test.go
[x] internal/engine/workflow/execute.go
[ ] internal/engine/workflow/execute_heal.go
[ ] internal/engine/workflow/execute_preflight.go
[ ] internal/engine/workflow/execute_preflight_test.go
[ ] internal/engine/workflow/execute_quality.go
[ ] internal/engine/workflow/execute_quality_test.go
[ ] internal/engine/workflow/execute_test.go
[ ] internal/engine/workflow/fixes_test.go
[ ] internal/engine/workflow/init_deep.go
[ ] internal/engine/workflow/init_deep_test.go
[ ] internal/engine/workflow/initialize.go
[ ] internal/engine/workflow/initialize_test.go
[ ] internal/engine/workflow/instructions_source_test.go
[ ] internal/engine/workflow/integration_test.go
[ ] internal/engine/workflow/intent.go
[ ] internal/engine/workflow/intent_accuracy_test.go
[ ] internal/engine/workflow/intent_test.go
[ ] internal/engine/workflow/intermediate_progress_test.go
[ ] internal/engine/workflow/messages_race_test.go
[ ] internal/engine/workflow/narrative_events_test.go
[ ] internal/engine/workflow/parse_tool_calls_test.go
[ ] internal/engine/workflow/phase_coordinator.go
[ ] internal/engine/workflow/phase_coordinator_test.go
[ ] internal/engine/workflow/phase_transition_test.go
[ ] internal/engine/workflow/plan.go
[ ] internal/engine/workflow/plan_check.go
[ ] internal/engine/workflow/plan_check_test.go
[ ] internal/engine/workflow/plan_chunk.go
[ ] internal/engine/workflow/plan_chunk_test.go
[ ] internal/engine/workflow/plan_parser.go
[ ] internal/engine/workflow/plan_parser_extra_test.go
[ ] internal/engine/workflow/plan_parser_test.go
[ ] internal/engine/workflow/plan_race_test.go
[ ] internal/engine/workflow/plan_test.go
[ ] internal/engine/workflow/prompt_builder.go
[ ] internal/engine/workflow/prompt_builder_test.go
[ ] internal/engine/workflow/prompt_templates.go
[ ] internal/engine/workflow/prompt_templates_test.go
[ ] internal/engine/workflow/prompts/autonomous.md
[ ] internal/engine/workflow/prompts/base.md
[ ] internal/engine/workflow/prompts/code-intelligence.md
[ ] internal/engine/workflow/prompts/code-quality.md
[ ] internal/engine/workflow/prompts/context-awareness.md
[ ] internal/engine/workflow/prompts/demonstration-format.md
[ ] internal/engine/workflow/prompts/discuss-followup.md
[ ] internal/engine/workflow/prompts/discuss-questions.md
[ ] internal/engine/workflow/prompts/execute-task.md
[ ] internal/engine/workflow/prompts/intent-classify.md
[ ] internal/engine/workflow/prompts/loader.go
[ ] internal/engine/workflow/prompts/models/anthropic.txt
[ ] internal/engine/workflow/prompts/models/default.txt
[ ] internal/engine/workflow/prompts/models/google.txt
[ ] internal/engine/workflow/prompts/models/openai.txt
[ ] internal/engine/workflow/prompts/plan-check.md
[ ] internal/engine/workflow/prompts/plan-format.md
[ ] internal/engine/workflow/prompts/plan-outline.md
[ ] internal/engine/workflow/prompts/plan-revise.md
[ ] internal/engine/workflow/prompts/research.md
[ ] internal/engine/workflow/prompts/self-heal.md
[ ] internal/engine/workflow/prompts/tool-use.md
[ ] internal/engine/workflow/prompts/website-build.md
[ ] internal/engine/workflow/research.go
[ ] internal/engine/workflow/research_test.go
[ ] internal/engine/workflow/retry.go
[ ] internal/engine/workflow/retry_test.go
[ ] internal/engine/workflow/run_phase_test.go
[x] internal/engine/workflow/runtime.go
[ ] internal/engine/workflow/runtime_extra_test.go
[ ] internal/engine/workflow/runtime_proc_unix.go
[ ] internal/engine/workflow/runtime_proc_windows.go
[ ] internal/engine/workflow/self_heal_visibility_test.go
[ ] internal/engine/workflow/ship.go
[ ] internal/engine/workflow/ship_lock_unix.go
[ ] internal/engine/workflow/ship_lock_windows.go
[x] internal/engine/workflow/ship_preflight.go
[x] internal/engine/workflow/ship_preflight_test.go
[ ] internal/engine/workflow/ship_test.go
[x] internal/engine/workflow/state_machine.go
[ ] internal/engine/workflow/state_machine_test.go
[ ] internal/engine/workflow/streaming_progress_test.go
[ ] internal/engine/workflow/templates/website-nextjs/.gitignore
[ ] internal/engine/workflow/templates/website-nextjs/app/globals.css
[ ] internal/engine/workflow/templates/website-nextjs/app/layout.tsx
[ ] internal/engine/workflow/templates/website-nextjs/app/not-found.tsx
[ ] internal/engine/workflow/templates/website-nextjs/app/page.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components.json
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/accordion.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/alert-dialog.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/alert.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/aspect-ratio.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/avatar.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/badge.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/breadcrumb.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/button.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/card.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/checkbox.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/dialog.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/dropdown-menu.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/hover-card.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/input.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/label.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/navigation-menu.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/pagination.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/popover.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/progress.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/radio-group.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/scroll-area.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/select.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/separator.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/sheet.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/skeleton.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/slider.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/sonner.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/switch.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/table.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/tabs.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/textarea.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/toggle-group.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/toggle.tsx
[ ] internal/engine/workflow/templates/website-nextjs/components/ui/tooltip.tsx
[ ] internal/engine/workflow/templates/website-nextjs/hooks/use-media-query.ts
[ ] internal/engine/workflow/templates/website-nextjs/hooks/use-scroll.ts
[ ] internal/engine/workflow/templates/website-nextjs/lib/constants.ts
[ ] internal/engine/workflow/templates/website-nextjs/lib/utils.ts
[ ] internal/engine/workflow/templates/website-nextjs/next.config.ts
[ ] internal/engine/workflow/templates/website-nextjs/package.json
[ ] internal/engine/workflow/templates/website-nextjs/postcss.config.mjs
[ ] internal/engine/workflow/templates/website-nextjs/tailwind.config.ts
[ ] internal/engine/workflow/templates/website-nextjs/tsconfig.json
[ ] internal/engine/workflow/thinking_indicator_test.go
[ ] internal/engine/workflow/verify.go
[ ] internal/engine/workflow/verify_report.go
[ ] internal/engine/workflow/verify_report_test.go
[ ] internal/engine/workflow/verify_test.go
[ ] internal/engine/workflow/website_build_test.go
[ ] internal/engine/workflow/workflow_cache.go
[ ] internal/engine/workflow/workflow_cache_test.go
[ ] internal/engine/workflow/workflow_test.go
[ ] internal/infrastructure/fileutil/atomic.go
[ ] internal/infrastructure/fileutil/atomic_test.go
[ ] internal/infrastructure/fileutil/extra_test.go
[ ] internal/infrastructure/fileutil/lock_test.go
[ ] internal/infrastructure/fileutil/lock_unix.go
[ ] internal/infrastructure/fileutil/lock_windows.go
[ ] internal/infrastructure/retry/policy.go
[ ] internal/infrastructure/retry/policy_test.go
[ ] internal/integrations/arbitrage/arbitrage.go
[ ] internal/integrations/arbitrage/arbitrage_test.go
[ ] internal/integrations/arbitrage/doc.go
[ ] internal/integrations/autodream/autodream.go
[ ] internal/integrations/autodream/autodream_test.go
[ ] internal/integrations/autodream/doc.go
[ ] internal/integrations/autodream/extra_test.go
[ ] internal/integrations/codeintel/bench_test.go
[ ] internal/integrations/codeintel/cache.go
[ ] internal/integrations/codeintel/codeintel.go
[ ] internal/integrations/codeintel/codeintel_test.go
[ ] internal/integrations/codeintel/graph.go
[ ] internal/integrations/codeintel/graph_test.go
[ ] internal/integrations/codeintel/index.go
[ ] internal/integrations/codeintel/index_test.go
[ ] internal/integrations/codeintel/parser.go
[ ] internal/integrations/codeintel/parser_test.go
[ ] internal/integrations/codeintel/relevance.go
[ ] internal/integrations/codeintel/relevance_test.go
[ ] internal/integrations/codeintel/trie.go
[ ] internal/integrations/context/context_test.go
[ ] internal/integrations/context/registry.go
[ ] internal/integrations/context/source.go
[ ] internal/integrations/context/sources.go
[ ] internal/integrations/git/git.go
[ ] internal/integrations/git/git_extra_test.go
[ ] internal/integrations/git/git_test.go
[ ] internal/integrations/history/history.go
[ ] internal/integrations/history/history_test.go
[ ] internal/integrations/keychain/errors.go
[ ] internal/integrations/keychain/extra_test.go
[ ] internal/integrations/keychain/keychain.go
[ ] internal/integrations/keychain/keychain_darwin.go
[ ] internal/integrations/keychain/keychain_linux.go
[ ] internal/integrations/keychain/keychain_test.go
[ ] internal/integrations/keychain/keychain_windows.go
[ ] internal/integrations/ledger/doc.go
[ ] internal/integrations/ledger/extra_test.go
[ ] internal/integrations/ledger/ledger.go
[ ] internal/integrations/ledger/ledger_test.go
[ ] internal/integrations/log/extra_test.go
[ ] internal/integrations/log/log.go
[ ] internal/integrations/log/log_test.go
[ ] internal/integrations/metrics/collector.go
[ ] internal/integrations/metrics/metrics_test.go
[ ] internal/integrations/metrics/types.go
[ ] internal/integrations/provider/base_client.go
[ ] internal/integrations/provider/base_client_test.go
[ ] internal/integrations/provider/cache.go
[ ] internal/integrations/provider/cache_refresh_test.go
[ ] internal/integrations/provider/cache_test.go
[ ] internal/integrations/provider/capabilities.go
[ ] internal/integrations/provider/capabilities_test.go
[ ] internal/integrations/provider/common.go
[ ] internal/integrations/provider/common_test.go
[ ] internal/integrations/provider/extra_test.go
[ ] internal/integrations/provider/fallback.go
[ ] internal/integrations/provider/fallback_test.go
[x] internal/integrations/provider/interface.go
[ ] internal/integrations/provider/interface_test.go
[ ] internal/integrations/provider/model_metadata.go
[ ] internal/integrations/provider/model_metadata_test.go
[ ] internal/integrations/provider/nvidia/client.go
[ ] internal/integrations/provider/nvidia/integration_test.go
[ ] internal/integrations/provider/openrouter/client.go
[ ] internal/integrations/provider/openrouter/client_test.go
[ ] internal/integrations/provider/openrouter/extra_test.go
[ ] internal/integrations/provider/openrouter/integration_test.go
[ ] internal/integrations/provider/reasoning.go
[ ] internal/integrations/provider/reasoning_test.go
[ ] internal/integrations/provider/registry.go
[ ] internal/integrations/provider/registry_test.go
[ ] internal/integrations/provider/resilience_test.go
[x] internal/integrations/provider/sse.go
[ ] internal/integrations/provider/sse_test.go
[ ] internal/integrations/provider/zen/client.go
[ ] internal/integrations/provider/zen/client_test.go
[ ] internal/integrations/provider/zen/integration_test.go
[ ] internal/integrations/provider/zen/retry_test.go
[ ] internal/integrations/shell/shell_test.go
[x] internal/integrations/shell/shell_unix.go
[ ] internal/integrations/shell/shell_windows.go
[ ] internal/integrations/skills/discovery.go
[ ] internal/integrations/skills/loader.go
[ ] internal/integrations/skills/skill.go
[ ] internal/integrations/skills/skills_test.go
[ ] internal/tests/tools/exec/concurrency_test.go
[ ] internal/tests/tui/commands/search_fixes_test.go
[ ] internal/tests/tui/components/coverage_boost_test.go
[ ] internal/tests/tui/components/empty_state_templates_test.go
[ ] internal/tests/tui/components/pure_test.go
[ ] internal/tests/tui/components/sparkline_test.go
[ ] internal/tests/tui/components/starfield_test.go
[ ] internal/tests/tui/layout/m1_regression_test.go
[ ] internal/tests/tui/layout/page_test.go
[ ] internal/tests/tui/layout/responsive_test.go
[ ] internal/tests/tui/layout/stack_test.go
[ ] internal/tests/tui/tuitypes/tuitypes_test.go
[ ] internal/testutil/ci/ci.go
[ ] internal/testutil/ci/ci_test.go
[ ] internal/testutil/testtimeout/testtimeout.go
[ ] internal/testutil/testtimeout/testtimeout_test.go
[ ] internal/tools/agent_test.go
[ ] internal/tools/ai/agent.go
[ ] internal/tools/ai/question.go
[ ] internal/tools/ai/question_test.go
[ ] internal/tools/bash_kill_test.go
[ ] internal/tools/bash_sandbox_test.go
[ ] internal/tools/bash_security_test.go
[ ] internal/tools/bash_test.go
[ ] internal/tools/codeanalysis/codecomplexity.go
[ ] internal/tools/codeanalysis/codecomplexity_benchmark_test.go
[ ] internal/tools/codeanalysis/codecomplexity_test.go
[ ] internal/tools/codeanalysis/codemap.go
[ ] internal/tools/codeanalysis/codemap_test.go
[ ] internal/tools/constants.go
[ ] internal/tools/defaults.go
[ ] internal/tools/defaults_test.go
[x] internal/tools/dispatcher.go
[ ] internal/tools/dispatcher_test.go
[ ] internal/tools/edit_benchmark_test.go
[ ] internal/tools/edit_test.go
[x] internal/tools/exec/bash.go
[ ] internal/tools/exec/bash_sandbox_darwin.go
[x] internal/tools/exec/bash_sandbox_linux.go
[ ] internal/tools/exec/bash_sandbox_other.go
[ ] internal/tools/exec/bash_sandbox_windows.go
[x] internal/tools/exec/bash_test.go
[x] internal/tools/exec/bash_unix.go
[ ] internal/tools/exec/bash_windows.go
[ ] internal/tools/exec/concurrency.go
[ ] internal/tools/exec/concurrency_test.go
[x] internal/tools/exec/devserver.go
[ ] internal/tools/exec/devserver_test.go
[ ] internal/tools/exec/helpers_test.go
[ ] internal/tools/exec/output_store.go
[ ] internal/tools/exec/output_store_test.go
[ ] internal/tools/exec/prockill_unix.go
[ ] internal/tools/exec/prockill_windows.go
[x] internal/tools/extra_test.go
[ ] internal/tools/filedelete_test.go
[ ] internal/tools/filelist_test.go
[ ] internal/tools/filemove_test.go
[ ] internal/tools/fileops/backup.go
[ ] internal/tools/fileops/backup_test.go
[ ] internal/tools/fileops/constants.go
[ ] internal/tools/fileops/edit.go
[ ] internal/tools/fileops/edit_test.go
[ ] internal/tools/fileops/filedelete.go
[ ] internal/tools/fileops/filedelete_test.go
[ ] internal/tools/fileops/filelist.go
[ ] internal/tools/fileops/filelist_test.go
[ ] internal/tools/fileops/filemove.go
[ ] internal/tools/fileops/filemove_test.go
[ ] internal/tools/fileops/fileread.go
[ ] internal/tools/fileops/fileread_test.go
[ ] internal/tools/fileops/filewrite.go
[ ] internal/tools/fileops/filewrite_test.go
[ ] internal/tools/fileops/helpers.go
[ ] internal/tools/fileops/pathhelpers.go
[ ] internal/tools/fileops/pathhelpers_test.go
[ ] internal/tools/fileops/test_helpers.go
[ ] internal/tools/fileread_test.go
[ ] internal/tools/filewrite_test.go
[ ] internal/tools/git/git.go
[ ] internal/tools/git/git_test.go
[ ] internal/tools/glob_test.go
[ ] internal/tools/grep_skip_comments_test.go
[ ] internal/tools/grep_test.go
[ ] internal/tools/grep_truncation_test.go
[ ] internal/tools/interface.go
[ ] internal/tools/network/httpcheck.go
[ ] internal/tools/network/httpcheck_test.go
[ ] internal/tools/permission_timeout_test.go
[ ] internal/tools/permissions.go
[ ] internal/tools/permissions_extract_test.go
[ ] internal/tools/permissions_test.go
[ ] internal/tools/persistent_permissions.go
[ ] internal/tools/persistent_permissions_test.go
[ ] internal/tools/question_test.go
[ ] internal/tools/search/dns_cache.go
[ ] internal/tools/search/dns_cache_test.go
[ ] internal/tools/search/dns_cache_windows.go
[ ] internal/tools/search/glob.go
[ ] internal/tools/search/glob_test.go
[ ] internal/tools/search/grep.go
[ ] internal/tools/search/ip_filter.go
[ ] internal/tools/search/ip_filter_test.go
[ ] internal/tools/search/metrics.go
[ ] internal/tools/search/test_helpers.go
[ ] internal/tools/search/webfetch.go
[ ] internal/tools/search/webfetch_html.go
[ ] internal/tools/search/webfetch_test.go
[ ] internal/tools/search/websearch.go
[ ] internal/tools/search/websearch_test.go
[ ] internal/tools/subagent/events.go
[ ] internal/tools/subagent/events_test.go
[ ] internal/tools/subagent/extra_test.go
[ ] internal/tools/subagent/loop.go
[ ] internal/tools/subagent/loop_parse.go
[ ] internal/tools/subagent/loop_parse_test.go
[ ] internal/tools/subagent/manager.go
[ ] internal/tools/subagent/manager_test.go
[ ] internal/tools/subagent/profile.go
[ ] internal/tools/subagent/profile_filter.go
[ ] internal/tools/subagent/profile_test.go
[ ] internal/tools/subagent/worktree.go
[ ] internal/tools/subagent/worktree_test.go
[ ] internal/tools/testutil_test.go
[ ] internal/tools/todo/todo.go
[ ] internal/tools/todo/todo_test.go
[ ] internal/tools/todo/todoread.go
[ ] internal/tools/tooldefs.go
[ ] internal/tools/tooldefs_test.go
[ ] internal/tools/toolinput_test.go
[ ] internal/tools/tools_reexport.go
[ ] internal/tools/tools_test.go
[ ] internal/tools/webfetch_helpers_test.go
[ ] internal/tools/webfetch_security_test.go
[ ] internal/tools/webfetch_test.go
[ ] internal/tools/websearch_test.go
[ ] internal/ui/tui/README.md
[ ] internal/ui/tui/agent_loop_extra_test.go
[ ] internal/ui/tui/app.go
[ ] internal/ui/tui/app_agent.go
[ ] internal/ui/tui/app_agent_test.go
[ ] internal/ui/tui/app_channel.go
[ ] internal/ui/tui/app_channel_bench_test.go
[ ] internal/ui/tui/app_channel_test.go
[ ] internal/ui/tui/app_handlers.go
[ ] internal/ui/tui/app_handlers_config.go
[ ] internal/ui/tui/app_handlers_misc.go
[ ] internal/ui/tui/app_handlers_provider.go
[ ] internal/ui/tui/app_handlers_tick.go
[ ] internal/ui/tui/app_handlers_workflow.go
[ ] internal/ui/tui/app_helpers.go
[ ] internal/ui/tui/app_helpers_test.go
[ ] internal/ui/tui/app_input_action.go
[ ] internal/ui/tui/app_input_resize.go
[ ] internal/ui/tui/app_input_route.go
[ ] internal/ui/tui/app_input_test.go
[ ] internal/ui/tui/app_input_theme.go
[ ] internal/ui/tui/app_nav.go
[ ] internal/ui/tui/app_nav_test.go
[ ] internal/ui/tui/app_regression_test.go
[ ] internal/ui/tui/app_routing.go
[ ] internal/ui/tui/app_routing_test.go
[ ] internal/ui/tui/app_screens.go
[ ] internal/ui/tui/app_screens_test.go
[ ] internal/ui/tui/app_session.go
[ ] internal/ui/tui/app_session_test.go
[ ] internal/ui/tui/app_state.go
[ ] internal/ui/tui/app_update.go
[ ] internal/ui/tui/app_update_commands.go
[ ] internal/ui/tui/app_update_extra_test.go
[ ] internal/ui/tui/app_update_phase.go
[ ] internal/ui/tui/app_view.go
[ ] internal/ui/tui/app_view_extra_test.go
[ ] internal/ui/tui/bisect_model.go
[ ] internal/ui/tui/chathistory_model.go
[ ] internal/ui/tui/chathistory_view.go
[ ] internal/ui/tui/cmdpalette.go
[ ] internal/ui/tui/commandpalette_model.go
[ ] internal/ui/tui/commands.go
[ ] internal/ui/tui/commands/commands.go
[ ] internal/ui/tui/commands/commands_agent.go
[ ] internal/ui/tui/commands/commands_ai.go
[ ] internal/ui/tui/commands/commands_all_test.go
[ ] internal/ui/tui/commands/commands_analysis.go
[ ] internal/ui/tui/commands/commands_analysis_test.go
[ ] internal/ui/tui/commands/commands_config.go
[ ] internal/ui/tui/commands/commands_config_diskusage_unix.go
[ ] internal/ui/tui/commands/commands_config_diskusage_windows.go
[ ] internal/ui/tui/commands/commands_core.go
[ ] internal/ui/tui/commands/commands_extra_test.go
[ ] internal/ui/tui/commands/commands_git.go
[ ] internal/ui/tui/commands/commands_session.go
[ ] internal/ui/tui/commands/commands_workflow.go
[ ] internal/ui/tui/commands/coverage_boost_test.go
[ ] internal/ui/tui/components/badge.go
[ ] internal/ui/tui/components/base.go
[ ] internal/ui/tui/components/bordered_input.go
[ ] internal/ui/tui/components/breadcrumb.go
[ ] internal/ui/tui/components/card.go
[ ] internal/ui/tui/components/confirm.go
[ ] internal/ui/tui/components/cursor_indicator.go
[ ] internal/ui/tui/components/divider.go
[ ] internal/ui/tui/components/dropdown.go
[ ] internal/ui/tui/components/empty_state.go
[ ] internal/ui/tui/components/empty_state_templates.go
[ ] internal/ui/tui/components/error_banner.go
[ ] internal/ui/tui/components/extra_test.go
[ ] internal/ui/tui/components/filetree.go
[ ] internal/ui/tui/components/filterchips.go
[ ] internal/ui/tui/components/glamour_bench_test.go
[ ] internal/ui/tui/components/glamour_cache.go
[ ] internal/ui/tui/components/glamour_cache_test.go
[ ] internal/ui/tui/components/hint_bar.go
[ ] internal/ui/tui/components/loading_indicator.go
[ ] internal/ui/tui/components/logo.go
[ ] internal/ui/tui/components/markdown_lite.go
[ ] internal/ui/tui/components/message.go
[ ] internal/ui/tui/components/message_test.go
[ ] internal/ui/tui/components/metriccard.go
[ ] internal/ui/tui/components/notification_list.go
[ ] internal/ui/tui/components/permission.go
[ ] internal/ui/tui/components/permission_desc.go
[ ] internal/ui/tui/components/permission_desc_test.go
[ ] internal/ui/tui/components/permission_test.go
[ ] internal/ui/tui/components/progress.go
[ ] internal/ui/tui/components/question.go
[ ] internal/ui/tui/components/screen_title.go
[ ] internal/ui/tui/components/scroll_state.go
[ ] internal/ui/tui/components/sparkline.go
[ ] internal/ui/tui/components/spinner.go
[ ] internal/ui/tui/components/starfield.go
[ ] internal/ui/tui/components/statrow.go
[ ] internal/ui/tui/components/syntax.go
[ ] internal/ui/tui/components/tabbar.go
[ ] internal/ui/tui/components/taskgraph.go
[ ] internal/ui/tui/components/thinking.go
[ ] internal/ui/tui/components/thinking_test.go
[ ] internal/ui/tui/components/timeline.go
[ ] internal/ui/tui/components/toolcard.go
[ ] internal/ui/tui/components/toolcard_test.go
[ ] internal/ui/tui/components/toolrenderers.go
[ ] internal/ui/tui/components/tour.go
[ ] internal/ui/tui/components/truncate.go
[ ] internal/ui/tui/components/virtual_viewport.go
[ ] internal/ui/tui/components/workflow_phasebar.go
[ ] internal/ui/tui/config_model.go
[ ] internal/ui/tui/config_model_advanced.go
[ ] internal/ui/tui/config_model_extra_test.go
[ ] internal/ui/tui/config_model_general.go
[ ] internal/ui/tui/config_model_provider.go
[ ] internal/ui/tui/config_model_sections.go
[ ] internal/ui/tui/config_model_tui.go
[ ] internal/ui/tui/config_model_view.go
[ ] internal/ui/tui/confirmquit_model.go
[ ] internal/ui/tui/constants.go
[ ] internal/ui/tui/dashboard_model.go
[ ] internal/ui/tui/decision_screen.go
[ ] internal/ui/tui/decision_screen_test.go
[ ] internal/ui/tui/diff_model.go
[ ] internal/ui/tui/diff_view.go
[ ] internal/ui/tui/discuss_model.go
[ ] internal/ui/tui/emitter_stress_test.go
[ ] internal/ui/tui/execute_model.go
[ ] internal/ui/tui/execute_view.go
[ ] internal/ui/tui/fileexplorer_model.go
[ ] internal/ui/tui/filewatcher.go
[ ] internal/ui/tui/firstrun_extra_test.go
[ ] internal/ui/tui/firstrun_model.go
[ ] internal/ui/tui/firstrun_model_test.go
[ ] internal/ui/tui/firstrun_view.go
[ ] internal/ui/tui/firstrun_wizard.go
[ ] internal/ui/tui/ghostoutput_model.go
[ ] internal/ui/tui/ghostpicker_model.go
[ ] internal/ui/tui/goalinput_model.go
[ ] internal/ui/tui/handler_config.go
[ ] internal/ui/tui/handler_config_test.go
[ ] internal/ui/tui/handler_modal.go
[ ] internal/ui/tui/handler_modal_test.go
[ ] internal/ui/tui/handler_navigation.go
[ ] internal/ui/tui/handler_navigation_test.go
[ ] internal/ui/tui/handler_runtime.go
[ ] internal/ui/tui/handler_runtime_test.go
[ ] internal/ui/tui/handler_sidebar.go
[ ] internal/ui/tui/handler_sidebar_test.go
[ ] internal/ui/tui/handler_stream.go
[ ] internal/ui/tui/handler_stream_test.go
[ ] internal/ui/tui/handler_tool.go
[ ] internal/ui/tui/handler_tool_test.go
[ ] internal/ui/tui/handler_workflow.go
[ ] internal/ui/tui/handler_workflow_test.go
[ ] internal/ui/tui/help_model.go
[ ] internal/ui/tui/helpers_extra_test.go
[ ] internal/ui/tui/helpers_file.go
[ ] internal/ui/tui/helpers_string.go
[ ] internal/ui/tui/helpers_ui.go
[ ] internal/ui/tui/home_model.go
[ ] internal/ui/tui/home_model_test.go
[ ] internal/ui/tui/home_view.go
[ ] internal/ui/tui/keybindings.go
[ ] internal/ui/tui/keybindings_screens.go
[ ] internal/ui/tui/layout/box.go
[ ] internal/ui/tui/layout/constraints.go
[ ] internal/ui/tui/layout/coverage_boost_test.go
[ ] internal/ui/tui/layout/extra_test.go
[ ] internal/ui/tui/layout/layout_extra_test.go
[ ] internal/ui/tui/layout/minscreen.go
[ ] internal/ui/tui/layout/page.go
[ ] internal/ui/tui/layout/responsive.go
[ ] internal/ui/tui/layout/solver.go
[ ] internal/ui/tui/layout/stack.go
[ ] internal/ui/tui/ledger_model.go
[ ] internal/ui/tui/mention.go
[ ] internal/ui/tui/mention_view.go
[ ] internal/ui/tui/metrics_model.go
[ ] internal/ui/tui/modelselector_list.go
[ ] internal/ui/tui/modelselector_model.go
[ ] internal/ui/tui/modelselector_view.go
[ ] internal/ui/tui/narrative.go
[ ] internal/ui/tui/narrative_emitter.go
[ ] internal/ui/tui/narrative_handler.go
[ ] internal/ui/tui/narrative_test.go
[ ] internal/ui/tui/notification_model.go
[ ] internal/ui/tui/phase_transition_model.go
[ ] internal/ui/tui/phasemodelpicker.go
[ ] internal/ui/tui/phasemodelpicker_extra_test.go
[ ] internal/ui/tui/phasemodelpicker_view.go
[ ] internal/ui/tui/plan_extra_test.go
[ ] internal/ui/tui/plan_model.go
[ ] internal/ui/tui/plan_refine.go
[ ] internal/ui/tui/plan_view.go
[ ] internal/ui/tui/provider_registration.go
[ ] internal/ui/tui/pure_funcs_extra_test.go
[ ] internal/ui/tui/repl.go
[ ] internal/ui/tui/repl_clipboard.go
[ ] internal/ui/tui/repl_commands.go
[ ] internal/ui/tui/repl_footer.go
[ ] internal/ui/tui/repl_model.go
[ ] internal/ui/tui/repl_model_test.go
[ ] internal/ui/tui/repl_mouse.go
[ ] internal/ui/tui/repl_quickactions.go
[ ] internal/ui/tui/repl_scrollbar.go
[ ] internal/ui/tui/repl_search.go
[ ] internal/ui/tui/repl_state.go
[ ] internal/ui/tui/repl_state_extra_test.go
[ ] internal/ui/tui/repl_stream.go
[ ] internal/ui/tui/repl_thinking.go
[ ] internal/ui/tui/repl_view.go
[ ] internal/ui/tui/repl_welcome.go
[ ] internal/ui/tui/resume_extra_test.go
[ ] internal/ui/tui/resume_fixes_test.go
[ ] internal/ui/tui/resume_model.go
[ ] internal/ui/tui/resume_view.go
[ ] internal/ui/tui/rollback_extra_test.go
[ ] internal/ui/tui/rollback_model.go
[ ] internal/ui/tui/router.go
[ ] internal/ui/tui/router_test.go
[ ] internal/ui/tui/runtime_model.go
[ ] internal/ui/tui/runtime_view.go
[ ] internal/ui/tui/screen.go
[ ] internal/ui/tui/sessiondetail_model.go
[ ] internal/ui/tui/settings_extra_test.go
[ ] internal/ui/tui/settings_model.go
[ ] internal/ui/tui/settings_model_extra_test.go
[ ] internal/ui/tui/settings_tabs.go
[ ] internal/ui/tui/settings_view.go
[ ] internal/ui/tui/ship_model.go
[ ] internal/ui/tui/sidebar_extra_test.go
[ ] internal/ui/tui/sidebar_fixes_test.go
[ ] internal/ui/tui/sidebar_model.go
[ ] internal/ui/tui/sidebar_render.go
[ ] internal/ui/tui/stream_chunk_test.go
[ ] internal/ui/tui/streaming.go
[ ] internal/ui/tui/streaming/agent_loop.go
[ ] internal/ui/tui/streaming/agent_loop_extra_test.go
[ ] internal/ui/tui/streaming/agent_loop_test.go
[ ] internal/ui/tui/streaming/coverage_boost_test.go
[ ] internal/ui/tui/streaming/streaming.go
[ ] internal/ui/tui/streaming/streaming_extra_test.go
[ ] internal/ui/tui/subagent_bridge.go
[ ] internal/ui/tui/subagents_model.go
[ ] internal/ui/tui/test_helpers_test.go
[ ] internal/ui/tui/theme/bench_alloc_test.go
[ ] internal/ui/tui/theme/borders.go
[ ] internal/ui/tui/theme/cache.go
[ ] internal/ui/tui/theme/cache_gen.go
[ ] internal/ui/tui/theme/colors.go
[ ] internal/ui/tui/theme/coverage_boost_test.go
[ ] internal/ui/tui/theme/extra_test.go
[ ] internal/ui/tui/theme/nocolor_test.go
[ ] internal/ui/tui/theme/registry.go
[ ] internal/ui/tui/theme/registry_test.go
[ ] internal/ui/tui/theme/shadow.go
[ ] internal/ui/tui/theme/tabs.go
[ ] internal/ui/tui/theme/theme.go
[ ] internal/ui/tui/theme/theme_test.go
[ ] internal/ui/tui/theme/tokens.go
[ ] internal/ui/tui/theme/tokens_elevation.go
[ ] internal/ui/tui/theme/tokens_motion.go
[ ] internal/ui/tui/theme/tokens_semantic.go
[ ] internal/ui/tui/theme/tokens_spacing.go
[ ] internal/ui/tui/theme/tokens_typography.go
[ ] internal/ui/tui/theme/unicode.go
[ ] internal/ui/tui/toast.go
[ ] internal/ui/tui/tooldetail_model.go
[ ] internal/ui/tui/transition_extra_test.go
[ ] internal/ui/tui/transition_helpers.go
[ ] internal/ui/tui/transition_phase.go
[ ] internal/ui/tui/transition_screen.go
[ ] internal/ui/tui/transition_state.go
[ ] internal/ui/tui/truncate.go
[ ] internal/ui/tui/tui_harness_test.go
[ ] internal/ui/tui/tuitypes/coverage_boost_test.go
[ ] internal/ui/tui/tuitypes/tuitypes.go
[ ] internal/ui/tui/types.go
[ ] internal/ui/tui/verify_extra_test.go
[ ] internal/ui/tui/verify_model.go
[ ] m31a.json
[ ] scripts/validate-release.sh
[ ] scripts/verify_v1.sh
[ ] tests/e2e/e2e_test.go
[ ] tests/testutil/e2e/README.md
[ ] tests/testutil/envtest.go
[ ] tests/testutil/integration/README.md
[ ] tests/testutil/integration/tool_integration_test.go
[ ] tests/testutil/mocks/dispatcher.go
[ ] tests/testutil/mocks/provider.go
[ ] tests/testutil/mocks/tool.go


## Findings

### [HIGH] Environment Variable Scrubbing Bypass
- **File:** `internal/tools/exec/bash_sandbox_linux.go:181-256`
- **Category:** Security-relevant logic gaps
- **What's wrong:** The prefix-based env scrubbing logic checks for sensitive keywords (like `SECRET`, `PASSWORD`) using `hasPrefixFold` and `hasSuffixFold` but does *not* check for the pattern occurring in the middle of the string. The comment claims it does "prefix/suffix/contains matches", but the code doesn't check contains.
- **Evidence:** `if hasPrefixFold(key, pattern) || hasSuffixFold(key, pattern) { sensitive = true }`
- **Trigger / reachability:** Running a subagent or sandbox command when the host has an environment variable like `DB_PASSWORD_PROD` or `MY_SECRET_KEY`.
- **Impact:** Sensitive environment variables that match the pattern in the middle of the name bypass the sandbox and are leaked to the untrusted command environment.
- **Suggested fix:** Change the condition to include `strings.Contains(strings.ToLower(key), strings.ToLower(pattern))` or similar case-insensitive substring search.
- **Confidence:** High

### [HIGH] DevServer startServer Race Condition Allows Port Hijacking
- **File:** `internal/tools/exec/devserver.go:574-586`
- **Category:** Concurrency bugs
- **What's wrong:** The `waitForPort` function polls until it can connect to the specified port. However, `startServer` relies entirely on `waitForPort` to confirm the server started successfully. If the server process crashes immediately after starting, but another process is listening on the same port, `waitForPort` returns true and `startServer` reports success.
- **Evidence:** `ready := waitForPort(port, 30*time.Second); if !ready { ... }` checks port connectivity but does not check `entry.exited` which is updated asynchronously by `monitorCrash`.
- **Trigger / reachability:** Running the `devserver` tool with a port that is already in use by another process. The server will fail to bind and exit immediately, but `startServer` will incorrectly return success.
- **Impact:** Tool calling LLM believes the server started successfully but it actually failed. The output logs might be from a different process, causing confusion.
- **Suggested fix:** After `waitForPort` returns, or during the loop, explicitly check `d.mu.Lock(); exited := entry.exited; d.mu.Unlock()` to ensure the managed process hasn't crashed.
- **Confidence:** High

### [MEDIUM] API Key Loaded From Env Without Validation
- **File:** `internal/core/config/loader.go:568-600`
- **Category:** Error handling bugs
- **What's wrong:** API keys read from environment variables (`M31A_OPENROUTER_API_KEY`, etc.) are directly assigned to the configuration without being trimmed or validated for correct formatting.
- **Evidence:** `if key := os.Getenv("M31A_OPENROUTER_API_KEY"); key != "" { c.Provider.OpenRouter.APIKey = key }`
- **Trigger / reachability:** An environment variable set with accidental whitespace, like `export M31A_OPENROUTER_API_KEY="sk-...
"`.
- **Impact:** Malformed keys with trailing newlines will be injected into HTTP headers for provider requests, leading to silent auth failures or potential HTTP header injection.
- **Suggested fix:** Use `strings.TrimSpace(key)` before assignment.
- **Confidence:** High

### [HIGH] Shell Command Validation Incomplete on Command Substitution
- **File:** `internal/tools/exec/bash.go:468`
- **Category:** Security-relevant logic gaps
- **What's wrong:** `checkCommandChaining` incorrectly uses `strings.HasPrefix` and `strings.HasSuffix` on individual semicolon-separated segments to find command substitutions `$(...)`. This misses any command substitution embedded inside a longer string, e.g., `echo $(rm -rf /)` because it doesn't start with `$(`.
- **Evidence:** `if strings.HasPrefix(seg, "$(") && strings.HasSuffix(seg, ")") {`
- **Trigger / reachability:** Sending the command `echo $(rm -rf /)` through the bash tool. The validation will ignore the inner command because `seg` starts with `echo`, not `$(`.
- **Impact:** A malicious LLM or user input can completely bypass the command validation by embedding dangerous commands inside `$()` that aren't at the start of the string.
- **Suggested fix:** Parse command substitution using a regex or more robust parsing rather than just `HasPrefix` and `HasSuffix`.
- **Confidence:** High

### [HIGH] Runtime Servers Unscrubbed Environment Injection
- **File:** `internal/engine/workflow/runtime.go:226,245`
- **Category:** Security-relevant logic gaps
- **What's wrong:** `startGoServer` and `startRustServer` append to `os.Environ()` directly and pass it to `exec.CommandContext` without scrubbing. The `Bash` tool scrubs the environment using `ScrubEnvironment` to prevent sensitive API keys from leaking to untrusted processes, but these runtime dev servers run the user's code with the full M31A process environment.
- **Evidence:** `cmd.Env = append(os.Environ(), fmt.Sprintf("PORT=%d", port))`
- **Trigger / reachability:** When the LLM transitions to the Runtime phase and the engine detects a Go or Rust server, it starts it. If the user's Go/Rust code prints its environment or if it's malicious, it gains access to the host's `os.Environ()`, including M31A's OpenRouter/Zen API keys.
- **Impact:** Sandbox escape / secret leakage. Untrusted code execution in the runtime phase can steal API keys.
- **Suggested fix:** Apply `exec_tool.ScrubEnvironment` (or a similar scrubbing mechanism) to the `cmd.Env` for runtime servers as well.
- **Confidence:** High

### [HIGH] Execute Task Pause/Resume Racing and Missed Wakeups
- **File:** `internal/engine/workflow/execute.go:100-137`
- **Category:** Concurrency bugs
- **What's wrong:** `execFn` checks `e.pauseCh != nil` without blocking if it isn't paused. But if the engine is paused *while* a task is currently executing inside `executeTaskWithTools`, the task does not stop, and the pause isn't respected until the next task starts. Moreover, if `e.waitForPause(ctx)` returns on resume at line 100, the task starts. If the engine is paused right after line 100 before `execFn` runs, `execFn` runs and blocks on `consumeSkipOrCancel(ctx)`. `consumeSkipOrCancel(ctx)` blocks reading from channels like `skipTaskCh` or `resumeCh`. However, `executeTaskWithTools` itself isn't aware of the pause.
- **Evidence:** `PauseExecution` only creates channels (`pauseCh`, `resumeCh`). `execute.go` checks `paused` before `executeTaskWithTools`. If it's not paused then, it enters `executeTaskWithTools` which runs LLM calls and tool executions synchronously. Pausing the UI has no effect on the current long-running task.
- **Trigger / reachability:** The user presses `p` (pause) in the TUI while a long-running execution task is running (e.g. executing a Bash tool).
- **Impact:** The engine continues executing the task, spending LLM tokens and making system changes, despite the UI indicating that execution is paused.
- **Suggested fix:** Pass a context to `executeTaskWithTools` that is cancelled or paused by `PauseExecution`, or have the tool-calling loop inside `executeTaskWithTools` periodically check `waitForPause(ctx)`.
- **Confidence:** High

### [LOW] Test suite failure `TestLoadGitignoreCached_CacheInvalidation`
- **File:** `internal/tools/extra_test.go:516`
- **Category:** Test bugs that hide real bugs
- **What's wrong:** The test asserts that the cache is invalidated after writing a `.gitignore` and modifying it. However, the file write happens too quickly in succession, and the internal cache validation might use `ModTime` which only has second-level precision on some filesystems, leading to a race condition where the new `ModTime` is exactly the same as the old one. This causes the cache not to invalidate and the test fails.
- **Evidence:** `extra_test.go:516: expected 2 patterns after cache invalidation, got 1`
- **Trigger / reachability:** Running `make test` without artificial sleep between file writes.
- **Impact:** Flaky test suite that fails unpredictably.
- **Suggested fix:** Use a `time.Sleep` of at least 10ms or use a mock clock/filesystem to simulate the time jump.
- **Confidence:** High

## Confirmed / Refuted Known Issues

- `internal/integrations/provider/sse.go:26-32`: Refuted-with-reasoning. The watchdog timer closes the body after 30s of inactivity, which is correct and intended behavior to interrupt a blocked read on `scanner.Scan()`. The watchdog is properly reset on successful reads.
- `internal/tools/dispatcher.go:95-139`: Refuted-with-reasoning. The rate limiter tickers use separate goroutines to refill channel buckets. The `select { case d.rateTokens <- struct{}{}: default: }` pattern correctly drops tokens when the bucket is full. This is standard and correct token bucket behavior; there are no lost wakeups for a full channel, and drift between two independent resource limits is not a logical flaw.
- `internal/engine/workflow/engine.go:183-297` (pause/resume): Confirmed-as-described. `PauseExecution` does not pause in-flight tasks in `executeTaskWithTools`; it only checks for pause *between* tasks. A long-running task will continue to execute and use tokens despite the UI showing a paused state.
- `internal/engine/workflow/runtime.go:226,245`: Confirmed-as-described. `startGoServer` and `startRustServer` pass the entire M31A process environment unmodified, whereas the Bash tool correctly scrubs sensitive API keys.
- `internal/engine/workflow/ship_preflight_test.go:20`: Refuted-with-reasoning. The test creates a mock file explicitly containing `// TODO: fix this later` to verify that the `runShipPreflight` logic correctly detects it. The comment is not a real leftover TODO, but test data.

## Patterns Worth a Follow-Up Pass

- Wait for a follow-up pass to comprehensively review all `defer x.Close()` paths, trace all FSM transitions end-to-end, and check Bubble Tea state mutation invariants across the full TUI package.
