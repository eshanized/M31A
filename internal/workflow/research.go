package workflow

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
)

// runResearch performs a pre-plan research step that investigates the codebase,
// identifies patterns, and surfaces risks. The output is injected into the
// plan context to give the planner a solid foundation.
func (e *Engine) runResearch(ctx context.Context, goal string) (string, error) {
	e.emit(ResearchProgressMsg{
		Phase:   "plan",
		Message: "Researching codebase and approach...",
	})

	messages := e.buildResearchContext(goal)

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		e.emit(ResearchProgressMsg{
			Phase:    "plan",
			Message:  "Research failed: " + err.Error(),
			Complete: true,
		})
		return "", fmt.Errorf("research LLM call failed: %w", err)
	}

	e.researchOutput = content

	e.emit(ResearchProgressMsg{
		Phase:    "plan",
		Message:  "Research complete",
		Complete: true,
	})

	return content, nil
}

// buildResearchContext assembles messages for the research sub-step.
func (e *Engine) buildResearchContext(goal string) []m31types.Message {
	var messages []m31types.Message

	systemPrompt := e.buildSystemPrompt(e.prompts.Research)
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	var userCtx strings.Builder
	userCtx.WriteString(fmt.Sprintf("## Goal\n%s\n\n", goal))

	// Project info
	project, projErr := e.sessionMgr.LoadProject(e.sessionID)
	if projErr == nil && project != nil {
		userCtx.WriteString(fmt.Sprintf("## Project Context\nType: %s\nFramework: %s\n\n",
			project.ProjectType, project.Framework))
		if len(project.Answers) > 0 {
			userCtx.WriteString("## Discuss Phase Answers\n")
			for q, a := range project.Answers {
				userCtx.WriteString(fmt.Sprintf("- Q: %s → A: %s\n", q, a))
			}
			userCtx.WriteString("\n")
		}
	}

	// File schema
	fileSchema := listCwdFiles(e.workDir)
	if fileSchema != "" {
		userCtx.WriteString("## Existing Files\n")
		userCtx.WriteString(fileSchema)
		userCtx.WriteString("\n")
	}

	// Code intelligence summary
	if ci := e.getCodeIntel(context.Background()); ci != nil {
		if summary := ci.ProjectSummary(3000); summary != "" {
			userCtx.WriteString(summary)
			userCtx.WriteString("\n")
		}
	}

	// Cross-session memory
	sessionDir := filepath.Dir(e.planningDir)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	if mem, err := os.ReadFile(memPath); err == nil {
		memContent := string(mem)
		if len(memContent) > 2000 {
			memContent = memContent[len(memContent)-2000:]
		}
		userCtx.WriteString("## Cross-Session Memory\n")
		userCtx.WriteString(memContent)
		userCtx.WriteString("\n")
	}

	messages = append(messages, m31types.Message{Role: "user", Content: userCtx.String()})
	return messages
}

// isResearchWorthy returns true when the goal is complex enough to benefit
// from a research step. Triggers on: 2+ complex indicators from classify.go,
// or 20+ project files (proxy for large codebase).
func (e *Engine) isResearchWorthy(goal string) bool {
	lower := strings.ToLower(strings.TrimSpace(goal))

	complexCount := 0
	for _, ind := range complexIndicators {
		if strings.Contains(lower, ind) {
			complexCount++
		}
	}
	if complexCount >= 2 {
		return true
	}

	fileCount := countProjectFiles(e.workDir)
	return fileCount >= 20
}
