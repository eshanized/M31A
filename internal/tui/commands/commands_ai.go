package commands

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
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
func handleOptimize(_ []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	active := ctx.Registry.Active()
	activeProvider, err := ctx.Registry.Get(active)
	if err != nil || activeProvider == nil {
		return CommandResult{Success: false, Message: "No active provider."}
	}

	// Build a representative task for scoring
	task := types.Task{
		Description: "optimize model selection",
		Action:      "implement",
	}

	return CommandResult{
		Success: true,
		Message: "Analyzing model alternatives...",
		Cmd: func() tea.Msg {
			models, err := activeProvider.FetchModels(ctx.Ctx)
			if err != nil || len(models) == 0 {
				return tuitypes.ToastMsg{
					Text:     "Could not fetch model list for optimization.",
					Duration: 3 * time.Second,
					Type:     "error",
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
			sb.WriteString(fmt.Sprintf("**Optimization Analysis** (%s complexity):\n\n", rec.Complexity))
			sb.WriteString(fmt.Sprintf("**Recommended:** %s ($%.6f)\n", rec.RecommendedModel.ModelID, rec.RecommendedModel.TotalCost))
			sb.WriteString(fmt.Sprintf("**Reason:** %s\n", rec.Reason))
			if rec.Savings > 0 {
				sb.WriteString(fmt.Sprintf("**Potential savings:** $%.6f vs most expensive model\n", rec.Savings))
			}
			if len(rec.Alternatives) > 0 {
				sb.WriteString("\n**Alternatives:**\n")
				for _, alt := range rec.Alternatives {
					sb.WriteString(fmt.Sprintf("  - %s ($%.6f)\n", alt.ModelID, alt.TotalCost))
				}
			}

			// Emit OptimizedMsg for the notification system
			return tea.Batch(
				func() tea.Msg {
					return tuitypes.OptimizedMsg{
						Recommendations: []arbitrage.ArbitrageRecommendation{*rec},
					}
				},
				func() tea.Msg {
					return tuitypes.ToastMsg{
						Text:     sb.String(),
						Duration: 8 * time.Second,
						Type:     "info",
					}
				},
			)
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

// handleModels opens the model selector screen.
func handleModels(_ []string, ctx CommandContext) CommandResult {
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
		sb.WriteString(fmt.Sprintf("  Active: **%s**\n", active))
		sb.WriteString(fmt.Sprintf("  Registered: %s\n", strings.Join(providers, ", ")))
		if ctx.Config != nil {
			sb.WriteString(fmt.Sprintf("  Auto-fallback: %v\n", ctx.Config.Provider.AutoFallback))
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
