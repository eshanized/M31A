package tools

import (
	"context"
	"fmt"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/integrations/metrics"
	"github.com/eshanized/M31A/internal/core/types"
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
	fmt.Fprintf(&sb, "Session: %s\n", snap.SessionID)
	fmt.Fprintf(&sb, "Started: %s\n", snap.StartedAt.Format(time.RFC3339))
	fmt.Fprintf(&sb, "Last updated: %s\n\n", snap.UpdatedAt.Format(time.RFC3339))

	// Tool totals
	var totalCalls, totalSuccess, totalFail int64
	for _, t := range snap.Tools {
		totalCalls += t.CallCount
		totalSuccess += t.SuccessCount
		totalFail += t.FailCount
	}
	sb.WriteString("## Tools\n")
	fmt.Fprintf(&sb, "  Tool types used: %d\n", len(snap.Tools))
	fmt.Fprintf(&sb, "  Total calls: %d\n", totalCalls)
	fmt.Fprintf(&sb, "  Successes: %d\n", totalSuccess)
	fmt.Fprintf(&sb, "  Failures: %d\n", totalFail)
	if totalCalls > 0 {
		successRate := float64(totalSuccess) / float64(totalCalls) * 100
		fmt.Fprintf(&sb, "  Success rate: %.1f%%\n", successRate)
	}

	// LLM totals
	var totalTokens int64
	var totalCost float64
	for _, l := range snap.LLMs {
		totalTokens += l.TotalTokens
		totalCost += l.Cost
	}
	sb.WriteString("\n## LLM\n")
	fmt.Fprintf(&sb, "  Interactions: %d\n", totalInteractions(snap))
	fmt.Fprintf(&sb, "  Total tokens: %d\n", totalTokens)
	fmt.Fprintf(&sb, "  Total cost: $%.4f\n", totalCost)

	// Phase totals
	sb.WriteString("\n## Phases\n")
	fmt.Fprintf(&sb, "  Phases tracked: %d\n", len(snap.Phases))
	for _, p := range snap.Phases {
		fmt.Fprintf(&sb, "  - %s: %dms", p.Phase, p.DurationMs)
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
		fmt.Fprintf(&sb, "## %s\n", t.Name)
		fmt.Fprintf(&sb, "  Calls: %d (success: %d, fail: %d)\n", t.CallCount, t.SuccessCount, t.FailCount)
		fmt.Fprintf(&sb, "  Success rate: %.1f%%\n", rate)
		fmt.Fprintf(&sb, "  Avg duration: %.1fms\n", t.AvgDurMs)
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
		fmt.Fprintf(&sb, "## %s\n", p.Phase)
		fmt.Fprintf(&sb, "  Duration: %dms\n", p.DurationMs)
		fmt.Fprintf(&sb, "  Transitions: %d\n", p.TransitionCount)
		fmt.Fprintf(&sb, "  Heal triggers: %d\n", p.HealTriggerCount)
		fmt.Fprintf(&sb, "  Bisect triggers: %d\n", p.BisectTriggerCount)
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
		fmt.Fprintf(&sb, "## %s\n", l.Phase)
		fmt.Fprintf(&sb, "  Interactions: %d\n", l.InteractionCount)
		fmt.Fprintf(&sb, "  Prompt tokens: %d\n", l.PromptTokens)
		fmt.Fprintf(&sb, "  Completion tokens: %d\n", l.CompletionTokens)
		fmt.Fprintf(&sb, "  Total tokens: %d\n", l.TotalTokens)
		fmt.Fprintf(&sb, "  Cost: $%.4f\n", l.Cost)
		sb.WriteString("\n")
		grandTokens += l.TotalTokens
		grandCost += l.Cost
	}

	sb.WriteString("## Totals\n")
	fmt.Fprintf(&sb, "  Total tokens: %d\n", grandTokens)
	fmt.Fprintf(&sb, "  Total cost: $%.4f\n", grandCost)

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
