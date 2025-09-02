package workflow

import (
	"context"
	"fmt"
	"os"
	"time"

	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// runInitialize detects project type, initializes git if needed, and creates planning files.
func (e *Engine) runInitialize(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("initialize phase starting", "goal", goal)

	// 1. Parse goal
	project := &types.ProjectState{
		Goal:      goal,
		CreatedAt: time.Now(),
	}

	// 2. Project type detection
	project.ProjectType = detectProjectType(e.workDir)
	e.logger.Info("detected project type", "type", project.ProjectType)

	// 3. Init git if not a repo
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

	// Auto-transition to Discuss
	if err := e.Transition(ctx, types.PhaseInitialize, types.PhaseDiscuss); err != nil {
		return &PhaseResult{
			Phase:   types.PhaseInitialize,
			Success: true,
		}, fmt.Errorf("transition to discuss: %w", err)
	}

	return &PhaseResult{
		Phase:   types.PhaseInitialize,
		Success: true,
	}, nil
}
