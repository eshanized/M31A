package commands

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/tools"
)

func TestAnalysisCommands_Register(t *testing.T) {
	t.Parallel()

	d, err := tools.NewDispatcher("", "", "", &config.PermissionsConfig{}, &config.ToolsConfig{})
	if err != nil {
		t.Fatalf("NewDispatcher failed: %v", err)
	}
	if d == nil {
		t.Fatal("expected non-nil dispatcher")
	}
}
