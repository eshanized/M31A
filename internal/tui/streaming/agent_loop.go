package streaming

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
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

// agentToolCallAcc accumulates streaming tool call deltas by index.
type agentToolCallAcc struct {
	id    string
	name  string
	args  strings.Builder
	index int
}

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

		if contextLength <= 0 {
			contextLength = 128_000
		}
		estimator := tokens.NewEstimator(modelID)

		messages := make([]types.Message, 0, len(initialMessages)+20)

		systemMsg := types.Message{Role: "system", Content: systemPrompt}
		messages = append(messages, systemMsg)
		messages = append(messages, initialMessages...)

		toolDefs := tools.BuildToolDefs(dispatcher)

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
				ch <- AgentErrorMsg{Err: fmt.Errorf(
					"context window exceeded: estimated %d tokens (limit: %d at 95%%)", estimatedTokens, threshold95)}
				return
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
						acc = &agentToolCallAcc{index: chunk.Index}
						accMap[chunk.Index] = acc
					}
					if chunk.ToolCallID != "" {
						acc.id = chunk.ToolCallID
					}
					if chunk.ToolName != "" {
						acc.name = chunk.ToolName
					}
					acc.args.WriteString(chunk.ToolInput)
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
				result, execErr := dispatcher.Execute(ctx, tc)
				duration := time.Since(start).Milliseconds()

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
	if len(accMap) == 0 {
		return nil
	}
	indices := make([]int, 0, len(accMap))
	for idx := range accMap {
		indices = append(indices, idx)
	}
	sort.Ints(indices)
	var result []types.ToolCall
	for _, idx := range indices {
		acc := accMap[idx]
		id := acc.id
		if id == "" {
			id = acc.name
		}
		argsStr := acc.args.String()
		var input json.RawMessage
		if argsStr != "" {
			input = json.RawMessage(argsStr)
		} else {
			input = json.RawMessage("{}")
		}
		result = append(result, types.ToolCall{
			ID:    id,
			Name:  acc.name,
			Input: input,
		})
	}
	return result
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
