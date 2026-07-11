package tui

import (
	"context"
	"fmt"
	"log/slog"
	"path/filepath"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/arbitrage"
	"github.com/eshanized/M31A/pkg/metrics"
)

// ─── Bubble Tea Model interface ───────────────────────────────────────────────

// Init implements tea.Model. It starts the health ticker and permission listener.
func (m *AppState) Init() tea.Cmd {
	// Session retention cleanup: remove sessions older than configured retention.
	if m.sessionManager != nil && m.config != nil {
		retentionDays := m.config.Features.SessionRetentionDays
		if retentionDays <= 0 {
			retentionDays = 30
		}
		if removed, err := m.sessionManager.Cleanup(time.Duration(retentionDays) * 24 * time.Hour); err != nil {
			slog.Warn("session cleanup failed", "removed", removed, "error", err)
		} else if removed > 0 {
			slog.Info("session cleanup completed", "removed", removed)
		}
	}

	// Startup routing decision: skip first-run if provider is already configured.
	hasProvider := m.registry != nil && m.activeProvider != ""
	if hasProvider {
		m.screen = ScreenHome
		m.ensureReplModel()

		// Populate activeModel from config.Model.Default so the REPL can
		// send messages immediately without requiring /model first.
		// We set a stub now and kick off an async fetch to enrich it with
		// full metadata (pricing, context length, capabilities).
		if m.activeModel == nil && m.config != nil && m.config.Model.Default != "" {
			defaultModelID := m.config.Model.Default
			// Try the provider's cached model list first (zero-cost lookup)
			if p := m.registry.ActiveProvider(); p != nil {
				if info, err := p.GetModel(defaultModelID); err == nil && info != nil {
					m.activeModel = info
				}
			}
			// If not found in cache, use a stub — the async SetProvider fetch
			// will replace it with full metadata once it completes.
			if m.activeModel == nil {
				m.activeModel = &types.ModelInfo{ID: defaultModelID}
			}
		}

		baseCmds := []tea.Cmd{
			m.routeToScreen(),
			NextHealthTick(m.shutdownCtx, types.HealthCheckInterval),
		}
		if m.dispatcher != nil {
			baseCmds = append(baseCmds, permListenerCmd(m.shutdownCtx, m.dispatcher))
			baseCmds = append(baseCmds, questionListenerCmd(m.shutdownCtx, m.dispatcher))
		}
		if m.subagentManager != nil {
			baseCmds = append(baseCmds, subagentListenerCmd(m.shutdownCtx, m.subagentManager.Events()))
		}
		if m.sidebarModel != nil {
			baseCmds = append(baseCmds, m.sidebarModel.refreshCmd())
			baseCmds = append(baseCmds, NextSidebarRefreshTick(m.shutdownCtx, SidebarRefreshInterval))
		}
		// Periodic emitter drop counter logging (if any drops occurred).
		baseCmds = append(baseCmds, EmitterDropLogTick(m.shutdownCtx))
		// Start file watcher for real-time sidebar refresh
		baseCmds = append(baseCmds, m.startFileWatcher())

		// Start config watcher for hot-reload of config.toml
		baseCmds = append(baseCmds, m.startConfigWatcher())

		// Async provider+model enrichment: fetches the model catalog so the REPL
		// has full model metadata (pricing, context, capabilities) for display.
		if providerCmd := m.syncReplProvider(m.sessionID); providerCmd != nil {
			baseCmds = append(baseCmds, providerCmd)
		}

		if m.resumeSessionID != "" {
			resumeID := m.resumeSessionID
			m.resumeSessionID = ""
			baseCmds = append(baseCmds, m.loadAndRestoreSession(resumeID, true))
			return tea.Batch(baseCmds...)
		}
		baseCmds = append(baseCmds, m.startNewSession())
		return tea.Batch(baseCmds...)
	}

	cmds := []tea.Cmd{
		m.routeToScreen(),
		NextHealthTick(m.shutdownCtx, types.HealthCheckInterval),
	}
	if m.dispatcher != nil {
		cmds = append(cmds, permListenerCmd(m.shutdownCtx, m.dispatcher))
		cmds = append(cmds, questionListenerCmd(m.shutdownCtx, m.dispatcher))
	}
	if m.subagentManager != nil {
		cmds = append(cmds, subagentListenerCmd(m.shutdownCtx, m.subagentManager.Events()))
	}
	if m.sidebarModel != nil {
		cmds = append(cmds, m.sidebarModel.refreshCmd())
		cmds = append(cmds, NextSidebarRefreshTick(m.shutdownCtx, SidebarRefreshInterval))
	}
	// Start file watcher for real-time sidebar refresh
	cmds = append(cmds, m.startFileWatcher())

	// Start config watcher for hot-reload of config.toml
	cmds = append(cmds, m.startConfigWatcher())
	return tea.Batch(cmds...)
}

// ─── Shutdown ─────────────────────────────────────────────────────────────────

// startFileWatcher creates and starts a file watcher for the working directory.
// The watcher monitors filesystem changes and triggers immediate sidebar refreshes.
func (m *AppState) startFileWatcher() tea.Cmd {
	workDir := "."
	if m.git != nil {
		workDir = m.git.WorkDir()
	}
	if workDir == "" || workDir == "." {
		return nil
	}
	ch := make(chan tea.Msg, 16)
	fw, err := NewFileWatcher(workDir, ch)
	if err != nil {
		slog.Warn("file watcher init failed — sidebar auto-refresh disabled", "error", err)
		return nil
	}
	m.fileWatcher = fw
	// Drain the watcher channel: each file change triggers a sidebar refresh
	return func() tea.Msg {
		select {
		case msg := <-fw.Events:
			return msg
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// Shutdown cleanly tears down all background goroutines.
func (m *AppState) Shutdown() {
	// W2: Persist session state before cancelling contexts so messages,
	// workflow state, and checkpoints survive process exit.
	m.saveSessionOnShutdown()

	// W3: Stop all managed dev servers so child processes do not orphan.
	if m.dispatcher != nil {
		if tool, ok := m.dispatcher.GetTool("DevServer"); ok {
			if ds, castOK := tool.(*tools.DevServer); castOK {
				ds.StopAll()
			}
		}
	}

	if m.fileWatcher != nil {
		m.fileWatcher.Close()
	}
	if m.configWatcherStop != nil {
		close(m.configWatcherStop)
	}
	if m.frecentHistory != nil {
		if err := m.frecentHistory.Save(); err != nil {
			slog.Warn("failed to save history on shutdown", "error", err)
		}
	}
	// Flush metrics before shutting down
	if m.collector != nil {
		m.collector.Stop()
	}
	if m.workflowCancel != nil {
		m.workflowCancel()
	}
	if m.shutdownCancel != nil {
		m.shutdownCancel()
	}
	if m.streamCancelFn != nil {
		m.streamCancelFn()
	}
	if m.dispatcher != nil {
		m.dispatcher.Stop()
	}
	// W1: Close the decision logger to flush pending decisions and stop its goroutine.
	if eng, ok := m.workflowEngine.(*workflow.Engine); ok {
		eng.Close()
	}
	if m.subagentManager != nil {
		m.subagentManager.Shutdown(context.Background())
	}
	// Emitter drop observability: log total drops on shutdown
	if dropped := DroppedMessages(); dropped > 0 {
		slog.Warn("workflow->TUI channel drops during session", "total_dropped", dropped)
	}
}

// saveSessionOnShutdown persists messages, workflow state, and session metadata
// during graceful shutdown. Errors are logged but do not prevent shutdown.
func (m *AppState) saveSessionOnShutdown() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}

	// Persist workflow phase/goal if a workflow was active.
	if m.workflowPhase != types.PhaseIdle && m.workflowPhase != "" {
		if err := m.sessionManager.UpdateWorkflowState(
			m.sessionID,
			m.workflowGoal,
			m.workflowPhase,
			m.discussQuestions,
		); err != nil {
			slog.Warn("shutdown: failed to persist workflow state", "error", err)
		}
	}

	// Save the full session (messages + metadata) if the REPL model is available.
	if m.replModel != nil {
		sess, err := m.sessionManager.LoadSession(m.sessionID)
		if err != nil {
			slog.Warn("shutdown: failed to load session for save", "error", err)
			return
		}
		if sess != nil {
			msgs := m.replModel.Messages()
			sess.Messages = msgs
			sess.MessageCount = len(msgs)
			if m.activeProvider != "" {
				sess.Provider = m.activeProvider
			}
			if m.activeModel != nil {
				sess.Model = m.activeModel.ID
			}
			if err := m.sessionManager.SaveSession(sess); err != nil {
				slog.Warn("shutdown: failed to save session", "error", err)
			}
		}
	}
}

// addToast appends a toast with a unique ID and returns the assigned ID.
func (m *AppState) addToast(text, toastType string) int {
	m.nextToastID++
	id := m.nextToastID
	m.toasts = append(m.toasts, Toast{
		ID:        id,
		Text:      text,
		Type:      toastType,
		CreatedAt: time.Now(),
	})
	if len(m.toasts) > maxVisibleToasts+2 {
		m.toasts = m.toasts[len(m.toasts)-(maxVisibleToasts+2):]
	}
	// Eagerly create notification model so notifications are never lost
	if m.notifModel == nil {
		cw, ch := m.contentDimensions()
		m.notifModel = NewNotificationModel(m.themeManager.Current(), cw, ch)
	}
	// Also store in notification history
	m.notifModel.AddNotification(text, toastType)
	return id
}

// addToastCmd appends a toast and returns a tea.Cmd that emits ToastExpiryMsg after the duration.
func (m *AppState) addToastCmd(text, toastType string, duration time.Duration) tea.Cmd {
	id := m.addToast(text, toastType)
	if duration <= 0 {
		duration = 3 * time.Second
	}
	for i := len(m.toasts) - 1; i >= 0; i-- {
		if m.toasts[i].ID == id {
			m.toasts[i].Duration = duration
			break
		}
	}
	return tea.Tick(duration, func(time.Time) tea.Msg {
		return ToastExpiryMsg{ToastID: id}
	})
}

// removeToastByID removes a toast by its unique ID.
func (m *AppState) removeToastByID(id int) {
	for i, t := range m.toasts {
		if t.ID == id {
			m.toasts = append(m.toasts[:i], m.toasts[i+1:]...)
			return
		}
	}
}

// ─── Workflow ─────────────────────────────────────────────────────────────────

// setWorkflowPhase transitions to a new workflow phase, updating state.
func (m *AppState) setWorkflowPhase(phase types.WorkflowPhase) {
	m.workflowPhase = phase
	m.workflowPhaseIndex = phaseToIndex(phase)
	if m.replModel != nil {
		m.replModel.lastStatus = "Phase: " + string(phase)
		m.replModel.workflowPhase = string(phase)
		m.replModel.workflowPhaseIndex = m.workflowPhaseIndex
		m.replModel.totalPhases = totalPhases
	}
	if m.sidebarModel != nil {
		m.sidebarModel.SetCurrentPhase(string(phase))
	}
}

// RunPhaseCmd runs a workflow phase in a goroutine and returns a tea.Cmd.
func (m *AppState) RunPhaseCmd(phase types.WorkflowPhase) tea.Cmd {
	if m.workflowEngine == nil {
		return func() tea.Msg {
			return PhaseResultMsg{
				Phase:   phase,
				Success: false,
				Error:   "Cannot start workflow — no provider configured. Run /settings to configure a provider.",
			}
		}
	}

	ctx, cancel := context.WithCancel(m.shutdownCtx)
	m.workflowCancel = cancel

	m.checkAutoArbitrage()

	// Inline phase starting feedback in the REPL
	if m.replModel != nil {
		m.replModel.AddMessage(makeAssistantMsg(
			fmt.Sprintf("**Phase: %s** — starting…", phase),
		))
	}

	engine := m.workflowEngine
	goal := m.workflowGoal

	phaseCmd := func() tea.Msg {
		result, err := engine.RunPhase(ctx, phase, goal)
		if err != nil {
			return PhaseResultMsg{
				Phase:   phase,
				Success: false,
				Error:   err.Error(),
			}
		}
		if result == nil {
			return PhaseResultMsg{Phase: phase, Success: true}
		}
		return PhaseResultMsg{
			Phase:                   phase,
			Tasks:                   result.Tasks,
			Messages:                result.Messages,
			Success:                 result.Error == "",
			Error:                   result.Error,
			NeedsAnswers:            result.NeedsAnswers,
			RequiresManualInput:     result.RequiresManualInput,
			DurationMs:              result.DurationMs,
			Usage:                   result.Usage,
			Cost:                    result.Cost,
			ToolCalls:               result.ToolCalls,
			Commits:                 result.Commits,
			DiffStats:               result.DiffStats,
			Demonstration:           result.Demonstration,
			ManualVerificationSteps: result.ManualVerificationSteps,
			WorkflowMode:            result.WorkflowMode,
			RuntimeSummary:          result.RuntimeSummary,
		}
	}
	// Bootstrap the emitter drain chain so intermediate workflow messages
	// (TaskStartMsg, TaskUpdateMsg, ToolStartMsg, etc.) are delivered to
	// Update() during phase execution. Without this, messages accumulate
	// in the channel buffer and are never consumed.
	return tea.Batch(phaseCmd, m.drainEmitterCmd())
}

// ─── Permission/Question listeners ───────────────────────────────────────────

// permListenerCmd reads one permission request from the dispatcher and emits it.
func permListenerCmd(ctx context.Context, d *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		select {
		case req := <-d.RequestCh():
			return PermissionRequestMsg{Request: req}
		case <-ctx.Done():
			return nil
		}
	}
}

// questionListenerCmd reads one question request from the dispatcher and emits it.
func questionListenerCmd(ctx context.Context, d *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		select {
		case req := <-d.QuestionRequestCh():
			return QuestionRequestMsg{
				ID:       req.ID,
				Question: req.Question,
				Header:   req.Header,
				Options:  req.Options,
			}
		case <-ctx.Done():
			return nil
		}
	}
}

// ─── Workflow engine initialization ──────────────────────────────────────────

// initWorkflowEngine creates a new workflow.Engine for the current session.
func (m *AppState) initWorkflowEngine() tea.Cmd {
	if m.workflowEngine != nil {
		return nil
	}
	if m.registry == nil || m.activeProvider == "" {
		slog.Warn("cannot init workflow engine: no provider")
		return nil
	}

	p := m.registry.ActiveProvider()
	if p == nil {
		return nil
	}

	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}
	if m.config != nil && modelID == "" {
		modelID = m.config.Model.Default
	}

	workDir := "."
	if m.git != nil {
		workDir = m.git.WorkDir()
	}

	planningDir := filepath.Join(workDir, ".m31a")
	backupDir := filepath.Join(workDir, ".m31a", "backups")

	tokenEst := tokens.NewEstimator(modelID)

	// Create metrics collector if enabled
	metricsEnabled := m.config != nil && m.config.Features.MetricsEnabled
	if m.collector == nil && metricsEnabled && m.sessionManager != nil {
		m.collector = metrics.NewCollector(m.sessionID, m.sessionManager.BaseDir(), true)
		m.dispatcher.SetCollector(m.collector)
		// Register MetricsTool so the LLM can query session metrics
		_ = m.dispatcher.Register(tools.NewMetricsTool(m.collector))
	}

	engine, err := workflow.NewEngine(
		m.sessionID,
		workDir,
		backupDir,
		planningDir,
		p,
		modelID,
		m.dispatcher,
		tokenEst,
		m.sessionManager,
		m.config,
	)
	if err != nil {
		slog.Error("workflow engine init failed", "err", err)
		m.addToast("Workflow engine failed to initialize: "+errors.UserMessage(err), "error")
		return nil
	}

	if m.git != nil {
		engine.SetGit(m.git)
	}
	if m.ledger != nil {
		engine.SetLedger(m.ledger)
	}
	if m.collector != nil {
		engine.SetCollector(m.collector)
	}

	// Connect the MsgEmitter so workflow events reach the TUI
	// Use narrative-aware emitter to intercept and process workflow messages
	narrativeEmitter := newNarrativeEmitter(nil, &globalDropCounter, m.config)
	narrativeEmitter.inner.ch = make(chan tea.Msg, ChannelCap)
	engine.SetMsgEmitter(narrativeEmitter)
	m.emitterCh = narrativeEmitter.inner.ch
	m.narrativeEngine = narrativeEmitter.engine
	m.narrativeBridge = narrativeEmitter.bridge
	m.narrativeState = NewNarrativeState()
	m.sidebarModel.SetNarrativeState(m.narrativeState)

	// Wire TodoWrite callback to update sidebar
	m.wireTodoWriteCallback()

	m.workflowEngine = engine
	return nil
}

// wireTodoWriteCallback connects the TodoWrite tool's callback to update the sidebar.
func (m *AppState) wireTodoWriteCallback() {
	if m.dispatcher == nil {
		return
	}
	m.dispatcher.SetTodoWriteCallback(func(items []tools.TodoItem) {
		if m.emitterCh == nil {
			return
		}
		sidebarItems := make([]SidebarTodoItem, len(items))
		for i, item := range items {
			sidebarItems[i] = SidebarTodoItem{
				Content:  item.Content,
				Status:   item.Status,
				Priority: item.Priority,
				Source:   "llm",
			}
		}
		msg := SidebarTodoUpdateMsg{Items: sidebarItems}
		// Bounded retry: try up to maxRetries times with fixed backoff.
		// Never spawn unbounded goroutines.
		for attempt := 0; attempt <= maxRetries; attempt++ {
			select {
			case m.emitterCh <- msg:
				return
			default:
				if attempt < maxRetries {
					time.Sleep(retryBackoff)
				}
			}
		}
		// All retries exhausted — drop the update.
		dropped := globalDropCounter.Add(1)
		slog.Warn("TodoWrite update dropped: channel full",
			"total_dropped", dropped)
	})
}

// drainEmitterCmd returns a tea.Cmd that reads one message from the workflow
// emitter channel and forwards it to the Bubble Tea update loop.
func (m *AppState) drainEmitterCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	return func() tea.Msg {
		select {
		case msg := <-m.emitterCh:
			return msg
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// drainMultipleCmd returns a tea.Cmd that reads up to maxDrainPerTick
// messages from the emitter channel and wraps them in a batched message.
// Used during high-load periods to prevent channel saturation.
// If only one message is available, it returns it directly (no wrapping).
func (m *AppState) drainMultipleCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	return func() tea.Msg {
		// Read first message (blocking).
		select {
		case first := <-m.emitterCh:
			// Try to read additional messages non-blockingly.
			var batch []tea.Msg
			batch = append(batch, first)
		drainLoop:
			for i := 1; i < maxDrainPerTick; i++ {
				select {
				case msg := <-m.emitterCh:
					batch = append(batch, msg)
				default:
					break drainLoop
				}
			}
			if len(batch) == 1 {
				return first
			}
			return DrainBatchMsg{Messages: batch}
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// DrainBatchMsg wraps multiple emitter messages for batch processing.
type DrainBatchMsg struct {
	Messages []tea.Msg
}

// emitterLoad returns the number of messages currently queued in the
// emitter channel. Used to decide between single and batch draining.
func (m *AppState) emitterLoad() int {
	if m.emitterCh == nil {
		return 0
	}
	return len(m.emitterCh)
}

// drainAdaptiveCmd returns a drain command appropriate for the current load.
// Under low load, reads one message. Under high load (>25% capacity),
// reads up to maxDrainPerTick messages in a batch.
func (m *AppState) drainAdaptiveCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	if m.emitterLoad() > ChannelCap/4 {
		return m.drainMultipleCmd()
	}
	return m.drainEmitterCmd()
}

// drainFileWatcherCmd returns a tea.Cmd that reads one message from the file
// watcher channel and forwards it to the Bubble Tea update loop.
func (m *AppState) drainFileWatcherCmd() tea.Cmd {
	if m.fileWatcher == nil {
		return nil
	}
	return func() tea.Msg {
		select {
		case msg := <-m.fileWatcher.Events:
			return msg
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// startConfigWatcher launches a background goroutine that watches the config
// file for changes and forwards ConfigReloadMsg to the Bubble Tea update loop.
func (m *AppState) startConfigWatcher() tea.Cmd {
	if m.configPath == "" {
		return nil
	}
	ch := make(chan config.ConfigReloadMsg, 4)
	m.configWatcherStop = make(chan struct{})
	go func() {
		defer close(ch)
		defer func() {
			if r := recover(); r != nil {
				slog.Error("config watcher panic", "error", r)
			}
		}()
		config.WatchConfig(m.shutdownCtx, m.configPath, ch)
	}()
	return func() tea.Msg {
		select {
		case msg, ok := <-ch:
			if !ok {
				return nil
			}
			return msg
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// persistWorkflowState saves the current workflow state to disk.
func (m *AppState) persistWorkflowState() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}
	if err := m.sessionManager.UpdateWorkflowState(
		m.sessionID,
		m.workflowGoal,
		m.workflowPhase,
		m.discussQuestions,
	); err != nil {
		slog.Warn("failed to persist workflow state", "error", err)
		m.addToast("Failed to save workflow state", "warning")
	}
}

// checkAutoDream syncs REPL messages to the AutoDream consolidator and
// triggers consolidation automatically when the context grows large enough.
func (m *AppState) checkAutoDream() {
	if m.autoDream == nil || m.replModel == nil {
		return
	}
	msgs := m.replModel.Messages()
	m.autoDream.SetMessages(msgs)
	if !m.autoDream.CanConsolidate() {
		return
	}
	result := m.autoDream.Consolidate()
	if result.Success {
		m.replModel.SetMessages(m.autoDream.Messages())
		m.addToast(fmt.Sprintf("Auto-compressed: %d messages removed, ~%d tokens saved", result.MessagesRemoved, result.TokensSaved), "info")
	}
}

// checkAutoArbitrage evaluates whether a cheaper model should be used for the
// next workflow phase when the AutoArbitrage config flag is enabled.
func (m *AppState) checkAutoArbitrage() {
	if m.config == nil || !m.config.Model.AutoArbitrage {
		return
	}
	if m.arbitrager == nil || m.registry == nil || m.workflowEngine == nil {
		return
	}
	p := m.registry.ActiveProvider()
	if p == nil {
		return
	}
	models := p.CachedModels()
	if len(models) == 0 {
		return
	}

	task := types.Task{
		Action:      "execute",
		Description: m.workflowGoal,
	}
	threshold := m.config.Model.ArbitrageThreshold
	if threshold == 0 {
		threshold = 0.2
	}

	rec, err := arbitrage.Recommend(models, task, threshold)
	if err != nil {
		return
	}
	if m.activeModel != nil && rec.RecommendedModel.ModelID == m.activeModel.ID {
		return
	}

	// Only switch if the savings exceed the threshold (use ShouldArbitrage).
	currentCost := 0.0
	if m.activeModel != nil && len(rec.Alternatives) > 0 {
		// Find current model's cost in alternatives to compare against recommended
		for _, alt := range rec.Alternatives {
			if alt.ModelID == m.activeModel.ID {
				currentCost = alt.TotalCost
				break
			}
		}
	}
	// If active model not found in alternatives, compute its cost from pricing
	if currentCost == 0 && m.activeModel != nil {
		// Estimate tokens using the same method as arbitrage
		inputTokens := len(m.workflowGoal) / 4
		outputTokens := inputTokens * 2
		inputCost := float64(inputTokens) * (m.activeModel.Pricing.InputPerMToken / 1_000_000.0)
		outputCost := float64(outputTokens) * (m.activeModel.Pricing.OutputPerMToken / 1_000_000.0)
		currentCost = inputCost + outputCost
	}
	if currentCost == 0 {
		return // cannot determine current model cost, skip arbitrage
	}
	if !arbitrage.ShouldArbitrage(currentCost, rec.RecommendedModel.TotalCost, threshold) {
		return
	}

	recommended := rec.RecommendedModel.ModelID
	modelInfo, err := p.GetModel(recommended)
	if err != nil {
		return
	}

	m.workflowEngine.SetModel(recommended, p)
	m.activeModel = modelInfo
	m.addToast(fmt.Sprintf("Auto-arbitrage: switched to %s (%s task, saving $%.4f)", recommended, rec.Complexity, rec.Savings), "info")
}
