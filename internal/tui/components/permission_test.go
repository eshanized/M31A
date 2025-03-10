package components

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewPermissionModal_DangerousTool(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:    "Bash",
		Command:     "rm -rf /",
		RiskLevel:   types.RiskDangerous,
		TimeoutSecs: 300,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	if m == nil {
		t.Fatal("expected non-nil PermissionModal")
	}
	if m.IsResponded() {
		t.Error("expected not responded initially")
	}
}

func TestPermissionModal_Render(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:    "Bash",
		Command:     "rm -rf node_modules",
		RiskLevel:   types.RiskDangerous,
		TimeoutSecs: 300,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	result := m.Render(80, 24)
	if result == "" {
		t.Error("expected non-empty render")
	}
}

func TestPermissionModal_Allow(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "ls",
		RiskLevel: types.RiskMedium,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	resp := m.Allow()
	if !resp.Allowed {
		t.Error("expected allowed=true")
	}
	if resp.Remember {
		t.Error("expected remember=false for Allow")
	}
	if !m.IsResponded() {
		t.Error("expected responded after Allow")
	}
}

func TestPermissionModal_AllowAlways(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "ls",
		RiskLevel: types.RiskMedium,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	resp := m.AllowAlways()
	if !resp.Allowed {
		t.Error("expected allowed=true")
	}
	if !resp.Remember {
		t.Error("expected remember=true for AllowAlways")
	}
}

func TestPermissionModal_Deny(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "rm -rf /",
		RiskLevel: types.RiskDangerous,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	resp := m.Deny()
	if resp.Allowed {
		t.Error("expected allowed=false for Deny")
	}
}

func TestPermissionModal_Countdown(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "ls",
		RiskLevel: types.RiskSafe,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)

	initial := m.Remaining()
	for i := 0; i < 10; i++ {
		m.Tick()
	}

	after := m.Remaining()
	if after >= initial {
		t.Error("expected remaining time to decrease after ticks")
	}
}

func TestPermissionModal_AutoDeny(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "ls",
		RiskLevel: types.RiskSafe,
	}
	m := NewPermissionModal(req, theme.Dark(), 1*time.Second)

	for i := 0; i < 20; i++ {
		m.Tick()
	}

	if m.Remaining() <= 0 {
		req2 := tools.PermissionRequest{
			ToolName:  "Bash",
			Command:   "ls",
			RiskLevel: types.RiskSafe,
		}
		m2 := NewPermissionModal(req2, theme.Dark(), 0)
		remaining := m2.Remaining()
		if remaining != 0 {
			t.Errorf("expected 0 remaining for zero timeout, got %v", remaining)
		}
	}
}

func TestPermissionModal_RiskColor_Dangerous(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "rm -rf /",
		RiskLevel: types.RiskDangerous,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	result := m.Render(80, 24)
	if result == "" {
		t.Error("expected non-empty render for dangerous tool")
	}
}

func TestPermissionModal_RiskColor_Destructive(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "format drive",
		RiskLevel: types.RiskDestructive,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	result := m.Render(80, 24)
	if result == "" {
		t.Error("expected non-empty render for destructive tool")
	}
}
