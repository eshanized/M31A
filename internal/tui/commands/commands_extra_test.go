package commands

import (
	"testing"
)

func TestParseCommand(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		wantName string
		wantArgs []string
		wantOk   bool
	}{
		{"no slash", "help", "", nil, false},
		{"empty", "/", "", nil, true},
		{"just slash", "/", "", nil, true},
		{"simple", "/help", "help", nil, true},
		{"with args", "/model gpt-4", "model", []string{"gpt-4"}, true},
		{"multiple args", "/set key value", "set", []string{"key", "value"}, true},
		{"extra spaces", "/help  extra  spaces", "help", []string{"extra", "spaces"}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			name, args, ok := ParseCommand(tt.input)
			if ok != tt.wantOk {
				t.Errorf("ok = %v, want %v", ok, tt.wantOk)
			}
			if name != tt.wantName {
				t.Errorf("name = %q, want %q", name, tt.wantName)
			}
			if len(args) != len(tt.wantArgs) {
				t.Errorf("args length = %d, want %d", len(args), len(tt.wantArgs))
			} else {
				for i, arg := range args {
					if arg != tt.wantArgs[i] {
						t.Errorf("args[%d] = %q, want %q", i, arg, tt.wantArgs[i])
					}
				}
			}
		})
	}
}

func TestLevenshtein(t *testing.T) {
	tests := []struct {
		name string
		a    string
		b    string
		want int
	}{
		{"empty strings", "", "", 0},
		{"a empty", "", "abc", 3},
		{"b empty", "abc", "", 3},
		{"identical", "hello", "hello", 0},
		{"one edit", "hello", "hallo", 1},
		{"two edits", "kitten", "sitting", 3},
		{"completely different", "abc", "xyz", 3},
		{"prefix", "abc", "abcdef", 3},
		{"single char", "a", "b", 1},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := levenshtein(tt.a, tt.b)
			if got != tt.want {
				t.Errorf("levenshtein(%q, %q) = %d, want %d", tt.a, tt.b, got, tt.want)
			}
		})
	}
}

func TestNewCommandRegistry(t *testing.T) {
	r := NewCommandRegistry()
	if r == nil {
		t.Fatal("NewCommandRegistry() returned nil")
	}
	if len(r.handlers) != 0 {
		t.Errorf("handlers length = %d, want 0", len(r.handlers))
	}
}

func TestCommandRegistry_Register(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{Success: true}
	}

	err := r.Register("test", handler, "Test command")
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}

	if len(r.handlers) != 1 {
		t.Errorf("handlers length = %d, want 1", len(r.handlers))
	}
}

func TestCommandRegistry_Register_Duplicate(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{Success: true}
	}

	_ = r.Register("test", handler, "Test command")
	err := r.Register("test", handler, "Test command again")
	if err == nil {
		t.Error("Register() should return error for duplicate")
	}
}

func TestCommandRegistry_Get(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{Success: true}
	}

	_ = r.Register("test", handler, "Test command")

	h, ok := r.Get("test")
	if !ok {
		t.Error("Get() returned false")
	}
	if h == nil {
		t.Error("Get() returned nil handler")
	}

	_, ok = r.Get("nonexistent")
	if ok {
		t.Error("Get() returned true for nonexistent command")
	}
}

func TestCommandRegistry_List(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{}
	}

	_ = r.Register("zebra", handler, "Zebra")
	_ = r.Register("alpha", handler, "Alpha")
	_ = r.Register("beta", handler, "Beta")

	list := r.List()
	if len(list) != 3 {
		t.Fatalf("List() length = %d, want 3", len(list))
	}

	// Should be sorted alphabetically
	if list[0] != "alpha" || list[1] != "beta" || list[2] != "zebra" {
		t.Errorf("List() = %v, want [alpha beta zebra]", list)
	}
}

func TestCommandRegistry_AllCommands(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{}
	}

	_ = r.Register("test", handler, "Test command")

	cmds := r.AllCommands()
	if len(cmds) != 1 {
		t.Fatalf("AllCommands() length = %d, want 1", len(cmds))
	}

	cmd := cmds[0]
	if cmd.Name != "test" {
		t.Errorf("Name = %q, want %q", cmd.Name, "test")
	}
	if cmd.Description != "Test command" {
		t.Errorf("Description = %q, want %q", cmd.Description, "Test command")
	}
	if cmd.Slash != "/test" {
		t.Errorf("Slash = %q, want %q", cmd.Slash, "/test")
	}
}

func TestCommandRegistry_AllCommandsWithExecute(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{}
	}

	_ = r.Register("test", handler, "Test command")

	cmds := r.AllCommandsWithExecute()
	if len(cmds) != 1 {
		t.Fatalf("AllCommandsWithExecute() length = %d, want 1", len(cmds))
	}

	cmd := cmds[0]
	if cmd.Execute == nil {
		t.Error("Execute function is nil")
	}
}

func TestCommandRegistry_Execute_NotFound(t *testing.T) {
	r := NewCommandRegistry()

	result, ok := r.Execute("/nonexistent", CommandContext{})
	if !ok {
		t.Error("Execute() returned false for nonexistent command")
	}
	if result.Success {
		t.Error("Execute() should return failure for nonexistent command")
	}
}

func TestCommandRegistry_Execute_EmptyCommand(t *testing.T) {
	r := NewCommandRegistry()

	result, ok := r.Execute("/", CommandContext{})
	if !ok {
		t.Error("Execute() returned false for empty command")
	}
	if result.Success {
		t.Error("Execute() should return failure for empty command")
	}
}

func TestCommandRegistry_Execute_UnknownCommand(t *testing.T) {
	r := NewCommandRegistry()

	result, ok := r.Execute("/unknown", CommandContext{})
	if !ok {
		t.Error("Execute() returned false for unknown command")
	}
	if result.Success {
		t.Error("Execute() should return failure for unknown command")
	}
}

func TestSuggestCommand(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{}
	}

	_ = r.Register("help", handler, "Help")
	_ = r.Register("model", handler, "Model")
	_ = r.Register("models", handler, "Models")

	// Close match
	suggestion := suggestCommand(r, "hep")
	if suggestion != "help" {
		t.Errorf("suggestCommand('hep') = %q, want %q", suggestion, "help")
	}

	// No close match
	suggestion = suggestCommand(r, "xyz")
	if suggestion != "" {
		t.Errorf("suggestCommand('xyz') = %q, want empty", suggestion)
	}
}

func TestDefaultCommands(t *testing.T) {
	r := DefaultCommands()
	if r == nil {
		t.Fatal("DefaultCommands() returned nil")
	}

	// Should have many commands registered
	commands := r.List()
	if len(commands) < 30 {
		t.Errorf("DefaultCommands() registered %d commands, want >= 30", len(commands))
	}

	// Check some expected commands
	expected := []string{"help", "clear", "settings", "model", "diff", "sessions"}
	for _, name := range expected {
		_, ok := r.Get(name)
		if !ok {
			t.Errorf("DefaultCommands() missing command: %s", name)
		}
	}
}

func TestSuggestCommand_DistanceThreshold(t *testing.T) {
	r := NewCommandRegistry()

	handler := func(args []string, ctx CommandContext) CommandResult {
		return CommandResult{}
	}

	_ = r.Register("help", handler, "Help")

	// Distance 1 - should suggest
	suggestion := suggestCommand(r, "hep")
	if suggestion != "help" {
		t.Errorf("suggestCommand('hep') = %q, want %q", suggestion, "help")
	}

	// Distance 2 - should suggest
	suggestion = suggestCommand(r, "hp")
	if suggestion != "help" {
		t.Errorf("suggestCommand('hp') = %q, want %q", suggestion, "help")
	}

	// Distance 3 - should NOT suggest (threshold is 2)
	suggestion = suggestCommand(r, "x")
	if suggestion != "" {
		t.Errorf("suggestCommand('x') = %q, want empty", suggestion)
	}
}
