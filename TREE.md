.
├── CHANGELOG.md
├── cmd
│   └── m31a
│       ├── main.go
│       └── usage.go
├── CODEBASE_AUDIT.md
├── CODE_OF_CONDUCT.md
├── CONCERNS.md
├── CONTEXT.md
├── CONTRIBUTING.md
├── DEV.md
├── dist
│   └── m31a-linux-amd64
├── docs
│   ├── ARCHITECTURE.md
│   ├── CONFIG.md
│   ├── INTERFACES.md
│   ├── KEYBINDINGS.md
│   ├── QUICKSTART.md
│   ├── SCREENS.md
│   ├── SLASH_COMMANDS.md
│   ├── TOOLS.md
│   ├── TROUBLESHOOTING.md
│   ├── TYPES.md
│   └── WORKFLOW.md
├── go.mod
├── go.sum
├── install.sh
├── internal
│   ├── codeintel
│   │   ├── codeintel.go
│   │   ├── codeintel_test.go
│   │   ├── graph.go
│   │   ├── graph_test.go
│   │   ├── index.go
│   │   ├── index_test.go
│   │   ├── parser.go
│   │   ├── parser_test.go
│   │   ├── relevance.go
│   │   └── relevance_test.go
│   ├── config
│   │   ├── extra_test.go
│   │   ├── loader.go
│   │   ├── loader_test.go
│   │   ├── project_context.go
│   │   ├── project_context_test.go
│   │   ├── types.go
│   │   └── types_test.go
│   ├── errors
│   │   ├── errors.go
│   │   └── errors_test.go
│   ├── fileutil
│   │   ├── atomic.go
│   │   ├── atomic_test.go
│   │   └── extra_test.go
│   ├── git
│   │   ├── git_extra_test.go
│   │   ├── git.go
│   │   └── git_test.go
│   ├── log
│   │   ├── extra_test.go
│   │   ├── log.go
│   │   └── log_test.go
│   ├── provider
│   │   ├── base_client.go
│   │   ├── base_client_test.go
│   │   ├── cache.go
│   │   ├── cache_refresh_test.go
│   │   ├── cache_test.go
│   │   ├── capabilities.go
│   │   ├── capabilities_test.go
│   │   ├── common.go
│   │   ├── common_test.go
│   │   ├── extra_test.go
│   │   ├── fallback.go
│   │   ├── fallback_test.go
│   │   ├── interface.go
│   │   ├── interface_test.go
│   │   ├── model_metadata.go
│   │   ├── model_metadata_test.go
│   │   ├── nvidia
│   │   │   └── client.go
│   │   ├── openrouter
│   │   │   ├── client.go
│   │   │   ├── client_test.go
│   │   │   └── extra_test.go
│   │   ├── reasoning.go
│   │   ├── reasoning_test.go
│   │   ├── registry.go
│   │   ├── registry_test.go
│   │   ├── resilience_test.go
│   │   ├── sse.go
│   │   ├── sse_test.go
│   │   └── zen
│   │       ├── client.go
│   │       └── client_test.go
│   ├── tokens
│   │   ├── context_warning_test.go
│   │   ├── estimator.go
│   │   └── estimator_test.go
│   ├── tools
│   │   ├── agent.go
│   │   ├── agent_test.go
│   │   ├── backup.go
│   │   ├── bash.go
│   │   ├── bash_kill_test.go
│   │   ├── bash_security_test.go
│   │   ├── bash_test.go
│   │   ├── bash_unix.go
│   │   ├── bash_windows.go
│   │   ├── codecomplexity.go
│   │   ├── codecomplexity_test.go
│   │   ├── codemap.go
│   │   ├── codemap_test.go
│   │   ├── constants.go
│   │   ├── defaults.go
│   │   ├── defaults_test.go
│   │   ├── dispatcher.go
│   │   ├── dispatcher_test.go
│   │   ├── edit.go
│   │   ├── edit_test.go
│   │   ├── extra_test.go
│   │   ├── filedelete.go
│   │   ├── filedelete_test.go
│   │   ├── filelist.go
│   │   ├── filelist_test.go
│   │   ├── filemove.go
│   │   ├── filemove_test.go
│   │   ├── fileread.go
│   │   ├── fileread_test.go
│   │   ├── filewrite.go
│   │   ├── filewrite_test.go
│   │   ├── glob.go
│   │   ├── glob_test.go
│   │   ├── grep.go
│   │   ├── grep_test.go
│   │   ├── grep_truncation_test.go
│   │   ├── interface.go
│   │   ├── metrics.go
│   │   ├── permissions.go
│   │   ├── permissions_test.go
│   │   ├── permission_timeout_test.go
│   │   ├── question.go
│   │   ├── question_test.go
│   │   ├── subagent
│   │   │   ├── events.go
│   │   │   ├── events_test.go
│   │   │   ├── extra_test.go
│   │   │   ├── loop.go
│   │   │   ├── loop_parse.go
│   │   │   ├── loop_parse_test.go
│   │   │   ├── manager.go
│   │   │   ├── manager_test.go
│   │   │   ├── worktree.go
│   │   │   └── worktree_test.go
│   │   ├── todo.go
│   │   ├── todo_test.go
│   │   ├── tooldefs.go
│   │   ├── tooldefs_test.go
│   │   ├── toolinput_test.go
│   │   ├── tools_test.go
│   │   ├── webfetch.go
│   │   ├── webfetch_helpers_test.go
│   │   ├── webfetch_security_test.go
│   │   ├── webfetch_test.go
│   │   ├── websearch.go
│   │   └── websearch_test.go
│   ├── tui
│   │   ├── a11y
│   │   │   └── announce.go
│   │   ├── agent_loop_extra_test.go
│   │   ├── app_channel.go
│   │   ├── app.go
│   │   ├── app_state.go
│   │   ├── app_update_commands.go
│   │   ├── app_update_extra_test.go
│   │   ├── app_update.go
│   │   ├── app_update_phase.go
│   │   ├── app_view_extra_test.go
│   │   ├── app_view.go
│   │   ├── bisect_model.go
│   │   ├── chathistory_model.go
│   │   ├── chathistory_view.go
│   │   ├── cmdpalette.go
│   │   ├── commandpalette_model.go
│   │   ├── commands
│   │   │   ├── commands_agent.go
│   │   │   ├── commands_ai.go
│   │   │   ├── commands_all_test.go
│   │   │   ├── commands_analysis.go
│   │   │   ├── commands_analysis_test.go
│   │   │   ├── commands_config_diskusage_unix.go
│   │   │   ├── commands_config_diskusage_windows.go
│   │   │   ├── commands_config.go
│   │   │   ├── commands_core.go
│   │   │   ├── commands_extra_test.go
│   │   │   ├── commands_git.go
│   │   │   ├── commands.go
│   │   │   ├── commands_session.go
│   │   │   ├── commands_workflow.go
│   │   │   └── search_fixes_test.go
│   │   ├── commands.go
│   │   ├── components
│   │   │   ├── badge.go
│   │   │   ├── base.go
│   │   │   ├── bash_renderer.go
│   │   │   ├── breadcrumb.go
│   │   │   ├── card.go
│   │   │   ├── codeblock.go
│   │   │   ├── confirm.go
│   │   │   ├── datatable.go
│   │   │   ├── divider.go
│   │   │   ├── dropdown.go
│   │   │   ├── empty_state.go
│   │   │   ├── extra_test.go
│   │   │   ├── file_renderers.go
│   │   │   ├── filetree.go
│   │   │   ├── filterchips.go
│   │   │   ├── focus.go
│   │   │   ├── logo.go
│   │   │   ├── message.go
│   │   │   ├── message_test.go
│   │   │   ├── metriccard.go
│   │   │   ├── notification_list.go
│   │   │   ├── permission.go
│   │   │   ├── permission_test.go
│   │   │   ├── progress.go
│   │   │   ├── question.go
│   │   │   ├── search.go
│   │   │   ├── shortcut_tip.go
│   │   │   ├── sparkline.go
│   │   │   ├── sparkline_test.go
│   │   │   ├── special_renderers.go
│   │   │   ├── spinner.go
│   │   │   ├── splitpane.go
│   │   │   ├── starfield.go
│   │   │   ├── starfield_test.go
│   │   │   ├── statrow.go
│   │   │   ├── syntax.go
│   │   │   ├── tabbar.go
│   │   │   ├── taskgraph.go
│   │   │   ├── thinking.go
│   │   │   ├── thinking_test.go
│   │   │   ├── timeline.go
│   │   │   ├── toolcard.go
│   │   │   ├── toolcard_test.go
│   │   │   ├── toolrenderers.go
│   │   │   ├── truncate.go
│   │   │   ├── virtual_viewport.go
│   │   │   └── workflow_phasebar.go
│   │   ├── config_model_extra_test.go
│   │   ├── config_model.go
│   │   ├── confirmquit_model.go
│   │   ├── constants.go
│   │   ├── dashboard_model.go
│   │   ├── diff_model.go
│   │   ├── diff_view.go
│   │   ├── discuss_model.go
│   │   ├── execute_model.go
│   │   ├── execute_view.go
│   │   ├── fileexplorer_model.go
│   │   ├── filewatcher.go
│   │   ├── firstrun_extra_test.go
│   │   ├── firstrun_model.go
│   │   ├── firstrun_view.go
│   │   ├── ghostoutput_model.go
│   │   ├── ghostpicker_model.go
│   │   ├── goalinput_model.go
│   │   ├── header.go
│   │   ├── helpers.go
│   │   ├── help_model.go
│   │   ├── keybindings.go
│   │   ├── keybindings_screens.go
│   │   ├── layout
│   │   │   ├── box.go
│   │   │   ├── constraints.go
│   │   │   ├── extra_test.go
│   │   │   ├── minscreen.go
│   │   │   ├── page.go
│   │   │   ├── page_test.go
│   │   │   ├── responsive.go
│   │   │   ├── responsive_test.go
│   │   │   ├── solver.go
│   │   │   ├── stack.go
│   │   │   └── stack_test.go
│   │   ├── ledger_model.go
│   │   ├── mention.go
│   │   ├── mention_view.go
│   │   ├── metrics_model.go
│   │   ├── modelselector_list.go
│   │   ├── modelselector_model.go
│   │   ├── modelselector_view.go
│   │   ├── notification_model.go
│   │   ├── phasemodelpicker_extra_test.go
│   │   ├── phasemodelpicker.go
│   │   ├── phasemodelpicker_view.go
│   │   ├── plan_extra_test.go
│   │   ├── plan_model.go
│   │   ├── plan_refine.go
│   │   ├── plan_view.go
│   │   ├── provider_registration.go
│   │   ├── pure_funcs_extra_test.go
│   │   ├── README.md
│   │   ├── render_diff.go
│   │   ├── repl_clipboard.go
│   │   ├── repl_commands.go
│   │   ├── repl_footer.go
│   │   ├── repl.go
│   │   ├── repl_model.go
│   │   ├── repl_mouse.go
│   │   ├── repl_quickactions.go
│   │   ├── repl_scrollbar.go
│   │   ├── repl_state_extra_test.go
│   │   ├── repl_state.go
│   │   ├── repl_stream.go
│   │   ├── repl_thinking.go
│   │   ├── repl_view.go
│   │   ├── repl_welcome.go
│   │   ├── resume_extra_test.go
│   │   ├── resume_fixes_test.go
│   │   ├── resume_model.go
│   │   ├── resume_view.go
│   │   ├── rollback_extra_test.go
│   │   ├── rollback_model.go
│   │   ├── sessiondetail_model.go
│   │   ├── settings_edit.go
│   │   ├── settings_extra_test.go
│   │   ├── settings_model_extra_test.go
│   │   ├── settings_model.go
│   │   ├── settings_tabs.go
│   │   ├── settings_view.go
│   │   ├── ship_model.go
│   │   ├── ship_view.go
│   │   ├── sidebar_extra_test.go
│   │   ├── sidebar_fixes_test.go
│   │   ├── sidebar_model.go
│   │   ├── streaming
│   │   │   ├── agent_loop_extra_test.go
│   │   │   ├── agent_loop.go
│   │   │   ├── agent_loop_test.go
│   │   │   └── streaming.go
│   │   ├── streaming.go
│   │   ├── subagent_bridge.go
│   │   ├── subagents_model.go
│   │   ├── test_helpers_test.go
│   │   ├── theme
│   │   │   ├── borders.go
│   │   │   ├── cache_gen.go
│   │   │   ├── cache.go
│   │   │   ├── colors.go
│   │   │   ├── extra_test.go
│   │   │   ├── registry.go
│   │   │   ├── registry_test.go
│   │   │   ├── shadow.go
│   │   │   ├── tabs.go
│   │   │   ├── theme.go
│   │   │   ├── theme_test.go
│   │   │   └── unicode.go
│   │   ├── themepicker_model.go
│   │   ├── toast.go
│   │   ├── tooldetail_model.go
│   │   ├── transition.go
│   │   ├── truncate.go
│   │   ├── tuitypes
│   │   │   └── tuitypes.go
│   │   ├── types.go
│   │   ├── verify_extra_test.go
│   │   └── verify_model.go
│   ├── types
│   │   ├── constants.go
│   │   ├── constants_test.go
│   │   ├── extra_test.go
│   │   ├── git.go
│   │   ├── git_test.go
│   │   ├── plan.go
│   │   ├── plan_test.go
│   │   ├── types.go
│   │   └── types_test.go
│   └── workflow
│       ├── classify.go
│       ├── classify_test.go
│       ├── coverage_gates.go
│       ├── coverage_gates_test.go
│       ├── discuss_check.go
│       ├── discuss_check_test.go
│       ├── discuss.go
│       ├── discuss_test.go
│       ├── engine_extra_test.go
│       ├── engine.go
│       ├── engine_messages.go
│       ├── engine_messages_test.go
│       ├── engine_parse.go
│       ├── engine_parse_test.go
│       ├── engine_test.go
│       ├── engine_verify.go
│       ├── engine_verify_test.go
│       ├── execute.go
│       ├── execute_preflight.go
│       ├── execute_preflight_test.go
│       ├── execute_quality.go
│       ├── execute_quality_test.go
│       ├── execute_test.go
│       ├── fixes_test.go
│       ├── init_deep.go
│       ├── init_deep_test.go
│       ├── initialize.go
│       ├── initialize_test.go
│       ├── integration_test.go
│       ├── intent.go
│       ├── intent_test.go
│       ├── intermediate_progress_test.go
│       ├── parse_tool_calls_test.go
│       ├── phase_transition_test.go
│       ├── plan_check.go
│       ├── plan_check_test.go
│       ├── plan_chunk.go
│       ├── plan_chunk_test.go
│       ├── plan.go
│       ├── plan_parser_extra_test.go
│       ├── plan_parser.go
│       ├── plan_parser_test.go
│       ├── plan_test.go
│       ├── prompts
│       │   ├── autonomous.md
│       │   ├── base.md
│       │   ├── code-intelligence.md
│       │   ├── code-quality.md
│       │   ├── context-awareness.md
│       │   ├── demonstration-format.md
│       │   ├── discuss-followup.md
│       │   ├── discuss-questions.md
│       │   ├── execute-task.md
│       │   ├── intent-classify.md
│       │   ├── plan-check.md
│       │   ├── plan-format.md
│       │   ├── plan-outline.md
│       │   ├── plan-revise.md
│       │   ├── research.md
│       │   ├── self-heal.md
│       │   └── tool-use.md
│       ├── research.go
│       ├── research_test.go
│       ├── self_heal_visibility_test.go
│       ├── ship.go
│       ├── ship_lock_unix.go
│       ├── ship_lock_windows.go
│       ├── ship_preflight.go
│       ├── ship_preflight_test.go
│       ├── ship_test.go
│       ├── streaming_progress_test.go
│       ├── thinking_indicator_test.go
│       ├── verify.go
│       ├── verify_report.go
│       ├── verify_report_test.go
│       ├── verify_test.go
│       └── workflow_test.go
├── INVESTOR_QA.md
├── LICENSE
├── logo.svg
├── m31a
├── m31a.json
├── M31A.wiki
│   ├── Architecture.md
│   ├── Chat-History.md
│   ├── CodeComplexity-Tool.md
│   ├── Code-Intelligence.md
│   ├── Command-Palette.md
│   ├── Configuration.md
│   ├── Development-Guide.md
│   ├── Error-Handling.md
│   ├── File-Utilities.md
│   ├── Getting-Started.md
│   ├── Git-Integration.md
│   ├── Home.md
│   ├── Keybindings.md
│   ├── Logging.md
│   ├── Metrics.md
│   ├── Mouse-Support.md
│   ├── NVIDIA-NIM-Provider.md
│   ├── Prompt-History.md
│   ├── Prompt-System.md
│   ├── Provider-System.md
│   ├── Public-Packages.md
│   ├── Security-Model.md
│   ├── Session-Management.md
│   ├── Six-Phase-Workflow.md
│   ├── Slash-Commands.md
│   ├── Subagents.md
│   ├── Task-Runner.md
│   ├── Terminal-UI.md
│   ├── Token-Estimation.md
│   └── Tools-System.md
├── Makefile
├── pkg
│   ├── arbitrage
│   │   ├── arbitrage.go
│   │   ├── arbitrage_test.go
│   │   └── doc.go
│   ├── autodream
│   │   ├── autodream.go
│   │   ├── autodream_test.go
│   │   ├── doc.go
│   │   └── extra_test.go
│   ├── bisect
│   │   ├── bisect.go
│   │   ├── bisect_test.go
│   │   ├── doc.go
│   │   ├── doc_test.go
│   │   ├── exec.go
│   │   └── extra_test.go
│   ├── history
│   │   ├── history.go
│   │   └── history_test.go
│   ├── keychain
│   │   ├── errors.go
│   │   ├── extra_test.go
│   │   ├── keychain_darwin.go
│   │   ├── keychain.go
│   │   ├── keychain_linux.go
│   │   ├── keychain_test.go
│   │   └── keychain_windows.go
│   ├── ledger
│   │   ├── doc.go
│   │   ├── extra_test.go
│   │   ├── ledger.go
│   │   └── ledger_test.go
│   ├── metrics
│   │   ├── collector.go
│   │   └── types.go
│   ├── rollback
│   │   ├── doc.go
│   │   ├── doc_test.go
│   │   ├── rollback.go
│   │   └── rollback_test.go
│   ├── session
│   │   ├── checkpoint.go
│   │   ├── checkpoint_test.go
│   │   ├── doc.go
│   │   ├── doc_test.go
│   │   ├── manager_extra_test.go
│   │   ├── manager.go
│   │   ├── manager_test.go
│   │   ├── planning_extra_test.go
│   │   ├── planning.go
│   │   ├── planning_test.go
│   │   ├── session.go
│   │   ├── session_info.go
│   │   ├── session_info_test.go
│   │   ├── session_resumedat_test.go
│   │   └── session_test.go
│   └── taskrunner
│       ├── doc.go
│       ├── doc_test.go
│       ├── runner.go
│       └── runner_test.go
├── PLAN_UI.md
├── promotion
│   ├── 01-hacker-news-show-hn.md
│   ├── 02-reddit-r-programming.md
│   ├── 03-reddit-r-golang.md
│   ├── 04-reddit-r-commandline.md
│   ├── 05-reddit-r-localllama.md
│   ├── 06-devto-blog.md
│   ├── 07-medium-blog.md
│   ├── 08-twitter-thread.md
│   ├── 09-lobsters.md
│   ├── 10-github-discussions-producthunt.md
│   └── 11-posting-schedule.md
├── README.md
├── RESEARCH.md
├── scripts
│   └── verify_v1.sh
├── SECURITY.md
├── space.txt
└── TREE.md

44 directories, 532 files
