package components

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// Notification represents a stored notification.
type Notification struct {
	ID        int
	Text      string
	Type      string // "info", "success", "warning", "error"
	Timestamp time.Time
}

// NotificationList renders a scrollable notification history.
type NotificationList struct {
	Items  []Notification
	Cursor int
	Offset int
	Height int
	Theme  theme.Theme
	Width  int
	NextID int
}

// Add appends a notification.
func (nl *NotificationList) Add(text, ntype string) {
	nl.Items = append(nl.Items, Notification{
		ID:        nl.NextID,
		Text:      text,
		Type:      ntype,
		Timestamp: time.Now(),
	})
	nl.NextID++
	if nl.Cursor >= len(nl.Items) {
		nl.Cursor = len(nl.Items) - 1
	}
}

// MoveCursor moves up/down.
func (nl *NotificationList) MoveCursor(delta int) {
	nl.Cursor += delta
	if nl.Cursor < 0 {
		nl.Cursor = 0
	}
	if nl.Cursor >= len(nl.Items) {
		nl.Cursor = len(nl.Items) - 1
	}
}

// View renders the notification list.
func (nl *NotificationList) View() string {
	t := nl.Theme
	s := theme.BuildSemanticStyles(t)

	if len(nl.Items) == 0 {
		return s.ListEmpty.PaddingLeft(2).Render("No notifications.")
	}

	end := nl.Offset + nl.Height
	if end > len(nl.Items) {
		end = len(nl.Items)
	}

	var lines []string
	for i := nl.Offset; i < end; i++ {
		item := nl.Items[i]
		selected := i == nl.Cursor

		var icon string
		var iconStyle lipgloss.Style
		switch item.Type {
		case "success":
			icon = "✓"
			iconStyle = s.ToastSuccess
		case "warning":
			icon = "⚠"
			iconStyle = s.ToastWarning
		case "error":
			icon = "✗"
			iconStyle = s.ToastError
		default:
			icon = "ℹ"
			iconStyle = s.ToastInfo
		}

		timeStr := item.Timestamp.Format("15:04:05")
		iconStyled := iconStyle.Render(icon)
		timeStyled := s.NotificationTime.Width(10).Render(timeStr)
		textStyle := s.NotificationText
		if selected {
			textStyle = s.NotificationSelected
		}
		textStyled := textStyle.Render(item.Text)

		prefix := "  "
		if selected {
			prefix = s.Cursor.Render("▶ ")
		}

		lines = append(lines, fmt.Sprintf("%s%s %s %s", prefix, iconStyled, timeStyled, textStyled))
	}

	info := s.Muted.PaddingLeft(2).
		Render(fmt.Sprintf("%d/%d notifications", nl.Cursor+1, len(nl.Items)))
	lines = append(lines, "", info)

	return strings.Join(lines, "\n")
}
