package workflow

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
)

// AgentSwitchMsg is emitted when the engine suggests switching from planner
// to builder agent after plan completion.
type AgentSwitchMsg struct {
	FromAgent   string
	ToAgent     string
	PlanPath    string
	PlanContent string
}

// PreparePlanFile creates a plan file at .m31a/plans/<session-id>.md and
// writes the plan content. Returns the file path.
func (e *Engine) PreparePlanFile(planContent string) (string, error) {
	plansDir := filepath.Join(e.workDir, ".m31a", "plans")
	if err := os.MkdirAll(plansDir, m31types.DirPermission); err != nil {
		return "", fmt.Errorf("create plans directory: %w", err)
	}

	filename := fmt.Sprintf("%s_%s.md", e.sessionID, time.Now().Format("20060102_150405"))
	path := filepath.Join(plansDir, filename)

	header := fmt.Sprintf("# Plan: %s\n\nGenerated: %s\nSession: %s\n\n",
		e.sessionID,
		time.Now().Format(time.RFC3339),
		e.sessionID,
	)

	content := header + planContent
	if err := os.WriteFile(path, []byte(content), m31types.FilePermission); err != nil {
		return "", fmt.Errorf("write plan file: %w", err)
	}

	return path, nil
}

// BuildAgentSwitchMessage creates a synthetic user message that instructs
// the builder agent to execute the plan.
func BuildAgentSwitchMessage(planPath, planContent string) m31types.Message {
	return m31types.Message{
		Role: "user",
		Content: fmt.Sprintf(
			"Switch to build mode. The implementation plan is at %s.\n\n"+
				"Execute the plan step by step, using all available tools.\n"+
				"Read the plan file first, then work through each task.\n\n"+
				"---\n\nPlan Summary:\n%s",
			planPath,
			truncatePlan(planContent, 4000),
		),
		CreatedAt: time.Now(),
	}
}

func truncatePlan(content string, maxChars int) string {
	if len(content) <= maxChars {
		return content
	}
	return content[:maxChars] + "\n\n...[plan truncated; read the plan file for full content]"
}

// IsPlanComplete checks if the engine's plan phase produced a valid plan.
func (e *Engine) IsPlanComplete() bool {
	return e.state.planMarkdown != "" && strings.Contains(e.state.planMarkdown, "##")
}
