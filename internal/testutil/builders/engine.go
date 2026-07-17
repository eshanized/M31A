// Package builders provides shared test setup functions for creating
// Engine and Dispatcher instances with sensible test defaults.
package builders

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/internal/testutil/mocks"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/workflow"
)

// NewTestEngine creates a workflow.Engine configured for testing with:
// - A temporary directory (auto-cleaned)
// - An initialized git repo
// - A session manager with a new session
// - A dispatcher with common test tools pre-registered and pre-approved
// - A token estimator
// - A MockProvider
//
// Returns the engine and a cleanup function (currently a no-op since
// t.TempDir handles cleanup, but the signature is preserved for
// backward compatibility with setupTestEngine callers).
func NewTestEngine(t *testing.T) (*workflow.Engine, func()) {
	t.Helper()
	dir := t.TempDir()

	// Init git repo
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	// Create session
	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	// Create dispatcher with common test tools
	dispatcher, err := tools.DefaultDispatcher(dir, filepath.Join(dir, "backups"), sessionBaseDir, nil, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	t.Cleanup(func() { dispatcher.Stop() })

	dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
	dispatcher.Register(tools.NewFileRead(dir))
	dispatcher.Register(tools.NewFileWrite(dir, filepath.Join(dir, "backups")))
	dispatcher.Register(tools.NewEdit(dir, filepath.Join(dir, "backups")))
	dispatcher.Register(tools.NewGlob(dir))
	dispatcher.Register(tools.NewGrep(dir))
	// Pre-approve all tools for tests
	dispatcher.SetPermission("Bash", true)
	dispatcher.SetPermission("FileRead", true)
	dispatcher.SetPermission("FileWrite", true)
	dispatcher.SetPermission("FileEdit", true)
	dispatcher.SetPermission("Glob", true)
	dispatcher.SetPermission("Grep", true)

	est := tokens.NewEstimator("test-model")
	provider := mocks.NewMockProvider("mock")

	engine, err := workflow.NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		provider, "test-model", dispatcher, est, mgr, nil)
	if err != nil {
		t.Fatalf("NewEngine failed: %v", err)
	}
	engine.SetGit(g)

	cleanup := func() {}
	return engine, cleanup
}
