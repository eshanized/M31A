package workflow

import (
	"context"
	"fmt"
	"os"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/session"
)

// runInitialize detects project type, initializes git if needed, and creates planning files.
func (e *Engine) runInitialize(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("initialize phase starting", "goal", goal)

	// 1. Parse goal
	project := &types.ProjectState{
		Goal:      goal,
		CreatedAt: time.Now(),
	}

	// Check for cancellation
	if err := ctx.Err(); err != nil {
		return nil, fmt.Errorf("initialize cancelled: %w", err)
	}

	// 2. Project type detection
	project.ProjectType = detectProjectType(e.workDir)
	e.logger.Info("detected project type", "type", project.ProjectType)

	// Check for cancellation
	if err := ctx.Err(); err != nil {
		return nil, fmt.Errorf("initialize cancelled: %w", err)
	}

	// ── Deep project analysis ─────────────────────────────────────────────
	deepEnabled := e.cfg != nil && e.cfg.Features.InitDeepAnalysis
	if deepEnabled {
		analysis := e.runDeepAnalysis(e.workDir)
		if analysis.Framework != "" {
			project.Framework = analysis.Framework
		}
		e.emit(InitAnalysisMsg{
			ProjectType:     analysis.ProjectType,
			Framework:       analysis.Framework,
			Language:        analysis.Language,
			DependencyCount: analysis.DependencyCount,
			TestFileRatio:   analysis.TestFileRatio,
			FileCount:       analysis.FileCount,
			HealthScore:     analysis.HealthScore,
		})
		e.logger.Info("deep analysis complete",
			"framework", analysis.Framework,
			"deps", analysis.DependencyCount,
			"test_ratio", analysis.TestFileRatio,
			"health", analysis.HealthScore)
	}

	// ── Environment pre-flight checks ─────────────────────────────────────
	preflightEnabled := e.cfg != nil && e.cfg.Features.InitPreflight
	if preflightEnabled {
		preflight := e.runEnvironmentPreflight()
		e.emit(InitPreflightMsg(preflight))
		if !preflight.Passed {
			e.logger.Warn("environment preflight found issues", "issues", preflight.Issues)
		}
	}

	// 3. Init git if not a repo
	if e.git == nil {
		return nil, fmt.Errorf("%w: call SetGit before runInitialize", m31errors.ErrGitNotInitialized)
	}
	if !e.git.IsRepo() {
		if err := e.git.Init(); err != nil {
			return nil, fmt.Errorf("git init: %w", err)
		}
		if err := e.git.ConfigUser(e.gitConfig().UserName, e.gitConfig().UserEmail); err != nil {
			return nil, fmt.Errorf("git config user: %w", err)
		}
		e.logger.Info("initialized git repository")
	}

	// 4. Create planning directory
	if err := os.MkdirAll(e.planningDir, types.DirPermission); err != nil {
		return nil, fmt.Errorf("create planning dir: %w", err)
	}

	// 5. Write PROJECT.md
	if err := e.sessionMgr.SaveProject(e.sessionID, project); err != nil {
		return nil, fmt.Errorf("save project: %w", err)
	}

	// 6. Write STATE.md
	if err := e.sessionMgr.SaveState(e.sessionID, types.PhaseInitialize, "initializing", "goal parsed"); err != nil {
		return nil, fmt.Errorf("save state: %w", err)
	}

	// 7. Save checkpoint
	cp := session.Checkpoint{
		Phase:     types.PhaseInitialize,
		Timestamp: time.Now(),
	}
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, cp); err != nil {
		e.logger.Warn("failed to save checkpoint", "error", err)
	}

	e.logger.Info("initialize phase complete")

	return &PhaseResult{
		Phase:   types.PhaseInitialize,
		Success: true,
	}, nil
}
