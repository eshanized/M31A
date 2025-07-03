package tui

import (
	"fmt"
	"strings"
	"time"
)

// M-30: default cooldown between /compress calls to prevent token burn.
const compressCooldown = 60 * time.Second

// handleReset returns a result that transitions the TUI to the first-run screen.
func handleReset(args []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream != nil {
		ctx.AutoDream.SetMessages(nil)
	}
	screen := ScreenFirstRun
	return CommandResult{Success: true, Screen: &screen, Message: "Resetting to first-run..."}
}

// handleCompress triggers AutoDream context consolidation.
func handleCompress(args []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream == nil {
		return CommandResult{Success: false, Message: "AutoDream not available."}
	}

	// M-30: Enforce cooldown between compress calls
	if ctx.CmdRegistry != nil {
		if !ctx.CmdRegistry.lastCompressTime.IsZero() &&
			time.Since(ctx.CmdRegistry.lastCompressTime) < compressCooldown {
			remaining := compressCooldown - time.Since(ctx.CmdRegistry.lastCompressTime)
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("/compress cooldown: try again in %ds", int(remaining.Seconds())),
			}
		}
	}

	result := ctx.AutoDream.Consolidate()

	// Record compress time after attempt (even on failure, to prevent rapid retry spam)
	if ctx.CmdRegistry != nil {
		ctx.CmdRegistry.lastCompressTime = time.Now()
	}

	if result.Error != "" {
		return CommandResult{Success: false, Message: fmt.Sprintf("Consolidation failed: %s", result.Error)}
	}
	return CommandResult{Success: true, Message: result.Summary}
}

// handleTokens estimates token count for provided text.
func handleTokens(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		return CommandResult{Success: false, Message: "Usage: /tokens <text to estimate>"}
	}

	text := strings.Join(args, " ")

	runes := len([]rune(text))
	estimated := float64(runes) / 4.0

	var b strings.Builder
	b.WriteString("Token estimation:\n")
	b.WriteString(fmt.Sprintf("  Characters: %d\n", runes))
	b.WriteString(fmt.Sprintf("  Words:      %d\n", len(strings.Fields(text))))
	b.WriteString(fmt.Sprintf("  Est. tokens: ~%.0f (English text, ~4 chars/token)\n", estimated))
	b.WriteString("Note: Actual token count depends on model tokenizer.")
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}
