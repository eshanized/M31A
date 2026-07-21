package mocks

import (
	"context"

	"github.com/eshanized/M31A/internal/core/types"
)

// MockTool implements types.Tool for testing.
type MockTool struct {
	Name_        string
	Description_ string
	RiskLevel_   types.RiskLevel
	ExecFunc     func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}

// NewMockTool creates a MockTool with the given name and risk level.
func NewMockTool(name string, riskLevel types.RiskLevel) *MockTool {
	return &MockTool{
		Name_:        name,
		Description_: "mock tool for testing",
		RiskLevel_:   riskLevel,
	}
}

func (m *MockTool) Name() string               { return m.Name_ }
func (m *MockTool) Description() string        { return m.Description_ }
func (m *MockTool) RiskLevel() types.RiskLevel { return m.RiskLevel_ }

func (m *MockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	if m.ExecFunc != nil {
		return m.ExecFunc(ctx, input)
	}
	return types.ToolResult{Output: "ok"}, nil
}
