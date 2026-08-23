package types

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type Event struct {
	ID        uuid.UUID       `json:"id"`
	Seq       int64           `json:"seq"`
	Type      EventType       `json:"type"`
	Timestamp time.Time       `json:"timestamp"`
	RunID     *uuid.UUID      `json:"run_id,omitempty"`
	SessionID *uuid.UUID      `json:"session_id,omitempty"`
	Payload   json.RawMessage `json:"payload"`
	Metadata  EventMetadata   `json:"metadata,omitempty"`
}

type EventType string

const (
	EventProjectInitialized    EventType = "ProjectInitialized"
	EventRepositoryIndexed     EventType = "RepositoryIndexed"
	EventSessionCreated        EventType = "SessionCreated"
	EventRunCreated            EventType = "RunCreated"
	EventIntentAccepted        EventType = "IntentAccepted"
	EventRequirementCreated    EventType = "RequirementCreated"
	EventRequirementUpdated    EventType = "RequirementUpdated"
	EventRequirementDeleted    EventType = "RequirementDeleted"
	EventDecisionLogged        EventType = "DecisionLogged"
	EventDecisionUpdated       EventType = "DecisionUpdated"
	EventPlanCreated           EventType = "PlanCreated"
	EventPlanUpdated           EventType = "PlanUpdated"
	EventTaskCreated           EventType = "TaskCreated"
	EventTaskUpdated           EventType = "TaskUpdated"
	EventAgentStarted          EventType = "AgentStarted"
	EventAgentCompleted        EventType = "AgentCompleted"
	EventToolCallRequested     EventType = "ToolCallRequested"
	EventToolCallCompleted     EventType = "ToolCallCompleted"
	EventFileChanged           EventType = "FileChanged"
	EventCheckpointRequested   EventType = "CheckpointRequested"
	EventCheckpointResolved    EventType = "CheckpointResolved"
	EventVerificationStarted   EventType = "VerificationStarted"
	EventVerificationCompleted EventType = "VerificationCompleted"
	EventTaskCompleted         EventType = "TaskCompleted"
	EventRunCompleted          EventType = "RunCompleted"
	EventRunFailed             EventType = "RunFailed"
	EventMigrationStarted      EventType = "MigrationStarted"
	EventMigrationCompleted    EventType = "MigrationCompleted"
	EventConfigChanged         EventType = "ConfigChanged"
	EventProjectUpdated        EventType = "ProjectUpdated"
	EventRepositoryUpdated     EventType = "RepositoryUpdated"
	EventWorkspaceCreated      EventType = "WorkspaceCreated"
	EventWorkspaceUpdated      EventType = "WorkspaceUpdated"
	EventArtifactCreated       EventType = "ArtifactCreated"
	EventArtifactUpdated       EventType = "ArtifactUpdated"
	EventResearchCompleted     EventType = "ResearchCompleted"
	EventProjectionRebuilt     EventType = "ProjectionRebuilt"
	EventBackupCompleted       EventType = "BackupCompleted"
)

type EventMetadata struct {
	SchemaVersion int      `json:"schema_version"`
	Tags          []string `json:"tags,omitempty"`
	Source        string   `json:"source,omitempty"`
}

type Query struct {
	RunID     *uuid.UUID `json:"run_id,omitempty"`
	SessionID *uuid.UUID `json:"session_id,omitempty"`
	Type      *EventType `json:"type,omitempty"`
	AfterSeq  int64      `json:"after_seq,omitempty"`
	BeforeSeq int64      `json:"before_seq,omitempty"`
	Limit     int        `json:"limit,omitempty"`
	Offset    int        `json:"offset,omitempty"`
}

func (e Event) MarshalJSON() ([]byte, error) {
	if e.Payload == nil {
		e.Payload = json.RawMessage("{}")
	}
	type eventAlias Event
	return json.Marshal(eventAlias(e))
}

func (m EventMetadata) MarshalJSON() ([]byte, error) {
	if m.Tags == nil {
		m.Tags = []string{}
	}
	type metadataAlias EventMetadata
	return json.Marshal(metadataAlias(m))
}

func (q Query) MarshalJSON() ([]byte, error) {
	type queryAlias Query
	return json.Marshal(queryAlias(q))
}