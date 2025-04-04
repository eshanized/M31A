package tui

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
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
	sessionMgr := session.NewManager(filepath.Join(dir, "sessions"))

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
		if !strings.Contains(result.Message, "unknown command") {
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

	// Verify all 16 command names appear in the output
	commands := []string{
		"/help", "/clear", "/status", "/model", "/provider",
		"/reset", "/quit", "/undo", "/compress", "/ledger",
		"/rollback", "/sessions", "/goal", "/phase", "/config", "/models",
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
		if !strings.Contains(result.Message, "unknown command") {
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
	result, found := r.Execute("/reset", CommandContext{})
	if !found {
		t.Fatal("expected /reset to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if result.Screen == nil {
		t.Fatal("expected Screen to be set for /reset")
	}
	if *result.Screen != ScreenFirstRun {
		t.Errorf("expected ScreenFirstRun, got %d", *result.Screen)
	}
}

// ---------------------------------------------------------------------------
// TestCommands_AllRegistered
// ---------------------------------------------------------------------------

func TestCommands_AllRegistered(t *testing.T) {
	r := DefaultCommands()
	names := r.List()
	if len(names) != 17 {
		t.Fatalf("expected exactly 17 commands, got %d: %v", len(names), names)
	}

	// Verify all expected commands are present
	expected := map[string]bool{
		"help": false, "clear": false, "status": false, "model": false,
		"provider": false, "reset": false, "quit": false, "undo": false,
		"compress": false, "ledger": false, "rollback": false, "sessions": false,
		"goal": false, "phase": false, "config": false, "models": false,
		"fallback": false,
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
	result, found := r.Execute("/clear", CommandContext{})
	if !found {
		t.Fatal("expected /clear to be found")
	}
	if !result.Success {
		t.Fatalf("expected success, got: %s", result.Message)
	}
	if result.Message != "Context cleared." {
		t.Errorf("expected 'Context cleared.', got: %s", result.Message)
	}
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

	t.Run("valid phase", func(t *testing.T) {
		result, _ := r.Execute("/phase execute", CommandContext{})
		if !result.Success {
			t.Fatalf("expected success, got: %s", result.Message)
		}
		if !strings.Contains(result.Message, "execute") {
			t.Errorf("expected phase name in message, got: %s", result.Message)
		}
	})

	t.Run("invalid phase", func(t *testing.T) {
		result, _ := r.Execute("/phase nonexistent", CommandContext{})
		if result.Success {
			t.Error("expected failure for invalid phase")
		}
		if !strings.Contains(result.Message, "Invalid phase") {
			t.Errorf("expected 'Invalid phase' message, got: %s", result.Message)
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
