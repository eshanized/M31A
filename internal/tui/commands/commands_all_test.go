package commands

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/autodream"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/ledger"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// ─── ParseCommand tests ──────────────────────────────────────────────────────

func TestParseCommand_VariousInputs(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		input    string
		wantName string
		wantArgs []string
		wantOK   bool
	}{
		{"empty prefix", "", "", nil, false},
		{"bare slash", "/", "", nil, true},
		{"single command", "/help", "help", nil, true},
		{"command with multiple args", "/diff HEAD~1 HEAD", "diff", []string{"HEAD~1", "HEAD"}, true},
		{"non-slash input", "hello", "", nil, false},
		{"extra spaces", "/model  ", "model", nil, true},
		{"empty after trim", "/   ", "", nil, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			name, args, ok := ParseCommand(tt.input)
			if ok != tt.wantOK {
				t.Errorf("ok = %v, want %v", ok, tt.wantOK)
			}
			if name != tt.wantName {
				t.Errorf("name = %q, want %q", name, tt.wantName)
			}
			if len(args) != len(tt.wantArgs) {
				t.Errorf("args = %v, want %v", args, tt.wantArgs)
			} else {
				for i := range args {
					if args[i] != tt.wantArgs[i] {
						t.Errorf("args[%d] = %q, want %q", i, args[i], tt.wantArgs[i])
					}
				}
			}
		})
	}
}

// ─── Registry tests ──────────────────────────────────────────────────────────

func TestDefaultCommands_AllRegistered(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	expected := []string{
		"help", "clear", "status", "reset", "quit", "exit", "chat", "flush", "search", "about",
		"undo", "history", "prompt-history", "health", "tools", "copy-error",
		"settings", "config", "cost", "log", "key", "keychain", "tokens", "dream",
		"compress", "memory", "optimize", "model", "fallback", "provider",
		"diff", "rollback", "bisect",
		"sessions", "export", "fork", "prev", "next", "save", "goal", "resume", "ledger",
		"new", "workflow", "plan", "refine", "execute", "verify", "runtime", "ship", "phase", "pause", "pending", "resume-task", "agent-mode",
		"metrics", "dashboard", "notifications", "files", "ghost",
		"getting-started", "quick", "skip",
		"agent", "agent-cancel", "complexity", "decisions",
	}
	registered := r.List()
	if len(registered) != len(expected) {
		t.Errorf("registered %d commands, expected %d\nGot: %v", len(registered), len(expected), registered)
	}
	for _, name := range expected {
		if _, ok := r.Get(name); !ok {
			t.Errorf("expected command %q not registered", name)
		}
	}
}

func TestRegistry_DuplicateRegister(t *testing.T) {
	t.Parallel()
	r := NewCommandRegistry()
	handler := func(_ []string, _ CommandContext) CommandResult { return CommandResult{} }
	if err := r.Register("test", handler, "desc"); err != nil {
		t.Fatalf("first register failed: %v", err)
	}
	if err := r.Register("test", handler, "desc2"); err == nil {
		t.Error("duplicate register should fail")
	}
}

func TestRegistry_Execute_EmptyCommand(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/", CommandContext{})
	if !handled {
		t.Error("bare / should be handled")
	}
	if result.Success {
		t.Error("bare / should fail")
	}
}

func TestRegistry_Execute_UnknownCommand(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/foobar", CommandContext{})
	if !handled {
		t.Error("unknown command should be handled")
	}
	if result.Success {
		t.Error("unknown command should fail")
	}
}

func TestSuggestCommand_CloseMatch(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	tests := []struct {
		input string
		want  string
	}{
		{"hep", "help"},
		{"statu", "status"},
		{"modl", "model"},
		{"cler", "clear"},
		{"quit", "exit"},
	}
	for _, tt := range tests {
		got := suggestCommand(r, tt.input)
		if got != tt.want {
			t.Errorf("suggestCommand(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestLevenshtein_Distances(t *testing.T) {
	t.Parallel()
	tests := []struct {
		a, b string
		want int
	}{
		{"", "", 0},
		{"abc", "abc", 0},
		{"abc", "ab", 1},
		{"abc", "aXc", 1},
		{"kitten", "sitting", 3},
		{"", "abc", 3},
		{"abc", "", 3},
	}
	for _, tt := range tests {
		got := levenshtein(tt.a, tt.b)
		if got != tt.want {
			t.Errorf("levenshtein(%q, %q) = %d, want %d", tt.a, tt.b, got, tt.want)
		}
	}
}

// ─── Core commands — extended tests ──────────────────────────────────────────

func TestHandleClear_ExecutesClearMessages(t *testing.T) {
	t.Parallel()
	cleared := false
	r := handleClear(nil, CommandContext{
		ClearMessages: func() { cleared = true },
	})
	if r.Cmd == nil {
		t.Fatal("Cmd is nil")
	}
	r.Cmd()
	if !cleared {
		t.Error("ClearMessages was not called")
	}
}

func TestHandleStatus_NilSessionManager(t *testing.T) {
	t.Parallel()
	r := handleStatus(nil, CommandContext{SessionManager: nil, SessionID: "test"})
	if r.Success {
		t.Error("should fail with nil SessionManager")
	}
}

func TestHandleCopyError_WithFunc(t *testing.T) {
	t.Parallel()
	cmdCalled := false
	r := handleCopyError(nil, CommandContext{
		CopyError: func() tea.Cmd {
			return func() tea.Msg {
				cmdCalled = true
				return nil
			}
		},
	})
	if !r.Success || r.Cmd == nil {
		t.Error("should succeed with Cmd")
	}
	r.Cmd()
	if !cmdCalled {
		t.Error("CopyError cmd was not called")
	}
}

// ─── Config commands — extended tests ────────────────────────────────────────

func TestHandleCost_TogglesToFalse(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{UI: config.UIConfig{ShowCostEstimate: true}}
	r := handleCost(nil, CommandContext{Config: cfg})
	if !r.Success {
		t.Error("should succeed")
	}
	if cfg.UI.ShowCostEstimate {
		t.Error("should have toggled to false")
	}
}

func TestHandleCost_TogglesToTrue(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{UI: config.UIConfig{ShowCostEstimate: false}}
	r := handleCost(nil, CommandContext{Config: cfg})
	if !r.Success {
		t.Error("should succeed")
	}
	if !cfg.UI.ShowCostEstimate {
		t.Error("should have toggled to true")
	}
}

func TestHandleLog_WithLineCount(t *testing.T) {
	t.Parallel()
	r := handleLog([]string{"5"}, CommandContext{})
	if !r.Success {
		t.Errorf("handleLog with line count should succeed: %s", r.Message)
	}
}

func TestHandleKey_WithBothKeys(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{Provider: config.ProviderConfig{
		OpenRouter: config.ProviderCredentialConfig{APIKey: "or-key"},
		Zen:        config.ProviderCredentialConfig{APIKey: "zen-key"},
	}}
	r := handleKey(nil, CommandContext{Config: cfg})
	if !r.Success {
		t.Error("should succeed")
	}
	if !containsSubstring(r.Message, "OpenRouter") || !containsSubstring(r.Message, "Zen") {
		t.Error("should mention both providers")
	}
}

func TestHandleKey_OpenRouterOnly(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{Provider: config.ProviderConfig{
		OpenRouter: config.ProviderCredentialConfig{APIKey: "or-key"},
	}}
	r := handleKey(nil, CommandContext{Config: cfg})
	if !r.Success {
		t.Error("should succeed")
	}
	if !containsSubstring(r.Message, "set") {
		t.Error("should mention key status")
	}
}

func TestHandleKey_NeitherKey(t *testing.T) {
	t.Parallel()
	cfg := &config.Config{Provider: config.ProviderConfig{}}
	r := handleKey(nil, CommandContext{Config: cfg})
	if !r.Success {
		t.Error("should succeed")
	}
	if !containsSubstring(r.Message, "not set") {
		t.Error("should mention keys not set")
	}
}

// ─── AI/model commands — extended tests ──────────────────────────────────────

func TestHandleMemory_AllSubcommands(t *testing.T) {
	t.Parallel()
	ad := autodream.New(nil)
	tests := []struct {
		subcmd string
		args   []string
		wantOK bool
	}{
		{"view", []string{"view"}, true},
		{"pause", []string{"pause"}, true},
		{"resume", []string{"resume"}, true},
		{"revert", []string{"revert"}, true},
		{"invalid", []string{"invalid"}, false},
		{"no args defaults to view", nil, true},
	}
	for _, tt := range tests {
		t.Run(tt.subcmd, func(t *testing.T) {
			t.Parallel()
			r := handleMemory(tt.args, CommandContext{AutoDream: ad})
			if r.Success != tt.wantOK {
				t.Errorf("memory %q: success=%v, want %v, msg=%s", tt.subcmd, r.Success, tt.wantOK, r.Message)
			}
		})
	}
}

func TestHandleModel_OpensScreen(t *testing.T) {
	t.Parallel()
	r := handleModel(nil, CommandContext{})
	if !r.Success || r.Screen == nil || *r.Screen != tuitypes.ScreenModelSelector {
		t.Errorf("handleModel: got %+v", r)
	}
}

// ─── Git commands — extended tests ───────────────────────────────────────────

func TestHandleDiff_NilGit(t *testing.T) {
	t.Parallel()
	r := handleDiff(nil, CommandContext{})
	if r.Success {
		t.Error("should fail without git")
	}
}

func TestHandleRollback_NilRollback(t *testing.T) {
	t.Parallel()
	r := handleRollback(nil, CommandContext{})
	if r.Success {
		t.Error("should fail without rollback")
	}
}

func TestHandleBisect_NilGit(t *testing.T) {
	t.Parallel()
	r := handleBisect(nil, CommandContext{})
	if r.Success {
		t.Error("should fail without git")
	}
}

// ─── Session commands — extended tests ───────────────────────────────────────

func TestHandleFork_Disabled(t *testing.T) {
	t.Parallel()
	r := handleFork(nil, CommandContext{})
	if r.Success {
		t.Error("fork should be disabled")
	}
	if !containsSubstring(r.Message, "not available") {
		t.Error("should explain why disabled")
	}
}

func TestHandlePrev_Disabled(t *testing.T) {
	t.Parallel()
	r := handlePrev(nil, CommandContext{})
	if r.Success {
		t.Error("prev should be disabled")
	}
}

func TestHandleNext_Disabled(t *testing.T) {
	t.Parallel()
	r := handleNext(nil, CommandContext{})
	if r.Success {
		t.Error("next should be disabled")
	}
}

func TestHandleGoal_SetWithArgs_NoSession(t *testing.T) {
	t.Parallel()
	r := handleGoal([]string{"set", "my", "goal"}, CommandContext{})
	if r.Success {
		t.Error("should fail without session")
	}
}

func TestHandleLedger_StatsWithLedger(t *testing.T) {
	t.Parallel()
	l := &ledger.Ledger{}
	r := handleLedger([]string{"stats"}, CommandContext{Ledger: l})
	if !r.Success {
		t.Errorf("ledger stats should succeed: %s", r.Message)
	}
}

func TestHandleExport_InvalidFormat(t *testing.T) {
	t.Parallel()
	r := handleExport([]string{"yaml"}, CommandContext{
		SessionManager: &session.Manager{},
		SessionID:      "test",
	})
	if r.Success {
		t.Error("should fail for invalid format")
	}
}

// ─── Workflow commands — extended tests ──────────────────────────────────────

func TestHandlePhase_NoArgs_NoSession(t *testing.T) {
	t.Parallel()
	r := handlePhase(nil, CommandContext{})
	if r.Success {
		t.Error("should fail without session")
	}
}

func TestHandlePhase_AllPhases(t *testing.T) {
	t.Parallel()
	phases := []string{"discuss", "plan", "execute", "verify", "runtime", "ship", "idle"}
	for _, p := range phases {
		t.Run(p, func(t *testing.T) {
			t.Parallel()
			r := handlePhase([]string{p}, CommandContext{})
			// Should fail without session, but phase name is valid
			if r.Success {
				t.Error("should fail without session")
			}
		})
	}
}

func TestHandlePhase_InvalidPhaseName(t *testing.T) {
	t.Parallel()
	r := handlePhase([]string{"bogus"}, CommandContext{
		SessionManager: &session.Manager{},
		SessionID:      "test",
	})
	// Will fail because LoadWorkflowState fails on empty manager
	if r.Success {
		t.Error("expected handlePhase to fail with bogus phase name")
	}
}

func TestHandlePause_WithAutoDream(t *testing.T) {
	t.Parallel()
	ad := autodream.New(nil)
	r := handlePause(nil, CommandContext{AutoDream: ad})
	if !r.Success {
		t.Error("pause with autodream should succeed")
	}
}

func TestHandleAgentMode_AllTransitions(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name    string
		args    []string
		initial bool
		wantOK  bool
	}{
		{"on from false", []string{"on"}, false, true},
		{"off from true", []string{"off"}, true, true},
		{"on from true", []string{"on"}, true, true},
		{"off from false", []string{"off"}, false, true},
		{"invalid", []string{"maybe"}, false, false},
		{"no args", nil, false, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			agentMode := tt.initial
			r := handleAgentMode(tt.args, CommandContext{
				AgentMode:    &agentMode,
				SetAgentMode: func(v bool) { agentMode = v },
			})
			if r.Success != tt.wantOK {
				t.Errorf("success=%v, want %v", r.Success, tt.wantOK)
			}
		})
	}
}

// ─── Screen launcher commands — extended tests ───────────────────────────────

func TestHandleMetrics_OpensScreen(t *testing.T) {
	t.Parallel()
	r := handleMetrics(nil, CommandContext{})
	if !r.Success || r.Screen == nil || *r.Screen != tuitypes.ScreenMetrics {
		t.Errorf("handleMetrics: got %+v", r)
	}
}

func TestHandleDashboard_OpensScreen(t *testing.T) {
	t.Parallel()
	r := handleDashboard(nil, CommandContext{})
	if !r.Success || r.Screen == nil || *r.Screen != tuitypes.ScreenDashboard {
		t.Errorf("handleDashboard: got %+v", r)
	}
}

func TestHandleNotifications_OpensScreen(t *testing.T) {
	t.Parallel()
	r := handleNotifications(nil, CommandContext{})
	if !r.Success || r.Screen == nil || *r.Screen != tuitypes.ScreenNotifications {
		t.Errorf("handleNotifications: got %+v", r)
	}
}

func TestHandleFiles_OpensScreen(t *testing.T) {
	t.Parallel()
	r := handleFiles(nil, CommandContext{})
	if !r.Success || r.Screen == nil || *r.Screen != tuitypes.ScreenFileExplorer {
		t.Errorf("handleFiles: got %+v", r)
	}
}

func TestHandleGhost_OpensScreen(t *testing.T) {
	t.Parallel()
	r := handleGhost(nil, CommandContext{})
	if !r.Success || r.Screen == nil || *r.Screen != tuitypes.ScreenGhostPicker {
		t.Errorf("handleGhost: got %+v", r)
	}
}

// ─── Subagent commands — extended tests ──────────────────────────────────────

func TestHandleAgent_NoArgs_NilManager(t *testing.T) {
	t.Parallel()
	r := handleAgent(nil, CommandContext{})
	if r.Success {
		t.Error("should fail without manager")
	}
}

func TestHandleAgentCancel_NoArgs(t *testing.T) {
	t.Parallel()
	r := handleAgentCancel(nil, CommandContext{})
	if r.Success {
		t.Error("should fail without args")
	}
}

// ─── AllCommands / AllCommandsWithExecute ─────────────────────────────────────

func TestAllCommands_VerifyFields(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	cmds := r.AllCommands()
	if len(cmds) == 0 {
		t.Error("AllCommands should return commands")
	}
	for _, cmd := range cmds {
		if cmd.Name == "" {
			t.Error("command has empty Name")
		}
		if cmd.Description == "" {
			t.Errorf("command %q has empty Description", cmd.Name)
		}
		if cmd.Slash != "/"+cmd.Name {
			t.Errorf("command %q has Slash=%q, want /%s", cmd.Name, cmd.Slash, cmd.Name)
		}
	}
}

func TestAllCommandsWithExecute_AllHaveFunc(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	cmds := r.AllCommandsWithExecute()
	if len(cmds) == 0 {
		t.Error("AllCommandsWithExecute should return commands")
	}
	for _, cmd := range cmds {
		if cmd.Execute == nil {
			t.Errorf("command %q has nil Execute", cmd.Name)
		}
	}
}

// ─── Execute integration — all commands ──────────────────────────────────────

func TestRegistry_Execute_CoreCommands(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	commands := []string{"/help", "/quit", "/settings", "/config", "/model", "/new", "/resume", "/metrics", "/dashboard", "/notifications", "/files", "/ghost", "/pause"}
	for _, cmd := range commands {
		t.Run(cmd, func(t *testing.T) {
			t.Parallel()
			result, handled := r.Execute(cmd, CommandContext{
				Config: &config.Config{UI: config.UIConfig{ShowCostEstimate: true}},
			})
			if !handled {
				t.Errorf("%s should be handled", cmd)
			}
			if !result.Success {
				t.Errorf("%s should succeed, got: %s", cmd, result.Message)
			}
		})
	}
}

func TestRegistry_Execute_FailingCommands(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	commands := []string{"/fork", "/prev", "/next", "/tokens", "/diff", "/rollback", "/bisect", "/sessions", "/save", "/goal", "/workflow", "/refine", "/resume-task", "/agent-cancel", "/agent", "/health", "/tools", "/copy-error", "/memory", "/compress", "/optimize", "/fallback", "/provider"}
	for _, cmd := range commands {
		t.Run(cmd, func(t *testing.T) {
			t.Parallel()
			result, handled := r.Execute(cmd, CommandContext{})
			if !handled {
				t.Errorf("%s should be handled", cmd)
			}
			if result.Success {
				t.Errorf("%s should fail without deps", cmd)
			}
		})
	}
}

func TestRegistry_Execute_PhaseAliases(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	aliases := []string{"/plan", "/execute", "/verify", "/ship", "/phase"}
	for _, cmd := range aliases {
		t.Run(cmd, func(t *testing.T) {
			t.Parallel()
			result, handled := r.Execute(cmd, CommandContext{
				SessionManager: &session.Manager{},
				SessionID:      "test",
			})
			if !handled {
				t.Errorf("%s should be handled", cmd)
			}
			// May succeed or fail depending on session state
			_ = result
		})
	}
}

func TestRegistry_Execute_Cost_WithConfig(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/cost", CommandContext{
		Config: &config.Config{UI: config.UIConfig{ShowCostEstimate: true}},
	})
	if !handled {
		t.Error("cost should be handled")
	}
	if !result.Success {
		t.Error("cost with config should succeed")
	}
}

func TestRegistry_Execute_Key_WithConfig(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/key", CommandContext{
		Config: &config.Config{},
	})
	if !handled {
		t.Error("key should be handled")
	}
	if !result.Success {
		t.Error("key with config should succeed")
	}
}

func TestRegistry_Execute_Reset(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/reset", CommandContext{})
	if !handled {
		t.Error("reset should be handled")
	}
	if !result.ConfirmRequired {
		t.Error("reset should require confirm")
	}
}

func TestRegistry_Execute_Clear(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/clear", CommandContext{})
	if !handled {
		t.Error("clear should be handled")
	}
	if !result.ConfirmRequired {
		t.Error("clear should require confirm")
	}
}

func TestRegistry_Execute_AgentMode_On(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	agentMode := false
	result, handled := r.Execute("/agent-mode on", CommandContext{
		AgentMode:    &agentMode,
		SetAgentMode: func(v bool) { agentMode = v },
	})
	if !handled {
		t.Error("agent-mode should be handled")
	}
	if !result.Success || !agentMode {
		t.Error("agent-mode on should succeed")
	}
}

func TestRegistry_Execute_AgentMode_Off(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	agentMode := true
	result, handled := r.Execute("/agent-mode off", CommandContext{
		AgentMode:    &agentMode,
		SetAgentMode: func(v bool) { agentMode = v },
	})
	if !handled {
		t.Error("agent-mode should be handled")
	}
	if !result.Success || agentMode {
		t.Error("agent-mode off should succeed")
	}
}

func TestRegistry_Execute_Ledger_NoArgs(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/ledger", CommandContext{
		Ledger: &ledger.Ledger{},
	})
	if !handled {
		t.Error("ledger should be handled")
	}
	if !result.Success {
		t.Error("ledger with ledger should succeed")
	}
}

func TestRegistry_Execute_Memory_View(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/memory view", CommandContext{})
	if !handled {
		t.Error("memory should be handled")
	}
	if result.Success {
		t.Error("memory view without autodream should fail")
	}
}

func TestRegistry_Execute_Log(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/log", CommandContext{})
	if !handled {
		t.Error("log should be handled")
	}
	_ = result
}

func TestRegistry_Execute_Tokens_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/tokens", CommandContext{})
	if !handled {
		t.Error("tokens should be handled")
	}
	if result.Success {
		t.Error("tokens should fail without session")
	}
}

func TestRegistry_Execute_Diff_NilGit(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/diff", CommandContext{})
	if !handled {
		t.Error("diff should be handled")
	}
	if result.Success {
		t.Error("diff should fail without git")
	}
}

func TestRegistry_Execute_Rollback_NilRollback(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/rollback", CommandContext{})
	if !handled {
		t.Error("rollback should be handled")
	}
	if result.Success {
		t.Error("rollback should fail without rollback")
	}
}

func TestRegistry_Execute_Bisect_NilGit(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/bisect", CommandContext{})
	if !handled {
		t.Error("bisect should be handled")
	}
	if result.Success {
		t.Error("bisect should fail without git")
	}
}

func TestRegistry_Execute_Sessions_NoManager(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/sessions", CommandContext{})
	if !handled {
		t.Error("sessions should be handled")
	}
	if result.Success {
		t.Error("sessions should fail without manager")
	}
}

func TestRegistry_Execute_Fork(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/fork", CommandContext{})
	if !handled {
		t.Error("fork should be handled")
	}
	if result.Success {
		t.Error("fork should fail (disabled)")
	}
}

func TestRegistry_Execute_Prev(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/prev", CommandContext{})
	if !handled {
		t.Error("prev should be handled")
	}
	if result.Success {
		t.Error("prev should fail (disabled)")
	}
}

func TestRegistry_Execute_Next(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/next", CommandContext{})
	if !handled {
		t.Error("next should be handled")
	}
	if result.Success {
		t.Error("next should fail (disabled)")
	}
}

func TestRegistry_Execute_Save_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/save", CommandContext{})
	if !handled {
		t.Error("save should be handled")
	}
	if result.Success {
		t.Error("save should fail without session")
	}
}

func TestRegistry_Execute_Goal_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/goal", CommandContext{})
	if !handled {
		t.Error("goal should be handled")
	}
	if result.Success {
		t.Error("goal should fail without session")
	}
}

func TestRegistry_Execute_Ledger_NilLedger(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/ledger", CommandContext{})
	if !handled {
		t.Error("ledger should be handled")
	}
	if result.Success {
		t.Error("ledger should fail without ledger")
	}
}

func TestRegistry_Execute_Export_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/export", CommandContext{})
	if !handled {
		t.Error("export should be handled")
	}
	if result.Success {
		t.Error("export should fail without session")
	}
}

func TestRegistry_Execute_Workflow_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/workflow", CommandContext{})
	if !handled {
		t.Error("workflow should be handled")
	}
	if result.Success {
		t.Error("workflow should fail without session")
	}
}

func TestRegistry_Execute_Refine_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/refine", CommandContext{})
	if !handled {
		t.Error("refine should be handled")
	}
	if result.Success {
		t.Error("refine should fail without session")
	}
}

func TestRegistry_Execute_ResumeTask_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/resume-task", CommandContext{})
	if !handled {
		t.Error("resume-task should be handled")
	}
	if result.Success {
		t.Error("resume-task should fail without session")
	}
}

func TestRegistry_Execute_Agent_Cancel_NilManager(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/agent-cancel", CommandContext{})
	if !handled {
		t.Error("agent-cancel should be handled")
	}
	if result.Success {
		t.Error("agent-cancel should fail without manager")
	}
}

func TestRegistry_Execute_Agent_NilManager(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/agent", CommandContext{})
	if !handled {
		t.Error("agent should be handled")
	}
	if result.Success {
		t.Error("agent should fail without manager")
	}
}

func TestRegistry_Execute_History_NilHistory(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/history", CommandContext{})
	if !handled {
		t.Error("history should be handled")
	}
	if !result.Success {
		t.Error("history should succeed and open chat history screen")
	}
	if result.Screen == nil {
		t.Error("history should return a screen")
	}
}

func TestRegistry_Execute_Health_NilRegistry(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/health", CommandContext{})
	if !handled {
		t.Error("health should be handled")
	}
	if result.Success {
		t.Error("health should fail without registry")
	}
}

func TestRegistry_Execute_Tools_NilDispatcher(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/tools", CommandContext{})
	if !handled {
		t.Error("tools should be handled")
	}
	if result.Success {
		t.Error("tools should fail without dispatcher")
	}
}

func TestRegistry_Execute_CopyError_NilFunc(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/copy-error", CommandContext{})
	if !handled {
		t.Error("copy-error should be handled")
	}
	if result.Success {
		t.Error("copy-error should fail without func")
	}
}

func TestRegistry_Execute_Memory_NilAutoDream(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/memory", CommandContext{})
	if !handled {
		t.Error("memory should be handled")
	}
	if result.Success {
		t.Error("memory should fail without autodream")
	}
}

func TestRegistry_Execute_Compress_NilAutoDream(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/compress", CommandContext{})
	if !handled {
		t.Error("compress should be handled")
	}
	if result.Success {
		t.Error("compress should fail without autodream")
	}
}

func TestRegistry_Execute_Optimize_NilRegistry(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/optimize", CommandContext{})
	if !handled {
		t.Error("optimize should be handled")
	}
	if result.Success {
		t.Error("optimize should fail without registry")
	}
}

func TestRegistry_Execute_Fallback_NilRegistry(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/fallback", CommandContext{})
	if !handled {
		t.Error("fallback should be handled")
	}
	if result.Success {
		t.Error("fallback should fail without registry")
	}
}

func TestRegistry_Execute_Provider_NilRegistry(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/provider", CommandContext{})
	if !handled {
		t.Error("provider should be handled")
	}
	if result.Success {
		t.Error("provider should fail without registry")
	}
}

func TestRegistry_Execute_Status_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/status", CommandContext{})
	if !handled {
		t.Error("status should be handled")
	}
	if result.Success {
		t.Error("status should fail without session")
	}
}

func TestRegistry_Execute_Undo_NoSession(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	result, handled := r.Execute("/undo", CommandContext{})
	if !handled {
		t.Error("undo should be handled")
	}
	if result.Success {
		t.Error("undo should fail without session")
	}
}

func TestRegistry_Execute_AgentMode_NoArgs(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	agentMode := false
	result, handled := r.Execute("/agent-mode", CommandContext{
		AgentMode: &agentMode,
	})
	if !handled {
		t.Error("agent-mode should be handled")
	}
	if !result.Success {
		t.Error("agent-mode no args should succeed")
	}
}

func TestRegistry_Execute_AgentMode_Invalid(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	agentMode := false
	result, handled := r.Execute("/agent-mode maybe", CommandContext{
		AgentMode: &agentMode,
	})
	if !handled {
		t.Error("agent-mode should be handled")
	}
	if result.Success {
		t.Error("agent-mode with invalid arg should fail")
	}
}

// ─── helpers ─────────────────────────────────────────────────────────────────

func containsSubstring(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
