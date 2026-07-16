package tools

import (
	"context"
	"strings"
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func TestCodeMap_Name(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	if cm.Name() != "CodeMap" {
		t.Errorf("expected 'CodeMap', got %s", cm.Name())
	}
}

func TestCodeMap_Description(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	if cm.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestCodeMap_RiskLevel(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	if cm.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", cm.RiskLevel())
	}
}

func TestCodeMap_ParameterSchema(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	schema := cm.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
}

func TestCodeMap_Execute_MissingQuery(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	_, err := cm.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"mode": "upstream",
		},
	})
	if err == nil {
		t.Fatal("expected error for missing query")
	}
	if !strings.Contains(err.Error(), "query") {
		t.Errorf("expected query error, got: %v", err)
	}
}

func TestCodeMap_Execute_MissingMode(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	_, err := cm.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": "foo",
		},
	})
	if err == nil {
		t.Fatal("expected error for missing mode")
	}
	if !strings.Contains(err.Error(), "mode") {
		t.Errorf("expected mode error, got: %v", err)
	}
}

func TestCodeMap_Execute_UnknownMode(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	_, err := cm.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": "foo",
			"mode":  "nonexistent",
		},
	})
	if err == nil {
		t.Fatal("expected error for unknown mode")
	}
	if !strings.Contains(err.Error(), "unknown mode") {
		t.Errorf("expected unknown mode error, got: %v", err)
	}
}

func TestCodeMap_FormatFileList(t *testing.T) {
	t.Parallel()
	result := formatFileList("Files", nil)
	if !strings.Contains(result, "none found") {
		t.Errorf("expected 'none found', got: %s", result)
	}

	result = formatFileList("Files", []string{"a.go", "b.go"})
	if !strings.Contains(result, "2 files") {
		t.Errorf("expected '2 files', got: %s", result)
	}
	if !strings.Contains(result, "a.go") {
		t.Errorf("expected 'a.go', got: %s", result)
	}
}

func TestCodeMap_FormatSymbolLocations(t *testing.T) {
	t.Parallel()
	result := formatSymbolLocations("Symbols", nil)
	if !strings.Contains(result, "not found") {
		t.Errorf("expected 'not found', got: %s", result)
	}
}

func TestCodeMap_FormatScoredFiles(t *testing.T) {
	t.Parallel()
	result := formatScoredFiles("Files", nil)
	if !strings.Contains(result, "no relevant files") {
		t.Errorf("expected 'no relevant files', got: %s", result)
	}
}

func TestCodeMap_DepthClamping(t *testing.T) {
	t.Parallel()
	cm := NewCodeMap(t.TempDir())
	// Test with invalid depth values - should not panic
	_, err := cm.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": "test",
			"mode":  "upstream",
			"depth": float64(-1),
		},
	})
	// Will fail because indexer can't build, but shouldn't panic on depth
	if err == nil {
		t.Log("no error (indexer may have succeeded on empty dir)")
	}
}
