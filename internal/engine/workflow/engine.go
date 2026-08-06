// Package workflow implements the seven-phase M31A workflow engine.
//
// The engine is split across multiple files by concern:
//   - engine.go: Core Engine struct, lifecycle, RunPhase, and orchestration
//   - engine_pause.go: Pause/resume/skip/cancel for execute phase
//   - engine_streaming.go: LLM streaming and token estimation
//   - engine_checkpoint.go: Checkpoint save/load, recovery, and rollback
//   - engine_model.go: Per-phase model selection and routing
//   - engine_helpers.go: Config adapters, caching, template extraction
//   - engine_concurrency.go: Lock ordering documentation
//   - engine_discuss.go: Discussion phase support and refinement feedback
//   - engine_context.go: Context management and proactive compaction
//   - engine_tools.go: Tool definitions, system prompt, and code intelligence
package workflow

import (
	"context"
	"embed"
	"fmt"
	"log/slog"
	"os/exec"
	"path/filepath"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/compaction"
	"github.com/eshanized/M31A/internal/engine/decision"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	ctxsrc "github.com/eshanized/M31A/internal/integrations/context"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/ledger"
	"github.com/eshanized/M31A/internal/integrations/metrics"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/tools"
)

//go:embed templates/website-nextjs/*
var websiteTemplateFS embed.FS

// Engine orchestrates the seven active workflow phases (Initialize, Discuss, Plan, Execute, Verify, Runtime, Ship).
type Engine struct {
	sessionID        string
	workDir          string
	backupDir        string
	planningDir      string
	provider         provider.LLMProvider
	modelID          string
	modelIDMu        sync.RWMutex
	stateMachine     *StateMachine
	cache            *WorkflowCache
	cfg              *config.Config
	git              *git.Git
	dispatcher       *tools.Dispatcher
	tokens           *tokens.Estimator
	sessionMgr       *session.Manager
	promptBuilder    *PromptBuilder
	logger           *slog.Logger
	startTime        time.Time
	sessionStartHash string
	discussState     DiscussState
	execCommand      func(name string, args ...string) *exec.Cmd
	msgEmitter       MsgEmitter
	callCounter      int64
	costTracker      *CostTracker
	contextBuilder   *ContextBuilder
	// workflowMode controls phase-skipping behaviour based on prompt complexity.
	workflowMode   m31types.WorkflowMode
	workflowModeMu sync.RWMutex
	// perPhaseModels holds per-phase model overrides set by the TUI via SetPhaseModel.
	// Keys are WorkflowPhase values; values are model ID strings.
	// When set, takes precedence over AgentsConfig and cfg.Model.Default.
	perPhaseModels   map[m31types.WorkflowPhase]string
	perPhaseModelsMu sync.RWMutex
	// Codebase intelligence layer (lazy-built, invalidated between execute groups)
	codeIntel      *codeintel.Indexer
	codeIntelMu    sync.Mutex
	codeIntelBuilt bool
	// websiteTemplateDir holds the path to the extracted website template directory.
	// Set when scope includes "website"; used to inject template path into plan/execute context.
	websiteTemplateDir string
	// Shared ledger instance for session record persistence (uses the
	// application-configured path, not a hardcoded ~/.m31a/LEDGER.md).
	ledger *ledger.Ledger
	// compactor manages automatic session compaction when context fills up.
	compactor *compaction.Compactor
	// contextRegistry manages dynamic system context sources.
	contextRegistry *ctxsrc.Registry
	// collector captures session metrics (tool calls, LLM usage, phase durations, heals).
	collector *metrics.Collector
	// toolCallsSinceLastCompact counts tool calls since last proactive compaction check.
	toolCallsSinceLastCompact int
	// phaseCoordinator delegates pre-phase setup, post-phase metrics, and transition side effects.
	phaseCoordinator *PhaseCoordinator
	// cacheMu protects cache from concurrent access during shutdown
	cacheMu sync.RWMutex
	// state groups mutable session state (plan, cache, intent, v1.5 subsystems).
	state *WorkflowState
	// done is closed when the workflow completes or is shut down.
	done chan struct{}
	// ctx is the root context for the engine's lifecycle; cancelled via cancel.
	ctx context.Context
	// cancel cancels the running workflow context on shutdown.
	cancel context.CancelFunc
	// recoveryPath is the path to the recovery state file for this session.
	// Set during initialization to sessionDir/.m31a/recovery.json.
	recoveryPath string

	// pause/resume support for execute phase
	pauseMu       sync.Mutex
	pauseCh       chan struct{} // closed when paused; nil when running
	resumeCh      chan struct{} // closed when resumed; nil when running
	skipTaskCh    chan int      // send task ID to skip (while paused)
	cancelTaskCh  chan int      // send task ID to cancel (while paused)
	cancelGroupCh chan struct{} // closed to cancel entire group (while paused)
}

// Close flushes and shuts down the decision logger. Safe to call multiple times.
func (e *Engine) Close() {
	if e.state != nil {
		dl := e.state.DecisionLog()
		if dl != nil {
			dl.Close()
		}
	}
}

// Shutdown gracefully stops the workflow engine. It cancels the running
// workflow context, waits for the current phase to complete (with timeout),
// and cleans up resources. Returns an error if the shutdown timeout is
// exceeded while a workflow is still running.
func (e *Engine) Shutdown(ctx context.Context) error {
	// Cancel the engine's root context, propagating to all goroutines
	// using Engine.WithContext().
	if e.cancel != nil {
		e.cancel()
	}

	// Wait for current phase to complete with timeout.
	if e.running() {
		select {
		case <-e.done:
			// Workflow completed within the timeout.
		case <-ctx.Done():
			return fmt.Errorf("shutdown timeout: workflow still running")
		}
	}

	// Cleanup resources.
	e.cacheMu.Lock()
	e.cache = nil
	e.cacheMu.Unlock()

	return nil
}

// running returns true if the engine has an active workflow that has not
// completed or been shut down.
func (e *Engine) running() bool {
	select {
	case <-e.done:
		return false
	default:
		return true
	}
}

// Complete signals that the workflow has finished. It closes the done channel
// to unblock any waiting Shutdown calls.
func (e *Engine) Complete() {
	select {
	case <-e.done:
		// Already closed — safe to call multiple times.
	default:
		close(e.done)
	}
}

// EngineOptions holds all parameters for creating a new Engine.
type EngineOptions struct {
	SessionID   string
	WorkDir     string
	BackupDir   string
	PlanningDir string
	Provider    provider.LLMProvider
	ModelID     string
	Dispatcher  *tools.Dispatcher
	TokenEst    *tokens.Estimator
	SessionMgr  *session.Manager
	Config      *config.Config
	Collector   *metrics.Collector
}

// NewEngine creates a workflow engine.
func NewEngine(sessionID, workDir, backupDir, planningDir string, p provider.LLMProvider, modelID string,
	dispatcher *tools.Dispatcher, tokenEst *tokens.Estimator, sessionMgr *session.Manager, cfg *config.Config) (*Engine, error) {
	return NewEngineFromOptions(EngineOptions{
		SessionID:   sessionID,
		WorkDir:     workDir,
		BackupDir:   backupDir,
		PlanningDir: planningDir,
		Provider:    p,
		ModelID:     modelID,
		Dispatcher:  dispatcher,
		TokenEst:    tokenEst,
		SessionMgr:  sessionMgr,
		Config:      cfg,
	})
}

// NewEngineFromOptions creates a workflow engine from an EngineOptions struct.
func NewEngineFromOptions(opts EngineOptions) (*Engine, error) {

	promptCfg := config.PromptConfig{}
	projectRoot := opts.WorkDir
	if opts.Config != nil {
		promptCfg = opts.Config.Prompts
	}
	promptBuilder, err := NewPromptBuilder(promptCfg, projectRoot)
	if err != nil {
		return nil, fmt.Errorf("failed to load prompts: %w", err)
	}

	// Create a root context for the engine's lifecycle. This context is
	// cancelled when Shutdown is called, propagating cancellation to all
	// long-running operations that use Engine.WithContext().
	ctx, cancel := context.WithCancel(context.Background())

	// Build recovery path from session directory
	sessDir := filepath.Join(opts.WorkDir, ".m31a")

	e := &Engine{
		sessionID:     opts.SessionID,
		workDir:       opts.WorkDir,
		backupDir:     opts.BackupDir,
		planningDir:   opts.PlanningDir,
		provider:      opts.Provider,
		modelID:       opts.ModelID,
		cfg:           opts.Config,
		dispatcher:    opts.Dispatcher,
		tokens:        opts.TokenEst,
		sessionMgr:    opts.SessionMgr,
		promptBuilder: promptBuilder,
		logger:        slog.Default(),
		startTime:     time.Now(),
		execCommand:   exec.Command,
		costTracker:   NewCostTracker(budgetFromConfig(opts.Config)),
		compactor:     compaction.New(compactionConfig(opts.Config), opts.TokenEst),
		contextRegistry: ctxsrc.NewRegistry(
			ctxsrc.DateTimeSource{},
			ctxsrc.EnvironmentSource{WorkDir: opts.WorkDir},
			ctxsrc.GitSource{WorkDir: opts.WorkDir},
			ctxsrc.InstructionsSource{ProjectRoot: projectRoot, WorkDir: opts.WorkDir},
		),
		collector:    opts.Collector,
		stateMachine: NewStateMachine(),
		cache:        NewWorkflowCache(),
		done:         make(chan struct{}),
		ctx:          ctx,
		cancel:       cancel,
		recoveryPath: recoveryPath(sessDir),
		state: &WorkflowState{
			decisionLog: decision.NewLogger(256),
		},
	}

	// Wire PhaseCoordinator for delegated phase lifecycle management.
	e.phaseCoordinator = NewPhaseCoordinator(
		e.stateMachine,
		e.cache,
		opts.SessionMgr,
		opts.SessionID,
		e.costTracker,
		opts.Collector,
		opts.Dispatcher,
		e.logger,
		e.emit,
	)

	// Create contextBuilder with a callback to Engine's modelForPhase.
	// The callback captures the engine pointer, which is safe because
	// contextBuilder is only used after the engine is fully initialized.
	e.contextBuilder = NewContextBuilder(
		promptBuilder,
		opts.TokenEst,
		opts.Config,
		e.state,
		opts.WorkDir,
		e.contextRegistry,
		func(phase string) string { return e.modelForPhase(m31types.WorkflowPhase(phase)) },
	)

	return e, nil
}

// RunPhase executes the given workflow phase and returns the result.
func (e *Engine) RunPhase(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	// Store current goal for checkpoint persistence (B14)
	e.state.SetCurrentGoal(goal)
	// Budget guardrail: check cumulative cost before each phase.
	// Kept inline because e.costTracker may be reassigned after construction
	// (e.g., in tests), while PhaseCoordinator holds the original reference.
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		cost := e.costTracker.TotalCost()
		if cost >= e.cfg.Features.BudgetLimitUSD {
			return &PhaseResult{
				Phase:   phase,
				Success: false,
				Error:   fmt.Sprintf("budget limit exceeded: $%.4f spent of $%.4f limit", cost, e.cfg.Features.BudgetLimitUSD),
			}, fmt.Errorf("budget limit exceeded: $%.4f of $%.4f", cost, e.cfg.Features.BudgetLimitUSD)
		}
	}

	start := time.Now()

	// Delegate pre-phase setup to PhaseCoordinator
	// Accessor methods handle locking for Messages access
	currentMessages := e.state.MessagesSnapshot()
	var err error
	newMessages, err := e.phaseCoordinator.PrePhaseSetup(
		ctx,
		phase,
		&budgetConfigAdapter{cfg: e.cfg},
		currentMessages,
		e.proactiveCompactCheck,
	)
	e.state.SetMessages(newMessages)
	if err != nil {
		return &PhaseResult{
			Phase:   phase,
			Success: false,
			Error:   err.Error(),
		}, err
	}

	// Reset tool call counter for proactive compaction tracking
	e.toolCallsSinceLastCompact = 0

	from := e.stateMachine.CurrentPhase()

	// Persist recovery state before phase transition for crash safety
	e.persistRecovery()

	if transitionErr := e.stateMachine.Transition(from, phase); transitionErr != nil {
		return nil, fmt.Errorf("phase transition to %s: %w", phase, transitionErr)
	}

	var result *PhaseResult

	switch phase {
	case m31types.PhaseInitialize:
		result, err = e.runInitialize(ctx, goal)
	case m31types.PhaseDiscuss:
		result, err = e.runDiscuss(ctx, goal)
	case m31types.PhasePlan:
		result, err = e.runPlan(ctx, goal)
	case m31types.PhaseExecute:
		result, err = e.runExecute(ctx, goal)
	case m31types.PhaseVerify:
		result, err = e.runVerify(ctx, goal)
	case m31types.PhaseRuntime:
		result, err = e.runRuntime(ctx, goal)
	case m31types.PhaseShip:
		result, err = e.runShip(ctx, goal)
	default:
		return nil, fmt.Errorf("%w: unknown phase %s", m31errors.ErrPhaseTransition, phase)
	}

	if result != nil {
		result.WorkflowMode = e.WorkflowMode()
	}

	// Delegate post-phase metrics to PhaseCoordinator
	e.phaseCoordinator.PostPhaseExecution(phase, result, start)

	// Clear recovery on successful phase completion
	if result != nil && result.Success {
		if clearErr := e.ClearRecovery(); clearErr != nil {
			e.logger.Warn("failed to clear recovery state", "error", clearErr)
		}
	}

	return result, err
}

// HealTask triggers self-healing for a specific task by ID.
// Returns true if healing was attempted, false if the task cannot be healed.
func (e *Engine) HealTask(ctx context.Context, taskID int) (bool, error) {
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		return false, fmt.Errorf("failed to load tasks for heal: %w", err)
	}
	// Add a timeout to prevent indefinite hangs from build/test commands
	verifyCtx, verifyCancel := e.verifyTaskContext(ctx)
	defer verifyCancel()
	for i, task := range tasks {
		if task.ID == taskID && task.Status == m31types.StatusFailed {
			if task.HealsAttempted >= m31types.MaxHealAttempts {
				return false, fmt.Errorf("task %d already at max heal attempts (%d)", taskID, m31types.MaxHealAttempts)
			}
			tasks[i].HealsAttempted++
			e.emit(SelfHealStartMsg{
				TaskID:  task.ID,
				Attempt: tasks[i].HealsAttempted,
				Max:     m31types.MaxHealAttempts,
			})
			failure := fmt.Sprintf(
				"Manual heal triggered by user.\nTask %d failed verification: %v\n"+
					"Task description: %s\nFiles: %v\nAcceptance criteria: %v\n"+
					"Inspect the files listed above, identify any issues, and apply a fix.",
				taskID, e.verifyTask(verifyCtx, task).Errors,
				task.Description, task.Files, task.AcceptanceCriteria,
			)
			healResult := e.healTask(ctx, task, failure, "")
			e.emit(SelfHealCompleteMsg{
				TaskID:  task.ID,
				Attempt: tasks[i].HealsAttempted,
				Max:     m31types.MaxHealAttempts,
				Success: healResult.Success,
				Error:   healResult.Error,
			})
			if healResult.Success {
				newResult := e.verifyTask(verifyCtx, tasks[i])
				if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
					tasks[i].Status = m31types.StatusDone
				} else {
					tasks[i].Status = m31types.StatusFailed
				}
			} else {
				if tasks[i].HealsAttempted >= m31types.MaxHealAttempts {
					tasks[i].Status = m31types.StatusUnrecoverable
				}
			}
			if saveErr := e.sessionMgr.SaveTasks(e.sessionID, tasks); saveErr != nil {
				return true, fmt.Errorf("heal attempted but failed to save tasks: %w", saveErr)
			}
			return true, nil
		}
	}
	return false, fmt.Errorf("task %d not found or not in failed state", taskID)
}


