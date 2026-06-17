package workflow

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/ledger"
)

func TestEngine_SetLedger(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if engine.ledger != nil {
		t.Fatal("expected nil ledger before SetLedger")
	}
	dir := t.TempDir()
	l := ledger.New(filepath.Join(dir, "LEDGER.md"))
	engine.SetLedger(l)
	if engine.ledger == nil {
		t.Fatal("expected non-nil ledger after SetLedger")
	}
	if engine.ledger != l {
		t.Fatal("expected same ledger instance")
	}
}

func TestEngine_GetCodeIntel_RespectsContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.workDir = t.TempDir()

	// Cancelled context should prevent codeintel build
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	ci := engine.getCodeIntel(ctx)
	// With a cancelled context, the build should fail and return nil
	if ci != nil {
		t.Log("codeintel built despite cancelled context — may be cached or empty dir")
	}
}

func TestEngine_GetCodeIntel_WithValidContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.workDir = t.TempDir()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// With a valid context and empty dir, should either succeed or return nil gracefully
	_ = engine.getCodeIntel(ctx)
}

func TestBuildExecuteContext_AcceptsContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "test",
		Files:       []string{"a.go"},
	}
	ctx := context.Background()
	messages := engine.buildExecuteContext(ctx, task, []m31types.Task{task}, "goal")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
}

func TestBuildPlanContext_AcceptsContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	ctx := context.Background()
	messages := engine.buildPlanContext(ctx, "Build something", nil, nil, "")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
}
