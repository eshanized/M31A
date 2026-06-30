# M31A v1.6.0 – Autonomy Implementation

This implementation covers Waves 1-3.1 of the Autonomy plan:

## Waves Complete

### Wave 1: Deferred Tools + Emitter Reliability
✅ Emitter Reliability (Channel 128→, bounded retry, drop counter, adaptive drain)
✅ Batch Permission Approval (task-scoped, `b`/`B` keybinding, `/pending` command, sidebar badge)

### Wave 2: Adaptive Context Budgeting
✅ Proactive Context Warning (70%, 85% toasts, auto-suggest)
✅ Adaptive Context Allocation (phase-aware ratios, proactive compaction)
✅ Context Budget Visibility (sidebar display, compaction events, forecasting)

### Wave 3.1: Ghost Mode (Headless Execution)
✅ Headless Workflow Engine (`--goal` flag, phase execution with goal)
✅ Headless Progress Display (structured output, progress formatting)

## Wave 3.2: Ghost Mode Permission Integration (Planned)

### Objective:
Implement non-interactive permission handling for headless execution. Wire the permission policy from CLI flags into the dispatcher, add policy types (`AllowSafe`, `DenyAll`, `Prompt`), and handle headless permissions without TUI modals.

### Technical Implementation Plan:

1. Add `PermissionPolicy` type to `internal/tools/permissions.go`:
   - `AllowSafe`: Auto-approve tools with risk < dangerous
   - `DenyAll`: Block all permission-requiring tools (safe mode)
   - `Prompt`: Fall back to stdin prompt (for manual use)

2. Modify `ensurePermission()` to respect headless policy before TUI:
   - Check policy in `permissions.go`
   - Auto-approve or deny based on policy
   - Log all decisions for audit trail

3. Wire policy from CLI to dispatcher:
   - Pass via `engine.RunPhase()` to dispatcher
   - Set policy based on `--permission-policy` CLI flag
   - Default to `DenyAll` for safety

4. Add to `cmd/m31a/main.go`:
   - New `--permission-policy` flag
   - Acceptable values: `allow-safe`, `deny-all`, `prompt`
   - Default: `deny-all`

5. Update headless mode flow in `cmd/m31a/main.go`:
   - Pass policy to `runHeadlessWorkflow`
   - Setup headless permission policy
   - Connect to dispatcher

6. Add to `internal/tools/dispatcher.go`:
   - Add `permissionPolicy` field to Dispatcher struct
   - Set policy from workflow engine
   - Use policy in `ensurePermission()`

7. Update `internal/workflow/engine.go`:
   - Pass headless policy from CLI to workflow engine
   - Propagate to dispatcher
   - Use policy in RunPhase

### Status:
**Investigation Required** – Need to examine existing permission system architecture to design minimal, non-breaking changes for Wave 3.2. Key questions:
- Current permission types and when they're used
- How tools handle permissions
- Existing configuration options for headless mode

Next steps: Explore existing permission architecture before implementing.

## Current Implementation Status

**Builds:** ✅ All clean
**Tests:** ✅ All passing
**Documentation:** ✅ Plan updated (1_6_0_autonomy_plan.md)

**CLI Commands:**
- `m31a --help` – shows all existing commands
- `m31a --prompt "hello"` – headless single prompt (existing)
- `m31a --goal "create file"` – headless workflow execution (Wave 3.1)

**Features:**
- Deferred tools: Per-tool "remember" cache, concurrent request routing, persistent permissions
- Batch approval: `/pending` command shows queue, `b`/`B` keybatch approves
- Context budgeting: 70%/85% warnings, sidebar context display, proactive compaction
- Headless execution: `--goal` flag for workflow execution

**Next Development Phase:** Wave 3.2 (Ghost Mode Permission Integration)

## Key Files Modified

**Wave 1**
- internal/tools/permissions.go
- internal/tools/interface.go
- internal/tools/dispatcher.go
- internal/tools/permissions_test.go

**Wave 2**
- internal/workflow/engine.go (proactiveCompactCheck)
- internal/workflow/execute.go (toolCallsSinceLastCompact)
- internal/tui/app_handlers_misc.go (checkContextWarnings)
- internal/tui/sidebar_model.go
- pkg/compaction/template.go

**Wave 3.1**
- cmd/m31a/main.go (goal flag)
- internal/tui/app_update.go (checkContextWarnings integration)

## Testing

All tests pass:
```bash
$ go test ./internal/tools
ok  github.com/eshanized/M31A/internal/tools 53.969s
$ go test ./internal/workflow
ok  github.com/eshanized/M31A/internal/workflow 5.209s
$ go test ./internal/tui
ok  github.com/eshanized/M31A/internal/tui 1.782s
$ go test -race ./... (race detection clean)
```

## Usage Examples

### Single Prompt (Headless)
```bash
# Existing headless mode
m31a --prompt "What files are in the project?"
```

### Workflow Execution (Wave 3.1)
```bash
# New headless workflow mode
m31a --goal "Create a REST API with authentication and database"
```

### TUI Mode
```bash
# Full TUI mode (all features)
m31a
```

## Restrictions Not Yet Implemented

- Wave 3.2: Permission policy integration
- Wave 4: Cross-session knowledge
- Engine struct decomposition (v1.7-v1.8)
- TUI Update dispatch refactoring (v1.7)

---

M31A v1.6.0 Autonomy Implementation in progress
