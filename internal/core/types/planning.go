package types

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type IntentType string

const (
	IntentFeature     IntentType = "feature"
	IntentBugfix      IntentType = "bugfix"
	IntentRefactor    IntentType = "refactor"
	IntentQuestion    IntentType = "question"
	IntentExplanation IntentType = "explanation"
	IntentExploration IntentType = "exploration"
	IntentChore       IntentType = "chore"
)

type ComplexityLevel string

const (
	ComplexityTrivial  ComplexityLevel = "trivial"
	ComplexitySimple   ComplexityLevel = "simple"
	ComplexityModerate ComplexityLevel = "moderate"
	ComplexityComplex  ComplexityLevel = "complex"
)

type Intent struct {
	Type       IntentType      `json:"type"`
	Complexity ComplexityLevel `json:"complexity"`
	Summary    string          `json:"summary"`
	RawPrompt  string          `json:"raw_prompt"`
	Confidence float64         `json:"confidence"`
}

type Requirement struct {
	ID           uuid.UUID         `json:"id"`
	Title        string            `json:"title"`
	Description  string            `json:"description"`
	Phase        int               `json:"phase"`
	Status       RequirementStatus `json:"status"`
	Traceability []string          `json:"traceability"`
	CreatedAt    time.Time         `json:"created_at"`
	UpdatedAt    time.Time         `json:"updated_at"`
}

type RequirementStatus string

const (
	RequirementStatusPending   RequirementStatus = "pending"
	RequirementStatusActive    RequirementStatus = "active"
	RequirementStatusCompleted RequirementStatus = "completed"
	RequirementStatusDeferred  RequirementStatus = "deferred"
	RequirementStatusDropped   RequirementStatus = "dropped"
)

type Decision struct {
	ID           uuid.UUID      `json:"id"`
	Title        string         `json:"title"`
	Status       DecisionStatus `json:"status"`
	Rationale    string         `json:"rationale"`
	Alternatives []Alternative  `json:"alternatives,omitempty"`
	Timestamp    time.Time      `json:"timestamp"`
}

type DecisionStatus string

const (
	DecisionStatusProposed   DecisionStatus = "proposed"
	DecisionStatusAccepted   DecisionStatus = "accepted"
	DecisionStatusRejected   DecisionStatus = "rejected"
	DecisionStatusSuperseded DecisionStatus = "superseded"
)

type Alternative struct {
	Description string   `json:"description"`
	Pros        []string `json:"pros"`
	Cons        []string `json:"cons"`
}

type Plan struct {
	ID           uuid.UUID   `json:"id"`
	Title        string      `json:"title"`
	Objective    string      `json:"objective"`
	Requirements []uuid.UUID `json:"requirements"`
	Tasks        []Task      `json:"tasks"`
	RiskLevel    RiskLevel   `json:"risk_level"`
	Status       PlanStatus  `json:"status"`
	CreatedAt    time.Time   `json:"created_at"`
	UpdatedAt    time.Time   `json:"updated_at"`
}

type PlanStatus string

const (
	PlanStatusDraft     PlanStatus = "draft"
	PlanStatusApproved  PlanStatus = "approved"
	PlanStatusExecuting PlanStatus = "executing"
	PlanStatusCompleted PlanStatus = "completed"
	PlanStatusFailed    PlanStatus = "failed"
)

type Task struct {
	ID                 int        `json:"id"`
	Description        string     `json:"description"`
	Action             string     `json:"action"`
	Category           string     `json:"category,omitempty"`
	PlanSection        string     `json:"plan_section,omitempty"`
	Dependencies       []int      `json:"dependencies"`
	Files              []string   `json:"files"`
	AcceptanceCriteria []string   `json:"acceptance_criteria"`
	Status             TaskStatus `json:"status"`
	HealsAttempted     int        `json:"heals_attempted"`
	CommitHash         string     `json:"commit_hash,omitempty"`
}

type TaskStatus string

const (
	TaskStatusPending       TaskStatus = "pending"
	TaskStatusRunning       TaskStatus = "running"
	TaskStatusDone          TaskStatus = "done"
	TaskStatusFailed        TaskStatus = "failed"
	TaskStatusSkipped       TaskStatus = "skipped"
	TaskStatusUnrecoverable TaskStatus = "unrecoverable"
)

type TaskGraph struct {
	ID        uuid.UUID       `json:"id"`
	PlanID    uuid.UUID       `json:"plan_id"`
	Tasks     []Task          `json:"tasks"`
	Status    TaskGraphStatus `json:"status"`
	CreatedAt time.Time       `json:"created_at"`
	UpdatedAt time.Time       `json:"updated_at"`
}

type TaskGraphStatus string

const (
	TaskGraphStatusPlanned   TaskGraphStatus = "planned"
	TaskGraphStatusValidated TaskGraphStatus = "validated"
	TaskGraphStatusExecuting TaskGraphStatus = "executing"
	TaskGraphStatusCompleted TaskGraphStatus = "completed"
	TaskGraphStatusFailed    TaskGraphStatus = "failed"
)

func (r Requirement) MarshalJSON() ([]byte, error) {
	if r.Traceability == nil {
		r.Traceability = []string{}
	}
	type requirementAlias Requirement
	return json.Marshal(requirementAlias(r))
}

func (d Decision) MarshalJSON() ([]byte, error) {
	if d.Alternatives == nil {
		d.Alternatives = []Alternative{}
	}
	type decisionAlias Decision
	return json.Marshal(decisionAlias(d))
}

func (p Plan) MarshalJSON() ([]byte, error) {
	if p.Requirements == nil {
		p.Requirements = []uuid.UUID{}
	}
	if p.Tasks == nil {
		p.Tasks = []Task{}
	}
	type planAlias Plan
	return json.Marshal(planAlias(p))
}

func (t Task) MarshalJSON() ([]byte, error) {
	if t.Dependencies == nil {
		t.Dependencies = []int{}
	}
	if t.Files == nil {
		t.Files = []string{}
	}
	if t.AcceptanceCriteria == nil {
		t.AcceptanceCriteria = []string{}
	}
	type taskAlias Task
	return json.Marshal(taskAlias(t))
}

func (tg TaskGraph) MarshalJSON() ([]byte, error) {
	if tg.Tasks == nil {
		tg.Tasks = []Task{}
	}
	type taskGraphAlias TaskGraph
	return json.Marshal(taskGraphAlias(tg))
}
