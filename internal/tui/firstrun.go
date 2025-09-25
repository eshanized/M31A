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
	"github.com/eshanized/M31A/internal/tui/components"
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
	t := m.theme
	w := m.width
	h := m.height

	if w == 0 || h == 0 {
		return ""
	}

	// 1. Starfield background (rendered as full terminal grid)
	starfield := components.RenderStarfield(w, h, 42, t)

	// 2. ASCII art logo (compact, 4 lines max)
	logo := m.renderWelcomeLogo()

	// 3. Subtitle
	subtitle := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("Terminal AI Coding Agent  " + m.versionLabel())

	// 4. 2x2 feature card grid (30+ cols each)
	features := m.renderFeatureCards()

	// 5. CTA box with SurfaceElevated background
	ctaText := "▶  Press Enter to begin setup"
	ctaBox := lipgloss.NewStyle().
		Background(t.SurfaceElevated).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 3).
		Foreground(t.Brand).
		Bold(true).
		Render(ctaText)

	// 6. Footer with keyboard shortcuts
	footer := m.renderLaunchpadFooter()

	// Stack content vertically
	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		subtitle,
		"",
		features,
		"",
		ctaBox,
		"",
		footer,
	)

	// Overlay content on starfield by placing content centered within terminal size
	// and compositing it over the starfield string line-by-line.
	_ = starfield // starfield is used as decorative background; lipgloss.Place centers content
	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content,
		lipgloss.WithWhitespaceChars("·"),
		lipgloss.WithWhitespaceForeground(t.TextMuted))
}

// renderWelcomeLogo renders a simplified 4-line ASCII art logo.
func (m *FirstRunModel) renderWelcomeLogo() string {
	t := m.theme
	logo := `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/`

	lines := strings.Split(logo, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
}

// versionLabel returns the version string for display.
func (m *FirstRunModel) versionLabel() string {
	if m.version == "" {
		return "v1.x"
	}
	return "v" + m.version
}

// renderFeatureCards renders a 2x2 grid of feature cards, 30+ cols each.
// At < 80 cols, cards stack vertically (2 columns → 1 column).
func (m *FirstRunModel) renderFeatureCards() string {
	t := m.theme

	type feature struct {
		icon  string
		title string
		desc  string
	}

	features := []feature{
		{"⚡", "Fast Execution", "Parallel task runner with dependency graph"},
		{"🤖", "AI-Powered Coding", "Natural language goals → production code"},
		{"🔄", "Self-Healing", "Auto-retry & rollback via commit bisect"},
		{"📦", "Git Native", "Atomic commits per task with rollback browser"},
	}

	cardWidth := 30
	if m.width >= 120 {
		cardWidth = 35
	}
	if m.width < 60 {
		cardWidth = m.width - 4
	}

	cards := make([]string, len(features))
	for i, f := range features {
		iconStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
		descStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)

		cardContent := lipgloss.JoinVertical(lipgloss.Left,
			iconStyle.Render(f.icon+"  "+f.title),
			descStyle.Render(f.desc),
		)

		card := lipgloss.NewStyle().
			Background(t.SurfaceElevated).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(1, 2).
			Width(cardWidth).
			Render(cardContent)

		cards[i] = card
	}

	// 2x2 grid if wide enough, otherwise 1-column stack
	if m.width >= 80 {
		row1 := lipgloss.JoinHorizontal(lipgloss.Top, cards[0], " ", cards[1])
		row2 := lipgloss.JoinHorizontal(lipgloss.Top, cards[2], " ", cards[3])
		return lipgloss.JoinVertical(lipgloss.Center, row1, row2)
	}

	// Narrow: stack vertically
	return lipgloss.JoinVertical(lipgloss.Center, cards...)
}

// renderLaunchpadFooter renders the keyboard shortcut footer.
func (m *FirstRunModel) renderLaunchpadFooter() string {
	t := m.theme

	shortcuts := []struct {
		key   string
		label string
	}{
		{"ctrl+p", "commands"},
		{"ctrl+b", "sidebar"},
		{"/help", "help"},
		{"MIT License", ""},
	}

	parts := make([]string, 0, len(shortcuts))
	for _, s := range shortcuts {
		if s.label == "" {
			// Non-interactive text (like "MIT License")
			parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(s.key))
		} else {
			keyStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
			labelStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
			parts = append(parts, keyStyle.Render(s.key)+" "+labelStyle.Render(s.label))
		}
	}

	return lipgloss.JoinHorizontal(lipgloss.Center, parts...)
}

func (m *FirstRunModel) viewProviderSelect() string {
	t := m.theme

	// Breadcrumb-style title
	title := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("M31A"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › "),
		lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render("Provider Setup"),
	)

	subtitle := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("Which AI gateway will power M31A?")

	subHint := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("You can add more later via /settings → Provider")

	// Provider options as full-width cards
	type providerOpt struct {
		key       string
		icon      string
		name      string
		desc      string
		coverage  int  // coverage percentage
		recommended bool
		isCard    bool // false for skip option
	}

	providers := []providerOpt{
		{key: "1", icon: "◆", name: "OpenRouter", desc: "100+ models · pay-per-use", coverage: 94, isCard: true},
		{key: "2", icon: "◈", name: "Zen", desc: "Fast inference · competitive pricing", coverage: 61, isCard: true},
		{key: "3", icon: "◆◈", name: "Both", desc: "OpenRouter + Zen with automatic failover", coverage: 98, recommended: true, isCard: true},
		{key: "4", icon: "○", name: "Skip", desc: "configure via /settings later", isCard: false},
	}

	var lines []string
	for i, p := range providers {
		if p.isCard {
			card := m.renderProviderCard(p, i == m.cursor)
			lines = append(lines, card)
		} else {
			// Skip option: less visual weight, no card border
			marker := "  "
			if i == m.cursor {
				marker = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
			}
			skipLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Render(fmt.Sprintf("%s%s  %s  %s", marker, lipgloss.NewStyle().Foreground(t.Brand).Render("["+p.key+"]"), p.icon, p.name+" — "+p.desc))
			lines = append(lines, skipLine)
		}
	}

	// Keyboard hints
	hints := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("↑↓ navigate  ·  Enter select  ·  1-4 jump")

	content := lipgloss.JoinVertical(lipgloss.Center,
		title,
		subtitle,
		subHint,
		"",
		strings.Join(lines, "\n"),
		"",
		hints,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

// renderProviderCard renders a full-width provider option as a card with coverage bar.
func (m *FirstRunModel) renderProviderCard(p struct {
	key       string
	icon      string
	name      string
	desc      string
	coverage  int
	recommended bool
	isCard    bool
}, isActive bool) string {
	t := m.theme

	// Build card content
	marker := "  "
	if isActive {
		marker = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}

	iconStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
	nameStyle := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	descStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)

	headerLine := fmt.Sprintf("%s%s %s  %s",
		marker,
		lipgloss.NewStyle().Foreground(t.Brand).Render("["+p.key+"]"),
		iconStyle.Render(p.icon),
		nameStyle.Render(p.name),
	)

	descLine := lipgloss.NewStyle().PaddingLeft(5).Render(descStyle.Render(p.desc))

	// Coverage bar
	barWidth := 30
	filled := p.coverage * barWidth / 100
	empty := barWidth - filled
	bar := strings.Repeat("█", filled) + strings.Repeat("░", empty)
	coverageText := fmt.Sprintf("  Coverage: %d%%", p.coverage)
	barLine := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.Success).Render(bar),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(coverageText),
	)

	// Recommended badge
	var badgeLine string
	if p.recommended {
		badge := t.SuccessBadge.Render(" Recommended")
		badgeLine = lipgloss.NewStyle().PaddingLeft(5).Render(badge)
	}

	content := lipgloss.JoinVertical(lipgloss.Left,
		headerLine,
		descLine,
		barLine,
	)
	if badgeLine != "" {
		content += "\n" + badgeLine
	}

	// Card border
	borderColor := t.Border
	if isActive {
		borderColor = t.Brand
	}

	card := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Background(t.Surface).
		Padding(0, 1).
		Width(m.width - 8).
		Render(content)

	return card
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
		Foreground(m.theme.BadgeForeground).
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
			baseURL = "https://openrouter.ai/api/v1"
		}
		url = baseURL + "/auth/key"
	case "zen":
		baseURL := zenBaseURL
		if baseURL == "" {
			baseURL = "https://opencode.ai/zen/v1"
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
	req.Header.Set("User-Agent", "M31A/dev")
	if provider == "openrouter" {
		referer := openrouterReferer
		if referer == "" {
			referer = "https://github.com/eshanized/M31A"
		}
		title := openrouterTitle
		if title == "" {
			title = "M31A"
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
