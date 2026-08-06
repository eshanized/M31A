package workflow

// LLM streaming support. Handles chat completion streaming, tool call accumulation, retry logic, and token calibration.

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/infrastructure/retry"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

// consumeStream reads all chunks from the iterator and returns the concatenated content.
// Enforces MaxLLMResponseBytes limit to prevent OOM from pathological responses.
// Returns partial content before non-EOF errors so callers can inspect what was received.
// Also captures usage data from the final chunk for token calibration.
func (e *Engine) consumeStream(iterator *m31types.StreamIterator) (string, *m31types.Usage, error) {
	var sb strings.Builder
	defer func() {
		if err := iterator.Close(); err != nil {
			e.logger.Debug("close stream iterator", "error", err, "resource", "consumeStream")
		}
	}()

	var lastUsage *m31types.Usage
	for {
		chunk, err := iterator.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			// Preserve partial content before non-EOF errors for caller inspection
			if chunk != nil && chunk.Delta != "" {
				sb.WriteString(chunk.Delta)
			}
			return sb.String(), lastUsage, err
		}
		if chunk != nil {
			if chunk.Delta != "" {
				sb.WriteString(chunk.Delta)
				// Enforce max response size incrementally as chunks arrive
				if sb.Len() > m31types.MaxLLMResponseBytes {
					return sb.String(), lastUsage, fmt.Errorf("LLM response exceeds maximum size of %d bytes: %w",
						m31types.MaxLLMResponseBytes, m31errors.ErrContextExceeded)
				}
			}
			if chunk.Usage != nil {
				lastUsage = chunk.Usage
			}
		}
	}
	return sb.String(), lastUsage, nil
}

// toolCallBuilder accumulates streamed tool_call chunks for a single tool invocation.
type toolCallBuilder struct {
	id        string
	name      string
	arguments strings.Builder
}

// consumeStreamWithTools reads all chunks from the iterator, collecting both
// text content and native tool_call chunks. Returns the concatenated content,
// any structured tool calls, captured usage data, and an error.
//
// Native tool_call chunks arrive with Type="tool_call" and incremental argument
// deltas in ToolInput. They are accumulated by Index and finalized into ToolCall
// structs with parsed JSON arguments.
func (e *Engine) consumeStreamWithTools(iterator *m31types.StreamIterator) (string, []m31types.ToolCall, *m31types.Usage, error) {
	var content strings.Builder
	builders := map[int]*toolCallBuilder{}
	defer func() {
		if err := iterator.Close(); err != nil {
			e.logger.Debug("close stream iterator", "error", err, "resource", "consumeStreamWithTools")
		}
	}()

	var lastUsage *m31types.Usage
	for {
		chunk, err := iterator.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			if chunk != nil && chunk.Delta != "" {
				content.WriteString(chunk.Delta)
			}
			// Discard partial tool calls from truncated streams to prevent
			// dispatching incomplete/malformed tool calls.
			return content.String(), nil, lastUsage, err
		}
		if chunk == nil {
			continue
		}

		if chunk.Usage != nil {
			lastUsage = chunk.Usage
		}

		switch chunk.Type {
		case "tool_call":
			b, ok := builders[chunk.Index]
			if !ok {
				b = &toolCallBuilder{
					id:   chunk.ToolCallID,
					name: chunk.ToolName,
				}
				builders[chunk.Index] = b
			}
			if chunk.ToolInput != "" {
				b.arguments.WriteString(chunk.ToolInput)
			}
			if chunk.ToolCallID != "" && b.id == "" {
				b.id = chunk.ToolCallID
			}
			if chunk.ToolName != "" && b.name == "" {
				b.name = chunk.ToolName
			}

		default:
			if chunk.Delta != "" {
				content.WriteString(chunk.Delta)
				if content.Len() > m31types.MaxLLMResponseBytes {
					return content.String(), nil, lastUsage, fmt.Errorf("LLM response exceeds maximum size of %d bytes: %w",
						m31types.MaxLLMResponseBytes, m31errors.ErrContextExceeded)
				}
			}
		}
	}

	toolCalls := finalizeToolCalls(builders, e)
	return content.String(), toolCalls, lastUsage, nil
}

// finalizeToolCalls converts accumulated toolCallBuilders into ToolCall structs.
// Sorts by index for deterministic ordering. Normalizes tool names and parses
// arguments as JSON.
func finalizeToolCalls(builders map[int]*toolCallBuilder, e *Engine) []m31types.ToolCall {
	if len(builders) == 0 {
		return nil
	}

	indices := make([]int, 0, len(builders))
	for idx := range builders {
		indices = append(indices, idx)
	}
	sort.Ints(indices)

	if len(indices) > m31types.MaxToolsPerCall {
		e.logger.Warn("finalizeToolCalls: tool count exceeded cap, truncating",
			"count", len(indices), "cap", m31types.MaxToolsPerCall)
		indices = indices[:m31types.MaxToolsPerCall]
	}

	calls := make([]m31types.ToolCall, 0, len(indices))
	for _, idx := range indices {
		b := builders[idx]
		name := normalizeToolName(b.name)

		args := b.arguments.String()
		var input json.RawMessage
		if args != "" {
			input = json.RawMessage(args)
		} else {
			input = json.RawMessage("{}")
		}

		id := b.id
		if id == "" {
			id = fmt.Sprintf("call_%s_%d", name, e.nextCallID())
		}

		calls = append(calls, m31types.ToolCall{
			ID:    id,
			Name:  name,
			Input: input,
		})
	}
	return calls
}

// prepareStreamRequest handles the shared preamble for all streamLLM variants:
// preflight context check, build ChatRequest, emit thinking start, and open
// the stream with retry. Returns the iterator on success.
func (e *Engine) prepareStreamRequest(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (*m31types.StreamIterator, error) {
	msgs, err := e.preflightContextCheck(messages)
	if err != nil {
		return nil, err
	}

	e.emit(ThinkingStartMsg{
		Context: "LLM processing...",
	})

	req := provider.ChatRequest{
		Model:            e.modelForPhase(e.stateMachine.CurrentPhase()),
		Messages:         msgs,
		ReasoningEnabled: true,
	}
	if toolsEnabled {
		req.Tools = e.buildToolDefinitions()
	}

	rp, _ := e.providerAndModel()
	iterator, err := rp.ChatCompletionStream(ctx, req)
	if err != nil {
		iterator, err = e.retryChatStream(ctx, req, err)
		if err != nil {
			e.emit(ThinkingCompleteMsg{
				Context: "LLM processing failed",
			})
			return nil, err
		}
	}
	return iterator, nil
}

// emitThinkingDone emits the thinking complete message.
func (e *Engine) emitThinkingDone() {
	e.emit(ThinkingCompleteMsg{
		Context: "LLM processing complete",
	})
}

// calibrateFromUsage feeds actual API token counts back into the estimator
// so provider-specific heuristics self-correct over the lifetime of a workflow.
// Only PromptTokens is used because it represents the model's actual view of
// the input context, which is what EstimateMessages tries to approximate.
func (e *Engine) calibrateFromUsage(messages []m31types.Message, usage *m31types.Usage) {
	if e.tokens == nil || usage == nil || usage.PromptTokens <= 0 {
		return
	}
	estimated := e.tokens.EstimateMessages(messages)
	e.tokens.Calibrate(estimated, usage.PromptTokens)
}

// streamLLMWithTools sends a chat request with tool definitions and returns
// both the text content and any native tool calls from the response.
// Used by execute and heal phases for structured tool dispatch.
func (e *Engine) streamLLMWithTools(ctx context.Context, messages []m31types.Message) (string, []m31types.ToolCall, error) {
	iterator, err := e.prepareStreamRequest(ctx, messages, true)
	if err != nil {
		return "", nil, err
	}

	content, toolCalls, usage, err := e.consumeStreamWithTools(iterator)
	e.calibrateFromUsage(messages, usage)
	e.recordLLMInteractionWithPrompt(messages, usage, 0)
	e.emitThinkingDone()
	return content, toolCalls, err
}

// streamLLM sends a chat request and returns the full response content.
func (e *Engine) streamLLM(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (string, error) {
	iterator, err := e.prepareStreamRequest(ctx, messages, toolsEnabled)
	if err != nil {
		return "", err
	}

	result, usage, err := e.consumeStream(iterator)
	e.calibrateFromUsage(messages, usage)
	e.recordLLMInteractionWithPrompt(messages, usage, 0)
	e.emitThinkingDone()
	return result, err
}

// streamLLMStreaming sends a chat request and returns the underlying
// StreamIterator. The caller is responsible for iterating via Next()
// and emitting each chunk to the TUI (typically via MsgEmitter).
func (e *Engine) streamLLMStreaming(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (*m31types.StreamIterator, error) {
	return e.prepareStreamRequest(ctx, messages, toolsEnabled)
}

// retryChatStream retries a failed ChatCompletionStream call using exponential
// backoff. The firstErr is the error from the initial attempt. Returns the
// iterator from a successful retry or the last error if all retries fail.
func (e *Engine) retryChatStream(ctx context.Context, req provider.ChatRequest, firstErr error) (*m31types.StreamIterator, error) {
	class, reason := retry.ClassifyError(firstErr)
	if !retry.IsRetryable(class) {
		return nil, firstErr
	}

	policy := retry.DefaultPolicy()
	if e.cfg != nil {
		policy = retry.ConfiguredPolicy(
			e.cfg.Features.RetryMaxAttempts,
			e.cfg.Features.RetryBaseDelayMs,
			e.cfg.Features.RetryMaxDelayMs,
			e.cfg.Features.RetryBackoffMultiplier,
		)
	}
	var lastErr = firstErr

	for attempt := 1; attempt <= policy.MaxAttempts; attempt++ {
		delay := policy.Delay(attempt, nil)
		e.logger.Debug("retrying stream", "attempt", attempt, "max", policy.MaxAttempts, "delay", delay, "reason", reason)

		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("%w: %v", ctx.Err(), lastErr)
		case <-time.After(delay):
		}

		sp, _ := e.providerAndModel()
		iterator, err := sp.ChatCompletionStream(ctx, req)
		if err == nil {
			return iterator, nil
		}
		lastErr = err

		class, reason = retry.ClassifyError(err)
		if !retry.IsRetryable(class) {
			return nil, err
		}
	}

	return nil, lastErr
}

// computePromptHash returns a short SHA-256 hash of the message content for metrics tracking.
func computePromptHash(messages []m31types.Message) string {
	h := sha256.New()
	for _, m := range messages {
		h.Write([]byte(m.Role))
		h.Write([]byte{0})
		h.Write([]byte(m.Content))
		h.Write([]byte{0})
	}
	return hex.EncodeToString(h.Sum(nil))[:16]
}

// recordLLMInteractionWithPrompt records detailed LLM metrics including prompt hash.
func (e *Engine) recordLLMInteractionWithPrompt(messages []m31types.Message, usage *m31types.Usage, cost float64) {
	if e.collector == nil || usage == nil {
		return
	}
	phase := e.stateMachine.CurrentPhase()
	promptHash := computePromptHash(messages)
	e.collector.RecordLLMInteractionWithPrompt(phase, usage, cost, promptHash, false)
}
