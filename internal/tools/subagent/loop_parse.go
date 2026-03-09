package subagent

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"regexp"
	"strings"
)

// toolCallJSON is the on-the-wire shape produced by the parent LLM for a
// single tool invocation. Both "input" and "params" are tolerated.
type toolCallJSON struct {
	Name   string          `json:"name"`
	Tool   string          `json:"tool"`
	Input  json.RawMessage `json:"input"`
	Params json.RawMessage `json:"params"`
	ID     string          `json:"id,omitempty"`
}

var fencedBlockRe = regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")

// parseToolCalls extracts tool calls from the assistant's content.
func parseToolCalls(content string, logger *slog.Logger) ([]ToolCallInput, error) {
	if strings.TrimSpace(content) == "" {
		return nil, nil
	}
	if logger == nil {
		logger = slog.Default()
	}

	var calls []ToolCallInput
	var firstErr error
	idCounter := 0
	nextID := func() string {
		idCounter++
		return fmt.Sprintf("subagent_call_%d", idCounter)
	}

	for _, match := range fencedBlockRe.FindAllStringSubmatch(content, -1) {
		if len(match) < 2 {
			continue
		}
		c, err := parseToolCallBlob([]byte(match[1]), nextID)
		if err != nil {
			if firstErr == nil {
				firstErr = err
			}
			logger.Debug("subagent: parse tool call in fenced block failed", "err", err)
			continue
		}
		calls = append(calls, c...)
	}

	if len(calls) == 0 {
		for _, obj := range extractJSONObjects(content) {
			if !looksLikeToolCall(obj) {
				continue
			}
			c, err := parseToolCallBlob([]byte(obj), nextID)
			if err != nil {
				if firstErr == nil {
					firstErr = err
				}
				continue
			}
			calls = append(calls, c...)
		}
	}

	return calls, firstErr
}

func parseToolCallBlob(data []byte, nextID func() string) ([]ToolCallInput, error) {
	trimmed := strings.TrimSpace(string(data))
	if trimmed == "" {
		return nil, nil
	}
	if trimmed[0] == '[' {
		var arr []toolCallJSON
		if err := json.Unmarshal([]byte(trimmed), &arr); err == nil {
			return convertToolCalls(arr, nextID), nil
		}
	}
	var one toolCallJSON
	if err := json.Unmarshal([]byte(trimmed), &one); err != nil {
		return nil, fmt.Errorf("tool call JSON: %w", err)
	}
	if one.Name == "" && one.Tool == "" {
		return nil, nil
	}
	return convertToolCalls([]toolCallJSON{one}, nextID), nil
}

func convertToolCalls(in []toolCallJSON, nextID func() string) []ToolCallInput {
	out := make([]ToolCallInput, 0, len(in))
	for _, raw := range in {
		name := raw.Name
		if name == "" {
			name = raw.Tool
		}
		if name == "" {
			continue
		}
		input := raw.Input
		if len(input) == 0 {
			input = raw.Params
		}
		if len(input) == 0 {
			input = []byte("{}")
		}
		id := raw.ID
		if id == "" {
			id = nextID()
		}
		out = append(out, ToolCallInput{
			ID:    id,
			Name:  name,
			Input: input,
		})
	}
	return out
}

func looksLikeToolCall(obj string) bool {
	return strings.Contains(obj, `"name"`) || strings.Contains(obj, `"tool"`)
}

func extractJSONObjects(s string) []string {
	var out []string
	n := len(s)
	i := 0
	for i < n {
		if s[i] != '{' {
			i++
			continue
		}
		depth := 0
		inString := false
		escaped := false
		start := i
		for ; i < n; i++ {
			c := s[i]
			if escaped {
				escaped = false
				continue
			}
			if c == '\\' && inString {
				escaped = true
				continue
			}
			if c == '"' {
				inString = !inString
				continue
			}
			if inString {
				continue
			}
			if c == '{' {
				depth++
			} else if c == '}' {
				depth--
				if depth == 0 {
					out = append(out, s[start:i+1])
					i++
					break
				}
			}
		}
	}
	return out
}
