package components

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
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

	if m.Remaining() > 0 {
		t.Error("expected remaining time to be <= 0 after sufficient ticks")
	}
}

func TestPermissionModal_RiskColor_Dangerous(t *testing.T) {
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

	// Verify risk color uses Warning (not Error) for Dangerous
	if m.riskStyle().GetBackground() != theme.Dark().Warning {
		t.Errorf("expected Dangerous risk level to use Warning color, got %v", m.riskStyle().GetBackground())
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

// L-14: zero timeout defaults to 10 minutes (no auto-deny)
func TestPermissionModal_ZeroTimeoutDefaults10m(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "ls",
		RiskLevel: types.RiskSafe,
	}
	m := NewPermissionModal(req, theme.Dark(), 0)
	remaining := m.Remaining()
	if remaining < 599*time.Second || remaining > 600*time.Second {
		t.Errorf("expected ~600s (10m) remaining for zero timeout, got %v", remaining)
	}
}

// M-33: Allow/AllowAlways/Deny are stateless pure constructors
func TestPermissionModal_AllowIsStateless(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "ls",
		RiskLevel: types.RiskSafe,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	resp1 := m.Allow()
	resp2 := m.Allow()
	if resp1 != resp2 {
		t.Errorf("expected Allow() to return same value both times, got %v and %v", resp1, resp2)
	}
}

func TestPermissionRiskLabel_Dangerous(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:    "Bash",
		Command:     "rm -rf /",
		RiskLevel:   types.RiskDangerous,
		TimeoutSecs: 300,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	result := m.Render(80, 24)
	if result == "" {
		t.Fatal("expected non-empty render")
	}
	if !containsText(result, "DANGER") {
		t.Errorf("expected DANGER risk label in output, got %q", result)
	}
}

func TestPermissionRiskLabel_Safe(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "FileRead",
		Command:   "read /etc/hosts",
		RiskLevel: types.RiskSafe,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	result := m.Render(80, 24)
	if result == "" {
		t.Fatal("expected non-empty render")
	}
	if !containsText(result, "SAFE") {
		t.Errorf("expected SAFE risk label in output, got %q", result)
	}
}

func TestPermissionRiskLabel_Medium(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:  "Bash",
		Command:   "npm install",
		RiskLevel: types.RiskMedium,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	result := m.Render(80, 24)
	if result == "" {
		t.Fatal("expected non-empty render")
	}
	if !containsText(result, "CAUTION") {
		t.Errorf("expected CAUTION risk label in output, got %q", result)
	}
}

func TestBatchApproval_ShowsKeybinding(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:    "Bash",
		Command:     "rm -rf node_modules",
		RiskLevel:   types.RiskDangerous,
		QueueDepth:  2,
		TimeoutSecs: 300,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	m.SetBatchAvailable(true)
	result := m.Render(80, 24)
	if result == "" {
		t.Fatal("expected non-empty render")
	}
	if !containsText(result, "Approve all 3") {
		t.Errorf("expected 'Approve all 3' in output, got %q", result)
	}
}

func TestBatchApproval_HiddenWhenNoQueue(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:    "Bash",
		Command:     "ls",
		RiskLevel:   types.RiskSafe,
		QueueDepth:  0,
		TimeoutSecs: 300,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	m.SetBatchAvailable(true)
	result := m.Render(80, 24)
	if result == "" {
		t.Fatal("expected non-empty render")
	}
	if containsText(result, "Approve all") {
		t.Errorf("expected no batch keybinding when QueueDepth=0, got %q", result)
	}
}

func TestBatchApproval_HiddenWhenNotAvailable(t *testing.T) {
	req := tools.PermissionRequest{
		ToolName:    "Bash",
		Command:     "rm -rf node_modules",
		RiskLevel:   types.RiskDangerous,
		QueueDepth:  2,
		TimeoutSecs: 300,
	}
	m := NewPermissionModal(req, theme.Dark(), 300*time.Second)
	// batchAvailable not set (default false)
	result := m.Render(80, 24)
	if result == "" {
		t.Fatal("expected non-empty render")
	}
	if containsText(result, "Approve all") {
		t.Errorf("expected no batch keybinding when batchAvailable=false, got %q", result)
	}
}

// containsText checks if the rendered output contains the given text.
// Uses a simple byte search since lipgloss output is styled but text is visible.
func containsText(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
