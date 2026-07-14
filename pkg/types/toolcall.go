package types

import (
	"encoding/json"
	"sort"
	"strings"
)

// ToolCallAcc accumulates streaming tool call deltas by index.
// Used by both the TUI streaming layer and the subagent loop to
// reconstruct complete tool calls from provider stream chunks.
type ToolCallAcc struct {
	ID    string
	Name  string
	Args  strings.Builder
	Index int
}

// BuildToolCallsFromAcc converts accumulated tool call deltas into
// []ToolCall sorted by index. This is the shared implementation
// used by streaming.go, agent_loop.go, and subagent/loop.go.
func BuildToolCallsFromAcc(accMap map[int]*ToolCallAcc) []ToolCall {
	if len(accMap) == 0 {
		return nil
	}
	indices := make([]int, 0, len(accMap))
	for idx := range accMap {
		indices = append(indices, idx)
	}
	sort.Ints(indices)
	var result []ToolCall
	for _, idx := range indices {
		acc := accMap[idx]
		id := acc.ID
		if id == "" {
			id = acc.Name
		}
		argsStr := acc.Args.String()
		var input json.RawMessage
		if argsStr != "" {
			input = json.RawMessage(argsStr)
		} else {
			input = json.RawMessage("{}")
		}
		result = append(result, ToolCall{
			ID:    id,
			Name:  acc.Name,
			Input: input,
		})
	}
	return result
}

// BuildToolCallsFromAccFiltered is like BuildToolCallsFromAcc but skips
// entries with empty names. Used by the subagent loop which discards
// unnamed tool call fragments.
func BuildToolCallsFromAccFiltered(accMap map[int]*ToolCallAcc) []ToolCall {
	if len(accMap) == 0 {
		return nil
	}
	indices := make([]int, 0, len(accMap))
	for idx := range accMap {
		indices = append(indices, idx)
	}
	sort.Ints(indices)
	var result []ToolCall
	for _, idx := range indices {
		acc := accMap[idx]
		if acc.Name == "" {
			continue
		}
		id := acc.ID
		if id == "" {
			id = acc.Name
		}
		result = append(result, ToolCall{
			ID:    id,
			Name:  acc.Name,
			Input: json.RawMessage(acc.Args.String()),
		})
	}
	return result
}
