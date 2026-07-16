package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/eshanized/M31A/pkg/types"
)

// ─── Message helpers ──────────────────────────────────────────────────────────

// activeModelID safely extracts the model ID from a potentially nil ModelInfo.
func activeModelID(m *types.ModelInfo) string {
	if m == nil {
		return ""
	}
	return m.ID
}

// makeAssistantMsg creates a standard assistant message with role, content, and segment.
func makeAssistantMsg(content string) types.Message {
	return types.Message{
		Role:    "assistant",
		Content: content,
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// makeErrorBannerMsg creates an assistant message carrying a plain-text error
// banner. Styling is applied by the message renderer (not here) so that ANSI
// escape codes never enter the markdown pipeline and get mangled.
func makeErrorBannerMsg(text string, providerName string) types.Message {
	content := text
	if providerName != "" {
		content = text + " (" + providerName + ")"
	}
	return types.Message{
		Role:    "assistant",
		Content: content,
		Segments: []types.MessageSegment{{
			Type:    "error",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// makeUserMsg creates a standard user message with proper rendering properties.
func makeUserMsg(content string) types.Message {
	return makeUserMsgWithSkip(content, false)
}

// makeUserMsgWithSkip creates a user message with an optional SkipForLLM flag.
// When skipForLLM is true, the message is displayed in the REPL but excluded
// from the LLM chat history. This prevents double-sending when sendChatMessage
// replaces the display message with an enriched (or plain) version.
func makeUserMsgWithSkip(content string, skipForLLM bool) types.Message {
	return types.Message{
		Role:       "user",
		Content:    content,
		SkipForLLM: skipForLLM,
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// ─── Formatting utilities ──────────────────────────────────────────────────────

// formatSI formats an integer with SI suffix (K, M).
func formatSI(n int) string {
	switch {
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fM", float64(n)/1_000_000)
	case n >= 1_000:
		return fmt.Sprintf("%.0fK", float64(n)/1_000)
	default:
		return fmt.Sprintf("%d", n)
	}
}

// formatDurationMs formats a duration in milliseconds as a human-readable string.
func formatDurationMs(ms int64) string {
	if ms < 0 {
		return "0s"
	}
	s := ms / 1000
	m := s / 60
	h := m / 60
	switch {
	case h > 0:
		return fmt.Sprintf("%dh%dm", h, m%60)
	case m > 0:
		return fmt.Sprintf("%dm%ds", m, s%60)
	default:
		return fmt.Sprintf("%ds", s)
	}
}

// ProviderShortName returns a short display name for a provider.
func ProviderShortName(name string) string {
	switch strings.ToLower(name) {
	case types.ProviderOpenRouter:
		return "OR"
	case types.ProviderZen, "zen-gateway":
		return "Zen"
	case "openai":
		return "OAI"
	case "anthropic":
		return "AC"
	case types.ProviderNvidia, "nim":
		return "NV"
	default:
		if len(name) > 4 {
			return name[:4]
		}
		return name
	}
}
