package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

// --- bisect_model.go tests ---

func testCommits(n int) []bisectCommit {
	commits := make([]bisectCommit, n)
	for i := range commits {
		commits[i] = bisectCommit{
			Hash:    strings.Repeat("a", 40),
			Message: "commit message",
			Status:  "pending",
		}
	}
	return commits
}

func TestSetCommits(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(10)
	bm.SetCommits(commits)
	if bm.total != 10 {
		t.Errorf("total = %d, want 10", bm.total)
	}
	if bm.current != 5 {
		t.Errorf("current = %d, want 5 (midpoint)", bm.current)
	}
	if bm.status != "testing" {
		t.Errorf("status = %q, want testing", bm.status)
	}
	if bm.commits[5].Status != "testing" {
		t.Errorf("midpoint status = %q, want testing", bm.commits[5].Status)
	}
}

func TestSetCommits_SingleCommit(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(1))
	if bm.current != 0 {
		t.Errorf("current = %d, want 0", bm.current)
	}
	if bm.commits[0].Status != "testing" {
		t.Errorf("single commit status = %q, want testing", bm.commits[0].Status)
	}
}

func TestBisectModel_MarkCurrent(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	bm.markCurrent("good")
	if bm.commits[2].Status != "good" {
		t.Errorf("commit 2 status = %q, want good", bm.commits[2].Status)
	}
}

func TestBisectModel_MarkCurrent_OutOfBounds(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(3))
	bm.current = 10        // out of bounds
	bm.markCurrent("good") // should not panic
}

func TestBisectModel_Advance_AllPending(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	// All pending, advance should go to midpoint
	bm.advance()
	if bm.current != 2 {
		t.Errorf("advance from all-pending: current = %d, want 2", bm.current)
	}
}

func TestBisectModel_Advance_FirstGood(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(5)
	bm.SetCommits(commits)
	bm.commits[0].Status = "good"
	bm.advance()
	// low=0 (first good at 0), high=4, midpoint = 0 + (4-0)/2 = 2
	if bm.current != 2 {
		t.Errorf("advance with first good: current = %d, want 2", bm.current)
	}
}

func TestBisectModel_Advance_Converge(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(5)
	bm.SetCommits(commits)
	// Simulate: good at 0, bad at 4
	bm.commits[0].Status = "good"
	bm.commits[4].Status = "bad"
	bm.advance()
	// low=0, high=4, current=2
	if bm.current != 2 {
		t.Errorf("converge: current = %d, want 2", bm.current)
	}
	// Now mark good at 2
	bm.commits[2].Status = "good"
	bm.advance()
	// low=2, high=4, current=3
	if bm.current != 3 {
		t.Errorf("converge step 2: current = %d, want 3", bm.current)
	}
	// After marking 3 as bad, low=2, high=3 → low >= high-1 → done
	bm.commits[3].Status = "bad"
	bm.advance()
	if bm.status != "done" {
		t.Errorf("status = %q, want done", bm.status)
	}
}

func TestBisectModel_Reset(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	bm.markCurrent("good")
	bm.markCurrent("bad")
	bm.reset()
	// Midpoint gets "testing", all others get "pending"
	for i, c := range bm.commits {
		if i == bm.current {
			if c.Status != "testing" {
				t.Errorf("commit %d (midpoint) status = %q, want testing", i, c.Status)
			}
		} else {
			if c.Status != "pending" {
				t.Errorf("commit %d status = %q, want pending after reset", i, c.Status)
			}
		}
	}
	if bm.status != "testing" {
		t.Errorf("status = %q, want testing after reset", bm.status)
	}
}

func TestBisectModel_UpdateWindowSize(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	model, _ := bm.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	bm2 := model.(*BisectModel)
	if bm2.width != 120 || bm2.height != 40 {
		t.Errorf("dimensions = %dx%d, want 120x40", bm2.width, bm2.height)
	}
}

func TestBisectModel_UpdateKey_Good(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'g'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "good" {
		t.Errorf("g key: status = %q, want good", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Bad(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'b'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "bad" {
		t.Errorf("b key: status = %q, want bad", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Skip(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "skip" {
		t.Errorf("s key: status = %q, want skip", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Reset(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	bm.markCurrent("good")
	model, _ := bm.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	bm2 := model.(*BisectModel)
	if bm2.commits[2].Status != "testing" {
		t.Errorf("r key: status = %q, want testing", bm2.commits[2].Status)
	}
}

func TestBisectModel_UpdateKey_Esc(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	_, cmd := bm.Update(tea.KeyMsg{Type: tea.KeyEscape})
	if cmd == nil {
		t.Error("esc key should return a command")
	}
}

func TestBisectModel_View_Empty(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	view := bm.View()
	if !strings.Contains(view, "No bisect range") {
		t.Error("empty view should contain 'No bisect range'")
	}
}

func TestBisectModel_View_WithCommits(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetCommits(testCommits(5))
	view := bm.View()
	if !strings.Contains(view, "Git Bisect") {
		t.Error("view should contain 'Git Bisect'")
	}
	if !strings.Contains(view, "[g] Good") {
		t.Error("view should contain footer hints")
	}
}

func TestBisectModel_View_NarrowWidth(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 10, 24) // very narrow
	bm.SetCommits(testCommits(5))
	view := bm.View()
	if view == "" {
		t.Error("narrow view should return non-empty string")
	}
}

func TestBisectModel_View_Error(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.errMsg = "something went wrong"
	view := bm.View()
	if !strings.Contains(view, "something went wrong") {
		t.Error("view should contain error message")
	}
}

func TestBisectModel_View_LongHash(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(3)
	commits[0].Hash = "abcdef1234567890abcdef1234567890abcdef12"
	bm.SetCommits(commits)
	view := bm.View()
	if !strings.Contains(view, "abcdef1") {
		t.Error("view should contain first 7 chars of hash")
	}
}

func TestBisectModel_View_ScrollDown(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(20)
	bm.SetCommits(commits)
	// Move current past 5 to trigger scroll
	bm.current = 10
	view := bm.View()
	if view == "" {
		t.Error("scrolled view should return non-empty string")
	}
}

func TestBisectModel_View_AllStatuses(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(5)
	commits[0].Status = "good"
	commits[1].Status = "bad"
	commits[2].Status = "skip"
	commits[3].Status = "testing"
	commits[4].Status = "pending"
	bm.SetCommits(commits)
	bm.current = 0
	view := bm.View()
	// Check for at least some status icons (view may clip to window)
	hasIcon := false
	for _, icon := range []string{"✓", "✗", "⊘", "◐", "○"} {
		if strings.Contains(view, icon) {
			hasIcon = true
			break
		}
	}
	if !hasIcon {
		t.Error("view should contain at least one status icon")
	}
}

func TestBisectModel_View_LongMessage(t *testing.T) {
	t.Parallel()
	bm := NewBisectModel(testTheme(), 80, 24)
	commits := testCommits(3)
	commits[0].Message = strings.Repeat("very long commit message ", 10)
	bm.SetCommits(commits)
	view := bm.View()
	if view == "" {
		t.Error("view with long message should return non-empty string")
	}
}
