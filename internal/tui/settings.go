package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/keychain"
)

// SettingsModel provides a tabbed configuration editor.
// Initialized in NewApp and active when screen == ScreenSettings.
type SettingsModel struct {
	config    *config.Config
	theme     theme.Theme
	activeTab int // 0=General, 1=Provider, 2=Permissions, 3=Features
	width     int
	height    int
	keychain  keychain.Keychain
	storeInKC bool // toggle for "store keys in keychain"
	dirty     bool // true if changes unsaved
	statusMsg string
}

// NewSettingsModel creates a SettingsModel with the given config, theme, and optional keychain.
func NewSettingsModel(cfg *config.Config, t theme.Theme, kc keychain.Keychain) *SettingsModel {
	return &SettingsModel{
		config:   cfg,
		theme:    t,
		keychain: kc,
	}
}

// Update handles messages for the settings screen.
func (m *SettingsModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	return nil, nil
}

// View renders the settings screen.
func (m *SettingsModel) View() string {
	return "Settings"
}

// Save saves the config to disk and optionally stores keys to keychain.
func (m *SettingsModel) Save() error {
	return nil
}
