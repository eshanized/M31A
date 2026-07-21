package streaming

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/core/types"
)

// --- pruneOldAssistantContent tests ---

func TestPruneOldAssistantContent_NoAssistantMessages(t *testing.T) {
	t.Parallel()
	msgs := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "tool", Content: "result"},
	}
	pruneOldAssistantContent(msgs)
	if msgs[0].Content != "hello" {
		t.Error("user message should not be modified")
	}
}

func TestPruneOldAssistantContent_FewerThanKeep(t *testing.T) {
	t.Parallel()
	longContent := strings.Repeat("a", 1500)
	msgs := []types.Message{
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
	}
	pruneOldAssistantContent(msgs)
	for i, msg := range msgs {
		if msg.Content != longContent {
			t.Errorf("message %d should not be truncated", i)
		}
	}
}

func TestPruneOldAssistantContent_MoreThanKeep(t *testing.T) {
	t.Parallel()
	longContent := strings.Repeat("a", 1500)
	msgs := []types.Message{
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
	}
	pruneOldAssistantContent(msgs)
	// First 2 should be truncated (7 total, keep last 5)
	if len(msgs[0].Content) >= 1500 {
		t.Error("first message should be truncated")
	}
	if !strings.HasSuffix(msgs[0].Content, "\n...[truncated to save context]") {
		t.Error("truncated message should end with truncation marker")
	}
	// Last 5 should be intact
	for i := 2; i < 7; i++ {
		if msgs[i].Content != longContent {
			t.Errorf("message %d should not be truncated", i)
		}
	}
}

func TestPruneOldAssistantContent_ShortContent_NoTruncation(t *testing.T) {
	t.Parallel()
	shortContent := "short"
	msgs := []types.Message{
		{Role: "assistant", Content: shortContent},
		{Role: "assistant", Content: shortContent},
		{Role: "assistant", Content: shortContent},
		{Role: "assistant", Content: shortContent},
		{Role: "assistant", Content: shortContent},
		{Role: "assistant", Content: shortContent},
		{Role: "assistant", Content: shortContent},
	}
	pruneOldAssistantContent(msgs)
	// Short content should not be truncated even if over keep limit
	for i, msg := range msgs {
		if msg.Content != shortContent {
			t.Errorf("message %d should not be truncated for short content", i)
		}
	}
}

func TestPruneOldAssistantContent_MixedRoles(t *testing.T) {
	t.Parallel()
	longContent := strings.Repeat("b", 1500)
	msgs := []types.Message{
		{Role: "assistant", Content: longContent},
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: longContent},
		{Role: "user", Content: "world"},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
	}
	// 7 assistant messages (non-tool-call), keep last 5
	pruneOldAssistantContent(msgs)
	if len(msgs[0].Content) >= 1500 {
		t.Error("first assistant message should be truncated")
	}
}

func TestPruneOldAssistantContent_WithToolCalls(t *testing.T) {
	t.Parallel()
	longContent := strings.Repeat("c", 1500)
	msgs := []types.Message{
		{Role: "assistant", Content: longContent, ToolCalls: []types.ToolCall{{Name: "Bash"}}},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
		{Role: "assistant", Content: longContent},
	}
	// Only 6 non-tool-call assistant messages, keep last 5
	pruneOldAssistantContent(msgs)
	// First non-tool-call message should be truncated
	if len(msgs[1].Content) >= 1500 {
		t.Error("first non-tool-call assistant message should be truncated")
	}
}

// --- TruncateMessagesForLLM tests ---

func TestTruncateMessagesForLLM_SmallContext(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	msgs := []types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "hello"},
	}
	result, truncated := TruncateMessagesForLLM(msgs, 128000, estimator)
	if truncated {
		t.Error("should not truncate small context")
	}
	if len(result) != 2 {
		t.Errorf("result len = %d, want 2", len(result))
	}
}

func TestTruncateMessagesForLLM_ZeroContext(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	msgs := []types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "hello"},
	}
	// Zero context should use default 128K
	result, truncated := TruncateMessagesForLLM(msgs, 0, estimator)
	if truncated {
		t.Error("should not truncate with default context")
	}
	if len(result) != 2 {
		t.Errorf("result len = %d, want 2", len(result))
	}
}

func TestTruncateMessagesForLLM_SystemOnly(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	msgs := []types.Message{
		{Role: "system", Content: "You are helpful"},
	}
	result, _ := TruncateMessagesForLLM(msgs, 1000, estimator)
	if len(result) < 1 {
		t.Error("should keep at least system message")
	}
}

func TestTruncateMessagesForLLM_NoSystem(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	msgs := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi there"},
		{Role: "user", Content: "how are you"},
	}
	result, _ := TruncateMessagesForLLM(msgs, 128000, estimator)
	if len(result) == 0 {
		t.Error("should have some messages")
	}
}

// --- aggressiveCompress tests ---

func TestAggressiveCompress_TooFewMessages(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	msgs := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi"},
	}
	result := aggressiveCompress(msgs, estimator, 100)
	if len(result) != 2 {
		t.Errorf("result len = %d, want 2 (no compression)", len(result))
	}
}

func TestAggressiveCompress_WithSystem(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	msgs := []types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi"},
	}
	result := aggressiveCompress(msgs, estimator, 100)
	if len(result) != 3 {
		t.Errorf("result len = %d, want 3", len(result))
	}
}

func TestAggressiveCompress_ManyMessages(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	var msgs []types.Message
	msgs = append(msgs, types.Message{Role: "system", Content: "You are helpful"})
	for i := 0; i < 10; i++ {
		msgs = append(msgs, types.Message{Role: "user", Content: strings.Repeat("q", 100)})
		msgs = append(msgs, types.Message{Role: "assistant", Content: strings.Repeat("a", 100)})
	}
	result := aggressiveCompress(msgs, estimator, 100)
	// Should have system + memory + 4 recent = 6
	if len(result) > 8 {
		t.Errorf("result len = %d, expected compressed", len(result))
	}
	// First should be system
	if result[0].Role != "system" {
		t.Errorf("first msg role = %q, want system", result[0].Role)
	}
	// Second should be memory summary
	if result[1].Role != "system" || !strings.Contains(result[1].Content, "[Context compressed") {
		t.Error("second msg should be memory summary")
	}
}

func TestAggressiveCompress_WithToolCalls(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	var msgs []types.Message
	msgs = append(msgs, types.Message{Role: "system", Content: "system"})
	for i := 0; i < 10; i++ {
		msgs = append(msgs, types.Message{
			Role:      "assistant",
			ToolCalls: []types.ToolCall{{Name: "Bash"}},
		})
		msgs = append(msgs, types.Message{Role: "tool", Content: "result"})
	}
	result := aggressiveCompress(msgs, estimator, 100)
	if len(result) > 8 {
		t.Errorf("result len = %d, expected compressed", len(result))
	}
}

func TestAggressiveCompress_LongContent_Truncated(t *testing.T) {
	t.Parallel()
	estimator := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	var msgs []types.Message
	msgs = append(msgs, types.Message{Role: "system", Content: "system"})
	for i := 0; i < 10; i++ {
		msgs = append(msgs, types.Message{Role: "user", Content: strings.Repeat("x", 500)})
		msgs = append(msgs, types.Message{Role: "assistant", Content: strings.Repeat("y", 500)})
	}
	result := aggressiveCompress(msgs, estimator, 100)
	// Memory message should have truncated content
	if len(result) >= 2 {
		memContent := result[1].Content
		if !strings.Contains(memContent, "...") {
			t.Error("memory summary should truncate long content")
		}
	}
}

// --- Message struct tests ---

func TestAgentCompressedMsg(t *testing.T) {
	t.Parallel()
	msg := AgentCompressedMsg{MessagesRemoved: 5, TokensSaved: 1000}
	if msg.MessagesRemoved != 5 || msg.TokensSaved != 1000 {
		t.Error("AgentCompressedMsg fields wrong")
	}
}

func TestAgentIterationMsg(t *testing.T) {
	t.Parallel()
	msg := AgentIterationMsg{
		Iteration: 3,
		ToolCalls: []types.ToolCall{{Name: "Bash"}},
	}
	if msg.Iteration != 3 || len(msg.ToolCalls) != 1 {
		t.Error("AgentIterationMsg fields wrong")
	}
}

func TestAgentToolProgressMsg(t *testing.T) {
	t.Parallel()
	msg := AgentToolProgressMsg{
		ToolCall:  types.ToolCall{Name: "Bash"},
		ElapsedMs: 500,
	}
	if msg.ElapsedMs != 500 {
		t.Error("AgentToolProgressMsg ElapsedMs wrong")
	}
}

func TestAgentStreamMsg(t *testing.T) {
	t.Parallel()
	chunk := &types.StreamChunk{Delta: "hello"}
	msg := AgentStreamMsg{Chunk: chunk}
	if msg.Chunk.Delta != "hello" {
		t.Error("AgentStreamMsg Chunk wrong")
	}
}

func TestAgentToolStartMsg(t *testing.T) {
	t.Parallel()
	msg := AgentToolStartMsg{ToolCall: types.ToolCall{Name: "Edit"}}
	if msg.ToolCall.Name != "Edit" {
		t.Error("AgentToolStartMsg ToolCall wrong")
	}
}

func TestAgentToolDoneMsg(t *testing.T) {
	t.Parallel()
	msg := AgentToolDoneMsg{
		ToolCall:   types.ToolCall{Name: "Bash"},
		Result:     types.ToolResult{Output: "done"},
		DurationMs: 100,
	}
	if msg.DurationMs != 100 {
		t.Error("AgentToolDoneMsg DurationMs wrong")
	}
}

func TestAgentDoneMsg(t *testing.T) {
	t.Parallel()
	msg := AgentDoneMsg{Message: types.Message{Content: "final"}}
	if msg.Message.Content != "final" {
		t.Error("AgentDoneMsg wrong")
	}
}

func TestAgentErrorMsg(t *testing.T) {
	t.Parallel()
	err := types.Message{Content: "error"}
	msg := AgentErrorMsg{Err: nil}
	_ = msg
	_ = err
}

func TestAgentThinkingMsg(t *testing.T) {
	t.Parallel()
	msg := AgentThinkingMsg{Iteration: 2}
	if msg.Iteration != 2 {
		t.Error("AgentThinkingMsg Iteration wrong")
	}
}

func TestAgentIterationDoneMsg(t *testing.T) {
	t.Parallel()
	msg := AgentIterationDoneMsg{Iteration: 1, ToolCount: 3}
	if msg.Iteration != 1 || msg.ToolCount != 3 {
		t.Error("AgentIterationDoneMsg fields wrong")
	}
}

func TestAgentMaxIterations(t *testing.T) {
	t.Parallel()
	if agentMaxIterations != 50 {
		t.Errorf("agentMaxIterations = %d, want 50", agentMaxIterations)
	}
}

// --- streaming.go struct tests ---

func TestStreamMsg(t *testing.T) {
	t.Parallel()
	chunk := &types.StreamChunk{Delta: "chunk"}
	msg := StreamMsg{Chunk: chunk}
	if msg.Chunk.Delta != "chunk" {
		t.Error("StreamMsg wrong")
	}
}

func TestStreamDoneMsg(t *testing.T) {
	t.Parallel()
	msg := StreamDoneMsg{}
	_ = msg
}

func TestStreamErrorMsg(t *testing.T) {
	t.Parallel()
	msg := StreamErrorMsg{ProviderName: "openai"}
	if msg.ProviderName != "openai" {
		t.Error("StreamErrorMsg wrong")
	}
}

func TestTickMsg(t *testing.T) {
	t.Parallel()
	msg := TickMsg{}
	_ = msg
}
