package tui

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/textinput"
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
			return nil, nil
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
			if len(m.providers) == 0 {
				return nil, nil
			}
			if m.cursor > 0 {
				m.cursor--
			} else {
				m.cursor = len(m.providers) - 1
			}
		case "down":
			if len(m.providers) == 0 {
				return nil, nil
			}
			if m.cursor < len(m.providers)-1 {
				m.cursor++
			} else {
				m.cursor = 0
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
	m.state = FirstRunComplete
	return nil, &AppMsg{Screen: ScreenREPL}
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
	title := lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render("M31A")
	subtitle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("Your terminal AI coding assistant")
	prompt := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Press Enter to begin setup")
	footer := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("M31A v0.1.0 — MIT License")

	content := lipgloss.JoinVertical(lipgloss.Center, title, subtitle, "", prompt, "", footer)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewProviderSelect() string {
	header := lipgloss.NewStyle().Bold(true).Foreground(m.theme.TextPrimary).Render("Select AI Provider(s):")

	options := []string{
		"OpenRouter (sk-or-v1-...)",
		"Zen (zk-...)",
		"Both (OpenRouter + Zen)",
		"Skip — start without API key",
	}

	var lines []string
	for i, opt := range options {
		marker := "  "
		if i == m.cursor {
			marker = lipgloss.NewStyle().Foreground(m.theme.Brand).Render("> ")
		}
		lines = append(lines, fmt.Sprintf("%s%d. %s", marker, i+1, opt))
	}

	body := strings.Join(lines, "\n")
	footer := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("↑/↓ to navigate, Enter to select, 1-4 to jump")

	content := lipgloss.JoinVertical(lipgloss.Top, header, "", body, "", footer)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Top, content)
}

func (m *FirstRunModel) viewKeyInput() string {
	var parts []string

	header := lipgloss.NewStyle().Bold(true).Foreground(m.theme.TextPrimary).Render("Enter your API key:")
	parts = append(parts, header)

	providerInfo := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(fmt.Sprintf("For: %s", strings.Join(m.providers, ", ")))
	parts = append(parts, providerInfo)

	parts = append(parts, "")
	parts = append(parts, m.apiKeyInput.View())
	parts = append(parts, "")

	if m.validationErr != "" {
		errText := lipgloss.NewStyle().Foreground(m.theme.Error).Render(m.validationErr)
		parts = append(parts, errText)
	}

	footer := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("Enter to confirm, Esc to go back")
	parts = append(parts, footer)

	content := lipgloss.JoinVertical(lipgloss.Top, parts...)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Top, content)
}

func (m *FirstRunModel) viewValidating() string {
	spinner := m.theme.Spinner.Render("⟳")
	text := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Validating API key...")
	content := lipgloss.JoinVertical(lipgloss.Center, spinner, text)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewKeychainPrompt() string {
	title := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Store API key in system keychain?")
	desc := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("This avoids entering the key each time you start M31A")
	yes := lipgloss.NewStyle().Foreground(m.theme.Success).Render("Y") + lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(" / ") + lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Enter")
	no := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("N") + lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(" = No")

	content := lipgloss.JoinVertical(lipgloss.Center, title, desc, "", yes+"  |  "+no)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewComplete() string {
	text := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("Setup complete! Launching M31A...")
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, text)
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
