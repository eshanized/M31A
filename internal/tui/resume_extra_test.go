package tui

import (
	"testing"

	"github.com/eshanized/M31A/pkg/session"
)

func TestResumeSetters(t *testing.T) {
	rm := &ResumeModel{}
	rm.SetTheme(testTheme())
	rm.SetDimensions(100, 40)
	if rm.width != 100 || rm.height != 40 {
		t.Error("dimensions not set")
	}
}

func TestResumeRefresh(t *testing.T) {
	rm := &ResumeModel{
		allSessions: []session.SessionInfo{{ID: "1"}, {ID: "2"}},
		sessions:    []session.SessionInfo{{ID: "1"}},
		searching:   false,
		cursor:      5,
	}
	newSessions := []session.SessionInfo{{ID: "a"}, {ID: "b"}, {ID: "c"}}
	rm.Refresh(newSessions)
	if len(rm.allSessions) != 3 {
		t.Error("allSessions should be replaced")
	}
	if rm.cursor != 2 {
		t.Errorf("cursor should be clamped, got %d", rm.cursor)
	}
}

func TestResumeRefreshSearching(t *testing.T) {
	rm := &ResumeModel{
		allSessions: []session.SessionInfo{{ID: "x"}},
		sessions:    []session.SessionInfo{{ID: "x"}},
		searching:   true,
	}
	rm.Refresh([]session.SessionInfo{})
	if len(rm.sessions) != 0 {
		t.Error("filtered sessions should be empty")
	}
}

func TestResumeClampScroll(t *testing.T) {
	rm := &ResumeModel{height: 24, cursor: 5, offset: 0}
	rm.clampScroll()
	if rm.cursor < rm.offset || rm.cursor >= rm.offset+rm.visibleRows() {
		t.Error("cursor should be visible")
	}

	rm = &ResumeModel{height: 24, cursor: 0, offset: 10}
	rm.clampScroll()
	if rm.offset != 0 {
		t.Error("offset should be 0")
	}
}

func TestResumeVisibleRows(t *testing.T) {
	rm := &ResumeModel{height: 24}
	if got := rm.visibleRows(); got != 18 {
		t.Errorf("want 18, got %d", got)
	}
	rm.height = 2
	if got := rm.visibleRows(); got != 3 {
		t.Errorf("want 3 (min), got %d", got)
	}
}
