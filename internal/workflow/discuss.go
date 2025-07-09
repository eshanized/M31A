package workflow

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
)

// runDiscuss asks clarifying questions and collects user answers.
func (e *Engine) runDiscuss(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("discuss phase starting")

	// Emit intermediate progress
	e.emit(IntermediateProgressMsg{
		Phase:   "discuss",
		Message: "Generating clarifying questions...",
	})

	// 1. Build context
	messages := e.buildDiscussContext(goal)

	// 2. Stream LLM response with progressive token emission
	iterator, err := e.streamLLMStreaming(ctx, messages, false)
	if err != nil {
		return &PhaseResult{
			Phase:   m31types.PhaseDiscuss,
			Success: false,
			Error:   err.Error(),
		}, err
	}
	defer iterator.Close()

	// 3. Iterate chunks, accumulate content, emit chunks to TUI
	var content strings.Builder
	for {
		chunk, err := iterator.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return &PhaseResult{
				Phase:   m31types.PhaseDiscuss,
				Success: false,
				Error:   err.Error(),
			}, err
		}
		if chunk == nil {
			continue
		}
		if chunk.Delta != "" {
			content.WriteString(chunk.Delta)
		}

		// Emit the chunk to the TUI for progressive rendering
		if e.msgEmitter != nil {
			e.msgEmitter.Emit(m31types.StreamChunkMsg{
				Chunk:  chunk,
				Source: "discuss",
			})
		}
	}

	fullContent := content.String()

	// 4. Parse questions
	questions := parseQuestions(fullContent)

	msg := m31types.Message{
		Role:    "assistant",
		Content: fullContent,
	}

	e.logger.Info("parsed questions", "count", len(questions))

	// Store questions in engine state for answer collection
	e.discussState = DiscussState{
		Questions: questions,
	}

	// Return with questions — TUI handles Q&A collection, then calls SubmitDiscussAnswer/SkipDiscuss
	result := &PhaseResult{
		Phase:        m31types.PhaseDiscuss,
		Success:      true,
		Messages:     []m31types.Message{msg},
		NeedsAnswers: len(questions) > 0,
	}

	return result, nil
}

// buildDiscussContext creates the message list for the discuss phase.
func (e *Engine) buildDiscussContext(goal string) []m31types.Message {
	var messages []m31types.Message
	messages = append(messages, m31types.Message{Role: "system", Content: e.buildSystemPrompt(e.prompts.Discuss)})

	// Load MEMORY.md if exists
	sessionDir := filepath.Dir(e.planningDir)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	if mem, err := os.ReadFile(memPath); err == nil {
		messages = append(messages, m31types.Message{
			Role:    "user",
			Content: "Memory from previous sessions:\n" + string(mem),
		})
	}

	// Load PROJECT.md
	project, _ := e.sessionMgr.LoadProject(e.sessionID)
	projectType := "unknown"
	framework := ""
	if project != nil {
		if project.ProjectType != "" {
			projectType = project.ProjectType
		}
		framework = project.Framework
	}

	ctx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s", goal, projectType, framework)
	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: ctx + "\n\nAsk 2-4 clarifying questions to understand the requirements better. Number each question. Be specific and concise.",
	})

	return messages
}

// saveDiscussAnswers appends Q&A to PROJECT.md.
func (e *Engine) saveDiscussAnswers(project *m31types.ProjectState, questions, answers []string) error {
	if project == nil {
		project = &m31types.ProjectState{Answers: make(map[string]string)}
	}
	if project.Answers == nil {
		project.Answers = make(map[string]string)
	}

	for i, q := range questions {
		if i < len(answers) {
			project.Answers[q] = answers[i]
		}
	}

	return e.sessionMgr.SaveProject(e.sessionID, project)
}
