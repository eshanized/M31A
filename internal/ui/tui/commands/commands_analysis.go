package commands

import (
	"context"
	"fmt"

	"github.com/eshanized/M31A/internal/core/types"
)

// handleComplexity executes the CodeComplexity tool and returns the report.
func handleComplexity(_ []string, ctx CommandContext) CommandResult {
	if ctx.Dispatcher == nil {
		return CommandResult{Success: false, Message: "Tool dispatcher not available."}
	}

	tool, ok := ctx.Dispatcher.GetTool("CodeComplexity")
	if !ok {
		return CommandResult{Success: false, Message: "CodeComplexity tool not registered."}
	}

	input := types.ToolInput{
		Name:   "CodeComplexity",
		Params: map[string]any{},
	}
	result, err := tool.Execute(context.Background(), input)
	if err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Complexity analysis failed: %v", err),
		}
	}
	if result.Error != "" {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Complexity analysis failed: %s", result.Error),
		}
	}

	return CommandResult{Success: true, Message: result.Output}
}
