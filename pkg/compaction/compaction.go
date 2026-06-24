package compaction

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/types"
)

// Config holds compaction settings.
type Config struct {
	Auto       bool
	Buffer     int // tokens reserved before compaction triggers
	KeepTokens int // tokens of recent history to preserve verbatim
}

// DefaultConfig returns compaction defaults.
func DefaultConfig() Config {
	return Config{
		Auto:       true,
		Buffer:     20000,
		KeepTokens: 8000,
	}
}

// Compactor manages automatic session compaction. It monitors token usage
// against model context limits and generates structured summaries when
// usage exceeds the threshold.
type Compactor struct {
	cfg        Config
	tokenEst   *tokens.Estimator
	mu         sync.RWMutex
	lastResult *Result
}

// Result holds the outcome of a compaction operation.
type Result struct {
	Compacted       bool
	TokensBefore    int
	TokensAfter     int
	Summary         string
	MessagesRemoved int
	DurationMs      int64
}

// New creates a Compactor with the given config and token estimator.
func New(cfg Config, tokenEst *tokens.Estimator) *Compactor {
	return &Compactor{
		cfg:      cfg,
		tokenEst: tokenEst,
	}
}

// ShouldCompact returns true if the messages exceed the compaction threshold.
// threshold = contextLength - outputReserve - buffer
func (c *Compactor) ShouldCompact(messages []types.Message, contextLength int64) bool {
	if !c.cfg.Auto || c.tokenEst == nil {
		return false
	}
	if contextLength <= 0 {
		contextLength = types.DefaultContextLength
	}

	estimated := c.tokenEst.EstimateMessages(messages)
	threshold := int(contextLength) - c.cfg.Buffer
	if threshold <= 0 {
		return false
	}

	return estimated > threshold
}

// Compact generates a structured summary of old messages and returns a new
// message list with the old messages replaced by the summary. Uses the LLM
// provider to generate the summary.
func (c *Compactor) Compact(ctx context.Context, messages []types.Message, p provider.LLMProvider, modelID string) (*Result, error) {
	start := time.Now()

	if c.tokenEst == nil {
		return &Result{Compacted: false}, fmt.Errorf("token estimator required")
	}

	tokensBefore := c.tokenEst.EstimateMessages(messages)

	head, recent := SplitMessages(messages, c.cfg.KeepTokens, c.tokenEst.Estimate)
	if len(head) == 0 {
		return &Result{Compacted: false}, nil
	}

	headText := SerializeMessages(head)

	summary, err := c.generateSummary(ctx, headText, p, modelID)
	if err != nil {
		return &Result{Compacted: false, DurationMs: time.Since(start).Milliseconds()},
			fmt.Errorf("generate compaction summary: %w", err)
	}

	summaryMsg := types.Message{
		Role:    "system",
		Content: summary,
		Segments: []types.MessageSegment{
			{
				Type:    types.MessageCompaction,
				Content: summary,
				Visible: false,
			},
		},
		CreatedAt: time.Now(),
	}

	newMessages := make([]types.Message, 0, len(recent)+1)
	newMessages = append(newMessages, summaryMsg)
	newMessages = append(newMessages, recent...)

	tokensAfter := c.tokenEst.EstimateMessages(newMessages)

	result := &Result{
		Compacted:       true,
		TokensBefore:    tokensBefore,
		TokensAfter:     tokensAfter,
		Summary:         summary,
		MessagesRemoved: len(head),
		DurationMs:      time.Since(start).Milliseconds(),
	}

	c.mu.Lock()
	c.lastResult = result
	c.mu.Unlock()
	slog.Info("session compacted",
		"tokens_before", tokensBefore,
		"tokens_after", tokensAfter,
		"messages_removed", len(head),
		"duration_ms", result.DurationMs,
	)

	return result, nil
}

// generateSummary sends the serialized head to the LLM for summarization.
func (c *Compactor) generateSummary(ctx context.Context, headText string, p provider.LLMProvider, modelID string) (string, error) {
	if p == nil {
		return "", fmt.Errorf("provider required for compaction")
	}

	messages := []types.Message{
		{
			Role:    "system",
			Content: Template(),
		},
		{
			Role:    "user",
			Content: "Summarize the following conversation history:\n\n" + headText,
		},
	}

	req := provider.ChatRequest{
		Model:            modelID,
		Messages:         messages,
		ReasoningEnabled: false,
	}

	iterator, err := p.ChatCompletionStream(ctx, req)
	if err != nil {
		return "", fmt.Errorf("compaction LLM call: %w", err)
	}
	defer iterator.Close() //nolint:errcheck

	var sb strings.Builder
	for {
		chunk, err := iterator.Next()
		if err != nil {
			if sb.Len() > 0 {
				// Return partial content with the error so callers can decide
				// whether to use the truncated summary.
				return sb.String(), fmt.Errorf("compaction stream (partial content returned): %w", err)
			}
			return "", fmt.Errorf("compaction stream: %w", err)
		}
		if chunk == nil {
			break
		}
		if chunk.Delta != "" {
			sb.WriteString(chunk.Delta)
		}
	}

	summary := sb.String()
	if summary == "" {
		return "", fmt.Errorf("compaction produced empty summary")
	}

	return summary, nil
}

// LastResult returns the most recent compaction result, or nil if no
// compaction has been performed.
func (c *Compactor) LastResult() *Result {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.lastResult
}
