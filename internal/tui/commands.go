package tui

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"syscall"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

// CommandResult is the result of executing a slash command. It carries a
// success/failure status, a display message, and optional state transitions
// (screen change, session ID, or config update).
type CommandResult struct {
	Success   bool
	Message   string
	Screen    *Screen
	SessionID *string
	Config    *config.Config
	Cmd       func() tea.Msg // optional tea.Cmd to run after command
}

// CommandHandler is a function that handles a slash command.
// It receives the parsed arguments (without the command name) and a
// CommandContext with access to providers, sessions, git, etc.
type CommandHandler func(args []string, ctx CommandContext) CommandResult

// CommandContext carries shared components that command handlers need to
// inspect or mutate state. All fields are optional — handlers must
// gracefully handle nil receivers.
type CommandContext struct {
	Registry       *provider.Registry
	SessionManager *session.Manager
	SessionID      string
	Config         *config.Config
	Dispatcher     *tools.Dispatcher
	Git            *git.Git
	Ledger         *ledger.Ledger
	Rollback       *rollback.Rollback
	AutoDream      *autodream.Consolidator
	WorkflowEngine *workflow.Engine
}

// CommandRegistry holds a map of registered command handlers and their
// descriptions. Commands are registered at startup via DefaultCommands().
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

// Register adds a command handler with a description to the registry.
// If the command name already exists, it is overwritten.
func (r *CommandRegistry) Register(name string, handler CommandHandler, description string) {
	r.handlers[name] = handler
	r.descriptions[name] = description
}

// Get returns the handler for a command name. Returns nil, false if the
// command is not registered.
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

// AllCommands returns all registered commands as CommandInfo slices
// suitable for populating the command palette.
func (r *CommandRegistry) AllCommands() []CommandInfo {
	names := r.List()
	cmds := make([]CommandInfo, 0, len(names))
	for _, name := range names {
		desc := r.descriptions[name]
		cmds = append(cmds, CommandInfo{
			Name:        name,
			Description: desc,
			Slash:       "/" + name,
		})
	}
	return cmds
}

// Execute parses input as a command, looks up the handler, and runs it.
// Returns (result, true) if the input starts with "/" and was handled
// (including unknown commands). Returns (CommandResult{}, false) if the
// input does not start with "/".
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
// The input must start with "/". Returns ok=false if the input does not
// start with "/", or the name and args otherwise.
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

// DefaultCommands creates a CommandRegistry pre-populated with all 28
// slash commands available in M31A.
func DefaultCommands() *CommandRegistry {
	r := NewCommandRegistry()

	r.Register("help", handleHelp, "List all available commands")
	r.Register("clear", handleClear, "Clear current conversation context")
	r.Register("status", handleStatus, "Show current session info")
	r.Register("model", handleModel, "Show or switch model")
	r.Register("provider", handleProvider, "Show or switch provider")
	r.Register("reset", handleReset, "Reset to first-run screen")
	r.Register("quit", handleQuit, "Exit the application")
	r.Register("undo", handleUndo, "Restore latest checkpoint")
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
	r.Register("run", handleRun, "Execute a shell command directly")

	return r
}

// ---------------------------------------------------------------------------
// Handler implementations
// ---------------------------------------------------------------------------

// handleHelp lists all registered commands with their descriptions.
func handleHelp(args []string, ctx CommandContext) CommandResult {
	// This handler is registered statically — it always has access to the
	// list via the package-level DefaultCommands. We re-create the list
	// here to avoid needing the registry reference in the handler.
	r := DefaultCommands()
	names := r.List()

	var b strings.Builder
	b.WriteString("Available commands:\n")
	for _, name := range names {
		desc := r.descriptions[name]
		b.WriteString(fmt.Sprintf("  /%s — %s\n", name, desc))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleClear returns a confirmation that the context was cleared.
func handleClear(args []string, ctx CommandContext) CommandResult {
	return CommandResult{Success: true, Message: "Context cleared."}
}

// handleStatus returns current session information including session ID,
// active provider, active model, phase, and message count.
func handleStatus(args []string, ctx CommandContext) CommandResult {
	var b strings.Builder
	b.WriteString("Session Info:\n")

	if ctx.SessionID != "" {
		b.WriteString(fmt.Sprintf("  Session ID:  %s\n", ctx.SessionID))
	}

	if ctx.Registry != nil {
		active := ctx.Registry.Active()
		b.WriteString(fmt.Sprintf("  Provider:    %s\n", active))

		if ap := ctx.Registry.ActiveProvider(); ap != nil {
			models, err := ap.FetchModels(context.Background())
			if err == nil && len(models) > 0 {
				// Show the first model as a representative
				b.WriteString(fmt.Sprintf("  Models:      %d cached\n", len(models)))
			}
		}
	}

	// Try to load session for phase and message count
	if ctx.SessionManager != nil && ctx.SessionID != "" {
		s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
		if err == nil {
			b.WriteString(fmt.Sprintf("  Phase:       %s\n", s.WorkflowPhase))
			b.WriteString(fmt.Sprintf("  Messages:    %d\n", s.MessageCount))
		}
	}

	if ctx.Config != nil {
		b.WriteString(fmt.Sprintf("  Model:       %s\n", ctx.Config.Model.Default))
	}

	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleModel shows or switches the active model.
// With no arguments, shows a hint about the current model.
// With a model ID argument, attempts to look it up and show its info.
// With "--selector", triggers the model selector screen.
func handleModel(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		msg := "No model selected. Use /model <name> to view model info, or /models to browse available models."
		if ctx.Config != nil && ctx.Config.Model.Default != "" {
			msg = fmt.Sprintf("Current default model: %s\n", ctx.Config.Model.Default) + msg
		}
		return CommandResult{Success: true, Message: msg}
	}

	if args[0] == "--selector" {
		screen := ScreenModelSelector
		return CommandResult{Success: true, Screen: &screen, Message: "Opening model selector..."}
	}

	// Look up the model via the active provider
	if ctx.Registry != nil {
		ap := ctx.Registry.ActiveProvider()
		if ap != nil {
			modelInfo, err := ap.GetModel(args[0])
			if err != nil || modelInfo == nil {
				return CommandResult{
					Success: false,
					Message: fmt.Sprintf("Model %q not found in provider %q.", args[0], ctx.Registry.Active()),
				}
			}
			return CommandResult{
				Success: true,
				Message: fmt.Sprintf(
					"Model: %s\n  Provider:   %s\n  Context:    %d tokens\n  Pricing:    $%.4f / $%.4f (in/out per MTok)",
					modelInfo.ID, modelInfo.Provider, modelInfo.ContextLength,
					modelInfo.Pricing.InputPerMToken, modelInfo.Pricing.OutputPerMToken,
				),
			}
		}
	}

	return CommandResult{Success: false, Message: "No provider available to look up models."}
}

// handleProvider shows the active provider or switches to a different one.
// Registry.SetActive validates that the provider exists before switching.
func handleProvider(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Registry not available."}
	}

	if len(args) == 0 {
		active := ctx.Registry.Active()
		providers := strings.Join(ctx.Registry.List(), ", ")
		if active == "" {
			return CommandResult{Success: false, Message: "No active provider. Available: " + providers}
		}
		return CommandResult{Success: true, Message: fmt.Sprintf("Active provider: %s\nAvailable: %s", active, providers)}
	}

	if err := ctx.Registry.SetActive(args[0]); err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Cannot switch to provider %q: %v. Available: %s", args[0], err, strings.Join(ctx.Registry.List(), ", ")),
		}
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("Switched to provider: %s", args[0])}
}

// handleReset returns a result that transitions the TUI to the first-run screen.
func handleReset(args []string, ctx CommandContext) CommandResult {
	// Clear conversation history if available
	if ctx.AutoDream != nil {
		ctx.AutoDream.SetMessages(nil)
	}
	screen := ScreenFirstRun
	return CommandResult{Success: true, Screen: &screen, Message: "Resetting to first-run..."}
}

// handleQuit returns a goodbye message. The TUI is expected to detect the
// quit message in the AppState.Update() and call tea.Quit.
func handleQuit(args []string, ctx CommandContext) CommandResult {
	return CommandResult{Success: true, Message: "Goodbye!"}
}

// handleUndo loads the latest checkpoint and reports its details.
func handleUndo(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session. Start a session first."}
	}

	cp, err := ctx.SessionManager.LatestCheckpoint(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("No checkpoint available: %v", err)}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Latest checkpoint: phase=%s, timestamp=%s, messages=%d, tasks=%d",
			cp.Phase, cp.Timestamp.Format("2006-01-02 15:04:05"), cp.MessageCount, cp.TaskCount),
	}
}

// handleCompress triggers AutoDream context consolidation.
func handleCompress(args []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream == nil {
		return CommandResult{Success: false, Message: "AutoDream not available."}
	}

	result := ctx.AutoDream.Consolidate()
	if result.Error != "" {
		return CommandResult{Success: false, Message: fmt.Sprintf("Consolidation failed: %s", result.Error)}
	}
	return CommandResult{Success: true, Message: result.Summary}
}

// handleLedger shows session history from the learning ledger.
// Subcommands:
//
//	/ledger        — last 5 entries
//	/ledger stats  — aggregate statistics
//	/ledger <type> — entries filtered by project type
func handleLedger(args []string, ctx CommandContext) CommandResult {
	if ctx.Ledger == nil {
		return CommandResult{Success: false, Message: "Ledger not available."}
	}

	if len(args) > 0 && args[0] == "stats" {
		stats := ctx.Ledger.Stats()
		var b strings.Builder
		b.WriteString(fmt.Sprintf("Total sessions:      %d\n", stats.TotalSessions))
		b.WriteString(fmt.Sprintf("Avg tasks/session:   %.1f\n", stats.AvgTaskCount))
		b.WriteString(fmt.Sprintf("Avg cost:            $%.4f\n", stats.AvgCost))
		b.WriteString(fmt.Sprintf("Avg duration:        %.0f min\n", stats.AvgDurationMinutes))
		b.WriteString(fmt.Sprintf("Total failed tasks:  %d\n", stats.TotalFailedTasks))
		if len(stats.TopFrameworks) > 0 {
			b.WriteString(fmt.Sprintf("Top frameworks:      %s\n", strings.Join(stats.TopFrameworks, ", ")))
		}
		if len(stats.TopFailures) > 0 {
			b.WriteString(fmt.Sprintf("Top failures:        %s", strings.Join(stats.TopFailures, ", ")))
		}
		return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
	}

	if len(args) > 0 {
		// Filter by project type
		entries := ctx.Ledger.EntriesFiltered(args[0], nil, 5)
		return formatLedgerEntries(entries, fmt.Sprintf("Recent sessions (type: %s)", args[0]))
	}

	// No args: last 5 entries
	entries := ctx.Ledger.Entries()
	return formatLedgerEntries(entries, "Recent sessions")
}

// handleRollback shows the commit chain or performs a rollback.
// Subcommands:
//
//	/rollback           — show last 10 commits
//	/rollback <hash>    — soft reset to commit
//	/rollback --hard <hash> — hard reset to commit
func handleRollback(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		// Show commit chain
		if ctx.Rollback != nil {
			entries, err := ctx.Rollback.Chain(10)
			if err != nil {
				return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get commit chain: %v", err)}
			}
			commits := make([]git.CommitInfo, len(entries))
			for i, e := range entries {
				commits[i] = e.CommitInfo
			}
			return formatCommitChain(commits)
		}

		// Fallback to raw git log
		if ctx.Git != nil {
			commits, err := ctx.Git.Log(false, "")
			if err != nil {
				return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get git log: %v", err)}
			}
			if len(commits) > 10 {
				commits = commits[:10]
			}
			return formatCommitChain(commits)
		}

		return CommandResult{Success: false, Message: "No git repository available."}
	}

	// Rollback to a specific commit
	if ctx.Rollback == nil {
		return CommandResult{Success: false, Message: "Rollback not available."}
	}

	if args[0] == "--hard" && len(args) > 1 {
		result, err := ctx.Rollback.HardReset(args[1])
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Hard reset failed: %v", err)}
		}
		return CommandResult{Success: true, Message: result.Message}
	}

	result, err := ctx.Rollback.SoftReset(args[0])
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Soft reset failed: %v", err)}
	}
	return CommandResult{Success: true, Message: result.Message}
}

// handleFallback shows the active provider and available alternatives, or
// switches to a specified provider.
func handleFallback(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	active := ctx.Registry.Active()
	providers := ctx.Registry.List()

	if len(args) == 0 {
		if active == "" {
			return CommandResult{Success: false, Message: "No active provider. Available: " + strings.Join(providers, ", ")}
		}
		var b strings.Builder
		b.WriteString(fmt.Sprintf("Active provider: %s\n", active))
		b.WriteString(fmt.Sprintf("Available: %s\n", strings.Join(providers, ", ")))
		if len(providers) > 1 {
			b.WriteString("Use /fallback <provider_name> to switch.")
		}
		return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
	}

	target := args[0]
	if target == active {
		return CommandResult{Success: false, Message: fmt.Sprintf("Already using provider %q.", target)}
	}

	// Validate target is a registered provider
	valid := false
	for _, p := range providers {
		if p == target {
			valid = true
			break
		}
	}
	if !valid {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Provider %q not found. Available: %s", target, strings.Join(providers, ", ")),
		}
	}

	if err := ctx.Registry.SetActive(target); err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Cannot switch to provider %q: %v.", target, err),
		}
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("Switched from %s to %s. Use /status to confirm.", active, target)}
}

func formatCommitChain(commits []git.CommitInfo) CommandResult {
	if len(commits) == 0 {
		return CommandResult{Success: true, Message: "No commits in this repository."}
	}

	var b strings.Builder
	b.WriteString("Recent commits:\n")
	for i, c := range commits {
		marker := ""
		if i == 0 {
			marker = " [HEAD]"
		}
		shortHash := c.ShortHash
		if shortHash == "" && len(c.Hash) > 7 {
			shortHash = c.Hash[:7]
		} else if shortHash == "" {
			shortHash = c.Hash
		}
		b.WriteString(fmt.Sprintf("  %s %s%s\n", shortHash, c.Message, marker))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

func formatLedgerEntries(entries []ledger.LedgerEntry, title string) CommandResult {
	if len(entries) == 0 {
		return CommandResult{Success: true, Message: fmt.Sprintf("%s: none", title)}
	}

	if len(entries) > 5 {
		entries = entries[:5]
	}

	var b strings.Builder
	b.WriteString(title + ":\n")
	for i, e := range entries {
		b.WriteString(fmt.Sprintf("  %d. %s | %s/%s | %d msgs | %s\n",
			i+1, e.SessionID, e.Provider, e.Model, e.TaskCount, e.Timestamp.Format("Jan 02 15:04")))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleSessions lists recent sessions from the session manager.
func handleSessions(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil {
		return CommandResult{Success: false, Message: "Session manager not available."}
	}

	sessions, err := ctx.SessionManager.ListSessions()
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to list sessions: %v", err)}
	}

	if len(sessions) == 0 {
		return CommandResult{Success: true, Message: "No sessions available."}
	}

	var b strings.Builder
	b.WriteString("Sessions:\n")
	for i, s := range sessions {
		corrupt := ""
		if s.Corrupted {
			corrupt = " [!CORRUPT]"
		}
		b.WriteString(fmt.Sprintf("  %d. %s | %s/%s | %d msgs%s\n",
			i+1, s.ID, s.Provider, s.Model, s.MessageCount, corrupt))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleGoal shows or sets the session goal.
func handleGoal(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		// Show current goal
		if ctx.SessionManager != nil && ctx.SessionID != "" {
			s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
			if err == nil && s.Project != nil && s.Project.Goal != "" {
				return CommandResult{Success: true, Message: fmt.Sprintf("Session goal: %s", s.Project.Goal)}
			}
		}
		return CommandResult{Success: true, Message: "No goal set. Use /goal <your goal> to set one."}
	}

	goal := strings.Join(args, " ")

	// Persist goal to session
	if ctx.SessionManager != nil && ctx.SessionID != "" {
		s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
		if err == nil {
			if s.Project == nil {
				s.Project = &types.ProjectState{Goal: goal}
			} else {
				s.Project.Goal = goal
			}
			ctx.SessionManager.SaveProject(ctx.SessionID, s.Project)
		}
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("Goal set: %s", goal)}
}

// handlePhase shows or transitions the current workflow phase.
func handlePhase(args []string, ctx CommandContext) CommandResult {
	validPhases := []types.WorkflowPhase{
		types.PhaseIdle, types.PhaseInitialize, types.PhaseDiscuss,
		types.PhasePlan, types.PhaseExecute, types.PhaseVerify, types.PhaseShip,
	}

	if len(args) == 0 {
		// Show current phase
		if ctx.SessionManager != nil && ctx.SessionID != "" {
			s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
			if err == nil {
				return CommandResult{Success: true, Message: fmt.Sprintf("Current phase: %s", s.WorkflowPhase)}
			}
		}
		return CommandResult{Success: true, Message: fmt.Sprintf("Current phase: %s", types.PhaseIdle)}
	}

	phaseName := args[0]
	for _, p := range validPhases {
		if string(p) == phaseName {
			// Persist phase transition to session
			if ctx.SessionManager != nil && ctx.SessionID != "" {
				ctx.SessionManager.SaveState(ctx.SessionID, p, "manual transition", "user requested /phase "+phaseName)
			}
			return CommandResult{Success: true, Message: fmt.Sprintf("Phase transition to %s.", phaseName)}
		}
	}

	return CommandResult{
		Success: false,
		Message: fmt.Sprintf("Invalid phase %q. Valid phases: idle, initialize, discuss, plan, execute, verify, ship", phaseName),
	}
}

// handleConfig displays or modifies configuration values.
func handleConfig(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	if len(args) == 0 {
		return formatConfig(ctx.Config)
	}

	// Setting a config value (V1: simple key=value via dot notation)
	if len(args) >= 2 {
		key := args[0]
		value := args[1]

		switch key {
		case "ui.theme":
			ctx.Config.UI.Theme = value
			return CommandResult{Success: true, Message: fmt.Sprintf("ui.theme set to %q", value)}
		case "model.default":
			ctx.Config.Model.Default = value
			return CommandResult{Success: true, Message: fmt.Sprintf("model.default set to %q", value)}
		case "ui.compact_mode":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.UI.CompactMode = v
				return CommandResult{Success: true, Message: fmt.Sprintf("ui.compact_mode set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "ui.show_token_usage":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.UI.ShowTokenUsage = v
				return CommandResult{Success: true, Message: fmt.Sprintf("ui.show_token_usage set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "provider.auto_fallback":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.Provider.AutoFallback = v
				return CommandResult{Success: true, Message: fmt.Sprintf("provider.auto_fallback set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		default:
			return CommandResult{Success: false, Message: fmt.Sprintf("Unknown config key: %q. Use key names like \"ui.theme\", \"model.default\"", key)}
		}
	}

	return CommandResult{Success: false, Message: "Usage: /config [key value]"}
}

func formatConfig(cfg *config.Config) CommandResult {
	var b strings.Builder
	b.WriteString("Configuration:\n")
	b.WriteString(fmt.Sprintf("  provider.default:       %s\n", cfg.Provider.Default))
	b.WriteString(fmt.Sprintf("  provider.auto_fallback: %v\n", cfg.Provider.AutoFallback))
	b.WriteString(fmt.Sprintf("  model.default:          %s\n", cfg.Model.Default))
	b.WriteString(fmt.Sprintf("  model.arbitrage:        %v\n", cfg.Model.AutoArbitrage))
	b.WriteString(fmt.Sprintf("  model.arbitrage_thresh: %.2f\n", cfg.Model.ArbitrageThreshold))
	b.WriteString(fmt.Sprintf("  ui.theme:               %s\n", cfg.UI.Theme))
	b.WriteString(fmt.Sprintf("  ui.compact_mode:        %v\n", cfg.UI.CompactMode))
	b.WriteString(fmt.Sprintf("  ui.show_token_usage:    %v\n", cfg.UI.ShowTokenUsage))
	b.WriteString(fmt.Sprintf("  ui.show_cost_estimate:  %v\n", cfg.UI.ShowCostEstimate))
	b.WriteString(fmt.Sprintf("  permissions.mode:       %s\n", cfg.Permissions.DefaultMode))
	b.WriteString(fmt.Sprintf("  ledger.enabled:         %v\n", cfg.Ledger.Enabled))
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleModels lists all cached models from the active provider.
func handleModels(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	ap := ctx.Registry.ActiveProvider()
	if ap == nil {
		return CommandResult{Success: false, Message: "No active provider."}
	}

	models, err := ap.FetchModels(context.Background())
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to fetch models: %v", err)}
	}

	if len(models) == 0 {
		return CommandResult{Success: true, Message: "No models cached."}
	}

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Cached models (%s):\n", ctx.Registry.Active()))
	for i, m := range models {
		if i >= 20 {
			b.WriteString(fmt.Sprintf("  ... and %d more\n", len(models)-20))
			break
		}
		b.WriteString(fmt.Sprintf("  %s | ctx=%d | $%.4f/$%.4f\n",
			m.ID, m.ContextLength, m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// ---------------------------------------------------------------------------
// Additional handler implementations
// ---------------------------------------------------------------------------

// handleTools lists all registered tools with their descriptions and risk levels.
func handleTools(args []string, ctx CommandContext) CommandResult {
	if ctx.Dispatcher == nil {
		return CommandResult{Success: false, Message: "Dispatcher not available."}
	}

	names := ctx.Dispatcher.List()
	if len(names) == 0 {
		return CommandResult{Success: true, Message: "No tools registered."}
	}

	var b strings.Builder
	b.WriteString("Available tools:\n")
	for _, name := range names {
		tool, ok := ctx.Dispatcher.GetTool(name)
		if !ok {
			continue
		}
		risk := "safe"
		switch tool.RiskLevel() {
		case types.RiskDangerous:
			risk = "dangerous"
		case types.RiskDestructive:
			risk = "destructive"
		}
		b.WriteString(fmt.Sprintf("  %-15s [%-12s] %s\n", name, risk, tool.Description()))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleWorkflow shows the current workflow phase, tasks, and progress.
func handleWorkflow(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session. Start a workflow first."}
	}

	s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}

	var b strings.Builder
	b.WriteString("Workflow Status:\n")
	b.WriteString(fmt.Sprintf("  Session:  %s\n", ctx.SessionID))
	b.WriteString(fmt.Sprintf("  Phase:    %s\n", s.WorkflowPhase))

	// Show task progress
	if ctx.SessionManager != nil {
		tasks, err := ctx.SessionManager.LoadTasks(ctx.SessionID)
		if err == nil && len(tasks) > 0 {
			total, done, failed, skipped := 0, 0, 0, 0
			for _, t := range tasks {
				total++
				switch t.Status {
				case types.StatusDone:
					done++
				case types.StatusFailed:
					failed++
				case types.StatusSkipped:
					skipped++
				}
			}
			b.WriteString(fmt.Sprintf("  Tasks:    %d total, %d done, %d failed, %d skipped\n", total, done, failed, skipped))
		}
	}

	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleHistory shows the conversation message history.
func handleHistory(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}

	limit := 10
	if len(args) > 0 {
		if n, err := strconv.Atoi(args[0]); err == nil && n > 0 {
			limit = n
		}
	}

	msgs := s.Messages
	if len(msgs) == 0 {
		return CommandResult{Success: true, Message: "No messages in this session."}
	}

	if len(msgs) > limit {
		msgs = msgs[len(msgs)-limit:]
	}

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Recent messages (last %d of %d):\n", len(msgs), len(s.Messages)))
	for i, m := range msgs {
		preview := m.Content
		if len(preview) > 80 {
			preview = preview[:77] + "..."
		}
		b.WriteString(fmt.Sprintf("  %d. [%s] %s\n", i+1, m.Role, preview))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleDiff shows git diff of uncommitted changes.
func handleDiff(args []string, ctx CommandContext) CommandResult {
	if ctx.Git == nil {
		return CommandResult{Success: false, Message: "No git repository available."}
	}

	var diff string
	var err error

	if len(args) > 0 && args[0] == "--staged" {
		diff, err = ctx.Git.DiffStaged()
	} else {
		diff, err = ctx.Git.Diff("", "")
	}

	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get diff: %v", err)}
	}

	if diff == "" {
		if len(args) > 0 && args[0] == "--staged" {
			return CommandResult{Success: true, Message: "No staged changes."}
		}
		return CommandResult{Success: true, Message: "No uncommitted changes."}
	}

	// Truncate long diffs
	if len(diff) > 4096 {
		diff = diff[:4096] + "\n\n... [diff truncated, use git diff for full output]"
	}

	return CommandResult{Success: true, Message: diff}
}

// handleTheme switches between dark and light themes.
func handleTheme(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	if len(args) == 0 {
		return CommandResult{Success: true, Message: fmt.Sprintf("Current theme: %s. Use /theme dark or /theme light to switch.", ctx.Config.UI.Theme)}
	}

	switch args[0] {
	case "dark", "light":
		ctx.Config.UI.Theme = args[0]
		return CommandResult{Success: true, Message: fmt.Sprintf("Theme switched to %s. Restart required for full effect.", args[0])}
	default:
		return CommandResult{Success: false, Message: "Invalid theme. Use 'dark' or 'light'."}
	}
}

// handleSave saves the current session and conversation state.
func handleSave(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session to save."}
	}

	if err := ctx.SessionManager.SaveState(ctx.SessionID, "", "manual save", "user requested /save"); err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to save session: %v", err)}
	}

	return CommandResult{
		Success: true,
		Message: "Session saved successfully.",
		Cmd:     func() tea.Msg { return SettingsSavedMsg{} },
	}
}

// handleKey shows API key status and resolution source.
func handleKey(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	var b strings.Builder
	b.WriteString("API Key Status:\n")

	// Check OpenRouter key
	if ctx.Config.Provider.OpenRouter.APIKey != "" {
		key := ctx.Config.Provider.OpenRouter.APIKey
		masked := "***" + key[len(key)-4:]
		b.WriteString(fmt.Sprintf("  OpenRouter:  %s (from config)\n", masked))
	} else {
		b.WriteString("  OpenRouter:  not set\n")
	}

	// Check Zen key
	if ctx.Config.Provider.Zen.APIKey != "" {
		key := ctx.Config.Provider.Zen.APIKey
		masked := "***" + key[len(key)-4:]
		b.WriteString(fmt.Sprintf("  Zen:         %s (from config)\n", masked))
	} else {
		b.WriteString("  Zen:         not set\n")
	}

	b.WriteString("\nKey resolution order: environment variable → OS keychain → config file")
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleLog shows recent log entries.
func handleLog(args []string, ctx CommandContext) CommandResult {
	// Default log path
	home, err := os.UserHomeDir()
	if err != nil {
		return CommandResult{Success: false, Message: "Cannot determine home directory for log path."}
	}
	logPath := filepath.Join(home, ".m31a", "m31a.log")

	entries, err := os.ReadFile(logPath)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Cannot read log file: %v", err)}
	}

	lines := strings.Split(string(entries), "\n")
	n := 20
	if len(args) > 0 {
		if count, err := strconv.Atoi(args[0]); err == nil && count > 0 {
			n = count
		}
	}

	if len(lines) > n {
		lines = lines[len(lines)-n:]
	}

	// Filter out empty lines
	var filtered []string
	for _, line := range lines {
		if strings.TrimSpace(line) != "" {
			filtered = append(filtered, line)
		}
	}

	if len(filtered) == 0 {
		return CommandResult{Success: true, Message: "No log entries found."}
	}

	return CommandResult{Success: true, Message: strings.Join(filtered, "\n")}
}

// handleTokens estimates token count for provided text.
func handleTokens(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		return CommandResult{Success: false, Message: "Usage: /tokens <text to estimate>"}
	}

	text := strings.Join(args, " ")

	// Use rough estimation: ~4 chars per token for English text
	runes := len([]rune(text))
	estimated := float64(runes) / 4.0

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Token estimation:\n"))
	b.WriteString(fmt.Sprintf("  Characters: %d\n", runes))
	b.WriteString(fmt.Sprintf("  Words:      %d\n", len(strings.Fields(text))))
	b.WriteString(fmt.Sprintf("  Est. tokens: ~%.0f (English text, ~4 chars/token)\n", estimated))
	b.WriteString("Note: Actual token count depends on model tokenizer.")
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleHealth shows system health status.
func handleHealth(args []string, ctx CommandContext) CommandResult {
	var b strings.Builder
	b.WriteString("Health Status:\n")

	// Provider health
	if ctx.Registry != nil {
		active := ctx.Registry.Active()
		if active != "" {
			b.WriteString(fmt.Sprintf("  Provider:    %s (active)\n", active))
		} else {
			b.WriteString("  Provider:    none configured\n")
		}
	}

	// Session health
	if ctx.SessionManager != nil && ctx.SessionID != "" {
		b.WriteString(fmt.Sprintf("  Session:     %s (active)\n", ctx.SessionID))
	} else {
		b.WriteString("  Session:     none active\n")
	}

	// Git health
	if ctx.Git != nil {
		b.WriteString("  Git:         available\n")
	} else {
		b.WriteString("  Git:         not available\n")
	}

	// Tools health
	if ctx.Dispatcher != nil {
		toolCount := len(ctx.Dispatcher.List())
		b.WriteString(fmt.Sprintf("  Tools:       %d registered\n", toolCount))
	}

	// Disk space (best effort)
	home, err := os.UserHomeDir()
	if err == nil {
		var statfs syscall.Statfs_t
		if err := syscall.Statfs(home, &statfs); err == nil {
			availGB := float64(statfs.Bavail*uint64(statfs.Bsize)) / 1e9
			b.WriteString(fmt.Sprintf("  Disk:        %.1f GB available\n", availGB))
		}
	}

	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleRun executes a shell command directly.
func handleRun(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		return CommandResult{Success: false, Message: "Usage: /run <command>"}
	}

	if ctx.Dispatcher == nil {
		return CommandResult{Success: false, Message: "Dispatcher not available."}
	}

	cmd := strings.Join(args, " ")

	// Execute via Bash tool
	bash, ok := ctx.Dispatcher.GetTool("Bash")
	if !ok {
		return CommandResult{Success: false, Message: "Bash tool not available."}
	}

	input := types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"command": cmd},
	}

	result, err := bash.Execute(context.Background(), input)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Command failed: %v", err)}
	}

	output := result.Output
	if len(output) > 4096 {
		output = output[:4096] + "\n\n... [output truncated]"
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("$ %s\n%s", cmd, output)}
}
