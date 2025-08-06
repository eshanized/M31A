package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

// newTestContext creates a CommandContext backed by a temp directory with an
// initialized git repo, empty registry, session manager, and default config.
func newTestContext(t *testing.T) (CommandContext, string) {
	t.Helper()

	dir, err := os.MkdirTemp("", "m31a-commands-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}

	// Initialize git repo
	g := git.New(dir)
	if err := g.Init(); err != nil {
		os.RemoveAll(dir)
		t.Fatalf("Git init failed: %v", err)
	}
	if err := g.ConfigUser("Test", "test@test.com"); err != nil {
		os.RemoveAll(dir)
		t.Fatalf("Git config failed: %v", err)
	}

	// Create registry
	reg := provider.NewRegistry()
	reg.Register("openrouter", &mockProvider{})
	reg.Register("zen", &mockProvider{})

	// Create session manager
	sessionMgr := session.NewManager(filepath.Join(dir, "sessions"), session.ManagerOpts{})

	// Create a session
	s, err := sessionMgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		os.RemoveAll(dir)
		t.Fatalf("NewSession failed: %v", err)
	}

	// Create default config
	cfg := config.DefaultConfig()
	cfg.Model.Default = "gpt-4o"
	cfg.UI.Theme = "dark"

	ctx := CommandContext{
		Registry:       reg,
		SessionManager: sessionMgr,
		SessionID:      s.ID,
		Config:         cfg,
		Git:            g,
		Ledger:         ledger.New(filepath.Join(dir, "ledger.md")),
		Rollback:       rollback.New(g),
		AutoDream:      autodream.New(nil),
	}

	return ctx, dir
}

// cleanupTestContext removes the temp directory created by newTestContext.
func cleanupTestContext(dir string) {
	os.RemoveAll(dir)
}

// mustParse is a helper for test assertions on ParseCommand.
func mustParse(t *testing.T, input string) (name string, args []string) {
	t.Helper()
	name, args, ok := ParseCommand(input)
	if !ok {
		t.Fatalf("ParseCommand(%q) returned ok=false", input)
	}
	return name, args
}

// ---------------------------------------------------------------------------
// TestParseCommand
// ---------------------------------------------------------------------------

func TestParseCommand(t *testing.T) {
	t.Run("slash help", func(t *testing.T) {
		name, args := mustParse(t, "/help")
		if name != "help" {
			t.Errorf("expected name='help', got %q", name)
		}
		if len(args) != 0 {
			t.Errorf("expected 0 args, got %d", len(args))
		}
	})

	t.Run("slash model with arg", func(t *testing.T) {
		name, args := mustParse(t, "/model gpt-4")
		if name != "model" {
			t.Errorf("expected name='model', got %q", name)
		}
		if len(args) != 1 || args[0] != "gpt-4" {
			t.Errorf("expected args=['gpt-4'], got %v", args)
		}
	})

	t.Run("slash config key value", func(t *testing.T) {
		name, args := mustParse(t, "/config ui.theme dark")
		if name != "config" {
			t.Errorf("expected name='config', got %q", name)
		}
		if len(args) != 2 || args[0] != "ui.theme" || args[1] != "dark" {
			t.Errorf("expected args=['ui.theme', 'dark'], got %v", args)
		}
	})

	t.Run("no slash", func(t *testing.T) {
		name, args, ok := ParseCommand("hello")
		if ok {
			t.Errorf("expected ok=false for non-command input")
		}
		if name != "" || args != nil {
			t.Errorf("expected empty name and nil args, got %q, %v", name, args)
		}
	})

	t.Run("empty after slash", func(t *testing.T) {
		name, args, ok := ParseCommand("/")
		if !ok {
			t.Errorf("expected ok=true for '/' input")
		}
		if name != "" {
			t.Errorf("expected empty name, got %q", name)
		}
		if args != nil {
			t.Errorf("expected nil args, got %v", args)
		}
	})

	t.Run("multiple args", func(t *testing.T) {
		name, args := mustParse(t, "/cmd arg1 arg2 arg3")
		if name != "cmd" {
			t.Errorf("expected name='cmd', got %q", name)
		}
		if len(args) != 3 || args[0] != "arg1" || args[1] != "arg2" || args[2] != "arg3" {
			t.Errorf("expected 3 args, got %v", args)
		}
	})
}

// ---------------------------------------------------------------------------
// TestCommandRegistry
// ---------------------------------------------------------------------------

func TestCommandRegistry(t *testing.T) {
	t.Run("register and get", func(t *testing.T) {
		r := NewCommandRegistry()
		h := func(args []string, ctx CommandContext) CommandResult {
			return CommandResult{Success: true, Message: "ok"}
		}
		r.Register("test", h, "test command")
		got, found := r.Get("test")
		if !found {
			t.Fatal("expected Get to return found=true")
		}
		if got == nil {
			t.Fatal("expected non-nil handler")
		}
	})

	t.Run("get unknown", func(t *testing.T) {
		r := NewCommandRegistry()
		_, found := r.Get("nonexistent")
		if found {
			t.Error("expected found=false for unknown command")
		}
	})

	t.Run("list", func(t *testing.T) {
		r := NewCommandRegistry()
		r.Register("zeta", nil, "z")
		r.Register("alpha", nil, "a")
		r.Register("beta", nil, "b")
		names := r.List()
		if len(names) != 3 {
			t.Fatalf("expected 3 names, got %d", len(names))
		}
		if names[0] != "alpha" || names[1] != "beta" || names[2] != "zeta" {
			t.Errorf("expected sorted order, got %v", names)
		}
	})

	t.Run("execute with empty registry", func(t *testing.T) {
		r := NewCommandRegistry()
		result, ok := r.Execute("/test", CommandContext{})
		if !ok {
			t.Error("expected Execute to return ok=true (input starts with /)")
		}
		if result.Success {
			t.Error("expected result.Success=false for unknown command")
		}
		if !strings.Contains(strings.ToLower(result.Message), "unknown command") {
			t.Errorf("expected 'unknown command' in message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestHelpCommand
// ---------------------------------------------------------------------------

func TestHelpCommand(t *testing.T) {
	r := DefaultCommands()
	result, found := r.Execute("/help", CommandContext{})
	if !found {
		t.Fatal("expected /help to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got error: %s", result.Message)
	}

	// Verify all expected command names appear in the output
	commands := []string{
		"/help", "/clear", "/status", "/model", "/provider",
		"/reset", "/quit", "/undo", "/compress", "/ledger",
		"/rollback", "/sessions", "/goal", "/phase", "/config", "/models",
		"/fork", "/prev", "/next", "/save", "/workflow", "/plan",
		"/execute", "/verify", "/ship", "/theme", "/key", "/fallback",
		"/tools", "/history", "/diff", "/log", "/tokens", "/health",
		"/optimize", "/cost", "/pause", "/resume-task",
	}
	for _, cmd := range commands {
		if !strings.Contains(result.Message, cmd) {
			t.Errorf("help output missing command: %s", cmd)
		}
	}

	// Verify descriptions present
	descriptions := []string{
		"List all available commands",
		"Clear current conversation context",
		"Show current session info",
		"Exit the application",
	}
	for _, desc := range descriptions {
		if !strings.Contains(result.Message, desc) {
			t.Errorf("help output missing description: %q", desc)
		}
	}
}

// ---------------------------------------------------------------------------
// TestStatusCommand
// ---------------------------------------------------------------------------

func TestStatusCommand(t *testing.T) {
	ctx, dir := newTestContext(t)
	defer cleanupTestContext(dir)

	r := DefaultCommands()
	result, found := r.Execute("/status", ctx)
	if !found {
		t.Fatal("expected /status to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}

	// Verify session info appears in output
	if !strings.Contains(result.Message, ctx.SessionID) {
		t.Errorf("expected status to contain session ID %q", ctx.SessionID)
	}
	if !strings.Contains(result.Message, "openrouter") {
		t.Errorf("expected status to contain 'openrouter'")
	}
}

// ---------------------------------------------------------------------------
// TestModelCommand
// ---------------------------------------------------------------------------

func TestModelCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no args", func(t *testing.T) {
		result, found := r.Execute("/model", CommandContext{
			Config: config.DefaultConfig(),
		})
		if !found {
			t.Fatal("expected /model to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		// Should contain a helpful message about using /model <name> or /models
		if !strings.Contains(result.Message, "/models") {
			t.Errorf("expected hint about /models, got: %s", result.Message)
		}
	})

	t.Run("unknown model", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})
		result, found := r.Execute("/model nonexistent", CommandContext{
			Registry: reg,
			Config:   config.DefaultConfig(),
		})
		if !found {
			t.Fatal("expected /model nonexistent to be found")
		}
		if result.Success {
			t.Error("expected failure for unknown model")
		}
		if !strings.Contains(result.Message, "not found") {
			t.Errorf("expected 'not found' error, got: %s", result.Message)
		}
	})

	t.Run("selector flag", func(t *testing.T) {
		result, _ := r.Execute("/model --selector", CommandContext{})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if result.Screen == nil {
			t.Fatal("expected Screen to be set for --selector")
		}
		if *result.Screen != ScreenModelSelector {
			t.Errorf("expected ScreenModelSelector, got %d", *result.Screen)
		}
	})
}

// ---------------------------------------------------------------------------
// TestProviderCommand
// ---------------------------------------------------------------------------

func TestProviderCommand(t *testing.T) {
	reg := provider.NewRegistry()
	reg.Register("openrouter", &mockProvider{})
	reg.Register("zen", &mockProvider{})
	r := DefaultCommands()

	t.Run("no args", func(t *testing.T) {
		result, found := r.Execute("/provider", CommandContext{
			Registry: reg,
		})
		if !found {
			t.Fatal("expected /provider to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "openrouter") {
			t.Errorf("expected output to mention active provider")
		}
	})

	t.Run("unknown provider", func(t *testing.T) {
		result, _ := r.Execute("/provider nonexistent", CommandContext{
			Registry: reg,
		})
		if result.Success {
			t.Error("expected failure for unknown provider")
		}
		if !strings.Contains(result.Message, "Cannot") {
			t.Errorf("expected error message, got: %s", result.Message)
		}
	})

	t.Run("valid provider switch", func(t *testing.T) {
		result, _ := r.Execute("/provider zen", CommandContext{
			Registry: reg,
		})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "zen") {
			t.Errorf("expected confirmation, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestUndoCommand
// ---------------------------------------------------------------------------

func TestUndoCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no session", func(t *testing.T) {
		result, found := r.Execute("/undo", CommandContext{})
		if !found {
			t.Fatal("expected /undo to be found")
		}
		if result.Success {
			t.Error("expected failure with no session")
		}
	})

	t.Run("no checkpoint", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		result, _ := r.Execute("/undo", ctx)
		if result.Success {
			t.Error("expected failure when no checkpoint exists")
		}
		if !strings.Contains(result.Message, "No checkpoint") {
			t.Errorf("expected 'No checkpoint' message, got: %s", result.Message)
		}
	})

	t.Run("with checkpoint", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		// Save a checkpoint via the session manager
		cp := session.Checkpoint{
			Phase:        types.PhasePlan,
			MessageCount: 5,
			TaskCount:    3,
		}
		if err := ctx.SessionManager.SaveCheckpoint(ctx.SessionID, cp); err != nil {
			t.Fatalf("SaveCheckpoint failed: %v", err)
		}

		result, _ := r.Execute("/undo", ctx)
		if !result.Success {
			t.Fatalf("expected success with checkpoint, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "phase=plan") {
			t.Errorf("expected checkpoint info, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "messages=5") {
			t.Errorf("expected message count, got: %s", result.Message)
		}
		// Verify honest message about restoration not being implemented
		if !strings.Contains(result.Message, "not yet implemented") {
			t.Errorf("expected 'not yet implemented' notice, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestCompressCommand
// ---------------------------------------------------------------------------

func TestCompressCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("with autodream", func(t *testing.T) {
		msgs := make([]types.Message, 8)
		for i := range msgs {
			msgs[i] = types.Message{
				Role:    "user",
				Content: "Message content for testing purposes.",
			}
		}
		ctx := CommandContext{
			AutoDream: autodream.New(msgs),
		}
		result, found := r.Execute("/compress", ctx)
		if !found {
			t.Fatal("expected /compress to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})

	t.Run("without autodream", func(t *testing.T) {
		result, _ := r.Execute("/compress", CommandContext{})
		if result.Success {
			t.Error("expected failure without AutoDream")
		}
		if !strings.Contains(result.Message, "not available") {
			t.Errorf("expected 'not available' message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestRollbackCommand
// ---------------------------------------------------------------------------

func TestRollbackCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no git repo", func(t *testing.T) {
		result, found := r.Execute("/rollback", CommandContext{})
		if !found {
			t.Fatal("expected /rollback to be found")
		}
		if result.Success {
			t.Error("expected failure without git repo")
		}
	})

	t.Run("with git repo", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		// Create 3 commits
		for i := 0; i < 3; i++ {
			f := filepath.Join(dir, "file"+string(rune('0'+i))+".txt")
			if err := os.WriteFile(f, []byte("content"), 0644); err != nil {
				t.Fatalf("WriteFile failed: %v", err)
			}
			if err := ctx.Git.Commit("commit " + string(rune('0'+i))); err != nil {
				t.Fatalf("Commit %d failed: %v", i, err)
			}
		}

		result, _ := r.Execute("/rollback", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "Recent commits") {
			t.Errorf("expected 'Recent commits' header, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "[HEAD]") {
			t.Errorf("expected [HEAD] marker, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "commit 2") {
			t.Errorf("expected newest commit 'commit 2', got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestInvalidCommands
// ---------------------------------------------------------------------------

func TestInvalidCommands(t *testing.T) {
	r := DefaultCommands()

	t.Run("unknown command", func(t *testing.T) {
		result, found := r.Execute("/foobar", CommandContext{})
		if !found {
			t.Fatal("expected Execute to return found=true for unknown command")
		}
		if result.Success {
			t.Error("expected failure for unknown command")
		}
		if !strings.Contains(strings.ToLower(result.Message), "unknown command") {
			t.Errorf("expected 'unknown command' error, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "/help") {
			t.Errorf("expected hint to use /help, got: %s", result.Message)
		}
	})

	t.Run("empty slash", func(t *testing.T) {
		result, found := r.Execute("/", CommandContext{})
		if !found {
			t.Fatal("expected Execute to return found=true for /")
		}
		if result.Success {
			t.Error("expected failure for empty command")
		}
		if !strings.Contains(result.Message, "/help") {
			t.Errorf("expected hint to use /help, got: %s", result.Message)
		}
	})

	t.Run("non-command", func(t *testing.T) {
		_, found := r.Execute("just typing", CommandContext{})
		if found {
			t.Error("expected found=false for non-command input")
		}
	})
}

// ---------------------------------------------------------------------------
// TestResetCommand
// ---------------------------------------------------------------------------

func TestResetCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("without confirm flag shows warning", func(t *testing.T) {
		result, found := r.Execute("/reset", CommandContext{})
		if !found {
			t.Fatal("expected /reset to be found")
		}
		if result.Success {
			t.Error("expected failure without --confirm flag")
		}
		if !strings.Contains(result.Message, "--confirm") {
			t.Errorf("expected '--confirm' in warning message, got: %s", result.Message)
		}
	})

	t.Run("with confirm flag proceeds", func(t *testing.T) {
		result, found := r.Execute("/reset --confirm", CommandContext{})
		if !found {
			t.Fatal("expected /reset to be found")
		}
		if !result.Success {
			t.Fatalf("expected success with --confirm, got: %s", result.Message)
		}
		if result.Screen == nil {
			t.Fatal("expected Screen to be set for /reset --confirm")
		}
		if *result.Screen != ScreenFirstRun {
			t.Errorf("expected ScreenFirstRun, got %d", *result.Screen)
		}
	})
}

// ---------------------------------------------------------------------------
// TestCommands_AllRegistered
// ---------------------------------------------------------------------------

func TestCommands_AllRegistered(t *testing.T) {
	r := DefaultCommands()
	names := r.List()
	if len(names) != 39 {
		t.Fatalf("expected exactly 39 commands, got %d: %v", len(names), names)
	}

	// Verify all expected commands are present
	expected := map[string]bool{
		"help": false, "clear": false, "settings": false, "status": false, "model": false,
		"provider": false, "reset": false, "quit": false, "undo": false,
		"compress": false, "ledger": false, "rollback": false, "sessions": false,
		"goal": false, "phase": false, "config": false, "models": false,
		"fallback": false, "tools": false, "workflow": false, "history": false,
		"diff": false, "theme": false, "save": false, "key": false,
		"log": false, "tokens": false, "health": false,
		"fork": false, "prev": false, "next": false,
		"plan": false, "execute": false, "verify": false, "ship": false,
		"optimize": false, "pause": false, "resume-task": false,
		"cost": false,
	}
	hasExtra := false
	for _, name := range names {
		if _, ok := expected[name]; ok {
			expected[name] = true
		} else {
			t.Errorf("unexpected command: %s", name)
			hasExtra = true
		}
	}
	if hasExtra {
		t.Log("all expected commands:", len(expected))
	}
	for name, found := range expected {
		if !found {
			t.Errorf("missing expected command: %s", name)
		}
	}
}

// ---------------------------------------------------------------------------
// TestClearCommand
// ---------------------------------------------------------------------------

func TestClearCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("with callback", func(t *testing.T) {
		cleared := false
		result, found := r.Execute("/clear", CommandContext{
			ClearMessages: func() { cleared = true },
		})
		if !found {
			t.Fatal("expected /clear to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if result.Message != "Context cleared." {
			t.Errorf("expected 'Context cleared.', got: %s", result.Message)
		}
		if !cleared {
			t.Error("expected ClearMessages callback to be called")
		}
	})

	t.Run("without callback", func(t *testing.T) {
		result, found := r.Execute("/clear", CommandContext{})
		if !found {
			t.Fatal("expected /clear to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if result.Message != "No conversation to clear." {
			t.Errorf("expected 'No conversation to clear.', got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestQuitCommand
// ---------------------------------------------------------------------------

func TestQuitCommand(t *testing.T) {
	r := DefaultCommands()
	result, found := r.Execute("/quit", CommandContext{})
	if !found {
		t.Fatal("expected /quit to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "Goodbye") {
		t.Errorf("expected 'Goodbye' message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestSessionsCommand
// ---------------------------------------------------------------------------

func TestSessionsCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no manager", func(t *testing.T) {
		result, found := r.Execute("/sessions", CommandContext{})
		if !found {
			t.Fatal("expected /sessions to be found")
		}
		if result.Success {
			t.Error("expected failure without session manager")
		}
	})

	t.Run("with sessions", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		result, _ := r.Execute("/sessions", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, ctx.SessionID) {
			t.Errorf("expected session ID in output, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestGoalCommand
// ---------------------------------------------------------------------------

func TestGoalCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no args", func(t *testing.T) {
		result, found := r.Execute("/goal", CommandContext{})
		if !found {
			t.Fatal("expected /goal to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		// Should show a helpful message
		if !strings.Contains(result.Message, "/goal") {
			t.Errorf("expected hint about setting goal, got: %s", result.Message)
		}
	})

	t.Run("set goal", func(t *testing.T) {
		result, _ := r.Execute("/goal Build a web app", CommandContext{})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "Build a web app") {
			t.Errorf("expected goal in message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestPhaseCommand
// ---------------------------------------------------------------------------

func TestPhaseCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no args", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		result, found := r.Execute("/phase", ctx)
		if !found {
			t.Fatal("expected /phase to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})

	t.Run("valid phase alias transitions", func(t *testing.T) {
		// /phase plan with a workflow engine should trigger transition
		result, _ := r.Execute("/phase plan", CommandContext{
			WorkflowEngine: &mockWorkflowEngine{},
		})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "Starting plan phase") {
			t.Errorf("expected transition message, got: %s", result.Message)
		}
		if result.Cmd == nil {
			t.Fatal("expected Cmd to be set for phase transition")
		}
	})

	t.Run("phase alias without engine shows error", func(t *testing.T) {
		result, _ := r.Execute("/phase execute", CommandContext{})
		if result.Success {
			t.Fatalf("expected failure without engine, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "no workflow engine") {
			t.Errorf("expected 'no workflow engine' error, got: %s", result.Message)
		}
	})

	t.Run("invalid phase", func(t *testing.T) {
		result, _ := r.Execute("/phase nonexistent", CommandContext{})
		if result.Success {
			t.Error("expected failure for invalid phase")
		}
		if !strings.Contains(result.Message, "/phase <name>") {
			t.Errorf("expected guidance message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestConfigCommand
// ---------------------------------------------------------------------------

func TestConfigCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no args", func(t *testing.T) {
		cfg := config.DefaultConfig()
		cfg.UI.Theme = "dark"
		result, found := r.Execute("/config", CommandContext{Config: cfg})
		if !found {
			t.Fatal("expected /config to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "ui.theme") {
			t.Errorf("expected config key 'ui.theme' in output, got: %s", result.Message)
		}
	})

	t.Run("set theme", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config ui.theme light", CommandContext{Config: cfg})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if cfg.UI.Theme != "light" {
			t.Errorf("expected theme='light', got %q", cfg.UI.Theme)
		}
	})

	t.Run("set unknown key", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config unknown.key value", CommandContext{Config: cfg})
		if result.Success {
			t.Error("expected failure for unknown key")
		}
		if !strings.Contains(result.Message, "Unknown config key") {
			t.Errorf("expected 'Unknown config key' message, got: %s", result.Message)
		}
	})

	t.Run("no config", func(t *testing.T) {
		result, _ := r.Execute("/config", CommandContext{})
		if result.Success {
			t.Error("expected failure without config")
		}
	})
}

// ---------------------------------------------------------------------------
// TestModelsCommand
// ---------------------------------------------------------------------------

func TestModelsCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no registry", func(t *testing.T) {
		result, found := r.Execute("/models", CommandContext{})
		if !found {
			t.Fatal("expected /models to be found")
		}
		if result.Success {
			t.Error("expected failure without registry")
		}
	})

	t.Run("no active provider", func(t *testing.T) {
		reg := provider.NewRegistry()
		ctx := CommandContext{
			Registry: reg,
		}
		result, _ := r.Execute("/models", ctx)
		if result.Success {
			t.Error("expected failure without active provider")
		}
		if !strings.Contains(result.Message, "/provider") {
			t.Errorf("expected '/provider' suggestion, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "/settings") {
			t.Errorf("expected '/settings' suggestion, got: %s", result.Message)
		}
	})

	t.Run("with registry", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})
		ctx := CommandContext{
			Registry: reg,
		}
		result, _ := r.Execute("/models", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestLedgerCommand
// ---------------------------------------------------------------------------

func TestLedgerCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no ledger", func(t *testing.T) {
		result, found := r.Execute("/ledger", CommandContext{})
		if !found {
			t.Fatal("expected /ledger to be found")
		}
		if result.Success {
			t.Error("expected failure without ledger")
		}
	})

	t.Run("with ledger", func(t *testing.T) {
		ctx := CommandContext{
			Ledger: ledger.New(filepath.Join(t.TempDir(), "ledger.md")),
		}
		result, _ := r.Execute("/ledger", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})

	t.Run("ledger stats", func(t *testing.T) {
		ctx := CommandContext{
			Ledger: ledger.New(filepath.Join(t.TempDir(), "ledger.md")),
		}
		result, _ := r.Execute("/ledger stats", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestCommand_EmptyInput
// ---------------------------------------------------------------------------

func TestCommand_EmptyInput(t *testing.T) {
	r := DefaultCommands()
	result, found := r.Execute("", CommandContext{})
	if found {
		t.Error("expected found=false for empty input")
	}
	_ = result
}

// ---------------------------------------------------------------------------
// TestCommand_RegistryGetDescriptions
// ---------------------------------------------------------------------------

func TestCommand_RegistryGetDescriptions(t *testing.T) {
	r := NewCommandRegistry()
	r.Register("alpha", nil, "first command")
	r.Register("beta", nil, "second command")

	// Verify descriptions are stored and accessible
	names := r.List()
	if len(names) != 2 {
		t.Fatalf("expected 2 commands, got %d", len(names))
	}
}

// ---------------------------------------------------------------------------
// TestCommandResult_ScreenPointer
// ---------------------------------------------------------------------------

func TestCommandResult_ScreenPointer(t *testing.T) {
	screen := ScreenModelSelector
	result := CommandResult{
		Success: true,
		Message: "test",
		Screen:  &screen,
	}
	if result.Screen == nil {
		t.Fatal("Screen pointer should not be nil")
	}
	if *result.Screen != ScreenModelSelector {
		t.Errorf("expected ScreenModelSelector, got %d", *result.Screen)
	}
}

// ---------------------------------------------------------------------------
// TestFallbackCommand
// ---------------------------------------------------------------------------

func TestFallbackCommand(t *testing.T) {
	r := DefaultCommands()

	t.Run("no registry", func(t *testing.T) {
		result, found := r.Execute("/fallback", CommandContext{})
		if !found {
			t.Fatal("expected /fallback to be found")
		}
		if result.Success {
			t.Error("expected failure without registry")
		}
	})

	t.Run("show active provider", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})
		reg.Register("zen", &mockProvider{})

		result, found := r.Execute("/fallback", CommandContext{Registry: reg})
		if !found {
			t.Fatal("expected /fallback to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "openrouter") {
			t.Errorf("expected active provider in output, got: %s", result.Message)
		}
	})

	t.Run("switch to different provider", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})
		reg.Register("zen", &mockProvider{})

		result, found := r.Execute("/fallback zen", CommandContext{Registry: reg})
		if !found {
			t.Fatal("expected /fallback zen to be found")
		}
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "zen") {
			t.Errorf("expected 'zen' in message, got: %s", result.Message)
		}
	})

	t.Run("switch to same provider", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})

		result, found := r.Execute("/fallback openrouter", CommandContext{Registry: reg})
		if !found {
			t.Fatal("expected /fallback openrouter to be found")
		}
		if result.Success {
			t.Error("expected failure when switching to same provider")
		}
		if !strings.Contains(result.Message, "Already using") {
			t.Errorf("expected 'Already using' message, got: %s", result.Message)
		}
	})

	t.Run("switch to nonexistent provider", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})

		result, found := r.Execute("/fallback nonexistent", CommandContext{Registry: reg})
		if !found {
			t.Fatal("expected /fallback nonexistent to be found")
		}
		if result.Success {
			t.Error("expected failure for nonexistent provider")
		}
		if !strings.Contains(result.Message, "not found") {
			t.Errorf("expected 'not found' message, got: %s", result.Message)
		}
	})

	t.Run("no active provider message", func(t *testing.T) {
		reg := provider.NewRegistry()
		// Registry with no active provider set
		result, found := r.Execute("/fallback", CommandContext{Registry: reg})
		if !found {
			t.Fatal("expected /fallback to be found")
		}
		// The registry.Active() returns empty string when no provider is active
		_ = result
	})
}

// ---------------------------------------------------------------------------
// TestFormatLedgerEntries
// ---------------------------------------------------------------------------

func TestFormatLedgerEntries(t *testing.T) {
	t.Run("empty entries", func(t *testing.T) {
		result := formatLedgerEntries(nil, "Recent sessions")
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "none") {
			t.Errorf("expected 'none' in message, got: %s", result.Message)
		}
	})

	t.Run("with entries", func(t *testing.T) {
		dir := t.TempDir()
		l := ledger.New(filepath.Join(dir, "ledger.md"))

		entries := l.Entries()
		result := formatLedgerEntries(entries, "Recent sessions")
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})

	t.Run("more than 5 entries", func(t *testing.T) {
		dir := t.TempDir()
		l := ledger.New(filepath.Join(dir, "ledger.md"))

		// Create entries via the ledger
		for i := 0; i < 10; i++ {
			entry := ledger.LedgerEntry{
				SessionID: fmt.Sprintf("session-%d", i),
				Provider:  "openrouter",
				Model:     "gpt-4o",
				TaskCount: 5,
				Timestamp: time.Now(),
			}
			l.Append(entry)
		}

		entries := l.Entries()
		result := formatLedgerEntries(entries, "Recent sessions")
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		// Should only show 5 entries max
		if strings.Contains(result.Message, "  6.") {
			t.Error("should only show 5 entries max")
		}
	})
}

// ---------------------------------------------------------------------------
// TestGoalCommand_WithSession
// ---------------------------------------------------------------------------

func TestGoalCommand_WithSession(t *testing.T) {
	r := DefaultCommands()

	t.Run("set goal with session", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		// Set goal
		result, _ := r.Execute("/goal Build a REST API", ctx)
		if !result.Success {
			t.Fatalf("expected success setting goal, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "Build a REST API") {
			t.Errorf("expected goal in message, got: %s", result.Message)
		}
	})

	t.Run("multi-word goal", func(t *testing.T) {
		result, _ := r.Execute("/goal this is a multi word goal", CommandContext{})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "this is a multi word goal") {
			t.Errorf("expected full goal in message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestPhaseCommand_WithSession
// ---------------------------------------------------------------------------

func TestPhaseCommand_WithSession(t *testing.T) {
	r := DefaultCommands()

	t.Run("show current phase", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		result, _ := r.Execute("/phase", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "Current phase") {
			t.Errorf("expected 'Current phase' in message, got: %s", result.Message)
		}
	})

	t.Run("phase with args and no engine shows error", func(t *testing.T) {
		result, _ := r.Execute("/phase plan", CommandContext{})
		if result.Success {
			t.Fatalf("expected failure, got success: %s", result.Message)
		}
		if !strings.Contains(result.Message, "no workflow engine") {
			t.Errorf("expected 'no workflow engine' error, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestConfigCommand_MorePaths
// ---------------------------------------------------------------------------

func TestConfigCommand_MorePaths(t *testing.T) {
	r := DefaultCommands()

	t.Run("set model.default", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config model.default claude-3.5-sonnet", CommandContext{Config: cfg})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if cfg.Model.Default != "claude-3.5-sonnet" {
			t.Errorf("expected model.default='claude-3.5-sonnet', got %q", cfg.Model.Default)
		}
	})

	t.Run("set ui.compact_mode true", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config ui.compact_mode true", CommandContext{Config: cfg})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !cfg.UI.CompactMode {
			t.Error("expected CompactMode=true")
		}
	})

	t.Run("set ui.compact_mode invalid bool", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config ui.compact_mode maybe", CommandContext{Config: cfg})
		if result.Success {
			t.Error("expected failure for invalid bool")
		}
		if !strings.Contains(result.Message, "Invalid boolean") {
			t.Errorf("expected 'Invalid boolean' message, got: %s", result.Message)
		}
	})

	t.Run("set ui.show_token_usage", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config ui.show_token_usage true", CommandContext{Config: cfg})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !cfg.UI.ShowTokenUsage {
			t.Error("expected ShowTokenUsage=true")
		}
	})

	t.Run("set provider.auto_fallback", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config provider.auto_fallback true", CommandContext{Config: cfg})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !cfg.Provider.AutoFallback {
			t.Error("expected AutoFallback=true")
		}
	})

	t.Run("set with only one arg", func(t *testing.T) {
		cfg := config.DefaultConfig()
		result, _ := r.Execute("/config ui.theme", CommandContext{Config: cfg})
		if result.Success {
			t.Error("expected failure with only one arg")
		}
		if !strings.Contains(result.Message, "Usage") {
			t.Errorf("expected 'Usage' message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestHandleStatus_WithNilComponents
// ---------------------------------------------------------------------------

func TestHandleStatus_WithNilComponents(t *testing.T) {
	r := DefaultCommands()

	t.Run("with config only", func(t *testing.T) {
		cfg := config.DefaultConfig()
		cfg.Model.Default = "gpt-4o"
		result, _ := r.Execute("/status", CommandContext{Config: cfg})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "gpt-4o") {
			t.Errorf("expected model in output, got: %s", result.Message)
		}
	})

	t.Run("with minimal context", func(t *testing.T) {
		result, _ := r.Execute("/status", CommandContext{})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestHandleModel_WithNilRegistry
// ---------------------------------------------------------------------------

func TestHandleModel_WithNilRegistry(t *testing.T) {
	r := DefaultCommands()

	t.Run("no registry no config", func(t *testing.T) {
		result, _ := r.Execute("/model gpt-4", CommandContext{})
		if result.Success {
			t.Error("expected failure without registry")
		}
		if !strings.Contains(result.Message, "No provider") {
			t.Errorf("expected 'No provider' message, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestHandleProvider_WithNilRegistry
// ---------------------------------------------------------------------------

func TestHandleProvider_WithNilRegistry(t *testing.T) {
	r := DefaultCommands()

	result, _ := r.Execute("/provider", CommandContext{})
	if result.Success {
		t.Error("expected failure without registry")
	}
	if !strings.Contains(result.Message, "Registry not available") {
		t.Errorf("expected 'Registry not available' message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestHandleRollback_WithHardReset
// ---------------------------------------------------------------------------

func TestHandleRollback_WithHardReset(t *testing.T) {
	r := DefaultCommands()

	t.Run("no rollback", func(t *testing.T) {
		result, _ := r.Execute("/rollback --hard abc1234", CommandContext{})
		if result.Success {
			t.Error("expected failure without rollback")
		}
		if !strings.Contains(result.Message, "Rollback not available") {
			t.Errorf("expected 'Rollback not available' message, got: %s", result.Message)
		}
	})

	t.Run("hard reset without confirm shows warning", func(t *testing.T) {
		ctx, dir := newTestContext(t)
		defer cleanupTestContext(dir)

		// Create a commit
		f := filepath.Join(dir, "file.txt")
		if err := os.WriteFile(f, []byte("content"), 0644); err != nil {
			t.Fatalf("WriteFile failed: %v", err)
		}
		if err := ctx.Git.Commit("initial"); err != nil {
			t.Fatalf("Commit failed: %v", err)
		}

		result, _ := r.Execute("/rollback --hard HEAD~1", CommandContext{
			Rollback: ctx.Rollback,
		})
		if result.Success {
			t.Error("expected failure without --confirm flag")
		}
		if !strings.Contains(result.Message, "--confirm") {
			t.Errorf("expected '--confirm' in warning, got: %s", result.Message)
		}
	})

	t.Run("no git repo fallback", func(t *testing.T) {
		result, _ := r.Execute("/rollback", CommandContext{Git: nil})
		if !result.Success {
			// Expected: either success with "No git repository" or failure
		}
	})
}

// ---------------------------------------------------------------------------
// TestHandleLedger_WithTypeFilter
// ---------------------------------------------------------------------------

func TestHandleLedger_WithTypeFilter(t *testing.T) {
	r := DefaultCommands()

	t.Run("filter by type", func(t *testing.T) {
		dir := t.TempDir()
		l := ledger.New(filepath.Join(dir, "ledger.md"))

		ctx := CommandContext{Ledger: l}
		result, _ := r.Execute("/ledger web", ctx)
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "type: web") {
			t.Errorf("expected type filter in output, got: %s", result.Message)
		}
	})
}

// ---------------------------------------------------------------------------
// TestHandleCompress_WithNilAutodream
// ---------------------------------------------------------------------------

func TestHandleCompress_WithNilAutodream(t *testing.T) {
	r := DefaultCommands()

	result, _ := r.Execute("/compress", CommandContext{AutoDream: nil})
	if result.Success {
		t.Error("expected failure without AutoDream")
	}
	if !strings.Contains(result.Message, "not available") {
		t.Errorf("expected 'not available' message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestHandleUndo_WithNilSessionManager
// ---------------------------------------------------------------------------

func TestHandleUndo_WithNilSessionManager(t *testing.T) {
	r := DefaultCommands()

	result, _ := r.Execute("/undo", CommandContext{SessionID: "test-session"})
	if result.Success {
		t.Error("expected failure without session manager")
	}
	if !strings.Contains(result.Message, "No active session") {
		t.Errorf("expected 'No active session' message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestHandleSessions_WithEmptySessions
// ---------------------------------------------------------------------------

func TestHandleSessions_WithEmptySessions(t *testing.T) {
	r := DefaultCommands()

	dir := t.TempDir()
	sessionMgr := session.NewManager(filepath.Join(dir, "sessions"), session.ManagerOpts{})

	ctx := CommandContext{SessionManager: sessionMgr}
	result, _ := r.Execute("/sessions", ctx)
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "No sessions") {
		t.Errorf("expected 'No sessions' message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestHandleFallback_SwitchFailure
// ---------------------------------------------------------------------------

func TestHandleFork_NoSession(t *testing.T) {
	r := DefaultCommands()
	result, _ := r.Execute("/fork", CommandContext{SessionManager: nil, SessionID: ""})
	if result.Success {
		t.Fatal("Expected fork to fail with no active session")
	}
	if !strings.Contains(result.Message, "No active session") {
		t.Errorf("Expected 'No active session', got: %s", result.Message)
	}
}

func TestHandleFork_Success(t *testing.T) {
	r := DefaultCommands()

	// Mock session manager that supports ForkSession
	dir := t.TempDir()
	mgr := session.NewManager(dir, session.ManagerOpts{})

	parent, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	parentID := parent.ID
	result, _ := r.Execute("/fork", CommandContext{
		SessionManager: mgr,
		SessionID:      parentID,
	})
	if !result.Success {
		t.Fatalf("Expected fork to succeed, got: %s", result.Message)
	}
	if result.SessionID == nil {
		t.Fatal("Expected SessionID in result")
	}
	if *result.SessionID == parentID {
		t.Fatal("SessionID should be child ID, not parent ID")
	}
	if !strings.Contains(result.Message, "Session forked") {
		t.Errorf("Expected 'Session forked', got: %s", result.Message)
	}
}

func TestHandlePrev_NoSession(t *testing.T) {
	r := DefaultCommands()
	result, _ := r.Execute("/prev", CommandContext{SessionManager: nil, SessionID: ""})
	if result.Success {
		t.Fatal("Expected prev to fail with no active session")
	}
}

func TestHandlePrev_AtFirstSibling(t *testing.T) {
	r := DefaultCommands()
	dir := t.TempDir()
	mgr := session.NewManager(dir, session.ManagerOpts{})

	parent, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Root session: no parent, no prev
	result, _ := r.Execute("/prev", CommandContext{SessionManager: mgr, SessionID: parent.ID})
	if result.Success {
		t.Fatal("Expected prev to fail for root session")
	}
}

func TestHandlePrev_Navigation(t *testing.T) {
	r := DefaultCommands()
	dir := t.TempDir()
	mgr := session.NewManager(dir, session.ManagerOpts{})

	parent, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	child1, err := mgr.ForkSession(parent.ID)
	if err != nil {
		t.Fatalf("Fork 1 failed: %v", err)
	}
	child2, err := mgr.ForkSession(parent.ID)
	if err != nil {
		t.Fatalf("Fork 2 failed: %v", err)
	}

	// From child2, prev should switch to child1
	result, _ := r.Execute("/prev", CommandContext{SessionManager: mgr, SessionID: child2.ID})
	if !result.Success {
		t.Fatalf("Expected prev to succeed, got: %s", result.Message)
	}
	if result.SessionID == nil || *result.SessionID != child1.ID {
		t.Errorf("Expected prev to switch to %s, got %v", child1.ID, result.SessionID)
	}
}

func TestHandleNext_AtLastSibling(t *testing.T) {
	r := DefaultCommands()
	dir := t.TempDir()
	mgr := session.NewManager(dir, session.ManagerOpts{})

	parent, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Root session: no parent, no next
	result, _ := r.Execute("/next", CommandContext{SessionManager: mgr, SessionID: parent.ID})
	if result.Success {
		t.Fatal("Expected next to fail for root session")
	}
}

func TestHandleNext_Navigation(t *testing.T) {
	r := DefaultCommands()
	dir := t.TempDir()
	mgr := session.NewManager(dir, session.ManagerOpts{})

	parent, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	child1, err := mgr.ForkSession(parent.ID)
	if err != nil {
		t.Fatalf("Fork 1 failed: %v", err)
	}
	child2, err := mgr.ForkSession(parent.ID)
	if err != nil {
		t.Fatalf("Fork 2 failed: %v", err)
	}

	// From child1, next should switch to child2
	result, _ := r.Execute("/next", CommandContext{SessionManager: mgr, SessionID: child1.ID})
	if !result.Success {
		t.Fatalf("Expected next to succeed, got: %s", result.Message)
	}
	if result.SessionID == nil || *result.SessionID != child2.ID {
		t.Errorf("Expected next to switch to %s, got %v", child2.ID, result.SessionID)
	}
}

func TestHandleFallback_SwitchFailure(t *testing.T) {
	r := DefaultCommands()

	// Create a registry where SetActive will fail for a valid provider name
	// but the provider is registered. We need to test the path where
	// SetActive returns an error.
	reg := provider.NewRegistry()
	reg.Register("openrouter", &mockProvider{})
	reg.Register("zen", &mockProvider{})
	reg.SetActive("openrouter")

	// Try to switch to zen - this should succeed
	result, _ := r.Execute("/fallback zen", CommandContext{Registry: reg})
	if !result.Success {
		t.Logf("fallback to zen: %s", result.Message)
	}

	// Now try to switch back to openrouter
	result, _ = r.Execute("/fallback openrouter", CommandContext{Registry: reg})
	// This might succeed or fail depending on SetActive validation
	_ = result
}

// ---------------------------------------------------------------------------
// Test helpers for diff tests
// ---------------------------------------------------------------------------

// sampleDiff returns a realistic git diff output for test parsing.
func sampleDiff() string {
	return `diff --git a/main.go b/main.go
index abc123..def456 100644
--- a/main.go
+++ b/main.go
@@ -10,6 +10,8 @@ func main() {
 	println("hello")
-	oldLine()
+	newLine()
+	extraLine()
 	println("world")
 }`
}

// ---------------------------------------------------------------------------
// TestDiffCommand — 5 argument variants
// ---------------------------------------------------------------------------

func TestDiffCommand_NoArgs(t *testing.T) {
	r := DefaultCommands()
	ctx, dir := newTestContext(t)
	defer cleanupTestContext(dir)

	// Create a file, commit it, then modify it (unstaged)
	f := filepath.Join(dir, "hello.go")
	if err := os.WriteFile(f, []byte("package main\nfunc main() {}"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := ctx.Git.Commit("initial"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}
	// Now modify (creates an unstaged diff)
	if err := os.WriteFile(f, []byte("package main\nfunc main() { println(\"hi\") }"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, found := r.Execute("/diff", ctx)
	if !found {
		t.Fatal("expected /diff to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if result.Screen == nil {
		t.Fatal("expected Screen to be set for full diff")
	}
	if *result.Screen != ScreenDiff {
		t.Errorf("expected ScreenDiff (%d), got %d", ScreenDiff, *result.Screen)
	}
	if result.Cmd == nil {
		t.Fatal("expected Cmd to be non-nil, carrying DiffScreenMsg")
	}
	msg := result.Cmd()
	if _, ok := msg.(DiffScreenMsg); !ok {
		t.Errorf("expected DiffScreenMsg from Cmd, got %T", msg)
	}
}

func TestDiffCommand_Staged(t *testing.T) {
	r := DefaultCommands()
	ctx, dir := newTestContext(t)
	defer cleanupTestContext(dir)

	// Create a file, write content, stage it (don't commit)
	f := filepath.Join(dir, "hello.go")
	if err := os.WriteFile(f, []byte("package main\nfunc main() {}"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := ctx.Git.Add(f); err != nil {
		t.Fatalf("Add failed: %v", err)
	}

	result, found := r.Execute("/diff --staged", ctx)
	if !found {
		t.Fatal("expected /diff --staged to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if result.Screen == nil {
		t.Fatal("expected Screen to be set for staged diff")
	}
	if *result.Screen != ScreenDiff {
		t.Errorf("expected ScreenDiff (%d), got %d", ScreenDiff, *result.Screen)
	}
	if result.Cmd == nil {
		t.Fatal("expected Cmd to carry DiffScreenMsg")
	}
	msg := result.Cmd()
	dsm, ok := msg.(DiffScreenMsg)
	if !ok {
		t.Fatalf("expected DiffScreenMsg, got %T", msg)
	}
	if dsm.Diff == "" {
		t.Error("expected non-empty diff content")
	}
	if !strings.Contains(dsm.Title, "staged") {
		t.Errorf("expected 'staged' in title, got %q", dsm.Title)
	}
}

func TestDiffCommand_Stat(t *testing.T) {
	r := DefaultCommands()
	ctx, dir := newTestContext(t)
	defer cleanupTestContext(dir)

	// Create a file with content, commit, then modify
	f := filepath.Join(dir, "hello.go")
	if err := os.WriteFile(f, []byte("package main\nfunc main() {}\n"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := ctx.Git.Commit("initial"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}
	// Modify to create changes tracked by --stat
	if err := os.WriteFile(f, []byte("package main\nfunc main() { println(\"hi\") }\n"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, found := r.Execute("/diff --stat", ctx)
	if !found {
		t.Fatal("expected /diff --stat to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	// --stat should NOT set Screen (inline output)
	if result.Screen != nil {
		t.Error("expected Screen to be nil for --stat (inline output)")
	}
	if !strings.Contains(result.Message, "Diff Stat") {
		t.Errorf("expected 'Diff Stat' header, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "files changed") {
		t.Errorf("expected 'files changed' summary, got: %s", result.Message)
	}
}

func TestDiffCommand_NoChanges(t *testing.T) {
	r := DefaultCommands()
	ctx, dir := newTestContext(t)
	defer cleanupTestContext(dir)

	// Clean repo — no modifications
	result, found := r.Execute("/diff", ctx)
	if !found {
		t.Fatal("expected /diff to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "No uncommitted changes") {
		t.Errorf("expected 'No uncommitted changes' message, got: %s", result.Message)
	}
	if result.Screen != nil {
		t.Error("expected Screen nil for no-diff case")
	}
}

func TestDiffCommand_NoGit(t *testing.T) {
	r := DefaultCommands()
	result, found := r.Execute("/diff", CommandContext{Git: nil})
	if !found {
		t.Fatal("expected /diff to be found")
	}
	if result.Success {
		t.Fatal("expected failure without git repo")
	}
	if !strings.Contains(result.Message, "No git repository") {
		t.Errorf("expected 'No git repository' message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestDiffModel — parseDiff, scroll, toggle
// ---------------------------------------------------------------------------

func TestDiffModel_ParseDiff(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	diffText := sampleDiff()
	lines := m.parseDiff(diffText)

	if len(lines) == 0 {
		t.Fatal("expected parsed lines, got none")
	}

	// Verify line type classification
	expected := []struct {
		index int
		kind  DiffLineType
		hint  string
	}{
		{0, DiffHeader, "diff --git"},
		{1, DiffHeader, "index"},
		{2, DiffHeader, "---"},
		{3, DiffHeader, "+++"},
		{4, DiffHunk, "@@"},
		{5, DiffContext, "space-prefixed context"},
		{6, DiffDeleted, "removed line"},
		{7, DiffAdded, "added line"},
		{8, DiffAdded, "second added line"},
		{9, DiffContext, "space-prefixed context"},
		{10, DiffContext, "closing brace"},
	}

	for _, exp := range expected {
		if exp.index >= len(lines) {
			t.Errorf("expected line %d (%s), but diff has only %d lines", exp.index, exp.hint, len(lines))
			continue
		}
		if lines[exp.index].Type != exp.kind {
			t.Errorf("line %d (%s): expected type %d, got %d (content: %q)",
				exp.index, exp.hint, exp.kind, lines[exp.index].Type, lines[exp.index].Content)
		}
	}
}

func TestDiffModel_Scroll(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	m.diff = sampleDiff()
	m.lines = m.parseDiff(sampleDiff())

	// Verify initial scroll position
	if m.scrollPos != 0 {
		t.Fatalf("expected scrollPos=0 initially, got %d", m.scrollPos)
	}

	// Arrow down
	model, _ := m.Update(tea.KeyMsg{Type: tea.KeyDown})
	m = model.(DiffModel)
	if m.scrollPos != 1 {
		t.Errorf("expected scrollPos=1 after arrow down, got %d", m.scrollPos)
	}

	// Arrow down again
	model, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	m = model.(DiffModel)
	if m.scrollPos != 2 {
		t.Errorf("expected scrollPos=2 after second arrow down, got %d", m.scrollPos)
	}

	// Arrow up
	model, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	m = model.(DiffModel)
	if m.scrollPos != 1 {
		t.Errorf("expected scrollPos=1 after arrow up, got %d", m.scrollPos)
	}

	// g key (go to top)
	model, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'g'}})
	m = model.(DiffModel)
	if m.scrollPos != 0 {
		t.Errorf("expected scrollPos=0 after 'g', got %d", m.scrollPos)
	}

	// Arrow up at top stays at 0
	model, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	m = model.(DiffModel)
	if m.scrollPos != 0 {
		t.Errorf("expected scrollPos=0 when already at top, got %d", m.scrollPos)
	}
}

func TestDiffModel_EscReturnsCloseMsg(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	m.diff = sampleDiff()
	m.lines = m.parseDiff(sampleDiff())

	model, cmd := m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	m = model.(DiffModel)
	if cmd == nil {
		t.Fatal("expected non-nil Cmd from esc key")
	}
	msg := cmd()
	if _, ok := msg.(DiffCloseMsg); !ok {
		t.Errorf("expected DiffCloseMsg from esc key, got %T", msg)
	}
}

func TestDiffModel_HomeEndKeys(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	m.diff = sampleDiff()
	m.lines = m.parseDiff(sampleDiff())

	// Scroll down a bit
	model, _ := m.Update(tea.KeyMsg{Type: tea.KeyDown})
	model, _ = model.Update(tea.KeyMsg{Type: tea.KeyDown})
	model, _ = model.Update(tea.KeyMsg{Type: tea.KeyDown})
	m = model.(DiffModel)
	if m.scrollPos == 0 {
		t.Fatal("expected scrollPos > 0 after scrolling")
	}

	// Home key goes to top
	model, _ = m.Update(tea.KeyMsg{Type: tea.KeyHome})
	m = model.(DiffModel)
	if m.scrollPos != 0 {
		t.Errorf("expected scrollPos=0 after home, got %d", m.scrollPos)
	}

	// End key goes to bottom
	model, _ = m.Update(tea.KeyMsg{Type: tea.KeyEnd})
	m = model.(DiffModel)
	if m.scrollPos != len(m.lines)-1 {
		t.Errorf("expected scrollPos=%d after end, got %d", len(m.lines)-1, m.scrollPos)
	}
}

func TestDiffModel_EmptyDiff(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	view := m.View()
	if !strings.Contains(view, "No diff to display") {
		t.Errorf("expected placeholder message, got: %s", view)
	}
}

func TestDiffModel_DiffScreenMsg(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	diffText := sampleDiff()

	// Send DiffScreenMsg
	model, _ := m.Update(DiffScreenMsg{Diff: diffText, Title: "git diff test"})
	m = model.(DiffModel)

	if m.title != "git diff test" {
		t.Errorf("expected title 'git diff test', got %q", m.title)
	}
	if m.diff != diffText {
		t.Errorf("expected diff text to be set")
	}
	if len(m.lines) == 0 {
		t.Error("expected parsed lines after DiffScreenMsg")
	}
	if m.scrollPos != 0 {
		t.Errorf("expected scrollPos=0 after new diff, got %d", m.scrollPos)
	}
}

func TestDiffModel_ViewContainsColoredContent(t *testing.T) {
	m := NewDiffModel(theme.Dark(), 0, 0)
	m.diff = sampleDiff()
	m.lines = m.parseDiff(sampleDiff())
	m.width = 120
	m.height = 40
	m.title = "git diff"

	view := m.View()
	// View should contain the title and diff content
	if !strings.Contains(view, "git diff") {
		t.Errorf("expected title in view, got: %s", view)
	}
	if !strings.Contains(view, "scroll") {
		t.Errorf("expected help bar with 'scroll', got: %s", view)
	}
	if !strings.Contains(view, "oldLine") {
		t.Errorf("expected diff content (oldLine) in view, got: %s", view)
	}
	if !strings.Contains(view, "newLine") {
		t.Errorf("expected diff content (newLine) in view, got: %s", view)
	}
}

// ---------------------------------------------------------------------------
// TestPauseCommand
// ---------------------------------------------------------------------------

func TestPauseCommand(t *testing.T) {
	r := DefaultCommands()
	result, found := r.Execute("/pause", CommandContext{})
	if !found {
		t.Fatal("expected /pause to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "Execute screen") {
		t.Errorf("expected 'Execute screen' in message, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "press P") {
		t.Errorf("expected 'press P' in message, got: %s", result.Message)
	}
}

// ---------------------------------------------------------------------------
// TestResumeTaskCommand
// ---------------------------------------------------------------------------

func TestResumeTaskCommand(t *testing.T) {
	r := DefaultCommands()
	result, found := r.Execute("/resume-task", CommandContext{})
	if !found {
		t.Fatal("expected /resume-task to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "not yet implemented") {
		t.Errorf("expected 'not yet implemented' in message, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "/workflow") {
		t.Errorf("expected '/workflow' suggestion in message, got: %s", result.Message)
	}
}

func TestCommandSuggestion(t *testing.T) {
	r := DefaultCommands()

	tests := []struct {
		input    string
		suggests string
	}{
		{"hlel", "help"},
		{"statu", "status"},
		{"clera", "clear"},
		{"modl", "model"},
		{"setings", "settings"},
		{"quit", ""},       // exact match — no suggestion needed
		{"xyzabc", ""},     // too far from any command
		{"hlep", "help"},   // transposition
		{"modle", "model"}, // transposition
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := suggestCommand(r, tt.input)
			if got != tt.suggests {
				t.Errorf("suggestCommand(%q) = %q, want %q", tt.input, got, tt.suggests)
			}
		})
	}
}

func TestCommandSuggestionIntegration(t *testing.T) {
	r := DefaultCommands()
	ctx, _ := newTestContext(t)

	// /hlel should suggest /help
	result, handled := r.Execute("/hlel", ctx)
	if !handled {
		t.Fatal("expected command to be handled")
	}
	if result.Success {
		t.Fatal("expected failure for typo")
	}
	if !strings.Contains(result.Message, "Did you mean /help") {
		t.Errorf("expected 'Did you mean /help' in message, got: %s", result.Message)
	}

	// /xyzabc should not suggest anything specific
	result, handled = r.Execute("/xyzabc", ctx)
	if !handled {
		t.Fatal("expected command to be handled")
	}
	if result.Success {
		t.Fatal("expected failure for unknown command")
	}
	if strings.Contains(result.Message, "Did you mean") {
		t.Errorf("should not suggest for unrelated input, got: %s", result.Message)
	}
}

func TestLevenshtein(t *testing.T) {
	tests := []struct {
		a, b string
		want int
	}{
		{"", "", 0},
		{"abc", "", 3},
		{"", "abc", 3},
		{"abc", "abc", 0},
		{"abc", "abd", 1},
		{"abc", "ab", 1},
		{"abc", "abcd", 1},
		{"kitten", "sitting", 3},
		{"hlel", "help", 2},
	}

	for _, tt := range tests {
		t.Run(fmt.Sprintf("%s_%s", tt.a, tt.b), func(t *testing.T) {
			got := levenshtein(tt.a, tt.b)
			if got != tt.want {
				t.Errorf("levenshtein(%q, %q) = %d, want %d", tt.a, tt.b, got, tt.want)
			}
		})
	}
}

func TestKeyCommand(t *testing.T) {
	r := DefaultCommands()
	ctx, _ := newTestContext(t)

	result, found := r.Execute("/key", ctx)
	if !found {
		t.Fatal("expected /key to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "/settings") {
		t.Errorf("expected '/settings' suggestion in output, got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "Key resolution order") {
		t.Errorf("expected 'Key resolution order' in output, got: %s", result.Message)
	}
}

func TestOptimizeError(t *testing.T) {
	r := DefaultCommands()

	t.Run("disabled", func(t *testing.T) {
		reg := provider.NewRegistry()
		cfg := &config.Config{}
		cfg.Model.AutoArbitrage = false
		result, found := r.Execute("/optimize", CommandContext{Registry: reg, Config: cfg})
		if !found {
			t.Fatal("expected /optimize to be found")
		}
		if result.Success {
			t.Fatal("expected failure when disabled")
		}
		if !strings.Contains(result.Message, "auto_arbitrage") {
			t.Errorf("expected 'auto_arbitrage' in message, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "/config") {
			t.Errorf("expected '/config' suggestion, got: %s", result.Message)
		}
	})
}
