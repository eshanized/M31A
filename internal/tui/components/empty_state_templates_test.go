package components

import (
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestEmptyState_Render(t *testing.T) {
	es := EmptyState{
		Icon:     "◈",
		Title:    "Test Title",
		Subtitle: "Test subtitle",
		Actions: []Action{
			{Label: "Action 1", Hint: "hint 1"},
			{Label: "Action 2", Hint: "hint 2"},
		},
		Theme:  theme.Dark(),
		Width:  80,
		Height: 24,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string")
	}
}

func TestEmptyState_RenderMinimal(t *testing.T) {
	es := EmptyState{
		Title: "Minimal",
		Theme: theme.Dark(),
		Width: 40,
		Height: 10,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string for minimal config")
	}
}

func TestEmptyState_RenderSmall(t *testing.T) {
	es := EmptyState{
		Title: "Small",
		Theme: theme.Dark(),
		Width: 10,
		Height: 2,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string for small dimensions")
	}
}

func TestREPLEmptyState(t *testing.T) {
	es := REPLEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("REPLEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("REPLEmptyState.Title should not be empty")
	}
	if len(es.Actions) == 0 {
		t.Error("REPLEmptyState.Actions should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("REPLEmptyState.Render() returned empty string")
	}
}

func TestPlanEmptyState(t *testing.T) {
	es := PlanEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("PlanEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("PlanEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("PlanEmptyState.Render() returned empty string")
	}
}

func TestExecuteEmptyState(t *testing.T) {
	es := ExecuteEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("ExecuteEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("ExecuteEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("ExecuteEmptyState.Render() returned empty string")
	}
}

func TestVerifyEmptyState(t *testing.T) {
	es := VerifyEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("VerifyEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("VerifyEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("VerifyEmptyState.Render() returned empty string")
	}
}

func TestShipEmptyState(t *testing.T) {
	es := ShipEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("ShipEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("ShipEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("ShipEmptyState.Render() returned empty string")
	}
}

func TestDiscussEmptyState(t *testing.T) {
	es := DiscussEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("DiscussEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("DiscussEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("DiscussEmptyState.Render() returned empty string")
	}
}

func TestRuntimeEmptyState(t *testing.T) {
	es := RuntimeEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("RuntimeEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("RuntimeEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("RuntimeEmptyState.Render() returned empty string")
	}
}

func TestSettingsEmptyState(t *testing.T) {
	es := SettingsEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("SettingsEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("SettingsEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("SettingsEmptyState.Render() returned empty string")
	}
}

func TestLedgerEmptyState(t *testing.T) {
	es := LedgerEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("LedgerEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("LedgerEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("LedgerEmptyState.Render() returned empty string")
	}
}

func TestMetricsEmptyState(t *testing.T) {
	es := MetricsEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("MetricsEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("MetricsEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("MetricsEmptyState.Render() returned empty string")
	}
}

func TestRollbackEmptyState(t *testing.T) {
	es := RollbackEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("RollbackEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("RollbackEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("RollbackEmptyState.Render() returned empty string")
	}
}

func TestBisectEmptyState(t *testing.T) {
	es := BisectEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("BisectEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("BisectEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("BisectEmptyState.Render() returned empty string")
	}
}

func TestDecisionsEmptyState(t *testing.T) {
	es := DecisionsEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("DecisionsEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("DecisionsEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("DecisionsEmptyState.Render() returned empty string")
	}
}

func TestSubagentsEmptyState(t *testing.T) {
	es := SubagentsEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("SubagentsEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("SubagentsEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("SubagentsEmptyState.Render() returned empty string")
	}
}

func TestDiffEmptyState(t *testing.T) {
	es := DiffEmptyState(theme.Dark(), 80, 24)
	if es.Icon == "" {
		t.Error("DiffEmptyState.Icon should not be empty")
	}
	if es.Title == "" {
		t.Error("DiffEmptyState.Title should not be empty")
	}
	result := es.Render()
	if result == "" {
		t.Error("DiffEmptyState.Render() returned empty string")
	}
}

func TestEmptyState_NoActions(t *testing.T) {
	es := EmptyState{
		Icon:     "◈",
		Title:    "No Actions",
		Subtitle: "This empty state has no actions",
		Theme:    theme.Dark(),
		Width:    80,
		Height:   24,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string for no actions")
	}
}

func TestEmptyState_DefaultIcon(t *testing.T) {
	es := EmptyState{
		Title:  "Default Icon",
		Theme:  theme.Dark(),
		Width:  80,
		Height: 24,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string for default icon")
	}
}

func TestEmptyState_NarrowTerminal(t *testing.T) {
	es := EmptyState{
		Title:  "Narrow",
		Theme:  theme.Dark(),
		Width:  40,
		Height: 10,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string for narrow terminal")
	}
}

func TestEmptyState_WideTerminal(t *testing.T) {
	es := EmptyState{
		Title:  "Wide",
		Theme:  theme.Dark(),
		Width:  200,
		Height: 50,
	}

	result := es.Render()
	if result == "" {
		t.Error("EmptyState.Render() returned empty string for wide terminal")
	}
}
