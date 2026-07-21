package workflow

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/core/types"
)

type raceMockProvider struct {
	provider.LLMProvider
}

func (m *raceMockProvider) Name() string   { return "mock" }
func (m *raceMockProvider) APIKey() string { return "test-key" }
func (m *raceMockProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return nil, nil
}
func (m *raceMockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: "test"}, nil
	}
	close := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: close}, nil
}
func (m *raceMockProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (m *raceMockProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return m31types.HealthStatus{Status: "live"}
}
func (m *raceMockProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }
func (m *raceMockProvider) CachedModels() []m31types.ModelInfo              { return nil }

func setupRaceTestEngine(t *testing.T) *Engine {
	t.Helper()
	dir := t.TempDir()

	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	dispatcher, err := tools.DefaultDispatcher(dir, filepath.Join(dir, "backups"), sessionBaseDir, nil, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	t.Cleanup(func() { dispatcher.Stop() })
	dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
	dispatcher.Register(tools.NewFileRead(dir))
	dispatcher.Register(tools.NewFileWrite(dir, filepath.Join(dir, "backups")))
	dispatcher.SetPermission("Bash", true)
	dispatcher.SetPermission("FileRead", true)
	dispatcher.SetPermission("FileWrite", true)

	est := tokens.NewEstimator("test-model")

	engine, _ := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		&raceMockProvider{}, "test-model", dispatcher, est, mgr, nil)
	engine.git = g

	return engine
}

// TestConcurrentPlanReadDuringWrite tests for race conditions between
// concurrent writes to planMarkdown/planVersion (in workflow goroutine)
// and reads via PlanContent()/PlanVersion() (from TUI thread).
//
// This test uses the mutex-protected write path (simulating the fixed workflow behavior)
// and the mutex-protected read path to verify the race is fixed.
func TestConcurrentPlanReadDuringWrite(t *testing.T) {
	engine := setupRaceTestEngine(t)

	var wg sync.WaitGroup
	wg.Add(2)

	// Writer goroutine - simulates workflow goroutine writing plan state
	// using the mutex-protected write path (as the fixed workflow does)
	go func() {
		defer wg.Done()
		for i := 0; i < 10000; i++ {
			// Use the mutex directly like the fixed workflow code does
			engine.state.planMu.Lock()
			engine.state.planMarkdown = "# Plan " + string(rune('A'+(i%26)))
			engine.state.planVersion = i
			engine.state.planMu.Unlock()
			time.Sleep(time.Microsecond)
		}
	}()

	// Reader goroutine - simulates TUI calling PlanContent()/PlanVersion()
	go func() {
		defer wg.Done()
		for i := 0; i < 10000; i++ {
			_ = engine.PlanContent()
			_ = engine.PlanVersion()
			time.Sleep(time.Microsecond)
		}
	}()

	wg.Wait()
}
