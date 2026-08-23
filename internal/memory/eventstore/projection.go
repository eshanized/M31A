package eventstore

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"sync"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	_ "modernc.org/sqlite"
)

var (
	ErrProjectionNotFound  = errors.New("projection not found")
	ErrProjectionApplyFailed = errors.New("projection apply failed")
)

type Projection interface {
	Name() string
	Apply(event types.Event) error
	State() any
	Checkpoint() ([]byte, error)
	Restore(data []byte) error
}

type ProjectionManager struct {
	store      *SQLiteEventStore
	projections map[string]Projection
	mu         sync.RWMutex
}

func NewProjectionManager(store *SQLiteEventStore) *ProjectionManager {
	return &ProjectionManager{
		store:       store,
		projections: make(map[string]Projection),
	}
}

func (pm *ProjectionManager) Register(p Projection) {
	pm.mu.Lock()
	defer pm.mu.Unlock()
	pm.projections[p.Name()] = p
}

func (pm *ProjectionManager) Get(name string) (Projection, bool) {
	pm.mu.RLock()
	defer pm.mu.RUnlock()
	p, ok := pm.projections[name]
	return p, ok
}

func (pm *ProjectionManager) Rebuild(ctx context.Context, name string) error {
	pm.mu.RLock()
	p, ok := pm.projections[name]
	pm.mu.RUnlock()
	if !ok {
		return coreerrors.Wrap(ErrProjectionNotFound, "projection not found: "+name)
	}

	// Load last checkpoint
	var lastSeq int64
	err := pm.store.db.QueryRowContext(ctx,
		"SELECT last_seq FROM projections WHERE name = ?", name).Scan(&lastSeq)
	if err != nil && err != sql.ErrNoRows {
		return coreerrors.Wrap(err, "load checkpoint")
	}

	// Replay events from last_seq
	events, err := pm.store.Query(ctx, types.Query{AfterSeq: lastSeq, Limit: 1000})
	if err != nil {
		return coreerrors.Wrap(err, "query events for replay")
	}

	for _, evt := range events {
		if err := p.Apply(evt); err != nil {
			return coreerrors.Wrapf(ErrProjectionApplyFailed, "apply event seq=%d: %v", evt.Seq, err)
		}
		lastSeq = evt.Seq
	}

	// Save checkpoint
	if _, err := p.Checkpoint(); err != nil {
		return coreerrors.Wrap(err, "create checkpoint")
	}

	_, err = pm.store.db.ExecContext(ctx, `
		INSERT INTO projections (name, last_seq, updated_at)
		VALUES (?, ?, ?)
		ON CONFLICT(name) DO UPDATE SET last_seq = ?, updated_at = ?`,
		name, lastSeq, time.Now().UnixNano(), lastSeq, time.Now().UnixNano())
	if err != nil {
		return coreerrors.Wrap(err, "save checkpoint")
	}

	// Retention: keep last 10 checkpoints (per D-16)
	_, err = pm.store.db.ExecContext(ctx, `
		DELETE FROM projections 
		WHERE name = ? 
		AND updated_at NOT IN (
			SELECT updated_at FROM projections 
			WHERE name = ? 
			ORDER BY updated_at DESC LIMIT 10
		)
	`, name, name)
	return err
}

func (pm *ProjectionManager) GetState(name string) (any, bool) {
	pm.mu.RLock()
	defer pm.mu.RUnlock()
	p, ok := pm.projections[name]
	if !ok {
		return nil, false
	}
	return p.State(), true
}

// ProjectProjection handles Project, Repository, Workspace events
type ProjectProjection struct {
	state *types.Project
}

func NewProjectProjection() *ProjectProjection {
	return &ProjectProjection{state: &types.Project{}}
}

func (p *ProjectProjection) Name() string { return "project" }

func (p *ProjectProjection) Apply(event types.Event) error {
	switch event.Type {
	case types.EventProjectInitialized:
		var payload types.Project
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state = &payload
	case types.EventRepositoryIndexed:
		var payload types.Repository
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state.Repositories = append(p.state.Repositories, payload)
	case types.EventSessionCreated:
		var payload types.Session
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		// Sessions are tracked in runs, not directly in project
	case types.EventRunCreated:
		var payload types.Run
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		// Runs are tracked separately
	}
	return nil
}

func (p *ProjectProjection) State() any { return p.state }

func (p *ProjectProjection) Checkpoint() ([]byte, error) {
	return json.Marshal(p.state)
}

func (p *ProjectProjection) Restore(data []byte) error {
	p.state = &types.Project{}
	return json.Unmarshal(data, p.state)
}

// RequirementsProjection handles Requirement events
type RequirementsProjection struct {
	state map[string]*types.Requirement
}

func NewRequirementsProjection() *RequirementsProjection {
	return &RequirementsProjection{state: make(map[string]*types.Requirement)}
}

func (p *RequirementsProjection) Name() string { return "requirements" }

func (p *RequirementsProjection) Apply(event types.Event) error {
	switch event.Type {
	case types.EventRequirementCreated:
		var payload types.Requirement
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state[payload.ID.String()] = &payload
	case types.EventRequirementUpdated:
		var payload types.Requirement
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state[payload.ID.String()] = &payload
	case types.EventRequirementDeleted:
		var payload struct{ ID string }
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		delete(p.state, payload.ID)
	}
	return nil
}

func (p *RequirementsProjection) State() any { return p.state }

func (p *RequirementsProjection) Checkpoint() ([]byte, error) {
	return json.Marshal(p.state)
}

func (p *RequirementsProjection) Restore(data []byte) error {
	p.state = make(map[string]*types.Requirement)
	return json.Unmarshal(data, &p.state)
}

// DecisionsProjection handles Decision events
type DecisionsProjection struct {
	state []*types.Decision
}

func NewDecisionsProjection() *DecisionsProjection {
	return &DecisionsProjection{state: make([]*types.Decision, 0)}
}

func (p *DecisionsProjection) Name() string { return "decisions" }

func (p *DecisionsProjection) Apply(event types.Event) error {
	switch event.Type {
	case types.EventDecisionLogged:
		var payload types.Decision
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state = append(p.state, &payload)
	case types.EventDecisionUpdated:
		var payload types.Decision
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		for i, d := range p.state {
			if d.ID == payload.ID {
				p.state[i] = &payload
				return nil
			}
		}
		p.state = append(p.state, &payload)
	}
	return nil
}

func (p *DecisionsProjection) State() any { return p.state }

func (p *DecisionsProjection) Checkpoint() ([]byte, error) {
	return json.Marshal(p.state)
}

func (p *DecisionsProjection) Restore(data []byte) error {
	p.state = make([]*types.Decision, 0)
	return json.Unmarshal(data, &p.state)
}

// ResearchProjection handles Research events
type ResearchProjection struct {
	state []*types.Research
}

func NewResearchProjection() *ResearchProjection {
	return &ResearchProjection{state: make([]*types.Research, 0)}
}

func (p *ResearchProjection) Name() string { return "research" }

func (p *ResearchProjection) Apply(event types.Event) error {
	switch event.Type {
	case types.EventResearchCompleted:
		var payload types.Research
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state = append(p.state, &payload)
	}
	return nil
}

func (p *ResearchProjection) State() any { return p.state }

func (p *ResearchProjection) Checkpoint() ([]byte, error) {
	return json.Marshal(p.state)
}

func (p *ResearchProjection) Restore(data []byte) error {
	p.state = make([]*types.Research, 0)
	return json.Unmarshal(data, &p.state)
}

// RunProjection handles Run, Task, Agent, ToolCall events
type RunProjection struct {
	state map[string]*types.Run
}

func NewRunProjection() *RunProjection {
	return &RunProjection{state: make(map[string]*types.Run)}
}

func (p *RunProjection) Name() string { return "runs" }

func (p *RunProjection) Apply(event types.Event) error {
	switch event.Type {
	case types.EventRunCreated:
		var payload types.Run
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		p.state[payload.ID.String()] = &payload
	case types.EventIntentAccepted:
		// Update run with intent
	case types.EventPlanCreated:
		var payload types.Plan
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		// Plans are associated with runs
	case types.EventTaskCreated:
		var payload types.Task
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		// Tasks are part of plans
	case types.EventAgentStarted:
		var payload types.Agent
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
	case types.EventToolCallRequested:
		var payload types.ToolCall
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
return err
	}
	case types.EventFileChanged:
		// File change events are handled via projections
	case types.EventCheckpointRequested:
	case types.EventVerificationStarted:
	case types.EventTaskCompleted:
	case types.EventRunCompleted:
		var payload struct{ ID string }
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		if run, ok := p.state[payload.ID]; ok {
			run.Status = types.RunStatusCompleted
			now := time.Now()
			run.CompletedAt = &now
		}
	case types.EventRunFailed:
		var payload struct{ ID string }
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		if run, ok := p.state[payload.ID]; ok {
			run.Status = types.RunStatusFailed
			now := time.Now()
			run.CompletedAt = &now
		}
	}
	return nil
}

func (p *RunProjection) State() any { return p.state }

func (p *RunProjection) Checkpoint() ([]byte, error) {
	return json.Marshal(p.state)
}

func (p *RunProjection) Restore(data []byte) error {
	p.state = make(map[string]*types.Run)
	return json.Unmarshal(data, &p.state)
}