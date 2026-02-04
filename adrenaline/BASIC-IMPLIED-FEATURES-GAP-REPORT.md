# M31A Basic Implied Feature Gap Report

Generated: 2026-06-10

## Scope

This report studies the current codebase against the features a Go terminal AI
coding agent named and documented like M31A already implies. The focus is not on
future V1.1 features, subagents, ghost mode, or extra tools. It is limited to
baseline capabilities that the existing README, architecture docs, config
surface, command registry, and package layout already suggest should work.

Reviewed areas:

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/CONFIG.md`
- `docs/SLASH_COMMANDS.md`
- `cmd/m31a`
- `internal/config`
- `internal/provider`
- `internal/tui`
- `internal/workflow`
- `internal/tools`
- `internal/git`
- `pkg/session`
- `pkg/taskrunner`
- existing reports in `adrenaline/`

## Executive Summary

M31A has many of the major pieces in place: a Bubble Tea TUI, provider
registry, workflow engine, tool dispatcher, session manager, rollback, ledger,
arbitrage, AutoDream, and a broad slash-command surface. Several findings from
older adrenaline reports have also been fixed.

The remaining basic gaps are mostly wiring and contract gaps. The most important
ones are:

1. Startup routing and resume-on-startup are incomplete.
2. The workflow discuss phase is not actually connected end-to-end.
3. Tool-call schemas, parser output, and dispatcher input expectations do not
   agree.
4. Provider-native tool calls are advertised structurally but not parsed or
   executed as native tool calls.
5. Task execution can mark a task successful even when the model produced no
   tool calls and changed no files.
6. Auto-fallback exists as helper code and a manual command, but runtime chat and
   workflow paths do not use it.
7. Verification config is mostly unused, and non-Go verification commands are
   allowed to fail while still reporting success.
8. Workflow commits stage the whole worktree, which is unsafe in a dirty repo.
9. Some session lifecycle features exist in config/backend but are not wired into
   the TUI/startup path.
10. The tool prompt and name normalization lag behind the registered V1 toolset.

These are not exotic features. They are the operational basics users expect once
the project advertises session persistence, six-phase planning, tool execution,
verification, provider fallback, and safe coding-agent behavior.

## Already Fixed Or Stale Prior Findings

Several older adrenaline reports still mention issues that are no longer true in
the current tree. These should not be treated as open without re-checking:

- `/new` is registered and implemented through `handleNew`.
- `/export` is registered and implemented for session export.
- Shift+Enter multiline input is handled.
- `ctrl+y` copies the last assistant message.
- `/clear` and `/reset` use confirmation state.
- Permission listener re-arming is present.
- Workflow emitter messages are handled by the TUI update loop.
- Resume-screen ready messages are handled.
- Window resize handling no longer exits before updating model sizes.
- Theme propagation is wired across active screen models.
- AutoDream auto-trigger and auto-arbitrage integration exist for chat streams.
- Config save paths no longer write API keys into the config file.
- Config TOML merge uses `meta.Keys()`.
- `main.run()` has deferred cleanup and signal-driven `tea.Quit`.
- `workDir` and dispatcher initialization errors are checked.
- Git wrapper now has fetch, pull, and push support.
- Session backend supports labels, tags, search/filtering, rename, and export.

The gaps below are therefore focused on current behavior, not stale reports.

## Priority Table

| ID | Priority | Implied Feature | Current Gap |
| --- | --- | --- | --- |
| G01 | P0 | Configured app should start in a usable session or resume as configured | Default screen remains first-run, and `resume_on_startup` is set but unused |
| G02 | P0 | Discuss phase should ask, collect, and apply clarifying answers | `NeedsAnswers` is dropped, and discussion answers are not submitted to the workflow engine |
| G03 | P0 | Tools should execute using the schema exposed to the model | Parser emits direct args, dispatcher expects nested `params` |
| G04 | P0 | Tool-capable providers should support native tool calls end-to-end | Request shape and stream parser do not handle native OpenAI/OpenRouter `tool_calls` |
| G05 | P0 | Execute phase should not mark no-op prose as completed work | Empty/no-tool responses are treated as successful task results |
| G06 | P1 | Auto-fallback should happen when active provider is rate-limited or down | Fallback helpers are not called from chat streaming or workflow streaming |
| G07 | P1 | Verification should enforce configured build/test commands | `VerifyConfig` is not used by workflow verification |
| G08 | P1 | Verification failures should fail consistently across project types | Node/Python test commands include `|| true` and cannot fail verification |
| G09 | P1 | Workflow commits should not stage unrelated user changes | Execute and ship paths use `git add --all` behavior |
| G10 | P1 | Registered V1 tools should be represented in prompts and aliases | `FileList`, `FileDelete`, and `FileMove` are registered but not in tool-use prompt or aliases |
| G11 | P1 | Session lifecycle settings should affect runtime behavior | `resume_on_startup` and retention cleanup are not wired into app startup |
| G12 | P2 | Standard documented API-key env vars should work | Docs mention `OPENROUTER_API_KEY`/`ZEN_API_KEY`, code reads `M31A_*` names |
| G13 | P2 | Session browser should expose the backend's basic management abilities | Search, rename, tag filtering, and export are backend-only or command-only |
| G14 | P2 | Model selection should respect actual model capabilities | Tool capability defaults are optimistic and not validated before workflow/tool use |
| G15 | P2 | Cost tracking should imply basic budget guardrails | Cost is estimated/displayed, but no budget enforcement is wired |

## Detailed Findings

### G01 - Startup Routing And Resume-On-Startup Are Incomplete

Priority: P0

Implied behavior:

- A configured user should not be forced through first-run setup every launch.
- If `resume_on_startup` is enabled, the most recent session should be resumed.
- If no resumable session exists, the app should create a usable session and
  enter the REPL.

Current behavior:

- `ScreenFirstRun` is the zero-value screen in `internal/tui/types.go:17`.
- `NewApp` sets provider/config fields but does not choose a startup screen or
  create/resume a session in `internal/tui/app_state.go:175-240`.
- `routeToScreen` initializes whichever screen is already in `m.screen`, so the
  zero value naturally routes to first-run in `internal/tui/app_update.go:452-467`.
- `cmd/m31a/main.go:200-205` calls `app.SetResumeSessionID(sessions[0].ID)`,
  but `resumeSessionID` is only stored through `internal/tui/app_state.go:161-168`
  and is not read by the TUI startup flow.

Impact:

- Existing configured users can still start at first-run instead of REPL.
- `resume_on_startup` looks implemented in config and main, but is effectively a
  no-op.
- Startup can leave the app without a current session until a later user action.

Recommended fix:

- Add an explicit startup decision in `NewApp` or `Init`.
- If config/provider is valid and `resume_on_startup` has a candidate session,
  load that session before routing.
- If config/provider is valid but no session is selected, create a session and
  route to `ScreenREPL`.
- Reserve `ScreenFirstRun` for genuinely unconfigured installs or explicit reset.

### G02 - Discuss Phase Is Not Connected End-To-End

Priority: P0

Implied behavior:

- The six-phase workflow says discussion comes before planning.
- When the model asks clarifying questions, the TUI should collect answers and
  the workflow should use them before planning.

Current behavior:

- `runDiscuss` returns `NeedsAnswers: len(questions) > 0` in
  `internal/workflow/discuss.go:85-91`.
- `RunPhaseCmd` constructs `PhaseResultMsg` in `internal/tui/app.go:135-143`,
  but drops `NeedsAnswers`, `RequiresManualInput`, duration, commits, tool-call
  counts, and diff stats from the workflow result.
- `handlePhaseResult` only opens the discuss screen if `msg.NeedsAnswers` is
  true in `internal/tui/app_update_phase.go:42-54`.
- Because the flag is dropped, the TUI proceeds toward planning even when the
  workflow result contained questions.
- `DiscussModel` emits `QuestionResponseMsg` in `internal/tui/discuss.go:112-138`.
- `handleQuestionResponse` only responds to active `AskUserQuestion` tool
  requests where `m.questionRequest != nil`, in
  `internal/tui/app_update.go:1196-1207`.
- The TUI's `workflowEngineInterface` in `internal/tui/app_state.go:26-37` does
  not expose `SubmitDiscussAnswer`, `FinalizeDiscuss`, `DiscussState`, or
  `SkipDiscuss`, even though the workflow engine has those concepts.

Impact:

- Clarifying questions generated by the discuss phase are not reliably shown.
- If they are shown, answers are not submitted back to the workflow engine.
- Plan generation can run without the requirements context the discuss phase was
  designed to collect.

Recommended fix:

- Extend `PhaseResultMsg` to carry all relevant `workflow.PhaseResult` fields.
- Add discuss-specific TUI handlers that call workflow discuss methods instead of
  reusing the `AskUserQuestion` tool response path.
- Extend `workflowEngineInterface` with the discuss methods needed by the TUI.
- Add an integration test covering: goal submitted -> discuss questions shown ->
  answer submitted -> plan phase receives finalized discuss context.

### G03 - Tool-Call Argument Contract Does Not Match Dispatcher Expectations

Priority: P0

Implied behavior:

- The schemas sent to the model should describe exactly what the dispatcher can
  execute.
- A model following the advertised schema should be able to call `Bash`,
  `FileRead`, `Edit`, and other tools without hidden wrapper fields.

Current behavior:

- Tool definitions are built from registered tools in
  `internal/workflow/engine.go:439-459`.
- The parser accepts model output such as
  `{"name":"Bash","input":{"command":"echo hello"}}`, as shown by workflow
  parser tests.
- `parseSingleToolCall` stores that direct object as `types.ToolCall.Input` in
  `internal/workflow/engine_parse.go:383-455`.
- The dispatcher then unmarshals `ToolCall.Input` into `types.ToolInput` in
  `internal/tools/dispatcher.go:142-150`.
- Actual tools read `input.Params["path"]`, `input.Params["command"]`, and so on.
  Examples: `internal/tools/fileread.go:56-70` and
  `internal/tools/filewrite.go:61-80`.
- Execution tests use a nested shape such as
  `{"name":"FileWrite","input":{"params":{"path":"...","content":"..."}}}`,
  which differs from the natural schema shape.

Impact:

- A model can follow the schema exactly and still fail execution because the
  dispatcher receives an empty `Params` map.
- Tool use reliability becomes prompt-dependent instead of contract-dependent.
- This undermines the core coding-agent loop.

Recommended fix:

- Choose one canonical tool-call argument contract.
- Prefer direct schema args, for example `{"path":"README.md"}`, because that
  matches normal function-call JSON schema behavior.
- Normalize in one place:
  - If `input.params` exists, use it for backward compatibility.
  - Otherwise treat `input` as the params object.
- Update parser, dispatcher tests, and tool-use prompt to match.

### G04 - Native Provider Tool Calls Are Not Implemented End-To-End

Priority: P0

Implied behavior:

- A provider request carrying `Tools []ToolDefinition` should either use native
  provider tool calling correctly or intentionally stay in text-tool mode.
- For OpenRouter/OpenAI-compatible streaming, native `tool_calls` deltas should
  be parsed and executed if tools are sent as tools.

Current behavior:

- `provider.BuildChatBody` adds `"tools": req.Tools` directly in
  `internal/provider/common.go:38-55`.
- `ToolDefinition` is `{name, description, parameters}` in
  `internal/provider/interface.go:28-32`, not the OpenAI/OpenRouter native shape
  `{"type":"function","function":{...}}`.
- `ParseSSEChunk` in `internal/provider/reasoning.go:110-191` parses content,
  reasoning, and finish reasons, but ignores streamed `delta.tool_calls`.
- `types.StreamChunk` in `internal/types/types.go:169-174` has no tool-call
  field.
- The workflow therefore depends on text JSON extraction through
  `internal/workflow/engine_parse.go`.

Impact:

- Native tool-call capable models may not receive valid native tool definitions.
- If a provider returns native tool calls anyway, the stream parser drops them.
- Tool execution only works when the model emits the project's text JSON format.

Recommended fix:

- Decide explicitly between text-tool protocol and native provider tools.
- If native tools are intended:
  - Convert `ToolDefinition` into provider-native request shape.
  - Add streamed and non-streamed tool-call parsing.
  - Extend `StreamChunk` or add a separate message for accumulated tool calls.
  - Add provider contract tests using representative OpenRouter chunks.
- If text tools are intended for V1:
  - Do not send the `tools` field as native tools.
  - Put the text protocol in the system prompt and remove ambiguous request
    fields.

### G05 - Execute Phase Can Mark No-Op Responses As Successful

Priority: P0

Implied behavior:

- A coding-agent execute phase should only complete implementation tasks when it
  made the requested change or at least produced an explicit, validated result.
- A prose response with zero tool calls should not mark a file-changing task as
  done.

Current behavior:

- `executeTaskWithTools` parses tool calls and loops over them in
  `internal/workflow/execute.go:157-291`.
- If the model returns no tool calls, the loop is skipped.
- The function can return `TaskResult{Success: true, ToolCalls: 0}`.
- `internal/workflow/execute_test.go:208-227` explicitly expects an empty model
  response to succeed.
- `pkg/taskrunner/runner.go:230-239` marks successful results as completed.

Impact:

- Tasks can be marked done without file edits, tests, or verifiable output.
- The workflow can advance toward verification and ship with missing
  implementation.
- This creates false confidence in the six-phase automation.

Recommended fix:

- Add task completion criteria:
  - File-changing tasks require at least one successful mutating tool call or a
    verified file diff.
  - Analysis-only tasks may allow no file changes only if explicitly marked as
    such.
  - Empty model responses should fail or retry.
- Update tests to assert no-op task responses fail for implementation tasks.

### G06 - Auto-Fallback Is Not Wired Into Runtime Failures

Priority: P1

Implied behavior:

- README and architecture claims around provider fallback imply that when the
  active provider rate-limits or fails, M31A can automatically try the configured
  fallback provider.

Current behavior:

- Fallback selection helpers exist in `internal/provider/fallback.go:22-60` and
  `internal/provider/fallback.go:101-128`.
- Runtime searches show those helpers are used by tests and by manual fallback
  command paths, not by chat streaming or workflow streaming.
- `/fallback` is manual and implemented in `internal/tui/commands_ai.go:217-252`.
- `StartStreamCmd` returns `StreamErrorMsg` on provider errors in
  `internal/tui/streaming.go:70-81`.
- `handleStreamErrorMsg` records the error and updates UI state in
  `internal/tui/repl_stream.go:190-214`, but does not invoke fallback.
- The UI says "Rate limited. Auto-fallback in progress..." in
  `internal/tui/repl_stream.go:159-162`, but no automatic fallback is started.
- Workflow streaming in `internal/workflow/engine.go:502-535` returns provider
  errors directly.

Impact:

- Users see auto-fallback language without auto-fallback behavior.
- Workflow execution can fail on transient provider errors despite an available
  fallback provider.

Recommended fix:

- Route provider errors through a common fallback/retry policy.
- Cover both chat REPL streaming and workflow LLM calls.
- Preserve the no-direct-provider rule: fallback should only switch between
  configured OpenRouter and Zen clients.
- Add tests for rate limit, temporary 5xx, unavailable provider, and no fallback
  configured.

### G07 - Configured Build/Test Verification Is Not Used

Priority: P1

Implied behavior:

- `VerifyConfig` and config docs imply users can set project build/test commands
  and the workflow will use them for verification.

Current behavior:

- `internal/config/types.go:27-32` defines `VerifyConfig` with `BuildCommand`
  and `TestCommand`.
- Workflow verification uses auto-detected commands in
  `internal/workflow/engine_verify.go:114-243`.
- Searches did not show workflow verification reading `cfg.Verify`.

Impact:

- Users can configure verification commands that do not affect the verify phase.
- Non-standard projects cannot make verification authoritative without changing
  code.

Recommended fix:

- Pass verify config into the workflow engine.
- If configured commands are present, run them before or instead of detected
  defaults.
- Record command, exit code, stdout/stderr summary, and duration in verification
  results.
- Add tests proving configured commands override or augment auto-detection.

### G08 - Node And Python Verification Commands Cannot Fail Tests

Priority: P1

Implied behavior:

- Verification should fail when configured or detected tests fail.

Current behavior:

- Go build/test checks return errors correctly in
  `internal/workflow/engine_verify.go:141-146` and
  `internal/workflow/engine_verify.go:218-222`.
- Node build uses `npm run build 2>&1 || tsc --noEmit 2>&1 || true` in
  `internal/workflow/engine_verify.go:162`.
- Node tests use `npm test 2>&1 || true` in
  `internal/workflow/engine_verify.go:227`.
- Python tests use `python3 -m pytest 2>&1 || true` in
  `internal/workflow/engine_verify.go:235`.
- Because of `|| true`, those shell commands can return success even when tests
  fail.

Impact:

- Failed Node/Python tests can be reported as passing verification.
- Verification quality differs sharply by language.

Recommended fix:

- Remove `|| true` from test commands that should be authoritative.
- If fallback command probing is needed, implement it in Go:
  - Try command A if project files indicate it should exist.
  - If command A is unavailable, try command B.
  - If an available command runs and fails, mark verification failed.

### G09 - Workflow Commits Stage The Whole Worktree

Priority: P1

Implied behavior:

- A coding agent should avoid committing unrelated user changes in a dirty
  worktree.
- Task commits should be scoped to files the task changed or to the workflow's
  known output set.

Current behavior:

- `executeTaskWithTools` calls `git.AddAll()` when `len(task.Files) > 0` in
  `internal/workflow/execute.go:268-274`.
- `internal/git/git.go:81-90` also calls `AddAll()` inside `Commit`.
- Ship flow also stages all changes in `internal/workflow/ship.go:51-58`.

Impact:

- Unrelated user edits can be swept into automated task or ship commits.
- This violates the basic expectation that an agent works with a dirty tree
  conservatively.

Recommended fix:

- Capture pre-task git status before tool execution.
- Stage only files changed by the task or listed in the task plan.
- Refuse to commit unrelated pre-existing dirty files unless explicitly allowed.
- Remove implicit `AddAll()` from `git.Commit`, or add a separate
  `CommitAlreadyStaged` API.

### G10 - Registered Tools Are Not Fully Reflected In Prompts Or Aliases

Priority: P1

Implied behavior:

- All V1 registered tools should be visible to the model and normalized from
  common names.
- Safe file operations like list, delete, and move should be available when
  registered.

Current behavior:

- `internal/tools/defaults.go:5-45` registers `FileList`, `FileDelete`, and
  `FileMove` in addition to the core tools.
- `internal/workflow/prompts/tool-use.md:10-84` documents Bash, FileRead,
  FileWrite, Edit, Glob, Grep, WebFetch, TodoWrite, and AskUserQuestion, but not
  `FileList`, `FileDelete`, or `FileMove`.
- `normalizeToolName` in `internal/workflow/engine_parse.go:457-482` normalizes
  aliases for the older set, but not common aliases for `FileList`,
  `FileDelete`, or `FileMove`.

Impact:

- Models are less likely to use safer file-management tools.
- Calls using common aliases such as `list_files`, `delete_file`, or `move_file`
  may fail even though the tools exist.

Recommended fix:

- Update the tool-use prompt from the dispatcher registry or keep a tested static
  list in sync.
- Add aliases for `FileList`, `FileDelete`, and `FileMove`.
- Add a test that every registered default tool appears in prompt guidance or in
  generated tool definitions.

### G11 - Session Lifecycle Settings Are Not Fully Wired

Priority: P1

Implied behavior:

- A terminal agent with session persistence should support reliable resume,
  retention, and cleanup behavior.

Current behavior:

- `resume_on_startup` is read in `cmd/m31a/main.go:200-205`, but the selected ID
  is not consumed by the TUI startup flow.
- `SessionRetentionDays` exists in `internal/config/types.go:199-200`.
- `pkg/session/manager.go:680-722` has cleanup logic.
- The startup path does not appear to call session cleanup.

Impact:

- Session retention config is not operational.
- Resume behavior depends on manual navigation instead of configured startup
  policy.

Recommended fix:

- Wire cleanup into startup using the configured retention window.
- Consume `resumeSessionID` during TUI initialization.
- Add startup tests for:
  - resume enabled with existing session,
  - resume enabled with missing session,
  - resume disabled,
  - retention cleanup.

### G12 - Documented API-Key Environment Variables Do Not Match Code

Priority: P2

Implied behavior:

- Standard provider environment variables documented to users should work.

Current behavior:

- `docs/CONFIG.md` documents `OPENROUTER_API_KEY` and `ZEN_API_KEY`.
- The loader reads `M31A_OPENROUTER_API_KEY` and `M31A_ZEN_API_KEY` in
  `internal/config/loader.go:590-626`.

Impact:

- Users following docs or common provider conventions may be told no API key is
  available.
- First-run setup burden increases unnecessarily.

Recommended fix:

- Support both names:
  - Prefer explicit `M31A_OPENROUTER_API_KEY` and `M31A_ZEN_API_KEY`.
  - Fall back to `OPENROUTER_API_KEY` and `ZEN_API_KEY`.
- Update docs to state the precedence clearly.

### G13 - Session Browser Does Not Expose Basic Backend Management Features

Priority: P2

Implied behavior:

- If sessions can be searched, renamed, tagged, and exported, the session browser
  should expose at least the basic flows.

Current behavior:

- Backend session manager supports labels, tags, search/filtering, rename, and
  export in `pkg/session/manager.go:747-805`.
- `types.Session` includes `Label` and `Tags` in `internal/types/types.go:155-160`.
- Resume screen navigation in `internal/tui/resume_model.go:51-95` supports
  up/down/enter/new-session only.
- Resume view displays labels but does not expose editing or filtering in
  `internal/tui/resume_view.go:79-84`.
- Export is available through `/export`, not from the browser.

Impact:

- Session persistence scales poorly as session count grows.
- Existing backend features are not discoverable from the primary session UI.

Recommended fix:

- Add search/filter input to resume screen.
- Add rename and export commands or keybindings from selected session.
- Show tags and allow tag filtering if tags remain part of the session model.

### G14 - Model Capability Validation Is Optimistic

Priority: P2

Implied behavior:

- If workflow execution depends on tool use, M31A should avoid or warn about
  models that cannot use tools.

Current behavior:

- `ParseModelCapabilities` defaults `Tools: true` in
  `internal/provider/capabilities.go:12-20`.
- Tool use is then attempted through workflow/tool prompts without a hard check
  that the selected model is actually tool-capable.
- Vision capability is displayed in model metadata, but there is no corresponding
  image input workflow/tool path.

Impact:

- Users can select models that appear capable but fail at execution time.
- Capability flags can become decorative instead of operational.

Recommended fix:

- Treat unknown tool capability as unknown, not true.
- Validate selected model against the active workflow's needs.
- Warn or block workflow execution if the model is not known to support the
  required interaction mode.

### G15 - Cost Tracking Does Not Enforce Budgets

Priority: P2

Implied behavior:

- Once cost estimation and model arbitrage are visible, basic budget guardrails
  are expected for long autonomous workflows.

Current behavior:

- Cost estimation and usage fields exist across provider and workflow results.
- Auto-arbitrage can select cheaper models.
- There is no clear budget limit or stop condition wired into workflow
  execution.

Impact:

- A long workflow can continue spending without a project/session cap.
- Cost reporting is informational only.

Recommended fix:

- Add optional per-session and per-workflow budget limits.
- Check estimated cost before each phase/task.
- Stop or ask for confirmation when a budget would be exceeded.
- Keep this optional so normal chat remains lightweight.

## Suggested Implementation Order

1. Fix the tool-call contract first. Tool execution is central to chat, workflow,
   tests, and verification.
2. Fix discuss-phase result propagation and answer submission. This protects the
   quality of plans before execution.
3. Make no-op execution fail for implementation tasks. This prevents false task
   completion.
4. Wire startup routing and resume-on-startup. This improves daily usability and
   makes session persistence real.
5. Wire auto-fallback into chat and workflow LLM calls. This aligns behavior with
   the UI/docs.
6. Make verification authoritative: use configured commands and remove
   `|| true` failure masking.
7. Replace whole-worktree commit staging with scoped staging.
8. Bring registered tools, prompt docs, and alias normalization into sync.
9. Expose session browser search/rename/export and retention cleanup.
10. Add model capability and budget guardrails.

## Minimum Acceptance Tests To Add

The following tests would prevent regressions in the highest-risk gaps:

1. Startup with valid provider and `resume_on_startup = true` resumes the newest
   session and routes to REPL.
2. Startup with valid provider and `resume_on_startup = false` creates or opens a
   usable REPL session without first-run.
3. Discuss phase with generated questions shows discuss UI, submits answers to
   the workflow engine, and only then runs plan.
4. Tool dispatcher accepts both direct args and legacy nested `params`.
5. Provider SSE parser preserves native streamed tool calls.
6. Execute phase fails an implementation task when the model returns no tool
   calls and no diff.
7. Chat stream rate limit triggers configured fallback provider.
8. Workflow LLM rate limit triggers configured fallback provider.
9. Configured verify build/test commands are executed and failing exit codes fail
   verification.
10. Node/Python failing tests fail verification.
11. Task commit stages only task-owned files and leaves unrelated dirty files
   unstaged.
12. Every default registered tool is represented in prompt guidance and name
   normalization.

## Boundary Notes

The following were intentionally not counted as missing basic features because
they are prohibited or outside the current V1 scope:

- Direct Anthropic provider support.
- Direct OpenAI provider support.
- Subagents.
- Ghost mode.
- Picture-in-picture mode.
- Deferred tools.
- Tool additions beyond the current registered V1 set.
- Concurrent V1 task execution.
- Telemetry or analytics.
