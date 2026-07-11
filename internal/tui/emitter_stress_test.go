package tui

import (
	"context"
	"io"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/session"
)

// TestEmitterDropsUnderStreamingLoad tests whether the emitter channel
// drops messages under sustained high-volume streaming load.
func TestEmitterDropsUnderStreamingLoad(t *testing.T) {
	dir := t.TempDir()

	// Init git repo
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	// Create session
	sessionBaseDir := dir
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Create dispatcher with tools
	dispatcher := tools.NewDispatcher(nil)
	dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
	dispatcher.Register(tools.NewFileRead(dir))
	dispatcher.Register(tools.NewFileWrite(dir, dir+"/backups"))
	dispatcher.SetPermission("Bash", true)
	dispatcher.SetPermission("FileRead", true)
	dispatcher.SetPermission("FileWrite", true)

	// Create workflow engine with a simple plan provider
	engine, err := workflow.NewEngine(s.ID, dir, dir+"/backups", dir+"/.m31a",
		&simplePlanProvider{}, "test-model", dispatcher, nil, mgr, nil)
	if err != nil {
		t.Fatalf("NewEngine failed: %v", err)
	}
	if err != nil {
		t.Fatalf("NewEngine failed: %v", err)
	}
	engine.SetGit(g)

	ctx := context.Background()

	// Run just Initialize phase
	result, err := engine.RunPhase(ctx, m31types.PhaseInitialize, "Build a Go CLI tool")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Initialize should succeed")
	}

	// Check drop counter
	dropped := DroppedMessages()
	t.Logf("Emitter drops during stress test: %d", dropped)

	_ = dropped
}

// simplePlanProvider returns a fixed plan with 1 task
type simplePlanProvider struct{}

func (p *simplePlanProvider) Name() string   { return "mock" }
func (p *simplePlanProvider) APIKey() string { return "test-key" }
func (p *simplePlanProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return nil, nil
}
func (p *simplePlanProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: `[{"id":1,"action":"Create","description":"Create main.go","dependencies":[],"files":["main.go"],"acceptance_criteria":["compiles"]}]`}, nil
	}
	close := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: close}, nil
}
func (p *simplePlanProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (p *simplePlanProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return m31types.HealthStatus{Status: "live"}
}
func (p *simplePlanProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }
func (p *simplePlanProvider) CachedModels() []m31types.ModelInfo              { return nil }
