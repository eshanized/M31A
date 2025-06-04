# Phase 11: Session & Config Adaptations — Context

**Gathered:** 2026-06-01
**Status:** Ready for planning
**Source:** `rush/opencode_adaptation_report.md` (adaptation items 2, 9, 10)

<domain>
## Phase Boundary

Phase 11 implements session branching, configuration layering, and permission rule completion — three OpenCode adaptations that enhance M31A's state management and security model.

**Depends on:** Phase 10 (Provider & Message Layer Adaptations)

**Duration:** 2 weeks | **Complexity:** 6/10 | **Milestone:** Session forking creates child sessions; multi-layer config with project-level override; permission rules use glob matching

### Adaptations

1. **Session Forking** (adoption #2) — Add `ParentID` to session struct, `/fork` creates child sessions with copied message history, sibling navigation via `/prev`/`/next`.

2. **Multi-Layer Configuration** (adoption #10) — Add project-level `m31a.toml`, config validation with error messages, variable substitution `${VAR}`.

3. **Permission Ruleset Completion** (adoption #9) — `PermissionRule.Pattern` field exists in config but is never matched against file paths in the dispatcher. Add glob matching logic and per-agent permission profiles.
</domain>

<decisions>
## Implementation Decisions

### Session Forking

- Add `ParentID string` to `types.Session` and `pkg/session.Session`
- `ForkSession(parentID string) (*Session, error)` on `session.Manager`:
  - Load parent session's messages.json
  - Copy to new session directory with new 8-char ID
  - Set ParentID on new session
  - Save both sessions (parent's ChildIDs updated)
- `/fork` command: calls `ForkSession(ctx.SessionID)`, returns new session ID
- `/prev` command: loads parent session's siblings, switches to previous sibling
- `/next` command: loads parent session's siblings, switches to next sibling
- Sibling navigation switches `ctx.SessionID` in the command result, triggering TUI state transition
- `session.Manager` stores sibling index via `Session.ChildrenIDs []string` field

### Multi-Layer Configuration

- Config loading order (later overrides earlier):
  1. `~/.m31a/config.toml` (default)
  2. Environment variable overrides (`M31A_*`)
  3. `./m31a.toml` (project-level, cwd)
- Schema validation: after TOML parsing, validate known fields have expected types
  - String fields must be strings, int fields must be ints, boolean fields must be bools
  - Unknown top-level keys produce warnings but are not errors (forward compat)
- Variable substitution: `${VAR}` patterns in string values replaced from env
  - `api_key = "${OPENROUTER_API_KEY}"` resolves at load time
- Config file discovery: walk up from cwd to find `m31a.toml` (max 3 levels)

### Permission Ruleset Completion

- `PermissionRule.Pattern` uses `doublestar` glob matching (already a dependency)
- Dispatcher checks: for each rule matching `Tool == toolName`, test `Pattern` against file paths in `ToolInput.Params`
- Actions: `allow` → skip further checks, `deny` → reject immediately, `ask` → show permission modal
- Per-agent profiles: `[permissions.agents.{agent_name}]` section in config with default action
- If no rule matches, fall back to existing risk-level gating
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Files to Modify/Create

- `internal/types/types.go` — Session struct (add ParentID, ChildrenIDs)
- `pkg/session/session.go` — Session struct (add ParentID, ChildrenIDs fields)
- `pkg/session/manager.go` — ForkSession(), SiblingSessions(), session tree
- `internal/tui/commands.go` — /fork, /prev, /next commands
- `internal/config/loader.go` — project-level config, validation, var substitution
- `internal/config/types.go` — validation tags
- `internal/tools/dispatcher.go` — glob pattern matching in permission check
- `internal/config/types.go` — agent permission profiles (already has PermissionRule struct)
- `internal/tui/components/permission.go` — show matched rule info

### Existing Code to Study

- `internal/config/loader.go` — Load(), ResolveAPIKeys(), env var resolution
- `internal/config/types.go` — Config struct, PermissionRule, PermissionsConfig, FeaturesConfig
- `internal/tools/dispatcher.go` — Execute(), permission check flow
- `pkg/session/manager.go` — NewSession(), ListSessions(), existing lifecycle
- `internal/tui/commands.go` — command patterns from Phase 7
- `internal/tui/app.go` — AppState, session switching
- `docs/INTERFACES.md` — interface definitions
</canonical_refs>

<specifics>
## Wave Structure

**Wave 1** (no dependencies):
- Plan 01: Session Forking (pkg/session + commands)

**Wave 2** (parallel):
- Plan 02: Multi-Layer Configuration (config loader)
- Plan 03: Permission Ruleset Completion (dispatcher)

### Key Patterns to Follow

- Atomic file writes for session state (temp file + rename)
- CommandResult pattern for TUI state transitions
- Doublestar library already in go.mod for glob matching
- Bubble Tea single-threaded model — all mutations through Update()
- Existing sentinel errors from internal/errors/
</specifics>

<deferred>
## Deferred Ideas

- Tree view of session fork hierarchy in TUI
- Visual diff between sibling sessions
- Config hot-reload on file change
- 8-layer config (OpenCode has remote well-known, global, project, env, etc.)
</deferred>

---

*Phase: 11-session-config-adaptations*
*Context gathered: 2026-06-01 via OpenCode Adaptation Report*
