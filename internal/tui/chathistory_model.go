package tui

import (
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ChatHistoryModel displays all messages in the current session as a scrollable table.
type ChatHistoryModel struct {
	theme    theme.Theme
	messages []types.Message
	viewport viewport.Model
	loaded   bool
	width    int
	height   int
	cursor   int // selected row index
}

// NewChatHistoryModel creates a ChatHistoryModel.
func NewChatHistoryModel(t theme.Theme, w, h int) *ChatHistoryModel {
	vpH := h - 6
	if vpH < 3 {
		vpH = 3
	}
	return &ChatHistoryModel{
		theme:    t,
		width:    w,
		height:   h,
		viewport: viewport.New(w, vpH),
	}
}

// SetTheme updates the theme.
func (ch *ChatHistoryModel) SetTheme(t theme.Theme) {
	ch.theme = t
}

// SetDimensions updates the chat history model dimensions.
func (ch *ChatHistoryModel) SetDimensions(w, h int) {
	ch.width = w
	ch.height = h
	vpH := h - 6
	if vpH < 3 {
		vpH = 3
	}
	ch.viewport = viewport.New(w, vpH)
	if ch.loaded {
		ch.viewport.SetContent(ch.renderTable())
	}
}

// SetMessages sets the messages to display.
func (ch *ChatHistoryModel) SetMessages(msgs []types.Message) {
	ch.messages = msgs
	ch.loaded = true
	ch.cursor = 0
	ch.viewport.SetContent(ch.renderTable())
	ch.viewport.GotoTop()
}

// Init implements tea.Model.
func (ch *ChatHistoryModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (ch *ChatHistoryModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		ch.SetDimensions(msg.Width, msg.Height)
		return ch, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return ch, func() tea.Msg {
				return PopScreenMsg{}
			}
		case "up", "k":
			if ch.cursor > 0 {
				ch.cursor--
				ch.clampScroll()
			}
		case "down", "j":
			if ch.cursor < len(ch.messages)-1 {
				ch.cursor++
				ch.clampScroll()
			}
		case "g":
			ch.cursor = 0
			ch.clampScroll()
		case "G":
			if len(ch.messages) > 0 {
				ch.cursor = len(ch.messages) - 1
				ch.clampScroll()
			}
		case "enter":
			if len(ch.messages) > 0 {
				idx := ch.cursor
				return ch, func() tea.Msg {
					return ChatHistoryContinueMsg{MessageIndex: idx}
				}
			}
		}
	}
	return ch, nil
}

// View implements tea.Model — content only, chrome handled by PageLayout.
func (ch *ChatHistoryModel) View() string {
	return ch.renderChatHistory()
}

// clampScroll ensures the cursor stays in the visible viewport.
func (ch *ChatHistoryModel) clampScroll() {
	listH := ch.visibleRows()
	if ch.cursor < ch.viewport.YOffset {
		ch.viewport.YOffset = ch.cursor
	}
	if ch.cursor >= ch.viewport.YOffset+listH {
		ch.viewport.YOffset = ch.cursor - listH + 1
	}
}

// visibleRows returns the number of rows that fit in the visible area.
func (ch *ChatHistoryModel) visibleRows() int {
	h := ch.height - 6
	if h < 3 {
		return 3
	}
	return h
}
