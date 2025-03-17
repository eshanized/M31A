package session

import (
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// Session wraps types.Session with additional runtime state fields.
type Session struct {
	types.Session
	Messages []types.Message     `json:"messages"`
	Tasks    []types.Task        `json:"tasks"`
	Project  *types.ProjectState `json:"project"`
}

// NewSession creates a new Session with default values.
func NewSession(id, model, provider string) *Session {
	return &Session{
		Session: types.Session{
			ID:            id,
			Model:         model,
			Provider:      provider,
			StartedAt:     time.Now(),
			MessageCount:  0,
			WorkflowPhase: types.PhaseIdle,
		},
		Messages: make([]types.Message, 0),
		Tasks:    make([]types.Task, 0),
	}
}

// SetPhase updates the workflow phase on the session.
func (s *Session) SetPhase(phase types.WorkflowPhase) {
	s.WorkflowPhase = phase
}
