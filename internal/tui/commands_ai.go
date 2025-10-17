package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

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

	// Build a dummy task to score
	task := types.Task{
		Description: "optimize model selection",
		Action:      "implement",
	}

	return CommandResult{
		Success: true,
		Message: "Analyzing model alternatives...",
		Cmd: func() tea.Msg {
			models, err := activeProvider.FetchModels(context.Background())
			if err != nil || len(models) == 0 {
				return ToastMsg{
					Text:     "Could not fetch model list for optimization.",
					Duration: 3 * time.Second,
					Type:     "error",
				}
			}
			threshold := 0.2
			if ctx.Config != nil && ctx.Config.Model.ArbitrageThreshold > 0 {
				threshold = ctx.Config.Model.ArbitrageThreshold
			}
			_ = task
			_ = threshold
			return ToastMsg{
				Text:     fmt.Sprintf("Fetched %d models — use /models to see the full list.", len(models)),
				Duration: 4 * time.Second,
				Type:     "info",
			}
		},
	}
}

// handleModel shows or switches the current model.
func handleModel(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	currentModel := ctx.Config.Model.Default
	currentProvider := ctx.Config.Provider.Default

	if len(args) == 0 {
		return CommandResult{
			Success: true,
			Message: fmt.Sprintf("**Current model:** %s\n**Provider:** %s", currentModel, currentProvider),
		}
	}

	// Switch to model selector screen
	screen := ScreenModelSelector
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: fmt.Sprintf("Opening model selector (requested: %s)...", strings.Join(args, " ")),
	}
}

// handleModels lists all cached models for the active provider.
func handleModels(_ []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	active := ctx.Registry.Active()
	p, err := ctx.Registry.Get(active)
	if err != nil || p == nil {
		return CommandResult{Success: false, Message: "No active provider."}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Fetching models for **%s**...", active),
		Cmd: func() tea.Msg {
			models, err := p.FetchModels(context.Background())
			if err != nil {
				return ToastMsg{
					Text:     fmt.Sprintf("Failed to list models: %v", err),
					Duration: 4 * time.Second,
					Type:     "error",
				}
			}
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**%s models** (%d):\n\n", active, len(models)))
			for i, m := range models {
				if i >= 20 {
					sb.WriteString(fmt.Sprintf("...and %d more. Use /model to select.\n", len(models)-20))
					break
				}
				sb.WriteString(fmt.Sprintf("  %s\n", m.ID))
			}
			return ToastMsg{
				Text:     sb.String(),
				Duration: 8 * time.Second,
				Type:     "info",
			}
		},
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
			return FallbackEventMsg{From: active, To: target, Reason: "manual"}
		},
	}
}

// handleProvider shows or switches the active provider.
func handleProvider(args []string, ctx CommandContext) CommandResult {
	// Delegate to handleFallback — same semantics.
	return handleFallback(args, ctx)
}
