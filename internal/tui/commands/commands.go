package commands

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/history"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/skills"
)

// CommandInfo describes a slash command for autocomplete and help.
type CommandInfo struct {
	Name        string
	Description string
	Slash       string         // e.g. "/help"
	Execute     func() tea.Cmd // optional: used by command palette
}

// CommandResult is the result of executing a slash command.
type CommandResult struct {
	Success         bool
	Message         string
	Screen          *tuitypes.Screen
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
	Ctx             context.Context
	Registry        *provider.Registry
	SessionManager  *session.Manager
	SessionID       string
	Config          *config.Config
	ConfigPath      string
	Dispatcher      *tools.Dispatcher
	Git             *git.Git
	Ledger          *ledger.Ledger
	Rollback        *rollback.Rollback
	AutoDream       *autodream.Consolidator
	WorkflowEngine  tuitypes.WorkflowEngine
	CmdRegistry     *CommandRegistry
	FrecentHistory  *history.FrecentHistory
	SubagentManager *subagent.Manager
	ClearMessages   func()
	FlushViewport   func()
	CopyError       func() tea.Cmd
	AgentMode       *bool
	SetAgentMode    func(bool)
	CancelAgent     func()
	Version         string
	Keychain        keychain.Keychain
}

// CommandRegistry maps slash command names to handlers and descriptions.
type CommandRegistry struct {
	handlers     map[string]CommandHandler
	descriptions map[string]string
}

// NewCommandRegistry creates an empty CommandRegistry.
func NewCommandRegistry() *CommandRegistry {
	return &CommandRegistry{
		handlers:     make(map[string]CommandHandler),
		descriptions: make(map[string]string),
	}
}

// Register adds a command handler with description. Returns an error if a
// command with the same name is already registered.
func (r *CommandRegistry) Register(name string, handler CommandHandler, description string) error {
	if _, exists := r.handlers[name]; exists {
		return fmt.Errorf("command already registered: %s", name)
	}
	r.handlers[name] = handler
	r.descriptions[name] = description
	return nil
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
					return tuitypes.SlashCommandMsg{Command: "/" + cmdName}
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
	return tools.LevenshteinDistance(a, b)
}

// DefaultCommands creates a CommandRegistry pre-populated with all slash commands.
func DefaultCommands() *CommandRegistry {
	r := NewCommandRegistry()

	// Core
	_ = r.Register("help", handleHelp, "List available commands")
	_ = r.Register("clear", handleClear, "Clear conversation")
	_ = r.Register("status", handleStatus, "Show session info")
	_ = r.Register("reset", handleReset, "Reset to factory state (deletes config, keys, sessions)")
	_ = r.Register("quit", handleQuit, "Exit the application")
	_ = r.Register("exit", handleExit, "Exit the application (alias for /quit)")
	_ = r.Register("chat", handleChat, "Start a new chat session (clear messages)")
	_ = r.Register("flush", handleFlush, "Clear screen and reset view")
	_ = r.Register("search", handleSearch, "Search conversation messages")
	_ = r.Register("about", handleAbout, "Show version and system info")
	_ = r.Register("undo", handleUndo, "Show latest checkpoint info")
	_ = r.Register("history", handleHistory, "Show chat history table browser")
	_ = r.Register("prompt-history", handlePromptHistory, "Show recent prompt history")
	_ = r.Register("health", handleHealth, "Show system health status")
	_ = r.Register("tools", handleTools, "List available tools")
	_ = r.Register("copy-error", handleCopyError, "Copy last error to clipboard")

	// Config/settings
	_ = r.Register("settings", handleSettings, "Open settings editor")
	_ = r.Register("config", handleConfig, "Open full config editor (all sections, editable)")
	_ = r.Register("cost", handleCost, "Toggle cost display")
	_ = r.Register("log", handleLog, "Show recent log entries")
	_ = r.Register("key", handleKey, "Show API key status")
	_ = r.Register("keychain", handleKey, "Show API key status (alias for /key)")
	_ = r.Register("tokens", handleTokens, "Estimate token count")
	_ = r.Register("dream", handleMemory, "Manage context memory (alias for /memory)")

	// AI/model
	_ = r.Register("compress", handleCompress, "Trigger context consolidation")
	_ = r.Register("memory", handleMemory, "Manage context memory")
	_ = r.Register("optimize", handleOptimize, "Suggest cheaper model alternatives")
	_ = r.Register("model", handleModel, "Show or switch model")
	_ = r.Register("fallback", handleFallback, "Switch provider / show fallback status")
	_ = r.Register("provider", handleProvider, "Show or switch provider")

	// Git
	_ = r.Register("diff", handleDiff, "Show git diff")
	_ = r.Register("rollback", handleRollback, "Browse or reset to commit")
	_ = r.Register("bisect", handleBisect, "Git bisect info")

	// Session
	_ = r.Register("sessions", handleSessions, "List recent sessions")
	_ = r.Register("export", handleExport, "Export session to file")
	_ = r.Register("fork", handleFork, "Fork current session")
	_ = r.Register("prev", handlePrev, "Switch to previous session")
	_ = r.Register("next", handleNext, "Switch to next session")
	_ = r.Register("save", handleSave, "Save current session")
	_ = r.Register("goal", handleGoal, "Set or show session goal")
	_ = r.Register("resume", handleResume, "Open session browser")
	_ = r.Register("ledger", handleLedger, "Show learning ledger")

	// Workflow
	_ = r.Register("new", handleNew, "Start a new workflow")
	_ = r.Register("workflow", handleWorkflow, "Workflow control")
	_ = r.Register("plan", handlePhase, "Alias for /phase plan")
	_ = r.Register("refine", handleRefine, "Refine the current plan with feedback")
	_ = r.Register("execute", handlePhase, "Alias for /phase execute")
	_ = r.Register("verify", handlePhase, "Alias for /phase verify")
	_ = r.Register("runtime", handlePhase, "Alias for /phase runtime")
	_ = r.Register("ship", handlePhase, "Alias for /phase ship")
	_ = r.Register("phase", handlePhase, "Show or transition phase")
	_ = r.Register("pause", handlePause, "Pause workflow")
	_ = r.Register("resume-task", handleResumeTask, "Resume workflow")
	_ = r.Register("agent-mode", handleAgentMode, "Toggle autonomous agent mode")
	_ = r.Register("metrics", handleMetrics, "Open session analytics")
	_ = r.Register("dashboard", handleDashboard, "Open workflow dashboard")
	_ = r.Register("notifications", handleNotifications, "Open notification center")
	_ = r.Register("files", handleFiles, "Open file explorer")
	_ = r.Register("ghost", handleGhost, "Ghost write files")

	// Subagents
	_ = r.Register("agent", handleAgent, "Spawn a parallel subagent (or list active)")
	_ = r.Register("agent-cancel", handleAgentCancel, "Cancel a running subagent")

	// Analysis
	_ = r.Register("complexity", handleComplexity, "Show codebase complexity report")

	// Discover and register skills as dynamic slash commands
	discoverAndRegisterSkills(r)

	return r
}

// handleMetrics opens ScreenMetrics.
func handleMetrics(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenMetrics
	return CommandResult{Success: true, Screen: &screen, Message: "Opening metrics..."}
}

// handleDashboard opens ScreenDashboard.
func handleDashboard(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenDashboard
	return CommandResult{Success: true, Screen: &screen, Message: "Opening workflow dashboard..."}
}

// handleNotifications opens ScreenNotifications.
func handleNotifications(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenNotifications
	return CommandResult{Success: true, Screen: &screen, Message: "Opening notifications..."}
}

// handleFiles opens ScreenFileExplorer.
func handleFiles(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenFileExplorer
	return CommandResult{Success: true, Screen: &screen, Message: "Opening file explorer..."}
}

// handleGhost opens the ghost picker screen for ghost write operations.
func handleGhost(_ []string, ctx CommandContext) CommandResult {
	screen := tuitypes.ScreenGhostPicker
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Opening ghost write file selector...",
	}
}

// discoverAndRegisterSkills scans for skill files and registers them as
// dynamic slash commands in the command registry.
func discoverAndRegisterSkills(r *CommandRegistry) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return
	}
	baseDir := filepath.Join(homeDir, ".m31a")

	cwd, _ := os.Getwd()
	projectDir := cwd

	discovered := skills.Discover(baseDir, projectDir)
	for _, skill := range discovered {
		if !skill.Slash || !skill.IsValid() {
			continue
		}
		skillContent := skill.Content
		skillName := skill.Name
		_ = r.Register(skillName, func(args []string, ctx CommandContext) CommandResult {
			return CommandResult{
				Success: true,
				Message: skillContent,
			}
		}, skill.Description)
	}
}
