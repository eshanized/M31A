package tui

import (
	"context"
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

// CommandInfo describes a slash command for autocomplete and help.
type CommandInfo struct {
	Name        string
	Description string
	Slash       string // e.g. "/help"
	Execute     func() tea.Cmd // optional: used by command palette
}

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
	ConfirmRequired bool
	ConfirmPrompt   string
}

// CommandHandler processes a slash command invocation.
type CommandHandler func(args []string, ctx CommandContext) CommandResult

// CommandContext carries shared resources that command handlers need.
type CommandContext struct {
	Ctx            context.Context
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
	ClearMessages  func()
}

// CommandRegistry maps slash command names to handlers and descriptions.
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

// Register adds a command handler with description.
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

// AllCommandsWithExecute returns commands with Execute functions wired to emit SlashCommandMsg.
// Used by the command palette.
func (r *CommandRegistry) AllCommandsWithExecute() []CommandInfo {
	names := r.List()
	cmds := make([]CommandInfo, 0, len(names))
	for _, name := range names {
		cmdName := name
		cmds = append(cmds, CommandInfo{
			Name:        cmdName,
			Description: r.descriptions[cmdName],
			Slash:       "/" + cmdName,
			Execute: func() tea.Cmd {
				return func() tea.Msg {
					return SlashCommandMsg{Command: "/" + cmdName}
				}
			},
		})
	}
	return cmds
}

// Execute parses input as a slash command and runs the handler.
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
		suggestion := suggestCommand(r, name)
		if suggestion != "" {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("Unknown command: /%s. Did you mean /%s?", name, suggestion),
			}, true
		}
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Unknown command: /%s. Type /help for available commands.", name),
		}, true
	}
	return handler(args, ctx), true
}

// ParseCommand splits a slash command input into name and args.
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

// suggestCommand returns the closest command by Levenshtein distance (threshold ≤ 2).
func suggestCommand(r *CommandRegistry, input string) string {
	var best string
	bestDist := 3
	for _, cmd := range r.List() {
		d := levenshtein(input, cmd)
		if d > 0 && d < bestDist {
			bestDist = d
			best = cmd
		}
	}
	return best
}

// levenshtein computes the edit distance between two strings.
func levenshtein(a, b string) int {
	la, lb := len(a), len(b)
	if la == 0 {
		return lb
	}
	if lb == 0 {
		return la
	}
	prev := make([]int, lb+1)
	curr := make([]int, lb+1)
	for j := 0; j <= lb; j++ {
		prev[j] = j
	}
	for i := 1; i <= la; i++ {
		curr[0] = i
		for j := 1; j <= lb; j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min(curr[j-1]+1, min(prev[j]+1, prev[j-1]+cost))
		}
		prev, curr = curr, prev
	}
	return prev[lb]
}

// DefaultCommands creates a CommandRegistry pre-populated with all slash commands.
func DefaultCommands() *CommandRegistry {
	r := NewCommandRegistry()

	// Core
	r.Register("help", handleHelp, "List available commands")
	r.Register("clear", handleClear, "Clear conversation")
	r.Register("status", handleStatus, "Show session info")
	r.Register("reset", handleReset, "Reset to first-run screen")
	r.Register("quit", handleQuit, "Exit the application")
	r.Register("undo", handleUndo, "Show latest checkpoint info")
	r.Register("history", handleHistory, "Show conversation history")
	r.Register("health", handleHealth, "Show system health status")
	r.Register("tools", handleTools, "List available tools")

	// Config/settings
	r.Register("settings", handleSettings, "Open settings editor")
	r.Register("config", handleConfig, "Show or set config value")
	r.Register("theme", handleTheme, "Switch dark/light theme")
	r.Register("cost", handleCost, "Toggle cost display")
	r.Register("log", handleLog, "Show recent log entries")
	r.Register("key", handleKey, "Show API key status")
	r.Register("tokens", handleTokens, "Estimate token count")

	// AI/model
	r.Register("compress", handleCompress, "Trigger context consolidation")
	r.Register("memory", handleMemory, "Manage context memory")
	r.Register("optimize", handleOptimize, "Suggest cheaper model alternatives")
	r.Register("model", handleModel, "Show or switch model")
	r.Register("models", handleModels, "List all cached models")
	r.Register("fallback", handleFallback, "Switch provider / show fallback status")
	r.Register("provider", handleProvider, "Show or switch provider")

	// Git
	r.Register("diff", handleDiff, "Show git diff")
	r.Register("rollback", handleRollback, "Browse or reset to commit")
	r.Register("bisect", handleBisect, "Git bisect info")

	// Session
	r.Register("sessions", handleSessions, "List recent sessions")
	r.Register("export", handleExport, "Export session to file")
	r.Register("fork", handleFork, "Fork current session")
	r.Register("prev", handlePrev, "Switch to previous session")
	r.Register("next", handleNext, "Switch to next session")
	r.Register("save", handleSave, "Save current session")
	r.Register("goal", handleGoal, "Set or show session goal")
	r.Register("resume", handleResume, "Open session browser")
	r.Register("ledger", handleLedger, "Show learning ledger")

	// Workflow
	r.Register("new", handleNew, "Start a new workflow")
	r.Register("workflow", handleWorkflow, "Workflow control")
	r.Register("plan", handlePhase, "Alias for /phase plan")
	r.Register("execute", handlePhase, "Alias for /phase execute")
	r.Register("verify", handlePhase, "Alias for /phase verify")
	r.Register("ship", handlePhase, "Alias for /phase ship")
	r.Register("phase", handlePhase, "Show or transition phase")
	r.Register("pause", handlePause, "Pause workflow")
	r.Register("resume-task", handleResumeTask, "Resume workflow")
	r.Register("metrics", handleMetrics, "Open session analytics")

	return r
}

// handleMetrics opens ScreenMetrics.
func handleMetrics(_ []string, _ CommandContext) CommandResult {
	screen := ScreenMetrics
	return CommandResult{Success: true, Screen: &screen, Message: "Opening metrics..."}
}
