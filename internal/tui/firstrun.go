package tui

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

type FirstRunState int

const (
	FirstRunWelcome FirstRunState = iota
	FirstRunProviderSelect
	FirstRunKeyInput
	FirstRunValidating
	FirstRunKeychainPrompt
	FirstRunComplete
)

type validationResultMsg struct {
	valid bool
	err   string
}

type FirstRunModel struct {
	state         FirstRunState
	theme         theme.Theme
	cursor        int
	providers     []string
	apiKeyInput   textinput.Model
	apiKeyValue   string
	validating    bool
	validationErr string
	statusMsg     string
	width         int
	height        int
	configPath    string
}

func NewFirstRunModel(t theme.Theme, configPath string) FirstRunModel {
	ti := textinput.New()
	ti.Placeholder = "sk-or-v1-..."
	ti.EchoMode = textinput.EchoPassword
	ti.Focus()
	ti.Width = 60
	ti.CharLimit = 128

	return FirstRunModel{
		state:       FirstRunWelcome,
		theme:       t,
		providers:   make([]string, 0),
		apiKeyInput: ti,
		configPath:  configPath,
	}
}

func (m *FirstRunModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c":
			// H-7 fix: return tea.Quit so user can exit first-run
			return []tea.Cmd{tea.Quit}, nil
		}
	}

	switch m.state {
	case FirstRunWelcome:
		return m.updateWelcome(msg)
	case FirstRunProviderSelect:
		return m.updateProviderSelect(msg)
	case FirstRunKeyInput:
		return m.updateKeyInput(msg)
	case FirstRunValidating:
		return m.updateValidating(msg)
	case FirstRunKeychainPrompt:
		return m.updateKeychainPrompt(msg)
	case FirstRunComplete:
		return m.updateComplete(msg)
	}

	return nil, nil
}

func (m *FirstRunModel) updateWelcome(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "enter", " ":
			m.state = FirstRunProviderSelect
		}
	}
	return nil, nil
}

func (m *FirstRunModel) updateProviderSelect(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "up":
			// 4 hardcoded options, navigate between them
			if m.cursor > 0 {
				m.cursor--
			} else {
				m.cursor = 3 // wrap to last
			}
		case "down":
			if m.cursor < 3 {
				m.cursor++
			} else {
				m.cursor = 0 // wrap to first
			}
		case "1":
			m.cursor = 0
			return nil, m.selectOption()
		case "2":
			m.cursor = 1
			return nil, m.selectOption()
		case "3":
			m.cursor = 2
			return nil, m.selectOption()
		case "4", "s":
			m.cursor = 3
			return nil, m.selectOption()
		case "enter":
			return nil, m.selectOption()
		}
	}
	return nil, nil
}

func (m *FirstRunModel) selectOption() *AppMsg {
	switch m.cursor {
	case 0:
		m.providers = []string{"openrouter"}
		m.state = FirstRunKeyInput
		m.apiKeyInput.Reset()
		m.apiKeyInput.Focus()
	case 1:
		m.providers = []string{"zen"}
		m.state = FirstRunKeyInput
		m.apiKeyInput.Reset()
		m.apiKeyInput.Focus()
	case 2:
		m.providers = []string{"openrouter", "zen"}
		m.state = FirstRunKeyInput
		m.apiKeyInput.Reset()
		m.apiKeyInput.Focus()
	case 3:
		m.statusMsg = "No API key configured"
		m.state = FirstRunComplete
		return &AppMsg{Screen: ScreenREPL}
	}
	return nil
}

func (m *FirstRunModel) updateKeyInput(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "esc":
			m.state = FirstRunProviderSelect
			m.validationErr = ""
			return nil, nil
		case "enter":
			input := strings.TrimSpace(m.apiKeyInput.Value())
			if input == "" {
				return nil, nil
			}
			m.apiKeyValue = input
			m.state = FirstRunValidating
			m.validating = true
			return []tea.Cmd{
				func() tea.Msg {
					if len(input) < 10 {
						return validationResultMsg{valid: false, err: "API key too short"}
					}
					// Validate key by making a test request
					for _, provider := range m.providers {
						if err := validateAPIKey(provider, input); err != nil {
							return validationResultMsg{valid: false, err: fmt.Sprintf("%s: %v", provider, err)}
						}
					}
					return validationResultMsg{valid: true}
				},
			}, nil
		}
	}

	var cmd tea.Cmd
	m.apiKeyInput, cmd = m.apiKeyInput.Update(msg)
	return []tea.Cmd{cmd}, nil
}

func (m *FirstRunModel) updateValidating(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case validationResultMsg:
		m.validating = false
		if msg.valid {
			m.state = FirstRunKeychainPrompt
		} else {
			m.validationErr = msg.err
			m.state = FirstRunKeyInput
			m.apiKeyInput.Focus()
		}
	}
	return nil, nil
}

func (m *FirstRunModel) updateKeychainPrompt(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "y", "Y", "enter":
			m.state = FirstRunComplete
			return nil, &AppMsg{Screen: ScreenREPL, SaveKeychain: true}
		case "n", "N":
			m.state = FirstRunComplete
			return nil, &AppMsg{Screen: ScreenREPL, SaveKeychain: false}
		}
	}
	return nil, nil
}

func (m *FirstRunModel) updateComplete(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	// H-6 fix: make this a no-op after the first call. Previously, every
	// message (resize, keystroke, etc.) would emit AppMsg{Screen: ScreenREPL},
	// causing repeated screen transitions and potential duplicate session creation.
	return nil, nil
}

func (m *FirstRunModel) State() FirstRunState {
	return m.state
}

func (m *FirstRunModel) SelectedProviders() []string {
	return m.providers
}

func (m *FirstRunModel) APIKey() string {
	return m.apiKeyValue
}

func (m *FirstRunModel) SetTheme(t theme.Theme) {
	m.theme = t
}

func (m *FirstRunModel) View() string {
	switch m.state {
	case FirstRunWelcome:
		return m.viewWelcome()
	case FirstRunProviderSelect:
		return m.viewProviderSelect()
	case FirstRunKeyInput:
		return m.viewKeyInput()
	case FirstRunValidating:
		return m.viewValidating()
	case FirstRunKeychainPrompt:
		return m.viewKeychainPrompt()
	case FirstRunComplete:
		return m.viewComplete()
	}
	return ""
}

func (m *FirstRunModel) viewWelcome() string {
	// ASCII art logo (same as REPL)
	logo := m.renderWelcomeLogo()

	// Feature preview cards
	features := m.renderFeatureCards()

	// Keyboard shortcuts preview
	shortcuts := m.renderShortcutsPreview()

	// Prompt card
	promptCard := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Brand).
		Padding(1, 3).
		Render(
			lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true).Render("Press Enter to begin setup"),
		)

	// Footer
	footer := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("M31A v0.1.0 — MIT License")

	// Stack: Logo → Features → Prompt → Shortcuts → Footer
	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		features,
		"",
		promptCard,
		"",
		shortcuts,
		"",
		footer,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

// renderWelcomeLogo renders the M31A ASCII art logo.
func (m *FirstRunModel) renderWelcomeLogo() string {
	banner := `░███     ░███  ░██████    ░██      ░███    ░██
░████   ░████ ░██   ░██ ░████     ░██░██   ░██
░██░██ ░██░██       ░██   ░██    ░██  ░██  ░██
░██ ░████ ░██   ░█████    ░██   ░█████████ ░██
░██  ░██  ░██       ░██   ░██   ░██    ░██ ░██
░██       ░██ ░██   ░██   ░██   ░██    ░██ ░██
░██       ░██  ░██████  ░██████ ░██    ░██ ░██
                                            
                                            `

	lines := strings.Split(banner, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(m.theme.Brand).Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
}

// renderFeatureCards shows what M31A can do.
func (m *FirstRunModel) renderFeatureCards() string {
	features := []struct {
		icon  string
		title string
		desc  string
	}{
		{"🤖", "AI-Powered", "Natural language to code"},
		{"⚡", "Fast Execution", "Parallel task execution"},
		{"🔄", "Self-Healing", "Auto-fixes failed tasks"},
		{"⚙", "Git Integrated", "Auto-commits your work"},
	}

	cards := make([]string, len(features))
	for i, f := range features {
		cardStyle := lipgloss.NewStyle().
			Background(m.theme.Surface).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Border).
			Padding(0, 2).
			Width(24)

		iconStyle := lipgloss.NewStyle().Foreground(m.theme.Brand)
		titleStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true)
		descStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

		content := lipgloss.JoinVertical(lipgloss.Center,
			iconStyle.Render(f.icon),
			titleStyle.Render(f.title),
			descStyle.Render(f.desc),
		)
		cards[i] = cardStyle.Render(content)
	}

	return lipgloss.JoinHorizontal(lipgloss.Top, cards...)
}

// renderShortcutsPreview shows key keyboard shortcuts.
func (m *FirstRunModel) renderShortcutsPreview() string {
	shortcuts := []struct {
		key   string
		label string
	}{
		{"Ctrl+P", "Commands"},
		{"Ctrl+B", "Sidebar"},
		{"Ctrl+X", "Leader Key"},
		{"/help", "Help"},
	}

	parts := make([]string, len(shortcuts))
	for i, s := range shortcuts {
		keyStyle := lipgloss.NewStyle().
			Background(m.theme.SurfaceElevated).
			Foreground(m.theme.Brand).
			Padding(0, 1).
			Bold(true)

		labelStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

		parts[i] = keyStyle.Render(s.key) + " " + labelStyle.Render(s.label)
	}

	return lipgloss.JoinHorizontal(lipgloss.Center, parts...)
}

func (m *FirstRunModel) viewProviderSelect() string {
	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.Brand).
		Render("Select AI Provider(s)")

	subtitle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Choose which AI provider(s) you want to configure")

	type provider struct {
		key       string
		icon      string
		name      string
		desc      string
		keyFormat string
	}

	providers := []provider{
		{key: "1", icon: "◆", name: "OpenRouter", desc: "100+ models, pay-per-use", keyFormat: "sk-or-v1-..."},
		{key: "2", icon: "◈", name: "Zen", desc: "Fast inference, competitive pricing", keyFormat: "zk-..."},
		{key: "3", icon: "◆◈", name: "Both", desc: "OpenRouter + Zen (auto-fallback)", keyFormat: ""},
		{key: "4", icon: "○", name: "Skip", desc: "Configure later via /settings", keyFormat: ""},
	}

	var lines []string
	for i, p := range providers {
		marker := "  "
		if i == m.cursor {
			marker = lipgloss.NewStyle().Foreground(m.theme.Brand).Render("▶ ")
		}

		keyStyle := lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)
		iconStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
		nameStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true)
		descStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

		line := fmt.Sprintf("%s%s %s  %s  %s",
			marker,
			keyStyle.Render("["+p.key+"]"),
			iconStyle.Render(p.icon),
			nameStyle.Render(p.name),
			descStyle.Render(p.desc),
		)

		if p.keyFormat != "" {
			formatStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("  (" + p.keyFormat + ")")
			line += formatStyle
		}

		lines = append(lines, line)
	}

	body := strings.Join(lines, "\n")

	// Keyboard hints in a styled box
	hints := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Padding(0, 2).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Foreground(m.theme.TextSecondary).
		Render("↑↓ navigate  ·  Enter select  ·  1-4 jump")

	content := lipgloss.JoinVertical(lipgloss.Center,
		header,
		subtitle,
		"",
		body,
		"",
		hints,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewKeyInput() string {
	var parts []string

	// Header with icon
	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.Brand).
		Render("🔑 Enter your API key")

	parts = append(parts, header)

	// Provider info badge
	providerInfo := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Foreground(m.theme.TextSecondary).
		Padding(0, 2).
		Render(fmt.Sprintf("Configuring: %s", strings.Join(m.providers, " + ")))

	parts = append(parts, providerInfo)
	parts = append(parts, "")

	// API key input in a bordered container
	inputContainer := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(1, 2).
		Render(m.apiKeyInput.View())

	parts = append(parts, inputContainer)

	// Validation error if present
	if m.validationErr != "" {
		errBox := lipgloss.NewStyle().
			Background(m.theme.Surface).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Error).
			Padding(0, 2).
			Foreground(m.theme.Error).
			Render(" " + m.validationErr)
		parts = append(parts, "")
		parts = append(parts, errBox)
	}

	// Keyboard hints
	footer := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Enter to confirm  ·  Esc to go back")

	parts = append(parts, "")
	parts = append(parts, footer)

	content := lipgloss.JoinVertical(lipgloss.Center, parts...)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewValidating() string {
	spinner := m.theme.Spinner.Render("⟳")
	text := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Bold(true).
		Render("Validating API key...")

	subtitle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("This may take a few seconds")

	content := lipgloss.JoinVertical(lipgloss.Center,
		spinner,
		"",
		text,
		subtitle,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewKeychainPrompt() string {
	title := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Bold(true).
		Render(" Store API key in system keychain?")

	desc := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("This keeps your key secure and avoids re-entering it")

	yes := lipgloss.NewStyle().
		Background(m.theme.Success).
		Foreground(lipgloss.Color("#000000")).
		Bold(true).
		Padding(0, 2).
		Render("Y / Enter = Yes")

	no := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Foreground(m.theme.TextSecondary).
		Padding(0, 2).
		Render("N = No")

	content := lipgloss.JoinVertical(lipgloss.Center,
		title,
		"",
		desc,
		"",
		lipgloss.JoinHorizontal(lipgloss.Center, yes, "   ", no),
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewComplete() string {
	checkmark := lipgloss.NewStyle().
		Foreground(m.theme.Success).
		Bold(true).
		Render(" Setup Complete!")

	text := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Render("Launching M31A...")

	content := lipgloss.JoinVertical(lipgloss.Center,
		checkmark,
		"",
		text,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

// validateAPIKey makes a test HTTP request to verify the API key works.
func validateAPIKey(provider, key string) error {
	client := &http.Client{
		Timeout: 10 * time.Second,
		Transport: &http.Transport{
			DialContext: (&net.Dialer{Timeout: 5 * time.Second}).DialContext,
		},
	}

	var url string
	switch provider {
	case "openrouter":
		url = "https://openrouter.ai/api/v1/auth/key"
	case "zen":
		url = "https://opencode.ai/zen/v1/models"
	default:
		return fmt.Errorf("unknown provider: %s", provider)
	}

	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, url, nil)
	if err != nil {
		return fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+key)
	req.Header.Set("User-Agent", "M31A/dev")
	if provider == "openrouter" {
		req.Header.Set("HTTP-Referer", "https://github.com/eshanized/M31A")
		req.Header.Set("X-Title", "M31A")
	}

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		if resp.StatusCode == http.StatusUnauthorized {
			return fmt.Errorf("invalid API key")
		}
		return fmt.Errorf("server returned status %d", resp.StatusCode)
	}
	return nil
}
