package tools

import (
	"context"
	"fmt"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/metrics"
)

// Compile-time interface check
var _ types.Tool = (*MetricsTool)(nil)

// MetricsTool provides query access to session observability metrics.
// It supports four modes: summary, tool-stats, phase-stats, cost-report.
type MetricsTool struct {
	collector *metrics.Collector
}

// NewMetricsTool creates a MetricsTool backed by the given collector.
func NewMetricsTool(collector *metrics.Collector) *MetricsTool {
	return &MetricsTool{collector: collector}
}

func (t *MetricsTool) Name() string {
	return "Metrics"
}

func (t *MetricsTool) Description() string {
	return "Query session observability metrics: tool execution stats, phase durations, token usage, and cost breakdown."
}

func (t *MetricsTool) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

// ParameterSchema returns the JSON Schema for Metrics tool parameters.
func (t *MetricsTool) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"mode": {
				"type": "string",
				"enum": ["summary", "tool-stats", "phase-stats", "cost-report"],
				"description": "Query mode: summary (overview), tool-stats (per-tool), phase-stats (per-phase), cost-report (token/cost by phase)"
			}
		},
		"required": ["mode"]
	}`
}

func (t *MetricsTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	modeRaw, ok := input.Params["mode"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: mode", m31errors.ErrToolExecution)
	}
	mode, ok := modeRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter mode must be a string", m31errors.ErrToolExecution)
	}

	if t.collector == nil || !t.collector.Enabled() {
		return types.ToolResult{
			Output: "Metrics collection is not enabled for this session.",
		}, nil
	}

	snap := t.collector.Snapshot()

	var output string
	switch mode {
	case "summary":
		output = formatSummary(snap)
	case "tool-stats":
		output = formatToolStats(snap)
	case "phase-stats":
		output = formatPhaseStats(snap)
	case "cost-report":
		output = formatCostReport(snap)
	default:
		return types.ToolResult{}, fmt.Errorf("%w: unknown mode %q (valid: summary, tool-stats, phase-stats, cost-report)", m31errors.ErrToolExecution, mode)
	}

	_ = time.Since(start).Milliseconds() // elapsed for future use

	return types.ToolResult{
		Output: output,
	}, nil
}

// formatSummary produces an overall session metrics overview.
func formatSummary(snap *metrics.SessionMetrics) string {
	var sb strings.Builder
	sb.WriteString("# Session Metrics Summary\n\n")
	sb.WriteString(fmt.Sprintf("Session: %s\n", snap.SessionID))
	sb.WriteString(fmt.Sprintf("Started: %s\n", snap.StartedAt.Format(time.RFC3339)))
	sb.WriteString(fmt.Sprintf("Last updated: %s\n\n", snap.UpdatedAt.Format(time.RFC3339)))

	// Tool totals
	var totalCalls, totalSuccess, totalFail int64
	for _, t := range snap.Tools {
		totalCalls += t.CallCount
		totalSuccess += t.SuccessCount
		totalFail += t.FailCount
	}
	sb.WriteString(fmt.Sprintf("## Tools\n"))
	sb.WriteString(fmt.Sprintf("  Tool types used: %d\n", len(snap.Tools)))
	sb.WriteString(fmt.Sprintf("  Total calls: %d\n", totalCalls))
	sb.WriteString(fmt.Sprintf("  Successes: %d\n", totalSuccess))
	sb.WriteString(fmt.Sprintf("  Failures: %d\n", totalFail))
	if totalCalls > 0 {
		successRate := float64(totalSuccess) / float64(totalCalls) * 100
		sb.WriteString(fmt.Sprintf("  Success rate: %.1f%%\n", successRate))
	}

	// LLM totals
	var totalTokens int64
	var totalCost float64
	for _, l := range snap.LLMs {
		totalTokens += l.TotalTokens
		totalCost += l.Cost
	}
	sb.WriteString(fmt.Sprintf("\n## LLM\n"))
	sb.WriteString(fmt.Sprintf("  Interactions: %d\n", totalInteractions(snap)))
	sb.WriteString(fmt.Sprintf("  Total tokens: %d\n", totalTokens))
	sb.WriteString(fmt.Sprintf("  Total cost: $%.4f\n", totalCost))

	// Phase totals
	sb.WriteString(fmt.Sprintf("\n## Phases\n"))
	sb.WriteString(fmt.Sprintf("  Phases tracked: %d\n", len(snap.Phases)))
	for _, p := range snap.Phases {
		sb.WriteString(fmt.Sprintf("  - %s: %dms", p.Phase, p.DurationMs))
		if !p.Success {
			sb.WriteString(" (failed)")
		}
		sb.WriteString("\n")
	}

	return sb.String()
}

// formatToolStats produces per-tool call counts, success rates, and avg durations.
func formatToolStats(snap *metrics.SessionMetrics) string {
	var sb strings.Builder
	sb.WriteString("# Tool Statistics\n\n")

	if len(snap.Tools) == 0 {
		sb.WriteString("No tool calls recorded.\n")
		return sb.String()
	}

	for _, t := range snap.Tools {
		rate := float64(0)
		if t.CallCount > 0 {
			rate = float64(t.SuccessCount) / float64(t.CallCount) * 100
		}
		sb.WriteString(fmt.Sprintf("## %s\n", t.Name))
		sb.WriteString(fmt.Sprintf("  Calls: %d (success: %d, fail: %d)\n", t.CallCount, t.SuccessCount, t.FailCount))
		sb.WriteString(fmt.Sprintf("  Success rate: %.1f%%\n", rate))
		sb.WriteString(fmt.Sprintf("  Avg duration: %.1fms\n", t.AvgDurMs))
		sb.WriteString("\n")
	}

	return sb.String()
}

// formatPhaseStats produces per-phase durations and transition counts.
func formatPhaseStats(snap *metrics.SessionMetrics) string {
	var sb strings.Builder
	sb.WriteString("# Phase Statistics\n\n")

	if len(snap.Phases) == 0 {
		sb.WriteString("No phase data recorded.\n")
		return sb.String()
	}

	for _, p := range snap.Phases {
		sb.WriteString(fmt.Sprintf("## %s\n", p.Phase))
		sb.WriteString(fmt.Sprintf("  Duration: %dms\n", p.DurationMs))
		sb.WriteString(fmt.Sprintf("  Transitions: %d\n", p.TransitionCount))
		sb.WriteString(fmt.Sprintf("  Heal triggers: %d\n", p.HealTriggerCount))
		sb.WriteString(fmt.Sprintf("  Bisect triggers: %d\n", p.BisectTriggerCount))
		if !p.Success {
			sb.WriteString("  Status: FAILED\n")
		} else {
			sb.WriteString("  Status: OK\n")
		}
		sb.WriteString("\n")
	}

	return sb.String()
}

// formatCostReport produces token usage and cost breakdown by phase.
func formatCostReport(snap *metrics.SessionMetrics) string {
	var sb strings.Builder
	sb.WriteString("# Cost Report\n\n")

	if len(snap.LLMs) == 0 {
		sb.WriteString("No LLM interactions recorded.\n")
		return sb.String()
	}

	var grandTokens int64
	var grandCost float64

	for _, l := range snap.LLMs {
		sb.WriteString(fmt.Sprintf("## %s\n", l.Phase))
		sb.WriteString(fmt.Sprintf("  Interactions: %d\n", l.InteractionCount))
		sb.WriteString(fmt.Sprintf("  Prompt tokens: %d\n", l.PromptTokens))
		sb.WriteString(fmt.Sprintf("  Completion tokens: %d\n", l.CompletionTokens))
		sb.WriteString(fmt.Sprintf("  Total tokens: %d\n", l.TotalTokens))
		sb.WriteString(fmt.Sprintf("  Cost: $%.4f\n", l.Cost))
		sb.WriteString("\n")
		grandTokens += l.TotalTokens
		grandCost += l.Cost
	}

	sb.WriteString("## Totals\n")
	sb.WriteString(fmt.Sprintf("  Total tokens: %d\n", grandTokens))
	sb.WriteString(fmt.Sprintf("  Total cost: $%.4f\n", grandCost))

	return sb.String()
}

// totalInteractions sums interaction counts across all LLM metrics.
func totalInteractions(snap *metrics.SessionMetrics) int64 {
	var total int64
	for _, l := range snap.LLMs {
		total += l.InteractionCount
	}
	return total
}
