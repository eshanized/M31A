package workflow

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// runDiscuss asks clarifying questions and collects user answers.
// Integrates question quality checking with optional retry on blockers.
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
	defer func() {
		if err := iterator.Close(); err != nil {
			slog.Debug("close stream iterator", "error", err, "resource", "runDiscuss")
		}
	}()

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

	// 5. Question quality checker
	qualityEnabled := e.cfg != nil && e.cfg.Features.DiscussQualityCheck
	if qualityEnabled && len(questions) > 0 {
		issues := checkQuestionQuality(questions)
		blockers, warnings := countDiscussIssues(issues)

		e.emit(DiscussQualityMsg{
			Passed:   blockers == 0,
			Warnings: warnings,
		})

		if blockers > 0 {
			e.logger.Warn("question quality check found blockers", "blockers", blockers, "warnings", warnings)
			// Retry once: regenerate questions with quality feedback
			retryQuestions, retryErr := e.retryDiscussQuestions(ctx, goal, questions, issues)
			if retryErr == nil && len(retryQuestions) > 0 {
				questions = retryQuestions
				e.emit(DiscussQualityMsg{
					Passed:   true,
					Warnings: 0,
					Retried:  true,
				})
				e.logger.Info("questions regenerated after quality check", "new_count", len(questions))
			}
		} else if warnings > 0 {
			e.logger.Info("question quality check: warnings only", "warnings", warnings)
		}
	}

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

// retryDiscussQuestions re-invokes the LLM with quality feedback to regenerate questions.
func (e *Engine) retryDiscussQuestions(ctx context.Context, goal string, originalQuestions []string, issues []DiscussIssue) ([]string, error) {
	var feedback strings.Builder
	feedback.WriteString("The following questions were generated but have quality issues:\n\n")
	for _, issue := range issues {
		if issue.Severity == "blocker" {
			fmt.Fprintf(&feedback, "- %s\n", issue.Message)
		}
	}
	feedback.WriteString("\nPlease regenerate the questions, fixing the issues above. Keep the same format (numbered, with suggested defaults).")

	messages := e.buildDiscussContext(goal)
	// Add the feedback as a follow-up user message
	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: feedback.String(),
	})

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("discuss retry LLM call failed: %w", err)
	}

	return parseQuestions(content), nil
}

// CheckDiscussCompleteness evaluates how well the collected answers cover the goal.
// Called by the TUI after all answers are collected, before proceeding to Plan.
func (e *Engine) CheckDiscussCompleteness() DiscussCompleteness {
	completenessEnabled := e.cfg != nil && e.cfg.Features.DiscussCompleteness
	if !completenessEnabled {
		return DiscussCompleteness{Score: 100}
	}

	goal := ""
	project := e.loadProjectCached()
	if project != nil {
		goal = project.Goal
	}

	completeness := checkAnswerCompleteness(e.discussState.Questions, e.discussState.Answers, goal)

	e.emit(DiscussCompletenessMsg{
		Score:        completeness.Score,
		MissingAreas: completeness.MissingAreas,
	})

	e.logger.Info("discuss completeness check",
		"score", completeness.Score,
		"answered", completeness.Answered,
		"skipped", completeness.Skipped,
		"missing", len(completeness.MissingAreas))

	return completeness
}

// GenerateFollowUpsIfNeeded creates follow-up questions when answers are incomplete.
// Called by the TUI after CheckDiscussCompleteness if score is low.
func (e *Engine) GenerateFollowUpsIfNeeded(ctx context.Context) ([]string, error) {
	goal := ""
	project := e.loadProjectCached()
	if project != nil {
		goal = project.Goal
	}

	completeness := checkAnswerCompleteness(e.discussState.Questions, e.discussState.Answers, goal)
	if !e.shouldGenerateFollowUps(completeness) {
		return nil, nil
	}

	return e.generateFollowUps(ctx, goal, e.discussState.Questions, e.discussState.Answers)
}

// countDiscussIssues returns blocker and warning counts from discuss issues.
func countDiscussIssues(issues []DiscussIssue) (blockers, warnings int) {
	for _, i := range issues {
		if i.Severity == "blocker" {
			blockers++
		} else {
			warnings++
		}
	}
	return
}

// buildDiscussContext creates the message list for the discuss phase.
// Injects code intelligence and file listing for richer context.
func (e *Engine) buildDiscussContext(goal string) []m31types.Message {
	var messages []m31types.Message
	messages = append(messages, m31types.Message{Role: "system", Content: e.buildSystemPrompt(e.promptOrGet("discuss"))})

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
	project := e.loadProjectCached()
	projectType := "unknown"
	framework := ""
	if project != nil {
		if project.ProjectType != "" {
			projectType = project.ProjectType
		}
		framework = project.Framework
	}

	userCtx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s", goal, projectType, framework)

	// Inject intent classification if available
	if e.state.intentResult != nil {
		userCtx += fmt.Sprintf("\n\n## Intent Classification\n- Intent: %s\n- Complexity: %s\n- Confidence: %.0f%%\n- Summary: %s",
			e.state.intentResult.Intent, e.state.intentResult.Complexity, e.state.intentResult.Confidence*100, e.state.intentResult.Summary)
		if len(e.state.intentResult.Scope) > 0 {
			userCtx += "\n- Scope: " + strings.Join(e.state.intentResult.Scope, ", ")
		}
	}

	// Inject file listing so the LLM can see what's already in the codebase
	fileSchema := listCwdFiles(e.workDir)
	if fileSchema != "" {
		userCtx += "\n\n## Existing Files\n" + fileSchema
	}

	// Inject code intelligence summary so the LLM knows the codebase structure
	if ci := e.getCodeIntel(context.Background()); ci != nil {
		if summary := ci.ProjectSummary(2000); summary != "" {
			userCtx += "\n" + summary
		}
	}

	// Inform the model if answers have already been captured so it doesn't repeat covered ground.
	if project != nil && len(project.Answers) > 0 {
		userCtx += "\n\nNote: the following questions have already been answered — do not repeat them:\n"
		for q := range project.Answers {
			userCtx += "- " + q + "\n"
		}
	}

	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: userCtx,
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
