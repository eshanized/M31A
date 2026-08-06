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
package workflow

import (
	"context"
	"embed"
	"encoding/json"
	"fmt"
	"log/slog"
	"os/exec"
	"path/filepath"
	"strings"
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

// SetIntentResult stores the LLM-classified intent result for downstream enrichment.
func (e *Engine) SetIntentResult(ir *m31types.IntentResult) {
	e.state.SetIntentResult(ir)
	// Log intent classification decision
	if ir != nil {
		e.LogDecision(decision.DecisionReceipt{
			Decision:  fmt.Sprintf("intent:%s (complexity:%s, confidence:%.0f%%)", ir.Intent, ir.Complexity, ir.Confidence*100),
			Rationale: ir.Summary,
			Category:  decision.CategoryIntent,
		})
	}
}

// IntentResult returns the stored intent classification result, or nil if unset.
func (e *Engine) IntentResult() *m31types.IntentResult {
	return e.state.IntentResult()
}

// ScopeIncludes returns true if the intent result's scope contains the given term.
func (e *Engine) ScopeIncludes(term string) bool {
	ir := e.state.IntentResult()
	if ir == nil {
		return false
	}
	for _, s := range ir.Scope {
		if strings.EqualFold(s, term) {
			return true
		}
	}
	return false
}

// LogDecision records a decision in the session log.
func (e *Engine) LogDecision(r decision.DecisionReceipt) {
	dl := e.state.DecisionLog()
	if dl != nil {
		dl.Log(r)
		e.emitDecisionsSnapshot()
	}
}

// emitDecisionsSnapshot emits a snapshot of current decisions to the TUI.
func (e *Engine) emitDecisionsSnapshot() {
	if e.msgEmitter == nil {
		return
	}
	decisions := e.SnapshotDecisions()
	if len(decisions) > 0 {
		e.msgEmitter.Emit(DecisionsSnapshotMsg{Decisions: decisions})
	}
}

// FlushDecisions synchronously returns all logged decisions and resets the buffer.
func (e *Engine) FlushDecisions() []decision.DecisionReceipt {
	dl := e.state.DecisionLog()
	if dl == nil {
		return nil
	}
	return dl.Flush()
}

// SnapshotDecisions returns a copy of buffered decisions without flushing.
func (e *Engine) SnapshotDecisions() []decision.DecisionReceipt {
	dl := e.state.DecisionLog()
	if dl == nil {
		return nil
	}
	return dl.Snapshot()
}

// LastHealReport returns the most recent self-heal report, or nil if none.
func (e *Engine) LastHealReport() *m31types.HealReport {
	return e.state.LastHealReport()
}

// WithContext returns a derived context that is cancelled when the engine
// shuts down. All long-running operations should use this context so
// cancellation propagates through the engine's lifecycle.
func (e *Engine) WithContext(ctx context.Context) context.Context {
	if e.ctx != nil {
		return e.ctx
	}
	return ctx
}

// Context returns the engine's root context. It is cancelled when Shutdown
// is called. Returns context.Background() if the engine has no root context
// (e.g., in tests).
func (e *Engine) Context() context.Context {
	if e.ctx != nil {
		return e.ctx
	}
	return context.Background()
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

// SetCollector attaches a metrics collector to the engine for recording
// tool calls, LLM interactions, phase durations, and heal/bisect events.
func (e *Engine) SetCollector(c *metrics.Collector) {
	e.collector = c
}

// GetCostInfo returns the current cost and budget information.
func (e *Engine) GetCostInfo() (totalCost float64, budgetLimit float64, budgetRemaining float64) {
	totalCost = e.costTracker.TotalCost()
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		budgetLimit = e.cfg.Features.BudgetLimitUSD
		budgetRemaining = budgetLimit - totalCost
		if budgetRemaining < 0 {
			budgetRemaining = 0
		}
	}
	return
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
			slog.Warn("failed to clear recovery state", "error", clearErr)
		}
	}

	return result, err
}

// RunPhaseDirect executes a single phase WITHOUT state machine transition validation.
// INTENDED FOR TEST USE ONLY — does not enforce phase ordering.
// Production code MUST use RunPhase which validates transitions via stateMachine.Transition().
func (e *Engine) RunPhaseDirect(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	e.state.SetCurrentGoal(goal)

	// Budget check (same as RunPhase)
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		cost := e.costTracker.TotalCost()
		if cost >= e.cfg.Features.BudgetLimitUSD {
			return &PhaseResult{Phase: phase, Success: false, Error: "budget limit exceeded"}, fmt.Errorf("budget limit exceeded")
		}
	}

	start := time.Now()

	// PrePhaseSetup (same as RunPhase)
	currentMessages := e.state.MessagesSnapshot()
	var err error
	newMessages, err := e.phaseCoordinator.PrePhaseSetup(ctx, phase, &budgetConfigAdapter{cfg: e.cfg}, currentMessages, e.proactiveCompactCheck)
	e.state.SetMessages(newMessages)
	if err != nil {
		return &PhaseResult{Phase: phase, Success: false, Error: err.Error()}, err
	}

	e.toolCallsSinceLastCompact = 0

	// KEY DIFFERENCE: Skip e.stateMachine.Transition(from, phase) — this is the bypass

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

	e.phaseCoordinator.PostPhaseExecution(phase, result, start)
	return result, err
}

// maxDiscussPlanCycles caps the number of Plan→Discuss→Plan round-trips to
// prevent infinite oscillation between the two phases (BUG-12). One cycle
// (e.g. Plan→Discuss→Plan once) is a normal refinement; beyond that suggests
// a TUI state bug or automated retry loop.
const maxDiscussPlanCycles = 3

// Transition saves a checkpoint and writes STATE.md for the new phase.
// Validates the transition is allowed by the phase ordering guard.
func (e *Engine) Transition(ctx context.Context, from, to m31types.WorkflowPhase) error {
	e.state.transitionMu.Lock()
	defer e.state.transitionMu.Unlock()

	// Delegate transition validation to StateMachine
	if err := e.stateMachine.Transition(from, to); err != nil {
		return fmt.Errorf("phase transition %s -> %s: %w", from, to, err)
	}

	// Delegate transition side effects to PhaseCoordinator
	goal := e.state.CurrentGoal()
	pv := e.state.PlanVersion()
	return e.phaseCoordinator.CoordinateTransition(ctx, from, to, goal, pv)
}

// SetGit sets the git instance on the engine.
// Required before running any phase that uses git operations.
func (e *Engine) SetGit(g *git.Git) {
	e.git = g
	if h, err := g.HeadHash(); err == nil {
		e.sessionStartHash = h
	}
}

// SetLedger sets the shared ledger instance on the engine so the ship phase
// writes session records to the application-configured path instead of
// creating a new ledger at a hardcoded location.
func (e *Engine) SetLedger(l *ledger.Ledger) {
	e.ledger = l
}

// SessionID returns the current session ID.
func (e *Engine) SessionID() string {
	return e.sessionID
}

// SetSessionID updates the engine's session ID.
func (e *Engine) SetSessionID(id string) {
	e.sessionID = id
}

// SetMsgEmitter sets the callback for emitting messages back to the TUI.
func (e *Engine) SetMsgEmitter(em MsgEmitter) {
	e.msgEmitter = em
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

// emit sends a message to the TUI if an emitter is configured.
func (e *Engine) emit(msg any) {
	if e.msgEmitter != nil {
		e.msgEmitter.Emit(msg)
	}
}

// preflightContextCheck estimates token usage before each LLM request
// and attempts automatic truncation if the estimate exceeds 80% of the
// model's context window. Returns ErrContextExceeded only if truncation
// cannot bring usage below 95%. Returns a (possibly truncated) copy of the
// messages so the caller's original slice is never mutated.
func (e *Engine) preflightContextCheck(messages []m31types.Message) ([]m31types.Message, error) {
	if e.tokens == nil {
		return messages, nil
	}
	p, _ := e.providerAndModel()
	if p == nil {
		return messages, nil
	}
	modelInfo, err := p.GetModel(e.modelForPhase(e.stateMachine.CurrentPhase()))
	if err != nil || modelInfo == nil {
		return messages, nil
	}
	estimated := e.tokens.EstimateMessages(messages)
	contextLength := modelInfo.ContextLength
	if contextLength <= 0 {
		contextLength = m31types.DefaultContextLength
	}

	threshold80 := int(float64(contextLength) * 0.80)
	if e.cfg != nil && e.cfg.Features.ContextTruncationThreshold > 0 {
		threshold80 = int(float64(contextLength) * e.cfg.Features.ContextTruncationThreshold)
	}
	threshold95 := int(float64(contextLength) * 0.95)

	if estimated <= threshold80 {
		return messages, nil
	}

	var msgs []m31types.Message

	// Try auto-compaction before falling back to crude truncation
	if e.compactor != nil && e.compactor.ShouldCompact(messages, contextLength) {
		slog.Info("auto-compaction triggered", "estimated_tokens", estimated, "context_length", contextLength)
		compactCtx, compactCancel := context.WithTimeout(context.Background(), 60*time.Second)
		cp, _ := e.providerAndModel()
		result, compactErr := e.compactor.Compact(compactCtx, messages, cp, e.modelForPhase(e.stateMachine.CurrentPhase()))
		compactCancel()
		if compactErr == nil && result.Compacted {
			e.emit(CompactionCompleteMsg{
				TokensBefore:    result.TokensBefore,
				TokensAfter:     result.TokensAfter,
				MessagesRemoved: result.MessagesRemoved,
			})
			// Re-estimate on the compacted messages
			compactedMsgs := e.compactedMessages(messages, result.Summary)
			compactedEstimate := e.tokens.EstimateMessages(compactedMsgs)
			if compactedEstimate <= threshold95 {
				return compactedMsgs, nil
			}
			// Compaction wasn't sufficient, fall through to truncation with compacted messages
			msgs = compactedMsgs
		} else if compactErr != nil {
			slog.Warn("auto-compaction failed, falling back to truncation", "error", compactErr)
		}
	}

	// Work on a shallow copy to avoid mutating the caller's slice.
	if msgs == nil {
		msgs = make([]m31types.Message, len(messages))
		copy(msgs, messages)
	}

	// Cache per-message token counts to avoid O(N*K) recomputation in truncation loops.
	// Each message's token count is estimated once, then updated incrementally after truncation.
	// B17: Include per-message overhead (4 tokens) to match EstimateMessages used in preflight.
	const perMessageOverhead = 4
	msgTokens := make([]int, len(msgs))
	for i, msg := range msgs {
		msgTokens[i] = e.tokens.Estimate(msg.Content) + perMessageOverhead
		for _, tc := range msg.ToolCalls {
			if len(tc.Input) > 0 {
				msgTokens[i] += e.tokens.Estimate(string(tc.Input))
			}
			msgTokens[i] += e.tokens.Estimate(tc.Name)
		}
	}
	estimateTotal := func() int {
		total := 0
		for _, t := range msgTokens {
			total += t
		}
		return total
	}
	estimated = estimateTotal()

	// Try progressive truncation before giving up
	// Pass 1: truncate old tool results
	if estimated > threshold80 {
		for i := 0; i < len(msgs) && estimated > threshold80; i++ {
			if msgs[i].Role == "tool" && len(msgs[i].Content) > 500 {
				msgs[i].Content = msgs[i].Content[:500] + "\n...[truncated for context]"
				msgTokens[i] = e.tokens.Estimate(msgs[i].Content)
				estimated = estimateTotal()
			}
		}
	}

	// Pass 2: truncate old assistant messages
	if estimated > threshold80 {
		for i := 0; i < len(msgs) && estimated > threshold80; i++ {
			if msgs[i].Role == "assistant" && len(msgs[i].ToolCalls) == 0 && len(msgs[i].Content) > 1000 {
				msgs[i].Content = msgs[i].Content[:1000] + "\n...[truncated for context]"
				msgTokens[i] = e.tokens.Estimate(msgs[i].Content)
				estimated = estimateTotal()
			}
		}
	}

	// Pass 3: remove oldest non-system, non-recent messages
	if estimated > threshold80 {
		keepRecent := 6
		if len(msgs) > keepRecent+1 {
			idx := 1 // start after the first (system) message
			for idx < len(msgs)-keepRecent && estimated > threshold80 {
				if msgs[idx].Role == "system" {
					idx++
					continue
				}
				msgs = append(msgs[:idx], msgs[idx+1:]...)
				msgTokens = append(msgTokens[:idx], msgTokens[idx:]...)
				estimated = estimateTotal()
				// Don't increment idx — next message slides into same position
			}
		}
	}

	if estimated > threshold95 {
		return msgs, fmt.Errorf("%w: estimated %d tokens exceeds 95%% of %d context (auto-truncation insufficient)", m31errors.ErrContextExceeded, estimated, contextLength)
	}

	if estimated > threshold80 {
		slog.Warn("context usage approaching limit after truncation", "estimated", estimated, "limit", contextLength)
	}
	return msgs, nil
}

// proactiveCompactCheck performs compaction if context usage exceeds the
// configured threshold. Called before phase transitions and periodically
// during Execute phase. This is a no-op if compactor is nil, proactive
// compaction is disabled, or no messages are available.
func (e *Engine) proactiveCompactCheck(messages []m31types.Message) []m31types.Message {
	if e.compactor == nil || e.tokens == nil {
		return messages
	}
	p, _ := e.providerAndModel()
	if p == nil {
		return messages
	}
	if e.cfg == nil || !e.cfg.Compaction.Proactive {
		return messages
	}

	modelInfo, err := p.GetModel(e.modelForPhase(e.stateMachine.CurrentPhase()))
	if err != nil || modelInfo == nil {
		return messages
	}
	contextLength := modelInfo.ContextLength
	if contextLength <= 0 {
		contextLength = m31types.DefaultContextLength
	}

	estimated := e.tokens.EstimateMessages(messages)
	threshold := int(float64(contextLength) * float64(e.cfg.Compaction.PhaseTransitionPct) / 100.0)
	if threshold <= 0 {
		threshold = int(float64(contextLength) * 0.60)
	}

	if estimated <= threshold {
		return messages
	}

	slog.Info("proactive compaction triggered",
		"phase", e.stateMachine.CurrentPhase(),
		"estimated_tokens", estimated,
		"context_length", contextLength,
		"threshold_pct", e.cfg.Compaction.PhaseTransitionPct)

	compactCtx, compactCancel := context.WithTimeout(context.Background(), 60*time.Second)
	cp2, _ := e.providerAndModel()
	result, compactErr := e.compactor.Compact(compactCtx, messages, cp2, e.modelForPhase(e.stateMachine.CurrentPhase()))
	compactCancel()

	if compactErr != nil {
		slog.Warn("proactive compaction failed", "error", compactErr)
		return messages
	}
	if !result.Compacted {
		return messages
	}

	e.emit(CompactionCompleteMsg{
		TokensBefore:    result.TokensBefore,
		TokensAfter:     result.TokensAfter,
		MessagesRemoved: result.MessagesRemoved,
	})

	compacted := e.compactedMessages(messages, result.Summary)
	slog.Info("proactive compaction complete",
		"tokens_before", result.TokensBefore,
		"tokens_after", result.TokensAfter,
		"messages_removed", result.MessagesRemoved)
	return compacted
}

// SubmitDiscussAnswer records an answer for a discuss question.
func (e *Engine) SubmitDiscussAnswer(index int, answer string) error {
	if e.discussState.Questions == nil {
		return fmt.Errorf("%w: no discuss questions", m31errors.ErrPhaseTransition)
	}
	if index < 0 || index >= len(e.discussState.Questions) {
		return fmt.Errorf("%w: invalid question index %d", m31errors.ErrPhaseTransition, index)
	}
	if e.discussState.Answers == nil {
		e.discussState.Answers = make(map[int]string)
	}
	e.discussState.Answers[index] = answer
	return nil
}

// DiscussState returns a copy of the current discuss state. The TUI
// uses this to read the parsed questions after the discuss phase
// returns. The returned struct is a value copy so mutations by the
// TUI do not affect the engine's internal state.
func (e *Engine) DiscussState() DiscussState {
	return e.discussState
}

// SkipDiscuss fills default (empty) answers and saves.
func (e *Engine) SkipDiscuss() error {
	if e.discussState.Questions == nil {
		return nil
	}
	if e.discussState.Answers == nil {
		e.discussState.Answers = make(map[int]string)
	}
	for i := range e.discussState.Questions {
		if _, ok := e.discussState.Answers[i]; !ok {
			e.discussState.Answers[i] = ""
		}
	}
	return e.FinalizeDiscuss()
}

func (e *Engine) FinalizeDiscuss() error {
	project := e.loadProjectCached()
	var questions, answers []string
	for i, q := range e.discussState.Questions {
		questions = append(questions, q)
		a := ""
		if ans, ok := e.discussState.Answers[i]; ok {
			a = ans
		}
		answers = append(answers, a)
	}
	if err := e.saveDiscussAnswers(project, questions, answers); err != nil {
		return fmt.Errorf("save discuss answers: %w", err)
	}
	return nil
}

// SetRefinementFeedback stores user feedback for the next plan regeneration.
// The plan phase reads this field to inject feedback into the LLM context.
// Duplicate feedback is ignored — only new feedback bumps the plan version.
func (e *Engine) SetRefinementFeedback(feedback string) {
	if feedback != "" {
		currentFB := e.state.RefineFeedback()
		if feedback != currentFB {
			e.state.SetRefineFeedback(feedback)
			pv := e.state.IncrementPlanVersion()
			// Log plan revision decision
			e.LogDecision(decision.DecisionReceipt{
				Decision:  fmt.Sprintf("plan revision requested (v%d)", pv),
				Rationale: truncateForLog(feedback, 200),
				Category:  decision.CategoryPlan,
				Cost: decision.Cost{
					Attempts: pv,
				},
			})
		}
	} else {
		e.state.SetRefineFeedback(feedback)
	}
}

// PlanContent returns the current plan markdown content.
func (e *Engine) PlanContent() string {
	return e.state.PlanContent()
}

// PlanVersion returns the current plan version number.
// Version 1 is the initial plan; each refinement increments it.
func (e *Engine) PlanVersion() int {
	return e.state.PlanVersion()
}

// buildToolDefinitions returns the tool definitions for the LLM.
// Results are cached after the first call since tool definitions
// don't change during a session (PERF-23). Each definition's
// ParametersParsed field is populated once to avoid repeated
// json.Unmarshal in BuildChatBody (PERF-25).
func (e *Engine) buildToolDefinitions() []provider.ToolDefinition {
	e.cacheMu.RLock()
	cache := e.cache
	e.cacheMu.RUnlock()
	return cache.GetToolDefs(func() []provider.ToolDefinition {
		var defs []provider.ToolDefinition
		for _, name := range e.dispatcher.List() {
			tool, ok := e.dispatcher.GetTool(name)
			if !ok {
				continue
			}
			def := provider.ToolDefinition{
				Name:        tool.Name(),
				Description: tool.Description(),
				Parameters:  "{}",
			}
			if sp, ok := tool.(m31types.SchemaProvider); ok {
				def.Parameters = sp.ParameterSchema()
			}
			// Pre-parse JSON to avoid repeated Unmarshal in BuildChatBody
			if def.Parameters != "" {
				var parsed any
				if err := json.Unmarshal([]byte(def.Parameters), &parsed); err == nil {
					def.ParametersParsed = parsed
				}
			}
			defs = append(defs, def)
		}
		return defs
	})
}

// buildSystemPrompt composes the system prompt from base + optional extras.
// Delegates to ContextBuilder for prompt composition logic.
func (e *Engine) buildSystemPrompt(extra ...string) string {
	return e.contextBuilder.BuildSystemPrompt(string(e.stateMachine.CurrentPhase()), extra...)
}

// getCodeIntel lazily builds the codebase intelligence indexer.
// Returns nil if building fails or the workDir is empty.
// The caller's context is used for the build, so cancellation propagates.
func (e *Engine) getCodeIntel(ctx context.Context) *codeintel.Indexer {
	e.codeIntelMu.Lock()
	defer e.codeIntelMu.Unlock()
	if e.codeIntelBuilt {
		return e.codeIntel
	}
	e.codeIntelBuilt = true
	idx := codeintel.NewIndexer(e.workDir)
	buildCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	if err := idx.Build(buildCtx); err != nil {
		e.logger.Warn("codeintel build failed", "error", err)
		return nil
	}
	e.codeIntel = idx
	return e.codeIntel
}
