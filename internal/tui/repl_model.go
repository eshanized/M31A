package tui

import (
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/integrations/history"
	"github.com/eshanized/M31A/internal/integrations/provider"
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
	styleCache   *theme.StyleCache
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
	frecentHistory *history.FrecentHistory
	historyIndex   int
	savedInput     string // saves current input during history navigation

	// Quick actions overlay state (ctrl+q toggles the dropdown)
	quickActionsVisible bool

	// Inline viewport search state (ctrl+f toggles)
	search searchState

	// Activity tracking
	lastActivity     time.Time
	lastStatus       string
	lastUsage        *types.Usage
	lastCost         float64
	sessionSparkline string

	// Workflow phase tracking for status bar progress indicator
	workflowPhase      string // current workflow phase name (e.g., "plan", "execute")
	workflowPhaseIndex int    // numeric phase index (0-based)
	totalPhases        int    // total number of workflow phases

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
	mentionAtPos     int // absolute position of the '@' trigger in the textarea value

	// Project info for the welcome screen (pushed from SidebarRefreshMsg)
	changedFiles int

	// "New messages" indicator: tracks messages received while user is scrolled up
	newMessagesWhileScrolled int

	// Typing indicator: true between user submit and first streaming token
	awaitingResponse bool

	// Render throttle: skip renderMessages() if called within minRenderInterval
	// of the previous render. Reduces CPU during high-frequency streaming ticks.
	lastRenderTime time.Time

	// Smooth scroll: ease-out scrolling toward smoothScrollTarget during streaming
	smoothScrollTarget int
	viewportContent    string // cached viewport content for line counting

	// Incremental rendering cache: avoids full re-render during streaming
	cachedMessageContent string // cached rendered content of all finalized messages
	cachedMessageCount   int    // number of messages in the cache

	// Streaming render cache: avoids re-allocating components on every tick
	cachedThinkingBlock   *components.ThinkingBlock // cached ThinkingBlock during streaming
	cachedThinkingContent string                    // content used to create cachedThinkingBlock

	// Lightweight markdown parser for streaming content (D-27)
	lightweightMarkdown *components.LightweightMarkdown

	// Viewport virtualization: only render visible messages (D-30)
	visibleRangeStart int // first visible message index
	visibleRangeEnd   int // last visible message index (exclusive)
	virtualBuffer     int // messages to render above/below visible range

	// Resize debounce: prevent flicker during rapid resize events (D-13)
	resizeTimer   *time.Timer
	lastWidth     int // track last rendered width to skip height-only changes
	resizePending bool

	// Mouse interaction state
	scrollbarDragging bool // true while the user is dragging the scrollbar thumb

	// messageLineOffsets[i] is the line offset in viewportContent where
	// messages[i] begins. Populated by renderMessages() for mouse hit-testing.
	messageLineOffsets []int

	// Live tool tracking: maps tool name → message index for in-progress agent
	// loop tool cards, so AgentToolDoneMsg can update them in-place.
	liveToolIndex map[string]int

	// waveOffset drives the animated wave separator during streaming/thinking.
	// Incremented on each spinner tick to create a travelling ▁▂▃▄ wave.
	waveOffset int

	// welcomeRevealCount tracks how many getting-started prompts have been
	// revealed via the typewriter animation on the welcome screen (max 3).
	welcomeRevealCount int
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

	// Visible cursor: block char + brand color so the caret stands out.
	ta.Cursor.SetChar("█")
	ta.Cursor.Style = lipgloss.NewStyle().Foreground(t.Brand)

	m := ReplModel{
		theme:               t,
		styleCache:          theme.NewStyleCache(t),
		version:             version,
		textarea:            ta,
		spinner:             components.NewSpinner(),
		thinkingBlocks:      make(map[int]*components.ThinkingBlock),
		toolCards:           make(map[int]*components.ToolCard),
		liveToolIndex:       make(map[string]int),
		modelValid:          true,
		historyIndex:        -1,
		lightweightMarkdown: components.NewLightweightMarkdown(t),
		virtualBuffer:       5, // render 5 messages above/below visible range
	}
	return m
}
