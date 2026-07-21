package commands

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
)

// --- handleHelp ---

func TestHandleHelp(t *testing.T) {
	t.Parallel()
	result := handleHelp(nil, CommandContext{})
	if !result.Success {
		t.Error("handleHelp should succeed")
	}
	if result.Screen == nil {
		t.Fatal("handleHelp should set Screen")
	}
}

// --- handleClear ---

func TestHandleClear(t *testing.T) {
	t.Parallel()
	result := handleClear(nil, CommandContext{})
	if !result.Success {
		t.Error("handleClear should succeed")
	}
	if !result.ConfirmRequired {
		t.Error("handleClear should require confirmation")
	}
	if result.ConfirmPrompt == "" {
		t.Error("handleClear should have confirmation prompt")
	}
}

// --- handleStatus ---

func TestHandleStatus_NoSession(t *testing.T) {
	t.Parallel()
	result := handleStatus(nil, CommandContext{})
	if result.Success {
		t.Error("handleStatus should fail without session")
	}
	if result.Message != "No active session." {
		t.Errorf("Message = %q, want 'No active session.'", result.Message)
	}
}

func TestHandleStatus_NilManager(t *testing.T) {
	t.Parallel()
	ctx := CommandContext{SessionID: "sess1"}
	result := handleStatus(nil, ctx)
	if result.Success {
		t.Error("handleStatus should fail with nil manager")
	}
}

// --- handleReset ---

func TestHandleReset(t *testing.T) {
	t.Parallel()
	result := handleReset(nil, CommandContext{})
	if !result.Success {
		t.Error("handleReset should succeed")
	}
	if !result.ConfirmRequired {
		t.Error("handleReset should require confirmation")
	}
}

// --- handleQuit ---

func TestHandleQuit(t *testing.T) {
	t.Parallel()
	result := handleQuit(nil, CommandContext{})
	if !result.Success {
		t.Error("handleQuit should succeed")
	}
	if result.Message != "Goodbye!" {
		t.Errorf("Message = %q, want Goodbye!", result.Message)
	}
	if result.Cmd == nil {
		t.Error("handleQuit should set Cmd")
	}
}

// --- handleExit ---

func TestHandleExit(t *testing.T) {
	t.Parallel()
	result := handleExit(nil, CommandContext{})
	if !result.Success {
		t.Error("handleExit should succeed")
	}
	if result.Message != "Goodbye!" {
		t.Errorf("Message = %q, want Goodbye!", result.Message)
	}
}

// --- handleHistory ---

func TestHandleHistory(t *testing.T) {
	t.Parallel()
	result := handleHistory(nil, CommandContext{})
	if !result.Success {
		t.Error("handleHistory should succeed")
	}
	if result.Screen == nil {
		t.Fatal("handleHistory should set Screen")
	}
}

// --- handlePromptHistory ---

func TestHandlePromptHistory_NilHistory(t *testing.T) {
	t.Parallel()
	result := handlePromptHistory(nil, CommandContext{})
	if result.Success {
		t.Error("handlePromptHistory should fail with nil history")
	}
	if result.Message != "Prompt history not available." {
		t.Errorf("Message = %q", result.Message)
	}
}

// --- handleChat ---

func TestHandleChat(t *testing.T) {
	t.Parallel()
	result := handleChat(nil, CommandContext{})
	if !result.Success {
		t.Error("handleChat should succeed")
	}
	if !result.ConfirmRequired {
		t.Error("handleChat should require confirmation")
	}
}

// --- handleFlush ---

func TestHandleFlush(t *testing.T) {
	t.Parallel()
	result := handleFlush(nil, CommandContext{})
	if !result.Success {
		t.Error("handleFlush should succeed")
	}
	if result.Message != "Screen flushed." {
		t.Errorf("Message = %q, want Screen flushed.", result.Message)
	}
}

// --- handleSearch ---

func TestHandleSearch_NoArgs(t *testing.T) {
	t.Parallel()
	result := handleSearch(nil, CommandContext{})
	if result.Success {
		t.Error("handleSearch should fail without args")
	}
	if result.Message != "Usage: `/search <query>`" {
		t.Errorf("Message = %q", result.Message)
	}
}

func TestHandleSearch_NoSession(t *testing.T) {
	t.Parallel()
	result := handleSearch([]string{"query"}, CommandContext{})
	if result.Success {
		t.Error("handleSearch should fail without session")
	}
}

// --- handleAbout ---

func TestHandleAbout(t *testing.T) {
	t.Parallel()
	result := handleAbout(nil, CommandContext{Version: "1.0.0"})
	if !result.Success {
		t.Error("handleAbout should succeed")
	}
	if result.Message == "" {
		t.Error("handleAbout should have message")
	}
}

func TestHandleAbout_NoVersion(t *testing.T) {
	t.Parallel()
	result := handleAbout(nil, CommandContext{})
	if !result.Success {
		t.Error("handleAbout should succeed")
	}
	if result.Message == "" {
		t.Error("handleAbout should have message")
	}
}

// --- handleUndo ---

func TestHandleUndo_NoSession(t *testing.T) {
	t.Parallel()
	result := handleUndo(nil, CommandContext{})
	if result.Success {
		t.Error("handleUndo should fail without session")
	}
	if result.Message != "No active session." {
		t.Errorf("Message = %q, want 'No active session.'", result.Message)
	}
}

// --- handleSettings ---

func TestHandleSettings(t *testing.T) {
	t.Parallel()
	result := handleSettings(nil, CommandContext{})
	if !result.Success {
		t.Error("handleSettings should succeed")
	}
	if result.Screen == nil {
		t.Fatal("handleSettings should set Screen")
	}
}

// --- handleConfig ---

func TestHandleConfig(t *testing.T) {
	t.Parallel()
	result := handleConfig(nil, CommandContext{})
	if !result.Success {
		t.Error("handleConfig should succeed")
	}
	if result.Screen == nil {
		t.Fatal("handleConfig should set Screen")
	}
}

// --- handleCost ---

func TestHandleCost_NilConfig(t *testing.T) {
	t.Parallel()
	result := handleCost(nil, CommandContext{})
	if result.Success {
		t.Error("handleCost should fail with nil config")
	}
	if result.Message != "Config not available." {
		t.Errorf("Message = %q", result.Message)
	}
}

func TestHandleCost_Toggle(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{}
	cfg.UI.ShowCostEstimate = false
	ctx := CommandContext{Config: cfg}
	result := handleCost(nil, ctx)
	if !result.Success {
		t.Error("handleCost should succeed")
	}
	if !cfg.UI.ShowCostEstimate {
		t.Error("handleCost should toggle ShowCostEstimate to true")
	}
}

func TestHandleCost_ToggleOff(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{}
	cfg.UI.ShowCostEstimate = true
	ctx := CommandContext{Config: cfg}
	result := handleCost(nil, ctx)
	if !result.Success {
		t.Error("handleCost should succeed")
	}
	if cfg.UI.ShowCostEstimate {
		t.Error("handleCost should toggle ShowCostEstimate to false")
	}
}

// --- handleLog ---

func TestHandleLog_NoLogFile(t *testing.T) {
	t.Parallel()
	result := handleLog(nil, CommandContext{})
	if !result.Success {
		t.Error("handleLog should succeed even without log file")
	}
}

// --- handleKey ---

func TestHandleKey_NilConfig(t *testing.T) {
	t.Parallel()
	result := handleKey(nil, CommandContext{})
	if result.Success {
		t.Error("handleKey should fail with nil config")
	}
}

func TestHandleKey_WithConfig(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{}
	ctx := CommandContext{Config: cfg}
	result := handleKey(nil, ctx)
	if !result.Success {
		t.Error("handleKey should succeed")
	}
}

// --- handleTokens ---

func TestHandleTokens_NoSession(t *testing.T) {
	t.Parallel()
	result := handleTokens(nil, CommandContext{})
	if result.Success {
		t.Error("handleTokens should fail without session")
	}
	if result.Message != "No active session." {
		t.Errorf("Message = %q", result.Message)
	}
}

// --- handleCopyError ---

func TestHandleCopyError_NilCopyError(t *testing.T) {
	t.Parallel()
	result := handleCopyError(nil, CommandContext{})
	if result.Success {
		t.Error("handleCopyError should fail with nil CopyError")
	}
}

// --- diskUsageFormatted ---

func TestDiskUsageFormatted_NonexistentPath(t *testing.T) {
	t.Parallel()
	result := diskUsageFormatted("/nonexistent/path/that/does/not/exist")
	if result != "unknown" {
		t.Errorf("diskUsageFormatted for nonexistent path = %q, want 'unknown'", result)
	}
}

// --- readTailLines ---

func TestReadTailLines_NonexistentFile(t *testing.T) {
	t.Parallel()
	_, err := readTailLines("/nonexistent/file.log", 10)
	if err == nil {
		t.Error("readTailLines should fail for nonexistent file")
	}
}

// --- handleCopyError with func ---

func TestHandleCopyError_NilFuncPath(t *testing.T) {
	t.Parallel()
	// Verify nil CopyError returns failure
	result := handleCopyError(nil, CommandContext{})
	if result.Success {
		t.Error("handleCopyError should fail with nil CopyError func")
	}
}

// --- handleHealth ---

func TestHandleHealth_NilRegistry(t *testing.T) {
	t.Parallel()
	result := handleHealth(nil, CommandContext{})
	if result.Success {
		t.Error("handleHealth should fail with nil registry")
	}
	if result.Message != "Provider registry not available." {
		t.Errorf("Message = %q", result.Message)
	}
}

// --- handleTools ---

func TestHandleTools_NilDispatcher(t *testing.T) {
	t.Parallel()
	result := handleTools(nil, CommandContext{})
	if result.Success {
		t.Error("handleTools should fail with nil dispatcher")
	}
	if result.Message != "Tool dispatcher not available." {
		t.Errorf("Message = %q", result.Message)
	}
}

// --- handleUndo ---

func TestHandleUndo_NilManager(t *testing.T) {
	t.Parallel()
	ctx := CommandContext{SessionID: "sess1"}
	result := handleUndo(nil, ctx)
	if result.Success {
		t.Error("handleUndo should fail with nil manager")
	}
	if result.Message != "No active session." {
		t.Errorf("Message = %q", result.Message)
	}
}

// --- handleStatus with config ---

func TestHandleStatus_WithConfig(t *testing.T) {
	t.Parallel()
	result := handleStatus(nil, CommandContext{})
	if result.Success {
		t.Error("handleStatus should fail without session manager")
	}
}
