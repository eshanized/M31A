---
phase: 11-session-config-adaptations
reviewed: 2026-06-01T22:00:00Z
depth: deep
files_reviewed: 14
files_reviewed_list:
  - internal/tools/dispatcher.go
  - internal/tools/interface.go
  - internal/tui/app.go
  - internal/tui/repl.go
  - internal/tui/commands.go
  - internal/tui/components/permission.go
  - internal/config/types.go
  - internal/config/loader.go
  - internal/config/loader_test.go
  - internal/types/types.go
  - pkg/session/manager.go
  - pkg/session/manager_test.go
  - pkg/session/session.go
  - pkg/session/session_info.go
findings:
  critical: 2
  warning: 7
  info: 3
  total: 12
status: issues_found
---

# Phase 11: Code Review Report — Session & Config Adaptations

**Reviewed:** 2026-06-01T22:00:00Z
**Depth:** deep (cross-file analysis)
**Files Reviewed:** 14 source files + 15 test files
**Status:** issues_found

## Summary

This phase implements three adaptations: session forking (11-01), multi-layer configuration (11-02), and permission rule glob matching (11-03). The implementation is largely solid with good test coverage, but two critical defects were found — the most severe being that permission rules defined in `config.toml` are silently ignored at runtime (CR-01). A second critical issue causes session data to be written to the wrong directory after `/fork`, `/prev`, or `/next` (CR-02).

Seven warnings cover boolean merge asymmetry, variable substitution ordering issues, agent bypass conditions, and test quality concerns. Three info items note opportunity areas for robustness.

---

## Critical Issues

### CR-01: Permission rules from config.toml never loaded into dispatcher

**File:** `internal/tui/app.go:144`, `internal/tools/dispatcher.go:455-456`
**Severity:** CRITICAL — Security feature failure

**Issue:**
`NewApp` creates the dispatcher via `DefaultDispatcher(cwd, backupDir, sessionBaseDir)` (app.go:144), which internally calls `NewDispatcher(nil)` (dispatcher.go:456). The `cfg` object loaded at line 101 (`config.Load(configPath)`) contains the user's `PermissionsConfig` with rules, agents, and default modes, but the `cfg.Permissions` is never propagated to the dispatcher.

The result: **every permission rule defined in `~/.m31a/config.toml` is silently ignored.** A `deny` rule blocking `rm -rf /` via Bash will not prevent execution. An `allow` rule for FileRead of specific paths will not be respected. Only the fallback RiskLevel check remains, which is coarser-grained.

**Call chain:**
1. `config.Load(configPath)` → returns full `Config` including `Permissions` (line 101)
2. `DefaultDispatcher(cwd, backupDir, sessionBaseDir)` → calls `NewDispatcher(nil)` (line 144)
3. `NewDispatcher(nil)` → creates dispatcher with empty rules, `d.rules = []config.PermissionRule{}` (line 42)
4. Any `checkPermission()` call iterates an empty rules slice — no rules ever match
5. Falls through to RiskLevel check for all permission decisions

**Fix — change `DefaultDispatcher` to accept config:**

```go
// Option A: Modify DefaultDispatcher to accept config
func DefaultDispatcher(workDir, backupDir, sessionsDir string, cfg *config.PermissionsConfig) *Dispatcher {
    d := NewDispatcher(cfg)
    d.Register(NewBash(workDir))
    d.Register(NewFileRead(workDir))
    // ... remaining tool registrations
    return d
}

// Then in app.go:
dispatcher: tools.DefaultDispatcher(cwd, backupDir, sessionBaseDir, &cfg.Permissions),
```

```go
// Option B: Create dispatcher directly in NewApp
d := tools.NewDispatcher(&cfg.Permissions)
d.Register(tools.NewBash(cwd))
d.Register(tools.NewFileRead(cwd))
d.Register(tools.NewFileWrite(cwd, backupDir, sessionBaseDir))
d.Register(tools.NewEdit(cwd, backupDir))
d.Register(tools.NewGlob(cwd))
d.Register(tools.NewGrep(cwd))
d.Register(tools.NewGlob(cwd))
// ...
app.dispatcher = d
```

---

### CR-02: Workflow engine session ID stale after `/fork`, `/prev`, `/next`

**File:** `internal/tui/app.go:652-691`
**Severity:** CRITICAL — Data loss, wrong session writes

**Issue:**
When processing session-switching commands (`/fork`, `/prev`, `/next`), the session ID is captured from `m.workflowEngine.SessionID()` at line 653, *before* the session switch executes. After the switch (lines 674-691):
- `m.replModel` is updated with the new session ID via `SetProvider`
- Messages are loaded from the new session
- But `m.workflowEngine` still holds the **old** session ID

Subsequent commands that query `m.workflowEngine.SessionID()` — including `/save`, `/status`, `/phase`, `/goal`, and automatic message persistence — will write to the original session's directory, not the forked/navigated session. This causes:
- Messages persisted to the wrong session
- Checkpoints associated with the wrong session
- `planning/` state files written to the wrong path
- Resume from the new session will find incomplete state

**Fix — update workflow engine after session switch:**

```go
// After session switch, around line 691:
m.replModel.SetProvider(m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
m.replModel.ClearMessages()
for _, msg := range sess.Messages {
    m.replModel.AddMessage(msg)
}
// ADD: Update workflow engine session ID
m.workflowEngine.SetSessionID(*result.SessionID)
m.currentOperation = fmt.Sprintf("Session %s loaded", *result.SessionID)
```

If `workflowEngine` does not have a `SetSessionID` method, the workflow engine must be reinitialized with the new session ID:
```go
m.workflowEngine = workflow.New(*result.SessionID, m.registry, m.dispatcher, m.config)
```

---

## Warnings

### WR-01: Boolean fields cannot be overridden to `false` in project config

**File:** `internal/config/loader.go:mergeConfig` (lines 118-198)
**Severity:** WARNING — Configuration incorrectness

**Issue:**
All boolean merge operations use the pattern:
```go
if overlay.Features.AutoBackup {
    base.Features.AutoBackup = true
}
```

This propagates `true` from the project config to the base but cannot propagate `false`. If the global config sets `AutoBackup = true` and the project config sets `AutoBackup = false`, the merge preserves `true` — the user's explicit `false` is silently discarded.

**Affected fields (9 total):**
- `Provider.AutoFallback` (line 118)
- `Model.ShowThinkingByDefault` (line 135)
- `Model.AutoCollapseTools` (line 138)
- `Model.AutoArbitrage` (line 141)
- `UI.CompactMode` (line 152)
- `UI.ShowTokenUsage` (line 155)
- `UI.ShowCostEstimate` (line 158)
- `Features.AutoBackup` (line 185)
- `Features.ResumeOnStartup` (line 188)
- `Ledger.Enabled` (line 193)

**Fix:**
Use a sentinel or pointer type pattern:
```go
// Option A: Add "IsSet" pattern
if overlay.isAutoBackupSet {
    base.Features.AutoBackup = overlay.Features.AutoBackup
}

// Option B: Use three-way merge (toml decode marks presence)
// TOML libraries can help distinguish "not present" from "false"
// Currently none of the bools use pointers.
```

For a practical V1 fix, document the limitation and consider switching to `*bool` for new config fields moving forward.

---

### WR-02: Variable substitution runs after validation, breaking `${VAR}` in enum fields

**File:** `internal/config/loader.go:Load()` (lines 81-87)
**Severity:** WARNING — Configuration usability

**Issue:**
Validation (step 6, line 82) runs before variable substitution (step 7, line 87). If a user writes:
```toml
[ui]
theme = "${M31A_THEME}"
```
The validator checks `"${M31A_THEME}"` against the allowed set `{"dark", "light", "auto"}` and rejects it because `${M31A_THEME}` is not a valid theme name. The same applies to `permissions.default_mode`.

The workaround is to use the env var override mechanism (`M31A_THEME` env var directly, which is applied at step 4, line 61-62 before validation). But the `${VAR}` pattern is documented as a general-purpose feature, so users naturally expect it to work for all string fields.

**Fix:**
Swap the order of validation and substitution, or add a pre-validation pass that replaces `${...}` patterns with a sentinel token:
```go
// Swap order:
// Step 6: Variable substitution
applyVarSubstitution(cfg)
// Step 7: Validation
if err := validateConfig(cfg); err != nil {
    return nil, fmt.Errorf("config validation: %w", err)
}
```

**Risk:** Swapping the order means substitution errors (undefined variables) would be silently preserved as `${VAR}` strings in validated fields. This is acceptable since the doc comment says "If the variable is not set, the original ${VAR} pattern is preserved as-is."

---

### WR-03: Agent default `"ask"` action bypassed for safe/medium tools

**File:** `internal/tools/dispatcher.go:100-135`
**Severity:** WARNING — Logic gap

**Issue:**
In `Execute`, when `checkPermission` returns `(false, pctx with Source="agent_default", nil)` (i.e., an agent profile matched with a `default_action = "ask"`), the code at line 102 checks:
```go
if pctx != nil && pctx.Source == "rule" && pctx.RuleAction == "ask" {
```

Since `pctx.Source` is `"agent_default"` (not `"rule"`), this condition is false. Control falls to the `else` branch (line 135), which checks:
```go
if risk == types.RiskDangerous || risk == types.RiskDestructive {
```

For `RiskSafe` and `RiskMedium` tools, **no permission prompt is shown** — the tool executes without asking. An agent configured with `default_action = "ask"` is silently downgraded to `"allow"` for safe and medium tools.

This is inconsistent with user expectations: if a user sets `default_action = "ask"` for an agent, they expect all tools to prompt, not just dangerous/destructive ones.

**Fix:**
In the `else` branch, check for agent_default source in addition to rule:
```go
} else {
    // Check if agent default action was "ask" — prompt even for safe tools
    if pctx != nil && pctx.Source == "agent_default" && pctx.RuleAction == "ask" {
        // Same permission prompt as the rule-based "ask" branch
        // ... (send PermissionRequest, wait for response)
    } else if risk == types.RiskDangerous || risk == types.RiskDestructive {
        // existing RiskLevel check
        ...
    }
}
```

Alternatively, handle all agent defaults (allow/deny/ask) before falling through to RiskLevel:
```go
if !allowedByRule {
    // Handle rule-based "ask"
    if pctx != nil && pctx.Source == "rule" && pctx.RuleAction == "ask" { ... }
    
    // Handle agent-based defaults
    if pctx != nil && pctx.Source == "agent_default" {
        switch pctx.RuleAction {
        case "allow": // proceed
        case "deny": return error
        case "ask":  // prompt
        }
        return // don't fall through to RiskLevel
    }
    
    // Fall through to RiskLevel for unmatched tools
    if risk == types.RiskDangerous || risk == types.RiskDestructive { ... }
}
```

---

### WR-04: `applyVarSubstitution` does not cover Agent profile `DefaultAction`

**File:** `internal/config/loader.go:applyVarSubstitution()` (lines 315-328)
**Severity:** WARNING — Incomplete feature

**Issue:**
The `applyVarSubstitution` function substitutes `${VAR}` in `Permissions.Rules[].Tool/Pattern/Action` (lines 323-327) but does **not** substitute in `Permissions.Agents[].DefaultAction`. The plan (11-02-PLAN.md §4.c) explicitly requires:
> Agent config `default_action` supports `${VAR}` substitution

The missing substitution means agent default actions cannot be configured via environment variables, while rule actions can.

**Fix:**
Add agent substitution after the rules loop:
```go
for name := range cfg.Permissions.Agents {
    agent := cfg.Permissions.Agents[name]
    agent.DefaultAction = substituteVars(agent.DefaultAction)
    for j := range agent.Rules {
        agent.Rules[j].Tool = substituteVars(agent.Rules[j].Tool)
        agent.Rules[j].Pattern = substituteVars(agent.Rules[j].Pattern)
        agent.Rules[j].Action = substituteVars(agent.Rules[j].Action)
    }
    cfg.Permissions.Agents[name] = agent
}
```

---

### WR-05: `matchAnyParamValue` silently skips non-string param values

**File:** `internal/tools/dispatcher.go:316-331`
**Severity:** WARNING — Silent correctness gap

**Issue:**
The function iterates `map[string]any` and only matches string values:
```go
str, ok := v.(string)
if !ok {
    continue  // silently skip numbers, booleans, nested objects, arrays
}
```

If a tool passes a numeric parameter (e.g., `count = 5`) or a boolean parameter (e.g., `recursive = true`), glob patterns like `*5*` or `*true*` will never match against those values. This is a silent correctness gap — users who write glob patterns expecting to match all input values may miss values that happen to be non-string.

**Fix:**
Convert all values to string before matching:
```go
func matchAnyParamValue(pattern string, params map[string]any) bool {
    for _, v := range params {
        var str string
        switch val := v.(type) {
        case string:
            str = val
        default:
            str = fmt.Sprintf("%v", val)
        }
        matched, err := doublestar.Match(pattern, str)
        if err != nil {
            continue
        }
        if matched {
            return true
        }
    }
    return false
}
```

---

### WR-06: Duplicate nil check in `TestCheckPermission_ToolFilter` (dead code)

**File:** `internal/tools/dispatcher_test.go:615-617`
**Severity:** WARNING — Dead code in test

**Issue:**
Lines 615-617 contain a duplicate nil check:
```go
614: if pctx == nil {
615:     t.Fatal("expected non-nil PermissionContext")
616: }
617: if pctx == nil {
618:     t.Fatal("expected non-nil PermissionContext")
619: }
620: if pctx.Source != "risk_level" {
```

The second `if pctx == nil` (line 617) is unreachable — if `pctx` were nil, the `t.Fatal` on line 615 would have already terminated the test.

**Fix:**
Remove the duplicate check at lines 617-619.

---

### WR-07: Test `TestHandlePrev_AtFirstSibling` tests root session, not first sibling

**File:** `internal/tui/commands_test.go` (search for `TestHandlePrev_AtFirstSibling`)
**Severity:** WARNING — Misleading test name, missing coverage

**Issue:**
The test creates a root session with no parent (`ParentID == ""`) and calls `/prev`, expecting an error. This tests the "root session has no siblings" case, not the "session has siblings but is already the first one" case. The test name implies it tests the latter scenario, which is untested.

**Fix:**
Rename to `TestHandlePrev_RootSession` (documents what it actually tests) and add a separate test:
```go
func TestHandlePrev_AtFirstSibling(t *testing.T) {
    // Create parent, then 3 children
    parent, _ := mgr.CreateSession(...)
    child1, _ := mgr.ForkSession(parent, "model1")
    child2, _ := mgr.ForkSession(parent, "model2")
    child3, _ := mgr.ForkSession(parent, "model3")
    
    // Navigate to child1 (first sibling) and try /prev
    sess, _ := mgr.LoadSession(child1.ID)
    ctx.Session = sess
    result, _ := handlePrev(ctx)
    
    // Should error — already at first sibling
    if result.Error == nil {
        t.Error("expected error for first sibling, got nil")
    }
}
```

---

## Info

### IN-01: `PermissionGate` interface defined but never used

**File:** `internal/tools/interface.go:34-37`
**Severity:** INFO — Dead code

**Issue:**
The `PermissionGate` interface is defined but never referenced by any type, function signature, struct field, or test. The actual permission gating logic lives directly in `Dispatcher` methods.

**Fix:**
Either remove the unused interface, or implement a `PermissionGate` that can be injected into `Dispatcher` to support test mocking or pluggable strategies.

---

### IN-02: `matchToolName` doc comment understates capability

**File:** `internal/tools/dispatcher.go:257-258, 305-312`
**Severity:** INFO — Documentation gap

**Issue:**
The comment at line 258 says "only match that tool name" (implying exact match), but the actual implementation uses `doublestar.Match` which supports glob patterns like `Bash*` or `*Write`. The function signature and behavior are correct (glob is more expressive), but the comment is misleading.

**Fix:**
Update the comment to reflect glob behavior:
```go
// Tool filter: if rule.Tool is set, match against tool name using glob semantics
```

---

### IN-03: Workflow integration tests pass `nil` for permission config

**Files:**
- `internal/workflow/engine_test.go:41`
- `internal/workflow/integration_test.go:81`
- `internal/workflow/verify_test.go:201`

**Severity:** INFO — Test coverage gap

**Issue:**
All three workflow test files create the dispatcher with `NewDispatcher(nil)`, meaning the rule-based permission code paths are never exercised during integration tests. If rule matching were to silently break (e.g., `matchAnyParamValue` returns wrong results, glob matching changes behavior), the workflow tests would not catch it.

**Fix:**
Add a test fixture with permission rules to one of the integration tests. For example, add a `deny` rule for a specific tool and verify the tool call triggers `ErrPermissionDenied`.

---

## Developer Notes

### Naming consistency

The `DefaultDispatcher` function registers tools using names embedded in the tool structs (via `tool.Name()`) rather than explicit name strings. Ensure all new tools implement `Name()` returning a unique, capitalized name. Current registrations are consistent (Bash, FileRead, FileWrite, Edit, TodoWrite, WebFetch, AskUserQuestion, Glob, Grep).

### Validation order sensitivity

If the substitution-before-validation swap (WR-02) is implemented, the `substituteVars` function preserves unresolved `${VAR}` patterns as-is. This means an unset variable like `${NONEXISTENT}` would pass validation as a literal string. If this is undesirable, validation should warn about unresolvable `${...}` patterns. This is a design trade-off, not a bug.

### RiskLevel field on PermissionRule

The `PermissionRule.RiskLevel` field is validated as a valid `types.RiskLevel` value but is never consumed by the permission check logic. It is stored in `PermissionContext` and passed through to `PermissionRequest` via the modal, but the actual permission decision depends on `Action` (allow/deny/ask), not `RiskLevel`. This is by design for future use.

---

_Reviewed: 2026-06-01T22:00:00Z_
_Reviewer: gsd-code-reviewer (deep mode)_
_Depth: deep (14 source files + 15 test files, cross-file call chain analysis)_
