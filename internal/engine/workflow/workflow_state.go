package workflow

import (
	"sync"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
)

// WorkflowState groups mutable session state extracted from Engine.
//
// Lock ordering (MUST be respected by all callers):
//
//	transitionMu > planMu > messagesMu > intentResultMu > cachedFullPromptsMu > checkpointMu
//
// All lock acquisitions must follow this order to prevent deadlocks.
// When acquiring multiple locks, always acquire in the documented order.
type WorkflowState struct {
	// transitionMu serializes phase transitions to prevent interleaved checkpoint saves.
	transitionMu sync.Mutex

	// planMu guards planMarkdown, planVersion, refineFeedback, and researchOutput
	// to prevent races between workflow goroutine writes and TUI reads.
	planMu sync.RWMutex

	// messagesMu guards Messages to prevent races between workflow goroutine
	// writes (via PrePhaseSetup) and any concurrent reads.
	messagesMu sync.RWMutex

	// intentResultMu guards intentResult and lastHealReport.
	intentResultMu sync.RWMutex

	// cachedFullPromptsMu guards cachedFullPrompts and cachedDynamicContext.
	cachedFullPromptsMu sync.Mutex

	// checkpointMu guards checkpointData and currentGoal.
	checkpointMu sync.RWMutex

	// Plan state
	planMarkdown   string // current plan content for refinement context
	planVersion    int    // current plan version (increments on refine)
	refineFeedback string // pending refinement feedback from user
	researchOutput string // pre-plan research results for injection into plan context

	// Cached base prompt (built once, used by ContextBuilder)
	cachedBasePrompt     string
	cachedBasePromptOnce sync.Once

	// Cached full system prompts per extras signature (used by ContextBuilder)
	cachedFullPrompts map[string]string

	// Conversation messages for the workflow (updated by /compress proactively)
	Messages []types.Message

	// Intent classification result from the LLM-based classifier.
	// Set before the workflow starts; used to enrich discuss/research/plan context.
	intentResult *types.IntentResult

	// Dynamic context change detection (used by ContextBuilder)
	contextSnapshot      map[string]string
	cachedDynamicContext string

	// v1.5: Decision logging
	decisionLog *decision.Logger

	// v1.5: Self-heal explanation
	lastHealReport *types.HealReport

	// v1.5: Checkpoint resume
	checkpointData *CheckpointData

	// currentGoal tracks the active workflow goal for checkpoint persistence.
	currentGoal string
}

// PlanState accessors

// PlanContent returns the current plan markdown content.
func (ws *WorkflowState) PlanContent() string {
	ws.planMu.RLock()
	defer ws.planMu.RUnlock()
	return ws.planMarkdown
}

// SetPlanContent sets the plan markdown content.
func (ws *WorkflowState) SetPlanContent(content string) {
	ws.planMu.Lock()
	defer ws.planMu.Unlock()
	ws.planMarkdown = content
}

// PlanVersion returns the current plan version number.
// Version 1 is the initial plan; each refinement increments it.
func (ws *WorkflowState) PlanVersion() int {
	ws.planMu.RLock()
	defer ws.planMu.RUnlock()
	return ws.planVersion
}

// SetPlanVersion sets the plan version directly.
func (ws *WorkflowState) SetPlanVersion(v int) {
	ws.planMu.Lock()
	defer ws.planMu.Unlock()
	ws.planVersion = v
}

// IncrementPlanVersion increments the plan version and returns the new value.
func (ws *WorkflowState) IncrementPlanVersion() int {
	ws.planMu.Lock()
	defer ws.planMu.Unlock()
	ws.planVersion++
	return ws.planVersion
}

// RefineFeedback returns the pending refinement feedback.
func (ws *WorkflowState) RefineFeedback() string {
	ws.planMu.RLock()
	defer ws.planMu.RUnlock()
	return ws.refineFeedback
}

// SetRefineFeedback sets the refinement feedback.
func (ws *WorkflowState) SetRefineFeedback(feedback string) {
	ws.planMu.Lock()
	defer ws.planMu.Unlock()
	ws.refineFeedback = feedback
}

// ResearchOutput returns the pre-plan research results.
func (ws *WorkflowState) ResearchOutput() string {
	ws.planMu.RLock()
	defer ws.planMu.RUnlock()
	return ws.researchOutput
}

// SetResearchOutput sets the pre-plan research results.
func (ws *WorkflowState) SetResearchOutput(output string) {
	ws.planMu.Lock()
	defer ws.planMu.Unlock()
	ws.researchOutput = output
}

// MessageState accessors

// MessagesSnapshot returns a copy of the current conversation messages.
func (ws *WorkflowState) MessagesSnapshot() []types.Message {
	ws.messagesMu.RLock()
	defer ws.messagesMu.RUnlock()
	snapshot := make([]types.Message, len(ws.Messages))
	copy(snapshot, ws.Messages)
	return snapshot
}

// AppendMessage adds a message to the conversation.
func (ws *WorkflowState) AppendMessage(msg types.Message) {
	ws.messagesMu.Lock()
	defer ws.messagesMu.Unlock()
	ws.Messages = append(ws.Messages, msg)
}

// SetMessages replaces the entire message list.
func (ws *WorkflowState) SetMessages(msgs []types.Message) {
	ws.messagesMu.Lock()
	defer ws.messagesMu.Unlock()
	ws.Messages = msgs
}

// ClearMessages resets the message list to empty.
func (ws *WorkflowState) ClearMessages() {
	ws.messagesMu.Lock()
	defer ws.messagesMu.Unlock()
	ws.Messages = nil
}

// IntentState accessors

// IntentResult returns the stored intent classification result, or nil if unset.
func (ws *WorkflowState) IntentResult() *types.IntentResult {
	ws.intentResultMu.RLock()
	defer ws.intentResultMu.RUnlock()
	return ws.intentResult
}

// SetIntentResult stores the LLM-classified intent result.
func (ws *WorkflowState) SetIntentResult(ir *types.IntentResult) {
	ws.intentResultMu.Lock()
	defer ws.intentResultMu.Unlock()
	ws.intentResult = ir
}

// LastHealReport returns the most recent self-heal report, or nil if none.
func (ws *WorkflowState) LastHealReport() *types.HealReport {
	ws.intentResultMu.RLock()
	defer ws.intentResultMu.RUnlock()
	return ws.lastHealReport
}

// SetLastHealReport stores the most recent self-heal report.
func (ws *WorkflowState) SetLastHealReport(hr *types.HealReport) {
	ws.intentResultMu.Lock()
	defer ws.intentResultMu.Unlock()
	ws.lastHealReport = hr
}

// CheckpointState accessors

// CheckpointData returns the current checkpoint data, or nil if none.
func (ws *WorkflowState) CheckpointData() *CheckpointData {
	ws.checkpointMu.RLock()
	defer ws.checkpointMu.RUnlock()
	return ws.checkpointData
}

// SetCheckpointData stores checkpoint data for resume.
func (ws *WorkflowState) SetCheckpointData(data *CheckpointData) {
	ws.checkpointMu.Lock()
	defer ws.checkpointMu.Unlock()
	ws.checkpointData = data
}

// CurrentGoal returns the active workflow goal.
func (ws *WorkflowState) CurrentGoal() string {
	ws.checkpointMu.RLock()
	defer ws.checkpointMu.RUnlock()
	return ws.currentGoal
}

// SetCurrentGoal sets the active workflow goal.
func (ws *WorkflowState) SetCurrentGoal(goal string) {
	ws.checkpointMu.Lock()
	defer ws.checkpointMu.Unlock()
	ws.currentGoal = goal
}

// DecisionLog accessors

// DecisionLog returns the decision logger. The logger is thread-safe by design.
func (ws *WorkflowState) DecisionLog() *decision.Logger {
	return ws.decisionLog
}

// CachedContext accessors

// CachedBasePrompt returns the cached base prompt string.
func (ws *WorkflowState) CachedBasePrompt() string {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	return ws.cachedBasePrompt
}

// SetCachedBasePrompt stores the cached base prompt.
func (ws *WorkflowState) SetCachedBasePrompt(prompt string) {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	ws.cachedBasePrompt = prompt
}

// DoCachedBasePromptOnce executes fn exactly once for base prompt initialization.
func (ws *WorkflowState) DoCachedBasePromptOnce(fn func()) {
	ws.cachedBasePromptOnce.Do(fn)
}

// GetCachedFullPrompt retrieves a cached full prompt by key.
func (ws *WorkflowState) GetCachedFullPrompt(key string) (string, bool) {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	if ws.cachedFullPrompts == nil {
		return "", false
	}
	v, ok := ws.cachedFullPrompts[key]
	return v, ok
}

// SetCachedFullPrompt stores a cached full prompt.
func (ws *WorkflowState) SetCachedFullPrompt(key, value string) {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	if ws.cachedFullPrompts == nil {
		ws.cachedFullPrompts = make(map[string]string)
	}
	ws.cachedFullPrompts[key] = value
}

// EnsureCachedFullPrompts initializes the cachedFullPrompts map if nil.
func (ws *WorkflowState) EnsureCachedFullPrompts() {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	if ws.cachedFullPrompts == nil {
		ws.cachedFullPrompts = make(map[string]string)
	}
}

// ContextSnapshot returns the current dynamic context snapshot.
func (ws *WorkflowState) ContextSnapshot() map[string]string {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	return ws.contextSnapshot
}

// SetContextSnapshot sets the dynamic context snapshot.
func (ws *WorkflowState) SetContextSnapshot(snapshot map[string]string) {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	ws.contextSnapshot = snapshot
}

// CachedDynamicContext returns the cached dynamic context string.
func (ws *WorkflowState) CachedDynamicContext() string {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	return ws.cachedDynamicContext
}

// SetCachedDynamicContext sets the cached dynamic context string.
func (ws *WorkflowState) SetCachedDynamicContext(ctx string) {
	ws.cachedFullPromptsMu.Lock()
	defer ws.cachedFullPromptsMu.Unlock()
	ws.cachedDynamicContext = ctx
}
