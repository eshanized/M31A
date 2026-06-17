package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// SubagentRow is the per-agent snapshot kept by the SubagentsModel.
// It mirrors subagent.SubagentInfo plus UI-only fields (expanded, lastUpdate).
type SubagentRow struct {
	Info      subagent.SubagentInfo
	LastEvent string // human-readable "Grep foo" / "Read x.go" / "done"
	Expanded  bool
	UpdatedAt time.Time
}

// SubagentsModel renders the parallel-subagent panel shown between the
// header and the REPL (or beside the REPL in wide layouts). It maintains
// an ordered list of rows keyed by agent ID; rows are appended as events
// arrive and are never reordered so the user's mental model stays stable.
type SubagentsModel struct {
	theme theme.Theme
	rows  []SubagentRow
	index map[string]int // id -> position in rows

	cursor int // highlighted row
	width  int
	height int // panel height budget (rows of text)
}

// NewSubagentsModel creates an empty panel.
func NewSubagentsModel(t theme.Theme) *SubagentsModel {
	return &SubagentsModel{
		theme: t,
		index: make(map[string]int),
	}
}

// GetStatus returns the total and active sub-agent counts.
func (m *SubagentsModel) GetStatus() (total, active int) {
	total = len(m.rows)
	for _, row := range m.rows {
		if row.Info.Status == "running" || row.Info.Status == "" {
			active++
		}
	}
	return
}

// SetTheme refreshes the theme.
func (m *SubagentsModel) SetTheme(t theme.Theme) { m.theme = t }

// SetSize updates the panel budget. The TUI calls this on WindowSizeMsg.
func (m *SubagentsModel) SetSize(width, height int) {
	m.width = width
	m.height = height
}

// ApplyEvent mutates the model in response to a subagent event.
// All calls are idempotent; out-of-order events simply overwrite state.
func (m *SubagentsModel) ApplyEvent(ev subagent.SubagentEvent) {
	switch ev.Type {
	case subagent.EventSpawned:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.Info.ID = ev.AgentID
			r.Info.Name = ev.Name
			r.Info.Status = subagent.StatusRunning
			r.Info.Worktree = ev.Worktree
			r.Info.StartedAt = ev.Timestamp
			r.LastEvent = "spawned"
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventToolStart:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.Info.LastToolName = ev.ToolName
			r.Info.LastToolStatus = "running"
			r.LastEvent = fmt.Sprintf("%s %s", ev.ToolName, abbrev(ev.ToolInput, 40))
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventToolDone:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.Info.LastToolName = ev.ToolName
			r.Info.LastToolStatus = "done"
			r.Info.ToolCalls++
			errTag := ""
			if ev.ToolError != "" {
				errTag = " ✗"
			}
			r.LastEvent = fmt.Sprintf("%s done%s (%dms)", ev.ToolName, errTag, ev.DurationMs)
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventTextDelta:
		// Don't spam the panel; only record that text is flowing.
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			if r.LastEvent == "" || r.LastEvent == "spawned" {
				r.LastEvent = "streaming…"
			}
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventThinking:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.LastEvent = "thinking…"
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventDone:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.Info.Status = subagent.StatusDone
			r.Info.ToolCalls = ev.ToolCalls
			r.Info.InputToks = ev.InputToks
			r.Info.OutputToks = ev.OutputToks
			r.Info.FinishedAt = ev.Timestamp
			r.Info.LastSummary = ev.Summary
			r.LastEvent = "done"
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventError:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.Info.Status = subagent.StatusError
			r.Info.LastError = ev.Error
			r.Info.FinishedAt = ev.Timestamp
			r.LastEvent = "error: " + abbrev(ev.Error, 60)
			r.UpdatedAt = ev.Timestamp
		})
	case subagent.EventCancelled:
		m.upsert(ev.AgentID, func(r *SubagentRow) {
			r.Info.Status = subagent.StatusCancel
			r.Info.FinishedAt = ev.Timestamp
			r.LastEvent = "cancelled"
			r.UpdatedAt = ev.Timestamp
		})
	}
}

// MoveCursor shifts the highlighted row by delta (clamped).
func (m *SubagentsModel) MoveCursor(delta int) {
	if len(m.rows) == 0 {
		return
	}
	m.cursor += delta
	if m.cursor < 0 {
		m.cursor = 0
	}
	if m.cursor >= len(m.rows) {
		m.cursor = len(m.rows) - 1
	}
}

// ToggleExpand flips the expanded state of the highlighted row.
func (m *SubagentsModel) ToggleExpand() {
	if len(m.rows) == 0 {
		return
	}
	m.rows[m.cursor].Expanded = !m.rows[m.cursor].Expanded
}

// Selected returns the currently-highlighted row (or nil if empty).
func (m *SubagentsModel) Selected() *SubagentRow {
	if len(m.rows) == 0 {
		return nil
	}
	return &m.rows[m.cursor]
}

// IsEmpty reports whether there are any rows.
func (m *SubagentsModel) IsEmpty() bool { return len(m.rows) == 0 }

// View renders the panel.
func (m *SubagentsModel) View() string {
	if m.IsEmpty() {
		return ""
	}
	t := m.theme

	headerStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(t.Primary).
		Padding(0, 1)
	rowStyle := lipgloss.NewStyle().PaddingLeft(2)
	selectedStyle := lipgloss.NewStyle().
		PaddingLeft(2).
		Background(t.SelectionBg).
		Foreground(t.Text)
	statusDone := lipgloss.NewStyle().Foreground(t.Success)
	statusRun := lipgloss.NewStyle().Foreground(t.Warning)
	statusErr := lipgloss.NewStyle().Foreground(t.Error)
	statusCancel := lipgloss.NewStyle().Foreground(t.TextMuted)
	dim := lipgloss.NewStyle().Foreground(t.TextMuted)

	var sb strings.Builder
	done := 0
	for _, r := range m.rows {
		if r.Info.Status == subagent.StatusDone {
			done++
		}
	}
	sb.WriteString(headerStyle.Render(fmt.Sprintf("▸ Subagents (%d/%d done)", done, len(m.rows))))
	sb.WriteString(dim.Render("   [↑/↓ nav · enter expand · ctrl+x hide]"))
	sb.WriteString("\n")

	maxRows := m.height - 2
	if maxRows < 3 {
		maxRows = 3
	}
	start := 0
	if m.cursor >= maxRows {
		start = m.cursor - maxRows + 1
	}
	end := start + maxRows
	if end > len(m.rows) {
		end = len(m.rows)
	}

	for i := start; i < end; i++ {
		r := m.rows[i]
		label := r.Info.Name
		if label == "" {
			label = r.Info.Description
		}
		if label == "" {
			label = r.Info.ID
		}
		status := renderStatus(r.Info.Status, statusDone, statusRun, statusErr, statusCancel)
		tokens := ""
		if r.Info.InputToks+r.Info.OutputToks > 0 {
			tokens = fmt.Sprintf("%d+%d toks", r.Info.InputToks, r.Info.OutputToks)
		}
		tools := ""
		if r.Info.ToolCalls > 0 {
			tools = fmt.Sprintf("%d tools", r.Info.ToolCalls)
		}
		marker := "▸"
		if r.Expanded {
			marker = "▾"
		}
		line := fmt.Sprintf("%s %-32s  %-8s  %-10s  %s",
			marker,
			abbrev(label, 32),
			status,
			tools,
			dim.Render(tokens),
		)
		if i == m.cursor {
			sb.WriteString(selectedStyle.Render(line))
		} else {
			sb.WriteString(rowStyle.Render(line))
		}
		sb.WriteString("\n")

		// Expanded body: last event + summary snippet.
		if r.Expanded {
			body := rowStyle.PaddingLeft(6).Render
			if r.LastEvent != "" {
				sb.WriteString(body(dim.Render("last: " + r.LastEvent)))
				sb.WriteString("\n")
			}
			if r.Info.LastSummary != "" {
				sb.WriteString(body(abbrev(r.Info.LastSummary, m.width-12)))
				sb.WriteString("\n")
			}
			if r.Info.LastError != "" {
				sb.WriteString(body(statusErr.Render("error: " + abbrev(r.Info.LastError, m.width-20))))
				sb.WriteString("\n")
			}
		}
	}
	return sb.String()
}

func renderStatus(s subagent.Status, done, run, err, cancel lipgloss.Style) string {
	switch s {
	case subagent.StatusDone:
		return done.Render("done")
	case subagent.StatusRunning:
		return run.Render("running")
	case subagent.StatusError:
		return err.Render("error")
	case subagent.StatusCancel:
		return cancel.Render("cancelled")
	default:
		return string(s)
	}
}

func abbrev(s string, n int) string {
	s = strings.TrimSpace(strings.ReplaceAll(s, "\n", " "))
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	if n < 1 {
		return ""
	}
	return string(r[:n-1]) + "…"
}

// upsert ensures a row exists for id and applies the mutator.
func (m *SubagentsModel) upsert(id string, mutate func(*SubagentRow)) {
	if idx, ok := m.index[id]; ok {
		mutate(&m.rows[idx])
		return
	}
	m.rows = append(m.rows, SubagentRow{Info: subagent.SubagentInfo{ID: id}})
	m.index[id] = len(m.rows) - 1
	mutate(&m.rows[len(m.rows)-1])
}
