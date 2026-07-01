package components

import (
	"github.com/eshanized/M31A/internal/tui/theme"
)

// EmptyStateTemplate provides pre-built empty states for common screens.
type EmptyStateTemplate struct {
	Icon    string
	Title   string
	Hint    string
	Actions []Action
}

// REPLEmptyState returns the empty state for the REPL when no conversation is active.
func REPLEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◈",
		Title:    "M 3 1 A",
		Subtitle: "What are we building?",
		Actions: []Action{
			{Label: "Fix failing tests", Hint: "auto-fix"},
			{Label: "Add error handling", Hint: "refactor"},
			{Label: "Explain architecture", Hint: "explore"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// PlanEmptyState returns the empty state for the Plan screen before tasks are generated.
func PlanEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◇",
		Title:    "Planning",
		Subtitle: "Tasks will appear here once the plan is generated",
		Actions: []Action{
			{Label: "Describe your goal to start planning", Hint: "type in chat"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// ExecuteEmptyState returns the empty state for the Execute screen before tasks run.
func ExecuteEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "▶",
		Title:    "Executing",
		Subtitle: "Tasks will run here",
		Actions: []Action{
			{Label: "Approve the plan to begin execution", Hint: "press Enter"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// VerifyEmptyState returns the empty state for the Verify screen before verification.
func VerifyEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "✓",
		Title:    "Verifying",
		Subtitle: "Verification results will appear here",
		Actions: []Action{
			{Label: "Tasks are being verified automatically", Hint: "wait"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// ShipEmptyState returns the empty state for the Ship screen before completion.
func ShipEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "⊞",
		Title:    "Ready to Ship",
		Subtitle: "Session summary will appear here",
		Actions: []Action{
			{Label: "Complete verification to ship", Hint: "wait"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// DiscussEmptyState returns the empty state for the Discuss screen before questions.
func DiscussEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "?",
		Title:    "Discussing",
		Subtitle: "Questions will appear here for clarification",
		Actions: []Action{
			{Label: "The AI may ask questions to refine the plan", Hint: "wait"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// RuntimeEmptyState returns the empty state for the Runtime screen.
func RuntimeEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◉",
		Title:    "Runtime",
		Subtitle: "Dev server status will appear here",
		Actions: []Action{
			{Label: "Start a dev server to test your changes", Hint: "/devserver start"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// SettingsEmptyState returns the empty state for the Settings screen.
func SettingsEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "⚙",
		Title:    "Settings",
		Subtitle: "Configure your M31A experience",
		Actions: []Action{
			{Label: "Set up a provider to get started", Hint: "select a tab"},
			{Label: "Configure theme and keybindings", Hint: "navigate tabs"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// LedgerEmptyState returns the empty state for the Ledger screen.
func LedgerEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◈",
		Title:    "Session Ledger",
		Subtitle: "Past sessions will appear here",
		Actions: []Action{
			{Label: "Complete a workflow to create a ledger entry", Hint: "run /new"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// MetricsEmptyState returns the empty state for the Metrics screen.
func MetricsEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◆",
		Title:    "Metrics",
		Subtitle: "Usage statistics will appear here",
		Actions: []Action{
			{Label: "Metrics are collected during workflow execution", Hint: "run a workflow"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// RollbackEmptyState returns the empty state for the Rollback screen.
func RollbackEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "↺",
		Title:    "Rollback",
		Subtitle: "No commits to roll back",
		Actions: []Action{
			{Label: "Commits will appear here after shipping", Hint: "run /ship"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// BisectEmptyState returns the empty state for the Bisect screen.
func BisectEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "⚡",
		Title:    "Git Bisect",
		Subtitle: "Automated bug finding via binary search",
		Actions: []Action{
			{Label: "Start a bisect session to find problematic commits", Hint: "run /bisect"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// DecisionsEmptyState returns the empty state for the Decisions screen.
func DecisionsEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◇",
		Title:    "Decisions",
		Subtitle: "AI decisions will be logged here",
		Actions: []Action{
			{Label: "The AI records decisions during workflow execution", Hint: "run a workflow"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// SubagentsEmptyState returns the empty state for the Subagents screen.
func SubagentsEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "◎",
		Title:    "Sub-agents",
		Subtitle: "No active sub-agents",
		Actions: []Action{
			{Label: "The AI spawns sub-agents for parallel work", Hint: "automatic"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}

// DiffEmptyState returns the empty state for the Diff screen.
func DiffEmptyState(t theme.Theme, w, h int) EmptyState {
	return EmptyState{
		Icon:     "±",
		Title:    "Diff View",
		Subtitle: "No changes to display",
		Actions: []Action{
			{Label: "Changes will appear here after file modifications", Hint: "run a workflow"},
		},
		Theme:  t,
		Width:  w,
		Height: h,
	}
}
