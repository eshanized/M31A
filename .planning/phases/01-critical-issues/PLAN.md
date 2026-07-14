---
wave: 1
depends_on: []
files_modified:
  - internal/tools/bash.go
  - install.sh
  - cmd/m31a/main.go
  - internal/tui/components/permission.go
autonomous: false
---

<planning_context>
**Phase:** 1
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

**Phase requirement IDs (every ID MUST appear in a plan's `requirements` field):** C1, C2, C3, C4

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
  <objective>Fix Bash Tool Blocks Legitimate Shell Syntax (C1)</objective>
  <action>
    Remove `$(`, `${`, and backtick from the obfuscation blocklist in internal/tools/bash.go at lines 478-481. These patterns are already handled by the containsVariableExpansion() check for actual injection attempts. The obfuscation list should only catch multi-character sequences that require multiple bash features to be dangerous together.
  </action>
  <read_first>
    internal/tools/bash.go
  </read_first>
  <acceptance_criteria>
    internal/tools/bash.go no longer contains `$(`, `${`, and backtick in the obfuscation blocklist around lines 478-481
    The containsVariableExpansion() function remains unchanged to catch actual injection attempts
    Bash tool allows commands like `echo $(date)`, `cd ${DIR}`, and `echo $HOME` without blocking
    Bash tool still blocks genuinely dangerous obfuscation attempts
  </acceptance_criteria>
</task>

<task>
  <id>2</id>
  <objective>Fix Installer Downloads Will 404 (C2)</objective>
  <action>
    Modify install.sh to use lowercase project name and keep the `v` prefix in the URL template. Change line 52-113 to construct URLs as `m31a_v{{.Version}}_{{.OS}}_{{.Arch}}.tar.gz` to match what goreleaser produces.
  </action>
  <read_first>
    install.sh
  </read_first>
  <acceptance_criteria>
    install.sh constructs download URLs using lowercase project name (m31a)
    install.sh includes the `v` prefix in version (e.g., v1.0.0)
    install.sh produces URLs matching goreleaser output format: m31a_v{{.Version}}_{{.OS}}_{{.Arch}}.tar.gz
    Installation via `curl | bash` works without 404 errors
  </acceptance_criteria>
</task>

<task>
  <id>3</id>
  <objective>Implement --goal Headless Mode (C3)</objective>
  <action>
    Complete the implementation of the runHeadlessWorkflow function in cmd/m31a/main.go (lines 57-59). Replace the stub that prints "not yet fully implemented" with proper implementation that executes the full workflow engine in headless mode, returning appropriate exit codes and handling errors correctly.
  </action>
  <read_first>
    cmd/m31a/main.go
    internal/workflow/engine.go
  </read_first>
  <acceptance_criteria>
    cmd/m31a/main.go runHeadlessWorkflow function is fully implemented
    Headless mode executes the complete workflow (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship)
    Headless mode returns exit code 0 on success
    Headless mode returns appropriate non-zero exit codes on failure
    Headless mode handles errors gracefully without panicking
  </acceptance_criteria>
</task>

<task>
  <id>4</id>
  <objective>Fix Permission Modal Timeout Is Invisible (C4)</objective>
  <action>
    Modify internal/tui/components/permission.go to show the countdown bar from the start of the 10-minute auto-deny timeout, not just when less than 5 minutes remain. Update the timeout display logic to show remaining time throughout the entire duration.
  </action>
  <read_first>
    internal/tui/components/permission.go
  </read_first>
  <acceptance_criteria>
    internal/tui/components/permission.go shows countdown from 10:00 when permission modal opens
    Countdown updates in real-time showing remaining time (mm:ss format)
    Users can see the timeout approaching throughout the entire 10-minute period
    Auto-deny still occurs after 10 minutes of inactivity
  </acceptance_criteria>
</task>

<must_haves>
  <truths>
    <!-- Critical issue fixes must be implemented -->
    <truth>Bash tool allows legitimate shell syntax like $(date) and ${VAR}</truth>
    <truth>Installer downloads correct URLs matching goreleaser output</truth>
    <truth>--goal headless mode executes full workflow and returns proper exit codes</truth>
    <truth>Permission modal shows countdown from start of 10-minute timeout</truth>
  </truths>
</must_haves>

<artifacts>
  <!-- Artifacts this phase produces -->
  <!-- This phase fixes existing code, doesn't create new symbols -->
</artifacts>
