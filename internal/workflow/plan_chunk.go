package workflow

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
)

// PlanOutline represents the high-level structure of a chunked plan.
type PlanOutline struct {
	Title      string        `json:"title"`
	Waves      []WaveOutline `json:"waves"`
	TotalTasks int           `json:"total_tasks"`
}

// WaveOutline groups tasks by execution wave.
type WaveOutline struct {
	Wave  int        `json:"wave"`
	Tasks []TaskStub `json:"tasks"`
}

// TaskStub is a minimal task representation for the outline phase.
type TaskStub struct {
	ID           int    `json:"id"`
	Action       string `json:"action"`
	Description  string `json:"description"`
	Dependencies []int  `json:"dependencies"`
	Category     string `json:"category"`
}

// generateOutline creates a high-level plan outline for chunked planning.
func (e *Engine) generateOutline(ctx context.Context, goal string) (*PlanOutline, error) {
	e.emit(IntermediateProgressMsg{
		Phase:   "plan",
		Message: "Generating plan outline...",
	})

	messages := e.buildOutlineContext(ctx, goal)

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("outline LLM call failed: %w", err)
	}

	outline, err := parseOutline(content)
	if err != nil {
		return nil, fmt.Errorf("outline parse failed: %w", err)
	}

	return outline, nil
}

// expandWave fleshes out a single wave from the outline into full tasks
// with acceptance criteria and file mappings.
func (e *Engine) expandWave(ctx context.Context, outline *PlanOutline, waveNum int, goal string) ([]m31types.Task, error) {
	wave := findWave(outline, waveNum)
	if wave == nil {
		return nil, fmt.Errorf("wave %d not found in outline", waveNum)
	}

	e.emit(PlanChunkProgressMsg{
		Wave:        waveNum,
		TotalWaves:  len(outline.Waves),
		TasksInWave: len(wave.Tasks),
	})

	messages := e.buildWaveExpandContext(outline, wave, goal)

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("wave %d expansion failed: %w", waveNum, err)
	}

	tasks, parseErr := parseTasksFromJSON(content)
	if parseErr != nil {
		return nil, fmt.Errorf("wave %d task parse failed: %w", waveNum, parseErr)
	}

	return tasks, nil
}

// shouldChunk returns true when the goal complexity and project size
// suggest chunked planning would be beneficial.
func (e *Engine) shouldChunk(goal string) bool {
	complexity := ClassifyPrompt(goal, e.workDir)
	if complexity != m31types.ComplexityComplex {
		return false
	}
	fileCount := countProjectFiles(e.workDir)
	return fileCount >= 30
}

// chunkThreshold returns the configured or default task threshold for chunking.
func (e *Engine) chunkThreshold() int {
	if e.cfg != nil && e.cfg.Features.PlanChunkThreshold > 0 {
		return e.cfg.Features.PlanChunkThreshold
	}
	return 10
}

// buildOutlineContext assembles messages for outline generation.
func (e *Engine) buildOutlineContext(ctx context.Context, goal string) []m31types.Message {
	var messages []m31types.Message

	systemPrompt := e.buildSystemPrompt(e.prompts.PlanOutline, e.prompts.ContextAwareness)
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	var userCtx strings.Builder
	fmt.Fprintf(&userCtx, "## Goal\n%s\n\n", goal)

	project, projErr := e.sessionMgr.LoadProject(e.sessionID)
	if projErr == nil && project != nil {
		fmt.Fprintf(&userCtx, "## Project Context\nType: %s\nFramework: %s\n\n",
			project.ProjectType, project.Framework)
	}

	fileSchema := listCwdFiles(e.workDir)
	if fileSchema != "" {
		userCtx.WriteString("## Existing Files\n")
		userCtx.WriteString(fileSchema)
		userCtx.WriteString("\n")
	}

	if ci := e.getCodeIntel(ctx); ci != nil {
		if summary := ci.ProjectSummary(3000); summary != "" {
			userCtx.WriteString(summary)
			userCtx.WriteString("\n")
		}
	}

	// Inject research output if available
	if e.researchOutput != "" {
		userCtx.WriteString("## Research Findings\n")
		userCtx.WriteString(e.researchOutput)
		userCtx.WriteString("\n")
	}

	messages = append(messages, m31types.Message{Role: "user", Content: userCtx.String()})
	return messages
}

// buildWaveExpandContext assembles messages for expanding a single wave.
func (e *Engine) buildWaveExpandContext(outline *PlanOutline, wave *WaveOutline, goal string) []m31types.Message {
	var messages []m31types.Message

	systemPrompt := e.buildSystemPrompt(e.prompts.PlanFormat, e.prompts.ToolUse, e.prompts.ContextAwareness)
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	var userCtx strings.Builder
	fmt.Fprintf(&userCtx, "## Goal\n%s\n\n", goal)

	fmt.Fprintf(&userCtx, "## Expanding Wave %d of %d\n\n", wave.Wave, len(outline.Waves))

	userCtx.WriteString("### Tasks in this wave:\n")
	for _, stub := range wave.Tasks {
		deps := "none"
		if len(stub.Dependencies) > 0 {
			depStrs := make([]string, len(stub.Dependencies))
			for i, d := range stub.Dependencies {
				depStrs[i] = fmt.Sprintf("%d", d)
			}
			deps = strings.Join(depStrs, ", ")
		}
		fmt.Fprintf(&userCtx, "- Task %d: [%s] %s (deps: %s, category: %s)\n",
			stub.ID, stub.Action, stub.Description, deps, stub.Category)
	}

	userCtx.WriteString("\n## Instructions\n")
	userCtx.WriteString("For each task above, generate:\n")
	userCtx.WriteString("1. A list of files to create or modify\n")
	userCtx.WriteString("2. Specific acceptance criteria (grep-verifiable)\n\n")
	userCtx.WriteString("Return the tasks as a JSON array following the standard task schema.\n")

	messages = append(messages, m31types.Message{Role: "user", Content: userCtx.String()})
	return messages
}

// parseOutline extracts a PlanOutline from LLM response content.
func parseOutline(content string) (*PlanOutline, error) {
	// Try to find JSON object in the content
	jsonStr := extractJSONObject(content)
	if jsonStr == "" {
		// Try stripping code blocks first
		stripped := stripCodeBlocks(content)
		jsonStr = extractJSONObject(stripped)
	}
	if jsonStr == "" {
		return nil, fmt.Errorf("no JSON object found in outline response")
	}

	var outline PlanOutline
	if err := json.Unmarshal([]byte(jsonStr), &outline); err != nil {
		return nil, fmt.Errorf("JSON unmarshal failed: %w", err)
	}

	if len(outline.Waves) == 0 {
		return nil, fmt.Errorf("outline has no waves")
	}

	// Calculate total tasks
	total := 0
	for _, w := range outline.Waves {
		total += len(w.Tasks)
	}
	outline.TotalTasks = total

	return &outline, nil
}

// findWave returns the WaveOutline for a given wave number.
func findWave(outline *PlanOutline, waveNum int) *WaveOutline {
	for i, w := range outline.Waves {
		if w.Wave == waveNum {
			return &outline.Waves[i]
		}
	}
	return nil
}
