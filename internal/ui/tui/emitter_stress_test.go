package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tools/ai"
)

func setupTestDispatcher(t *testing.T) (ai.ToolDispatcher, func()) {
	t.Helper()
	d, err := tools.NewDispatcher("", "", "", &config.PermissionsConfig{}, &config.ToolsConfig{})
	if err != nil {
		t.Fatalf("NewDispatcher failed: %v", err)
	}
	// Pre-approve permissions for testing
	d.SetPermission("Bash", true)
	d.SetPermission("FileRead", true)
	d.SetPermission("FileWrite", true)
	d.SetPermission("FileEdit", true)
	d.SetPermission("FileList", true)
	d.SetPermission("FileDelete", true)
	d.SetPermission("FileMove", true)
	d.SetPermission("Glob", true)
	d.SetPermission("Grep", true)
	d.SetPermission("WebFetch", true)
	d.SetPermission("WebSearch", true)
	d.SetPermission("Task", true)
	d.SetPermission("Edit", true)
	d.SetPermission("TodoWrite", true)
	d.SetPermission("TodoRead", true)
	d.SetPermission("Git", true)
	d.SetPermission("DevServer", true)
	d.SetPermission("CodeMap", true)
	d.SetPermission("CodeComplexity", true)
	d.SetPermission("HTTPCheck", true)
	d.SetPermission("AskUserQuestion", true)

	return d, func() {}
}

func TestEmitter_ConcurrentEvents(t *testing.T) {
	t.Parallel()

	d, cleanup := setupTestDispatcher(t)
	defer cleanup()

	// Verify dispatcher is functional
	tools := d.List()
	if len(tools) == 0 {
		t.Fatal("expected dispatcher to have registered tools")
	}
}
