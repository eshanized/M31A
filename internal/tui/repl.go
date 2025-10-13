package tui

import (
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
)

// fileRefPattern matches @filepath references for editor context auto-include.
// It captures @./path, @../path, @path/to/file, and @/absolute/path.
// It does NOT match standalone @user or @mention (requires a path separator '/').
var fileRefPattern = regexp.MustCompile(`@(\./|\.\./|[^\s@]+/)[^\s@]+`)

// ShellResultMsg is sent when a shell command (!prefix) completes execution.
type ShellResultMsg struct {
	Command string
	Output  string
	Err     string
}

// ReplModel struct and NewReplModel are defined in repl_model.go

// autoScrollConditionally, atBottom, and NewReplModel are defined in repl_model.go

func (m *ReplModel) Update(msg tea.Msg) ([]tea.Cmd, bool) {
	// Check if fallback banner has expired
	if m.fallbackBanner != "" && time.Now().After(m.fallbackBannerAt) {
		m.fallbackBanner = ""
	}

	// Handle slash command autocomplete key interactions
	if m.slashVisible {
		if keyMsg, ok := msg.(tea.KeyMsg); ok {
			switch keyMsg.String() {
			case "tab":
				// Second Tab: accept the first match
				if len(m.slashSuggestions) > 0 {
					sel := m.slashSuggestions[0]
					current := m.textarea.Value()
					parts := strings.Fields(current)
					if len(parts) > 0 && strings.HasPrefix(parts[0], "/") {
						parts[0] = sel.Slash
						completed := strings.Join(parts, " ")
						m.textarea.SetValue(completed)
						m.textarea.CursorEnd()
					}
				}
				m.slashVisible = false
				m.slashSuggestions = nil
				return nil, false
			case "up":
				if m.slashSelected > 0 {
					m.slashSelected--
				}
				return nil, false
			case "down":
				if m.slashSelected < len(m.slashSuggestions)-1 {
					m.slashSelected++
				}
				return nil, false
			case "esc":
				m.slashVisible = false
				m.slashSuggestions = nil
				return nil, false
			}
		}
	}

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		// Minimum input height is 1 line (even on very small terminals)
		inputHeight := 3
		const minInputHeight = 1
		// Account for textarea(3) + metadataRow(1) + bottomBorder(1) + statusBar(1) = 6 total chrome lines.
		const chromeHeight = 6
		availableForContent := msg.Height - chromeHeight
		if availableForContent < minInputHeight+1 {
			// Terminal too small — give input minimum height, viewport gets what's left
			inputHeight = minInputHeight
			if availableForContent > minInputHeight {
				inputHeight = availableForContent - 1
			}
		}
		vpHeight := msg.Height - chromeHeight - inputHeight
		if vpHeight < 1 {
			vpHeight = 1
		}
		replWidth := msg.Width - m.sidebarWidth
		if replWidth < 20 {
			replWidth = 20
		}
		m.viewport.Width = replWidth
		m.viewport.Height = vpHeight
		m.textarea.SetWidth(replWidth)
		m.textarea.SetHeight(inputHeight)
		if m.msgRenderer != nil {
			if err := m.msgRenderer.SetWidth(replWidth - 4); err != nil {
				m.lastStatus = fmt.Sprintf("Renderer resize failed: %v", err)
			}
		}
		// Show welcome message if no messages yet
		if len(m.messages) == 0 {
			m.viewport.SetContent(m.renderWelcome())
		}

	case StreamMsg:
		return m.handleStreamMsg(msg)

	case StreamDoneMsg:
		return m.handleStreamDoneMsg(msg)

	case StreamErrorMsg:
		return m.handleStreamErrorMsg(msg)

	case TickMsg:
		if m.streaming {
			m.renderMessages()
			m.autoScrollConditionally()
		}
		return m.streamTickCmds()

	case tea.KeyMsg:
		if m.streaming {
			return m.handleStreamingKeyMsg(msg)
		}

		return m.handleKeyMsg(msg)

	case ShellResultMsg:
		return m.handleShellResult(msg)

	case FallbackEventMsg:
		return m.handleFallbackEvent(msg)

	case spinner.TickMsg:
		var spCmd tea.Cmd
		m.spinner, spCmd = m.spinner.Update(msg)
		return []tea.Cmd{spCmd}, false
	}

	var cmds []tea.Cmd
	var taCmd tea.Cmd
	var vpCmd tea.Cmd
	var spCmd tea.Cmd

	m.textarea, taCmd = m.textarea.Update(msg)
	m.viewport, vpCmd = m.viewport.Update(msg)
	m.spinner, spCmd = m.spinner.Update(msg)

	// Detect slash command typing and update suggestions
	if m.cmdRegistry != nil {
		current := m.textarea.Value()
		if strings.HasPrefix(current, "/") {
			// Extract the partial command (first word after /)
			parts := strings.Fields(current)
			partial := ""
			if len(parts) > 0 {
				partial = strings.TrimPrefix(parts[0], "/")
			}

			// Generate matching commands
			allCmds := m.cmdRegistry.AllCommands()
			m.slashSuggestions = nil
			if partial == "" {
				// Show all commands when just "/" is typed
				m.slashSuggestions = allCmds
			} else {
				// Filter by partial match
				q := strings.ToLower(partial)
				for _, cmd := range allCmds {
					name := strings.ToLower(cmd.Name)
					slash := strings.ToLower(cmd.Slash)
					if strings.HasPrefix(name, q) || strings.HasPrefix(slash, q) || strings.Contains(name, q) {
						m.slashSuggestions = append(m.slashSuggestions, cmd)
					}
				}
			}

			// Show suggestions if we have matches
			if len(m.slashSuggestions) > 0 {
				m.slashVisible = true
				m.slashSelected = 0
				// Limit to 8 suggestions
				if len(m.slashSuggestions) > 8 {
					m.slashSuggestions = m.slashSuggestions[:8]
				}
			} else {
				m.slashVisible = false
			}
		} else {
			m.slashVisible = false
			m.slashSuggestions = nil
		}
	}

	cmds = append(cmds, taCmd, vpCmd, spCmd)
	return cmds, false
}

// Setter methods are defined in repl_state.go

// Setter/getter/state methods are defined in repl_state.go
