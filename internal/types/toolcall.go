package types

import (
	pkgTypes "github.com/eshanized/M31A/pkg/types"
)

// ToolCallAcc accumulates streaming tool call deltas by index.
type ToolCallAcc = pkgTypes.ToolCallAcc

// BuildToolCallsFromAcc converts accumulated tool call deltas into
// []ToolCall sorted by index.
func BuildToolCallsFromAcc(accMap map[int]*ToolCallAcc) []ToolCall {
	return pkgTypes.BuildToolCallsFromAcc(accMap)
}

// BuildToolCallsFromAccFiltered is like BuildToolCallsFromAcc but skips
// entries with empty names.
func BuildToolCallsFromAccFiltered(accMap map[int]*ToolCallAcc) []ToolCall {
	return pkgTypes.BuildToolCallsFromAccFiltered(accMap)
}
