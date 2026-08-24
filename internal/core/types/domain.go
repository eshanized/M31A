package types

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type Project struct {
	ID           uuid.UUID     `json:"id"`
	RootPath     string        `json:"root_path"`
	Name         string        `json:"name"`
	Repositories []Repository  `json:"repositories"`
	Stack        DetectedStack `json:"stack"`
	Config       ProjectConfig `json:"config"`
	CreatedAt    time.Time     `json:"created_at"`
	UpdatedAt    time.Time     `json:"updated_at"`
}

type Repository struct {
	ID            uuid.UUID `json:"id"`
	RootPath      string    `json:"root_path"`
	Remotes       []Remote  `json:"remotes"`
	DefaultBranch string    `json:"default_branch"`
}

type Remote struct {
	Name string `json:"name"`
	URL  string `json:"url"`
}

type DetectedStack struct {
	Language   string   `json:"language"`
	Framework  string   `json:"framework"`
	BuildTool  string   `json:"build_tool"`
	TestRunner string   `json:"test_runner"`
	PackageMgr string   `json:"package_manager"`
	Files      []string `json:"files"`
}

type ProjectConfig struct {
	Provider     string   `json:"provider"`
	Model        string   `json:"model"`
	MaxParallel  int      `json:"max_parallel"`
	Permission   string   `json:"permission"`
	FeatureFlags []string `json:"feature_flags"`
}

type Workspace struct {
	ID           uuid.UUID    `json:"id"`
	ProjectID    uuid.UUID    `json:"project_id"`
	Name         string       `json:"name"`
	RootPath     string       `json:"root_path"`
	Repositories []Repository `json:"repositories"`
	CreatedAt    time.Time    `json:"created_at"`
	UpdatedAt    time.Time    `json:"updated_at"`
}

type Session struct {
	ID            uuid.UUID     `json:"id"`
	ProjectID     uuid.UUID     `json:"project_id"`
	Label         string        `json:"label,omitempty"`
	Tags          []string      `json:"tags,omitempty"`
	Model         string        `json:"model"`
	Provider      string        `json:"provider"`
	StartedAt     time.Time     `json:"started_at"`
	LastEventSeq  int64         `json:"last_event_seq"`
	WorkflowPhase WorkflowPhase `json:"workflow_phase"`
}

type Run struct {
	ID          uuid.UUID  `json:"id"`
	SessionID   uuid.UUID  `json:"session_id"`
	Intent      Intent     `json:"intent"`
	PlanID      *uuid.UUID `json:"plan_id,omitempty"`
	TaskGraphID *uuid.UUID `json:"task_graph_id,omitempty"`
	Status      RunStatus  `json:"status"`
	StartedAt   time.Time  `json:"started_at"`
	CompletedAt *time.Time `json:"completed_at,omitempty"`
}

type RunStatus string

const (
	RunStatusPlanned   RunStatus = "planned"
	RunStatusRunning   RunStatus = "running"
	RunStatusPaused    RunStatus = "paused"
	RunStatusCompleted RunStatus = "completed"
	RunStatusFailed    RunStatus = "failed"
)

func (p Project) MarshalJSON() ([]byte, error) {
	if p.Repositories == nil {
		p.Repositories = []Repository{}
	}
	type projectAlias Project
	return json.Marshal(projectAlias(p))
}

func (w Workspace) MarshalJSON() ([]byte, error) {
	if w.Repositories == nil {
		w.Repositories = []Repository{}
	}
	type workspaceAlias Workspace
	return json.Marshal(workspaceAlias(w))
}

func (s Session) MarshalJSON() ([]byte, error) {
	if s.Tags == nil {
		s.Tags = []string{}
	}
	type sessionAlias Session
	return json.Marshal(sessionAlias(s))
}
