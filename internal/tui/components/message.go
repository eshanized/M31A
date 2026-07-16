package components

import (
	"encoding/json"
	"strings"
	"time"

	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/glamour/ansi"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/types"
)

// GutterWidth is the fixed width for the role gutter.
// "┃ ◆  " ≈ 6 chars — compact brand icon + split border.
const GutterWidth = 6

// calcContentWidth returns the available content width, clamped to a minimum of 20 columns.
func calcContentWidth(width int) int {
	w := width - GutterWidth
	if w < 20 {
		w = 20
	}
	return w
}

type MessageRenderer struct {
	theme    theme.Theme
	styles   theme.SemanticStyles
	renderer *glamour.TermRenderer
	width    int

	// toolCallCache maps segment content string → pre-parsed ToolCall (TU-3 fix).
	// Avoids json.Unmarshal on every render for unchanged tool_use segments.
	toolCallCache map[string]*types.ToolCall

	// renderCache caches glamour-rendered output per (content, width).
	// Avoids re-rendering identical markdown on every frame.
	renderCache *GlamourCache

	// toolCardCollapsed tracks collapsed state per tool call ID (M8 progressive disclosure).
	// Persisted across renders so toggle/collapse state survives re-render.
	toolCardCollapsed map[string]bool
}

func NewMessageRenderer(t theme.Theme, width int) (*MessageRenderer, error) {
	mr := &MessageRenderer{
		theme:             t,
		styles:            theme.BuildSemanticStyles(t),
		width:             width,
		toolCallCache:     make(map[string]*types.ToolCall),
		renderCache:       NewGlamourCache(),
		toolCardCollapsed: make(map[string]bool),
	}
	if err := mr.createGlamourRenderer(); err != nil {
		return nil, err
	}
	return mr, nil
}

func (r *MessageRenderer) createGlamourRenderer() error {
	styleConfig := buildGlamourStyle(r.theme)
	renderer, err := glamour.NewTermRenderer(
		glamour.WithStyles(styleConfig),
		glamour.WithWordWrap(r.width-GutterWidth),
	)
	if err != nil {
		return err
	}
	r.renderer = renderer
	return nil
}

func strPtr(s string) *string { return &s }
func boolPtr(b bool) *bool    { return &b }

func buildGlamourStyle(t theme.Theme) ansi.StyleConfig {
	brand := string(t.Brand)
	text := string(t.TextPrimary)
	muted := string(t.TextMuted)
	bg := string(t.CodeBG)
	success := string(t.Success)
	errColor := string(t.Error)
	warning := string(t.Warning)
	secondary := string(t.Secondary)

	chromaTheme := "monokai"

	return ansi.StyleConfig{
		Document: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(text),
			},
		},
		Heading: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(brand),
				Bold:  boolPtr(true),
			},
		},
		H1: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(brand),
				Bold:  boolPtr(true),
			},
		},
		H2: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(brand),
				Bold:  boolPtr(true),
			},
		},
		H3: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(secondary),
				Bold:  boolPtr(true),
			},
		},
		Paragraph: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(text),
			},
		},
		Text: ansi.StylePrimitive{
			Color: strPtr(text),
		},
		Emph: ansi.StylePrimitive{
			Color:  strPtr(text),
			Italic: boolPtr(true),
		},
		Strong: ansi.StylePrimitive{
			Color: strPtr(text),
			Bold:  boolPtr(true),
		},
		HorizontalRule: ansi.StylePrimitive{
			Color: strPtr(muted),
		},
		BlockQuote: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(muted),
			},
			Indent:      uintPtr(2),
			IndentToken: strPtr("│ "),
		},
		List: ansi.StyleList{
			StyleBlock: ansi.StyleBlock{
				StylePrimitive: ansi.StylePrimitive{
					Color: strPtr(text),
				},
			},
			LevelIndent: 2,
		},
		Item: ansi.StylePrimitive{
			Color: strPtr(text),
		},
		Link: ansi.StylePrimitive{
			Color:     strPtr(brand),
			Underline: boolPtr(true),
		},
		LinkText: ansi.StylePrimitive{
			Color: strPtr(brand),
			Bold:  boolPtr(true),
		},
		Code: ansi.StyleBlock{
			StylePrimitive: ansi.StylePrimitive{
				Color:           strPtr(brand),
				BackgroundColor: strPtr(bg),
			},
		},
		CodeBlock: ansi.StyleCodeBlock{
			StyleBlock: ansi.StyleBlock{
				StylePrimitive: ansi.StylePrimitive{
					Color:           strPtr(text),
					BackgroundColor: strPtr(bg),
				},
				Margin: uintPtr(1),
			},
			Theme: chromaTheme,
			Chroma: &ansi.Chroma{
				GenericDeleted:  ansi.StylePrimitive{Color: strPtr(errColor)},
				GenericInserted: ansi.StylePrimitive{Color: strPtr(success)},
				GenericEmph:     ansi.StylePrimitive{Color: strPtr(warning), Italic: boolPtr(true)},
				GenericStrong:   ansi.StylePrimitive{Color: strPtr(text), Bold: boolPtr(true)},
			},
		},
		Strikethrough: ansi.StylePrimitive{
			CrossedOut: boolPtr(true),
		},
		Table: ansi.StyleTable{
			StyleBlock: ansi.StyleBlock{
				StylePrimitive: ansi.StylePrimitive{
					Color: strPtr(text),
				},
			},
		},
		DefinitionTerm: ansi.StylePrimitive{
			Color: strPtr(brand),
			Bold:  boolPtr(true),
		},
		DefinitionDescription: ansi.StylePrimitive{
			Color: strPtr(text),
		},
		Task: ansi.StyleTask{
			StylePrimitive: ansi.StylePrimitive{
				Color: strPtr(text),
			},
			Ticked:   "[✓] ",
			Unticked: "[ ] ",
		},
	}
}

func uintPtr(u uint) *uint { return &u }

func (r *MessageRenderer) SetWidth(width int) error {
	if width == r.width {
		return nil
	}
	r.width = width
	r.renderCache.Clear()
	_ = r.renderer.Close()
	return r.createGlamourRenderer()
}

// RenderMessage renders a message with a role gutter and optional timestamp.
func (r *MessageRenderer) RenderMessage(msg types.Message, width int) string {
	switch msg.Role {
	case "user":
		return r.renderUserMessage(msg, width)
	case "assistant":
		return r.renderAssistantMessage(msg, width)
	default:
		return r.renderAssistantMessage(msg, width)
	}
}

// RenderTimestampBar renders a compact timestamp between conversation turns.
// Format: ── 15:04 ──
func RenderTimestampBar(t theme.Theme, ts time.Time, width int) string {
	return RenderTimestampBarWithSummary(t, ts, width, "")
}

// RenderTimestampBarWithSummary renders a compact timestamp separator with an
// optional trailing summary suffix.
func RenderTimestampBarWithSummary(t theme.Theme, ts time.Time, width int, summary string) string {
	if ts.IsZero() {
		return ""
	}
	timeStr := ts.Format("15:04")
	s := theme.BuildSemanticStyles(t)

	if summary == "" {
		return s.Faint.Render("── " + timeStr + " ──")
	}

	return s.Faint.Render("── " + timeStr + " ── " + summary)
}

// renderUserMessage renders a user message with a distinct right-aligned badge.
//
// Layout:
//
//	┃ ● you
//	┃   <content>
func (r *MessageRenderer) renderUserMessage(msg types.Message, width int) string {
	s := r.styles
	t := r.theme
	contentWidth := calcContentWidth(width)

	borderChar := s.SecondaryText.Render("┃")

	roleBadge := lipgloss.NewStyle().
		Foreground(t.Surface).
		Background(t.Secondary).
		Bold(true).
		Padding(0, 1).
		Render("You")
	gutter := lipgloss.JoinHorizontal(lipgloss.Top, borderChar, " ", roleBadge)

	if msg.Content == "" {
		return ""
	}

	contentStyle := s.Body.
		Background(t.Surface).
		PaddingLeft(2).
		Width(contentWidth - 2).
		MaxWidth(contentWidth - 2)

	contentLine := lipgloss.JoinHorizontal(lipgloss.Top,
		s.SecondaryText.Render("┃"),
		contentStyle.Render(msg.Content),
	)

	return lipgloss.JoinVertical(lipgloss.Top, gutter, contentLine)
}

// renderAssistantMessage renders assistant content with a compact left border.
//
// Layout:
//
//	┃ ◆ assistant
//	  <segments...>
//
// Tool-only messages (no text content) are collapsed into a single summary line.
func (r *MessageRenderer) renderAssistantMessage(msg types.Message, width int) string {
	s := r.styles
	t := r.theme
	contentWidth := calcContentWidth(width)

	// Ambient border temperature: gutter color reflects message content
	gutterColor := t.Brand
	for _, seg := range msg.Segments {
		if seg.Type == "error" {
			gutterColor = t.Error
			break
		}
	}

	borderChar := lipgloss.NewStyle().Foreground(gutterColor).Render("┃")
	roleLabel := lipgloss.NewStyle().Foreground(gutterColor).Bold(true).Render(" ◆")
	gutter := lipgloss.JoinHorizontal(lipgloss.Top, borderChar, roleLabel)

	// Render segments
	var contentSegments []string
	var toolSegments []string
	hasContent := false

	if len(msg.Segments) == 0 && msg.Content != "" {
		stripped := StripANSI(msg.Content)
		contentSegments = append(contentSegments, r.renderContentSegment(stripped, contentWidth))
		hasContent = true
	} else {
		for _, seg := range msg.Segments {
			switch seg.Type {
			case "content":
				stripped := StripANSI(seg.Content)
				rendered := r.renderContentSegment(stripped, contentWidth)
				if rendered != "" {
					contentSegments = append(contentSegments, rendered)
					hasContent = true
				}
			case "thinking":
				tb := NewThinkingBlock(seg, t, true, 0)
				contentSegments = append(contentSegments, tb.Render(contentWidth))
				hasContent = true
			case "tool_use":
				tc, ok := r.toolCallCache[seg.Content]
				if !ok {
					var parsed types.ToolCall
					if err := json.Unmarshal([]byte(seg.Content), &parsed); err == nil {
						tc = &parsed
						r.toolCallCache[seg.Content] = tc
					}
				}
				if tc != nil {
					toolSegments = append(toolSegments, tc.Name)
					if hasContent {
						card := NewToolCard(*tc, nil, ToolRunning, t)
						// M8: Apply persisted collapsed state by tool call ID
						if tc.ID != "" && r.toolCardCollapsed[tc.ID] {
							card.SetCollapsed(true)
						}
						contentSegments = append(contentSegments, card.Render(contentWidth))
					}
				}
			case "error":
				contentSegments = append(contentSegments, r.renderErrorSegment(seg.Content, contentWidth))
				hasContent = true
			}
		}
	}

	if len(msg.ToolCalls) > 0 {
		for _, tc := range msg.ToolCalls {
			toolSegments = append(toolSegments, tc.Name)
			if hasContent {
				card := NewToolCard(tc, nil, ToolRunning, t)
				// M8: Apply persisted collapsed state by tool call ID
				if tc.ID != "" && r.toolCardCollapsed[tc.ID] {
					card.SetCollapsed(true)
				}
				contentSegments = append(contentSegments, card.Render(contentWidth))
			}
		}
	}

	// Tool-only messages: render each tool as a proper inline card
	if !hasContent && len(toolSegments) > 0 {
		// Re-parse tool_use segments and render as cards
		var toolCards []string
		for _, seg := range msg.Segments {
			if seg.Type == "tool_use" {
				tc, ok := r.toolCallCache[seg.Content]
				if !ok {
					var parsed types.ToolCall
					if err := json.Unmarshal([]byte(seg.Content), &parsed); err == nil {
						tc = &parsed
						r.toolCallCache[seg.Content] = tc
					}
				}
				if tc != nil {
					card := NewToolCard(*tc, nil, ToolRunning, t)
					// M8: Apply persisted collapsed state
					if tc.ID != "" && r.toolCardCollapsed[tc.ID] {
						card.SetCollapsed(true)
					}
					toolCards = append(toolCards, card.Render(contentWidth))
				}
			}
		}
		if len(toolCards) > 0 {
			return strings.Join(toolCards, "\n")
		}
		// Fallback: just show the badges
		toolBadges := strings.Join(toolSegments, " ")
		summary := s.Faint.Render("  " + toolBadges)
		return lipgloss.JoinVertical(lipgloss.Top, gutter, summary)
	}

	if len(contentSegments) == 0 {
		return ""
	}

	// Join content lines with left border
	content := lipgloss.JoinVertical(lipgloss.Top, contentSegments...)
	contentLines := strings.Split(content, "\n")
	borderedLines := make([]string, len(contentLines))
	for i, line := range contentLines {
		borderedLines[i] = lipgloss.JoinHorizontal(lipgloss.Top,
			lipgloss.NewStyle().Foreground(gutterColor).Render("┃"),
			lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-2).Render(line),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top, gutter, strings.Join(borderedLines, "\n"))
}

func (r *MessageRenderer) renderContentSegment(content string, width int) string {
	if content == "" {
		return ""
	}

	// Strip any ANSI escape codes before glamour to prevent mangling
	content = StripANSI(content)

	// Special-case: iteration chips — "iter:N:tools:X,Y,Z"
	if strings.HasPrefix(content, "iter:") {
		if out := r.renderIterationChips(content, width); out != "" {
			return out
		}
	}

	// Special-case: "Agent iteration N — tools: X, Y" → card + tool chips.
	if strings.HasPrefix(content, "**Agent iteration") {
		if out := r.renderAgentIteration(content, width); out != "" {
			return out
		}
	}

	// Check render cache before calling glamour
	if cached, ok := r.renderCache.Get(content, width); ok {
		return cached
	}

	rendered, err := r.renderer.Render(content)
	if err != nil {
		return r.styles.Body.Width(width).PaddingLeft(2).Render(content)
	}

	result := lipgloss.NewStyle().
		Width(width).
		PaddingLeft(2).
		Render(rendered)

	r.renderCache.Set(content, width, result)
	return result
}

// renderAgentIteration renders an agent-loop iteration as a compact inline line.
//
//	³ FileRead FileRead FileRead
//
// Input format: "**Agent iteration N** — tools: X, Y, Z"
// Returns "" if the content doesn't match the expected format.
func (r *MessageRenderer) renderAgentIteration(content string, width int) string {
	idx := strings.Index(content, "** — tools: ")
	if idx < 0 {
		return ""
	}
	toolsList := strings.TrimSpace(content[idx+len("** — tools: "):])
	if toolsList == "" {
		return ""
	}

	toolNames := strings.Split(toolsList, ", ")
	var badges []string
	for _, name := range toolNames {
		name = strings.TrimSpace(name)
		if name == "" {
			continue
		}
		badges = append(badges, name)
	}
	if len(badges) == 0 {
		return ""
	}

	// Extract iteration number
	title := strings.TrimPrefix(content[:idx], "**")
	iterNum := strings.TrimPrefix(title, "Agent iteration ")

	// Compact single-line: superscript iteration number + tool names
	superscript := toSuperscript(iterNum)

	line := r.styles.Faint.PaddingLeft(2).
		Render(superscript + " " + strings.Join(badges, " "))

	return lipgloss.NewStyle().Width(width).Render(line)
}

// renderIterationChips renders "iter:N:chips:Name|input,Name|input" as a compact line:
//
//	⚡⁴  FileRead commands.go · FileRead app.go · Bash go test
func (r *MessageRenderer) renderIterationChips(content string, width int) string {
	s := r.styles
	t := r.theme

	// Parse format: "iter:N:chips:..." or legacy "iter:N:tools:..."
	parts := strings.SplitN(content, ":", 4)
	if len(parts) < 4 || parts[0] != "iter" {
		return ""
	}
	iterNum := parts[1]
	format := parts[2]
	data := parts[3]
	if data == "" {
		return ""
	}

	// Iteration badge: ⚡ with superscript number
	superscript := toSuperscript(iterNum)
	badge := s.BrandBold.Render("⚡" + superscript)

	var chips []string

	if format == "chips" {
		// New format: "Name|input,Name|input"
		entries := strings.Split(data, ",")
		for _, entry := range entries {
			entry = strings.TrimSpace(entry)
			if entry == "" {
				continue
			}
			nameInput := strings.SplitN(entry, "|", 2)
			name := nameInput[0]
			input := ""
			if len(nameInput) > 1 {
				input = nameInput[1]
			}

			// Tool name badge
			chipStyle, ok := t.ToolLabel[name]
			if !ok {
				chipStyle = s.Muted.Bold(true)
			}
			chip := chipStyle.Render(name)
			if input != "" {
				chip += " " + s.Muted.Render(input)
			}
			chips = append(chips, chip)
		}
	} else {
		// Legacy format: "Name,Name,Name"
		toolNames := strings.Split(data, ",")
		for _, name := range toolNames {
			name = strings.TrimSpace(name)
			if name == "" {
				continue
			}
			chipStyle, ok := t.ToolLabel[name]
			if !ok {
				chipStyle = s.Muted.Bold(true)
			}
			chips = append(chips, chipStyle.Render(name))
		}
	}

	if len(chips) == 0 {
		return ""
	}

	// Join chips with separator
	sep := s.Muted.Render(" · ")
	chipsStr := strings.Join(chips, sep)
	line := badge + "  " + chipsStr

	return lipgloss.NewStyle().PaddingLeft(2).Width(width).Render(line)
}

// toSuperscript converts a string of digits to superscript characters.
func toSuperscript(s string) string {
	var result strings.Builder
	for _, ch := range s {
		switch ch {
		case '0':
			result.WriteString("⁰")
		case '1':
			result.WriteString("¹")
		case '2':
			result.WriteString("²")
		case '3':
			result.WriteString("³")
		case '4':
			result.WriteString("⁴")
		case '5':
			result.WriteString("⁵")
		case '6':
			result.WriteString("⁶")
		case '7':
			result.WriteString("⁷")
		case '8':
			result.WriteString("⁸")
		case '9':
			result.WriteString("⁹")
		default:
			result.WriteRune(ch)
		}
	}
	return result.String()
}

// renderErrorSegment styles an error banner directly via lipgloss, bypassing
// glamour so that plain-text error content (with ✗/⚠ glyphs) is rendered
// without the markdown pipeline mangling any escape sequences.
func (r *MessageRenderer) renderErrorSegment(content string, width int) string {
	if content == "" {
		return ""
	}
	styled := r.styles.ErrorText.Bold(true).Render(content)
	return lipgloss.NewStyle().
		Width(width).
		PaddingLeft(2).
		Render(styled)
}

// FormatTimeBar formats a timestamp for use in timestamp bars.
func FormatTimeBar(ts time.Time) string {
	if ts.IsZero() {
		return ""
	}
	return ts.Format("15:04")
}

// WrapWithGutter wraps content with a gutter character on each line.
func WrapWithGutter(content string, gutterChar lipgloss.Style, width int) string {
	lines := strings.Split(content, "\n")
	result := make([]string, len(lines))
	contentWidth := calcContentWidth(width)
	for i, line := range lines {
		result[i] = lipgloss.JoinHorizontal(lipgloss.Top,
			gutterChar.Render("┃"),
			lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-4).Render(line),
		)
	}
	return strings.Join(result, "\n")
}

// ─── M8: Tool card collapsed state management ────────────────────────────────

// SetToolCardCollapsed sets the collapsed state for a tool call by ID (M8).
func (r *MessageRenderer) SetToolCardCollapsed(toolID string, collapsed bool) {
	r.toolCardCollapsed[toolID] = collapsed
}

// IsToolCardCollapsed returns the collapsed state for a tool call by ID (M8).
func (r *MessageRenderer) IsToolCardCollapsed(toolID string) bool {
	return r.toolCardCollapsed[toolID]
}

// SetAllToolCardsCollapsed sets collapsed state for all tool cards (M8).
func (r *MessageRenderer) SetAllToolCardsCollapsed(collapsed bool) {
	for id := range r.toolCardCollapsed {
		r.toolCardCollapsed[id] = collapsed
	}
}

// ResetToolCardCollapsed clears all collapsed state (M8).
func (r *MessageRenderer) ResetToolCardCollapsed() {
	r.toolCardCollapsed = make(map[string]bool)
}
