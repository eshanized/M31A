package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/internal/narrative"
	m31types "github.com/eshanized/M31A/internal/types"
)

func TestNarrativeStateUpdate(t *testing.T) {
	ns := NewNarrativeState()

	n1 := narrative.NarrativeObject{
		Type: narrative.NarrativeStartingTask,
		Text: "Implementing auth",
	}
	ns.Update(n1)

	if ns.Active.Type != narrative.NarrativeStartingTask {
		t.Errorf("Active.Type = %q, want %q", ns.Active.Type, narrative.NarrativeStartingTask)
	}
	if len(ns.History) != 0 {
		t.Errorf("History should be empty, got %d", len(ns.History))
	}
}

func TestNarrativeStateHistory(t *testing.T) {
	ns := NewNarrativeState()

	n1 := narrative.NarrativeObject{Type: narrative.NarrativeReadingFile, Text: "Reading main.go"}
	n2 := narrative.NarrativeObject{Type: narrative.NarrativeWritingFile, Text: "Writing config.go"}

	ns.Update(n1)
	ns.Update(n2)

	if ns.Active.Type != narrative.NarrativeWritingFile {
		t.Errorf("Active.Type = %q, want %q", ns.Active.Type, narrative.NarrativeWritingFile)
	}
	if len(ns.History) != 1 {
		t.Errorf("History length = %d, want 1", len(ns.History))
	}
	if ns.History[0].Type != narrative.NarrativeReadingFile {
		t.Errorf("History[0].Type = %q, want %q", ns.History[0].Type, narrative.NarrativeReadingFile)
	}
}

func TestNarrativeStateMaxHistory(t *testing.T) {
	ns := NewNarrativeState()

	for i := 0; i < 10; i++ {
		ns.Update(narrative.NarrativeObject{
			Type: narrative.NarrativeStartingTask,
			Text: "Task",
		})
	}

	if len(ns.History) > ns.MaxHist {
		t.Errorf("History length = %d, max = %d", len(ns.History), ns.MaxHist)
	}
}

func TestNarrativeStateClear(t *testing.T) {
	ns := NewNarrativeState()

	n1 := narrative.NarrativeObject{Type: narrative.NarrativeReadingFile, Text: "Reading"}
	ns.Update(n1)
	ns.Clear()

	if ns.Active.Type != "" {
		t.Error("Active should be empty after Clear")
	}
	if len(ns.History) != 1 {
		t.Errorf("History length = %d, want 1 after Clear", len(ns.History))
	}
}

func TestNarrativeBubbleMsg(t *testing.T) {
	msg := NarrativeBubbleMsg{
		Narrative: narrative.NarrativeObject{
			Type: narrative.NarrativeStartingTask,
			Text: "Implementing feature",
		},
	}

	if msg.Narrative.Type != narrative.NarrativeStartingTask {
		t.Errorf("Type = %q", msg.Narrative.Type)
	}
}

func TestNarrativeEmitterEndToEnd(t *testing.T) {
	// Create a mock emitter that captures messages
	ch := make(chan interface{}, 100)
	capturingEmitter := &capturingMsgEmitter{ch: ch}

	// Create the narrative bridge and engine
	bridge := narrative.NewBridge()
	engine := narrative.NewEngine(narrative.DefaultEngineConfig())

	// Simulate workflow messages
	msgs := []interface{}{
		workflow.PhaseTransitionStartMsg{From: "idle", To: "execute"},
		workflow.TaskStartMsg{Task: m31types.Task{ID: 1, Description: "fix auth"}},
		workflow.ToolStartMsg{ToolName: "FileRead", Description: "reading"},
		workflow.ToolCompleteMsg{ToolName: "FileRead", Success: true},
		workflow.TaskUpdateMsg{Task: m31types.Task{ID: 1, Description: "fix auth"}, Status: "done"},
	}

	for _, msg := range msgs {
		// Convert to RawEvent
		event, ok := bridge.MsgToRawEvent(msg)
		if !ok {
			continue
		}

		// Process through engine
		narratives := engine.ProcessEvent(event)

		// Emit narratives
		for _, n := range narratives {
			capturingEmitter.Emit(NarrativeBubbleMsg{Narrative: n})
		}
	}

	// Check that we got some narratives
	if len(ch) == 0 {
		t.Error("Expected at least one narrative message")
	}

	// Verify all messages are NarrativeBubbleMsg
	for i := 0; i < len(ch); i++ {
		msg := <-ch
		if _, ok := msg.(NarrativeBubbleMsg); !ok {
			t.Errorf("Message %d is not NarrativeBubbleMsg: %T", i, msg)
		}
	}
}

func TestRenderNarrativeSidebar(t *testing.T) {
	ns := NewNarrativeState()
	ns.Update(narrative.NarrativeObject{
		Type:     narrative.NarrativeStartingTask,
		Text:     "Implementing auth module",
		Category: narrative.CategoryExecuting,
	})

	lines := renderNarrativeSidebar(ns, testTheme(), 30)
	if len(lines) == 0 {
		t.Error("renderNarrativeSidebar should produce at least one line")
	}
}

func TestRenderNarrativeSidebarEmpty(t *testing.T) {
	lines := renderNarrativeSidebar(nil, testTheme(), 30)
	if lines != nil {
		t.Error("renderNarrativeSidebar(nil) should return nil")
	}
}

func TestRenderNarrativeFooter(t *testing.T) {
	ns := NewNarrativeState()
	ns.Update(narrative.NarrativeObject{
		Type: narrative.NarrativeStartingTask,
		Text: "Implementing auth",
	})

	result := renderNarrativeFooter(ns, testTheme(), 40)
	if result == "" {
		t.Error("renderNarrativeFooter should produce non-empty output")
	}
}

func TestRenderNarrativeFooterEmpty(t *testing.T) {
	result := renderNarrativeFooter(nil, testTheme(), 40)
	if result != "" {
		t.Error("renderNarrativeFooter(nil) should return empty string")
	}
}

func TestNarrativeCategoryLabel(t *testing.T) {
	tests := []struct {
		cat  narrative.Category
		want string
	}{
		{narrative.CategoryOrienting, "init"},
		{narrative.CategoryExecuting, "exec"},
		{narrative.CategoryAlerting, "alert"},
		{narrative.CategoryShipping, "ship"},
	}
	for _, tt := range tests {
		got := narrativeCategoryLabel(tt.cat)
		if got != tt.want {
			t.Errorf("narrativeCategoryLabel(%v) = %q, want %q", tt.cat, got, tt.want)
		}
	}
}

// capturingMsgEmitter captures emitted messages for testing.
type capturingMsgEmitter struct {
	ch chan interface{}
}

func (c *capturingMsgEmitter) Emit(msg interface{}) {
	select {
	case c.ch <- msg:
	default:
	}
}
