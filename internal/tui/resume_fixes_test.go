package tui

import (
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestResumeModel_SetTotalCount(t *testing.T) {
	sessions := []session.SessionInfo{
		{ID: "s1", StartedAt: time.Now()},
		{ID: "s2", StartedAt: time.Now()},
	}
	rm := NewResumeModel(sessions, theme.NewManager(theme.ModeDark).Current())
	rm.SetDimensions(80, 24)

	if rm.totalCount != 0 {
		t.Errorf("expected totalCount 0 before SetTotalCount, got %d", rm.totalCount)
	}

	rm.SetTotalCount(50)
	if rm.totalCount != 50 {
		t.Errorf("expected totalCount 50, got %d", rm.totalCount)
	}
}

func TestResumeModel_TruncationIndicator(t *testing.T) {
	sessions := make([]session.SessionInfo, 20)
	for i := range sessions {
		sessions[i] = session.SessionInfo{
			ID:        "s" + string(rune('a'+i)),
			StartedAt: time.Now(),
		}
	}
	rm := NewResumeModel(sessions, theme.NewManager(theme.ModeDark).Current())
	rm.SetDimensions(80, 40)
	rm.SetTotalCount(42)

	view := rm.renderResume()
	if !strings.Contains(view, "showing 20 of 42 total") {
		t.Errorf("expected truncation indicator in view, got: %s", view)
	}
}

func TestResumeModel_NoTruncationIndicator_WhenAllVisible(t *testing.T) {
	sessions := []session.SessionInfo{
		{ID: "s1", StartedAt: time.Now()},
		{ID: "s2", StartedAt: time.Now()},
	}
	rm := NewResumeModel(sessions, theme.NewManager(theme.ModeDark).Current())
	rm.SetDimensions(80, 24)
	rm.SetTotalCount(2)

	view := rm.renderResume()
	if strings.Contains(view, "showing") {
		t.Errorf("should not show truncation indicator when all sessions visible, got: %s", view)
	}
}
