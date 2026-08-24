package types

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type ArtifactType string

const (
	ArtifactTypeDocument ArtifactType = "document"
	ArtifactTypeCode     ArtifactType = "code"
	ArtifactTypeTest     ArtifactType = "test"
	ArtifactTypeConfig   ArtifactType = "config"
	ArtifactTypeBinary   ArtifactType = "binary"
	ArtifactTypeLog      ArtifactType = "log"
	ArtifactTypeReport   ArtifactType = "report"
	ArtifactTypeDiagram  ArtifactType = "diagram"
)

type Artifact struct {
	ID          uuid.UUID    `json:"id"`
	Name        string       `json:"name"`
	Type        ArtifactType `json:"type"`
	Path        string       `json:"path"`
	ContentHash string       `json:"content_hash"`
	Size        int64        `json:"size"`
	CreatedAt   time.Time    `json:"created_at"`
	CreatedBy   uuid.UUID    `json:"created_by"`
}

type VerificationLevel string

const (
	VerificationLevelStructural    VerificationLevel = "structural"
	VerificationLevelUnit          VerificationLevel = "unit"
	VerificationLevelIntegration   VerificationLevel = "integration"
	VerificationLevelBehavioral    VerificationLevel = "behavioral"
	VerificationLevelArchitectural VerificationLevel = "architectural"
)

type Verification struct {
	ID         uuid.UUID               `json:"id"`
	Level      VerificationLevel       `json:"level"`
	Criteria   []VerificationCriterion `json:"criteria"`
	Evidence   []Evidence              `json:"evidence"`
	Verdict    VerificationVerdict     `json:"verdict"`
	VerifiedAt *time.Time              `json:"verified_at,omitempty"`
	VerifiedBy uuid.UUID               `json:"verified_by"`
}

type VerificationCriterion struct {
	ID          string `json:"id"`
	Description string `json:"description"`
	Expected    string `json:"expected"`
	Actual      string `json:"actual,omitempty"`
	Passed      bool   `json:"passed"`
}

type Evidence struct {
	Source    string `json:"source"`
	Type      string `json:"type"`
	Reference string `json:"reference"`
	Content   string `json:"content,omitempty"`
}

type VerificationVerdict string

const (
	VerificationVerdictPassed  VerificationVerdict = "passed"
	VerificationVerdictFailed  VerificationVerdict = "failed"
	VerificationVerdictPending VerificationVerdict = "pending"
	VerificationVerdictSkipped VerificationVerdict = "skipped"
)

type CheckpointType string

const (
	CheckpointTypeHumanVerify   CheckpointType = "human_verify"
	CheckpointTypeDecision      CheckpointType = "decision"
	CheckpointTypeHumanAction   CheckpointType = "human_action"
	CheckpointTypeBlockingHuman CheckpointType = "blocking_human"
)

type Checkpoint struct {
	ID          uuid.UUID          `json:"id"`
	Type        CheckpointType     `json:"type"`
	Description string             `json:"description"`
	Risk        string             `json:"risk"`
	Reversible  bool               `json:"reversible"`
	Options     []CheckpointOption `json:"options,omitempty"`
	Status      CheckpointStatus   `json:"status"`
	CreatedAt   time.Time          `json:"created_at"`
	ResolvedAt  *time.Time         `json:"resolved_at,omitempty"`
	ResolvedBy  *uuid.UUID         `json:"resolved_by,omitempty"`
	Resolution  string             `json:"resolution,omitempty"`
}

type CheckpointOption struct {
	ID          string `json:"id"`
	Label       string `json:"label"`
	Description string `json:"description"`
	Risk        string `json:"risk"`
}

type CheckpointStatus string

const (
	CheckpointStatusPending  CheckpointStatus = "pending"
	CheckpointStatusActive   CheckpointStatus = "active"
	CheckpointStatusResolved CheckpointStatus = "resolved"
	CheckpointStatusSkipped  CheckpointStatus = "skipped"
)

type Research struct {
	ID         uuid.UUID `json:"id"`
	Question   string    `json:"question"`
	Findings   string    `json:"findings"`
	Confidence float64   `json:"confidence"`
	Sources    []string  `json:"sources"`
	CreatedAt  time.Time `json:"created_at"`
	CreatedBy  uuid.UUID `json:"created_by"`
}

func (a Artifact) MarshalJSON() ([]byte, error) {
	type artifactAlias Artifact
	return json.Marshal(artifactAlias(a))
}

func (v Verification) MarshalJSON() ([]byte, error) {
	if v.Criteria == nil {
		v.Criteria = []VerificationCriterion{}
	}
	if v.Evidence == nil {
		v.Evidence = []Evidence{}
	}
	type verificationAlias Verification
	return json.Marshal(verificationAlias(v))
}

func (c Checkpoint) MarshalJSON() ([]byte, error) {
	if c.Options == nil {
		c.Options = []CheckpointOption{}
	}
	type checkpointAlias Checkpoint
	return json.Marshal(checkpointAlias(c))
}

func (r Research) MarshalJSON() ([]byte, error) {
	if r.Sources == nil {
		r.Sources = []string{}
	}
	type researchAlias Research
	return json.Marshal(researchAlias(r))
}
