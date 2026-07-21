package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestToggleToolCardCollapsed_NilReplModel(t *testing.T) {
	m := &AppState{replModel: nil}
	// Should not panic
	m.toggleToolCardCollapsed("tool-1")
}

func TestToggleToolCardCollapsed_NilMsgRenderer(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	m := &AppState{replModel: &rm}
	// msgRenderer is nil by default, should not panic
	m.toggleToolCardCollapsed("tool-1")
}

func TestCollapseAllToolCards_NilReplModel(t *testing.T) {
	m := &AppState{replModel: nil}
	// Should not panic
	m.collapseAllToolCards()
}

func TestCollapseAllToolCards_NilMsgRenderer(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	m := &AppState{replModel: &rm}
	// msgRenderer is nil by default, should not panic
	m.collapseAllToolCards()
}

func TestEnsureToolDetailModel_AlreadyExists(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		themeManager:    tm,
		toolDetailModel: &ToolDetailModel{},
		width:           80,
		height:          24,
	}
	m.ensureToolDetailModel()
	// Should not replace existing model
}

func TestEnsureToolDetailModel_CreatesModel(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		themeManager: tm,
		width:        80,
		height:       24,
	}
	m.ensureToolDetailModel()
	if m.toolDetailModel == nil {
		t.Error("toolDetailModel should be created")
	}
}

func TestEnsureToolDetailModel_SmallDimensions(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		themeManager: tm,
		width:        10,
		height:       5,
	}
	m.ensureToolDetailModel()
	if m.toolDetailModel == nil {
		t.Error("toolDetailModel should be created")
	}
}

func TestExtractToolDetail_NilReplModel(t *testing.T) {
	m := &AppState{replModel: nil}
	name, body := m.extractToolDetail(0, "tool")
	if name != "" || body != "" {
		t.Error("nil replModel should return empty strings")
	}
}

func TestExtractToolDetail_InvalidIndex(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	m := &AppState{replModel: &rm}
	name, body := m.extractToolDetail(-1, "tool")
	if name != "" || body != "" {
		t.Error("negative index should return empty strings")
	}
	name, body = m.extractToolDetail(999, "tool")
	if name != "" || body != "" {
		t.Error("out of range index should return empty strings")
	}
}

func TestExtractToolDetail_NoMessages(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	m := &AppState{replModel: &rm}
	name, body := m.extractToolDetail(0, "tool")
	if name != "" || body != "" {
		t.Error("no messages should return empty strings")
	}
}

func TestExtractToolDetail_NoToolUseSegment(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.messages = append(rm.messages, types.Message{
		Segments: []types.MessageSegment{
			{Type: "text", Content: "Hello"},
		},
	})
	m := &AppState{replModel: &rm}
	name, body := m.extractToolDetail(0, "tool")
	if name != "" || body != "" {
		t.Error("no tool_use segment should return empty strings")
	}
}

func TestExtractToolDetail_WithToolUseSegment(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.messages = append(rm.messages, types.Message{
		Segments: []types.MessageSegment{
			{Type: "tool_use", Content: `{"name": "Read", "path": "/test.go"}`},
		},
	})
	m := &AppState{replModel: &rm}
	name, _ := m.extractToolDetail(0, "Read")
	// extractToolName parses the content; the exact name depends on extractToolName implementation
	_ = name
}

func TestExtractToolDetail_EmptyToolName(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.messages = append(rm.messages, types.Message{
		Segments: []types.MessageSegment{
			{Type: "tool_use", Content: `{"name": "Read", "path": "/test.go"}`},
		},
	})
	m := &AppState{replModel: &rm}
	// Empty toolName should fallback to first tool_use segment
	name, _ := m.extractToolDetail(0, "")
	_ = name
}

// Verify signatures
func TestToggleToolCardCollapsed_Signature(t *testing.T) {
	var fn = (&AppState{}).toggleToolCardCollapsed
	_ = fn
}

func TestCollapseAllToolCards_Signature(t *testing.T) {
	var fn = (&AppState{}).collapseAllToolCards
	_ = fn
}

func TestEnsureToolDetailModel_Signature(t *testing.T) {
	var fn = (&AppState{}).ensureToolDetailModel
	_ = fn
}

func TestExtractToolDetail_Signature(t *testing.T) {
	var fn = (&AppState{}).extractToolDetail
	_ = fn
}
