package tui

import (
	"context"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ReplModel holds all state for the REPL screen.
type ReplModel struct {
	theme          theme.Theme
	version        string
	messages       []types.Message
	viewport       viewport.Model
	textarea       textarea.Model
	spinner        spinner.Model
	scrollPos      int
	inputHistory   []string
	historyPos     int
	frecentHistory *FrecentHistory
	placeholder    string
	streaming      bool
	thinking       bool
	lastStatus     string
	width          int
	height         int
	sidebarWidth   int // width reserved for sidebar (0 if hidden)

	msgRenderer  *components.MessageRenderer
	streamCancel context.CancelFunc

	currentMessage    *types.Message
	streamSegments    []types.MessageSegment
	streamContent     strings.Builder
	thinkingStartAt   time.Time
	activeSegmentType string
	thinkingBlocks    map[int]*components.ThinkingBlock
	toolCards         map[int]*components.ToolCard

	lastArbitrageFetch time.Time // throttle auto-arbitrage model fetches

	fallbackBanner   string    // current fallback banner text, empty = no banner
	fallbackBannerAt time.Time // when the banner appeared (for 15s auto-dismiss)

	// Active question from AskUserQuestion tool
	activeQuestion *QuestionRequestMsg

	// Provider access for LLM calls
	registry       *provider.Registry
	activeProvider string
	activeModel    *types.ModelInfo
	modelValid     bool // false when the active model is not in the current provider's catalog
	sessionID      string
	cwd            string // working directory for @filepath resolution

	// Config access for arbitrage settings
	cfg *config.Config

	// Dispatcher for shell mode (! prefix) command execution
	dispatcher *tools.Dispatcher

	// Fix C-3: streamCh is a read-only reference to the channel owned by
	// StartStreamCmd's goroutine. The REPL never creates or closes this
	// channel — it only reads from it via continuation cmds. The goroutine
	// owns the write side and closes streamCh when the stream completes.
	streamCh <-chan tea.Msg

	// Last stream usage and cost (from StreamDoneMsg)
	lastUsage *types.Usage
	lastCost  float64

	// Per-block thinking focus
	thinkingFocusIndex int // -1 = no focus, otherwise index into thinkingBlocks

	// References for View() rendering
	keyRegistry  *KeyRegistry
	lastActivity time.Time

	// Slash command autocomplete
	slashSuggestions []CommandInfo
	slashSelected    int
	slashVisible     bool
	cmdRegistry      *CommandRegistry

	// Recent session activity sparkline (populated by AppState via SetSessionSparkline)
	sessionSparkline string

	// Auto-scroll control: track if user has manually scrolled up
	userScrolled bool

	// Debounce for thinking block toggle during streaming
	lastToggleAt time.Time
}

// autoScrollConditionally scrolls to bottom only if the user hasn't manually scrolled up.
func (m *ReplModel) autoScrollConditionally() {
	if !m.userScrolled {
		m.viewport.GotoBottom()
	}
}

// atBottom returns true if the viewport is at or near the bottom.
func (m *ReplModel) atBottom() bool {
	return m.viewport.AtBottom()
}

// MaxMessageHistory is the maximum number of messages retained in the REPL.
const MaxMessageHistory = 1000

// NewReplModel creates a new ReplModel with the given theme and version.
func NewReplModel(t theme.Theme, version string) ReplModel {
	ta := textarea.New()
	ta.Placeholder = "Type a message, /command, or goal..."
	ta.SetWidth(80)
	ta.SetHeight(3)
	ta.ShowLineNumbers = false
	ta.KeyMap.InsertNewline.SetEnabled(false)
	ta.Focus()
	ta.CharLimit = 0

	vp := viewport.New(80, 20)

	s := spinner.NewModel()
	s.Spinner = spinner.Dot
	s.Style = t.Spinner

	renderer, err := components.NewMessageRenderer(t, 80)
	if err != nil {
		renderer = nil
	}

	m := ReplModel{
		theme:          t,
		version:        version,
		viewport:       vp,
		textarea:       ta,
		spinner:        s,
		inputHistory:   make([]string, 0),
		historyPos:     -1,
		msgRenderer:    renderer,
		thinkingBlocks: make(map[int]*components.ThinkingBlock),
		toolCards:      make(map[int]*components.ToolCard),
	}

	// Set welcome message in viewport
	m.viewport.SetContent(m.renderWelcome())

	return m
}

// ProviderModelsFetchedMsg is returned when FetchModels completes asynchronously.
type ProviderModelsFetchedMsg struct {
	Models []types.ModelInfo
	Model  *types.ModelInfo
	Err    error
}
