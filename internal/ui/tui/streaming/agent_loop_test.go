package streaming

import (
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestAgentStreamMsg_NilChunk(t *testing.T) {
	t.Parallel()
	msg := AgentStreamMsg{}
	if msg.Chunk != nil {
		t.Error("expected nil chunk")
	}
}

func TestAgentDoneMsg_NilUsage(t *testing.T) {
	t.Parallel()
	msg := AgentDoneMsg{}
	if msg.Usage != nil {
		t.Error("expected nil usage")
	}
}

func TestAgentErrorMsg_NilError(t *testing.T) {
	t.Parallel()
	msg := AgentErrorMsg{}
	if msg.Err != nil {
		t.Error("expected nil error")
	}
}

func TestAgentIterationDoneMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := AgentIterationDoneMsg{
		Iteration: 5,
	}
	if msg.Iteration != 5 {
		t.Errorf("expected 5, got %d", msg.Iteration)
	}
}

func TestAgentThinkingMsg_Iteration(t *testing.T) {
	t.Parallel()
	msg := AgentThinkingMsg{
		Iteration: 3,
	}
	if msg.Iteration != 3 {
		t.Errorf("expected 3, got %d", msg.Iteration)
	}
}

func TestAgentToolStartMsg_ToolCall(t *testing.T) {
	t.Parallel()
	msg := AgentToolStartMsg{
		ToolCall: types.ToolCall{
			ID:   "call-1",
			Name: "Bash",
		},
	}
	if msg.ToolCall.Name != "Bash" {
		t.Errorf("expected 'Bash', got %q", msg.ToolCall.Name)
	}
}

func TestAgentToolDoneMsg_Result(t *testing.T) {
	t.Parallel()
	msg := AgentToolDoneMsg{
		ToolCall: types.ToolCall{
			ID:   "call-1",
			Name: "Bash",
		},
		Result: types.ToolResult{
			Output: "done",
		},
		DurationMs: 100,
	}
	if msg.DurationMs != 100 {
		t.Errorf("expected 100, got %d", msg.DurationMs)
	}
	if msg.Result.Output != "done" {
		t.Errorf("expected 'done', got %q", msg.Result.Output)
	}
}

func TestAgentMaxIterations_Value(t *testing.T) {
	t.Parallel()
	if agentMaxIterations != 50 {
		t.Errorf("expected 50, got %d", agentMaxIterations)
	}
}
