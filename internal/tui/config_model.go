package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ConfigModel renders a full read-only view of all config settings.
// Press 'e' to jump to the editable settings screen.
type ConfigModel struct {
	theme    theme.Theme
	cfg      *config.Config
	cfgPath  string
	width    int
	height   int
	content  string
	viewport viewport.Model
}

// NewConfigModel creates a ConfigModel.
func NewConfigModel(t theme.Theme, cfg *config.Config, cfgPath string, w, h int) *ConfigModel {
	vp := viewport.New(w-4, h-6)
	vp.Style = lipgloss.NewStyle().PaddingLeft(2)

	m := &ConfigModel{
		theme:    t,
		cfg:      cfg,
		cfgPath:  cfgPath,
		width:    w,
		height:   h,
		viewport: vp,
	}
	m.buildContent()
	return m
}

// buildContent renders all config sections into the content string.
func (m *ConfigModel) buildContent() {
	if m.cfg == nil {
		m.content = "Config not available."
		return
	}
	cfg := m.cfg
	var sections []string

	// Provider
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Default provider:    %s", orVal(cfg.Provider.Default)))
		lines = append(lines, fmt.Sprintf("Auto fallback:      %s", boolStr(cfg.Provider.AutoFallback)))
		lines = append(lines, fmt.Sprintf("OpenRouter base:    %s", orVal(cfg.Provider.OpenRouterBaseURL)))
		lines = append(lines, fmt.Sprintf("Zen base:           %s", orVal(cfg.Provider.ZenBaseURL)))
		lines = append(lines, fmt.Sprintf("OpenRouter referer: %s", orVal(cfg.Provider.OpenRouterReferer)))
		lines = append(lines, fmt.Sprintf("OpenRouter title:   %s", orVal(cfg.Provider.OpenRouterTitle)))
		lines = append(lines, fmt.Sprintf("OpenRouter key:     %s", keyStatus(cfg.Provider.OpenRouter.APIKey)))
		lines = append(lines, fmt.Sprintf("Zen key:            %s", keyStatus(cfg.Provider.Zen.APIKey)))
		sections = append(sections, m.renderSection("Provider", lines))
	}

	// Model
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Default model:              %s", orVal(cfg.Model.Default)))
		lines = append(lines, fmt.Sprintf("Context warning threshold:  %.0f%%", cfg.Model.ContextWarningThreshold*100))
		lines = append(lines, fmt.Sprintf("Show thinking by default:   %s", boolStr(cfg.Model.ShowThinkingByDefault)))
		lines = append(lines, fmt.Sprintf("Auto-collapse tools:        %s", boolStr(cfg.Model.AutoCollapseTools)))
		lines = append(lines, fmt.Sprintf("Auto-arbitrage:             %s", boolStr(cfg.Model.AutoArbitrage)))
		lines = append(lines, fmt.Sprintf("Arbitrage threshold:        %.2f", cfg.Model.ArbitrageThreshold))
		lines = append(lines, fmt.Sprintf("Default context length:     %d", cfg.Model.DefaultContextLength))
		lines = append(lines, fmt.Sprintf("Token EMA alpha:            %.2f", cfg.Model.TokenEMAAlpha))
		sections = append(sections, m.renderSection("Model", lines))
	}

	// UI
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Theme:                  %s", orVal(cfg.UI.Theme, "auto")))
		lines = append(lines, fmt.Sprintf("Compact mode:           %s", boolStr(cfg.UI.CompactMode)))
		lines = append(lines, fmt.Sprintf("Show token usage:       %s", boolStr(cfg.UI.ShowTokenUsage)))
		lines = append(lines, fmt.Sprintf("Show cost estimate:     %s", boolStr(cfg.UI.ShowCostEstimate)))
		lines = append(lines, fmt.Sprintf("Max iterations:         %d", cfg.UI.MaxIterations))
		lines = append(lines, fmt.Sprintf("Leader key:             %s", orVal(cfg.UI.LeaderKey)))
		lines = append(lines, fmt.Sprintf("Leader timeout (ms):    %d", cfg.UI.LeaderTimeoutMs))
		lines = append(lines, fmt.Sprintf("Sidebar width:          %d", cfg.UI.SidebarWidth))
		lines = append(lines, fmt.Sprintf("Sidebar width thresh:   %d", cfg.UI.SidebarWidthThreshold))
		lines = append(lines, fmt.Sprintf("Discuss timeout (s):    %d", cfg.UI.DiscussTimeout))
		lines = append(lines, fmt.Sprintf("Thinking max lines:     %d", cfg.UI.ThinkingMaxLines))
		lines = append(lines, fmt.Sprintf("Permission modal width: %d", cfg.UI.PermissionModalWidth))
		lines = append(lines, fmt.Sprintf("Max message history:    %d", cfg.UI.MaxMessageHistory))
		lines = append(lines, fmt.Sprintf("Fallback banner (s):    %d", cfg.UI.FallbackBannerSecs))
		lines = append(lines, fmt.Sprintf("Default log lines:      %d", cfg.UI.DefaultLogLines))
		lines = append(lines, fmt.Sprintf("Session list limit:     %d", cfg.UI.SessionListLimit))
		lines = append(lines, fmt.Sprintf("Thinking opacity:       %.1f", cfg.UI.ThinkingOpacity))
		lines = append(lines, fmt.Sprintf("Frecent history size:   %d", cfg.UI.FrecentHistorySize))
		sections = append(sections, m.renderSection("UI", lines))
	}

	// Permissions
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Default mode:     %s", orVal(cfg.Permissions.DefaultMode)))
		lines = append(lines, fmt.Sprintf("Timeout (s):      %d", cfg.Permissions.TimeoutSeconds))
		lines = append(lines, fmt.Sprintf("Rules:            %d defined", len(cfg.Permissions.Rules)))
		if len(cfg.Permissions.Agents) > 0 {
			for name := range cfg.Permissions.Agents {
				lines = append(lines, fmt.Sprintf("  Agent profile:  %s", name))
			}
		}
		sections = append(sections, m.renderSection("Permissions", lines))
	}

	// Features
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Auto backup:               %s", boolStr(cfg.Features.AutoBackup)))
		lines = append(lines, fmt.Sprintf("Resume on startup:         %s", boolStr(cfg.Features.ResumeOnStartup)))
		lines = append(lines, fmt.Sprintf("Model cache TTL (min):     %d", cfg.Features.ModelCacheTTLMinutes))
		lines = append(lines, fmt.Sprintf("Model cache stale (h):     %d", cfg.Features.ModelCacheStaleHours))
		lines = append(lines, fmt.Sprintf("Health check live (ms):    %d", cfg.Features.HealthCheckLiveMs))
		lines = append(lines, fmt.Sprintf("Health check slow (ms):    %d", cfg.Features.HealthCheckSlowMs))
		lines = append(lines, fmt.Sprintf("Session ID length:         %d", cfg.Features.SessionIDLength))
		lines = append(lines, fmt.Sprintf("Max recent models:         %d", cfg.Features.MaxRecentModels))
		lines = append(lines, fmt.Sprintf("Session retention (days):  %d", cfg.Features.SessionRetentionDays))
		lines = append(lines, fmt.Sprintf("Health check timeout (s):  %d", cfg.Features.HealthCheckTimeoutSecs))
		lines = append(lines, fmt.Sprintf("Rate limit backoff (s):    %d", cfg.Features.RateLimitBackoffSecs))
		sections = append(sections, m.renderSection("Features", lines))
	}

	// Ledger
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Enabled:      %s", boolStr(cfg.Ledger.Enabled)))
		lines = append(lines, fmt.Sprintf("Max entries:  %d", cfg.Ledger.MaxEntries))
		sections = append(sections, m.renderSection("Ledger", lines))
	}

	// Tools
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Max glob results:        %d", cfg.Tools.MaxGlobResults))
		lines = append(lines, fmt.Sprintf("Max grep results:        %d", cfg.Tools.MaxGrepResults))
		lines = append(lines, fmt.Sprintf("Bash kill grace (s):     %d", cfg.Tools.BashKillGraceSecs))
		lines = append(lines, fmt.Sprintf("Max backups per file:    %d", cfg.Tools.MaxBackupsPerFile))
		lines = append(lines, fmt.Sprintf("Webfetch max redirects:  %d", cfg.Tools.WebfetchMaxRedirects))
		lines = append(lines, fmt.Sprintf("Webfetch user agent:     %s", orVal(cfg.Tools.WebfetchUserAgent)))
		lines = append(lines, fmt.Sprintf("Skip dirs:               %s", orVal(strings.Join(cfg.Tools.SkipDirs, ", "))))
		sections = append(sections, m.renderSection("Tools", lines))
	}

	// Agents
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Default:  %s", orVal(cfg.Agents.Default)))
		lines = append(lines, fmt.Sprintf("Plan:     %s", orVal(cfg.Agents.Plan)))
		lines = append(lines, fmt.Sprintf("Execute:  %s", orVal(cfg.Agents.Execute)))
		lines = append(lines, fmt.Sprintf("Verify:   %s", orVal(cfg.Agents.Verify)))
		lines = append(lines, fmt.Sprintf("Ship:     %s", orVal(cfg.Agents.Ship)))
		lines = append(lines, fmt.Sprintf("Discuss:  %s", orVal(cfg.Agents.Discuss)))
		sections = append(sections, m.renderSection("Agents", lines))
	}

	// Git
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Commit prefix:  %s", orVal(cfg.Git.CommitPrefix)))
		lines = append(lines, fmt.Sprintf("Fix prefix:     %s", orVal(cfg.Git.FixPrefix)))
		lines = append(lines, fmt.Sprintf("Ship prefix:    %s", orVal(cfg.Git.ShipPrefix)))
		lines = append(lines, fmt.Sprintf("User name:      %s", orVal(cfg.Git.UserName)))
		lines = append(lines, fmt.Sprintf("User email:     %s", orVal(cfg.Git.UserEmail)))
		sections = append(sections, m.renderSection("Git", lines))
	}

	// Verify
	{
		var lines []string
		lines = append(lines, fmt.Sprintf("Build command:  %s", orVal(cfg.Verify.BuildCommand)))
		lines = append(lines, fmt.Sprintf("Test command:   %s", orVal(cfg.Verify.TestCommand)))
		sections = append(sections, m.renderSection("Verify", lines))
	}

	m.content = strings.Join(sections, "\n")
}

func (m *ConfigModel) renderSection(title string, lines []string) string {
	t := m.theme
	styledLines := make([]string, len(lines))
	for i, line := range lines {
		styledLines[i] = lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(4).Render(line)
	}
	header := lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).PaddingLeft(2).Render("── " + title)
	return header + "\n" + strings.Join(styledLines, "\n")
}

func (m *ConfigModel) Init() tea.Cmd {
	return nil
}

func (m *ConfigModel) Update(msg tea.Msg) (*ConfigModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return m, func() tea.Msg { return AppMsg{Screen: ScreenREPL} }
		case "e":
			// Jump to the editable settings screen
			return m, func() tea.Msg { return AppMsg{Screen: ScreenSettings} }
		}
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.viewport.Width = msg.Width - 4
		m.viewport.Height = msg.Height - 6
		m.buildContent()
	}
	var cmd tea.Cmd
	m.viewport, cmd = m.viewport.Update(msg)
	return m, cmd
}

func (m *ConfigModel) View() string {
	t := m.theme
	m.viewport.SetContent(m.content)

	header := lipgloss.NewStyle().
		Foreground(t.Text).
		Bold(true).
		Padding(0, 2).
		Render(fmt.Sprintf("Full Configuration — %s", m.cfgPath))

	footer := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("  ↑↓ scroll  e edit settings  esc/q back")

	return lipgloss.JoinVertical(lipgloss.Left,
		"",
		header,
		"",
		m.viewport.View(),
		"",
		footer,
	)
}

func orVal(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return "(not set)"
}

func keyStatus(key string) string {
	if key == "" {
		return "(not set)"
	}
	return maskedKey(key)
}
