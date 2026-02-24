package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Alignment for DataTable columns.
type Alignment int

const (
	AlignLeft Alignment = iota
	AlignRight
	AlignCenter
)

// Column defines a DataTable column.
type Column struct {
	Header string
	Width  int
	Align  Alignment
}

// DataTable is a sortable, scrollable table.
type DataTable struct {
	Columns  []Column
	Rows     [][]string
	SortCol  int
	SortAsc  bool
	Cursor   int
	Offset   int
	VisibleH int
	Theme    theme.Theme
	Width    int
}

// NewDataTable creates a DataTable.
func NewDataTable(cols []Column, rows [][]string, t theme.Theme, w, h int) *DataTable {
	return &DataTable{
		Columns:  cols,
		Rows:     rows,
		SortCol:  -1,
		SortAsc:  true,
		VisibleH: h,
		Theme:    t,
		Width:    w,
	}
}

// MoveCursor moves the cursor and adjusts scroll.
func (dt *DataTable) MoveCursor(delta int) {
	dt.Cursor += delta
	if dt.Cursor < 0 {
		dt.Cursor = 0
	}
	if dt.Cursor >= len(dt.Rows) {
		dt.Cursor = len(dt.Rows) - 1
	}
	dt.clampScroll()
}

func (dt *DataTable) clampScroll() {
	if dt.Cursor < dt.Offset {
		dt.Offset = dt.Cursor
	}
	if dt.Cursor >= dt.Offset+dt.VisibleH {
		dt.Offset = dt.Cursor - dt.VisibleH + 1
	}
}

// View renders the table.
func (dt *DataTable) View() string {
	t := dt.Theme

	if len(dt.Rows) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No data.")
	}

	var lines []string

	header := dt.renderHeader()
	lines = append(lines, header)
	lines = append(lines, dt.renderDivider())

	end := dt.Offset + dt.VisibleH
	if end > len(dt.Rows) {
		end = len(dt.Rows)
	}

	for i := dt.Offset; i < end; i++ {
		selected := i == dt.Cursor
		lines = append(lines, dt.renderRow(dt.Rows[i], selected))
	}

	info := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render(fmt.Sprintf("%d/%d rows", dt.Cursor+1, len(dt.Rows)))
	lines = append(lines, "", info)

	return strings.Join(lines, "\n")
}

func (dt *DataTable) renderHeader() string {
	t := dt.Theme
	var cells []string
	for i, col := range dt.Columns {
		hdr := col.Header
		if i == dt.SortCol {
			arrow := "▲"
			if !dt.SortAsc {
				arrow = "▼"
			}
			hdr += " " + arrow
		}
		styled := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			Bold(true).
			Width(col.Width).
			Render(hdr)
		cells = append(cells, "  "+styled)
	}
	return strings.Join(cells, "")
}

func (dt *DataTable) renderDivider() string {
	t := dt.Theme
	totalW := 0
	for _, col := range dt.Columns {
		totalW += col.Width + 2
	}
	return lipgloss.NewStyle().Foreground(t.Border).
		Render(strings.Repeat("─", totalW))
}

func (dt *DataTable) renderRow(row []string, selected bool) string {
	t := dt.Theme
	var cells []string
	for i, col := range dt.Columns {
		val := ""
		if i < len(row) {
			val = row[i]
		}
		if len(val) > col.Width {
			val = val[:col.Width-1] + "…"
		}
		styled := lipgloss.NewStyle().Width(col.Width)
		if selected {
			styled = styled.Foreground(t.Brand).Bold(true)
		} else {
			styled = styled.Foreground(t.Text)
		}
		cells = append(cells, "  "+styled.Render(val))
	}
	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}
	return prefix + strings.Join(cells, "")
}
