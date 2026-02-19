package session

import (
	"fmt"
	"time"
	"unicode"

	"github.com/eshanized/M31A/internal/types"
)

// CurrentSchemaVersion is the current session schema version.
// Increment when adding/removing fields that require migration.
const CurrentSchemaVersion = 1

// Session wraps types.Session with additional runtime state fields.
type Session struct {
	types.Session
	SchemaVersion int                 `json:"schema_version"`
	Messages      []types.Message     `json:"messages"`
	Tasks         []types.Task        `json:"tasks"`
	Project       *types.ProjectState `json:"project"`

	// ResumedAt records the time of the most recent resume. Nil for
	// never-resumed sessions. Updated on every Manager.LoadSession call.
	ResumedAt *time.Time `json:"resumed_at,omitempty"`

	// Workflow state (D-06 fix) — persisted to session.json so closing
	// the app mid-workflow doesn't abandon progress.
	WorkflowGoal     string   `json:"workflow_goal,omitempty"`
	DiscussQuestions []string `json:"discuss_questions,omitempty"`
}

// NewSession creates a new Session with default values.
func NewSession(id, model, provider string) *Session {
	return &Session{
		SchemaVersion: CurrentSchemaVersion,
		Session: types.Session{
			ID:            id,
			ChildrenIDs:   make([]string, 0),
			Model:         model,
			Provider:      provider,
			StartedAt:     time.Now(),
			MessageCount:  0,
			WorkflowPhase: types.PhaseIdle,
		},
		Messages:         make([]types.Message, 0),
		Tasks:            make([]types.Task, 0),
		DiscussQuestions: make([]string, 0),
	}
}

// SetPhase updates the workflow phase on the session.
func (s *Session) SetPhase(phase types.WorkflowPhase) {
	s.WorkflowPhase = phase
}

// SetWorkflowState records the workflow's current goal, phase, and
// pending discuss questions. The TUI calls this on every phase
// transition via Manager.UpdateWorkflowState.
func (s *Session) SetWorkflowState(goal string, phase types.WorkflowPhase, questions []string) {
	s.WorkflowGoal = goal
	s.WorkflowPhase = phase
	s.DiscussQuestions = questions
}

// WorkflowState returns the persisted workflow state. Zero values
// for the goal and questions, and PhaseIdle for the phase, mean
// no workflow is in progress.
func (s *Session) WorkflowState() (goal string, phase types.WorkflowPhase, questions []string) {
	return s.WorkflowGoal, s.WorkflowPhase, s.DiscussQuestions
}

// validateSessionID checks that id is exactly expectedLen
// lowercase hexadecimal characters [a-f0-9].
func validateSessionID(id string, expectedLen int) error {
	if expectedLen <= 0 {
		expectedLen = types.SessionIDLength
	}
	if len(id) != expectedLen {
		return fmt.Errorf("session ID must be %d chars, got %d", expectedLen, len(id))
	}
	for _, c := range id {
		if !unicode.IsDigit(c) && !(c >= 'a' && c <= 'f') {
			return fmt.Errorf("session ID must contain only lowercase hex chars [a-f0-9], got %q", id)
		}
	}
	return nil
}
