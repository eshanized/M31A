package tui

import (
	"context"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// SetTheme updates the theme and reinitializes the message renderer.
func (m *ReplModel) SetTheme(t theme.Theme) {
	m.theme = t
	if m.msgRenderer != nil {
		newRenderer, err := components.NewMessageRenderer(t, m.width-4)
		if err == nil {
			m.msgRenderer = newRenderer
		}
	}
}

// SetSidebarWidth updates the reserved width for the sidebar and recalculates
// the REPL's internal widths. Call this when the sidebar is shown/hidden.
func (m *ReplModel) SetSidebarWidth(sw int) {
	m.sidebarWidth = sw
	// Recalculate layout with current window dimensions
	replWidth := m.width - sw
	if replWidth < 20 {
		replWidth = 20
	}
	m.viewport.Width = replWidth
	if m.msgRenderer != nil {
		_ = m.msgRenderer.SetWidth(replWidth - 4)
	}
	m.textarea.SetWidth(replWidth)
}

// SetProvider configures the active provider and returns a tea.Cmd that
// asynchronously validates the model by fetching the provider's model catalog.
// The returned cmd performs FetchModels in the background and emits a
// ProviderModelsFetchedMsg when complete, keeping the TUI responsive.
func (m *ReplModel) SetProvider(registry *provider.Registry, activeProvider string, model *types.ModelInfo, sessionID string, cfg *config.Config) tea.Cmd {
	m.registry = registry
	m.activeProvider = activeProvider
	m.sessionID = sessionID
	m.cfg = cfg
	m.modelValid = true

	if model == nil || registry == nil {
		m.activeModel = model
		return nil
	}

	m.activeModel = model

	// Return an async command that validates the model against the provider's catalog
	p := registry.ActiveProvider()
	if p == nil {
		return nil
	}
	return func() tea.Msg {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		models, err := p.FetchModels(ctx)
		return ProviderModelsFetchedMsg{Models: models, Model: model, Err: err}
	}
}

// handleProviderModelsFetched processes the async result of SetProvider's FetchModels call.
func (m *ReplModel) handleProviderModelsFetched(msg ProviderModelsFetchedMsg) {
	if msg.Err != nil || msg.Model == nil {
		return
	}
	found := false
	for _, model := range msg.Models {
		if model.ID == msg.Model.ID {
			found = true
			break
		}
	}
	if found {
		// Refresh cached model info (pricing, context length may differ between providers)
		if m.registry != nil {
			p := m.registry.ActiveProvider()
			if p != nil {
				if info, _ := p.GetModel(msg.Model.ID); info != nil {
					m.activeModel = info
					return
				}
			}
		}
	}
	// Model not found on new provider
	m.modelValid = false
}

func (m *ReplModel) SetDispatcher(d *tools.Dispatcher) {
	m.dispatcher = d
}

func (m *ReplModel) SetCommandRegistry(reg *CommandRegistry) {
	m.cmdRegistry = reg
}

// SetFrecentHistory sets the frecency history for prompt history navigation.
func (m *ReplModel) SetFrecentHistory(fh *FrecentHistory) {
	m.frecentHistory = fh
}

// SetCwd sets the working directory for @filepath resolution.
func (m *ReplModel) SetCwd(cwd string) {
	m.cwd = cwd
}

func (m *ReplModel) SetKeyRegistry(kr *KeyRegistry) {
	m.keyRegistry = kr
}

// SetSessionSparkline updates the recent-activity sparkline shown in the
// provider card on the welcome screen. Passing an empty string hides it.
func (m *ReplModel) SetSessionSparkline(spark string) {
	m.sessionSparkline = spark
}

func (m *ReplModel) SetLastActivity(t time.Time) {
	m.lastActivity = t
}

func (m *ReplModel) SetStreaming(v bool) {
	m.streaming = v
}

func (m *ReplModel) SetThinking(v bool) {
	m.thinking = v
}

// SetSessionID updates the session ID for this REPL model.
func (m *ReplModel) SetSessionID(id string) {
	m.sessionID = id
}

// AddMessage adds a message to the REPL and triggers a re-render.
func (m *ReplModel) AddMessage(msg types.Message) {
	m.messages = append(m.messages, msg)
	if len(m.messages) > MaxMessageHistory {
		m.messages = m.messages[len(m.messages)-500:]
	}
	m.renderMessages()
	m.autoScrollConditionally()
}

// InputValue returns the trimmed current textarea input.
func (m *ReplModel) InputValue() string {
	return strings.TrimSpace(m.textarea.Value())
}

// Messages returns all messages in the REPL.
func (m *ReplModel) Messages() []types.Message {
	return m.messages
}

// ClearMessages removes all messages from the REPL model and re-renders the viewport.
// M-38 fix: also resets streaming state so /clear works during active streams.
func (m *ReplModel) ClearMessages() {
	m.messages = nil
	// M-38 fix: reset streaming state
	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.streamContent.Reset()
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	m.toolCards = make(map[int]*components.ToolCard)
	m.renderMessages()
	m.viewport.GotoBottom()
	m.userScrolled = false
}

// RefreshViewport forces a re-render of the viewport content.
// Call this after provider/model changes to update the welcome screen.
func (m *ReplModel) RefreshViewport() {
	m.renderMessages()
}

func (m *ReplModel) SpinnerTick() tea.Cmd {
	return m.spinner.Tick
}

func (m *ReplModel) GetStatusText() string {
	if m.streaming {
		return "Streaming..."
	}
	if m.thinking {
		return "Thinking..."
	}
	if m.lastStatus != "" {
		return m.lastStatus
	}
	return ""
}

// LastUsage returns the usage from the last completed stream.
func (m *ReplModel) LastUsage() *types.Usage {
	return m.lastUsage
}

// LastCost returns the estimated cost of the last completed stream.
func (m *ReplModel) LastCost() float64 {
	return m.lastCost
}
