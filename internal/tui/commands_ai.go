package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// M-30: default cooldown between /compress calls to prevent token burn.
var compressCooldown = types.CompressCooldown

// handleReset returns a result that transitions the TUI to the first-run screen.
// Requires --confirm flag to prevent accidental data loss.
func handleReset(args []string, ctx CommandContext) CommandResult {
	// Check for --confirm flag
	hasConfirm := false
	for _, arg := range args {
		if arg == "--confirm" {
			hasConfirm = true
			break
		}
	}

	if !hasConfirm {
		return CommandResult{
			Success: false,
			Message: "This will wipe all session data. Use /reset --confirm to proceed.",
		}
	}

	if ctx.AutoDream != nil {
		ctx.AutoDream.SetMessages(nil)
	}
	screen := ScreenFirstRun
	return CommandResult{Success: true, Screen: &screen, Message: "Resetting to first-run..."}
}

// handleCompress triggers AutoDream context consolidation.
// Subcommands:
//
//	/compress         — consolidate the conversation
//	/compress stats   — show consolidation statistics
//	/compress pause   — pause automatic consolidation
//	/compress resume  — resume automatic consolidation
func handleCompress(args []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream == nil {
		return CommandResult{Success: false, Message: "AutoDream not available."}
	}

	if len(args) > 0 {
		switch args[0] {
		case "stats":
			stats := ctx.AutoDream.Stats()
			var b strings.Builder
			b.WriteString("AutoDream statistics:\n")
			b.WriteString(fmt.Sprintf("  Total messages:        %v\n", stats["total_messages"]))
			b.WriteString(fmt.Sprintf("  Total consolidations:  %v\n", stats["total_consolidations"]))
			b.WriteString(fmt.Sprintf("  Paused:                %v\n", stats["paused"]))
			b.WriteString(fmt.Sprintf("  Estimated tokens:      %v\n", stats["estimated_tokens"]))
			if last, ok := stats["last_consolidation"].(string); ok && last != "" {
				b.WriteString(fmt.Sprintf("  Last consolidation:    %s\n", last))
			} else {
				b.WriteString("  Last consolidation:    never\n")
			}
			return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
		case "pause":
			ctx.AutoDream.Pause()
			return CommandResult{Success: true, Message: "AutoDream paused. Use /compress resume to re-enable."}
		case "resume":
			ctx.AutoDream.Resume()
			return CommandResult{Success: true, Message: "AutoDream resumed."}
		}
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

	// Pre-check: use CanConsolidate to fail fast with a friendly message.
	if !ctx.AutoDream.CanConsolidate() {
		return CommandResult{
			Success: false,
			Message: "Nothing to compress yet — conversation is still short or AutoDream is paused.",
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

	// Surface useful stats alongside the summary so the user sees the impact.
	msg := fmt.Sprintf("%s\n  Removed %d messages, saved ~%d tokens (%dms)",
		result.Summary, result.MessagesRemoved, result.TokensSaved, result.DurationMs)
	return CommandResult{Success: true, Message: msg}
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
