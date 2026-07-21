package settings

import (
	"context"
	"fmt"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/integrations/keychain"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// SettingsTab identifies which settings tab is active.
type SettingsTab int

const (
	TabProvider SettingsTab = iota
	TabModel
	TabUI
	TabKeys
	TabWorkflow
	TabAbout
)

var settingsTabNames = []string{
	"Provider", "Model", "UI", "Keys", "Workflow", "About",
}

// settingsField describes one editable row on a settings tab.
type settingsField struct {
	key       string // config key identifier
	label     string // display label
	fieldType string // "choice", "bool", "text", "number", "password"
	choices   []string
}

// providerHealthStatus tracks async health check results.
type providerHealthStatus struct {
	Name   string
	Status string
	Detail string
}

type settingsHealthMsg struct {
	Results []providerHealthStatus
}

// SettingsModel manages the settings editor (6-tab layout).
type SettingsModel struct {
	theme     theme.Theme
	config    *config.Config
	registry  *provider.Registry
	keychain  keychain.Keychain
	ctx       context.Context
	activeTab SettingsTab
	width     int
	height    int

	// Field cursor
	fields      []settingsField
	fieldCursor int

	// Editing state
	editing   bool
	editField string
	editValue textinput.Model

	// Config path
	configPath string

	// Async health check results
	healthResults []providerHealthStatus
	healthLoading bool

	// Status message
	statusMsg  string
	statusTime time.Time

	// Version
	version string
}

// NewSettingsModel creates a SettingsModel.
// Accepts a context that is cancelled on app shutdown to prevent resource leaks.
func NewSettingsModel(cfg *config.Config, registry *provider.Registry, t theme.Theme, configPath string, version string, kc keychain.Keychain, ctx context.Context) *SettingsModel {
	ti := textinput.New()
	ti.CharLimit = 512
	ti.Width = 40

	s := &SettingsModel{
		theme:      t,
		config:     cfg,
		registry:   registry,
		keychain:   kc,
		ctx:        ctx,
		editValue:  ti,
		configPath: configPath,
		version:    version,
	}
	s.buildFields()
	return s
}

// buildFields rebuilds the field list for the active tab.
func (s *SettingsModel) buildFields() {
	switch s.activeTab {
	case TabProvider:
		s.fields = []settingsField{
			{key: "provider", label: "Default provider", fieldType: "choice", choices: []string{types.ProviderOpenRouter, types.ProviderZen, types.ProviderNvidia}},
			{key: "auto_fallback", label: "Auto fallback", fieldType: "bool"},
		}
	case TabModel:
		s.fields = []settingsField{
			{key: "model", label: "Default model", fieldType: "text"},
			{key: "auto_collapse", label: "Auto-collapse tools", fieldType: "bool"},
			{key: "show_thinking", label: "Show thinking by default", fieldType: "bool"},
			{key: "auto_arbitrage", label: "Auto-arbitrage", fieldType: "bool"},
			{key: "context_length", label: "Context length", fieldType: "number"},
		}
	case TabUI:
		s.fields = []settingsField{
			{key: "show_cost", label: "Show cost estimate", fieldType: "bool"},
			{key: "show_tokens", label: "Show token usage", fieldType: "bool"},
			{key: "compact_mode", label: "Compact mode", fieldType: "bool"},
			{key: "sidebar_width", label: "Sidebar width", fieldType: "number"},
			{key: "leader_key", label: "Leader key", fieldType: "text"},
			{key: "max_history", label: "Max message history", fieldType: "number"},
		}
	case TabKeys:
		s.fields = []settingsField{
			{key: "apikey_or", label: "OpenRouter API key", fieldType: "password"},
			{key: "apikey_zen", label: "Zen API key", fieldType: "password"},
			{key: "apikey_nvidia", label: "NVIDIA NIM API key", fieldType: "password"},
		}
	case TabWorkflow:
		s.fields = []settingsField{
			{key: "perm_mode", label: "Permission mode", fieldType: "choice", choices: []string{"prompt", "allow", "deny"}},
			{key: "perm_timeout", label: "Permission timeout", fieldType: "number"},
			{key: "max_iterations", label: "Max iterations", fieldType: "number"},
			{key: "auto_backup", label: "Auto backup", fieldType: "bool"},
		}
	case TabAbout:
		s.fields = nil
	}
	if s.fieldCursor >= len(s.fields) {
		s.fieldCursor = 0
	}
}

func (s *SettingsModel) SetConfig(cfg *config.Config) { s.config = cfg }
func (s *SettingsModel) SetTheme(t theme.Theme)       { s.theme = t }

func (s *SettingsModel) SetDimensions(w, h int) {
	s.width = w
	s.height = h
	s.editValue.Width = max(20, min(40, w-12))
}

func (s *SettingsModel) Init() tea.Cmd {
	return s.startHealthChecks()
}

func (s *SettingsModel) startHealthChecks() tea.Cmd {
	if s.registry == nil {
		return nil
	}
	s.healthLoading = true
	names := s.registry.List()
	s.healthResults = make([]providerHealthStatus, len(names))
	for i, name := range names {
		s.healthResults[i] = providerHealthStatus{Name: name, Status: "checking"}
	}
	reg := s.registry
	return func() tea.Msg {
		var results []providerHealthStatus
		for _, name := range names {
			p, err := reg.Get(name)
			if err != nil {
				results = append(results, providerHealthStatus{Name: name, Status: "error", Detail: err.Error()})
				continue
			}
			ctx, cancel := context.WithTimeout(s.ctx, 5*time.Second)
			status := p.HealthCheck(ctx)
			cancel()
			hs := providerHealthStatus{Name: name}
			if status.Status == "ok" {
				hs.Status = "ok"
				hs.Detail = fmt.Sprintf("%dms", status.LatencyMs)
			} else {
				hs.Status = "error"
				hs.Detail = status.Error
			}
			results = append(results, hs)
		}
		return settingsHealthMsg{Results: results}
	}
}

// Update handles settings screen key events.
func (s *SettingsModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case settingsHealthMsg:
		s.healthResults = msg.Results
		s.healthLoading = false
		return s, nil

	case tea.WindowSizeMsg:
		s.width = msg.Width
		s.height = msg.Height
		// Update input width to match terminal
		s.editValue.Width = max(20, min(40, msg.Width-12))
		return s, nil

	case tea.KeyMsg:
		if s.editing {
			return s.updateEditing(msg)
		}

		switch msg.String() {
		case "esc", "q":
			return s, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "tab":
			if len(s.fields) > 0 && s.isCurrentFieldChoice() {
				return s.cycleChoice(1)
			}
		case "shift+tab":
			if len(s.fields) > 0 && s.isCurrentFieldChoice() {
				return s.cycleChoice(-1)
			}
		case "]":
			s.activeTab = SettingsTab((int(s.activeTab) + 1) % len(settingsTabNames))
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "[":
			s.activeTab = SettingsTab((int(s.activeTab) - 1 + len(settingsTabNames)) % len(settingsTabNames))
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "up", "k":
			if s.fieldCursor > 0 {
				s.fieldCursor--
			}
		case "down", "j":
			if len(s.fields) > 0 && s.fieldCursor < len(s.fields)-1 {
				s.fieldCursor++
			}
		case "1":
			s.activeTab = TabProvider
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "2":
			s.activeTab = TabModel
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "3":
			s.activeTab = TabUI
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "4":
			s.activeTab = TabKeys
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "5":
			s.activeTab = TabWorkflow
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "6":
			s.activeTab = TabAbout
			s.fieldCursor = 0
			s.editing = false
			s.editField = ""
			s.buildFields()
		case "enter":
			return s.activateField()
		case "s":
			return s.saveConfig()
		case "L":
			return s.saveLocalConfig()
		case "r":
			if s.activeTab == TabProvider {
				return s, s.startHealthChecks()
			}
		}
	}
	return s, nil
}

func (s *SettingsModel) isCurrentFieldChoice() bool {
	if len(s.fields) == 0 || s.fieldCursor >= len(s.fields) {
		return false
	}
	return s.fields[s.fieldCursor].fieldType == "choice"
}

// cycleChoice changes the current choice field by delta.
func (s *SettingsModel) cycleChoice(delta int) (tea.Model, tea.Cmd) {
	if s.config == nil {
		return s, nil
	}
	f := s.fields[s.fieldCursor]
	current := s.getFieldValue(f)
	idx := 0
	for i, c := range f.choices {
		if c == current {
			idx = i
			break
		}
	}
	idx = (idx + delta + len(f.choices)) % len(f.choices)
	return s.setFieldValue(f, f.choices[idx])
}

// activateField starts editing or toggling the current field.
func (s *SettingsModel) activateField() (tea.Model, tea.Cmd) {
	if len(s.fields) == 0 || s.fieldCursor >= len(s.fields) {
		return s, nil
	}
	f := s.fields[s.fieldCursor]

	switch f.fieldType {
	case "bool":
		return s.toggleBool(f)
	case "choice":
		return s.cycleChoice(1)
	case "text", "number", "password":
		s.editing = true
		s.editField = f.key
		s.editValue.SetValue(s.getFieldValue(f))
		s.editValue.Focus()
		s.editValue.Placeholder = f.label
		if f.fieldType == "password" {
			s.editValue.EchoMode = textinput.EchoPassword
		} else {
			s.editValue.EchoMode = textinput.EchoNormal
		}
		return s, textinput.Blink
	}
	return s, nil
}

func (s *SettingsModel) toggleBool(f settingsField) (tea.Model, tea.Cmd) {
	current := s.getFieldValue(f)
	newVal := "no"
	if current == "no" || current == "" {
		newVal = "yes"
	}
	return s.setFieldValue(f, newVal)
}

// getFieldValue reads the current display value for a field from config.
func (s *SettingsModel) getFieldValue(f settingsField) string {
	if s.config == nil {
		return ""
	}
	switch f.key {
	case "provider":
		return s.config.Provider.Default
	case "auto_fallback":
		return boolStr(s.config.Provider.AutoFallback)
	case "model":
		return s.config.Model.Default
	case "auto_collapse":
		return boolStr(s.config.Model.AutoCollapseTools)
	case "show_thinking":
		return boolStr(s.config.Model.ShowThinkingByDefault)
	case "auto_arbitrage":
		return boolStr(s.config.Model.AutoArbitrage)
	case "context_length":
		return fmt.Sprintf("%d", s.config.Model.DefaultContextLength)
	case "show_cost":
		return boolStr(s.config.UI.ShowCostEstimate)
	case "show_tokens":
		return boolStr(s.config.UI.ShowTokenUsage)
	case "compact_mode":
		return boolStr(s.config.UI.CompactMode)
	case "sidebar_width":
		return fmt.Sprintf("%d", s.config.UI.SidebarWidth)
	case "leader_key":
		return s.config.UI.LeaderKey
	case "max_history":
		return fmt.Sprintf("%d", s.config.UI.MaxMessageHistory)
	case "apikey_or":
		return s.config.Provider.OpenRouter.APIKey
	case "apikey_zen":
		return s.config.Provider.Zen.APIKey
	case "apikey_nvidia":
		return s.config.Provider.Nvidia.APIKey
	case "perm_mode":
		return s.config.Permissions.DefaultMode
	case "perm_timeout":
		return fmt.Sprintf("%d", s.config.Permissions.TimeoutSeconds)
	case "max_iterations":
		return fmt.Sprintf("%d", s.config.UI.MaxIterations)
	case "auto_backup":
		return boolStr(s.config.Features.AutoBackup)
	}
	return ""
}

// setFieldValue writes a new value to config and applies instant reload.
func (s *SettingsModel) setFieldValue(f settingsField, val string) (tea.Model, tea.Cmd) {
	if s.config == nil {
		s.config = config.DefaultConfig()
	}
	var cmd tea.Cmd
	switch f.key {
	case "provider":
		s.config.Provider.Default = val
	case "auto_fallback":
		s.config.Provider.AutoFallback = val == "yes"
	case "model":
		s.config.Model.Default = val
	case "auto_collapse":
		s.config.Model.AutoCollapseTools = val == "yes"
	case "show_thinking":
		s.config.Model.ShowThinkingByDefault = val == "yes"
	case "auto_arbitrage":
		s.config.Model.AutoArbitrage = val == "yes"
	case "context_length":
		n, err := strconv.Atoi(val)
		if err != nil {
			s.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid integer", val)
			s.statusTime = time.Now()
			return s, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid integer", val), "error")
		}
		s.config.Model.DefaultContextLength = n
	case "show_cost":
		s.config.UI.ShowCostEstimate = val == "yes"
	case "show_tokens":
		s.config.UI.ShowTokenUsage = val == "yes"
	case "compact_mode":
		s.config.UI.CompactMode = val == "yes"
	case "sidebar_width":
		n, err := strconv.Atoi(val)
		if err != nil {
			s.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid integer", val)
			s.statusTime = time.Now()
			return s, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid integer", val), "error")
		}
		s.config.UI.SidebarWidth = n
	case "leader_key":
		s.config.UI.LeaderKey = val
	case "max_history":
		n, err := strconv.Atoi(val)
		if err != nil {
			s.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid integer", val)
			s.statusTime = time.Now()
			return s, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid integer", val), "error")
		}
		s.config.UI.MaxMessageHistory = n
	case "apikey_or":
		s.config.Provider.OpenRouter.APIKey = val
	case "apikey_zen":
		s.config.Provider.Zen.APIKey = val
	case "apikey_nvidia":
		s.config.Provider.Nvidia.APIKey = val
	case "perm_mode":
		s.config.Permissions.DefaultMode = val
	case "perm_timeout":
		n, err := strconv.Atoi(val)
		if err != nil {
			s.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid integer", val)
			s.statusTime = time.Now()
			return s, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid integer", val), "error")
		}
		s.config.Permissions.TimeoutSeconds = n
	case "max_iterations":
		n, err := strconv.Atoi(val)
		if err != nil {
			s.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid integer", val)
			s.statusTime = time.Now()
			return s, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid integer", val), "error")
		}
		s.config.UI.MaxIterations = n
	case "auto_backup":
		s.config.Features.AutoBackup = val == "yes"
	}
	s.statusMsg = fmt.Sprintf("✓ %s updated", f.label)
	s.statusTime = time.Now()
	return s, cmd
}

func (s *SettingsModel) updateEditing(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		s.editing = false
		s.editField = ""
		s.editValue.EchoMode = textinput.EchoNormal
		return s, nil
	case "enter":
		val := strings.TrimSpace(s.editValue.Value())
		s.editing = false
		s.editValue.EchoMode = textinput.EchoNormal
		if val == "" {
			return s, nil
		}
		// Find the field and set its value
		for _, f := range s.fields {
			if f.key == s.editField {
				return s.setFieldValue(f, val)
			}
		}
		return s, nil
	}
	var cmd tea.Cmd
	s.editValue, cmd = s.editValue.Update(msg)
	return s, cmd
}

func (s *SettingsModel) saveConfig() (tea.Model, tea.Cmd) {
	if s.config == nil || s.configPath == "" {
		s.statusMsg = "No config path set."
		s.statusTime = time.Now()
		return s, nil
	}
	if err := s.config.SaveWithKeychain(s.configPath, s.keychain); err != nil {
		s.statusMsg = fmt.Sprintf("Save failed: %v", err)
		s.statusTime = time.Now()
		return s, nil
	}
	s.statusMsg = "✓ Config saved to " + s.configPath
	s.statusTime = time.Now()
	return s, func() tea.Msg { return tuitypes.SettingsSavedMsg{} }
}

func (s *SettingsModel) saveLocalConfig() (tea.Model, tea.Cmd) {
	if s.config == nil {
		s.statusMsg = "No config loaded."
		s.statusTime = time.Now()
		return s, nil
	}
	localPath, err := config.LocalConfigPath()
	if err != nil {
		s.statusMsg = fmt.Sprintf("Cannot determine working directory: %v", err)
		s.statusTime = time.Now()
		return s, nil
	}
	if err := s.config.SaveProject(localPath); err != nil {
		s.statusMsg = fmt.Sprintf("Local save failed: %v", err)
		s.statusTime = time.Now()
		return s, nil
	}
	s.statusMsg = "✓ Project config saved to " + localPath
	s.statusTime = time.Now()
	return s, func() tea.Msg { return tuitypes.SettingsSavedMsg{} }
}

// View renders the settings screen content.
// Header, footer, and chrome are handled by the unified PageLayout.
func (s *SettingsModel) View() string {
	t := s.theme
	w := s.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}
	if w < 30 {
		return lipgloss.NewStyle().Foreground(t.Warning).
			Render("Terminal too narrow (need ≥30 cols)")
	}

	// Nav width: proportional to terminal, 12–20 cols
	navWidth := max(12, min(20, w/6))

	leftNav := s.renderLeftNav()
	content := s.renderTabContent()
	tabNames := []string{"Provider", "Model", "UI", "Keys", "Workflow", "About"}
	title := tabNames[s.activeTab]
	contentCard := renderSettingCard(t, title, content, max(20, w-navWidth))

	mainArea := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Width(navWidth).Padding(0, 1).Render(leftNav),
		contentCard,
	)

	status := ""
	if s.statusMsg != "" && time.Since(s.statusTime) < 5*time.Second {
		color := t.Success
		if strings.Contains(s.statusMsg, "failed") || strings.Contains(s.statusMsg, "Failed") {
			color = t.Error
		}
		status = lipgloss.NewStyle().Foreground(color).PaddingLeft(2).Render(s.statusMsg)
	}

	if s.editing {
		editBox := s.renderEditBox()
		return lipgloss.JoinVertical(lipgloss.Left, mainArea, "", editBox, "", status)
	}

	// Hint line: clarify this is a simplified view
	hint := lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).
		Render("Showing common options -- see Config editor for advanced settings")
	return lipgloss.JoinVertical(lipgloss.Left, mainArea, hint, status)
}

func (s *SettingsModel) renderTabContent() string {
	switch s.activeTab {
	case TabProvider:
		return s.renderProviderTab()
	case TabModel:
		return s.renderModelTab()
	case TabUI:
		return s.renderUITab()
	case TabKeys:
		return s.renderKeysTab()
	case TabWorkflow:
		return s.renderWorkflowTab()
	case TabAbout:
		return s.renderAboutTab()
	}
	return ""
}

// renderFieldRow renders a single settings field with cursor highlight.
func (s *SettingsModel) renderFieldRow(f settingsField, idx int) string {
	t := s.theme
	val := s.getFieldValue(f)
	selected := idx == s.fieldCursor && !s.editing

	// Cursor indicator
	cursor := "  "
	if selected {
		cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
	}

	// Label width: ideal 28, scaled down on narrow terminals
	labelW := 28
	navW := max(12, min(20, s.width/6))
	avail := s.width - navW - 10 // cursor(2) + padding(4) + value margin(4)
	if avail < labelW {
		labelW = max(10, avail/2)
	}

	labelStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Width(labelW)
	valStyle := lipgloss.NewStyle().Foreground(t.Text)

	if selected {
		labelStyle = labelStyle.Foreground(t.Text).Bold(true)
		valStyle = valStyle.Foreground(t.Brand).Bold(true)
	}

	switch f.fieldType {
	case "bool":
		icon := "○"
		color := t.TextMuted
		if val == "yes" {
			icon = "●"
			color = t.Success
		}
		if selected {
			color = t.Brand
		}
		toggle := lipgloss.NewStyle().Foreground(color).Render(icon)
		return cursor + labelStyle.Render(f.label) + toggle + " " + valStyle.Render(val)

	case "choice":
		// Render choice as [ option1 | ○ option2 | option3 ]
		var parts []string
		for _, c := range f.choices {
			if c == val {
				parts = append(parts, lipgloss.NewStyle().
					Foreground(t.Brand).Bold(true).
					Render("● "+c))
			} else {
				parts = append(parts, lipgloss.NewStyle().
					Foreground(t.TextMuted).
					Render("○ "+c))
			}
		}
		return cursor + labelStyle.Render(f.label) + strings.Join(parts, "  ")

	case "password":
		source := s.keySource(f.key)
		sourceBadge := ""
		if source != "" {
			sourceBadge = " " + lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).
				Render("["+source+"]")
		}
		return cursor + labelStyle.Render(f.label) + sourceBadge + " " + valStyle.Render(maskedKey(val))

	default:
		return cursor + labelStyle.Render(f.label) + valStyle.Render(val)
	}
}

func (s *SettingsModel) renderProviderTab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, renderSectionHeader("Provider Settings", s.width))
	lines = append(lines, "")

	for i, f := range s.fields {
		lines = append(lines, s.renderFieldRow(f, i))
	}

	lines = append(lines, "")

	// Provider health
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).PaddingLeft(2).
		Render("Provider Status:"))

	if len(s.healthResults) == 0 {
		if s.config != nil {
			for _, name := range []string{types.ProviderOpenRouter, types.ProviderZen, types.ProviderNvidia} {
				hasKey := name == types.ProviderOpenRouter && s.config.Provider.OpenRouter.APIKey != "" ||
					name == types.ProviderZen && s.config.Provider.Zen.APIKey != "" ||
					name == types.ProviderNvidia && s.config.Provider.Nvidia.APIKey != ""
				if hasKey {
					lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
						Render("● "+name+" (key set)"))
				} else {
					lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
						Render("○ "+name+" (no key)"))
				}
			}
		}
	} else {
		for _, hs := range s.healthResults {
			icon, color, label := "⟳", t.Warning, "Checking"
			switch hs.Status {
			case "ok":
				icon, color, label = "●", t.Success, "Live"
			case "error":
				icon, color, label = "○", t.Error, "Offline"
			case "slow":
				icon, color, label = "◐", t.Warning, "Slow"
			}
			detail := ""
			if hs.Detail != "" {
				detail = lipgloss.NewStyle().Foreground(t.TextMuted).Render(" — " + hs.Detail)
			}
			statusText := fmt.Sprintf("%s %s %s", icon, hs.Name, label)
			lines = append(lines, lipgloss.NewStyle().Foreground(color).PaddingLeft(4).
				Render(statusText)+detail)
		}
	}

	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
		Render("Press 'r' to refresh health checks."))

	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderModelTab() string {
	var lines []string
	lines = append(lines, "")
	lines = append(lines, renderSectionHeader("Model Settings", s.width))
	lines = append(lines, "")
	for i, f := range s.fields {
		lines = append(lines, s.renderFieldRow(f, i))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderUITab() string {
	var lines []string
	lines = append(lines, "")
	lines = append(lines, renderSectionHeader("UI Settings", s.width))
	lines = append(lines, "")
	for i, f := range s.fields {
		lines = append(lines, s.renderFieldRow(f, i))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderKeysTab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, renderSectionHeader("API Keys", s.width))
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
		Render("Keys are resolved: env var → OS keychain → config file."))
	lines = append(lines, "")
	for i, f := range s.fields {
		lines = append(lines, s.renderFieldRow(f, i))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderWorkflowTab() string {
	var lines []string
	lines = append(lines, "")
	lines = append(lines, renderSectionHeader("Workflow Settings", s.width))
	lines = append(lines, "")
	for i, f := range s.fields {
		lines = append(lines, s.renderFieldRow(f, i))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderAboutTab() string {
	t := s.theme
	versionLine := ""
	if s.version != "" {
		versionLine = lipgloss.NewStyle().Foreground(t.Brand).PaddingLeft(4).Bold(true).Render("Version: "+s.version) + "\n"
	}
	return lipgloss.JoinVertical(lipgloss.Left,
		"",
		renderSectionHeader("About M31A", s.width),
		"",
		lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(4).Bold(true).Render("M31A — Terminal AI Coding Agent"),
		versionLine,
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Module: github.com/eshanized/M31A"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Built with:"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/bubbletea  — TUI framework"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/lipgloss  — terminal styling"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/bubbles   — TUI components"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/glamour   — markdown rendering"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Config:   ~/.m31a/config.toml"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Sessions: ~/.m31a/sessions/"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Ledger:   ~/.m31a/LEDGER.md"),
	)
}

func (s *SettingsModel) renderEditBox() string {
	t := s.theme
	label := "Editing: " + s.editField
	// Edit box width: clamped to terminal, never larger than screen
	editW := min(50, max(30, s.width-8))
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 2).
		Width(editW).
		Render(lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(label),
			s.editValue.View(),
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("↵ confirm  esc cancel"),
		))
}

// keySource returns a label indicating where an API key is sourced from.
func (s *SettingsModel) keySource(fieldKey string) string {
	switch fieldKey {
	case "apikey_or":
		if os.Getenv("OPENROUTER_API_KEY") != "" {
			return "env"
		}
		if s.config != nil && s.config.Provider.OpenRouter.APIKey != "" {
			return "config"
		}
	case "apikey_zen":
		if os.Getenv("ZEN_API_KEY") != "" {
			return "env"
		}
		if s.config != nil && s.config.Provider.Zen.APIKey != "" {
			return "config"
		}
	case "apikey_nvidia":
		if os.Getenv("NVIDIA_API_KEY") != "" {
			return "env"
		}
		if s.config != nil && s.config.Provider.Nvidia.APIKey != "" {
			return "config"
		}
	}
	return ""
}

func boolStr(b bool) string {
	if b {
		return "yes"
	}
	return "no"
}

// newToastCmd returns a tea.Cmd that displays a toast notification.
func newToastCmd(text, toastType string) tea.Cmd {
	return func() tea.Msg {
		return tuitypes.ToastMsg{
			Text:     text,
			Duration: 3 * time.Second,
			Type:     toastType,
		}
	}
}
