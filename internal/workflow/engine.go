package workflow

import (
	"context"
	"embed"
	"fmt"
	"io"
	"log/slog"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

//go:embed prompts/*.md
var promptFS embed.FS

// PromptRegistry holds all loaded prompt templates.
type PromptRegistry struct {
	Base          string
	ToolUse       string
	PlanFormat    string
	ExecuteTask   string
	Discuss       string
	SelfHeal      string
	Demonstration string
}

// LoadPrompts reads all embedded prompt files and returns a registry.
func LoadPrompts() (*PromptRegistry, error) {
	r := &PromptRegistry{}
	files := map[string]*string{
		"prompts/base.md":                 &r.Base,
		"prompts/tool-use.md":             &r.ToolUse,
		"prompts/plan-format.md":          &r.PlanFormat,
		"prompts/execute-task.md":         &r.ExecuteTask,
		"prompts/discuss-questions.md":    &r.Discuss,
		"prompts/self-heal.md":            &r.SelfHeal,
		"prompts/demonstration-format.md": &r.Demonstration,
	}
	for path, ptr := range files {
		data, err := promptFS.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("load prompt %s: %w", path, err)
		}
		*ptr = strings.TrimSpace(string(data))
	}
	return r, nil
}

// Engine orchestrates the six-phase workflow.
type Engine struct {
	sessionID        string
	workDir          string
	backupDir        string
	sessionsRoot     string // store sessions root for reliable planningDir recalculation
	planningDir      string
	provider         provider.LLMProvider
	modelID          string
	activePhase      m31types.WorkflowPhase
	cfg              *config.Config
	git              *git.Git
	dispatcher       *tools.Dispatcher
	tokens           *tokens.Estimator
	sessionMgr       *session.Manager
	prompts          *PromptRegistry
	logger           *slog.Logger
	startTime        time.Time
	sessionStartHash string
	discussState     DiscussState
	execCommand      func(name string, args ...string) *exec.Cmd
	msgEmitter       MsgEmitter
	callCounter      int64
	totalCost        float64 // cumulative cost for budget tracking
	planMarkdown     string  // current plan content for refinement context
	planVersion      int     // current plan version (increments on refine)
	refineFeedback   string  // pending refinement feedback from user
	// perPhaseModels holds per-phase model overrides set by the TUI via SetPhaseModel.
	// Keys are WorkflowPhase values; values are model ID strings.
	// When set, takes precedence over AgentsConfig and cfg.Model.Default.
	perPhaseModels map[m31types.WorkflowPhase]string
}

// gitConfig returns the git config with safe defaults when cfg is nil.
func (e *Engine) gitConfig() config.GitConfig {
	if e.cfg != nil {
		return e.cfg.Git
	}
	return config.DefaultGitConfig()
}

// modelForPhase returns the per-phase model ID, checked in priority order:
//  1. perPhaseModels (set interactively by the TUI at workflow start)
//  2. AgentsConfig from config.toml
//  3. cfg.Model.Default
//  4. the engine's active modelID
func (e *Engine) modelForPhase(phase m31types.WorkflowPhase) string {
	// 1. Interactive per-phase override (highest priority)
	if id, ok := e.perPhaseModels[phase]; ok && id != "" {
		return id
	}
	if e.cfg == nil {
		return e.modelID
	}
	// 2. AgentsConfig from config.toml
	var override string
	switch phase {
	case m31types.PhasePlan:
		override = e.cfg.Agents.Plan
	case m31types.PhaseExecute:
		override = e.cfg.Agents.Execute
	case m31types.PhaseVerify:
		override = e.cfg.Agents.Verify
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
	if e.perPhaseModels == nil {
		e.perPhaseModels = make(map[m31types.WorkflowPhase]string)
	}
	if modelID != "" {
		e.perPhaseModels[phase] = modelID
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

	prompts, err := LoadPrompts()
	if err != nil {
		return nil, fmt.Errorf("failed to load prompts: %w", err)
	}

	// derive sessionsRoot from planningDir reliably
	// planningDir = <sessionsRoot>/<sessionID>/planning
	sessionsRoot := filepath.Dir(filepath.Dir(opts.PlanningDir))

	return &Engine{
		sessionID:    opts.SessionID,
		workDir:      opts.WorkDir,
		backupDir:    opts.BackupDir,
		sessionsRoot: sessionsRoot,
		planningDir:  opts.PlanningDir,
		provider:     opts.Provider,
		modelID:      opts.ModelID,
		cfg:          opts.Config,
		dispatcher:   opts.Dispatcher,
		tokens:       opts.TokenEst,
		sessionMgr:   opts.SessionMgr,
		prompts:      prompts,
		logger:       slog.Default(),
		startTime:    time.Now(),
		execCommand:  exec.Command,
	}, nil
}

// SetModel updates the active model ID and provider for the engine.
func (e *Engine) SetModel(modelID string, p provider.LLMProvider) {
	e.modelID = modelID
	if p != nil {
		e.provider = p
	}
}

// RunPhase executes the given workflow phase and returns the result.
func (e *Engine) RunPhase(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	// Budget guardrail: check cumulative cost before each phase
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		if e.totalCost >= e.cfg.Features.BudgetLimitUSD {
			return &PhaseResult{
				Phase:   phase,
				Success: false,
				Error:   fmt.Sprintf("budget limit exceeded: $%.4f spent of $%.4f limit", e.totalCost, e.cfg.Features.BudgetLimitUSD),
			}, fmt.Errorf("budget limit exceeded: $%.4f of $%.4f", e.totalCost, e.cfg.Features.BudgetLimitUSD)
		}
	}

	start := time.Now()
	e.activePhase = phase

	var result *PhaseResult
	var err error

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
	case m31types.PhaseShip:
		result, err = e.runShip(ctx, goal)
	default:
		return nil, fmt.Errorf("unknown phase: %s", phase)
	}

	if result != nil {
		result.DurationMs = time.Since(start).Milliseconds()
		result.Phase = phase
		// Accumulate cost for budget tracking
		if result.Cost > 0 {
			e.totalCost += result.Cost
		}
	}

	return result, err
}

// validPhaseTransitions defines which phase transitions are allowed.
// Any transition not in this map is rejected with ErrPhaseTransition.
var validPhaseTransitions = map[m31types.WorkflowPhase][]m31types.WorkflowPhase{
	m31types.PhaseIdle:       {m31types.PhaseInitialize},
	m31types.PhaseInitialize: {m31types.PhaseDiscuss, m31types.PhaseIdle},
	m31types.PhaseDiscuss:    {m31types.PhasePlan, m31types.PhaseIdle},
	m31types.PhasePlan:       {m31types.PhaseExecute, m31types.PhasePlan, m31types.PhaseDiscuss, m31types.PhaseIdle},
	m31types.PhaseExecute:    {m31types.PhaseVerify, m31types.PhaseIdle},
	m31types.PhaseVerify:     {m31types.PhaseShip, m31types.PhaseExecute, m31types.PhaseIdle},
	m31types.PhaseShip:       {m31types.PhaseIdle},
}

// Transition saves a checkpoint and writes STATE.md for the new phase.
// Validates the transition is allowed by the phase ordering guard.
func (e *Engine) Transition(ctx context.Context, from, to m31types.WorkflowPhase) error {
	// Phase transition guard — reject out-of-order transitions.
	allowed, ok := validPhaseTransitions[from]
	if !ok {
		return fmt.Errorf("invalid phase transition from %s to %s: %w", from, to, m31errors.ErrPhaseTransition)
	}
	valid := false
	for _, a := range allowed {
		if a == to {
			valid = true
			break
		}
	}
	if !valid {
		return fmt.Errorf("invalid phase transition from %s to %s: %w", from, to, m31errors.ErrPhaseTransition)
	}

	// Emit phase transition start message
	e.emit(PhaseTransitionStartMsg{
		From:    string(from),
		To:      string(to),
		Context: fmt.Sprintf("Moving to %s phase...", to),
	})

	// Save checkpoint
	cp := session.Checkpoint{
		Phase:     to,
		Timestamp: time.Now(),
	}
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, cp); err != nil {
		e.emit(PhaseTransitionCompleteMsg{
			From:    string(from),
			To:      string(to),
			Success: false,
			Error:   err.Error(),
		})
		return fmt.Errorf("save checkpoint: %w", err)
	}

	// Write STATE.md
	if err := e.sessionMgr.SaveState(e.sessionID, to, "transitioning", string(to)); err != nil {
		e.emit(PhaseTransitionCompleteMsg{
			From:    string(from),
			To:      string(to),
			Success: false,
			Error:   err.Error(),
		})
		return fmt.Errorf("save state: %w", err)
	}

	// Emit phase transition complete message
	e.emit(PhaseTransitionCompleteMsg{
		From:    string(from),
		To:      string(to),
		Success: true,
	})

	return nil
}

// SetGit sets the git instance on the engine.
// Required before running any phase that uses git operations.
func (e *Engine) SetGit(g *git.Git) {
	e.git = g
	if h, err := g.HeadHash(); err == nil {
		e.sessionStartHash = h
	}
}

// SessionID returns the current session ID.
func (e *Engine) SessionID() string {
	return e.sessionID
}

// SetSessionID updates the engine's session ID and replans the planning
// directory to point to the new session's planning folder. Used after
// session-switching commands like /fork, /prev, /next.
func (e *Engine) SetSessionID(id string) {
	e.sessionID = id
	// use stored sessionsRoot instead of brittle .. navigation
	e.planningDir = filepath.Join(e.sessionsRoot, id, "planning")
}

// SetMsgEmitter sets the callback for emitting messages back to the TUI.
func (e *Engine) SetMsgEmitter(em MsgEmitter) {
	e.msgEmitter = em
}

// HealTask triggers self-healing for a specific task by ID.
// Returns true if healing was attempted, false if the task cannot be healed.
func (e *Engine) HealTask(ctx context.Context, taskID int) bool {
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		e.logger.Warn("failed to load tasks for heal", "error", err)
		return false
	}
	for i, task := range tasks {
		if task.ID == taskID && task.Status == m31types.StatusFailed {
			if task.HealsAttempted >= m31types.MaxHealAttempts {
				e.logger.Warn("task already at max heal attempts", "id", taskID)
				return false
			}
			tasks[i].HealsAttempted++
			e.emit(SelfHealStartMsg{
				TaskID:  task.ID,
				Attempt: tasks[i].HealsAttempted,
				Max:     m31types.MaxHealAttempts,
			})
			failure := fmt.Sprintf(
				"Manual heal triggered by user.\nTask description: %s\nFiles: %v\nAcceptance criteria: %v\n"+
					"Inspect the files listed above, identify any issues, and apply a fix.",
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
				newResult := e.verifyTask(ctx, task)
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
			_ = e.sessionMgr.SaveTasks(e.sessionID, tasks)
			return true
		}
	}
	return false
}

// emit sends a message to the TUI if an emitter is configured.
func (e *Engine) emit(msg any) {
	if e.msgEmitter != nil {
		e.msgEmitter.Emit(msg)
	}
}

// preflightContextCheck estimates token usage before each LLM request
// and returns ErrContextExceeded if the estimate exceeds 95% of the
// model's context window. Returns nil if estimation is unavailable.
func (e *Engine) preflightContextCheck(messages []m31types.Message) error {
	if e.tokens == nil || e.provider == nil {
		return nil
	}
	modelInfo, err := e.provider.GetModel(e.modelForPhase(e.activePhase))
	if err != nil || modelInfo == nil {
		return nil
	}
	estimated := e.tokens.EstimateMessages(messages)
	contextLength := modelInfo.ContextLength
	if contextLength <= 0 {
		contextLength = m31types.DefaultContextLength
	}
	if float64(estimated) > 0.95*float64(contextLength) {
		return fmt.Errorf("%w: estimated %d tokens exceeds 95%% of %d context", m31errors.ErrContextExceeded, estimated, contextLength)
	}
	if float64(estimated) > 0.80*float64(contextLength) {
		slog.Warn("context usage approaching limit", "estimated", estimated, "limit", contextLength, "pct", float64(estimated)/float64(contextLength))
	}
	return nil
}

// SubmitDiscussAnswer records an answer for a discuss question.
func (e *Engine) SubmitDiscussAnswer(index int, answer string) error {
	if e.discussState.Questions == nil {
		return fmt.Errorf("no discuss questions to answer")
	}
	if index < 0 || index >= len(e.discussState.Questions) {
		return fmt.Errorf("invalid question index: %d", index)
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
	project, projErr := e.sessionMgr.LoadProject(e.sessionID)
	if projErr != nil {
		e.logger.Warn("failed to load project for discuss finalization", "error", projErr)
	}
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
func (e *Engine) SetRefinementFeedback(feedback string) {
	e.refineFeedback = feedback
	if feedback != "" {
		e.planVersion++
	}
}

// PlanContent returns the current plan markdown content.
func (e *Engine) PlanContent() string {
	return e.planMarkdown
}

// PlanVersion returns the current plan version number.
// Version 1 is the initial plan; each refinement increments it.
func (e *Engine) PlanVersion() int {
	return e.planVersion
}

// buildToolDefinitions returns the tool definitions for the LLM.
func (e *Engine) buildToolDefinitions() []provider.ToolDefinition {
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
		// use SchemaProvider interface for real parameter schemas
		if sp, ok := tool.(m31types.SchemaProvider); ok {
			def.Parameters = sp.ParameterSchema()
		}
		defs = append(defs, def)
	}
	return defs
}

// buildSystemPrompt composes the system prompt from base + optional extras.
func (e *Engine) buildSystemPrompt(extra ...string) string {
	parts := []string{e.prompts.Base}
	for _, p := range extra {
		if p != "" {
			parts = append(parts, p)
		}
	}
	return strings.Join(parts, "\n\n---\n\n")
}

// consumeStream reads all chunks from the iterator and returns the concatenated content.
// Enforces MaxLLMResponseBytes limit to prevent OOM from pathological responses.
func (e *Engine) consumeStream(iterator *m31types.StreamIterator) (string, error) {
	var sb strings.Builder
	defer iterator.Close()

	for {
		chunk, err := iterator.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			// Preserve partial content before non-EOF errors
			if chunk != nil && chunk.Delta != "" {
				sb.WriteString(chunk.Delta)
			}
			return sb.String(), err
		}
		if chunk != nil && chunk.Delta != "" {
			sb.WriteString(chunk.Delta)
			// Enforce max response size incrementally as chunks arrive
			if sb.Len() > m31types.MaxLLMResponseBytes {
				return sb.String(), fmt.Errorf("LLM response exceeds maximum size of %d bytes: %w",
					m31types.MaxLLMResponseBytes, m31errors.ErrContextExceeded)
			}
		}
	}
	return sb.String(), nil
}

// streamLLM sends a chat request and returns the full response content.
func (e *Engine) streamLLM(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (string, error) {
	// preflight context check before sending to LLM
	if err := e.preflightContextCheck(messages); err != nil {
		return "", err
	}

	// Emit thinking start message
	e.emit(ThinkingStartMsg{
		Context: "LLM processing...",
	})

	req := provider.ChatRequest{
		Model:            e.modelForPhase(e.activePhase),
		Messages:         messages,
		ReasoningEnabled: true,
	}
	if toolsEnabled {
		req.Tools = e.buildToolDefinitions()
	}

	iterator, err := e.provider.ChatCompletionStream(ctx, req)
	if err != nil {
		e.emit(ThinkingCompleteMsg{
			Context: "LLM processing failed",
		})
		return "", err
	}

	result, err := e.consumeStream(iterator)
	e.emit(ThinkingCompleteMsg{
		Context: "LLM processing complete",
	})
	return result, err
}

// streamLLMStreaming sends a chat request and returns the underlying
// StreamIterator. The caller is responsible for iterating via Next()
// and emitting each chunk to the TUI (typically via MsgEmitter).
func (e *Engine) streamLLMStreaming(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (*m31types.StreamIterator, error) {
	req := provider.ChatRequest{
		Model:            e.modelForPhase(e.activePhase),
		Messages:         messages,
		ReasoningEnabled: true,
	}
	if toolsEnabled {
		req.Tools = e.buildToolDefinitions()
	}

	return e.provider.ChatCompletionStream(ctx, req)
}
