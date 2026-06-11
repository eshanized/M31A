package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// ─── Message helpers ──────────────────────────────────────────────────────────

// makeAssistantMsg creates a standard assistant message with role, content, and segment.
func makeAssistantMsg(content string) types.Message {
	return types.Message{
		Role:    "assistant",
		Content: content,
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// makeErrorBannerMsg creates an assistant message carrying a plain-text error
// banner. Styling is applied by the message renderer (not here) so that ANSI
// escape codes never enter the markdown pipeline and get mangled.
func makeErrorBannerMsg(text string, providerName string) types.Message {
	content := text
	if providerName != "" {
		content = text + " (" + providerName + ")"
	}
	return types.Message{
		Role:    "assistant",
		Content: content,
		Segments: []types.MessageSegment{{
			Type:    "error",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// makeUserMsg creates a standard user message with proper rendering properties.
func makeUserMsg(content string) types.Message {
	return makeUserMsgWithSkip(content, false)
}

// makeUserMsgWithSkip creates a user message with an optional SkipForLLM flag.
// When skipForLLM is true, the message is displayed in the REPL but excluded
// from the LLM chat history. This prevents double-sending when sendChatMessage
// replaces the display message with an enriched (or plain) version.
func makeUserMsgWithSkip(content string, skipForLLM bool) types.Message {
	return types.Message{
		Role:       "user",
		Content:    content,
		SkipForLLM: skipForLLM,
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// ─── AppState session helpers ─────────────────────────────────────────────────

// loadAndRestoreSession loads a session from disk and restores it to the REPL.
func (m *AppState) loadAndRestoreSession(sessionID string, clearExisting bool) tea.Cmd {
	return func() tea.Msg {
		if m.sessionManager == nil {
			return ErrorMsg{Err: fmt.Errorf("session manager not initialized")}
		}
		sess, err := m.sessionManager.LoadSession(sessionID)
		if err != nil {
			return ErrorMsg{Err: fmt.Errorf("failed to load session: %w", err)}
		}
		return sessionRestoredMsg{sess: sess, clearExisting: clearExisting}
	}
}

// sessionRestoredMsg carries a restored session from loadAndRestoreSession.
type sessionRestoredMsg struct {
	sess          *session.Session
	clearExisting bool
}

// ensureSidebarModel creates the sidebar model if not yet initialized.
func (m *AppState) ensureSidebarModel() {
	if m.sidebarModel == nil {
		m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
		m.sidebarModel.SetHeight(m.height)
	}
}

// propagateSessionID propagates the session ID to all workflow sub-models.
func (m *AppState) propagateSessionID(id string) {
	if m.planModel != nil {
		m.planModel.sessionID = id
	}
	if m.executeModel != nil {
		m.executeModel.sessionID = id
	}
	if m.verifyModel != nil {
		m.verifyModel.sessionID = id
	}
	if m.shipModel != nil {
		m.shipModel.sessionID = id
	}
	if m.sidebarModel != nil {
		m.sidebarModel.SetSessionID(id)
	}
	if m.workflowEngine != nil {
		m.workflowEngine.SetSessionID(id)
	}
	if m.dispatcher != nil {
		m.dispatcher.SetSessionID(id)
	}
}

// applySessionRestored applies a sessionRestoredMsg to the REPL.
func (m *AppState) applySessionRestored(msg sessionRestoredMsg) tea.Cmd {
	sess := msg.sess
	m.ensureReplModel()
	providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
	m.replModel.SetDispatcher(m.dispatcher)
	m.replModel.SetSessionID(sess.ID)
	m.sessionID = sess.ID

	if msg.clearExisting {
		m.replModel.ClearMessages()
		m.replModel.SetCommandRegistry(m.cmdRegistry)
	}

	for _, message := range sess.Messages {
		m.replModel.AddMessage(message)
	}
	m.propagateSessionID(sess.ID)
	m.screen = ScreenREPL

	return providerCmd
}

// ─── ReplModel helpers ────────────────────────────────────────────────────────

// updateSlashSuggestions updates slash command autocomplete based on current input.
func (m *ReplModel) updateSlashSuggestions() {
	if m.cmdRegistry == nil {
		return
	}

	current := m.textarea.Value()
	if !strings.HasPrefix(current, "/") {
		m.slashVisible = false
		m.slashSuggestions = nil
		return
	}

	parts := strings.Fields(current)
	partial := ""
	if len(parts) > 0 {
		partial = strings.TrimPrefix(parts[0], "/")
	}

	allCmds := m.cmdRegistry.AllCommands()
	m.slashSuggestions = nil

	if partial == "" {
		m.slashSuggestions = allCmds
	} else {
		q := strings.ToLower(partial)
		for _, cmd := range allCmds {
			name := strings.ToLower(cmd.Name)
			slash := strings.ToLower(strings.TrimPrefix(cmd.Slash, "/"))
			if strings.HasPrefix(slash, q) || strings.HasPrefix(name, q) || strings.Contains(name, q) {
				m.slashSuggestions = append(m.slashSuggestions, cmd)
			}
		}
	}

	if len(m.slashSuggestions) > 0 {
		m.slashVisible = true
		m.slashSelected = 0
		if len(m.slashSuggestions) > 8 {
			m.slashSuggestions = m.slashSuggestions[:8]
		}
	} else {
		m.slashVisible = false
	}
}

// ─── Layout / rendering utilities ─────────────────────────────────────────────

// centerScreen centers content both horizontally and vertically in the terminal.
func centerScreen(content string, w, h int) string {
	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}

// renderSectionHeader renders a titled divider line.
func renderSectionHeader(title string, width int) string {
	prefix := fmt.Sprintf("── %s ", title)
	remaining := width - lipgloss.Width(prefix)
	if remaining < 0 {
		remaining = 0
	}
	return prefix + strings.Repeat("─", remaining)
}

// renderLoading renders a branded loading indicator with a spinner and label,
// centered within the given dimensions.
func renderLoading(label string, w, h int, t theme.Theme) string {
	spinner := t.Spinner.Render("⠋")
	text := lipgloss.NewStyle().Foreground(t.TextMuted).Render(label)
	content := spinner + " " + text
	if w < 1 {
		w = 40
	}
	if h < 1 {
		h = 3
	}
	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}

// ─── Formatting utilities ──────────────────────────────────────────────────────

// formatSI formats an integer with SI suffix (K, M).
func formatSI(n int) string {
	switch {
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fM", float64(n)/1_000_000)
	case n >= 1_000:
		return fmt.Sprintf("%.0fK", float64(n)/1_000)
	default:
		return fmt.Sprintf("%d", n)
	}
}

// formatDurationMs formats a duration in milliseconds as a human-readable string.
func formatDurationMs(ms int64) string {
	if ms < 0 {
		return "0s"
	}
	s := ms / 1000
	m := s / 60
	h := m / 60
	switch {
	case h > 0:
		return fmt.Sprintf("%dh%dm", h, m%60)
	case m > 0:
		return fmt.Sprintf("%dm%ds", m, s%60)
	default:
		return fmt.Sprintf("%ds", s)
	}
}

// ProviderShortName returns a short display name for a provider.
func ProviderShortName(name string) string {
	switch strings.ToLower(name) {
	case "openrouter":
		return "OR"
	case "zen", "zen-gateway":
		return "Zen"
	case "openai":
		return "OAI"
	case "anthropic":
		return "AC"
	default:
		if len(name) > 4 {
			return name[:4]
		}
		return name
	}
}

// ─── Infrastructure command factories ──────────────────────────────────────────

// HealthCheckTicker returns a tea.Cmd that emits a HealthCheckTickMsg after the
// given duration. The app re-schedules it in response to HealthCheckResultMsg.
func HealthCheckTicker(ctx context.Context, d time.Duration) tea.Cmd {
	return tea.Tick(d, func(t time.Time) tea.Msg {
		return HealthCheckTickMsg{Time: t}
	})
}

// NextHealthTick returns a tea.Cmd for the next health check tick.
func NextHealthTick(ctx context.Context, d time.Duration) tea.Cmd {
	return HealthCheckTicker(ctx, d)
}

// HealthCheckCmd runs a health check against the given provider in a goroutine
// and emits a HealthCheckResultMsg when it completes.
func HealthCheckCmd(ctx context.Context, p provider.LLMProvider, timeout time.Duration) tea.Cmd {
	return func() tea.Msg {
		if timeout <= 0 {
			timeout = types.HealthCheckInterval
		}
		hCtx, cancel := context.WithTimeout(ctx, timeout)
		defer cancel()
		status := p.HealthCheck(hCtx)
		return HealthCheckResultMsg{Result: status}
	}
}

// SidebarRefreshTicker returns a tea.Cmd that emits a SidebarRefreshTickMsg after the
// given duration. The sidebar re-schedules it in response to the tick.
func SidebarRefreshTicker(ctx context.Context, d time.Duration) tea.Cmd {
	return tea.Tick(d, func(time.Time) tea.Msg {
		return SidebarRefreshTickMsg{}
	})
}

// NextSidebarRefreshTick returns a tea.Cmd for the next sidebar refresh tick.
func NextSidebarRefreshTick(ctx context.Context, d time.Duration) tea.Cmd {
	return SidebarRefreshTicker(ctx, d)
}

// CacheRefreshTicker returns a tea.Cmd that emits a RefreshCacheMsg after d.
func CacheRefreshTicker(providerName string, d time.Duration) tea.Cmd {
	return tea.Tick(d, func(time.Time) tea.Msg {
		return RefreshCacheMsg{ProviderName: providerName}
	})
}

// NextCacheRefreshTick returns a tea.Cmd for the next cache refresh tick.
func NextCacheRefreshTick(d time.Duration) tea.Cmd {
	return tea.Tick(d, func(time.Time) tea.Msg {
		return RefreshCacheMsg{}
	})
}

// CacheRefreshCmd runs FetchModels in a goroutine and emits CacheRefreshResultMsg.
func CacheRefreshCmd(ctx context.Context, registry *provider.Registry, providerName string) tea.Cmd {
	return func() tea.Msg {
		if registry == nil {
			return CacheRefreshResultMsg{ErrMsg: "no registry"}
		}
		p, err := registry.Get(providerName)
		if err != nil {
			return CacheRefreshResultMsg{ErrMsg: err.Error()}
		}
		fetchCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		defer cancel()
		if _, err := p.FetchModels(fetchCtx); err != nil {
			return CacheRefreshResultMsg{
				ErrMsg:  err.Error(),
				NextCmd: NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
			}
		}
		return CacheRefreshResultMsg{
			NextCmd: NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
		}
	}
}
