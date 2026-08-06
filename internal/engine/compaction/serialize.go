package compaction

import (
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/core/types"
)

const maxToolOutputChars = 2000

// SerializeMessages converts a slice of messages into a compactable text
// representation. Tool outputs are truncated to maxToolOutputChars to keep
// the summary lean.
func SerializeMessages(messages []types.Message) string {
	var sb strings.Builder
	for i, msg := range messages {
		if msg.SkipForLLM {
			continue
		}
		switch msg.Role {
		case "system":
			fmt.Fprintf(&sb, "[System %d] %s\n", i, truncate(msg.Content, maxToolOutputChars))
		case "user":
			fmt.Fprintf(&sb, "[User %d] %s\n", i, msg.Content)
		case "assistant":
			fmt.Fprintf(&sb, "[Assistant %d] %s\n", i, msg.Content)
			for _, tc := range msg.ToolCalls {
				input := string(tc.Input)
				if len(input) > 500 {
					input = input[:500] + "..."
				}
				fmt.Fprintf(&sb, "  -> ToolCall: %s(%s)\n", tc.Name, input)
			}
		case "tool":
			content := msg.Content
			if len(content) > maxToolOutputChars {
				content = content[:maxToolOutputChars] + "...[truncated]"
			}
			fmt.Fprintf(&sb, "[Tool %d] %s\n", i, content)
		}
		sb.WriteByte('\n')
	}
	return sb.String()
}

// SplitMessages divides messages into a "head" (to summarize) and "recent"
// (to keep verbatim). The split point is determined by walking backwards
// from the end until keepTokens worth of content is accumulated.
// Tool-role messages are always preserved verbatim in the recent set,
// regardless of their position, to ensure actionable tool results survive compaction.
func SplitMessages(messages []types.Message, keepTokens int, estimateFn func(string) int) (head []types.Message, recent []types.Message) {
	if len(messages) == 0 {
		return nil, nil
	}

	// First pass: identify tool-role messages that must be preserved
	toolIndices := make(map[int]bool)
	for i, msg := range messages {
		if msg.Role == "tool" {
			toolIndices[i] = true
		}
	}

	// Second pass: determine split point, accounting for mandatory tool messages
	accumulated := 0
	splitIdx := len(messages)

	for i := len(messages) - 1; i >= 0; i-- {
		// Skip tool messages in the token count — they're always preserved
		if toolIndices[i] {
			continue
		}

		msg := messages[i]
		tokens := estimateFn(msg.Content)
		for _, tc := range msg.ToolCalls {
			tokens += estimateFn(string(tc.Input))
		}

		if accumulated+tokens > keepTokens && accumulated > 0 {
			break
		}
		accumulated += tokens
		splitIdx = i
	}

	if splitIdx <= 0 {
		return nil, messages
	}
	if splitIdx >= len(messages) {
		return messages, nil
	}

	// Ensure all tool messages in the "head" section are moved to "recent"
	head = messages[:splitIdx]
	recent = messages[splitIdx:]

	// Move any tool messages from head to recent (preserving order)
	var additionalRecent []types.Message
	var newHead []types.Message
	for _, msg := range head {
		if msg.Role == "tool" {
			additionalRecent = append(additionalRecent, msg)
		} else {
			newHead = append(newHead, msg)
		}
	}

	// Prepend additional tool messages to recent to maintain chronological order
	if len(additionalRecent) > 0 {
		combined := make([]types.Message, 0, len(additionalRecent)+len(recent))
		combined = append(combined, additionalRecent...)
		combined = append(combined, recent...)
		recent = combined
	}

	return newHead, recent
}

func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}
