package commands

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/integrations/arbitrage"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"github.com/eshanized/M31A/internal/types"
)

// handleMemory manages context memory (AutoDream consolidation).
func handleMemory(args []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream == nil {
		return CommandResult{Success: false, Message: "Context memory not available."}
	}

	subcmd := "view"
	if len(args) > 0 {
		subcmd = strings.ToLower(args[0])
	}

	switch subcmd {
	case "view":
		stats := ctx.AutoDream.Stats()
		paused := "no"
		if stats["paused"] == true {
			paused = "yes"
		}
		return CommandResult{
			Success: true,
			Message: fmt.Sprintf(
				"**Context Memory:**\n  Messages: %v\n  Consolidations: %v\n  Paused: %s\n  Estimated tokens: %v\n  Last consolidation: %v",
				stats["total_messages"], stats["total_consolidations"], paused, stats["estimated_tokens"], stats["last_consolidation"],
			),
		}
	case "pause":
		ctx.AutoDream.Pause()
		return CommandResult{Success: true, Message: "Auto-compression paused. Use `/memory resume` to re-enable."}
	case "resume":
		ctx.AutoDream.Resume()
		return CommandResult{Success: true, Message: "Auto-compression resumed."}
	case "revert":
		// Revert: pause consolidation and reload original messages from session
		ctx.AutoDream.Pause()
		return CommandResult{
			Success: true,
			Message: "Context memory consolidation paused and state reset. Use `/memory resume` to re-enable, or `/clear` to start fresh.",
		}
	default:
		return CommandResult{
			Success: false,
			Message: "Usage: `/memory [view|pause|resume|revert]`",
		}
	}
}

// handleCompress triggers context consolidation via AutoDream.
func handleCompress(_ []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream == nil {
		return CommandResult{Success: false, Message: "AutoDream consolidator not available."}
	}

	if !ctx.AutoDream.CanConsolidate() {
		return CommandResult{
			Success: false,
			Message: "Nothing to compress yet — conversation is still short or consolidation is paused.",
		}
	}

	result := ctx.AutoDream.Consolidate()
	if !result.Success {
		return CommandResult{Success: false, Message: fmt.Sprintf("Compression failed: %s", result.Error)}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf(
			"✅ Context compressed: **%d** messages removed, ~**%d** tokens saved (%dms).",
			result.MessagesRemoved, result.TokensSaved, result.DurationMs,
		),
	}
}

// handleOptimize suggests cheaper model alternatives via the arbitrage engine.
func handleOptimize(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	active := ctx.Registry.Active()
	activeProvider, err := ctx.Registry.Get(active)
	if err != nil || activeProvider == nil {
		return CommandResult{Success: false, Message: "No active provider."}
	}

	// Build task from user-provided description, or use a generic default.
	desc := "general optimization"
	if len(args) > 0 {
		desc = strings.Join(args, " ")
	}
	task := types.Task{
		Description: desc,
		Action:      "implement",
	}

	return CommandResult{
		Success: true,
		Message: "Analyzing model alternatives...",
		Cmd: func() tea.Msg {
			// Try cached models first for instant results.
			models := activeProvider.CachedModels()
			if len(models) == 0 {
				// Fall back to fetch with timeout.
				fetchCtx, cancel := context.WithTimeout(ctx.Ctx, types.FetchModelsTimeout)
				defer cancel()
				models, err = activeProvider.FetchModels(fetchCtx)
				if err != nil || len(models) == 0 {
					return tuitypes.ToastMsg{
						Text:     "Could not fetch model list for optimization.",
						Duration: 3 * time.Second,
						Type:     "error",
					}
				}
			}
			threshold := 0.2
			if ctx.Config != nil && ctx.Config.Model.ArbitrageThreshold > 0 {
				threshold = ctx.Config.Model.ArbitrageThreshold
			}

			// Use arbitrage engine to recommend cheapest model
			rec, err := arbitrage.Recommend(models, task, threshold)
			if err != nil {
				return tuitypes.ToastMsg{
					Text:     fmt.Sprintf("Optimization analysis failed: %v", err),
					Duration: 4 * time.Second,
					Type:     "error",
				}
			}

			// Build result message
			var sb strings.Builder
			fmt.Fprintf(&sb, "**Optimization Analysis** (%s task, %s complexity):\n\n", desc, rec.Complexity)
			fmt.Fprintf(&sb, "**Recommended:** %s ($%.6f)\n", rec.RecommendedModel.ModelID, rec.RecommendedModel.TotalCost)
			fmt.Fprintf(&sb, "**Reason:** %s\n", rec.Reason)
			if rec.Savings > 0 {
				fmt.Fprintf(&sb, "**Potential savings:** $%.6f vs most expensive model\n", rec.Savings)
			}
			if len(rec.Alternatives) > 0 {
				sb.WriteString("\n**Alternatives:**\n")
				for _, alt := range rec.Alternatives {
					fmt.Fprintf(&sb, "  - %s ($%.6f)\n", alt.ModelID, alt.TotalCost)
				}
			}

			return tuitypes.ToastMsg{
				Text:     sb.String(),
				Duration: 10 * time.Second,
				Type:     "info",
			}
		},
	}
}

// handleModel opens the model selector screen.
func handleModel(args []string, ctx CommandContext) CommandResult {
	screen := tuitypes.ScreenModelSelector
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Opening model selector...",
	}
}

// handleFallback shows fallback provider status or switches provider.
func handleFallback(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	active := ctx.Registry.Active()
	providers := ctx.Registry.ListAll()

	if len(args) == 0 {
		var sb strings.Builder
		sb.WriteString("**Provider fallback status:**\n\n")
		fmt.Fprintf(&sb, "  Active: **%s**\n", active)
		fmt.Fprintf(&sb, "  Registered: %s\n", strings.Join(providers, ", "))
		if ctx.Config != nil {
			fmt.Fprintf(&sb, "  Auto-fallback: %v\n", ctx.Config.Provider.AutoFallback)
		}
		return CommandResult{Success: true, Message: sb.String()}
	}

	target := args[0]
	if err := ctx.Registry.SetActive(target); err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Cannot switch to provider %q: %v", target, err),
		}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Switched provider from **%s** to **%s**.", active, target),
		Cmd: func() tea.Msg {
			return tuitypes.FallbackEventMsg{From: active, To: target, Reason: "manual"}
		},
	}
}

// handleProvider shows or switches the active provider.
func handleProvider(args []string, ctx CommandContext) CommandResult {
	// Delegate to handleFallback — same semantics.
	return handleFallback(args, ctx)
}
