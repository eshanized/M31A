package subagent

import (
	"context"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// loop is the per-subagent agentic conversation loop.
//
// It runs entirely inside a single goroutine started by Manager.runLoop and
// never touches shared Bubble Tea state. Communication with the parent is
// one-way: it writes SubagentEvents to the manager's event channel.
type loop struct {
	manager    *Manager
	agent      *Subagent
	dispatcher ToolDispatcher
	provider   provider.LLMProvider
	modelID    string
	maxTools   int
	maxTokens  int

	// runtime state
	messages     []types.Message
	toolCallsRun int
	inputToks    int
	outputToks   int
	lastUsage    *types.Usage
}

// maxTurns caps the number of LLM round-trips a subagent can make even if it
// keeps asking for tools.
const maxTurns = 25

// run drives the loop until completion, error, or cancellation.
func (l *loop) run(ctx context.Context) {
	l.messages = []types.Message{
		{
			Role:      "system",
			Content:   l.buildSystemPrompt(),
			CreatedAt: time.Now(),
		},
		{
			Role:      "user",
			Content:   l.agent.req.Prompt,
			CreatedAt: time.Now(),
		},
	}

	budgetExhausted := false
	for turn := 0; turn < maxTurns; turn++ {
		if err := ctx.Err(); err != nil {
			l.finishCancelled(err)
			return
		}
		// Enforce tool-call budget.
		if l.toolCallsRun >= l.maxTools {
			budgetExhausted = true
			l.messages = append(l.messages, types.Message{
				Role:      "user",
				Content:   "Tool-call budget exhausted. Summarize your findings now without further tool use.",
				CreatedAt: time.Now(),
			})
		}
		// Enforce token budget.
		if l.inputToks+l.outputToks >= l.maxTokens && l.maxTokens > 0 {
			budgetExhausted = true
			l.messages = append(l.messages, types.Message{
				Role:      "user",
				Content:   "Token budget exhausted. Summarize your findings now without further tool use.",
				CreatedAt: time.Now(),
			})
		}
		done, err := l.runOneTurn(ctx, budgetExhausted)
		if err != nil {
			l.manager.failAgent(l.agent, err)
			return
		}
		if done {
			l.finishDone()
			return
		}
	}
	l.manager.failAgent(l.agent, fmt.Errorf("subagent: exceeded %d turns", maxTurns))
}

// runOneTurn performs a single LLM request + tool dispatch round.
// Returns (true, nil) when the assistant produced no tool calls (terminal).
// When budgetExhausted is true, tool definitions are omitted and any tool
// calls in the response are discarded — the LLM is forced to summarize.
func (l *loop) runOneTurn(ctx context.Context, budgetExhausted bool) (bool, error) {
	if l.provider == nil {
		return false, fmt.Errorf("no LLM provider configured")
	}

	req := provider.ChatRequest{
		Model:            l.modelID,
		Messages:         l.messages,
		ReasoningEnabled: true,
	}
	if !budgetExhausted && l.toolCallsRun < l.maxTools {
		req.Tools = l.buildToolDefinitions()
	}

	iterator, err := l.provider.ChatCompletionStream(ctx, req)
	if err != nil {
		return false, fmt.Errorf("chat stream: %w", err)
	}

	content, thinking, usage, err := l.consume(iterator)
	if err != nil {
		return false, fmt.Errorf("consume stream: %w", err)
	}
	if usage != nil {
		l.lastUsage = usage
		l.inputToks += usage.PromptTokens
		l.outputToks += usage.CompletionTokens
	}
	if thinking != "" {
		l.manager.emit(SubagentEvent{
			Type:    EventThinking,
			AgentID: l.agent.Info.ID,
			Delta:   thinking,
		})
	}

	toolCalls, parseErr := l.parseToolCalls(content)
	if parseErr != nil && len(toolCalls) == 0 && content != "" {
		l.messages = append(l.messages, types.Message{
			Role:      "assistant",
			Content:   content,
			CreatedAt: time.Now(),
			Usage:     usage,
		})
		return true, nil
	}

	if len(toolCalls) == 0 {
		l.messages = append(l.messages, types.Message{
			Role:      "assistant",
			Content:   content,
			CreatedAt: time.Now(),
			Usage:     usage,
		})
		return true, nil
	}

	// When the budget is exhausted, discard any tool calls the LLM tried to
	// generate — tools were not advertised, so executing them would be wrong.
	if budgetExhausted {
		l.messages = append(l.messages, types.Message{
			Role:      "assistant",
			Content:   content,
			CreatedAt: time.Now(),
			Usage:     usage,
		})
		return true, nil
	}

	// Append assistant message with tool calls, then dispatch each tool.
	// Convert the loop's ToolCallInput to the provider-shaped types.ToolCall
	// so the message history remains compatible with the parent's format.
	typedCalls := make([]types.ToolCall, len(toolCalls))
	for i, tc := range toolCalls {
		typedCalls[i] = types.ToolCall{
			ID:    tc.ID,
			Name:  tc.Name,
			Input: tc.Input,
		}
	}
	l.messages = append(l.messages, types.Message{
		Role:      "assistant",
		Content:   content,
		ToolCalls: typedCalls,
		CreatedAt: time.Now(),
		Usage:     usage,
	})
	for _, tc := range toolCalls {
		if err := l.runTool(ctx, tc); err != nil {
			return false, err
		}
	}
	return false, nil
}

// runTool dispatches a single tool call and appends the result as a tool
// message to the conversation history.
func (l *loop) runTool(ctx context.Context, tc ToolCallInput) error {
	start := time.Now()
	l.manager.emit(SubagentEvent{
		Type:       EventToolStart,
		AgentID:    l.agent.Info.ID,
		ToolCallID: tc.ID,
		ToolName:   tc.Name,
		ToolInput:  abbreviate(string(tc.Input), 120),
	})
	l.updateLastTool(tc.Name, "running")

	result, err := l.dispatcher.Execute(ctx, tc)
	elapsed := time.Since(start).Milliseconds()

	ev := SubagentEvent{
		Type:       EventToolDone,
		AgentID:    l.agent.Info.ID,
		ToolCallID: tc.ID,
		ToolName:   tc.Name,
		DurationMs: elapsed,
	}
	outText := result.Output
	if result.Error != "" {
		ev.ToolError = result.Error
		outText = "ERROR: " + result.Error
	} else if err != nil {
		ev.ToolError = err.Error()
		outText = "ERROR: " + err.Error()
	}
	ev.ToolOutput = abbreviate(outText, 400)
	l.manager.emit(ev)

	l.toolCallsRun++
	l.updateLastTool(tc.Name, "done")

	l.messages = append(l.messages, types.Message{
		Role:       "tool",
		Content:    outText,
		ToolCallID: tc.ID,
		CreatedAt:  time.Now(),
	})
	return nil
}

func (l *loop) updateLastTool(name, status string) {
	l.agent.mu.Lock()
	l.agent.Info.LastToolName = name
	l.agent.Info.LastToolStatus = status
	l.agent.Info.ToolCalls = l.toolCallsRun
	l.agent.Info.InputToks = l.inputToks
	l.agent.Info.OutputToks = l.outputToks
	l.agent.mu.Unlock()
}

// consume reads a StreamIterator to completion.
func (l *loop) consume(it *types.StreamIterator) (string, string, *types.Usage, error) {
	defer it.Close() //nolint:errcheck

	var content, thinking strings.Builder
	var lastUsage *types.Usage
	lastChunkType := ""

	for {
		chunk, err := it.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			if chunk != nil && chunk.Delta != "" {
				content.WriteString(chunk.Delta)
			}
			return content.String(), thinking.String(), lastUsage, err
		}
		if chunk == nil {
			continue
		}
		if chunk.Usage != nil {
			lastUsage = chunk.Usage
		}
		switch chunk.Type {
		case "thinking":
			thinking.WriteString(chunk.Delta)
		case "content", "":
			content.WriteString(chunk.Delta)
			if lastChunkType != "content" {
				l.manager.emit(SubagentEvent{
					Type:    EventTextDelta,
					AgentID: l.agent.Info.ID,
					Delta:   chunk.Delta,
				})
			}
		}
		lastChunkType = chunk.Type
	}
	return content.String(), thinking.String(), lastUsage, nil
}

func (l *loop) finishDone() {
	summary := l.extractSummary()
	l.agent.mu.Lock()
	l.agent.Info.Status = StatusDone
	l.agent.Info.FinishedAt = time.Now()
	l.agent.Info.LastSummary = summary
	l.agent.Info.ToolCalls = l.toolCallsRun
	l.agent.Info.InputToks = l.inputToks
	l.agent.Info.OutputToks = l.outputToks
	l.agent.mu.Unlock()

	l.manager.emit(SubagentEvent{
		Type:       EventDone,
		AgentID:    l.agent.Info.ID,
		Name:       l.agent.Info.Name,
		Summary:    summary,
		ToolCalls:  l.toolCallsRun,
		InputToks:  l.inputToks,
		OutputToks: l.outputToks,
		Usage:      l.lastUsage,
	})
}

func (l *loop) finishCancelled(err error) {
	l.agent.mu.Lock()
	l.agent.Info.Status = StatusCancel
	l.agent.Info.FinishedAt = time.Now()
	l.agent.Info.LastError = err.Error()
	l.agent.mu.Unlock()
	l.manager.emit(SubagentEvent{
		Type:    EventCancelled,
		AgentID: l.agent.Info.ID,
		Error:   err.Error(),
	})
}

func (l *loop) extractSummary() string {
	for i := len(l.messages) - 1; i >= 0; i-- {
		m := l.messages[i]
		if m.Role == "assistant" && strings.TrimSpace(m.Content) != "" {
			return abbreviate(m.Content, 800)
		}
	}
	return ""
}

func (l *loop) buildSystemPrompt() string {
	var sb strings.Builder
	sb.WriteString("You are a focused subagent running inside a parallel exploration swarm.\n")
	sb.WriteString("Your parent has given you a single, well-defined task. Execute it efficiently.\n\n")

	// Provide workspace context so the subagent can orient itself.
	fmt.Fprintf(&sb, "Working directory: %s\n", l.agent.Info.Worktree)
	if l.agent.Info.Isolation == IsolationWorktree {
		sb.WriteString("You are in an isolated git worktree — file changes here do not affect the parent.\n")
	} else {
		sb.WriteString("You share the parent's working directory — file changes affect the parent directly.\n")
	}
	sb.WriteString("\nGuidelines:\n")
	sb.WriteString("- Prefer parallel-friendly tools (Glob, Grep, FileRead, Bash with read-only commands).\n")
	sb.WriteString("- Do not ask the user questions; you run autonomously.\n")
	sb.WriteString("- When you have enough information, stop calling tools and write a clear summary.\n")
	sb.WriteString("- Keep tool calls minimal; every call costs tokens and time.\n")
	sb.WriteString("- Read files before editing them to understand existing code.\n")
	fmt.Fprintf(&sb, "- Tool-call budget: %d calls. Stop before exhausting it.\n", l.maxTools)
	fmt.Fprintf(&sb, "- Token budget: %d tokens (input + output).\n", l.maxTokens)
	fmt.Fprintf(&sb, "- Describe: %s\n", l.agent.req.Description)
	if l.agent.req.Name != "" {
		fmt.Fprintf(&sb, "- Name: %s\n", l.agent.req.Name)
	}
	return sb.String()
}

// buildToolDefinitions converts dispatcher descriptors into the provider's
// ToolDefinition format, omitting AskUserQuestion since subagents run
// without user interaction.
func (l *loop) buildToolDefinitions() []provider.ToolDefinition {
	descs := l.dispatcher.ListTools()
	defs := make([]provider.ToolDefinition, 0, len(descs))
	for _, d := range descs {
		if d.Name == "AskUserQuestion" {
			continue
		}
		params := d.ParameterSchema
		if params == "" {
			params = "{}"
		}
		defs = append(defs, provider.ToolDefinition{
			Name:        d.Name,
			Description: d.Description,
			Parameters:  params,
		})
	}
	return defs
}

func abbreviate(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	if n < 1 {
		return ""
	}
	return string(r[:n-1]) + "…"
}

func (l *loop) parseToolCalls(content string) ([]ToolCallInput, error) {
	return parseToolCalls(content, l.manager.deps.Logger)
}
