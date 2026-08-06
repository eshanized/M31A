package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

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

// renderEmptyState renders a centered empty state with a title and hint,
// telling the user why the screen is empty and what to do about it.
// Uses the EmptyState component for consistent styling.
func renderEmptyState(title, hint string, w, h int, t theme.Theme) string {
	if w < 1 {
		w = 40
	}
	if h < 1 {
		h = 3
	}

	// Use the EmptyState component for consistent styling
	es := components.EmptyState{
		Icon:     "◇",
		Title:    title,
		Subtitle: hint,
		Theme:    t,
		Width:    w,
		Height:   h,
	}
	return es.Render()
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

// EmitterDropLogInterval is how often to log the emitter drop counter (30s).
const EmitterDropLogInterval = 30 * time.Second

// EmitterDropLogTick returns a tea.Cmd that emits EmitterDropLogTickMsg
// periodically to log the drop counter if any drops have occurred.
func EmitterDropLogTick(ctx context.Context) tea.Cmd {
	return tea.Tick(EmitterDropLogInterval, func(time.Time) tea.Msg {
		return EmitterDropLogTickMsg{}
	})
}

// NextCacheRefreshTick returns a tea.Cmd for the next cache refresh tick.
func NextCacheRefreshTick(d time.Duration) tea.Cmd {
	return tea.Tick(d, func(time.Time) tea.Msg {
		return RefreshCacheMsg{}
	})
}

// CacheRefreshCmd runs FetchModels in a goroutine and emits CacheRefreshResultMsg.
func CacheRefreshCmd(ctx context.Context, registry provider.RegistryInterface, providerName string) tea.Cmd {
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
