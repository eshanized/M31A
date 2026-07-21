package components

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ScrollState manages cursor and scroll offset for list views.
// It provides consistent scroll behavior across all list-based screens.
type ScrollState struct {
	Cursor       int // Current cursor position (0-indexed)
	Offset       int // First visible item index
	Total        int // Total number of items
	VisibleRows  int // Number of visible rows (height - chrome)
	ChromeHeight int // Rows consumed by title, footer, margins
	MinHeight    int // Minimum height guard
}

// NewScrollState creates a new ScrollState with the given parameters.
func NewScrollState(total, height, chromeHeight int) ScrollState {
	visibleRows := height - chromeHeight
	if visibleRows < 1 {
		visibleRows = 1
	}

	return ScrollState{
		Total:        total,
		VisibleRows:  visibleRows,
		ChromeHeight: chromeHeight,
		MinHeight:    chromeHeight + 1,
	}
}

// ClampScroll adjusts cursor and offset to valid positions.
// Call this after any cursor movement or window resize.
func (s *ScrollState) ClampScroll() {
	if s.Total == 0 {
		s.Cursor = 0
		s.Offset = 0
		return
	}

	// Clamp cursor to valid range
	if s.Cursor < 0 {
		s.Cursor = 0
	}
	if s.Cursor >= s.Total {
		s.Cursor = s.Total - 1
	}

	// Ensure cursor is visible
	if s.Cursor < s.Offset {
		s.Offset = s.Cursor
	}
	if s.Cursor >= s.Offset+s.VisibleRows {
		s.Offset = s.Cursor - s.VisibleRows + 1
	}

	// Clamp offset
	if s.Offset < 0 {
		s.Offset = 0
	}
	maxOffset := s.Total - s.VisibleRows
	if maxOffset < 0 {
		maxOffset = 0
	}
	if s.Offset > maxOffset {
		s.Offset = maxOffset
	}
}

// MoveCursor moves the cursor by delta positions and clamps.
func (s *ScrollState) MoveCursor(delta int) {
	s.Cursor += delta
	s.ClampScroll()
}

// MoveToStart moves the cursor to the first item.
func (s *ScrollState) MoveToStart() {
	s.Cursor = 0
	s.ClampScroll()
}

// MoveToEnd moves the cursor to the last item.
func (s *ScrollState) MoveToEnd() {
	s.Cursor = s.Total - 1
	s.ClampScroll()
}

// PageUp moves the cursor up by one page.
func (s *ScrollState) PageUp() {
	s.Cursor -= s.VisibleRows
	s.ClampScroll()
}

// PageDown moves the cursor down by one page.
func (s *ScrollState) PageDown() {
	s.Cursor += s.VisibleRows
	s.ClampScroll()
}

// VisibleRange returns the range of visible item indices [start, end).
func (s ScrollState) VisibleRange() (start, end int) {
	start = s.Offset
	end = s.Offset + s.VisibleRows
	if end > s.Total {
		end = s.Total
	}
	return
}

// ScrollIndicator renders a position indicator like "3/15".
type ScrollIndicator struct {
	Cursor      int
	Total       int
	Theme       theme.Theme
	PaddingLeft int
}

// Render returns the styled scroll indicator.
func (si ScrollIndicator) Render() string {
	t := si.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	padLeft := si.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	if si.Total == 0 {
		return lipgloss.NewStyle().
			Foreground(t.TextMuted).
			PaddingLeft(padLeft).
			Render("0/0")
	}

	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		PaddingLeft(padLeft).
		Render(fmt.Sprintf("%d/%d", si.Cursor+1, si.Total))
}
