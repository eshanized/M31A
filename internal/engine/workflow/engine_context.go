package workflow

// Context management, proactive compaction, and preflight token estimation.

import (
	"context"
	"fmt"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
)

// preflightContextCheck estimates token usage before each LLM request
// and attempts automatic truncation if the estimate exceeds 80% of the
// model's context window. Returns ErrContextExceeded only if truncation
// cannot bring usage below 95%. Returns a (possibly truncated) copy of the
// messages so the caller's original slice is never mutated.
func (e *Engine) preflightContextCheck(messages []m31types.Message) ([]m31types.Message, error) {
	if e.tokens == nil {
		return messages, nil
	}
	p, _ := e.providerAndModel()
	if p == nil {
		return messages, nil
	}
	modelInfo, err := p.GetModel(e.modelForPhase(e.stateMachine.CurrentPhase()))
	if err != nil || modelInfo == nil {
		return messages, nil
	}
	estimated := e.tokens.EstimateMessages(messages)
	contextLength := modelInfo.ContextLength
	if contextLength <= 0 {
		contextLength = m31types.DefaultContextLength
	}

	threshold80 := int(float64(contextLength) * 0.80)
	if e.cfg != nil && e.cfg.Features.ContextTruncationThreshold > 0 {
		threshold80 = int(float64(contextLength) * e.cfg.Features.ContextTruncationThreshold)
	}
	threshold95 := int(float64(contextLength) * 0.95)

	if estimated <= threshold80 {
		return messages, nil
	}

	var msgs []m31types.Message

	// Try auto-compaction before falling back to crude truncation
	if e.compactor != nil && e.compactor.ShouldCompact(messages, contextLength) {
		e.logger.Debug("auto-compaction triggered", "estimated_tokens", estimated, "context_length", contextLength)
		compactCtx, compactCancel := context.WithTimeout(context.Background(), 60*time.Second)
		cp, _ := e.providerAndModel()
		result, compactErr := e.compactor.Compact(compactCtx, messages, cp, e.modelForPhase(e.stateMachine.CurrentPhase()))
		compactCancel()
		if compactErr == nil && result.Compacted {
			e.emit(CompactionCompleteMsg{
				TokensBefore:    result.TokensBefore,
				TokensAfter:     result.TokensAfter,
				MessagesRemoved: result.MessagesRemoved,
			})
			// Re-estimate on the compacted messages
			compactedMsgs := e.compactedMessages(messages, result.Summary)
			compactedEstimate := e.tokens.EstimateMessages(compactedMsgs)
			if compactedEstimate <= threshold95 {
				return compactedMsgs, nil
			}
			// Compaction wasn't sufficient, fall through to truncation with compacted messages
			msgs = compactedMsgs
		} else if compactErr != nil {
			e.logger.Warn("auto-compaction failed, falling back to truncation", "error", compactErr)
		}
	}

	// Work on a shallow copy to avoid mutating the caller's slice.
	if msgs == nil {
		msgs = make([]m31types.Message, len(messages))
		copy(msgs, messages)
	}

	// Cache per-message token counts to avoid O(N*K) recomputation in truncation loops.
	// Each message's token count is estimated once, then updated incrementally after truncation.
	// B17: Include per-message overhead (4 tokens) to match EstimateMessages used in preflight.
	const perMessageOverhead = 4
	msgTokens := make([]int, len(msgs))
	for i, msg := range msgs {
		msgTokens[i] = e.tokens.Estimate(msg.Content) + perMessageOverhead
		for _, tc := range msg.ToolCalls {
			if len(tc.Input) > 0 {
				msgTokens[i] += e.tokens.Estimate(string(tc.Input))
			}
			msgTokens[i] += e.tokens.Estimate(tc.Name)
		}
	}
	estimateTotal := func() int {
		total := 0
		for _, t := range msgTokens {
			total += t
		}
		return total
	}
	estimated = estimateTotal()

	// Try progressive truncation before giving up
	// Pass 1: truncate old tool results
	if estimated > threshold80 {
		for i := 0; i < len(msgs) && estimated > threshold80; i++ {
			if msgs[i].Role == "tool" && len(msgs[i].Content) > 500 {
				msgs[i].Content = msgs[i].Content[:500] + "\n...[truncated for context]"
				msgTokens[i] = e.tokens.Estimate(msgs[i].Content)
				estimated = estimateTotal()
			}
		}
	}

	// Pass 2: truncate old assistant messages
	if estimated > threshold80 {
		for i := 0; i < len(msgs) && estimated > threshold80; i++ {
			if msgs[i].Role == "assistant" && len(msgs[i].ToolCalls) == 0 && len(msgs[i].Content) > 1000 {
				msgs[i].Content = msgs[i].Content[:1000] + "\n...[truncated for context]"
				msgTokens[i] = e.tokens.Estimate(msgs[i].Content)
				estimated = estimateTotal()
			}
		}
	}

	// Pass 3: remove oldest non-system, non-recent messages
	if estimated > threshold80 {
		keepRecent := 6
		if len(msgs) > keepRecent+1 {
			idx := 1 // start after the first (system) message
			for idx < len(msgs)-keepRecent && estimated > threshold80 {
				if msgs[idx].Role == "system" {
					idx++
					continue
				}
				msgs = append(msgs[:idx], msgs[idx+1:]...)
				msgTokens = append(msgTokens[:idx], msgTokens[idx:]...)
				estimated = estimateTotal()
				// Don't increment idx — next message slides into same position
			}
		}
	}

	if estimated > threshold95 {
		return msgs, fmt.Errorf("%w: estimated %d tokens exceeds 95%% of %d context (auto-truncation insufficient)", m31errors.ErrContextExceeded, estimated, contextLength)
	}

	if estimated > threshold80 {
		e.logger.Warn("context usage approaching limit after truncation", "estimated", estimated, "limit", contextLength)
	}
	return msgs, nil
}

// proactiveCompactCheck performs compaction if context usage exceeds the
// configured threshold. Called before phase transitions and periodically
// during Execute phase. This is a no-op if compactor is nil, proactive
// compaction is disabled, or no messages are available.
func (e *Engine) proactiveCompactCheck(messages []m31types.Message) []m31types.Message {
	if e.compactor == nil || e.tokens == nil {
		return messages
	}
	p, _ := e.providerAndModel()
	if p == nil {
		return messages
	}
	if e.cfg == nil || !e.cfg.Compaction.Proactive {
		return messages
	}

	modelInfo, err := p.GetModel(e.modelForPhase(e.stateMachine.CurrentPhase()))
	if err != nil || modelInfo == nil {
		return messages
	}
	contextLength := modelInfo.ContextLength
	if contextLength <= 0 {
		contextLength = m31types.DefaultContextLength
	}

	estimated := e.tokens.EstimateMessages(messages)
	threshold := int(float64(contextLength) * float64(e.cfg.Compaction.PhaseTransitionPct) / 100.0)
	if threshold <= 0 {
		threshold = int(float64(contextLength) * 0.60)
	}

	if estimated <= threshold {
		return messages
	}

	e.logger.Debug("proactive compaction triggered",
		"phase", e.stateMachine.CurrentPhase(),
		"estimated_tokens", estimated,
		"context_length", contextLength,
		"threshold_pct", e.cfg.Compaction.PhaseTransitionPct)

	compactCtx, compactCancel := context.WithTimeout(context.Background(), 60*time.Second)
	cp2, _ := e.providerAndModel()
	result, compactErr := e.compactor.Compact(compactCtx, messages, cp2, e.modelForPhase(e.stateMachine.CurrentPhase()))
	compactCancel()

	if compactErr != nil {
		e.logger.Warn("proactive compaction failed", "error", compactErr)
		return messages
	}
	if !result.Compacted {
		return messages
	}

	e.emit(CompactionCompleteMsg{
		TokensBefore:    result.TokensBefore,
		TokensAfter:     result.TokensAfter,
		MessagesRemoved: result.MessagesRemoved,
	})

	compacted := e.compactedMessages(messages, result.Summary)
	e.logger.Debug("proactive compaction complete",
		"tokens_before", result.TokensBefore,
		"tokens_after", result.TokensAfter,
		"messages_removed", result.MessagesRemoved)
	return compacted
}
