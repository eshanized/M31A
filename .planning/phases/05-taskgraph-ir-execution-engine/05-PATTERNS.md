# Phase 5: TaskGraph IR & Execution Engine - Pattern Map

**Mapped:** 2026-09-05
**Files analyzed:** 10 (new/modified)
**Analogs found:** 10 / 10

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `internal/core/types/planning.go` | model | CRUD | `internal/core/types/planning.go` (self) | exact |
| `internal/core/types/types.go` | model | CRUD | `internal/core/types/types.go` (self) | exact |
| `internal/core/types/event.go` | model | CRUD | `internal/core/types/event.go` (self) | exact |
| `internal/engine/taskrunner/runner.go` | service | request-response, event-driven | `internal/engine/taskrunner/runner.go` (self) | exact |
| `internal/engine/taskrunner/scheduler.go` | service | transform | `internal/engine/taskrunner/runner.go:76-146` (Schedule) | role-match |
| `internal/engine/taskrunner/state_machine.go` | model | CRUD | RESEARCH.md Pattern 1 + `internal/core/errors/errors.go` | design-match |
| `internal/engine/taskrunner/checkpoint.go` | service | event-driven, pub-sub | `internal/memory/eventstore/append.go` + `internal/ui/tui/app_channel.go` | pattern-match |
| `internal/engine/taskrunner/recovery.go` | service | request-response, event-driven | `internal/engine/taskrunner/runner.go:242-277` + `internal/integrations/provider/registry.go` | pattern-match |
| `internal/memory/eventstore/projection.go` | service | event-driven | `internal/memory/eventstore/projection.go` (existing projections) | exact |
| `internal/core/config/loader.go` | config | file-I/O, transform | `internal/core/config/loader.go` (existing sections) | exact |

---

## Pattern Assignments

### `internal/core/types/planning.go` (model, CRUD)

**Analog:** `internal/core/types/planning.go` (existing file — extending it)

**Imports pattern** (lines 1-8):
```go
package types

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)
```

**Type definition pattern** (lines 39-197):
```go
// All types follow this pattern:
// 1. Struct with JSON tags
// 2. Status/enum type as string constants
// 3. MarshalJSON method that initializes nil slices to empty slices

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

func (r Requirement) MarshalJSON() ([]byte, error) {
	if r.Traceability == nil {
		r.Traceability = []string{}
	}
	type requirementAlias Requirement
	return json.Marshal(requirementAlias(r))
}
```

**Plan type with methods** (lines 84-197):
```go
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

// CompileToIR() will be added here — Plan owns compilation logic (D-03)
```

---

### `internal/core/types/types.go` (model, CRUD)

**Analog:** `internal/core/types/types.go` (existing file — extending it)

**Enum definition pattern** (lines 9-16, 18-29):
```go
type RiskLevel string

const (
	RiskSafe        RiskLevel = "safe"
	RiskMedium      RiskLevel = "medium"
	RiskDangerous   RiskLevel = "dangerous"
	RiskDestructive RiskLevel = "destructive"
)

type WorkflowPhase string

const (
	PhaseIdle       WorkflowPhase = "idle"
	PhaseInitialize WorkflowPhase = "initialize"
	PhaseDiscuss    WorkflowPhase = "discuss"
	PhasePlan       WorkflowPhase = "plan"
	PhaseExecute    WorkflowPhase = "execute"
	PhaseVerify     WorkflowPhase = "verify"
	PhaseRuntime    WorkflowPhase = "runtime"
	PhaseShip       WorkflowPhase = "ship"
)
```

**Helper functions for enums** (lines 50-72):
```go
func WorkflowModeForIntent(ir IntentResult) WorkflowMode {
	switch ir.Intent {
	case IntentChore:
		return ModeDirect
	case IntentFeature, IntentBugfix, IntentRefactor:
		return WorkflowModeForIntentComplexity(ir.Complexity)
	default:
		return ModeFull
	}
}
```

**Constants for defaults** (referenced in config loader):
```go
const (
	DefaultMaxParallelTasks = 4
	BashTimeout             = 30 * time.Minute
	MaxHealAttempts         = 3
	MaxPlanRetries          = 3
)
```

---

### `internal/core/types/event.go` (model, CRUD)

**Analog:** `internal/core/types/event.go` (existing file — extending it)

**EventType enum pattern** (lines 21-77):
```go
type EventType string

const (
	EventProjectInitialized    EventType = "ProjectInitialized"
	EventSessionCreated        EventType = "SessionCreated"
	EventRunCreated            EventType = "RunCreated"
	EventTaskCreated           EventType = "TaskCreated"
	EventTaskUpdated           EventType = "TaskUpdated"
	EventCheckpointRequested   EventType = "CheckpointRequested"
	EventCheckpointResolved    EventType = "CheckpointResolved"
	EventTaskCompleted         EventType = "TaskCompleted"
	// ... new events for Phase 5 per D-16:
	// EventTaskGraphCreated, EventTaskGraphValidated, EventTaskPlanned,
	// EventTaskReady, EventTaskRunning, EventTaskWaiting, EventTaskVerifying,
	// EventTaskFailed, EventTaskRetried, EventTaskRepaired,
	// EventTaskReplanned, EventTaskEscalated, EventCheckpointApproved,
	// EventCheckpointDenied
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
```

**Payload structs follow this pattern** (new ones to add):
```go
type TaskStateChangedPayload struct {
	TaskID         int            `json:"task_id"`
	FromState      ExecutionState `json:"from_state"`
	ToState        ExecutionState `json:"to_state"`
	RunID          uuid.UUID      `json:"run_id"`
	AttemptNumber  int            `json:"attempt_number"`
	RecoveryAction *RecoveryAction `json:"recovery_action,omitempty"`
}

type CheckpointRequestedPayload struct {
	Checkpoint Checkpoint `json:"checkpoint"`
	RunID      uuid.UUID  `json:"run_id"`
}
```

---

### `internal/engine/taskrunner/runner.go` (service, request-response + event-driven)

**Analog:** `internal/engine/taskrunner/runner.go` (existing file — major extension)

**Imports pattern** (lines 1-11):
```go
package taskrunner

import (
	"context"
	"fmt"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/memory/eventstore"
)
```

**Runner struct with new fields** (lines 26-43, extended):
```go
type Runner struct {
	tasks           []types.TaskIR          // Changed from types.Task
	status          map[int]types.ExecutionState
	results         map[int]TaskResult
	idToIdx         map[int]int
	mu              sync.RWMutex
	OnTaskStart     func(task types.TaskIR)
	OnTaskUpdate    func(task types.TaskIR, state types.ExecutionState)
	TaskTimeout     time.Duration
	MaxRetries      int
	MaxParallel     int
	MaxRepairs      int           // NEW: separate from MaxRetries
	
	// NEW: EventStore for persistence
	EventStore      *eventstore.SQLiteEventStore
	RunID           uuid.UUID
	SessionID       uuid.UUID
	
	// NEW: Checkpoint approval channel (D-08)
	ApproveCh       chan CheckpointApproval
	
	// NEW: Lock manager for Task.LockNames[]
	locks           map[string]chan struct{}
	
	// NEW: Provider registry for REPAIR
	ProviderRegistry *provider.Registry
	ProviderConfig   *config.ProviderConfig
}
```

**New() constructor extended** (lines 53-71):
```go
func New(tasks []types.TaskIR, opts ...RunnerOption) *Runner {
	r := &Runner{
		tasks:        tasks,
		status:       make(map[int]types.ExecutionState),
		results:      make(map[int]TaskResult),
		idToIdx:      make(map[int]int),
		TaskTimeout:  types.BashTimeout,
		MaxRetries:   0,
		MaxParallel:  0,
		MaxRepairs:   1,  // NEW default
		locks:        make(map[string]chan struct{}),
	}
	for i, t := range tasks {
		r.idToIdx[t.ID] = i
		if t.ExecutionState == "" {
			r.status[t.ID] = types.ExecutionStatePlanned
		} else {
			r.status[t.ID] = t.ExecutionState
		}
	}
	// Apply options
	for _, opt := range opts {
		opt(r)
	}
	return r
}

// RunnerOption pattern for optional dependencies
type RunnerOption func(*Runner)

func WithEventStore(es *eventstore.SQLiteEventStore) RunnerOption {
	return func(r *Runner) { r.EventStore = es }
}

func WithRunID(id uuid.UUID) RunnerOption {
	return func(r *Runner) { r.RunID = id }
}

func WithProviderRegistry(reg *provider.Registry, cfg *config.ProviderConfig) RunnerOption {
	return func(r *Runner) { r.ProviderRegistry = reg; r.ProviderConfig = cfg }
}
```

**Kahn's algorithm Schedule()** (lines 76-146, VERIFIED):
```go
func (r *Runner) Schedule() ([][]int, error) {
	if len(r.tasks) == 0 {
		return [][]int{}, nil
	}

	// Self-reference detection
	for _, t := range r.tasks {
		for _, dep := range t.Dependencies {
			if dep == t.ID {
				return nil, fmt.Errorf("task %d: %w", t.ID, m31errors.ErrCircularDependency)
			}
		}
	}

	// Build adjacency list and in-degree count
	n := len(r.tasks)
	idSet := make(map[int]bool)
	for _, t := range r.tasks {
		idSet[t.ID] = true
	}

	inDegree := make(map[int]int)
	dependents := make(map[int][]int)

	for _, t := range r.tasks {
		if _, ok := inDegree[t.ID]; !ok {
			inDegree[t.ID] = 0
		}
		for _, dep := range t.Dependencies {
			if !idSet[dep] {
				return nil, fmt.Errorf("task %d: references non-existent dependency %d", t.ID, dep)
			}
			dependents[dep] = append(dependents[dep], t.ID)
			inDegree[t.ID]++
		}
	}

	// Kahn's algorithm
	var groups [][]int
	var queue []int

	for _, t := range r.tasks {
		if inDegree[t.ID] == 0 {
			queue = append(queue, t.ID)
		}
	}

	processed := 0
	for len(queue) > 0 {
		groups = append(groups, queue)
		processed += len(queue)

		var nextQueue []int
		for _, id := range queue {
			for _, depID := range dependents[id] {
				inDegree[depID]--
				if inDegree[depID] == 0 {
					nextQueue = append(nextQueue, depID)
				}
			}
		}
		queue = nextQueue
	}

	if processed < n {
		return nil, m31errors.ErrCircularDependency
	}

	return groups, nil
}
```

**ExecuteGroup with bounded parallelism** (lines 148-303, VERIFIED):
```go
func (r *Runner) ExecuteGroup(ctx context.Context, group []int, fn ExecuteFunc) error {
	// Pre-filter: skip tasks already in terminal state or with failed dependencies
	type readyTask struct {
		idx  int
		task types.TaskIR
	}
	var ready []readyTask
	for _, id := range group {
		idx, ok := r.idToIdx[id]
		if !ok {
			return fmt.Errorf("task %d: not found in runner", id)
		}
		task := r.tasks[idx]

		r.mu.Lock()
		curStatus := r.status[task.ID]
		// NEW: Check terminal states via ExecutionState.IsTerminal()
		if curStatus.IsTerminal() {
			r.mu.Unlock()
			continue
		}

		allDepsOK := true
		for _, depID := range task.Dependencies {
			depStatus := r.status[depID]
			if depStatus == types.ExecutionStateFailed || depStatus == types.ExecutionStateSkipped {
				// Use TransitionTo for state changes (emits event)
				_ = r.transitionTaskLocked(task.ID, types.ExecutionStateSkipped, 
					fmt.Sprintf("dependency %d failed/skipped", depID))
				allDepsOK = false
				break
			}
			if depStatus != types.ExecutionStateCompleted {
				_ = r.transitionTaskLocked(task.ID, types.ExecutionStateSkipped,
					fmt.Sprintf("dependency %d not completed", depID))
				allDepsOK = false
				break
			}
		}
		r.mu.Unlock()

		if allDepsOK {
			ready = append(ready, readyTask{idx: idx, task: task})
		}
	}

	if len(ready) == 0 {
		return nil
	}

	// Execute tasks concurrently with bounded parallelism
	var wg sync.WaitGroup
	sem := make(chan struct{}, r.maxParallel())

	for _, rt := range ready {
		select {
		case <-ctx.Done():
			wg.Wait()
			return ctx.Err()
		default:
		}

		task := rt.task
		idx := rt.idx

		wg.Add(1)
		sem <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-sem }()

			select {
			case <-ctx.Done():
				return
			default:
			}

			// Transition to RUNNING via validated state machine
			if err := r.transitionTask(task.ID, types.ExecutionStateRunning); err != nil {
				return
			}
			if r.OnTaskStart != nil {
				r.OnTaskStart(task)
			}

			var result TaskResult
			maxAttempts := r.MaxRetries + 1
			for attempt := 0; attempt < maxAttempts; attempt++ {
				var cancel context.CancelFunc
				var taskCtx context.Context
				if r.TaskTimeout > 0 {
					taskCtx, cancel = context.WithTimeout(ctx, r.TaskTimeout)
				} else {
					taskCtx, cancel = context.WithCancel(ctx)
				}
				result = fn(taskCtx, task)

				if result.Success || attempt == maxAttempts-1 {
					cancel()
					break
				}

				cancel()
				backoff := time.Duration(attempt+1) * time.Second
				timer := time.NewTimer(backoff)
				select {
				case <-timer.C:
				case <-ctx.Done():
					timer.Stop()
					cancel()
					_ = r.transitionTask(task.ID, types.ExecutionStateFailed, ctx.Err().Error())
					return
				}
				cancel()
			}

			r.mu.Lock()
			r.results[task.ID] = result
			if result.Success {
				_ = r.transitionTaskLocked(task.ID, types.ExecutionStateVerifying)
				if r.OnTaskUpdate != nil {
					r.OnTaskUpdate(task, "verifying")
				}
			} else {
				_ = r.transitionTaskLocked(task.ID, types.ExecutionStateFailed)
				if r.OnTaskUpdate != nil {
					r.OnTaskUpdate(task, "failed")
				}
			}
			r.mu.Unlock()
		}()
	}

	wg.Wait()
	return nil
}
```

**NEW: transitionTask helper with EventStore emission** (new method):
```go
func (r *Runner) transitionTask(taskID int, nextState types.ExecutionState, errMsg ...string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.transitionTaskLocked(taskID, nextState, errMsg...)
}

func (r *Runner) transitionTaskLocked(taskID int, nextState types.ExecutionState, errMsg ...string) error {
	currentState := r.status[taskID]
	if err := currentState.TransitionTo(nextState); err != nil {
		return err
	}
	r.status[taskID] = nextState

	// Emit event to EventStore if available
	if r.EventStore != nil {
		payload := types.TaskStateChangedPayload{
			TaskID:        taskID,
			FromState:     currentState,
			ToState:       nextState,
			RunID:         r.RunID,
			AttemptNumber: r.getAttemptNumber(taskID), // track per-task
		}
		if len(errMsg) > 0 {
			payload.Error = errMsg[0]
		}
		evt := types.Event{
			Type:      types.EventTaskStateChanged,
			RunID:     &r.RunID,
			SessionID: &r.SessionID,
			Payload:   mustMarshal(payload),
		}
		r.EventStore.Append(context.Background(), evt)
	}

	// Emit to TUI via MsgEmitter (if callback set)
	if r.OnTaskUpdate != nil {
		// Find task and emit
		for _, t := range r.tasks {
			if t.ID == taskID {
				r.OnTaskUpdate(t, string(nextState))
				break
			}
		}
	}
	return nil
}
```

---

### `internal/engine/taskrunner/scheduler.go` (service, transform)

**Analog:** `internal/engine/taskrunner/runner.go:76-146` (Schedule method)

**Imports pattern:**
```go
package taskrunner

import (
	"fmt"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)
```

**Schedule function extracted** (from runner.go lines 76-146):
```go
// Schedule performs topological sort using Kahn's algorithm and returns execution waves.
// Each wave contains task IDs that can run in parallel (dependencies satisfied).
func Schedule(tasks []types.TaskIR) ([][]int, error) {
	if len(tasks) == 0 {
		return [][]int{}, nil
	}

	// Self-reference detection
	for _, t := range tasks {
		for _, dep := range t.Dependencies {
			if dep == t.ID {
				return nil, fmt.Errorf("task %d: %w", t.ID, m31errors.ErrCircularDependency)
			}
		}
	}

	// Build adjacency list and in-degree count
	n := len(tasks)
	idSet := make(map[int]bool)
	for _, t := range tasks {
		idSet[t.ID] = true
	}

	inDegree := make(map[int]int)
	dependents := make(map[int][]int)

	for _, t := range tasks {
		if _, ok := inDegree[t.ID]; !ok {
			inDegree[t.ID] = 0
		}
		for _, dep := range t.Dependencies {
			if !idSet[dep] {
				return nil, fmt.Errorf("task %d: references non-existent dependency %d", t.ID, dep)
			}
			dependents[dep] = append(dependents[dep], t.ID)
			inDegree[t.ID]++
		}
	}

	// Kahn's algorithm
	var waves [][]int
	var queue []int

	for _, t := range tasks {
		if inDegree[t.ID] == 0 {
			queue = append(queue, t.ID)
		}
	}

	processed := 0
	for len(queue) > 0 {
		waves = append(waves, queue)
		processed += len(queue)

		var nextQueue []int
		for _, id := range queue {
			for _, depID := range dependents[id] {
				inDegree[depID]--
				if inDegree[depID] == 0 {
					nextQueue = append(nextQueue, depID)
				}
			}
		}
		queue = nextQueue
	}

	if processed < n {
		return nil, m31errors.ErrCircularDependency
	}

	return waves, nil
}
```

**Wave struct for TaskGraphIR** (from RESEARCH.md):
```go
type Wave struct {
	TaskIDs []int
	Index   int
}
```

---

### `internal/engine/taskrunner/state_machine.go` (model, CRUD)

**Analog:** RESEARCH.md Pattern 1 (ExecutionState design) + `internal/core/errors/errors.go` error wrapping

**Imports pattern:**
```go
package taskrunner

import (
	"fmt"

	"github.com/eshanized/M31A/internal/core/types"
)
```

**ExecutionState with validated transitions** (RESEARCH.md lines 231-271):
```go
// ExecutionState represents the validated execution state of a task.
// Transitions are enforced via TransitionTo() method.
type ExecutionState string

const (
	ExecutionStatePlanned    ExecutionState = "PLANNED"
	ExecutionStateReady      ExecutionState = "READY"
	ExecutionStateRunning    ExecutionState = "RUNNING"
	ExecutionStateWaiting    ExecutionState = "WAITING"
	ExecutionStateVerifying  ExecutionState = "VERIFYING"
	ExecutionStateCompleted  ExecutionState = "COMPLETED"
	ExecutionStateFailed     ExecutionState = "FAILED"
	ExecutionStateSkipped    ExecutionState = "SKIPPED"  // for blocked dependents
)

// validTransitions defines the allowed state graph.
// FAILED can only transition to READY via RecoveryAction (handled by Runner).
var validTransitions = map[ExecutionState][]ExecutionState{
	ExecutionStatePlanned:   {ExecutionStateReady},
	ExecutionStateReady:     {ExecutionStateRunning, ExecutionStateWaiting, ExecutionStateSkipped},
	ExecutionStateRunning:   {ExecutionStateWaiting, ExecutionStateVerifying, ExecutionStateFailed},
	ExecutionStateWaiting:   {ExecutionStateRunning, ExecutionStateVerifying, ExecutionStateFailed},
	ExecutionStateVerifying: {ExecutionStateCompleted, ExecutionStateFailed},
	ExecutionStateCompleted: {},  // terminal
	ExecutionStateFailed:    {ExecutionStateReady},  // only via RecoveryAction.RETRY/REPAIR
	ExecutionStateSkipped:   {},  // terminal
}

// TransitionTo validates and performs a state transition.
// Returns error if transition is not allowed.
func (s ExecutionState) TransitionTo(next ExecutionState) error {
	allowed, ok := validTransitions[s]
	if !ok {
		return fmt.Errorf("unknown state %s", s)
	}
	for _, a := range allowed {
		if a == next {
			return nil
		}
	}
	return fmt.Errorf("invalid transition: %s → %s", s, next)
}

func (s ExecutionState) IsTerminal() bool {
	return s == ExecutionStateCompleted || s == ExecutionStateFailed || s == ExecutionStateSkipped
}

func (s ExecutionState) IsActive() bool {
	return s == ExecutionStateRunning || s == ExecutionStateWaiting || s == ExecutionStateVerifying
}
```

**RecoveryAction enum** (from CONTEXT.md D-06, D-10):
```go
type RecoveryAction string

const (
	RecoveryActionRetry   RecoveryAction = "RETRY"
	RecoveryActionRepair  RecoveryAction = "REPAIR"
	RecoveryActionReplan  RecoveryAction = "REPLAN"
	RecoveryActionEscalate RecoveryAction = "ESCALATE"
)

// RecoveryAction methods
func (a RecoveryAction) IsValid() bool {
	switch a {
	case RecoveryActionRetry, RecoveryActionRepair, RecoveryActionReplan, RecoveryActionEscalate:
		return true
	default:
		return false
	}
}
```

**CheckpointType enum** (from CONTEXT.md D-07):
```go
type CheckpointType string

const (
	CheckpointTypeOneWayDoor CheckpointType = "ONE_WAY_DOOR"
	CheckpointTypeReview     CheckpointType = "REVIEW"
)
```

---

### `internal/engine/taskrunner/checkpoint.go` (service, event-driven + pub-sub)

**Analog:** `internal/memory/eventstore/append.go` (EventStore append) + `internal/ui/tui/app_channel.go` (channelEmitter pattern)

**Imports pattern:**
```go
package taskrunner

import (
	"context"
	"encoding/json"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/memory/eventstore"
	"github.com/google/uuid"
)
```

**CheckpointGate struct** (from RESEARCH.md Pattern 3, lines 354-392):
```go
type CheckpointGate struct {
	EventStore *eventstore.SQLiteEventStore
	ApproveCh  chan CheckpointApproval // in-memory, per-run
}

type CheckpointApproval struct {
	CheckpointID string
	Approved     bool
	Approver     string
	EvidenceRefs []string
}

func (g *CheckpointGate) RequestApproval(ctx context.Context, cp types.Checkpoint, runID uuid.UUID) (CheckpointApproval, error) {
	// 1. Emit CheckpointRequested event to EventStore
	evt := types.Event{
		Type:      types.EventCheckpointRequested,
		RunID:     &runID,
		Payload:   mustMarshal(types.CheckpointRequestedPayload{Checkpoint: cp, RunID: runID}),
	}
	if err := g.EventStore.Append(ctx, evt); err != nil {
		return CheckpointApproval{}, err
	}

	// 2. Block on channel (with context cancellation for timeout)
	select {
	case approval := <-g.ApproveCh:
		if approval.CheckpointID != cp.ID {
			// Correlation ID mismatch - wait for correct one
			// (In practice, use per-checkpoint channel or map)
			return g.RequestApproval(ctx, cp, runID)
		}
		// 3. Emit CheckpointApproved/Denied event
		approvedEvt := types.Event{
			Type:    types.EventCheckpointResolved,
			RunID:   &runID,
			Payload: mustMarshal(types.CheckpointResolvedPayload{CheckpointID: cp.ID, Approved: approval.Approved, EvidenceRefs: approval.EvidenceRefs}),
		}
		g.EventStore.Append(ctx, approvedEvt)
		return approval, nil
	case <-ctx.Done():
		// Emit CheckpointDenied on timeout/cancellation
		deniedEvt := types.Event{
			Type:    types.EventCheckpointResolved,
			RunID:   &runID,
			Payload: mustMarshal(types.CheckpointResolvedPayload{CheckpointID: cp.ID, Approved: false, Reason: ctx.Err().Error()}),
		}
		g.EventStore.Append(ctx, deniedEvt)
		return CheckpointApproval{}, ctx.Err()
	}
}
```

**Checkpoint struct** (from CONTEXT.md specifics, lines 146-156):
```go
type Checkpoint struct {
	ID                string            `json:"id"`
	Type              CheckpointType    `json:"type"`
	TaskIDs           []int             `json:"task_ids"`
	RequiredApprovers int               `json:"required_approvers"`
	EvidenceRefs      []string          `json:"evidence_refs"`
	Description       string            `json:"description"`
}
```

---

### `internal/engine/taskrunner/recovery.go` (service, request-response + event-driven)

**Analog:** `internal/engine/taskrunner/runner.go:242-277` (retry loop) + `internal/integrations/provider/registry.go` (provider selection)

**Imports pattern:**
```go
package taskrunner

import (
	"context"
	"fmt"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/memory/eventstore"
	"github.com/google/uuid"
)
```

**Retry strategy** (from runner.go lines 242-277, adapted):
```go
func (r *Runner) handleRetry(ctx context.Context, taskID int, task types.TaskIR, failure FailureContext) error {
	// Check retry count
	if task.RetryCount >= r.MaxRetries {
		return fmt.Errorf("max retries (%d) exceeded for task %d", r.MaxRetries, taskID)
	}

	// Increment retry count and transition back to READY
	task.RetryCount++
	if err := r.transitionTask(taskID, types.ExecutionStateReady); err != nil {
		return err
	}

	// Emit TaskRetried event
	r.emitEvent(types.EventTaskRetried, types.TaskRetriedPayload{
		TaskID:       taskID,
		RetryCount:   task.RetryCount,
		RunID:        r.RunID,
		FailureError: failure.Error,
	})
	return nil
}
```

**REPAIR strategy** (from RESEARCH.md lines 580-611):
```go
func (r *Runner) handleRepair(ctx context.Context, taskID int, task types.TaskIR, failure FailureContext) error {
	// Check repair count
	if task.RepairCount >= r.MaxRepairs {
		return fmt.Errorf("max repairs (%d) exceeded for task %d", r.MaxRepairs, taskID)
	}

	// Get provider for REPAIR
	prov, _, err := r.ProviderRegistry.GetProviderForRequest(
		types.ChatRequest{},
		r.ProviderConfig.FallbackMode,
		r.ProviderConfig.FallbackPriority,
		r.ProviderConfig.HealthCheckTimeoutSecs,
	)
	if err != nil {
		return fmt.Errorf("no provider available for repair: %w", err)
	}

	// Build repair prompt with context
	prompt := buildRepairPrompt(task, failure)
	
	resp, err := prov.ChatCompletion(ctx, types.ChatRequest{
		Model:    r.ProviderConfig.Default,
		Messages: []types.Message{{Role: "user", Content: prompt}},
	})
	if err != nil {
		return fmt.Errorf("repair LLM call failed: %w", err)
	}

	patch := extractPatch(resp.Content)
	
	// Apply patch via FileEdit tool (Phase 7)
	result := r.ToolsDispatcher.Execute(ctx, types.ToolInput{
		Name: "edit_file",
		Params: map[string]any{"path": task.Files[0], "patch": patch},
	})
	if result.Error != "" {
		return fmt.Errorf("patch apply failed: %s", result.Error)
	}

	task.RepairCount++
	// Transition to READY for retry
	return r.transitionTask(taskID, types.ExecutionStateReady)
}

func buildRepairPrompt(task types.TaskIR, failure FailureContext) string {
	return fmt.Sprintf(`Task failed. Generate a patch to fix it.

Task: %s
Action: %s
Files: %v
Error: %s
Acceptance Criteria: %v

Return a unified diff patch only.`, 
		task.Description, task.Action, task.Files, failure.Error, task.AcceptanceCriteria)
}
```

**REPLAN strategy** (from CONTEXT.md D-12):
```go
func (r *Runner) handleReplan(ctx context.Context, taskID int, task types.TaskIR, failure FailureContext) error {
	// Emit ReplanRequested event with failure context
	payload := types.ReplanRequestedPayload{
		FailedTaskID:  taskID,
		RunID:         r.RunID,
		FailureError:  failure.Error,
		FailureContext: failure,
	}
	r.emitEvent(types.EventTaskReplanned, payload)
	
	// Runner will receive new TaskGraphIR via event subscription and restart
	return nil
}
```

**ESCALATE strategy** (from CONTEXT.md D-12):
```go
func (r *Runner) handleEscalate(ctx context.Context, taskID int, task types.TaskIR, failure FailureContext) error {
	// Emit EscalationRequested event for TUI S16 screen
	payload := types.EscalationRequestedPayload{
		TaskID:        taskID,
		RunID:         r.RunID,
		FailureError:  failure.Error,
		Options:       []RecoveryAction{RecoveryActionRetry, RecoveryActionRepair, RecoveryActionReplan},
	}
	r.emitEvent(types.EventTaskEscalated, payload)
	
	// Block until human decision via channel (similar to checkpoint)
	// TUI S16 will send decision back
	return nil
}
```

---

### `internal/memory/eventstore/projection.go` (service, event-driven)

**Analog:** `internal/memory/eventstore/projection.go` (existing projections — ProjectProjection, RequirementsProjection, RunProjection)

**Imports pattern** (lines 1-14):
```go
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
```

**Projection interface** (lines 21-27):
```go
type Projection interface {
	Name() string
	Apply(event types.Event) error
	State() any
	Checkpoint() ([]byte, error)
	Restore(data []byte) error
}
```

**New TaskGraphExecutionProjection** (following RunProjection pattern, lines 299-380):
```go
// TaskGraphExecutionProjection rebuilds task execution state from events
type TaskGraphExecutionProjection struct {
	state map[string]*TaskGraphExecutionState // keyed by RunID
	mu    sync.RWMutex
}

type TaskGraphExecutionState struct {
	TaskGraphIR *types.TaskGraphIR
	TaskStates  map[int]types.ExecutionState
	RetryCounts map[int]int
	RepairCounts map[int]int
	CurrentWave int
	Checkpoints map[string]CheckpointStatus
}

type CheckpointStatus struct {
	Approved   bool
	Approver   string
	Evidence   []string
	ResolvedAt time.Time
}

func NewTaskGraphExecutionProjection() *TaskGraphExecutionProjection {
	return &TaskGraphExecutionProjection{
		state: make(map[string]*TaskGraphExecutionState),
	}
}

func (p *TaskGraphExecutionProjection) Name() string { return "taskgraph_execution" }

func (p *TaskGraphExecutionProjection) Apply(event types.Event) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	runKey := ""
	if event.RunID != nil {
		runKey = event.RunID.String()
	}
	if runKey == "" {
		return nil // not relevant
	}

	st, ok := p.state[runKey]
	if !ok {
		st = &TaskGraphExecutionState{
			TaskStates:  make(map[int]types.ExecutionState),
			RetryCounts: make(map[int]int),
			RepairCounts: make(map[int]int),
			Checkpoints: make(map[string]CheckpointStatus),
		}
		p.state[runKey] = st
	}

	switch event.Type {
	case types.EventTaskGraphCreated:
		var payload types.TaskGraphCreatedPayload
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		st.TaskGraphIR = &payload.TaskGraphIR
		// Initialize all tasks to PLANNED
		for _, task := range payload.TaskGraphIR.Tasks {
			st.TaskStates[task.ID] = types.ExecutionStatePlanned
		}

	case types.EventTaskStateChanged:
		var payload types.TaskStateChangedPayload
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		st.TaskStates[payload.TaskID] = payload.ToState
		if payload.RecoveryAction == types.RecoveryActionRetry {
			st.RetryCounts[payload.TaskID]++
		}
		if payload.RecoveryAction == types.RecoveryActionRepair {
			st.RepairCounts[payload.TaskID]++
		}

	case types.EventCheckpointResolved:
		var payload types.CheckpointResolvedPayload
		if err := json.Unmarshal(event.Payload, &payload); err != nil {
			return err
		}
		st.Checkpoints[payload.CheckpointID] = CheckpointStatus{
			Approved:   payload.Approved,
			Approver:   payload.Approver,
			Evidence:   payload.EvidenceRefs,
			ResolvedAt: time.Now(),
		}
	}
	return nil
}

func (p *TaskGraphExecutionProjection) State() any { return p.state }

func (p *TaskGraphExecutionProjection) Checkpoint() ([]byte, error) {
	return json.Marshal(p.state)
}

func (p *TaskGraphExecutionProjection) Restore(data []byte) error {
	p.state = make(map[string]*TaskGraphExecutionState)
	return json.Unmarshal(data, &p.state)
}
```

---

### `internal/core/config/loader.go` (config, file-I/O + transform)

**Analog:** `internal/core/config/loader.go` (existing file — adding [execution] section)

**Config struct extension** (following existing sections like ProviderConfig, UIConfig, etc.):
```go
// Add to Config struct in loader.go:
type Config struct {
	// ... existing fields ...
	Execution ExecutionConfig `toml:"execution"`
}

// New ExecutionConfig section (per D-13, D-18)
type ExecutionConfig struct {
	MaxParallelTasks   int    `toml:"max_parallel_tasks"`   // default 4
	TaskTimeout        string `toml:"task_timeout"`         // default "30m"
	WaveTimeout        string `toml:"wave_timeout"`         // default "0" (no limit)
	CheckpointInterval int    `toml:"checkpoint_interval"`  // default 100 (from EventStore config)
	MaxRetries         int    `toml:"max_retries"`          // default 3 (from Features.RetryMaxAttempts)
	MaxRepairs         int    `toml:"max_repairs"`          // default 1
}
```

**DefaultConfig() extension** (lines 27-246, add to default):
```go
func DefaultConfig() *Config {
	return &Config{
		// ... existing ...
		Execution: ExecutionConfig{
			MaxParallelTasks:   4,
			TaskTimeout:        "30m",
			WaveTimeout:        "0",
			CheckpointInterval: 100,
			MaxRetries:         3,
			MaxRepairs:         1,
		},
	}
}
```

**Config loading uses same layered merge** (lines 248-380, no changes needed — mergeConfig handles new section automatically).

---

## Shared Patterns

### Authentication / Authorization
*Not applicable for Phase 5 — no auth changes in this phase.*

### Error Handling
**Source:** `internal/core/errors/errors.go` (lines 156-174)

**Apply to:** All new service files (runner.go, recovery.go, checkpoint.go, scheduler.go)

```go
// Wrap wraps an error with a descriptive message, preserving the error chain.
// Returns nil if err is nil. Use this instead of fmt.Errorf("...: %w", err)
// for consistent error wrapping across the codebase.
func Wrap(err error, msg string) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", msg, err)
}

// Wrapf wraps an error with a formatted message, preserving the error chain.
func Wrapf(err error, format string, args ...any) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", fmt.Sprintf(format, args...), err)
}
```

**Sentinel errors** (lines 16-68):
```go
var (
	ErrCircularDependency = errors.New("circular dependency in task graph")
	ErrProviderUnreachable = errors.New("provider unreachable")
	ErrCheckpointNotFound  = errors.New("checkpoint not found")
	// ... new errors for Phase 5:
	ErrInvalidTransition    = errors.New("invalid state transition")
	ErrMaxRetriesExceeded   = errors.New("max retries exceeded")
	ErrMaxRepairsExceeded   = errors.New("max repairs exceeded")
	ErrCheckpointTimeout    = errors.New("checkpoint approval timeout")
	ErrNoProviderForRepair  = errors.New("no provider available for repair")
)
```

### EventStore Append Pattern
**Source:** `internal/memory/eventstore/append.go` (lines 14-79)

**Apply to:** runner.go (transitionTask), checkpoint.go (RequestApproval), recovery.go (all strategies)

```go
func (s *SQLiteEventStore) Append(ctx context.Context, events ...types.Event) error {
	return s.AppendEvents(ctx, events)
}

func (s *SQLiteEventStore) AppendEvents(ctx context.Context, events []types.Event) error {
	if len(events) == 0 {
		return nil
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return coreerrors.Wrap(err, "begin transaction")
	}
	defer tx.Rollback()

	stmt, err := tx.PrepareContext(ctx, `
		INSERT INTO events (id, type, timestamp, run_id, session_id, payload, metadata)
		VALUES (?, ?, ?, ?, ?, ?, ?)
	`)
	if err != nil {
		return coreerrors.Wrap(err, "prepare statement")
	}
	defer stmt.Close()

	for i := range events {
		if events[i].ID == (uuid.UUID{}) {
			events[i].ID = uuid.New()
		}
		if events[i].Timestamp.IsZero() {
			events[i].Timestamp = time.Now().UTC()
		}

		payload, err := json.Marshal(events[i].Payload)
		if err != nil {
			return coreerrors.Wrapf(err, "marshal payload for event %d", i)
		}
		metadata, err := json.Marshal(events[i].Metadata)
		if err != nil {
			return coreerrors.Wrapf(err, "marshal metadata for event %d", i)
		}

		var runID, sessionID any
		if events[i].RunID != nil {
			runID = events[i].RunID.String()
		}
		if events[i].SessionID != nil {
			sessionID = events[i].SessionID.String()
		}

		if _, err := stmt.ExecContext(ctx,
			events[i].ID.String(),
			string(events[i].Type),
			events[i].Timestamp.UnixNano(),
			runID,
			sessionID,
			payload,
			nullString(string(metadata)),
		); err != nil {
			return coreerrors.Wrapf(err, "exec event %d", i)
		}
	}

	if err := tx.Commit(); err != nil {
		return coreerrors.Wrap(err, "commit transaction")
	}
	return nil
}
```

### MsgEmitter / Channel Pattern (TUI Communication)
**Source:** `internal/engine/workflow/engine_messages.go` (lines 9-13) + `internal/ui/tui/app_channel.go` (lines 54-82)

**Apply to:** runner.go (OnTaskStart/OnTaskUpdate), checkpoint.go (approval channel)

```go
// MsgEmitter interface (engine_messages.go:11-13)
type MsgEmitter interface {
	Emit(msg any)
}

// channelEmitter (app_channel.go:57-82) - used by Runner to send to TUI
type channelEmitter struct {
	ch    chan tea.Msg
	drops *DropCounter
}

func (ce *channelEmitter) Emit(msg any) {
	for attempt := 0; attempt <= maxRetries; attempt++ {
		select {
		case ce.ch <- msg:
			return
		default:
			if attempt < maxRetries {
				time.Sleep(retryBackoff)
			}
		}
	}
	dropped := ce.drops.Add(1)
	slog.Warn("workflow message dropped: channel full after retries",
		"msg_type", fmt.Sprintf("%T", msg),
		"retries", maxRetries,
		"total_dropped", dropped)
}
```

**Message types for Phase 5** (add to engine_messages.go):
```go
// TaskStateChangedMsg emitted when task ExecutionState changes
type TaskStateChangedMsg struct {
	TaskID        int
	FromState     types.ExecutionState
	ToState       types.ExecutionState
	RecoveryAction *types.RecoveryAction
}

// CheckpointRequestedMsg emitted when checkpoint gate blocks
type CheckpointRequestedMsg struct {
	Checkpoint types.Checkpoint
	RunID      uuid.UUID
}

// CheckpointResolvedMsg emitted when checkpoint approved/denied
type CheckpointResolvedMsg struct {
	CheckpointID string
	Approved     bool
	Approver     string
}

// EscalationRequestedMsg emitted for S16 Failure/Recovery screen
type EscalationRequestedMsg struct {
	TaskID        int
	RunID         uuid.UUID
	FailureError  string
	Options       []types.RecoveryAction
}
```

### Config Layering Pattern
**Source:** `internal/core/config/loader.go` (lines 248-380)

**Apply to:** New `[execution]` section in config.toml

```toml
# Global ~/.m31a/config.toml
[execution]
max_parallel_tasks = 4
task_timeout = "30m"
wave_timeout = "0"
checkpoint_interval = 100
max_retries = 3
max_repairs = 1
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `internal/engine/taskrunner/state_machine.go` | model | CRUD | New ExecutionState type with TransitionTo() — no existing state machine in codebase. RESEARCH.md Pattern 1 is the design reference. |
| `internal/engine/taskrunner/checkpoint.go` | service | event-driven, pub-sub | Hybrid EventStore + channel checkpoint approval is a new pattern (D-08). Combines EventStore append (append.go) and channelEmitter (app_channel.go). |
| `internal/engine/taskrunner/recovery.go` | service | request-response, event-driven | RecoveryAction strategies (REPAIR via LLM, REPLAN via event) are new. RETRY pattern exists in runner.go:242-277. |
| `internal/engine/taskrunner/scheduler.go` | service | transform | Kahn's algorithm exists in runner.go:76-146 but is being extracted to separate file. |

---

## Metadata

**Analog search scope:** 
- `internal/core/types/` (planning.go, types.go, event.go, errors.go)
- `internal/engine/taskrunner/` (runner.go, runner_test.go)
- `internal/memory/eventstore/` (eventstore.go, projection.go, append.go)
- `internal/core/config/` (loader.go)
- `internal/integrations/provider/` (registry.go)
- `internal/engine/workflow/` (engine_messages.go)
- `internal/ui/tui/` (app_channel.go)

**Files scanned:** 15
**Pattern extraction date:** 2026-09-05