package tui

import (
	"fmt"
	"log/slog"
	"strconv"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/keychain"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ─── Field types ──────────────────────────────────────────────────────────────

type cfgFieldType int

const (
	cfgText     cfgFieldType = iota // free-form string
	cfgPassword                     // masked string
	cfgBool                         // yes/no toggle
	cfgNumber                       // integer
	cfgFloat                        // float64
	cfgChoice                       // one of a fixed set
	cfgReadOnly                     // display only (e.g. API key masked)
)

// cfgField describes one editable row.
type cfgField struct {
	key       string
	label     string
	fieldType cfgFieldType
	choices   []string // only for cfgChoice
	hint      string   // short help shown when selected
}

// cfgSection groups fields under a TOML section header.
type cfgSection struct {
	title  string
	fields []cfgField
}

// ─── ConfigSavedMsg ───────────────────────────────────────────────────────────

// ConfigSavedMsg is emitted when the config editor writes to disk.
type ConfigSavedMsg struct{}

// ─── ConfigModel ──────────────────────────────────────────────────────────────

// ConfigModel is the full interactive configuration editor for ScreenConfig.
// Every field from every section of config.Config is editable inline.
// Pressing 's' saves directly to the TOML file without leaving the screen.
type ConfigModel struct {
	theme    theme.Theme
	cfg      *config.Config
	cfgPath  string
	width    int
	height   int
	viewport viewport.Model
	keychain keychain.Keychain

	// Navigation
	sections   []cfgSection
	sectionIdx int // which section is active
	fieldIdx   int // which field inside the section

	// Editing
	editing   bool
	editInput textinput.Model

	// Status
	dirty          bool
	confirmingExit bool
	statusMsg      string
	statusTime     time.Time
	saveErr        string
}

// NewConfigModel creates a ConfigModel.
func NewConfigModel(t theme.Theme, cfg *config.Config, cfgPath string, w, h int, kc keychain.Keychain) *ConfigModel {
	ti := textinput.New()
	ti.CharLimit = 512
	// Width set relative to terminal; will be corrected on first WindowSizeMsg
	ti.Width = max(20, min(50, w-12))

	vpH := h - 8
	if vpH < 4 {
		vpH = 4
	}
	vpW := max(10, w-4)
	vp := viewport.New(vpW, vpH)
	vp.Style = lipgloss.NewStyle().PaddingLeft(0)

	m := &ConfigModel{
		theme:     t,
		cfg:       cfg,
		cfgPath:   cfgPath,
		width:     w,
		height:    h,
		editInput: ti,
		viewport:  vp,
		keychain:  kc,
	}
	m.buildSections()
	return m
}

// ─── Getters / Setters ────────────────────────────────────────────────────────

// getFieldValue reads the current value of a field from cfg.
// Delegates to section-specific handlers for maintainability.
func (m *ConfigModel) getFieldValue(f cfgField) string {
	if m.cfg == nil {
		return ""
	}
	c := m.cfg

	// Try each section handler in order
	if v, ok := getProviderFieldValue(c, f.key); ok {
		return v
	}
	if v, ok := getTUIFieldValue(c, f.key); ok {
		return v
	}
	if v, ok := getGeneralFieldValue(c, f.key); ok {
		return v
	}
	if v, ok := getAdvancedFieldValue(c, f.key); ok {
		return v
	}

	// Legacy ui.theme field (kept for config backward compatibility)
	if f.key == "ui.theme" {
		if c.UI.Theme == "" {
			return "dark"
		}
		return c.UI.Theme
	}
	return ""
}

// setFieldValue writes a new value into cfg.
// Delegates to section-specific handlers for maintainability.
func (m *ConfigModel) setFieldValue(f cfgField, val string) {
	if m.cfg == nil {
		m.cfg = config.DefaultConfig()
	}
	c := m.cfg

	// Try each section handler in order
	if setProviderFieldValue(c, f.key, val) {
		return
	}
	if setTUIFieldValue(c, f.key, val) {
		return
	}
	if setGeneralFieldValue(c, f.key, val) {
		return
	}
	if setAdvancedFieldValue(c, f.key, val) {
		return
	}

	// Legacy ui.theme field (kept for config backward compatibility)
	if f.key == "ui.theme" {
		c.UI.Theme = val
	}
}

// ─── Bubble Tea interface ──────────────────────────────────────────────────────

func (m *ConfigModel) Init() tea.Cmd { return nil }

// SetDimensions updates the config model dimensions.
func (m *ConfigModel) SetDimensions(w, h int) {
	m.width = w
	m.height = h
	m.viewport.Width = max(10, w-4)
	vpH := h - 8
	if vpH < 4 {
		vpH = 4
	}
	m.viewport.Height = vpH
	m.editInput.Width = max(20, min(50, w-12))
	m.updateViewportContent()
}

// SetTheme updates the theme.
func (m *ConfigModel) SetTheme(t theme.Theme) {
	m.theme = t
}

func (m *ConfigModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if m.confirmingExit {
			return m.updateConfirmExit(msg)
		}
		if m.editing {
			return m.updateEditing(msg)
		}
		return m.updateBrowsing(msg)

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.viewport.Width = max(10, msg.Width-4)
		h := msg.Height - 8
		if h < 4 {
			h = 4
		}
		m.viewport.Height = h
		m.editInput.Width = max(20, min(50, msg.Width-12))
	}

	var cmd tea.Cmd
	m.viewport, cmd = m.viewport.Update(msg)
	// Update viewport content to maintain Elm architecture purity
	m.updateViewportContent()
	return m, cmd
}

func (m *ConfigModel) updateConfirmExit(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "y", "Y":
		return m, func() tea.Msg { return PopScreenMsg{} }
	case "n", "N", "esc":
		m.confirmingExit = false
		return m, nil
	}
	return m, nil
}

func (m *ConfigModel) updateBrowsing(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc", "q":
		if m.dirty {
			m.confirmingExit = true
			return m, nil
		}
		return m, func() tea.Msg { return PopScreenMsg{} }

	case "tab", "]":
		m.sectionIdx = (m.sectionIdx + 1) % len(m.sections)
		m.fieldIdx = 0

	case "shift+tab", "[":
		m.sectionIdx = (m.sectionIdx - 1 + len(m.sections)) % len(m.sections)
		m.fieldIdx = 0

	case "up", "k":
		if m.fieldIdx > 0 {
			m.fieldIdx--
		} else if m.sectionIdx > 0 {
			m.sectionIdx--
			m.fieldIdx = len(m.sections[m.sectionIdx].fields) - 1
		}

	case "down", "j":
		sec := m.sections[m.sectionIdx]
		if m.fieldIdx < len(sec.fields)-1 {
			m.fieldIdx++
		} else if m.sectionIdx < len(m.sections)-1 {
			m.sectionIdx++
			m.fieldIdx = 0
		}

	case "enter", "e", " ":
		return m.activateField()

	case "s":
		return m.saveConfig()

	case "L":
		return m.saveLocalConfig()

	case "r":
		if m.cfgPath != "" {
			if cfg, err := config.Load(m.cfgPath); err == nil {
				m.cfg = cfg
				m.buildSections()
				m.updateViewportContent()
				m.statusMsg = "↺ Config reloaded from disk"
				m.statusTime = time.Now()
				return m, func() tea.Msg { return ConfigSavedMsg{} }
			}
		}
		m.buildSections()
		m.updateViewportContent()
		m.statusMsg = "↺ Config reloaded from memory"
		m.statusTime = time.Now()
	}

	m.updateViewportContent()
	m.scrollToField()
	return m, nil
}

func (m *ConfigModel) activateField() (tea.Model, tea.Cmd) {
	if len(m.sections) == 0 {
		return m, nil
	}
	sec := m.sections[m.sectionIdx]
	if m.fieldIdx >= len(sec.fields) {
		return m, nil
	}
	f := sec.fields[m.fieldIdx]

	switch f.fieldType {
	case cfgBool:
		cur := m.getFieldValue(f)
		newVal := "yes"
		if cur == "yes" {
			newVal = "no"
		}
		m.setFieldValue(f, newVal)
		m.dirty = true
		m.statusMsg = fmt.Sprintf("✎ %s → %s  (press s to save)", f.label, newVal)
		m.statusTime = time.Now()
		m.updateViewportContent()

	case cfgChoice:
		if len(f.choices) == 0 {
			return m, nil
		}
		cur := m.getFieldValue(f)
		idx := 0
		for i, c := range f.choices {
			if c == cur {
				idx = i
				break
			}
		}
		next := f.choices[(idx+1)%len(f.choices)]
		m.setFieldValue(f, next)
		m.dirty = true
		m.statusMsg = fmt.Sprintf("✎ %s → %s  (press s to save)", f.label, next)
		m.statusTime = time.Now()
		m.updateViewportContent()

	case cfgReadOnly:
		m.statusMsg = "(read-only field)"
		m.statusTime = time.Now()

	default: // text, password, number, float
		m.editing = true
		m.editInput.SetValue(m.getFieldValue(f))
		m.editInput.Focus()
		m.editInput.Placeholder = f.label
		if f.fieldType == cfgPassword {
			m.editInput.EchoMode = textinput.EchoPassword
		} else {
			m.editInput.EchoMode = textinput.EchoNormal
		}
		return m, textinput.Blink
	}
	return m, nil
}

func (m *ConfigModel) updateEditing(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		m.editing = false
		m.editInput.EchoMode = textinput.EchoNormal
		return m, nil
	case "enter":
		val := strings.TrimSpace(m.editInput.Value())
		m.editing = false
		m.editInput.EchoMode = textinput.EchoNormal
		if val == "" {
			return m, nil
		}
		sec := m.sections[m.sectionIdx]
		f := sec.fields[m.fieldIdx]
		// Validate numeric fields before setting
		if (f.fieldType == cfgNumber || f.fieldType == cfgFloat) && val != "" {
			if f.fieldType == cfgNumber {
				if _, err := strconv.Atoi(val); err != nil {
					m.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid integer", val)
					m.statusTime = time.Now()
					return m, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid integer", val), "error")
				}
			} else {
				if _, err := strconv.ParseFloat(val, 64); err != nil {
					m.statusMsg = fmt.Sprintf("Invalid number: %q is not a valid float", val)
					m.statusTime = time.Now()
					return m, newToastCmd(fmt.Sprintf("Invalid number: %q is not a valid float", val), "error")
				}
			}
		}
		m.setFieldValue(f, val)
		m.dirty = true
		displayVal := val
		if f.fieldType == cfgPassword && len(val) > 4 {
			displayVal = "••••" + val[len(val)-4:]
		}
		m.statusMsg = fmt.Sprintf("✎ %s → %s  (press s to save)", f.label, displayVal)
		m.statusTime = time.Now()
		m.updateViewportContent()
		return m, nil
	}

	var cmd tea.Cmd
	m.editInput, cmd = m.editInput.Update(msg)
	return m, cmd
}

func (m *ConfigModel) saveConfig() (tea.Model, tea.Cmd) {
	if m.cfg == nil || m.cfgPath == "" {
		m.statusMsg = "✗ No config path set"
		m.statusTime = time.Now()
		return m, nil
	}
	if err := m.cfg.SaveWithKeychain(m.cfgPath, m.keychain); err != nil {
		m.saveErr = err.Error()
		m.statusMsg = "✗ Save failed: " + err.Error()
		m.statusTime = time.Now()
		slog.Warn("config editor save failed", "error", err)
		return m, nil
	}
	m.dirty = false
	m.saveErr = ""
	m.statusMsg = fmt.Sprintf("✓ Saved to %s", m.cfgPath)
	m.statusTime = time.Now()
	return m, func() tea.Msg { return ConfigSavedMsg{} }
}

func (m *ConfigModel) saveLocalConfig() (tea.Model, tea.Cmd) {
	if m.cfg == nil {
		m.statusMsg = "✗ No config loaded"
		m.statusTime = time.Now()
		return m, nil
	}
	localPath, err := config.LocalConfigPath()
	if err != nil {
		m.statusMsg = fmt.Sprintf("✗ Cannot determine working directory: %v", err)
		m.statusTime = time.Now()
		return m, nil
	}
	if err := m.cfg.SaveProject(localPath); err != nil {
		m.saveErr = err.Error()
		m.statusMsg = "✗ Local save failed: " + err.Error()
		m.statusTime = time.Now()
		return m, nil
	}
	m.dirty = false
	m.saveErr = ""
	m.statusMsg = fmt.Sprintf("✓ Project config saved to %s", localPath)
	m.statusTime = time.Now()
	return m, func() tea.Msg { return ConfigSavedMsg{} }
}
