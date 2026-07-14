---
wave: 1
depends_on: ["01-critical-issues"]
files_modified:
  - internal/tools/git.go
  - internal/tools/bash.go
  - internal/tui/settings_model.go
  - internal/tui/config_model.go
  - internal/tui/app_update_phase.go
  - internal/workflow/execute.go
  - internal/workflow/engine_verify.go
  - internal/workflow/engine.go
  - internal/tools/persistent_permissions.go
autonomous: false
---

<planning_context>
**Phase:** 2
**Mode:** standard

<files_to_read>
- .planning/STATE.md (Project State)
- .planning/ROADMAP.md (Roadmap)
- DX_AUDIT.md (Requirements)
- .planning/phases/01-critical-issues/CONTEXT.md (USER DECISIONS from discuss-phase)
</files_to_read>

<agent_skills_planner>
- gsd-planner
</agent_skills_planner>

<review_incorporation_contract>
**If Mode is reviews:** REVIEWS.md is feedback input, not a hidden execution contract. /gsd-execute-phase primarily consumes PLAN.md plus the normal phase context, so every current actionable review finding must become visible in the relevant PLAN.md before planning can pass.

For each current actionable finding in REVIEWS.md, the planner MUST either:
- incorporate it into a PLAN.md task, `<action>`, `<acceptance_criteria>`, `<verify>`, `must_haves`, threat model, or artifact list; or
- explicitly document a deferral/rejection rationale in the relevant PLAN.md so the executor and reviewer can see the decision.

Historical findings already incorporated, explicitly deferred/rejected in PLAN.md, or marked fully resolved do not require new plan changes.
</review_incorporation_contract>

**Phase requirement IDs (every ID MUST appear in a plan's `requirements` field):** H1, H2, H3, H4, H5, H6, H7, H8, H9, H10

**Project instructions:** Read ./AGENTS.md or ./.opencode/AGENTS.md if either exists — follow project-specific guidelines
**Project skills:** Check .claude/skills/ or .agents/skills/ directory (if either exists) — read SKILL.md files, plans should account for project skill rules

**MVP_MODE:** false
**WALKING_SKELETON:** false
**Granularity:** fine
</planning_context>

<downstream_consumer>
Output consumed by /gsd-execute-phase. Plans need:
- Frontmatter (wave, depends_on, files_modified, autonomous)
- Tasks in XML format with read_first and acceptance_criteria fields (MANDATORY on every task)
- Verification criteria
- must_haves for goal-backward verification
- "Artifacts this phase produces" section (MANDATORY) — list every symbol this phase creates: decorators, classes, functions, CLI flags, struct/dataclass fields, new file paths
</downstream_consumer>

<deep_work_rules>
## Anti-Shallow Execution Rules (MANDATORY)

Every task MUST include these fields — they are NOT optional:

1. **`<read_first>`** — Files the executor MUST read before touching anything. Always include:
   - The file being modified (so executor sees current state, not assumptions)
   - Any "source of truth" file referenced in CONTEXT.md (reference implementations, existing patterns, config files, schemas)
   - Any file whose patterns, signatures, types, or conventions must be replicated or respected

2. **`<acceptance_criteria>`** — Verifiable conditions that prove the task was done correctly. Rules:
   - Every criterion must be checkable as a source assertion, behavior assertion, test command, or CLI output
   - NEVER use subjective language ("looks correct", "properly configured", "consistent with")
   - Include exact strings, patterns, values, command outputs, or observable behavior where that is the right proof
   - Examples:
     - Code: `auth.py contains def verify_token(` / `test_auth.py exits 0`
     - Behavior: `POST /api/auth/login returns 200 + httpOnly JWT cookie for valid credentials`
     - Config: `.env.example contains DATABASE_URL=` / `Dockerfile contains HEALTHCHECK`
     - Docs: `README.md contains '## Installation'` / `API.md lists all endpoints`
     - Infra: `deploy.yml has rollback step` / `docker-compose.yml has healthcheck for db`

3. **`<action>`** — Must include CONCRETE values, not references. Rules:
   - NEVER say "align X with Y", "match X to Y", "update to be consistent" without specifying the exact target state
   - Include concrete identifiers and reference values: config keys, function signatures, SQL table names, class names, import paths, env vars, endpoint paths, etc.
   - If CONTEXT.md has a comparison table or expected values, copy only the target identifiers/values needed to remove ambiguity
   - Do not include full file contents, fenced code blocks, or complete implementations in `<action>`
   - The executor should understand the intended target state from `<action>` and use `<read_first>` files for current implementation details, patterns, and source-of-truth context

**Why this matters:** Executor agents work from the plan text. Vague instructions like "update the config to match production" produce shallow one-line changes. Concrete instructions like "add DATABASE_URL, set POOL_SIZE=20, add REDIS_URL, and read config/runtime.ts before editing" produce complete work without turning the planner into the executor.
</deep_work_rules>

<quality_gate>
- [ ] PLAN.md files created in phase directory
- [ ] Each plan has valid frontmatter
- [ ] Tasks are specific and actionable
- [ ] Every task has `<read_first>` with at least the file being modified
- [ ] Every task has `<acceptance_criteria>` with behavior, test-command, CLI, or source assertions
- [ ] Every `<action>` contains concrete identifiers without fenced code blocks or full implementations
- [ ] Dependencies correctly identified
- [ ] Waves assigned for parallel execution
- [ ] must_haves derived from phase goal
- [ ] Every PLAN.md includes an "Artifacts this phase produces" section listing symbols created by this phase (decorators, classes, functions, CLI flags, struct/dataclass fields, new file paths)
</quality_gate>

<task>
  <id>1</id>
  <objective>Implement Dedicated Git Tool (H1)</objective>
  <action>
    Create a new file internal/tools/git.go with a Git tool that provides structured operations: git add, git commit, git diff, git log, git branch, git checkout, git stash. Parse output into structured types the LLM can reason about. Register the tool in internal/tools/defaults.go.
  </action>
  <read_first>
    internal/tools/bash.go
    internal/tools/defaults.go
    internal/tools/tool.go
  </read_first>
  <acceptance_criteria>
    internal/tools/git.go exists with Git tool implementation
    Git tool supports operations: add, commit, diff, log, branch, checkout, stash
    Git tool parses output into structured types
    Git tool is registered in internal/tools/defaults.go
    Git tool integrates with existing permission system
    Git commands are not blocked by dangerous-command blocker
  </acceptance_criteria>
</task>

<task>
  <id>2</id>
  <objective>Add workdir Parameter to Bash Tool (H2)</objective>
  <action>
    Modify internal/tools/bash.go to add an optional workdir parameter to the Bash tool schema. Update the Execute method to change to the specified working directory before executing commands. Update the tool description to document the new parameter.
  </action>
  <read_first>
    internal/tools/bash.go
    internal/tools/tool.go
  </read_first>
  <acceptance_criteria>
    Bash tool schema includes optional workdir parameter
    Execute method changes to specified directory before running commands
    Tool description documents workdir parameter
    Commands execute in correct working directory when workdir is specified
    Commands execute in default directory when workdir is not specified
  </acceptance_criteria>
</task>

<task>
  <id>3</id>
  <objective>Unify Settings Systems (H3)</objective>
  <action>
    Modify internal/tui/settings_model.go to make it a simplified view of the Config editor. Add a note indicating "showing common options -- see Config editor for advanced settings." Ensure all provider choices including "nvidia" are available in both views.
  </action>
  <read_first>
    internal/tui/settings_model.go
    internal/tui/config_model.go
    internal/tui/config_model_provider.go
  </read_first>
  <acceptance_criteria>
    Settings screen shows note about being simplified view
    Settings screen references Config editor for advanced options
    Config editor includes "nvidia" in provider choices
    Both views show consistent provider options
    No duplicate fields without explanation
  </acceptance_criteria>
</task>

<task>
  <id>4</id>
  <objective>Add Input Validation to Settings/Config Editors (H4)</objective>
  <action>
    Modify internal/tui/settings_model.go and internal/tui/config_model_*.go to add inline validation errors when values fail parsing. Highlight fields in error color and display a toast notification when validation fails.
  </action>
  <read_first>
    internal/tui/settings_model.go
    internal/tui/config_model.go
    internal/tui/toast.go
  </read_first>
  <acceptance_criteria>
    Numeric fields show error when non-numeric input is entered
    Invalid values are highlighted in error color
    Toast notification appears when validation fails
    Invalid values are not saved
    Valid values are saved normally
  </acceptance_criteria>
</task>

<task>
  <id>5</id>
  <objective>Add Phase Transition Confirmation (H5)</objective>
  <action>
    Modify internal/tui/app_update_phase.go to add a transition screen between phases (Discuss->Plan, Execute->Verify, Verify->Runtime). Show what was accomplished and ask "Proceed to [next phase]? [y/n/r]".
  </action>
  <read_first>
    internal/tui/app_update_phase.go
    internal/tui/app_model.go
  </read_first>
  <acceptance_criteria>
    Transition screen appears between Discuss->Plan phases
    Transition screen appears between Execute->Verify phases
    Transition screen appears between Verify->Runtime phases
    Transition screen shows what was accomplished in previous phase
    Transition screen asks for confirmation to proceed
    User can choose to proceed, go back, or cancel
  </acceptance_criteria>
</task>

<task>
  <id>6</id>
  <objective>Add Pause/Resume to Execution Loop (H6)</objective>
  <action>
    Modify internal/workflow/execute.go to add pause/resume functionality. Show a "pause" button in the execute screen. When paused, allow the user to review completed tasks, skip pending tasks, or cancel specific tasks.
  </action>
  <read_first>
    internal/workflow/execute.go
    internal/tui/execute_model.go
  </read_first>
  <acceptance_criteria>
    Execute screen shows pause button
    Pausing execution stops task processing
    Paused state allows reviewing completed tasks
    Paused state allows skipping pending tasks
    Paused state allows canceling specific tasks
    Resume continues execution from where it was paused
  </acceptance_criteria>
</task>

<task>
  <id>7</id>
  <objective>Improve Verification Checks (H7)</objective>
  <action>
    Modify internal/workflow/engine_verify.go to add configurable verify commands (build_command, test_command, lint_command). Improve placeholder detection with heuristics for empty function bodies. Increase the "suspiciously small" threshold from 20 bytes to a more reasonable value.
  </action>
  <read_first>
    internal/workflow/engine_verify.go
    internal/workflow/engine.go
  </read_first>
  <acceptance_criteria>
    Verification supports configurable build_command, test_command, lint_command
    Placeholder detection catches empty function bodies
    Placeholder detection catches stub implementations
    "Suspiciously small" threshold increased to reasonable value
    Verification runs linters and type-checkers when configured
  </acceptance_criteria>
</task>

<task>
  <id>8</id>
  <objective>Add Running Cost/Time Display (H8)</objective>
  <action>
    Modify internal/workflow/engine.go to show running cost counter in workflow sidebar. Update cost after each LLM call. Show estimated remaining cost based on current phase.
  </action>
  <read_first>
    internal/workflow/engine.go
    internal/tui/sidebar_model.go
  </read_first>
  <acceptance_criteria>
    Workflow sidebar shows running cost counter
    Cost updates after each LLM call
    Estimated remaining cost displayed
    Cost displayed in readable format (e.g., $0.15)
    Time elapsed displayed alongside cost
  </acceptance_criteria>
</task>

<task>
  <id>9</id>
  <objective>Implement Persistent Permission Saving (H9)</objective>
  <action>
    Modify internal/tools/persistent_permissions.go to add Save() method. After user selects "Allow for session," persist the rule to ~/.m31a/permissions.json.
  </action>
  <read_first>
    internal/tools/persistent_permissions.go
    internal/tools/permission.go
  </read_first>
  <acceptance_criteria>
    PersistentPermissions has Save() method
    Save() writes rules to ~/.m31a/permissions.json
    "Allow for session" option persists rules to file
    Persisted rules are loaded on startup
    Rules survive application restart
  </acceptance_criteria>
</task>

<task>
  <id>10</id>
  <objective>Show Workflow Mode Determination (H10)</objective>
  <action>
    Modify internal/tui/app_update_phase.go to show intent classification result and selected mode before starting workflow. Let the user override the mode choice.
  </action>
  <read_first>
    internal/tui/app_update_phase.go
    internal/workflow/engine.go
  </read_first>
  <acceptance_criteria>
    Intent classification result displayed before workflow starts
    Selected mode (Full/Fast/Direct) shown to user
    User can override mode choice
    Workflow uses user-selected mode if overridden
    Default behavior preserved if user doesn't override
  </acceptance_criteria>
</task>

<must_haves>
  <truths>
    <!-- High priority issue fixes must be implemented -->
    <truth>Dedicated Git tool with structured operations exists</truth>
    <truth>Bash tool supports workdir parameter</truth>
    <truth>Settings and Config editors are unified with clear explanation</truth>
    <truth>Input validation shows errors for invalid values</truth>
    <truth>Phase transitions require user confirmation</truth>
    <truth>Execution loop supports pause/resume</truth>
    <truth>Verification checks are configurable and comprehensive</truth>
    <truth>Running cost/time displayed during workflow</truth>
    <truth>Persistent permissions are saved to file</truth>
    <truth>Workflow mode determination is visible and overridable</truth>
  </truths>
</must_haves>

<artifacts>
  <!-- Artifacts this phase produces -->
  <symbol>
    <name>GitTool</name>
    <type>struct</type>
    <file>internal/tools/git.go</file>
    <description>Dedicated Git tool with structured operations</description>
  </symbol>
  <symbol>
    <name>workdir</name>
    <type>parameter</type>
    <file>internal/tools/bash.go</file>
    <description>Optional working directory parameter for Bash tool</description>
  </symbol>
  <symbol>
    <name>Save</name>
    <type>method</type>
    <file>internal/tools/persistent_permissions.go</file>
    <description>Save method for PersistentPermissions</description>
  </symbol>
</artifacts>
