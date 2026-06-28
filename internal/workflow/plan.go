package workflow

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/decision"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// runPlan generates a rich implementation plan and task list to accomplish the goal.
// Integrates GSD-inspired sub-steps: research → plan → check → gates.
// On refinement (when e.state.refineFeedback is non-empty), the previous plan is injected
// into the LLM context along with the user's feedback for revision.
func (e *Engine) runPlan(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("plan phase starting", "version", e.state.planVersion+1)

	var tasks []m31types.Task
	var planMarkdown string
	var lastErr error
	var rawResponse string
	var valErrs []string
	var allValErrs []string

	isRefinement := e.state.refineFeedback != ""
	if isRefinement {
		e.emit(IntermediateProgressMsg{
			Phase:   "plan",
			Message: fmt.Sprintf("Refining plan (v%d)...", e.state.planVersion+1),
		})
	}

	// ── Sub-step 1: Pre-plan Research ─────────────────────────────────────
	researchEnabled := e.cfg != nil && e.cfg.Features.PlanResearch
	if !isRefinement && (researchEnabled || e.isResearchWorthy(goal)) {
		e.logger.Info("plan phase: starting pre-plan research")
		research, researchErr := e.runResearch(ctx, goal)
		if researchErr != nil {
			e.logger.Warn("pre-plan research failed, continuing without", "error", researchErr)
		} else if research != "" {
			e.state.researchOutput = research
			e.logger.Info("pre-plan research complete", "output_len", len(research))
		}
	}

	// ── Sub-step 2: Chunked Planning (alternative to standard retry loop) ─
	chunkSucceeded := false
	chunkEnabled := e.cfg != nil && e.cfg.Features.PlanChunked
	if !isRefinement && (chunkEnabled || e.shouldChunk(goal)) {
		e.logger.Info("plan phase: using chunked planning mode")
		chunkTasks, chunkMarkdown, chunkErr := e.runChunkedPlan(ctx, goal)
		if chunkErr != nil {
			e.logger.Warn("chunked planning failed, falling back to standard", "error", chunkErr)
		} else if len(chunkTasks) > 0 && chunkMarkdown != "" {
			tasks = chunkTasks
			planMarkdown = chunkMarkdown
			chunkSucceeded = true
		}
	}

	// ── Standard Plan Generation (retry loop) ─────────────────────────────
	if !chunkSucceeded {
		for attempt := 0; attempt < m31types.MaxPlanRetries; attempt++ {
			if !isRefinement {
				e.emit(IntermediateProgressMsg{
					Phase:   "plan",
					Message: fmt.Sprintf("Creating implementation plan... (attempt %d)", attempt+1),
				})
			}

			messages := e.buildPlanContext(ctx, goal, tasks, valErrs, rawResponse)

			content, err := e.streamLLM(ctx, messages, false)
			if err != nil {
				lastErr = err
				e.logger.Warn("LLM error in plan phase", "attempt", attempt, "error", err)
				valErrs = []string{err.Error()}
				allValErrs = append(allValErrs, fmt.Sprintf("attempt %d LLM error: %s", attempt+1, err.Error()))
				rawResponse = ""
				continue
			}

			plan, parseErr := ParsePlan(content)
			if parseErr != nil {
				parsed, jsonErr := parseTasksFromJSON(content)
				if jsonErr != nil {
					lastErr = fmt.Errorf("parse error: %w", parseErr)
					e.logger.Warn("plan parse error", "attempt", attempt, "error", parseErr)
					valErrs = []string{lastErr.Error()}
					allValErrs = append(allValErrs, fmt.Sprintf("attempt %d parse error: %s", attempt+1, lastErr.Error()))
					rawResponse = content
					continue
				}
				tasks = parsed
				planMarkdown = content
			} else {
				tasks = plan.Tasks
				planMarkdown = content

				if len(tasks) == 0 {
					parsed, jsonErr := parseTasksFromJSON(content)
					if jsonErr == nil {
						tasks = parsed
					} else {
						lastErr = fmt.Errorf("no tasks found in plan: %w", jsonErr)
						valErrs = []string{"no tasks found in plan output"}
						allValErrs = append(allValErrs, fmt.Sprintf("attempt %d: no tasks found", attempt+1))
						rawResponse = content
						continue
					}
				}
			}

			valErrs = validateTasks(tasks)
			if len(valErrs) > 0 {
				lastErr = fmt.Errorf("validation errors: %s", strings.Join(valErrs, "; "))
				e.logger.Warn("task validation errors", "attempt", attempt, "errors", valErrs)
				allValErrs = append(allValErrs, fmt.Sprintf("attempt %d validation: %s", attempt+1, strings.Join(valErrs, "; ")))
				rawResponse = content
				tasks = nil
				continue
			}

			break
		}
	}

	if len(tasks) == 0 {
		e.logger.Error("failed to generate valid plan after retries", "retries", m31types.MaxPlanRetries)
		combinedErrs := strings.Join(allValErrs, "\n")
		return &PhaseResult{
			Phase:               m31types.PhasePlan,
			Success:             false,
			Error:               fmt.Sprintf("Plan generation failed after %d attempts:\n%s", m31types.MaxPlanRetries, combinedErrs),
			RequiresManualInput: true,
		}, lastErr
	}

	// ── Sub-step 3: Plan Checker + Revision Loop ──────────────────────────
	checkEnabled := e.cfg != nil && e.cfg.Features.PlanCheck
	if !isRefinement && checkEnabled {
		tasks, planMarkdown = e.runPlanChecker(ctx, tasks, planMarkdown, goal)
	}

	// ── Sub-step 4: Coverage Gates ────────────────────────────────────────
	if !isRefinement {
		tasks, planMarkdown = e.runCoverageGates(ctx, tasks, planMarkdown, goal)
	}

	for i := range tasks {
		tasks[i].Status = m31types.StatusPending
	}

	e.state.planMarkdown = planMarkdown
	if !isRefinement {
		e.state.planVersion = 1
	}

	if err := e.sessionMgr.SavePlan(e.sessionID, e.state.planVersion, planMarkdown); err != nil {
		e.logger.Warn("save plan.md failed", "error", err)
	}

	// Prepare plan file for agent switching (plan mode → build mode)
	if planPath, err := e.PreparePlanFile(planMarkdown); err == nil {
		e.emit(AgentSwitchMsg{
			FromAgent:   "plan",
			ToAgent:     "build",
			PlanPath:    planPath,
			PlanContent: planMarkdown,
		})
	} else {
		e.logger.Warn("prepare plan file failed", "error", err)
	}

	if err := e.sessionMgr.SaveTasks(e.sessionID, tasks); err != nil {
		return nil, fmt.Errorf("save tasks: %w", err)
	}

	// Auto-generate initial TODO.md from plan tasks so the user sees progress
	// tracking from the start, before execution begins.
	if syncErr := e.dispatcher.SyncTodoFromTasks(tasks); syncErr != nil {
		e.logger.Warn("initial todo sync from plan failed", "error", syncErr)
	}

	if err := e.sessionMgr.SaveTasksCheckbox(e.sessionID, tasks); err != nil {
		e.logger.Warn("save checkbox tasks.md failed", "error", err)
	}

	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("checkpoint save failed", "error", err)
	}

	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhasePlan, fmt.Sprintf("%d tasks generated (v%d)", len(tasks), e.state.planVersion), "plan complete"); err != nil {
		return nil, fmt.Errorf("save state: %w", err)
	}

	e.state.refineFeedback = ""

	e.logger.Info("plan phase complete", "task_count", len(tasks), "version", e.state.planVersion)

	// Log plan decision
	e.LogDecision(decision.DecisionReceipt{
		Decision:  fmt.Sprintf("plan generated: %d tasks (v%d)", len(tasks), e.state.planVersion),
		Rationale: "plan phase complete",
		Category:  decision.CategoryPlan,
		Cost: decision.Cost{
			Attempts: e.state.planVersion,
		},
	})

	return &PhaseResult{
		Phase:   m31types.PhasePlan,
		Success: true,
		Tasks:   tasks,
	}, nil
}

// runPlanChecker executes the plan checker + revision loop.
// Returns the (possibly revised) tasks and plan markdown.
func (e *Engine) runPlanChecker(ctx context.Context, tasks []m31types.Task, planMarkdown string, goal string) ([]m31types.Task, string) {
	plan, parseErr := ParsePlan(planMarkdown)
	if parseErr != nil || plan == nil {
		plan = &m31types.Plan{Tasks: tasks, RawMarkdown: planMarkdown}
	}

	maxIter := e.maxPlanRevisions()
	prevIssueCount := 0

	for iteration := 1; iteration <= maxIter; iteration++ {
		checkResult, err := e.checkPlan(ctx, plan, goal)
		if err != nil {
			e.logger.Warn("plan check failed", "iteration", iteration, "error", err)
			break
		}

		blockers, warnings := countIssuesByType(checkResult.Issues)
		e.emit(PlanCheckMsg{
			Passed:     checkResult.Passed,
			IssueCount: len(checkResult.Issues),
			Blockers:   blockers,
			Warnings:   warnings,
		})

		if checkResult.Passed {
			e.logger.Info("plan check passed", "iteration", iteration)
			e.LogDecision(decision.DecisionReceipt{
				Decision:  fmt.Sprintf("plan check passed (iteration %d)", iteration),
				Rationale: fmt.Sprintf("%d issues resolved", prevIssueCount),
				Category:  decision.CategoryPlan,
			})
			break
		}

		if isPlanCheckStalled(len(checkResult.Issues), prevIssueCount) {
			e.logger.Warn("plan check stalled, breaking revision loop",
				"iteration", iteration, "issues", len(checkResult.Issues), "prev", prevIssueCount)
			e.LogDecision(decision.DecisionReceipt{
				Decision:  "plan check stalled, accepting current plan",
				Rationale: fmt.Sprintf("%d issues remain after %d iterations", len(checkResult.Issues), iteration),
				Category:  decision.CategoryPlan,
			})
			break
		}
		prevIssueCount = len(checkResult.Issues)

		e.emit(PlanRevisionMsg{
			Iteration:       iteration,
			MaxIterations:   maxIter,
			IssuesRemaining: len(checkResult.Issues),
		})

		revised, revErr := e.revisePlan(ctx, plan, checkResult.Issues, goal)
		if revErr != nil {
			e.logger.Warn("plan revision failed", "iteration", iteration, "error", revErr)
			break
		}

		valErrs := validateTasks(revised.Tasks)
		if len(valErrs) > 0 {
			e.logger.Warn("revised plan has validation errors", "iteration", iteration, "errors", valErrs)
			break
		}

		plan = revised
		tasks = revised.Tasks
		planMarkdown = revised.RawMarkdown

		e.logger.Info("plan revised", "iteration", iteration, "tasks", len(tasks))
	}

	return tasks, planMarkdown
}

// runCoverageGates executes pure-Go coverage gates and feeds blockers back
// into the plan checker revision loop if needed.
// Returns the (possibly revised) tasks and plan markdown.
func (e *Engine) runCoverageGates(ctx context.Context, tasks []m31types.Task, planMarkdown string, goal string) ([]m31types.Task, string) {
	plan, parseErr := ParsePlan(planMarkdown)
	if parseErr != nil || plan == nil {
		plan = &m31types.Plan{Tasks: tasks, RawMarkdown: planMarkdown}
	} else {
		plan.Tasks = tasks
	}

	var allIssues []PlanIssue

	// Granularity gate — always runs
	allIssues = append(allIssues, granularityGate(plan)...)

	// Security gate — if flag or heuristic
	secEnabled := e.cfg != nil && e.cfg.Features.PlanSecurityGate
	if secEnabled || hasSecurityKeywords(plan) {
		allIssues = append(allIssues, securityGate(plan)...)
	}

	// Gap analysis gate — if flag
	gapEnabled := e.cfg != nil && e.cfg.Features.PlanGapAnalysis
	if gapEnabled {
		allIssues = append(allIssues, gapAnalysisGate(plan, goal)...)
	}

	// Requirements coverage gate — if flag
	covEnabled := e.cfg != nil && e.cfg.Features.PlanCoverageGate
	if covEnabled {
		allIssues = append(allIssues, requirementsCoverageGate(plan, goal)...)
	}

	blockers, warnings := countIssuesByType(allIssues)
	if blockers > 0 {
		e.logger.Info("coverage gates found issues", "blockers", blockers, "warnings", warnings)
		// Feed blockers back into a single revision pass
		checkEnabled := e.cfg != nil && e.cfg.Features.PlanCheck
		if checkEnabled {
			e.emit(PlanCheckMsg{
				Passed:     false,
				IssueCount: len(allIssues),
				Blockers:   blockers,
				Warnings:   warnings,
			})
			revised, revErr := e.revisePlan(ctx, plan, allIssues, goal)
			if revErr == nil {
				valErrs := validateTasks(revised.Tasks)
				if len(valErrs) == 0 {
					return revised.Tasks, revised.RawMarkdown
				}
				e.logger.Warn("gate-revised plan has validation errors", "errors", valErrs)
			}
		}
	} else if warnings > 0 {
		e.logger.Info("coverage gates: warnings only", "warnings", warnings)
		for _, issue := range allIssues {
			e.logger.Warn("coverage gate warning", "category", issue.Category, "message", issue.Message)
		}
	}

	return tasks, planMarkdown
}

// runChunkedPlan executes the chunked planning path: outline → per-wave expansion.
func (e *Engine) runChunkedPlan(ctx context.Context, goal string) ([]m31types.Task, string, error) {
	outline, err := e.generateOutline(ctx, goal)
	if err != nil {
		return nil, "", fmt.Errorf("outline generation failed: %w", err)
	}

	e.logger.Info("plan outline generated", "waves", len(outline.Waves), "total_tasks", outline.TotalTasks)
	e.emit(IntermediateProgressMsg{
		Phase:   "plan",
		Message: fmt.Sprintf("Outline: %d tasks in %d waves", outline.TotalTasks, len(outline.Waves)),
	})

	var allTasks []m31types.Task
	var planSections []string

	for _, wave := range outline.Waves {
		waveTasks, waveErr := e.expandWave(ctx, outline, wave.Wave, goal)
		if waveErr != nil {
			e.logger.Warn("wave expansion failed", "wave", wave.Wave, "error", waveErr)
			// Fall back: use stub tasks from outline
			for _, stub := range wave.Tasks {
				allTasks = append(allTasks, m31types.Task{
					ID:                 stub.ID,
					Action:             stub.Action,
					Description:        stub.Description,
					Dependencies:       stub.Dependencies,
					Category:           stub.Category,
					Files:              []string{},
					AcceptanceCriteria: []string{stub.Description},
					Status:             m31types.StatusPending,
				})
			}
			continue
		}

		allTasks = append(allTasks, waveTasks...)
		// Save incrementally for crash resilience
		if saveErr := e.sessionMgr.SaveTasks(e.sessionID, allTasks); saveErr != nil {
			e.logger.Warn("incremental task save failed", "error", saveErr)
		}

		e.logger.Info("wave expanded", "wave", wave.Wave, "tasks", len(waveTasks))
	}

	if len(allTasks) == 0 {
		return nil, "", fmt.Errorf("chunked planning produced no tasks")
	}

	// Validate all tasks
	valErrs := validateTasks(allTasks)
	if len(valErrs) > 0 {
		e.logger.Warn("chunked plan validation errors", "errors", valErrs)
		// Non-fatal: log but continue
	}

	// Compose plan markdown from outline + tasks
	planMarkdown := composeChunkedPlanMarkdown(outline, allTasks, planSections)

	return allTasks, planMarkdown, nil
}

// composeChunkedPlanMarkdown builds a plan document from the outline and expanded tasks.
func composeChunkedPlanMarkdown(outline *PlanOutline, tasks []m31types.Task, sections []string) string {
	var sb strings.Builder
	fmt.Fprintf(&sb, "# %s\n\n", outline.Title)
	sb.WriteString("## Summary\n\nGenerated via chunked planning mode.\n\n")
	sb.WriteString("## Task List\n\n```json\n")
	taskJSON, _ := json.MarshalIndent(tasks, "", "  ")
	sb.WriteString(string(taskJSON))
	sb.WriteString("\n```\n")
	return sb.String()
}

// buildPlanContext creates messages for the plan phase.
// On refinement, the previous plan and user feedback are injected.
func (e *Engine) buildPlanContext(ctx context.Context, goal string, existingTasks []m31types.Task, validationErrors []string, rawResponse string) []m31types.Message {
	var messages []m31types.Message
	extras := []string{e.prompts.ToolUse, e.prompts.PlanFormat, e.prompts.ContextAwareness, e.prompts.CodeQuality, e.prompts.CodeIntelligence}
	if e.ScopeIncludes("website") && e.prompts.WebsiteBuild != "" {
		extras = append(extras, e.prompts.WebsiteBuild)
	}
	messages = append(messages, m31types.Message{Role: "system", Content: e.buildSystemPrompt(extras...)})

	project := e.loadProjectCached()
	projectType := "unknown"
	framework := ""
	if project != nil {
		projectType = project.ProjectType
		framework = project.Framework
	}

	planCtx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s\n\n", goal, projectType, framework)

	// Extract and inject website template path when scope includes "website"
	if e.ScopeIncludes("website") {
		if templateDir, err := e.ExtractWebsiteTemplateTo(); err == nil {
			planCtx += fmt.Sprintf("## Website Template\nA bundled Next.js template has been extracted to: %s\n\nThis template contains: package.json, next.config.ts, tsconfig.json, postcss.config.mjs, app/globals.css (full design system), app/layout.tsx, app/page.tsx, app/not-found.tsx, lib/utils.ts, lib/constants.ts, hooks/use-media-query.ts, hooks/use-scroll.ts.\n\nYou MUST copy these files to the working directory as the starting point. Then create additional components and pages as needed.\n\n", templateDir)
		} else {
			e.logger.Warn("failed to extract website template", "error", err)
		}
	}

	// Inject pre-plan research output if available
	if e.state.researchOutput != "" {
		planCtx += "## Research Findings\n" + e.state.researchOutput + "\n\n"
	}

	sessionDir := filepath.Dir(e.planningDir)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	if mem, err := os.ReadFile(memPath); err == nil {
		planCtx += "## Cross-Session Memory\n" + string(mem) + "\n\n"
	}

	fileSchema := listCwdFiles(e.workDir)
	if fileSchema != "" {
		planCtx += "Existing files:\n" + fileSchema + "\n\n"
	}

	// Codebase intelligence — structural overview for planning
	if ci := e.getCodeIntel(ctx); ci != nil {
		if summary := ci.ProjectSummary(4000); summary != "" {
			planCtx += summary + "\n"
		}
	}

	if project != nil && len(project.Answers) > 0 {
		planCtx += "User answers from Discuss phase:\n"
		for q, a := range project.Answers {
			planCtx += fmt.Sprintf("- Q: %s → A: %s\n", q, a)
		}
		planCtx += "\n"
	}

	if e.state.refineFeedback != "" && e.state.planMarkdown != "" {
		prevPlan := e.state.planMarkdown
		if len(prevPlan) > 4000 {
			prevPlan = "... (summary truncated)\n" + prevPlan[len(prevPlan)-4000:]
		}
		planCtx += "## Previous Plan (v" + fmt.Sprintf("%d", e.state.planVersion) + ")\n" + prevPlan + "\n\n"
		planCtx += "## User Refinement Feedback\n" + e.state.refineFeedback + "\n\n"
		planCtx += "Please revise the plan above based on the user's feedback. Return the complete revised plan in the same format.\n\n"
	}

	if len(existingTasks) > 0 || len(validationErrors) > 0 {
		planCtx += "## Previous Attempt Failed\n"
		if len(validationErrors) > 0 {
			planCtx += "Errors:\n"
			for _, e := range validationErrors {
				planCtx += "- " + e + "\n"
			}
			planCtx += "\n"
		}
		if rawResponse != "" {
			planCtx += "Previous LLM response (truncated):\n" + rawResponse[:min(len(rawResponse), 2000)] + "\n\n"
		}
		planCtx += "Please fix the issues above and return a corrected plan.\n\n"
	}

	messages = append(messages, m31types.Message{Role: "user", Content: planCtx})

	return messages
}
