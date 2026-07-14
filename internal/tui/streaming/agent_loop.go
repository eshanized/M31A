package streaming

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
)

const agentMaxIterations = 50

// AgentStreamMsg carries a streaming content chunk from the agent loop.
type AgentStreamMsg struct {
	Chunk *types.StreamChunk
}

// AgentToolStartMsg signals that the agent loop is executing a tool.
type AgentToolStartMsg struct {
	ToolCall types.ToolCall
}

// AgentToolDoneMsg signals that a tool execution completed.
type AgentToolDoneMsg struct {
	ToolCall   types.ToolCall
	Result     types.ToolResult
	Err        error
	DurationMs int64
}

// AgentDoneMsg signals the agent loop completed with a final text response.
type AgentDoneMsg struct {
	Message types.Message
	Usage   *types.Usage
}

// AgentErrorMsg signals an error in the agent loop.
type AgentErrorMsg struct {
	Err error
}

// AgentCompressedMsg signals that the agent loop auto-compressed context
// to recover from context window overflow.
type AgentCompressedMsg struct {
	MessagesRemoved int
	TokensSaved     int
}

// AgentThinkingMsg signals the agent is waiting for LLM response.
type AgentThinkingMsg struct {
	Iteration int
}

// AgentIterationDoneMsg signals that one LLM streaming turn completed with
// tool calls. The TUI should finalize the current streaming message so the
// next iteration starts fresh. This prevents duplicate content accumulation.
type AgentIterationDoneMsg struct {
	Iteration int
	ToolCount int
}

// AgentIterationMsg signals the start of a new agent iteration with a
// summary of what the agent is about to do.
type AgentIterationMsg struct {
	Iteration int
	ToolCalls []types.ToolCall
}

// AgentToolProgressMsg signals periodic elapsed-time updates while a tool is executing.
type AgentToolProgressMsg struct {
	ToolCall  types.ToolCall
	ElapsedMs int64
}

// AgentToolCallAcc accumulates streaming tool call deltas by index.
type agentToolCallAcc = types.ToolCallAcc

// AgentLoop runs an autonomous tool-use loop: send to LLM, execute tool calls,
// feed results back, repeat until the LLM produces a text-only response.
// systemPrompt is the fully composed system prompt (base + autonomous + tool-use + project context).
// contextLength is the model's context window size in tokens (0 = use default 128K).
// All communication with the TUI happens through the returned channel.
func AgentLoop(
	ctx context.Context,
	p provider.LLMProvider,
	modelID string,
	dispatcher *tools.Dispatcher,
	initialMessages []types.Message,
	systemPrompt string,
	contextLength int64,
) (tea.Cmd, <-chan tea.Msg) {
	ch := make(chan tea.Msg, 64)

	go func() {
		defer close(ch)
		defer func() {
			if r := recover(); r != nil {
				ch <- AgentErrorMsg{Err: fmt.Errorf("agent panic: %w", r)}
			}
		}()

		childCtx, cancel := context.WithCancel(ctx)
		defer cancel()
		ctx = childCtx

		if contextLength <= 0 {
			contextLength = 128_000
		}
		estimator := tokens.NewEstimator(modelID)

		messages := make([]types.Message, 0, len(initialMessages)+20)

		systemMsg := types.Message{Role: "system", Content: systemPrompt}
		messages = append(messages, systemMsg)
		messages = append(messages, initialMessages...)

		toolDefs := tools.BuildToolDefs(dispatcher)

		// Auto-compress retry guard: at most one compression per loop to prevent infinite cycles.
		compressedThisLoop := false

		for iteration := 0; iteration < agentMaxIterations; iteration++ {
			select {
			case <-ctx.Done():
				return
			default:
			}

			ch <- AgentThinkingMsg{Iteration: iteration + 1}

			// Context pruning: estimate token usage and prune if needed
			estimatedTokens := estimator.EstimateMessages(messages)
			threshold70 := int(float64(contextLength) * 0.70)
			threshold80 := int(float64(contextLength) * 0.80)
			threshold95 := int(float64(contextLength) * 0.95)

			if estimatedTokens > threshold95 {
				// Attempt auto-compression before giving up
				if !compressedThisLoop {
					compressed := aggressiveCompress(messages, estimator, int(float64(contextLength)*0.50))
					if len(compressed) < len(messages) {
						removed := len(messages) - len(compressed)
						saved := estimatedTokens - estimator.EstimateMessages(compressed)
						messages = compressed
						compressedThisLoop = true
						ch <- AgentCompressedMsg{MessagesRemoved: removed, TokensSaved: saved}
						// Re-estimate after compression
						estimatedTokens = estimator.EstimateMessages(messages)
						if estimatedTokens > threshold95 {
							ch <- AgentErrorMsg{Err: fmt.Errorf(
								"context window exceeded after compression: estimated %d tokens (limit: %d at 95%%)", estimatedTokens, threshold95)}
							return
						}
					} else {
						ch <- AgentErrorMsg{Err: fmt.Errorf(
							"context window exceeded: estimated %d tokens (limit: %d at 95%%)", estimatedTokens, threshold95)}
						return
					}
				} else {
					ch <- AgentErrorMsg{Err: fmt.Errorf(
						"context window exceeded: estimated %d tokens (limit: %d at 95%%)", estimatedTokens, threshold95)}
					return
				}
			}

			// Progressive pruning: start at 70% to prevent hitting 95%
			if estimatedTokens > threshold70 {
				pruneOldToolResults(messages, estimator)
			}

			// Aggressive pruning at 80%: also prune old assistant messages with large content
			if estimatedTokens > threshold80 {
				pruneOldAssistantContent(messages)
			}

			req := provider.ChatRequest{
				Model:            modelID,
				Messages:         messages,
				Tools:            toolDefs,
				ReasoningEnabled: true,
			}

			iterator, err := p.ChatCompletionStream(ctx, req)
			if err != nil {
				// Check if this is a context overflow error from the provider
				if errors.Is(err, m31errors.ErrContextExceeded) && !compressedThisLoop {
					compressed := aggressiveCompress(messages, estimator, int(float64(contextLength)*0.50))
					if len(compressed) < len(messages) {
						removed := len(messages) - len(compressed)
						saved := estimator.EstimateMessages(messages) - estimator.EstimateMessages(compressed)
						messages = compressed
						compressedThisLoop = true
						ch <- AgentCompressedMsg{MessagesRemoved: removed, TokensSaved: saved}
						continue // retry the iteration with compressed context
					}
				}
				ch <- AgentErrorMsg{Err: fmt.Errorf("LLM request failed: %w", err)}
				return
			}

			// Unblock iterator.Next() on context cancellation.
			// Without this goroutine, scanner.Scan() blocks on network I/O
			// and the context check inside SSEParser.Next() never runs.
			iterDone := make(chan struct{})
			go func() {
				select {
				case <-ctx.Done():
					_ = iterator.Close()
				case <-iterDone:
				}
			}()

			var fullContent strings.Builder
			accMap := make(map[int]*agentToolCallAcc)
			var lastUsage *types.Usage

			for {
				chunk, err := iterator.Next()
				if err == io.EOF {
					break
				}
				if err != nil {
					if ctx.Err() != nil {
						_ = iterator.Close()
						return
					}
					ch <- AgentErrorMsg{Err: fmt.Errorf("stream error: %w", err)}
					_ = iterator.Close()
					return
				}
				if chunk == nil {
					continue
				}

				if chunk.Type == "tool_call" {
					acc, ok := accMap[chunk.Index]
					if !ok {
						acc = &agentToolCallAcc{Index: chunk.Index}
						accMap[chunk.Index] = acc
					}
					if chunk.ToolCallID != "" {
						acc.ID = chunk.ToolCallID
					}
					if chunk.ToolName != "" {
						acc.Name = chunk.ToolName
					}
					acc.Args.WriteString(chunk.ToolInput)
				}

				if chunk.Type == "content" {
					fullContent.WriteString(chunk.Delta)
					ch <- AgentStreamMsg{Chunk: chunk}
				}

				if chunk.Usage != nil {
					lastUsage = chunk.Usage
				}

				if chunk.Type == "done" {
					break
				}
			}
			_ = iterator.Close()
			close(iterDone)

			calls := buildAgentToolCalls(accMap)

			// Text-based fallback: if no native tool calls, try parsing
			// JSON tool calls embedded in the response text.
			if len(calls) == 0 {
				calls = parseTextToolCalls(fullContent.String())
			}

			if len(calls) == 0 {
				msg := types.Message{
					Role:    "assistant",
					Content: fullContent.String(),
					Usage:   lastUsage,
				}
				// Calibrate estimator with actual input token counts from the API.
				// PromptTokens is the model's actual count of the input context,
				// which we compare against our estimate of the full message list
				// to correct provider-specific heuristics over successive calls.
				if lastUsage != nil && lastUsage.PromptTokens > 0 {
					estimated := estimator.EstimateMessages(messages)
					estimator.Calibrate(estimated, lastUsage.PromptTokens)
				}
				ch <- AgentDoneMsg{Message: msg, Usage: lastUsage}
				return
			}

			// Finalize the current streaming message before tool execution
			ch <- AgentIterationDoneMsg{
				Iteration: iteration + 1,
				ToolCount: len(calls),
			}

			// Announce tool calls for this iteration
			ch <- AgentIterationMsg{
				Iteration: iteration + 1,
				ToolCalls: calls,
			}

			assistantMsg := types.Message{
				Role:      "assistant",
				Content:   fullContent.String(),
				ToolCalls: calls,
			}
			messages = append(messages, assistantMsg)

			for _, tc := range calls {
				ch <- AgentToolStartMsg{ToolCall: tc}

				start := time.Now()

				// Progress ticker: send elapsed time every 500ms.
				// progressDone is closed after tool execution completes,
				// stopping the ticker goroutine immediately rather than
				// deferring until the agent loop exits.
				progressDone := make(chan struct{})
				go func(toolCall types.ToolCall) {
					ticker := time.NewTicker(500 * time.Millisecond)
					defer ticker.Stop()
					for {
						select {
						case <-ticker.C:
							ch <- AgentToolProgressMsg{
								ToolCall:  toolCall,
								ElapsedMs: time.Since(start).Milliseconds(),
							}
						case <-progressDone:
							return
						case <-ctx.Done():
							return
						}
					}
				}(tc)

				result, execErr := dispatcher.Execute(ctx, tc)
				duration := time.Since(start).Milliseconds()
				close(progressDone) // stop ticker goroutine immediately

				ch <- AgentToolDoneMsg{
					ToolCall:   tc,
					Result:     result,
					Err:        execErr,
					DurationMs: duration,
				}

				toolMsg := types.Message{
					Role:       "tool",
					ToolCallID: tc.ID,
				}
				if execErr != nil {
					toolMsg.Content = fmt.Sprintf("Error: %v", execErr)
				} else {
					toolMsg.Content = result.Output
				}
				messages = append(messages, toolMsg)
			}
		}

		ch <- AgentErrorMsg{Err: fmt.Errorf("agent loop exceeded max iterations (%d)", agentMaxIterations)}
	}()

	cmd := func() tea.Msg {
		msg, ok := <-ch
		if !ok {
			return nil
		}
		return msg
	}

	return cmd, ch
}

// buildAgentToolCalls converts accumulated tool call deltas into []ToolCall sorted by index.
func buildAgentToolCalls(accMap map[int]*agentToolCallAcc) []types.ToolCall {
	return types.BuildToolCallsFromAcc(accMap)
}

// BuildAgentMessages constructs the initial message slice for the agent loop
// from the REPL conversation history and user input.
func BuildAgentMessages(replMessages []types.Message, userInput string) []types.Message {
	var msgs []types.Message
	for _, msg := range replMessages {
		if !msg.SkipForLLM {
			msgs = append(msgs, msg)
		}
	}

	userMsg := types.Message{
		Role:    "user",
		Content: userInput,
	}
	if len(msgs) > 0 && msgs[len(msgs)-1].Role == "user" {
		msgs[len(msgs)-1] = userMsg
	} else {
		msgs = append(msgs, userMsg)
	}

	return msgs
}

// TruncateMessagesForLLM smartly truncates message history to fit within
// the model's context window. Strategy:
// 1. Keep system messages (first position)
// 2. Keep the most recent N messages (for context continuity)
// 3. If still over budget, remove oldest non-critical messages
// Returns the truncated slice and whether truncation occurred.
func TruncateMessagesForLLM(messages []types.Message, contextLength int64, estimator *tokens.Estimator) ([]types.Message, bool) {
	if contextLength <= 0 {
		contextLength = 128_000
	}

	// Reserve 20% for response generation
	maxInputTokens := int(float64(contextLength) * 0.80)

	// Estimate current usage
	totalTokens := estimator.EstimateMessages(messages)
	if totalTokens <= maxInputTokens {
		return messages, false
	}

	// Smart truncation: keep system prompt + recent messages
	var systemMsgs []types.Message
	var otherMsgs []types.Message

	for _, msg := range messages {
		if msg.Role == "system" {
			systemMsgs = append(systemMsgs, msg)
		} else {
			otherMsgs = append(otherMsgs, msg)
		}
	}

	// Calculate tokens used by system messages
	systemTokens := estimator.EstimateMessages(systemMsgs)
	remainingBudget := maxInputTokens - systemTokens

	if remainingBudget <= 0 {
		// System messages alone exceed budget - return just system + last message
		if len(messages) > 0 {
			return append(systemMsgs, messages[len(messages)-1]), true
		}
		return systemMsgs, true
	}

	// Keep recent messages until budget is exhausted
	var keptMsgs []types.Message
	tokensUsed := 0

	// Iterate from most recent to oldest
	for i := len(otherMsgs) - 1; i >= 0; i-- {
		msgTokens := estimator.Estimate(otherMsgs[i].Content)
		// Add overhead for tool calls
		for _, tc := range otherMsgs[i].ToolCalls {
			if len(tc.Input) > 0 {
				msgTokens += estimator.Estimate(string(tc.Input))
			}
		}

		if tokensUsed+msgTokens > remainingBudget {
			break
		}

		keptMsgs = append(keptMsgs, otherMsgs[i])
		tokensUsed += msgTokens
	}

	// Reverse to restore chronological order
	for i, j := 0, len(keptMsgs)-1; i < j; i, j = i+1, j-1 {
		keptMsgs[i], keptMsgs[j] = keptMsgs[j], keptMsgs[i]
	}

	// Combine system + kept messages
	result := make([]types.Message, 0, len(systemMsgs)+len(keptMsgs))
	result = append(result, systemMsgs...)
	result = append(result, keptMsgs...)

	return result, true
}

// LoadProjectContextForAgent loads AGENTS.md/MEMORY.md context for the given
// working directory and returns the content string.
func LoadProjectContextForAgent(cwd string) string {
	content, _ := config.LoadProjectContext(cwd)
	return content
}

// pruneOldToolResults truncates older tool result messages to reduce token usage.
// It keeps the last 3 tool results in full and removes older ones completely.
func pruneOldToolResults(messages []types.Message, estimator *tokens.Estimator) {
	const keepRecent = 3

	var toolIndices []int
	for i, msg := range messages {
		if msg.Role == "tool" {
			toolIndices = append(toolIndices, i)
		}
	}

	if len(toolIndices) <= keepRecent {
		return
	}

	// Remove older tool messages completely (set content to empty and mark for skip)
	cutoff := len(toolIndices) - keepRecent
	for _, idx := range toolIndices[:cutoff] {
		messages[idx].Content = "[tool result pruned to save context]"
		messages[idx].SkipForLLM = true
	}
}

// pruneOldAssistantContent truncates older assistant messages to reduce token usage.
// Keeps the last 5 assistant messages in full, truncates older ones.
func pruneOldAssistantContent(messages []types.Message) {
	const keepRecent = 5
	const maxContentChars = 1000

	var assistantIndices []int
	for i, msg := range messages {
		if msg.Role == "assistant" && len(msg.ToolCalls) == 0 {
			assistantIndices = append(assistantIndices, i)
		}
	}

	if len(assistantIndices) <= keepRecent {
		return
	}

	// Truncate older assistant messages
	cutoff := len(assistantIndices) - keepRecent
	for _, idx := range assistantIndices[:cutoff] {
		content := messages[idx].Content
		if len(content) > maxContentChars {
			messages[idx].Content = content[:maxContentChars] + "\n...[truncated to save context]"
		}
	}
}

// toolCallTextJSON represents a tool call parsed from text-embedded JSON.
type toolCallTextJSON struct {
	Name   string          `json:"name"`
	Tool   string          `json:"tool"`
	Input  json.RawMessage `json:"input"`
	Params json.RawMessage `json:"params"`
}

// parseTextToolCalls scans text content for JSON objects that look like tool calls.
// This is a fallback for models that don't support native function calling and
// instead embed tool call JSON in code blocks or inline text.
func parseTextToolCalls(content string) []types.ToolCall {
	const maxScanBytes = 64 << 10
	const maxToolCalls = 20

	var calls []types.ToolCall
	scanLimit := len(content)
	if scanLimit > maxScanBytes {
		scanLimit = maxScanBytes
	}

	for i := 0; i < scanLimit && len(calls) < maxToolCalls; i++ {
		if content[i] != '{' {
			continue
		}

		obj := extractAgentJSONObject(content[i:])
		if obj == "" {
			continue
		}

		if !strings.Contains(obj, `"name"`) && !strings.Contains(obj, `"tool"`) {
			i += len(obj) - 1
			continue
		}

		var tc toolCallTextJSON
		if err := json.Unmarshal([]byte(obj), &tc); err != nil {
			i += len(obj) - 1
			continue
		}

		name := tc.Name
		if name == "" {
			name = tc.Tool
		}
		if name == "" {
			i += len(obj) - 1
			continue
		}

		input := tc.Input
		if len(input) == 0 {
			input = tc.Params
		}
		if len(input) == 0 {
			input = json.RawMessage("{}")
		}

		calls = append(calls, types.ToolCall{
			ID:    fmt.Sprintf("text_%s_%d", name, len(calls)+1),
			Name:  name,
			Input: input,
		})

		i += len(obj) - 1
	}

	return calls
}

// extractAgentJSONObject finds the first complete JSON object starting at the
// beginning of s. Returns empty string if no valid object is found.
func extractAgentJSONObject(s string) string {
	dec := json.NewDecoder(strings.NewReader(s))
	var raw json.RawMessage
	if err := dec.Decode(&raw); err != nil {
		return ""
	}
	return string(raw)
}

// aggressiveCompress reduces message history to fit within targetTokens by:
// 1. Keeping system messages
// 2. Keeping the last N messages (lastUserKeep)
// 3. Summarizing the oldest non-protected messages into a single memory message
// Returns the compressed message slice.
func aggressiveCompress(messages []types.Message, estimator *tokens.Estimator, targetTokens int) []types.Message {
	if len(messages) <= 2 {
		return messages
	}

	// Separate system from non-system
	var systemMsgs []types.Message
	var otherMsgs []types.Message
	for _, msg := range messages {
		if msg.Role == "system" {
			systemMsgs = append(systemMsgs, msg)
		} else {
			otherMsgs = append(otherMsgs, msg)
		}
	}

	if len(otherMsgs) <= 2 {
		return messages
	}

	// Keep last 4 messages intact for conversation continuity
	const lastKeep = 4
	if len(otherMsgs) <= lastKeep {
		return messages
	}

	recent := otherMsgs[len(otherMsgs)-lastKeep:]
	old := otherMsgs[:len(otherMsgs)-lastKeep]

	// Build a summary from the old messages
	var summaryParts []string
	summaryParts = append(summaryParts, "[Context compressed — older conversation summarized]")
	for _, msg := range old {
		switch msg.Role {
		case "user":
			content := msg.Content
			if len(content) > 200 {
				content = content[:200] + "..."
			}
			summaryParts = append(summaryParts, "User: "+content)
		case "assistant":
			if len(msg.ToolCalls) > 0 {
				names := make([]string, 0, len(msg.ToolCalls))
				for _, tc := range msg.ToolCalls {
					names = append(names, tc.Name)
				}
				summaryParts = append(summaryParts, fmt.Sprintf("Assistant called tools: %s", strings.Join(names, ", ")))
			} else {
				content := msg.Content
				if len(content) > 200 {
					content = content[:200] + "..."
				}
				summaryParts = append(summaryParts, "Assistant: "+content)
			}
		case "tool":
			content := msg.Content
			if len(content) > 100 {
				content = content[:100] + "..."
			}
			summaryParts = append(summaryParts, "Tool result: "+content)
		}
	}

	memoryMsg := types.Message{
		Role:    "system",
		Content: strings.Join(summaryParts, "\n"),
	}

	// Reconstruct: system + memory + recent
	result := make([]types.Message, 0, len(systemMsgs)+1+len(recent))
	result = append(result, systemMsgs...)
	result = append(result, memoryMsg)
	result = append(result, recent...)
	return result
}
