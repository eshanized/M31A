package tui

import (
	"testing"
)

func TestRollbackSetters(t *testing.T) {
	rm := NewRollbackModel(testTheme(), nil, nil, 80, 24)
	rm.SetTheme(testTheme())
	rm.SetDimensions(100, 40)
	if rm.width != 100 || rm.height != 40 {
		t.Error("dimensions not set")
	}
}

func TestRollbackLoadCommitsNilRollback(t *testing.T) {
	rm := NewRollbackModel(testTheme(), nil, nil, 80, 24)
	rm.LoadCommits()
	if rm.errMsg != "" {
		t.Error("should not set error for nil rollback")
	}
}

func TestRollbackClampScroll(t *testing.T) {
	rm := &RollbackModel{height: 24, cursor: 5, offset: 0}
	rm.clampScroll()
	if rm.cursor < rm.offset || rm.cursor >= rm.offset+rm.listVisibleRows() {
		t.Error("cursor should be in visible range")
	}

	rm = &RollbackModel{height: 24, cursor: 0, offset: 5}
	rm.clampScroll()
	if rm.offset != 0 {
		t.Error("offset should be 0 when cursor is 0")
	}
}

func TestRollbackListVisibleRows(t *testing.T) {
	rm := &RollbackModel{height: 24}
	if got := rm.listVisibleRows(); got != 18 {
		t.Errorf("want 18, got %d", got)
	}
	rm.height = 4
	if got := rm.listVisibleRows(); got != 3 {
		t.Errorf("want 3 (min), got %d", got)
	}
	rm.height = 10
	if got := rm.listVisibleRows(); got != 4 {
		t.Errorf("want 4, got %d", got)
	}
}
