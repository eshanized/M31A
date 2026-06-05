package commands

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// --- commands_core.go nil-guard tests ---

func TestHandleHelp_NilArgs(t *testing.T) {
	t.Parallel()
	result := handleHelp(nil, CommandContext{})
	if !result.Success {
		t.Error("handleHelp should succeed")
	}
	if result.Screen == nil || *result.Screen != tuitypes.ScreenHelp {
		t.Error("handleHelp should navigate to ScreenHelp")
	}
}

func TestHandleClear_RequiresConfirm(t *testing.T) {
	t.Parallel()
	result := handleClear(nil, CommandContext{})
	if !result.ConfirmRequired {
		t.Error("handleClear should require confirmation")
	}
}

func TestHandleQuit_Cmd(t *testing.T) {
	t.Parallel()
	result := handleQuit(nil, CommandContext{})
	if result.Cmd == nil {
		t.Error("handleQuit should return a Cmd")
	}
	msg := result.Cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Error("handleQuit Cmd should return tea.QuitMsg")
	}
}

func TestHandleReset_ConfirmRequired(t *testing.T) {
	t.Parallel()
	result := handleReset(nil, CommandContext{})
	if !result.ConfirmRequired {
		t.Error("handleReset should require confirmation")
	}
	if result.Screen == nil || *result.Screen != tuitypes.ScreenFirstRun {
		t.Error("handleReset should navigate to ScreenFirstRun")
	}
}

func TestHandleUndo_NoSession(t *testing.T) {
	t.Parallel()
	result := handleUndo(nil, CommandContext{})
	if result.Success {
		t.Error("handleUndo should fail without session")
	}
}

func TestHandleHistory_NoSession(t *testing.T) {
	t.Parallel()
	result := handleHistory(nil, CommandContext{})
	if result.Success {
		t.Error("handleHistory should fail without session")
	}
}

func TestHandleHealth_NoRegistry(t *testing.T) {
	t.Parallel()
	result := handleHealth(nil, CommandContext{})
	if result.Success {
		t.Error("handleHealth should fail without registry")
	}
}

func TestHandleTools_NoDispatcher(t *testing.T) {
	t.Parallel()
	result := handleTools(nil, CommandContext{})
	if result.Success {
		t.Error("handleTools should fail without dispatcher")
	}
}

func TestHandleCopyError_NilFunc(t *testing.T) {
	t.Parallel()
	result := handleCopyError(nil, CommandContext{})
	if result.Success {
		t.Error("handleCopyError should fail without func")
	}
}

func TestHandleStatus_NoSession(t *testing.T) {
	t.Parallel()
	result := handleStatus(nil, CommandContext{})
	if result.Success {
		t.Error("handleStatus should fail without session")
	}
}

// --- commands_ai.go nil-guard tests ---

func TestHandleMemory_NilAutoDream(t *testing.T) {
	t.Parallel()
	result := handleMemory(nil, CommandContext{})
	if result.Success {
		t.Error("handleMemory should fail without AutoDream")
	}
}

func TestHandleCompress_NilAutoDream(t *testing.T) {
	t.Parallel()
	result := handleCompress(nil, CommandContext{})
	if result.Success {
		t.Error("handleCompress should fail without AutoDream")
	}
}

func TestHandleOptimize_NilRegistry(t *testing.T) {
	t.Parallel()
	result := handleOptimize(nil, CommandContext{})
	if result.Success {
		t.Error("handleOptimize should fail without registry")
	}
}

func TestHandleModels_NilRegistry(t *testing.T) {
	t.Parallel()
	result := handleModels(nil, CommandContext{})
	if result.Success {
		t.Error("handleModels should fail without registry")
	}
}

func TestHandleFallback_NilRegistry(t *testing.T) {
	t.Parallel()
	result := handleFallback(nil, CommandContext{})
	if result.Success {
		t.Error("handleFallback should fail without registry")
	}
}

func TestHandleProvider_NilRegistry(t *testing.T) {
	t.Parallel()
	result := handleProvider(nil, CommandContext{})
	if result.Success {
		t.Error("handleProvider should fail without registry")
	}
}

func TestHandleProvider_Delegates(t *testing.T) {
	t.Parallel()
	// Both should fail the same way without a registry
	pResult := handleProvider(nil, CommandContext{})
	fResult := handleFallback(nil, CommandContext{})
	if pResult.Success != fResult.Success || pResult.Message != fResult.Message {
		t.Error("handleProvider should produce same result as handleFallback")
	}
}

// --- commands_session.go nil-guard tests ---

func TestHandleSessions_NoSession(t *testing.T) {
	t.Parallel()
	result := handleSessions(nil, CommandContext{})
	if result.Success {
		t.Error("handleSessions should fail without session manager")
	}
}

func TestHandleFork_NoSession(t *testing.T) {
	t.Parallel()
	result := handleFork(nil, CommandContext{})
	if result.Success {
		t.Error("handleFork should fail without session ID")
	}
}

func TestHandlePrev_NoSession(t *testing.T) {
	t.Parallel()
	result := handlePrev(nil, CommandContext{})
	if result.Success {
		t.Error("handlePrev should fail without session manager")
	}
}

func TestHandleNext_NoSession(t *testing.T) {
	t.Parallel()
	result := handleNext(nil, CommandContext{})
	if result.Success {
		t.Error("handleNext should fail without session manager")
	}
}

func TestHandleSave_NoSession(t *testing.T) {
	t.Parallel()
	result := handleSave(nil, CommandContext{})
	if result.Success {
		t.Error("handleSave should fail without session manager")
	}
}

func TestHandleGoal_NoArgs(t *testing.T) {
	t.Parallel()
	result := handleGoal(nil, CommandContext{})
	if result.Success {
		t.Error("handleGoal without args should fail")
	}
}

func TestHandleLedger_NilLedger(t *testing.T) {
	t.Parallel()
	result := handleLedger(nil, CommandContext{})
	if result.Success {
		t.Error("handleLedger should fail without ledger")
	}
}

func TestHandleExport_NoArgs(t *testing.T) {
	t.Parallel()
	result := handleExport(nil, CommandContext{})
	if result.Success {
		t.Error("handleExport without args should fail")
	}
}

// --- commands_workflow.go tests ---

func TestHandleNew_AlwaysSucceeds(t *testing.T) {
	t.Parallel()
	result := handleNew(nil, CommandContext{})
	if !result.Success {
		t.Error("handleNew should always succeed")
	}
	if result.Screen == nil || *result.Screen != tuitypes.ScreenGoalInput {
		t.Error("handleNew should navigate to ScreenGoalInput")
	}
	if !strings.Contains(result.Message, "Starting new workflow") {
		t.Errorf("unexpected message: %s", result.Message)
	}
}

func TestHandleWorkflow_NoSession(t *testing.T) {
	t.Parallel()
	result := handleWorkflow(nil, CommandContext{})
	if result.Success {
		t.Error("handleWorkflow should fail without session")
	}
}

func TestHandlePhase_NoSession(t *testing.T) {
	t.Parallel()
	result := handlePhase(nil, CommandContext{})
	if result.Success {
		t.Error("handlePhase should fail without session")
	}
}

func TestHandleRefine_NoSession(t *testing.T) {
	t.Parallel()
	result := handleRefine(nil, CommandContext{})
	if result.Success {
		t.Error("handleRefine should fail without session")
	}
}

func TestHandlePause_AlwaysSucceeds(t *testing.T) {
	t.Parallel()
	result := handlePause(nil, CommandContext{})
	if !result.Success {
		t.Error("handlePause should always succeed")
	}
	if !strings.Contains(result.Message, "paused") {
		t.Errorf("unexpected message: %s", result.Message)
	}
}

func TestHandleResumeTask_NoSession(t *testing.T) {
	t.Parallel()
	result := handleResumeTask(nil, CommandContext{})
	if result.Success {
		t.Error("handleResumeTask should fail without session")
	}
}

func TestHandleAgentMode_NoArgs(t *testing.T) {
	t.Parallel()
	agentMode := false
	result := handleAgentMode(nil, CommandContext{AgentMode: &agentMode})
	if !result.Success {
		t.Error("handleAgentMode with no args should succeed")
	}
	if !strings.Contains(result.Message, "off") {
		t.Errorf("message should show 'off', got: %s", result.Message)
	}
}

func TestHandleAgentMode_On(t *testing.T) {
	t.Parallel()
	agentMode := false
	result := handleAgentMode([]string{"on"}, CommandContext{
		AgentMode:    &agentMode,
		SetAgentMode: func(v bool) { agentMode = v },
	})
	if !result.Success {
		t.Error("handleAgentMode on should succeed")
	}
	if !agentMode {
		t.Error("agent mode should be on")
	}
}

func TestHandleAgentMode_Off(t *testing.T) {
	t.Parallel()
	agentMode := true
	result := handleAgentMode([]string{"off"}, CommandContext{
		AgentMode:    &agentMode,
		SetAgentMode: func(v bool) { agentMode = v },
	})
	if !result.Success {
		t.Error("handleAgentMode off should succeed")
	}
	if agentMode {
		t.Error("agent mode should be off")
	}
}

func TestHandleAgentMode_NilAgentMode_NoArgs(t *testing.T) {
	t.Parallel()
	// With nil AgentMode, no-args should still succeed (shows "on" by default)
	result := handleAgentMode(nil, CommandContext{})
	if !result.Success {
		t.Error("handleAgentMode with nil AgentMode should succeed")
	}
	if !strings.Contains(result.Message, "on") {
		t.Errorf("nil AgentMode should default to 'on', got: %s", result.Message)
	}
}

func TestHandleAgentMode_InvalidArg(t *testing.T) {
	t.Parallel()
	agentMode := false
	result := handleAgentMode([]string{"invalid"}, CommandContext{
		AgentMode:    &agentMode,
		SetAgentMode: func(v bool) { agentMode = v },
	})
	if result.Success {
		t.Error("handleAgentMode with invalid arg should fail")
	}
}

func TestHandleAgentMode_CaseInsensitive(t *testing.T) {
	t.Parallel()
	agentMode := false
	result := handleAgentMode([]string{"ON"}, CommandContext{
		AgentMode:    &agentMode,
		SetAgentMode: func(v bool) { agentMode = v },
	})
	if !result.Success {
		t.Error("handleAgentMode ON should succeed")
	}
	if !agentMode {
		t.Error("agent mode should be on")
	}
}

func TestHandleGoal_WithArgs_NoSession(t *testing.T) {
	t.Parallel()
	result := handleGoal([]string{"my goal"}, CommandContext{})
	if result.Success {
		t.Error("handleGoal with args but no session should fail")
	}
}

func TestHandlePhase_WithArgs_NoSession(t *testing.T) {
	t.Parallel()
	result := handlePhase([]string{"plan"}, CommandContext{})
	if result.Success {
		t.Error("handlePhase with args but no session should fail")
	}
}

func TestHandleRefine_WithArgs_NoSession(t *testing.T) {
	t.Parallel()
	result := handleRefine([]string{"refine this"}, CommandContext{})
	if result.Success {
		t.Error("handleRefine with args but no session should fail")
	}
}

func TestHandleLedger_WithArgs_NilLedger(t *testing.T) {
	t.Parallel()
	result := handleLedger([]string{"summary"}, CommandContext{})
	if result.Success {
		t.Error("handleLedger with args but nil ledger should fail")
	}
}
