package tui

import (
	"fmt"
	"sort"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

// CommandResult is the result of executing a slash command.
type CommandResult struct {
	Success         bool
	Message         string
	Screen          *Screen
	SessionID       *string
	Config          *config.Config
	Cmd             func() tea.Msg
	WorkflowResume  bool
	ResumePhase     types.WorkflowPhase
	ResumeGoal      string
	ResumeQuestions []string
}

// CommandHandler is a function that handles a slash command.
type CommandHandler func(args []string, ctx CommandContext) CommandResult

// CommandContext carries shared components that command handlers need.
type CommandContext struct {
	Registry       *provider.Registry
	SessionManager *session.Manager
	SessionID      string
	Config         *config.Config
	ConfigPath     string
	Dispatcher     *tools.Dispatcher
	Git            *git.Git
	Ledger         *ledger.Ledger
	Rollback       *rollback.Rollback
	AutoDream      *autodream.Consolidator
	WorkflowEngine workflowEngineInterface
	CmdRegistry    *CommandRegistry
	ClearMessages  func() // callback to clear REPL message history
}

// CommandRegistry holds a map of registered command handlers and their descriptions.
type CommandRegistry struct {
	handlers         map[string]CommandHandler
	descriptions     map[string]string
	lastCompressTime time.Time
}

// NewCommandRegistry creates an empty CommandRegistry.
func NewCommandRegistry() *CommandRegistry {
	return &CommandRegistry{
		handlers:     make(map[string]CommandHandler),
		descriptions: make(map[string]string),
	}
}

// Register adds a command handler with a description to the registry.
func (r *CommandRegistry) Register(name string, handler CommandHandler, description string) {
	r.handlers[name] = handler
	r.descriptions[name] = description
}

// Get returns the handler for a command name.
func (r *CommandRegistry) Get(name string) (CommandHandler, bool) {
	h, ok := r.handlers[name]
	return h, ok
}

// List returns all registered command names sorted alphabetically.
func (r *CommandRegistry) List() []string {
	names := make([]string, 0, len(r.handlers))
	for name := range r.handlers {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// AllCommands returns all registered commands as CommandInfo slices.
func (r *CommandRegistry) AllCommands() []CommandInfo {
	names := r.List()
	cmds := make([]CommandInfo, 0, len(names))
	for _, name := range names {
		cmds = append(cmds, CommandInfo{
			Name:        name,
			Description: r.descriptions[name],
			Slash:       "/" + name,
		})
	}
	return cmds
}

// Execute parses input as a command, looks up the handler, and runs it.
func (r *CommandRegistry) Execute(input string, ctx CommandContext) (CommandResult, bool) {
	name, args, ok := ParseCommand(input)
	if !ok {
		return CommandResult{}, false
	}
	if name == "" {
		return CommandResult{Success: false, Message: "Empty command. Type /help for available commands."}, true
	}
	handler, found := r.Get(name)
	if !found {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("unknown command: /%s. Type /help for available commands.", name),
		}, true
	}
	return handler(args, ctx), true
}

// ParseCommand splits an input string into a command name and arguments.
func ParseCommand(input string) (name string, args []string, ok bool) {
	if !strings.HasPrefix(input, "/") {
		return "", nil, false
	}
	input = strings.TrimPrefix(input, "/")
	parts := strings.Fields(input)
	if len(parts) == 0 {
		return "", nil, true
	}
	return parts[0], parts[1:], true
}

// DefaultCommands creates a CommandRegistry pre-populated with all slash commands.
func DefaultCommands() *CommandRegistry {
	r := NewCommandRegistry()

	r.Register("help", handleHelp, "List all available commands")
	r.Register("clear", handleClear, "Clear current conversation context")
	r.Register("settings", handleSettings, "Open settings editor")
	r.Register("status", handleStatus, "Show current session info")
	r.Register("model", handleModel, "Show or switch model")
	r.Register("provider", handleProvider, "Show or switch provider")
	r.Register("reset", handleReset, "Reset to first-run screen")
	r.Register("quit", handleQuit, "Exit the application")
	r.Register("undo", handleUndo, "Show latest checkpoint info (restoration not yet implemented)")
	r.Register("compress", handleCompress, "Trigger context consolidation")
	r.Register("ledger", handleLedger, "Show recent session entries")
	r.Register("rollback", handleRollback, "Show commit chain or reset to commit")
	r.Register("sessions", handleSessions, "List recent sessions")
	r.Register("goal", handleGoal, "Set or show session goal")
	r.Register("phase", handlePhase, "Show or transition to phase")
	r.Register("config", handleConfig, "Show or set config value")
	r.Register("models", handleModels, "List all cached models")
	r.Register("fallback", handleFallback, "Switch to alternative provider / show fallback status")
	r.Register("tools", handleTools, "List available tools and their descriptions")
	r.Register("workflow", handleWorkflow, "Show workflow status and phase progress")
	r.Register("history", handleHistory, "Show conversation message history")
	r.Register("diff", handleDiff, "Show git diff of uncommitted changes")
	r.Register("theme", handleTheme, "Switch between dark and light theme")
	r.Register("save", handleSave, "Save current session and conversation")
	r.Register("key", handleKey, "Show API key status and source")
	r.Register("log", handleLog, "Show recent log entries")
	r.Register("tokens", handleTokens, "Estimate token count for text")
	r.Register("health", handleHealth, "Show system health status")
	r.Register("fork", handleFork, "Fork current session into a new child session")
	r.Register("prev", handlePrev, "Switch to previous sibling session")
	r.Register("next", handleNext, "Switch to next sibling session")

	// Workflow phase aliases
	r.Register("plan", handlePhase, "Alias for /phase plan")
	r.Register("execute", handlePhase, "Alias for /phase execute")
	r.Register("verify", handlePhase, "Alias for /phase verify")
	r.Register("ship", handlePhase, "Alias for /phase ship")

	// Cost optimization
	r.Register("optimize", handleOptimize, "Suggest cheaper model alternatives")
	r.Register("cost", handleCost, "Toggle cost estimate display in header")

	// Workflow control
	r.Register("pause", handlePause, "Pause current workflow phase")
	r.Register("resume-task", handleResumeTask, "Resume task from last checkpoint")

	return r
}
