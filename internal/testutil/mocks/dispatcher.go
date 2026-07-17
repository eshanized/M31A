package mocks

import (
	"context"

	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/workflow"
)

// WorkflowDispatcher implements workflow.Dispatcher for testing.
// This is the minimal mock for phase coordinator tests that only need
// RevokeBatchApprovals().
type WorkflowDispatcher struct {
	Revoked bool
}

// Compile-time interface check.
var _ workflow.Dispatcher = (*WorkflowDispatcher)(nil)

func (d *WorkflowDispatcher) RevokeBatchApprovals() {
	d.Revoked = true
}

// SubagentDispatcher implements subagent.ToolDispatcher for testing.
// This provides a minimal mock for subagent loop tests.
type SubagentDispatcher struct {
	Tools []subagent.ToolDescriptor
}

// Compile-time interface check.
var _ subagent.ToolDispatcher = (*SubagentDispatcher)(nil)

func (d *SubagentDispatcher) Execute(_ context.Context, _ subagent.ToolCallInput) (subagent.ToolCallOutput, error) {
	return subagent.ToolCallOutput{Output: "ok"}, nil
}

func (d *SubagentDispatcher) ListTools() []subagent.ToolDescriptor {
	return d.Tools
}

func (d *SubagentDispatcher) UnregisterTool(name string) {
	newTools := make([]subagent.ToolDescriptor, 0, len(d.Tools))
	for _, t := range d.Tools {
		if t.Name != name {
			newTools = append(newTools, t)
		}
	}
	d.Tools = newTools
}

func (d *SubagentDispatcher) Stop() {}

func (d *SubagentDispatcher) SetPermission(_ string, _ bool) {}
