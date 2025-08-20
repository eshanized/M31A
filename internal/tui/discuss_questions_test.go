package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

// TestReplModel_ShowQuestion_DiscussQuestion verifies that the existing
// ReplModel.ShowQuestion handles a QuestionRequestMsg with no Options.
// Discuss questions are open-ended (unlike AskUserQuestion which has
// pre-defined options), so the renderer must accept empty Options
// and still display the question inline.
func TestReplModel_ShowQuestion_DiscussQuestion(t *testing.T) {
	th := theme.NewManager(theme.ModeDark).Current()
	m := NewReplModel(th, "test")
	m.ShowQuestion(QuestionRequestMsg{
		Question:    "Which web framework?",
		Header:      "Discuss Q1/3",
		Options:     []string{}, // Discuss questions have no options
		AllowCustom: true,
	})
	if m.activeQuestion == nil {
		t.Fatal("expected activeQuestion to be set")
	}
	if m.activeQuestion.Header != "Discuss Q1/3" {
		t.Errorf("expected header 'Discuss Q1/3', got %q", m.activeQuestion.Header)
	}
	if m.activeQuestion.Question != "Which web framework?" {
		t.Errorf("expected question preserved, got %q", m.activeQuestion.Question)
	}
	if !m.activeQuestion.AllowCustom {
		t.Error("expected AllowCustom to be true for discuss questions")
	}
}

// TestReplModel_ShowQuestion_WithOptions verifies the AskUserQuestion
// tool path still works — options list is non-empty.
func TestReplModel_ShowQuestion_WithOptions(t *testing.T) {
	th := theme.NewManager(theme.ModeDark).Current()
	m := NewReplModel(th, "test")
	m.ShowQuestion(QuestionRequestMsg{
		Question:    "Pick a language",
		Header:      "Language",
		Options:     []string{"Go", "Rust", "Python"},
		AllowCustom: false,
	})
	if m.activeQuestion == nil {
		t.Fatal("expected activeQuestion to be set")
	}
	if len(m.activeQuestion.Options) != 3 {
		t.Errorf("expected 3 options, got %d", len(m.activeQuestion.Options))
	}
}

// TestDiscussAnswerTimeoutMsg_HasQuestionIndex verifies the new
// DiscussAnswerTimeoutMsg type is usable (not a compile-time stub).
func TestDiscussAnswerTimeoutMsg_HasQuestionIndex(t *testing.T) {
	msg := DiscussAnswerTimeoutMsg{QuestionIndex: 2}
	if msg.QuestionIndex != 2 {
		t.Errorf("expected QuestionIndex=2, got %d", msg.QuestionIndex)
	}
}
