package workflow

import (
	"testing"
)

// WorkflowEvent is the interface that narrative engine expects.
// Duplicated here to avoid importing pkg/narrative in tests.
type WorkflowEvent interface {
	EventType() string
	EventData() map[string]interface{}
}

func TestWorkflowEvent_AgentSwitchMsg(t *testing.T) {
	msg := AgentSwitchMsg{
		FromAgent: "planner",
		ToAgent:   "builder",
		PlanPath:  "/plans/test.md",
	}

	// Type assertion — must implement WorkflowEvent
	var event WorkflowEvent = msg

	if event.EventType() != "agent_switch" {
		t.Errorf("AgentSwitchMsg.EventType() = %q, want %q", event.EventType(), "agent_switch")
	}

	data := event.EventData()
	if data["from_agent"] != "planner" {
		t.Errorf("EventData()[from_agent] = %v, want %q", data["from_agent"], "planner")
	}
	if data["to_agent"] != "builder" {
		t.Errorf("EventData()[to_agent] = %v, want %q", data["to_agent"], "builder")
	}
	if data["plan_path"] != "/plans/test.md" {
		t.Errorf("EventData()[plan_path] = %v, want %q", data["plan_path"], "/plans/test.md")
	}
}

func TestWorkflowEvent_DecisionsSnapshotMsg(t *testing.T) {
	msg := DecisionsSnapshotMsg{
		Decisions: nil,
	}

	// Type assertion — must implement WorkflowEvent
	var event WorkflowEvent = msg

	if event.EventType() != "decisions_snapshot" {
		t.Errorf("DecisionsSnapshotMsg.EventType() = %q, want %q", event.EventType(), "decisions_snapshot")
	}

	data := event.EventData()
	if data["decision_count"] != 0 {
		t.Errorf("EventData()[decision_count] = %v, want %d", data["decision_count"], 0)
	}
}
