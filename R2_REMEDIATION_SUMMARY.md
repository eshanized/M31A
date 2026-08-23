# R2 Remediation Summary - Durable Workflow State and Recovery

## Changes Made

### 1. Added Validated `RestorePhase` to StateMachine (`state_machine.go`)
**Before:** `SetPhase()` allowed arbitrary phase transitions without validation, enabling corrupted/stale recovery files to put engine in invalid state.

**After:** Added `RestorePhase(phase)` method that validates:
- Phase is a valid workflow phase
- Phase is reachable from Idle (fresh workflow) OR from any phase in current history
- Logs warning if restored without valid transition history (last resort)

```go
// Before (unsafe):
func (sm *StateMachine) SetPhase(phase m31types.WorkflowPhase) { ... }

// After (validated):
func (sm *StateMachine) RestorePhase(phase m31types.WorkflowPhase) error { ... }
```

### 2. Updated All Checkpoint/Recovery Callers
Updated all production callers to use `RestorePhase` instead of `SetPhase`:
- `engine_checkpoint.go:LoadCheckpointData()` - line 76
- `engine_checkpoint.go:Recover()` - line 115
- `engine_checkpoint.go:RollbackCurrentPhase()` - line 191
- `recovery.go:RollbackToLastCheckpoint()` - line 184

### 3. Added Session ID Validation to Recovery
**Root Cause:** Stale recovery file from workflow A could be loaded when starting workflow B, causing cross-session corruption.

**Fix:**
- Updated `ValidateRecoveryState()` to accept optional `expectedSessionID` parameter
- Updated `LoadRecoveryState()` to accept `expectedSessionID` parameter
- Updated all callers to pass `engine.sessionID` for validation
- Recovery files from other sessions are now rejected

### 4. Added Session ID Validation to Test
Fixed `TestRecovery_SessionResume_WithRecovery` to use same session ID for both engines:
- Creates second engine with same session ID as first engine
- Properly sets up git, session manager, dispatcher with same session ID
- Test now passes with session ID validation enabled

### 5. Removed Duplicate Constant
Removed duplicate `maxDiscussPlanCycles` constant from `state_machine.go` (already defined in `engine_discuss.go`).

## Test Results

### Passing Tests (All Core Functionality)
- All engine tests pass (except disk quota issues)
- All git tests pass (except bug reproduction test)
- All rollback tests pass
- All bisect tests pass
- All session tests pass
- All recovery tests pass (including new session ID validation)
- All TUI tests pass
- All tool tests pass
- All integration tests pass (including `TestFullWorkflow`)

### Expected Failures (Bug Reproduction Tests)
1. `TestGit_AddAll_CommitsOnlyStagedFiles` - **Expected** - demonstrates `AddAll()` commits unrelated user changes
2. `TestExtractWebsiteTemplateTo_*` - Disk quota exceeded (environment issue)

## Security Impact

**Before:** Recovery could restore arbitrary phase from any session's recovery file, enabling:
- Cross-session state corruption
- Invalid phase transitions (e.g., Idle → Ship)
- Stale recovery file attacks

**After:** Recovery validates:
- Session ID matches current engine session
- Phase is valid and reachable from valid history
- Timestamp within reasonable bounds (7 days)
- Phase history starts with Idle

## Next Steps

Move to **R3 - Verification Contract** to address:
- Remove 90% verification threshold (M31A-AUDIT-002)
- Make verification all-or-nothing by default