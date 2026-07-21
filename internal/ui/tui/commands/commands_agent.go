package commands

import (
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/tools/subagent"
)

// handleAgent spawns a parallel subagent from the REPL.
//
// Usage:
//
//	/agent <short description>: <full prompt>
//	/agent <prompt>
//
// Without arguments, lists the currently active subagents.
func handleAgent(args []string, ctx CommandContext) CommandResult {
	if ctx.SubagentManager == nil {
		return CommandResult{
			Success: false,
			Message: "Subagent manager not configured.",
		}
	}

	if len(args) == 0 {
		// List active subagents.
		infos := ctx.SubagentManager.List()
		if len(infos) == 0 {
			return CommandResult{
				Success: true,
				Message: "No subagents running. Use `/agent <description>: <prompt>` to spawn one.",
			}
		}
		var sb strings.Builder
		fmt.Fprintf(&sb, "**Subagents (%d):**\n\n", len(infos))
		for _, info := range infos {
			label := info.Name
			if label == "" {
				label = info.ID
			}
			fmt.Fprintf(&sb, "- `%s` **%s** — %s  \n  status: %s · tools: %d · tokens: %d+%d\n",
				info.ID, label, info.Description,
				info.Status, info.ToolCalls, info.InputToks, info.OutputToks)
		}
		return CommandResult{Success: true, Message: sb.String()}
	}

	joined := strings.Join(args, " ")
	description := joined
	prompt := joined
	if i := strings.Index(joined, ":"); i >= 0 {
		description = strings.TrimSpace(joined[:i])
		prompt = strings.TrimSpace(joined[i+1:])
	}
	if description == "" || prompt == "" {
		return CommandResult{
			Success: false,
			Message: "Usage: `/agent <description>: <prompt>` or `/agent <prompt>`",
		}
	}

	// Truncate overly long descriptions for display.
	if len([]rune(description)) > 48 {
		description = string([]rune(description)[:45]) + "…"
	}

	req := subagent.SpawnRequest{
		Description: description,
		Prompt:      prompt,
		Isolation:   subagent.IsolationWorktree,
		Background:  true,
	}
	id, _, err := ctx.SubagentManager.Spawn(ctx.Ctx, req)
	if err != nil {
		return CommandResult{
			Success: false,
			Message: "Failed to spawn subagent: " + err.Error(),
		}
	}
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Subagent **%s** spawned (`%s`). Progress appears in the subagent panel (ctrl+x).", description, id),
	}
}

// handleAgentCancel cancels a running subagent by ID.
func handleAgentCancel(args []string, ctx CommandContext) CommandResult {
	if ctx.SubagentManager == nil {
		return CommandResult{Success: false, Message: "Subagent manager not configured."}
	}
	if len(args) == 0 {
		return CommandResult{Success: false, Message: "Usage: `/agent-cancel <id>` (or `all`)"}
	}
	target := args[0]
	if target == "all" {
		ctx.SubagentManager.CancelAll()
		return CommandResult{Success: true, Message: "All subagents cancelled."}
	}
	ctx.SubagentManager.Cancel(target)
	return CommandResult{Success: true, Message: fmt.Sprintf("Subagent `%s` cancellation requested.", target)}
}
