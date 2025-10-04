package tui

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"sync"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
)



// setWorkflowPhase keeps workflowRunning and currentPhase synchronized.
// It also invalidates the header cache so the phase indicator updates.
func (m *AppState) setWorkflowPhase(phase types.WorkflowPhase) {
	// BUG-04 fix: flush pending stream chunks when workflow ends or transitions
	if phase == types.PhaseIdle && len(m.pendingStreamChunks) > 0 {
		for _, chunk := range m.pendingStreamChunks {
			if m.replModel != nil {
				m.replModel.AppendStreamChunk(chunk)
			}
		}
		m.pendingStreamChunks = nil
	}
	m.currentPhase = phase
	m.workflowRunning = (phase != types.PhaseIdle)
	// Sync phase breadcrumb visibility with workflow running state
	m.showPhaseBreadcrumb = m.workflowRunning
	m.headerCacheValid = false
}

// Shutdown cleanly stops all background goroutines.
// CR-06: cancels config watcher and waits for it to exit.
func (m *AppState) Shutdown() {
	// W-12: stop health check ticker
	if m.healthTicker != nil {
		m.healthTicker.Stop()
	}
	// CR-07: cancel listener goroutines
	if m.shutdownCancel != nil {
		m.shutdownCancel()
	}
	// CR-06: cancel config watcher
	if m.configWatchCancel != nil {
		m.configWatchCancel()
	}
	m.configWatcherWg.Wait()
}



// currentPhaseGen returns the current phaseGen value for snapshot use by
// the workflow drainer. The drainer captures this at spawn time and
// returns nil if the value changes (a new phase started). Returns 0
// when AppState is uninitialized.
func (m *AppState) currentPhaseGen() int {
	return m.phaseGen
}

// RunPhaseCmd returns a tea.Cmd that executes the given workflow phase in a
// goroutine and emits a PhaseResultMsg on completion. It also sets up a
// MsgEmitter on the engine so that TaskStartMsg and TaskUpdateMsg are emitted
// during execution and PlanReadyMsg when the plan phase completes.
//
// D-04 fix: per-phase lifecycle synchronization. Each call:
//  1. Cancels the previous phase's context
//  2. Increments app.phaseGen so the old drainer sees the change and stops
//  3. Closes the OLD app.msgDoneCloser (via channelCloser.close) to
//  4. Creates a fresh msgCh + doneCh pair
//  5. Captures the current phaseGen for the new drainer
//  6. Sets the engine's MsgEmitter to use the new channel
//
// The runner goroutine uses `defer close(doneCh)` to signal the drainer
// when the phase completes. The drainer selects on msgCh, done, and a
// 100ms poll timer, and returns nil if app.phaseGen has changed.
func RunPhaseCmd(app *AppState, phase types.WorkflowPhase, goal string) tea.Cmd {
	// 1. Cancel any previous phase's context to stop lingering goroutines
	if app.workflowCancel != nil {
		app.workflowCancel()
	}

	// 2. Increment phase generation FIRST so any drainer from the
	//    previous phase sees the change and stops on its next check.
	app.phaseGen++

	// 3. Close the OLD app.msgDoneCloser (defensive against double-close) to
	//    signal the old drainer to stop. The old channel reference is
	//    left in place until the drainer returns; we don't nil it out
	//    because the drainer might still be reading from it.
	if app.msgDoneCloser != nil {
		app.msgDoneCloser.close()
	}

	// 4. Create new per-phase channels
	msgCh := make(chan tea.Msg, 256)
	doneCh := make(chan struct{})
	app.msgChan = msgCh
	app.msgDoneCloser = newChannelCloser(doneCh)

	// Create a cancellable context for this phase
	ctx, cancel := context.WithCancel(context.Background())
	app.workflowCtx = ctx
	app.workflowCancel = cancel

	// 5. Capture the current phaseGen for the drainer. The drainer
	//    checks this on every invocation and returns nil if it changes.
	currentGen := app.phaseGen

	eng := app.workflowEngine
	eng.SetMsgEmitter(&channelEmitter{ch: msgCh})

	// Phase runner: executes the phase, emits PlanReadyMsg if applicable,
	// then closes the done channel to signal the drainer.
	runner := func() tea.Msg {
		defer close(doneCh) // <-- signal drainer when phase completes

		result, err := eng.RunPhase(ctx, phase, goal)
		cancel() // Ensure cleanup

		if err != nil {
			return PhaseResultMsg{Phase: phase, Error: err.Error()}
		}
		if result == nil {
			return PhaseResultMsg{Phase: phase, Error: "nil result"}
		}

		// Emit PlanReadyMsg when the plan phase completes successfully.
		if phase == types.PhasePlan && result.Success && len(result.Tasks) > 0 {
			select {
			case msgCh <- PlanReadyMsg{
				Tasks:        result.Tasks,
				CostEstimate: fmt.Sprintf("%d tasks", len(result.Tasks)),
				TimeEstimate: "",
			}:
case <-time.After(types.ChannelSendTimeout):
			slog.Warn("dropped PlanReadyMsg: channel full")
			}
		}

		return PhaseResultMsg{
			Phase:               phase,
			Tasks:               result.Tasks,
			Messages:            result.Messages,
			Success:             result.Success,
			Error:               result.Error,
			NeedsAnswers:        result.NeedsAnswers,
			RequiresManualInput: result.RequiresManualInput,
			DurationMs:          result.DurationMs,
			// Wire execution metrics
			Usage:     result.Usage,
			Cost:      result.Cost,
			ToolCalls: result.ToolCalls,
			Commits:   result.Commits,
			DiffStats: result.DiffStats,
		}
	}

	// Return a batch: the runner executes the phase, the drainer reads
	// emitted messages until done is closed or a new phase starts.
	return tea.Batch(runner, workflowMsgDrainer(app, currentGen, doneCh))
}

// workflowMsgDrainer returns a tea.Cmd that reads one message from the
// workflow message channel. Captures the phaseGen at spawn time; if
// the gen changes (a new phase started), the drainer returns nil and
// stops. Uses a blocking select on msgCh and done — no polling.
func workflowMsgDrainer(app *AppState, gen int, done chan struct{}) tea.Cmd {
	return func() tea.Msg {
		// If the phase has been superseded, stop draining immediately.
		if app.phaseGen != gen {
			return nil
		}
		// If the done channel is closed, the phase is finished.
		select {
		case <-done:
			return nil
		default:
		}
		// Block until a message arrives, the phase completes, or a new phase starts.
		select {
		case msg, ok := <-app.msgChan:
			if !ok {
				return nil
			}
			return msg
		case <-done:
			return nil
		}
	}
}

// channelCloser wraps a chan struct{} with a sync.Once to guarantee
// exactly-once close semantics without a global sync.Map. Each
// channelCloser is allocated per phase in RunPhaseCmd, eliminating
// the unbounded global map that previously tracked close-once state.
type channelCloser struct {
	ch   chan struct{}
	once sync.Once
}

// newChannelCloser creates a new channelCloser wrapping ch.
func newChannelCloser(ch chan struct{}) *channelCloser {
	return &channelCloser{ch: ch}
}

// close closes the underlying channel exactly once. Returns true if
// this call performed the close, false otherwise.
func (cc *channelCloser) close() bool {
	closed := false
	cc.once.Do(func() {
		close(cc.ch)
		closed = true
	})
	return closed
}

// chan returns the underlying channel (read-only for callers that
// need to select on it).
func (cc *channelCloser) chan_() chan struct{} {
	return cc.ch
}

// channelEmitter implements workflow.MsgEmitter by sending messages into a channel.
type channelEmitter struct {
	ch chan tea.Msg
}

func (ce *channelEmitter) Emit(msg tea.Msg) {
	select {
	case ce.ch <- msg:
	case <-time.After(types.ChannelSendTimeout):
		// Channel full after timeout — drop to avoid blocking the engine.
		slog.Warn("workflow message dropped: channel full", "msg_type", fmt.Sprintf("%T", msg))
	}
}

func (m *AppState) Init() tea.Cmd {
	cmds := []tea.Cmd{permissionListenerCmd(m.shutdownCtx, m.dispatcher), questionListenerCmd(m.shutdownCtx, m.dispatcher)}
	if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
		cmds = append(cmds, HealthCheckTicker(context.Background(), types.HealthCheckInterval))
		cmds = append(cmds, CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval))
	}
	return tea.Batch(cmds...)
}

// permissionListenerCmd returns a tea.Cmd that watches the dispatcher's
// permission request channel and feeds requests into the Bubble Tea event loop.
// CR-07: accepts context for clean shutdown via select on ctx.Done().
func permissionListenerCmd(ctx context.Context, dispatcher *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		select {
		case <-ctx.Done():
			return nil
		case req := <-dispatcher.RequestCh():
			return PermissionRequestMsg{Request: req}
		}
	}
}

// questionListenerCmd returns a tea.Cmd that watches the dispatcher's
// question request channel and feeds requests into the Bubble Tea event loop.
// CR-07: accepts context for clean shutdown via select on ctx.Done().
func questionListenerCmd(ctx context.Context, dispatcher *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		select {
		case <-ctx.Done():
			return nil
		case req := <-dispatcher.QuestionRequestCh():
			return QuestionRequestMsg{
				Question:    req.Question,
				Header:      req.Header,
				Options:     req.Options,
				AllowCustom: req.AllowCustom,
				TimeoutSecs: req.TimeoutSecs,
				ResponseCh:  dispatcher.QuestionResponseCh(),
			}
		}
	}
}

// formatDurationMs converts milliseconds to a human-readable duration string.
func formatDurationMs(ms int64) string {
	d := time.Duration(ms) * time.Millisecond
	if d < time.Minute {
		return fmt.Sprintf("%ds", int(d.Seconds()))
	}
	if d < time.Hour {
		return fmt.Sprintf("%dm %ds", int(d.Minutes()), int(d.Seconds())%60)
	}
	return fmt.Sprintf("%dh %dm %ds", int(d.Hours()), int(d.Minutes())%60, int(d.Seconds())%60)
}

// cycleRecentModel cycles the active model through the recent models list.
// Direction +1 = forward (newer), -1 = backward (older).
func (m *AppState) cycleRecentModel(direction int) tea.Cmd {
	if m.sessionManager == nil || m.activeProvider == "" {
		return nil
	}

	data, err := m.sessionManager.LoadRecentModels()
	if err != nil || len(data.Recent) == 0 {
		return nil
	}

	// Find current model index in recent list
	currentIdx := -1
	currentID := ""
	if m.activeModel != nil {
		currentID = m.activeModel.ID
		for i, id := range data.Recent {
			if id == currentID {
				currentIdx = i
				break
			}
		}
	}

	// Compute target index with wrap-around
	targetIdx := 0
	if currentIdx >= 0 {
		targetIdx = currentIdx + direction
		if targetIdx < 0 {
			targetIdx = len(data.Recent) - 1
		} else if targetIdx >= len(data.Recent) {
			targetIdx = 0
		}
	}

	// Look up model from active provider
	provider, err := m.registry.Get(m.activeProvider)
	if err != nil {
		return nil
	}
	model, err := provider.GetModel(data.Recent[targetIdx])
	if err != nil {
		return nil
	}

	m.activeModel = model
	var cmd tea.Cmd
	if m.replModel != nil {
		cmd = m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID, m.config)
		m.replModel.SetDispatcher(m.dispatcher)
	}

	// Mark the model as recently used
	m.sessionManager.AddRecentModel(model.ID) //nolint:errcheck

	// Emit toast feedback
	m.toastText = fmt.Sprintf("Model: %s", model.Name)
	m.toastExpires = time.Now().Add(2 * time.Second)
	m.toastType = "info"
	return cmd
}

func calculateNextInterval(status types.HealthStatus) time.Duration {
	if status.Error != "" &&
		(strings.Contains(strings.ToLower(status.Error), "rate limit") ||
			strings.Contains(strings.ToLower(status.Error), "429")) {
		return types.MaxRetryAfterWait
	}
	if status.Status == "offline" {
		return types.MaxRetryAfterWait
	}
	return types.HealthCheckInterval
}

func currentKeyContext(screen Screen) KeyContext {
	switch screen {
	case ScreenREPL:
		return CtxREPL
	case ScreenSettings:
		return CtxSettings
	case ScreenModelSelector:
		return CtxModelSel
	case ScreenResume:
		return CtxResume
	case ScreenFirstRun:
		return CtxFirstRun
	default:
		return CtxGlobal
	}
}
