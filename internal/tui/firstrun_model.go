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
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
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
	state              FirstRunState
	theme              theme.Theme
	version            string
	cursor             int
	providers          []string
	apiKeyInput        textinput.Model
	apiKeyValue        string
	validating         bool
	validationErr      string
	statusMsg          string
	width              int
	height             int
	configPath         string
	openrouterBaseURL  string
	zenBaseURL         string
	openrouterReferer  string
	openrouterTitle    string
}

type FirstRunOpts struct {
	OpenRouterBaseURL string
	ZenBaseURL        string
	OpenRouterReferer string
	OpenRouterTitle   string
}

func NewFirstRunModel(t theme.Theme, configPath string, version string, opts ...FirstRunOpts) FirstRunModel {
	ti := textinput.New()
	ti.Placeholder = "sk-or-v1-..."
	ti.EchoMode = textinput.EchoPassword
	ti.Focus()
	ti.Width = 60
	ti.CharLimit = 128

	m := FirstRunModel{
		state:       FirstRunWelcome,
		theme:       t,
		version:     version,
		providers:   make([]string, 0),
		apiKeyInput: ti,
		configPath:  configPath,
	}
	if len(opts) > 0 {
		o := opts[0]
		m.openrouterBaseURL = o.OpenRouterBaseURL
		m.zenBaseURL = o.ZenBaseURL
		m.openrouterReferer = o.OpenRouterReferer
		m.openrouterTitle = o.OpenRouterTitle
	}
	return m
}

func (m *FirstRunModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c":
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
			if m.cursor > 0 {
				m.cursor--
			} else {
				m.cursor = 3
			}
		case "down":
			if m.cursor < 3 {
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
					for _, provider := range m.providers {
						if err := validateAPIKey(provider, input, m.openrouterBaseURL, m.zenBaseURL, m.openrouterReferer, m.openrouterTitle); err != nil {
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

// validateAPIKey makes a test HTTP request to verify the API key works.
func validateAPIKey(provider, key, openrouterBaseURL, zenBaseURL, openrouterReferer, openrouterTitle string) error {
	client := &http.Client{
		Timeout: 10 * time.Second,
		Transport: &http.Transport{
			DialContext: (&net.Dialer{Timeout: 5 * time.Second}).DialContext,
		},
	}

	var url string
	switch provider {
	case "openrouter":
		baseURL := openrouterBaseURL
		if baseURL == "" {
			baseURL = types.DefaultOpenRouterBaseURL
		}
		url = baseURL + "/auth/key"
	case "zen":
		baseURL := zenBaseURL
		if baseURL == "" {
			baseURL = types.DefaultZenBaseURL
		}
		url = baseURL + "/models"
	default:
		return fmt.Errorf("unknown provider: %s", provider)
	}

	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, url, nil)
	if err != nil {
		return fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+key)
	req.Header.Set("User-Agent", types.DefaultUserAgent)
	if provider == "openrouter" {
		referer := openrouterReferer
		if referer == "" {
			referer = types.DefaultReferer
		}
		title := openrouterTitle
		if title == "" {
			title = types.DefaultXTitle
		}
		req.Header.Set("HTTP-Referer", referer)
		req.Header.Set("X-Title", title)
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
