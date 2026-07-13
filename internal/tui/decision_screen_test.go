package tui

import (
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/decision"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ═══ Constructor ═══

func TestNewDecisionScreen(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 100, 40)
	if ds == nil {
		t.Fatal("NewDecisionScreen returned nil")
	}
	if ds.width != 100 {
		t.Errorf("width = %d, want 100", ds.width)
	}
	if ds.height != 40 {
		t.Errorf("height = %d, want 40", ds.height)
	}
	if ds.decisions != nil {
		t.Error("decisions should be nil initially")
	}
}

// ═══ Init ═══

func TestDecisionScreen_Init(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)
	if cmd := ds.Init(); cmd != nil {
		t.Error("Init should return nil")
	}
}

// ═══ SetDecisions ═══

func TestDecisionScreen_SetDecisions(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	if ds.decisions != nil {
		t.Fatal("decisions should be nil initially")
	}

	now := time.Now()
	receipts := []decision.DecisionReceipt{
		{
			Timestamp: now,
			Decision:  "use bash tool",
			Category:  decision.CategoryTool,
			Cost:      decision.Cost{Tokens: 100, Duration: 0.5},
		},
		{
			Timestamp: now.Add(time.Second),
			Decision:  "retry with model B",
			Category:  decision.CategoryRetry,
			Cost:      decision.Cost{Tokens: 50, Duration: 0.2},
		},
	}

	ds.SetDecisions(receipts)
	if len(ds.decisions) != 2 {
		t.Errorf("decisions length = %d, want 2", len(ds.decisions))
	}
	if ds.decisions[0].Decision != "use bash tool" {
		t.Errorf("first decision = %q, want %q", ds.decisions[0].Decision, "use bash tool")
	}

	// Overwrite with empty
	ds.SetDecisions(nil)
	if len(ds.decisions) != 0 {
		t.Errorf("decisions should be empty after nil set, got %d", len(ds.decisions))
	}
}

// ═══ SetDimensions ═══

func TestDecisionScreen_SetDimensions(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	ds.SetDimensions(120, 40)
	if ds.width != 120 {
		t.Errorf("width = %d, want 120", ds.width)
	}
	if ds.height != 40 {
		t.Errorf("height = %d, want 40", ds.height)
	}
}

// ═══ SetTheme ═══

func TestDecisionScreen_SetTheme(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	dark := theme.Dark()
	ds.SetTheme(dark)
	// Verify SetTheme doesn't panic and the screen still renders
	_ = ds.View()
}

// ═══ View ═══

func TestDecisionScreen_ViewEmpty(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	view := ds.View()
	if view == "" {
		t.Error("View should not be empty for empty decisions (should show empty state)")
	}
	if !strings.Contains(view, "Decisions") {
		t.Error("empty state should contain 'Decisions' title")
	}
}

func TestDecisionScreen_ViewPopulated(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	now := time.Now()
	receipts := []decision.DecisionReceipt{
		{
			Timestamp: now,
			Decision:  "use bash tool",
			Category:  decision.CategoryTool,
			Cost:      decision.Cost{Tokens: 100, Duration: 0.5},
		},
		{
			Timestamp: now.Add(time.Second),
			Decision:  "retry with model B",
			Category:  decision.CategoryRetry,
			Cost:      decision.Cost{Tokens: 50, Duration: 0.2},
		},
	}
	ds.SetDecisions(receipts)

	view := ds.View()
	if view == "" {
		t.Error("View should not be empty with populated decisions")
	}
	if !strings.Contains(view, "Decision Log") {
		t.Error("populated view should contain 'Decision Log' header")
	}
	if !strings.Contains(view, "use bash tool") {
		t.Error("populated view should contain decision text")
	}
	if !strings.Contains(view, "2 decisions") {
		t.Error("populated view should contain summary with count")
	}
}

func TestDecisionScreen_ViewLongDecisionText(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	longText := strings.Repeat("x", 100)
	receipts := []decision.DecisionReceipt{
		{
			Timestamp: time.Now(),
			Decision:  longText,
			Category:  decision.CategoryTool,
			Cost:      decision.Cost{Tokens: 10, Duration: 0.1},
		},
	}
	ds.SetDecisions(receipts)

	view := ds.View()
	if view == "" {
		t.Error("View should not be empty with long decision text")
	}
	// Should contain truncated text with "..."
	if !strings.Contains(view, "...") {
		t.Error("long decision text should be truncated")
	}
}

// ═══ Update — key handling ═══

func TestDecisionScreen_UpdateEsc(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	msg := tea.KeyMsg{Type: tea.KeyEscape}
	_, cmd := ds.Update(msg)
	if cmd == nil {
		t.Fatal("esc should return a command")
	}
	msgResult := cmd()
	if _, ok := msgResult.(PopScreenMsg); !ok {
		t.Errorf("esc should return PopScreenMsg, got %T", msgResult)
	}
}

func TestDecisionScreen_UpdateQ(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'q'}}
	_, cmd := ds.Update(msg)
	if cmd == nil {
		t.Fatal("q should return a command")
	}
	msgResult := cmd()
	if _, ok := msgResult.(PopScreenMsg); !ok {
		t.Errorf("q should return PopScreenMsg, got %T", msgResult)
	}
}

func TestDecisionScreen_UpdateWindowSize(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	msg := tea.WindowSizeMsg{Width: 120, Height: 40}
	result, cmd := ds.Update(msg)
	if cmd != nil {
		t.Error("WindowSizeMsg should not return a command")
	}
	if result == nil {
		t.Fatal("Update should return a non-nil result")
	}
	updated, ok := result.(*DecisionScreen)
	if !ok {
		t.Fatalf("result should be *DecisionScreen, got %T", result)
	}
	if updated.width != 120 || updated.height != 40 {
		t.Errorf("dimensions = %dx%d, want 120x40", updated.width, updated.height)
	}
}

func TestDecisionScreen_UpdateOtherKey(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	// 'a' is not handled — should return nil cmd
	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}}
	_, cmd := ds.Update(msg)
	if cmd != nil {
		t.Error("unhandled key should return nil cmd")
	}
}

// ═══ View — category styling ═══

func TestDecisionScreen_ViewAllCategories(t *testing.T) {
	t.Parallel()
	ds := NewDecisionScreen(testTheme(), 80, 24)

	categories := []decision.Category{
		decision.CategoryTool,
		decision.CategoryPlan,
		decision.CategoryIntent,
		decision.CategoryRetry,
		decision.CategoryStrategy,
		decision.CategoryModel,
	}

	var receipts []decision.DecisionReceipt
	for i, cat := range categories {
		receipts = append(receipts, decision.DecisionReceipt{
			Timestamp: time.Now().Add(time.Duration(i) * time.Second),
			Decision:  "decision for " + string(cat),
			Category:  cat,
			Cost:      decision.Cost{Tokens: 10, Duration: 0.1},
		})
	}
	ds.SetDecisions(receipts)

	view := ds.View()
	if view == "" {
		t.Error("View should not be empty")
	}
	if !strings.Contains(view, "6 decisions") {
		t.Error("summary should show 6 decisions")
	}
}
