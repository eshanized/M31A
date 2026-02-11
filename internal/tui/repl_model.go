package tui

import (
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

const (
	// MaxMessageHistory is the maximum number of messages stored in the REPL.
	MaxMessageHistory = 1000
	// inputHeight is the number of rows in the textarea.
	inputHeight = 3
)

// ProviderModelsFetchedMsg carries the result of an async FetchModels call.
type ProviderModelsFetchedMsg struct {
	Models []types.ModelInfo
	Model  *types.ModelInfo
	Err    error
}

// ReplModel is the main chat interface model.
// It manages the conversation viewport, input textarea, streaming state,
// and renders the welcome screen when no messages are present.
type ReplModel struct {
	// Layout
	theme        theme.Theme
	version      string
	width        int
	height       int
	sidebarWidth int

	// Bubble Tea components
	viewport viewport.Model
	textarea textarea.Model
	spinner  components.Spinner

	// Message state
	messages     []types.Message
	msgRenderer  *components.MessageRenderer
	userScrolled bool

	// Streaming state
	streaming         bool
	thinking          bool
	streamCh          <-chan tea.Msg
	streamContent     strings.Builder
	streamSegments    []types.MessageSegment
	activeSegmentType string
	thinkingStartAt   time.Time
	thinkingBlocks    map[int]*components.ThinkingBlock
	toolCards         map[int]*components.ToolCard

	// Provider state
	registry       *provider.Registry
	activeProvider string
	activeModel    *types.ModelInfo
	modelValid     bool
	sessionID      string
	cfg            *config.Config

	// Command state
	dispatcher  *tools.Dispatcher
	cmdRegistry *CommandRegistry
	keyRegistry *KeyRegistry

	// Slash command autocomplete
	slashVisible     bool
	slashSuggestions []CommandInfo
	slashSelected    int

	// History
	frecentHistory *FrecentHistory
	historyIndex   int
	savedInput     string // saves current input during history navigation

	// Quick actions panel state
	quickActionsCollapsed bool

	// Activity tracking
	lastActivity     time.Time
	lastStatus       string
	lastUsage        *types.Usage
	lastCost         float64
	sessionSparkline string

	// Working directory for @filepath resolution
	cwd string

	// Git branch (from sidebar, displayed in status bar)
	sidebarBranch string

	// @mention autocomplete state
	mentionVisible   bool
	mentionQuery     string
	mentionEntries   []MentionEntry
	mentionSelected  int
	mentionCompleter *MentionCompleter
	mentionStartCol  int

	// Project info for the welcome screen (pushed from SidebarRefreshMsg)
	changedFiles int

	// "New messages" indicator: tracks messages received while user is scrolled up
	newMessagesWhileScrolled int

	// Typing indicator: true between user submit and first streaming token
	awaitingResponse bool
}

// NewReplModel creates a new ReplModel.
func NewReplModel(t theme.Theme, version string) ReplModel {
	ta := textarea.New()
	ta.Placeholder = "Type a message, /command, or goal..."
	ta.ShowLineNumbers = false
	ta.CharLimit = 0
	ta.SetHeight(inputHeight)
	ta.Focus()

	// Remove textarea border — inherits terminal background (opencode style)
	ta.FocusedStyle.Base = lipgloss.NewStyle()
	ta.BlurredStyle.Base = lipgloss.NewStyle()

	m := ReplModel{
		theme:          t,
		version:        version,
		textarea:       ta,
		spinner:        components.NewSpinner(),
		thinkingBlocks: make(map[int]*components.ThinkingBlock),
		toolCards:      make(map[int]*components.ToolCard),
		modelValid:     true,
		historyIndex:   -1,
	}
	return m
}
