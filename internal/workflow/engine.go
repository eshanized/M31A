package workflow

import (
	"context"
	"embed"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/codeintel"
	"github.com/eshanized/M31A/internal/compaction"
	"github.com/eshanized/M31A/internal/config"
	ctxsrc "github.com/eshanized/M31A/internal/context"
	"github.com/eshanized/M31A/internal/decision"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/ledger"
	"github.com/eshanized/M31A/internal/metrics"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/infrastructure/retry"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
)

//go:embed templates/website-nextjs/*
var websiteTemplateFS embed.FS

// WorkflowState groups mutable session state extracted from Engine.
// This struct owns plan state, cached data, intent classification,
// and v1.5 subsystems (decision log, knowledge, budget tracker).
// The Engine retains phase dispatch, LLM streaming, and subsystem orchestration.
type WorkflowState struct {
	// transitionMu serializes phase transitions to prevent interleaved checkpoint saves.
	transitionMu sync.Mutex

	// planMu guards planMarkdown and planVersion to prevent races between
	// workflow goroutine writes and TUI reads via PlanContent()/PlanVersion().
	planMu sync.RWMutex

	// messagesMu guards Messages to prevent races between workflow goroutine
	// writes (via PrePhaseSetup) and any concurrent reads.
	messagesMu sync.RWMutex

	// Plan state
	planMarkdown   string // current plan content for refinement context
	planVersion    int    // current plan version (increments on refine)
	refineFeedback string // pending refinement feedback from user
	researchOutput string // pre-plan research results for injection into plan context
	// Cached base prompt (built once, used by ContextBuilder)
	cachedBasePrompt     string
	cachedBasePromptOnce sync.Once

	// Cached full system prompts per extras signature (used by ContextBuilder)
	cachedFullPrompts   map[string]string
	cachedFullPromptsMu sync.Mutex

	// Conversation messages for the workflow (updated by /compress proactively)
	Messages []m31types.Message

	// Intent classification result from the LLM-based classifier.
	// Set before the workflow starts; used to enrich discuss/research/plan context.
	intentResult *m31types.IntentResult

	// Dynamic context change detection (used by ContextBuilder)
	contextSnapshot      map[string]string
	cachedDynamicContext string

	// v1.5: Decision logging
	decisionLog *decision.Logger

	// v1.5: Self-heal explanation
	lastHealReport *m31types.HealReport

	// v1.5: Checkpoint resume
	checkpointData *CheckpointData
}

// CheckpointData holds data that can be saved/restored across checkpoints.
type CheckpointData struct {
	Phase       m31types.WorkflowPhase     `json:"phase"`
	Goal        string                     `json:"goal"`
	PlanVersion int                        `json:"plan_version"`
	Decisions   []decision.DecisionReceipt `json:"decisions,omitempty"`
	Timestamp   time.Time                  `json:"timestamp"`
}

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
	// state groups mutable session state (plan, cache, intent, v1.5 subsystems).
	state *WorkflowState
	// done is closed when the workflow completes or is shut down.
	done chan struct{}
	// cancel cancels the running workflow context on shutdown.
	cancel context.CancelFunc

	// pause/resume support for execute phase
	pauseMu       sync.Mutex
	pauseCh       chan struct{} // closed when paused; nil when running
	resumeCh      chan struct{} // closed when resumed; nil when running
	skipTaskCh    chan int      // send task ID to skip (while paused)
	cancelTaskCh  chan int      // send task ID to cancel (while paused)
	cancelGroupCh chan struct{} // closed to cancel entire group (while paused)
}

// gitConfig returns the git config with safe defaults when cfg is nil.
func (e *Engine) gitConfig() config.GitConfig {
	if e.cfg != nil {
		return e.cfg.Git
	}
	return config.DefaultGitConfig()
}

// PauseExecution pauses the execute phase. While paused, task groups wait before executing.
// Returns true if the engine was paused, false if already paused or not in execute phase.
func (e *Engine) PauseExecution() bool {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.pauseCh != nil {
		return false // already paused
	}
	e.pauseCh = make(chan struct{})
	e.resumeCh = make(chan struct{})
	e.skipTaskCh = make(chan int, 1)
	e.cancelTaskCh = make(chan int, 1)
	e.cancelGroupCh = make(chan struct{})
	return true
}

// ResumeExecution resumes the execute phase after a pause.
// Returns true if the engine was resumed, false if not paused.
func (e *Engine) ResumeExecution() bool {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.pauseCh == nil {
		return false // not paused
	}
	close(e.resumeCh)
	e.pauseCh = nil
	e.resumeCh = nil
	e.skipTaskCh = nil
	e.cancelTaskCh = nil
	e.cancelGroupCh = nil
	return true
}

// IsPaused returns whether the execute phase is currently paused.
func (e *Engine) IsPaused() bool {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	return e.pauseCh != nil
}

// SkipCurrentTask sends a skip command for a specific task (must be called while paused).
func (e *Engine) SkipCurrentTask(taskID int) {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.skipTaskCh != nil {
		select {
		case e.skipTaskCh <- taskID:
		default:
		}
	}
}

// CancelCurrentTask sends a cancel command for a specific task (must be called while paused).
func (e *Engine) CancelCurrentTask(taskID int) {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.cancelTaskCh != nil {
		select {
		case e.cancelTaskCh <- taskID:
		default:
		}
	}
}

// CancelGroup cancels the entire current task group (must be called while paused).
func (e *Engine) CancelGroup() {
	e.pauseMu.Lock()
	defer e.pauseMu.Unlock()
	if e.cancelGroupCh != nil {
		close(e.cancelGroupCh)
		e.cancelGroupCh = nil
	}
}

// waitForPause blocks until the engine is resumed or context is cancelled.
// Returns true if resumed, false if context was cancelled.
func (e *Engine) waitForPause(ctx context.Context) bool {
	e.pauseMu.Lock()
	if e.pauseCh == nil {
		e.pauseMu.Unlock()
		return true // not paused
	}
	resumeCh := e.resumeCh
	e.pauseMu.Unlock()

	select {
	case <-resumeCh:
		return true
	case <-ctx.Done():
		return false
	}
}

// consumeSkipOrCancel reads skip/cancel commands while paused. Returns (skipID, cancelID, cancelledGroup, ok).
func (e *Engine) consumeSkipOrCancel(ctx context.Context) (skipID int, cancelID int, cancelledGroup bool, ok bool) {
	e.pauseMu.Lock()
	skipCh := e.skipTaskCh
	cancelCh := e.cancelTaskCh
	groupCh := e.cancelGroupCh
	resumeCh := e.resumeCh
	e.pauseMu.Unlock()

	select {
	case <-resumeCh:
		return 0, 0, false, true
	case id := <-skipCh:
		return id, 0, false, true
	case id := <-cancelCh:
		return 0, id, false, true
	case <-groupCh:
		return 0, 0, true, true
	case <-ctx.Done():
		return 0, 0, false, false
	}
}

// promptOrGet returns the named prompt or empty string if not found.
// Errors are logged at warn level. Used by callers where prompt names are
// compile-time constants and a missing prompt is a programming error, not
// a user-facing failure.
func (e *Engine) promptOrGet(name string) string {
	s, err := e.promptBuilder.Prompt(name)
	if err != nil {
		e.logger.Warn("missing prompt template", "name", name, "error", err)
		return ""
	}
	return s
}

// modelForPhase returns the per-phase model ID, checked in priority order:
//  1. perPhaseModels (set interactively by the TUI at workflow start)
//  2. AgentsConfig from config.toml
//  3. cfg.Model.Default
//  4. the engine's active modelID
func (e *Engine) modelForPhase(phase m31types.WorkflowPhase) string {
	// 1. Interactive per-phase override (highest priority)
	e.perPhaseModelsMu.RLock()
	if id, ok := e.perPhaseModels[phase]; ok && id != "" {
		e.perPhaseModelsMu.RUnlock()
		return id
	}
	e.perPhaseModelsMu.RUnlock()
	if e.cfg == nil {
		return e.modelID
	}
	// 2. AgentsConfig from config.toml
	var override string
	switch phase {
	case m31types.PhaseInitialize:
		override = e.cfg.Agents.Initialize
	case m31types.PhasePlan:
		override = e.cfg.Agents.Plan
	case m31types.PhaseExecute:
		override = e.cfg.Agents.Execute
	case m31types.PhaseVerify:
		override = e.cfg.Agents.Verify
	case m31types.PhaseRuntime:
		override = e.cfg.Agents.Runtime
	case m31types.PhaseShip:
		override = e.cfg.Agents.Ship
	case m31types.PhaseDiscuss:
		override = e.cfg.Agents.Discuss
	}
	if override != "" {
		return override
	}
	// 3. Global agent default
	if e.cfg.Agents.Default != "" {
		return e.cfg.Agents.Default
	}
	// 4. Engine model ID
	return e.modelID
}

// SetPhaseModel assigns a model ID to a specific workflow phase.
// This takes the highest priority over AgentsConfig and cfg.Model.Default.
// Called by the TUI after the user selects Planning/Coding models in the picker.
func (e *Engine) SetPhaseModel(phase m31types.WorkflowPhase, modelID string) {
	e.perPhaseModelsMu.Lock()
	defer e.perPhaseModelsMu.Unlock()
	if e.perPhaseModels == nil {
		e.perPhaseModels = make(map[m31types.WorkflowPhase]string)
	}
	if modelID != "" {
		e.perPhaseModels[phase] = modelID
	}
}

// SetWorkflowMode sets the mode that controls phase-skipping behaviour.
func (e *Engine) SetWorkflowMode(mode m31types.WorkflowMode) {
	e.workflowModeMu.Lock()
	e.workflowMode = mode
	e.workflowModeMu.Unlock()
}

// WorkflowMode returns the current workflow mode.
func (e *Engine) WorkflowMode() m31types.WorkflowMode {
	e.workflowModeMu.RLock()
	defer e.workflowModeMu.RUnlock()
	return e.workflowMode
}

// SetIntentResult stores the LLM-classified intent result for downstream enrichment.
func (e *Engine) SetIntentResult(ir *m31types.IntentResult) {
	e.state.intentResult = ir
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
	return e.state.intentResult
}

// ScopeIncludes returns true if the intent result's scope contains the given term.
func (e *Engine) ScopeIncludes(term string) bool {
	if e.state.intentResult == nil {
		return false
	}
	for _, s := range e.state.intentResult.Scope {
		if strings.EqualFold(s, term) {
			return true
		}
	}
	return false
}

// LogDecision records a decision in the session log.
func (e *Engine) LogDecision(r decision.DecisionReceipt) {
	if e.state.decisionLog != nil {
		e.state.decisionLog.Log(r)
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
	if e.state.decisionLog == nil {
		return nil
	}
	return e.state.decisionLog.Flush()
}

// SnapshotDecisions returns a copy of buffered decisions without flushing.
func (e *Engine) SnapshotDecisions() []decision.DecisionReceipt {
	if e.state.decisionLog == nil {
		return nil
	}
	return e.state.decisionLog.Snapshot()
}

// LastHealReport returns the most recent self-heal report, or nil if none.
func (e *Engine) LastHealReport() *m31types.HealReport {
	return e.state.lastHealReport
}

// SaveCheckpointData saves current workflow state for checkpoint resume.
// It persists to both in-memory state and disk via the session manager.
func (e *Engine) SaveCheckpointData(goal string) {
	decisions := e.SnapshotDecisions()
	cp := &CheckpointData{
		Phase:       e.stateMachine.CurrentPhase(),
		Goal:        goal,
		PlanVersion: e.state.planVersion,
		Decisions:   decisions,
		Timestamp:   time.Now(),
	}
	e.state.checkpointData = cp

	// Persist to disk so checkpoint data survives process crashes.
	sessCheckpoint := session.Checkpoint{
		Phase:       cp.Phase,
		Timestamp:   cp.Timestamp,
		Goal:        cp.Goal,
		PlanVersion: cp.PlanVersion,
	}
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, sessCheckpoint); err != nil {
		e.logger.Warn("failed to persist checkpoint to disk", "error", err)
	}
}

// LoadCheckpointData restores workflow state from a checkpoint.
// If data is nil, it attempts to load from disk via the session manager.
func (e *Engine) LoadCheckpointData(data *CheckpointData) {
	if data == nil {
		// Attempt to load from disk if no in-memory checkpoint exists.
		checkpoints, err := e.sessionMgr.LoadCheckpoints(e.sessionID)
		if err != nil {
			e.logger.Warn("failed to load checkpoints from disk", "error", err)
			return
		}
		if len(checkpoints) == 0 {
			return
		}
		// Use the most recent checkpoint (first element, sorted newest-first).
		cp := checkpoints[0]
		data = &CheckpointData{
			Phase:       cp.Phase,
			Goal:        cp.Goal,
			PlanVersion: cp.PlanVersion,
			Timestamp:   cp.Timestamp,
		}
	}
	e.state.checkpointData = data
	e.stateMachine.SetPhase(data.Phase)
	e.state.planMu.Lock()
	e.state.planVersion = data.PlanVersion
	e.state.planMu.Unlock()
	// Restore decisions to the log
	if data.Decisions != nil && e.state.decisionLog != nil {
		for _, d := range data.Decisions {
			e.state.decisionLog.Log(d)
		}
	}
}

// Close flushes and shuts down the decision logger. Safe to call multiple times.
func (e *Engine) Close() {
	if e.state != nil && e.state.decisionLog != nil {
		e.state.decisionLog.Close()
	}
}

// Shutdown gracefully stops the workflow engine. It cancels the running
// workflow context, waits for the current phase to complete (with timeout),
// and cleans up resources. Returns an error if the shutdown timeout is
// exceeded while a workflow is still running.
func (e *Engine) Shutdown(ctx context.Context) error {
	// Cancel current workflow if running.
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
	e.cache = nil

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

// GetCheckpointData returns the current checkpoint data, or nil if none.
func (e *Engine) GetCheckpointData() *CheckpointData {
	return e.state.checkpointData
}

// ExtractWebsiteTemplateTo extracts the bundled website template to a temporary
// directory and stores the path for later injection into the plan/execute context.
// Returns the path to the extracted template, or an error if extraction fails.
func (e *Engine) ExtractWebsiteTemplateTo() (string, error) {
	if e.websiteTemplateDir != "" {
		return e.websiteTemplateDir, nil
	}
	tmpDir, err := os.MkdirTemp("", "m31a-website-template-*")
	if err != nil {
		return "", fmt.Errorf("create temp dir: %w", err)
	}
	if err := ExtractWebsiteTemplate(tmpDir); err != nil {
		_ = os.RemoveAll(tmpDir)
		return "", fmt.Errorf("extract template: %w", err)
	}
	e.websiteTemplateDir = tmpDir
	return tmpDir, nil
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
		),
		collector:    opts.Collector,
		stateMachine: NewStateMachine(),
		cache:        NewWorkflowCache(),
		done:         make(chan struct{}),
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

// budgetFromConfig extracts the budget limit from config, returning 0 if nil.
func budgetFromConfig(cfg *config.Config) float64 {
	if cfg == nil {
		return 0
	}
	return cfg.Features.BudgetLimitUSD
}

// compactionConfig converts a config.CompactionConfig to a compaction.Config.
func compactionConfig(cfg *config.Config) compaction.Config {
	if cfg == nil {
		return compaction.DefaultConfig()
	}
	c := cfg.Compaction
	if c.Buffer <= 0 {
		c.Buffer = 20000
	}
	if c.KeepTokens <= 0 {
		c.KeepTokens = 8000
	}
	return compaction.Config{
		Auto:                c.Auto,
		Buffer:              c.Buffer,
		KeepTokens:          c.KeepTokens,
		SummaryTemplate:     c.SummaryTemplate,
		SummaryTemplateFile: c.SummaryTemplateFile,
	}
}

// budgetConfigAdapter adapts *config.Config to the budget limit interface
// expected by PhaseCoordinator.PrePhaseSetup.
type budgetConfigAdapter struct {
	cfg *config.Config
}

func (a *budgetConfigAdapter) GetBudgetLimit() float64 {
	if a.cfg == nil {
		return 0
	}
	return a.cfg.Features.BudgetLimitUSD
}

// compactedMessages builds a new message list with the compaction summary
// prepended and old messages replaced. Keeps the last N messages based on
// the compactor's KeepTokens setting.
func (e *Engine) compactedMessages(original []m31types.Message, summary string) []m31types.Message {
	if e.compactor == nil || e.tokens == nil {
		return original
	}
	keepTokens := 8000
	if e.cfg != nil && e.cfg.Compaction.KeepTokens > 0 {
		keepTokens = e.cfg.Compaction.KeepTokens
	}
	_, recent := compaction.SplitMessages(original, keepTokens, e.tokens.Estimate)

	summaryMsg := m31types.Message{
		Role:    "system",
		Content: "[Compacted Session History]\n" + summary,
		Segments: []m31types.MessageSegment{
			{
				Type:    m31types.MessageCompaction,
				Content: summary,
				Visible: false,
			},
		},
		CreatedAt: time.Now(),
	}

	result := make([]m31types.Message, 0, len(recent)+1)
	result = append(result, summaryMsg)
	result = append(result, recent...)
	return result
}

// loadProjectCached returns the cached project state, loading it from disk
// on first access per session. Avoids redundant disk I/O + JSON parse across
// buildDiscussContext, buildPlanContext, buildResearchContext, and buildExecuteContext.
func (e *Engine) loadProjectCached() *m31types.ProjectState {
	if cached := e.cache.GetProjectShared(e.sessionID); cached != nil {
		return cached
	}
	project, err := e.sessionMgr.LoadProject(e.sessionID)
	if err != nil {
		e.logger.Warn("failed to load project", "error", err)
		return nil
	}
	e.cache.SetProjectShared(e.sessionID, project)
	return project
}

// SetModel updates the active model ID and provider for the engine.
func (e *Engine) SetModel(modelID string, p provider.LLMProvider) {
	e.modelIDMu.Lock()
	defer e.modelIDMu.Unlock()
	e.modelID = modelID
	if p != nil {
		e.provider = p
	}
}

// providerAndModel returns the current provider and modelID atomically.
func (e *Engine) providerAndModel() (provider.LLMProvider, string) {
	e.modelIDMu.RLock()
	defer e.modelIDMu.RUnlock()
	return e.provider, e.modelID
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
	// messagesMu protects Messages from concurrent access during PrePhaseSetup
	e.state.messagesMu.Lock()
	var err error
	e.state.Messages, err = e.phaseCoordinator.PrePhaseSetup(
		ctx,
		phase,
		&budgetConfigAdapter{cfg: e.cfg},
		e.state.Messages,
		e.proactiveCompactCheck,
	)
	e.state.messagesMu.Unlock()
	if err != nil {
		return &PhaseResult{
			Phase:   phase,
			Success: false,
			Error:   err.Error(),
		}, err
	}

	// Reset tool call counter for proactive compaction tracking
	e.toolCallsSinceLastCompact = 0

	e.stateMachine.SetPhase(phase)

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
	return e.phaseCoordinator.CoordinateTransition(ctx, from, to)
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
	msgTokens := make([]int, len(msgs))
	for i, msg := range msgs {
		msgTokens[i] = e.tokens.Estimate(msg.Content)
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
	if feedback != "" && feedback != e.state.refineFeedback {
		e.state.refineFeedback = feedback
		e.state.planMu.Lock()
		e.state.planVersion++
		e.state.planMu.Unlock()
		// Log plan revision decision
		e.LogDecision(decision.DecisionReceipt{
			Decision:  fmt.Sprintf("plan revision requested (v%d)", e.state.planVersion),
			Rationale: truncateForLog(feedback, 200),
			Category:  decision.CategoryPlan,
			Cost: decision.Cost{
				Attempts: e.state.planVersion,
			},
		})
	} else if feedback == "" {
		e.state.refineFeedback = feedback
	}
}

// PlanContent returns the current plan markdown content.
func (e *Engine) PlanContent() string {
	e.state.planMu.RLock()
	defer e.state.planMu.RUnlock()
	return e.state.planMarkdown
}

// PlanVersion returns the current plan version number.
// Version 1 is the initial plan; each refinement increments it.
func (e *Engine) PlanVersion() int {
	e.state.planMu.RLock()
	defer e.state.planMu.RUnlock()
	return e.state.planVersion
}

// buildToolDefinitions returns the tool definitions for the LLM.
// Results are cached after the first call since tool definitions
// don't change during a session (PERF-23). Each definition's
// ParametersParsed field is populated once to avoid repeated
// json.Unmarshal in BuildChatBody (PERF-25).
func (e *Engine) buildToolDefinitions() []provider.ToolDefinition {
	return e.cache.GetToolDefs(func() []provider.ToolDefinition {
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

// renderDynamicContext formats a context snapshot map into a prompt section.
// Delegates to ContextBuilder.
func (e *Engine) renderDynamicContext(snapshot map[string]string) string {
	return e.contextBuilder.renderDynamicContext(snapshot)
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

// consumeStream reads all chunks from the iterator and returns the concatenated content.
// Enforces MaxLLMResponseBytes limit to prevent OOM from pathological responses.
// Returns partial content before non-EOF errors so callers can inspect what was received.
// Also captures usage data from the final chunk for token calibration.
func (e *Engine) consumeStream(iterator *m31types.StreamIterator) (string, *m31types.Usage, error) {
	var sb strings.Builder
	defer iterator.Close() //nolint:errcheck

	var lastUsage *m31types.Usage
	for {
		chunk, err := iterator.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			// Preserve partial content before non-EOF errors for caller inspection
			if chunk != nil && chunk.Delta != "" {
				sb.WriteString(chunk.Delta)
			}
			return sb.String(), lastUsage, err
		}
		if chunk != nil {
			if chunk.Delta != "" {
				sb.WriteString(chunk.Delta)
				// Enforce max response size incrementally as chunks arrive
				if sb.Len() > m31types.MaxLLMResponseBytes {
					return sb.String(), lastUsage, fmt.Errorf("LLM response exceeds maximum size of %d bytes: %w",
						m31types.MaxLLMResponseBytes, m31errors.ErrContextExceeded)
				}
			}
			if chunk.Usage != nil {
				lastUsage = chunk.Usage
			}
		}
	}
	return sb.String(), lastUsage, nil
}

// toolCallBuilder accumulates streamed tool_call chunks for a single tool invocation.
type toolCallBuilder struct {
	id        string
	name      string
	arguments strings.Builder
}

// consumeStreamWithTools reads all chunks from the iterator, collecting both
// text content and native tool_call chunks. Returns the concatenated content,
// any structured tool calls, captured usage data, and an error.
//
// Native tool_call chunks arrive with Type="tool_call" and incremental argument
// deltas in ToolInput. They are accumulated by Index and finalized into ToolCall
// structs with parsed JSON arguments.
func (e *Engine) consumeStreamWithTools(iterator *m31types.StreamIterator) (string, []m31types.ToolCall, *m31types.Usage, error) {
	var content strings.Builder
	builders := map[int]*toolCallBuilder{}
	defer iterator.Close() //nolint:errcheck

	var lastUsage *m31types.Usage
	for {
		chunk, err := iterator.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			if chunk != nil && chunk.Delta != "" {
				content.WriteString(chunk.Delta)
			}
			// Discard partial tool calls from truncated streams to prevent
			// dispatching incomplete/malformed tool calls.
			return content.String(), nil, lastUsage, err
		}
		if chunk == nil {
			continue
		}

		if chunk.Usage != nil {
			lastUsage = chunk.Usage
		}

		switch chunk.Type {
		case "tool_call":
			b, ok := builders[chunk.Index]
			if !ok {
				b = &toolCallBuilder{
					id:   chunk.ToolCallID,
					name: chunk.ToolName,
				}
				builders[chunk.Index] = b
			}
			if chunk.ToolInput != "" {
				b.arguments.WriteString(chunk.ToolInput)
			}
			if chunk.ToolCallID != "" && b.id == "" {
				b.id = chunk.ToolCallID
			}
			if chunk.ToolName != "" && b.name == "" {
				b.name = chunk.ToolName
			}

		default:
			if chunk.Delta != "" {
				content.WriteString(chunk.Delta)
				if content.Len() > m31types.MaxLLMResponseBytes {
					return content.String(), nil, lastUsage, fmt.Errorf("LLM response exceeds maximum size of %d bytes: %w",
						m31types.MaxLLMResponseBytes, m31errors.ErrContextExceeded)
				}
			}
		}
	}

	toolCalls := finalizeToolCalls(builders, e)
	return content.String(), toolCalls, lastUsage, nil
}

// finalizeToolCalls converts accumulated toolCallBuilders into ToolCall structs.
// Sorts by index for deterministic ordering. Normalizes tool names and parses
// arguments as JSON.
func finalizeToolCalls(builders map[int]*toolCallBuilder, e *Engine) []m31types.ToolCall {
	if len(builders) == 0 {
		return nil
	}

	indices := make([]int, 0, len(builders))
	for idx := range builders {
		indices = append(indices, idx)
	}
	sort.Ints(indices)

	if len(indices) > m31types.MaxToolsPerCall {
		slog.Warn("finalizeToolCalls: tool count exceeded cap, truncating",
			"count", len(indices), "cap", m31types.MaxToolsPerCall)
		indices = indices[:m31types.MaxToolsPerCall]
	}

	calls := make([]m31types.ToolCall, 0, len(indices))
	for _, idx := range indices {
		b := builders[idx]
		name := normalizeToolName(b.name)

		args := b.arguments.String()
		var input json.RawMessage
		if args != "" {
			input = json.RawMessage(args)
		} else {
			input = json.RawMessage("{}")
		}

		id := b.id
		if id == "" {
			id = fmt.Sprintf("call_%s_%d", name, e.nextCallID())
		}

		calls = append(calls, m31types.ToolCall{
			ID:    id,
			Name:  name,
			Input: input,
		})
	}
	return calls
}

// prepareStreamRequest handles the shared preamble for all streamLLM variants:
// preflight context check, build ChatRequest, emit thinking start, and open
// the stream with retry. Returns the iterator on success.
func (e *Engine) prepareStreamRequest(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (*m31types.StreamIterator, error) {
	msgs, err := e.preflightContextCheck(messages)
	if err != nil {
		return nil, err
	}

	e.emit(ThinkingStartMsg{
		Context: "LLM processing...",
	})

	req := provider.ChatRequest{
		Model:            e.modelForPhase(e.stateMachine.CurrentPhase()),
		Messages:         msgs,
		ReasoningEnabled: true,
	}
	if toolsEnabled {
		req.Tools = e.buildToolDefinitions()
	}

	rp, _ := e.providerAndModel()
	iterator, err := rp.ChatCompletionStream(ctx, req)
	if err != nil {
		iterator, err = e.retryChatStream(ctx, req, err)
		if err != nil {
			e.emit(ThinkingCompleteMsg{
				Context: "LLM processing failed",
			})
			return nil, err
		}
	}
	return iterator, nil
}

// emitThinkingDone emits the thinking complete message.
func (e *Engine) emitThinkingDone() {
	e.emit(ThinkingCompleteMsg{
		Context: "LLM processing complete",
	})
}

// calibrateFromUsage feeds actual API token counts back into the estimator
// so provider-specific heuristics self-correct over the lifetime of a workflow.
// Only PromptTokens is used because it represents the model's actual view of
// the input context, which is what EstimateMessages tries to approximate.
func (e *Engine) calibrateFromUsage(messages []m31types.Message, usage *m31types.Usage) {
	if e.tokens == nil || usage == nil || usage.PromptTokens <= 0 {
		return
	}
	estimated := e.tokens.EstimateMessages(messages)
	e.tokens.Calibrate(estimated, usage.PromptTokens)
}

// streamLLMWithTools sends a chat request with tool definitions and returns
// both the text content and any native tool calls from the response.
// Used by execute and heal phases for structured tool dispatch.
func (e *Engine) streamLLMWithTools(ctx context.Context, messages []m31types.Message) (string, []m31types.ToolCall, error) {
	iterator, err := e.prepareStreamRequest(ctx, messages, true)
	if err != nil {
		return "", nil, err
	}

	content, toolCalls, usage, err := e.consumeStreamWithTools(iterator)
	e.calibrateFromUsage(messages, usage)
	e.emitThinkingDone()
	return content, toolCalls, err
}

// streamLLM sends a chat request and returns the full response content.
func (e *Engine) streamLLM(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (string, error) {
	iterator, err := e.prepareStreamRequest(ctx, messages, toolsEnabled)
	if err != nil {
		return "", err
	}

	result, usage, err := e.consumeStream(iterator)
	e.calibrateFromUsage(messages, usage)
	e.emitThinkingDone()
	return result, err
}

// streamLLMStreaming sends a chat request and returns the underlying
// StreamIterator. The caller is responsible for iterating via Next()
// and emitting each chunk to the TUI (typically via MsgEmitter).
func (e *Engine) streamLLMStreaming(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (*m31types.StreamIterator, error) {
	return e.prepareStreamRequest(ctx, messages, toolsEnabled)
}

// retryChatStream retries a failed ChatCompletionStream call using exponential
// backoff. The firstErr is the error from the initial attempt. Returns the
// iterator from a successful retry or the last error if all retries fail.
func (e *Engine) retryChatStream(ctx context.Context, req provider.ChatRequest, firstErr error) (*m31types.StreamIterator, error) {
	class, reason := retry.ClassifyError(firstErr)
	if !retry.IsRetryable(class) {
		return nil, firstErr
	}

	policy := retry.DefaultPolicy()
	if e.cfg != nil {
		policy = retry.ConfiguredPolicy(
			e.cfg.Features.RetryMaxAttempts,
			e.cfg.Features.RetryBaseDelayMs,
			e.cfg.Features.RetryMaxDelayMs,
			e.cfg.Features.RetryBackoffMultiplier,
		)
	}
	var lastErr = firstErr

	for attempt := 1; attempt <= policy.MaxAttempts; attempt++ {
		delay := policy.Delay(attempt, nil)
		slog.Info("retrying LLM request", "attempt", attempt, "max", policy.MaxAttempts, "delay", delay, "reason", reason)

		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("%w: %v", ctx.Err(), lastErr)
		case <-time.After(delay):
		}

		sp, _ := e.providerAndModel()
		iterator, err := sp.ChatCompletionStream(ctx, req)
		if err == nil {
			return iterator, nil
		}
		lastErr = err

		class, reason = retry.ClassifyError(err)
		if !retry.IsRetryable(class) {
			return nil, err
		}
	}

	return nil, lastErr
}

// ExtractWebsiteTemplate extracts the bundled Next.js website template to the
// specified directory. It creates the directory structure and writes all template
// files. Returns an error if extraction fails.
func ExtractWebsiteTemplate(destDir string) error {
	srcDir := "templates/website-nextjs"
	return fs.WalkDir(websiteTemplateFS, srcDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return fmt.Errorf("walk template dir: %w", err)
		}
		// Compute the relative path within the template
		relPath, err := filepath.Rel(srcDir, path)
		if err != nil {
			return fmt.Errorf("compute relative path: %w", err)
		}
		if relPath == "." {
			return nil
		}
		dest := filepath.Join(destDir, relPath)
		if d.IsDir() {
			return os.MkdirAll(dest, 0o755)
		}
		data, err := websiteTemplateFS.ReadFile(path)
		if err != nil {
			return fmt.Errorf("read template file %q: %w", path, err)
		}
		// Ensure parent directory exists
		if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
			return fmt.Errorf("create parent dir %q: %w", filepath.Dir(dest), err)
		}
		return os.WriteFile(dest, data, 0o644)
	})
}

// truncateForLog truncates a string to maxLen, adding ellipsis if needed.
func truncateForLog(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
