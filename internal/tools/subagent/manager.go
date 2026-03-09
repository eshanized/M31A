package subagent

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"log/slog"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

const (
	// MaxConcurrent is the maximum number of subagents running at once.
	MaxConcurrent = 8
	// eventBuffer is the capacity of the outbound event channel.
	eventBuffer = 256
	// DefaultMaxTools is the per-subagent tool-call budget.
	DefaultMaxTools = 50
	// DefaultMaxTokens is the per-subagent token budget (input + output).
	DefaultMaxTokens = 50_000
)

// ErrMaxConcurrent is returned when Spawn cannot acquire a slot.
var ErrMaxConcurrent = errors.New("subagent: max concurrent agents reached")

// WorktreeOps abstracts git worktree operations so the manager can be tested
// without a real git repo. The production implementation lives in
// internal/tools/subagent/worktree.go.
type WorktreeOps interface {
	Create(ctx context.Context, parentWorkDir, agentID, branchSuffix string) (path string, err error)
	Remove(ctx context.Context, path string) error
	IsRepo(parentWorkDir string) bool
}

// Dependencies are shared resources the Manager needs.
type Dependencies struct {
	WorkDir       string
	Registry      *provider.Registry
	ActiveModel   *types.ModelInfo
	Logger        *slog.Logger
	Worktrees     WorktreeOps       // optional; nil = always use IsolationDefault
	NewDispatcher DispatcherFactory // required: builds a dispatcher per workspace
}

// Manager orchestrates subagent lifecycles.
type Manager struct {
	deps    Dependencies
	sem     chan struct{}
	agents  sync.Map // id -> *Subagent
	eventCh chan SubagentEvent
	spawnMu sync.Mutex
}

// Subagent is the live handle for a running child agent.
type Subagent struct {
	Info   SubagentInfo
	cancel context.CancelFunc
	done   chan struct{} // closed when the loop exits
	req    SpawnRequest
	mu     sync.Mutex // protects Info mutations from the loop goroutine
}

// NewManager creates a Manager ready to accept spawns.
func NewManager(deps Dependencies) *Manager {
	if deps.Logger == nil {
		deps.Logger = slog.Default()
	}
	return &Manager{
		deps:    deps,
		sem:     make(chan struct{}, MaxConcurrent),
		eventCh: make(chan SubagentEvent, eventBuffer),
	}
}

// Events returns the read-only event stream consumed by the TUI or tests.
func (m *Manager) Events() <-chan SubagentEvent { return m.eventCh }

// Spawn starts a new subagent and returns its ID. If req.Background is
// false, Spawn blocks until the subagent finishes.
func (m *Manager) Spawn(parentCtx context.Context, req SpawnRequest) (string, *Subagent, error) {
	if req.Description == "" {
		return "", nil, errors.New("subagent: description is required")
	}
	if req.Prompt == "" {
		return "", nil, errors.New("subagent: prompt is required")
	}
	if req.Isolation == "" {
		req.Isolation = IsolationDefault
	}
	if req.MaxTools <= 0 {
		req.MaxTools = DefaultMaxTools
	}
	if req.MaxTokens <= 0 {
		req.MaxTokens = DefaultMaxTokens
	}

	select {
	case m.sem <- struct{}{}:
	default:
		return "", nil, ErrMaxConcurrent
	}

	id, err := newAgentID()
	if err != nil {
		<-m.sem
		return "", nil, errors.New("subagent: generate id: " + err.Error())
	}

	modelID := req.ModelID
	if modelID == "" && m.deps.ActiveModel != nil {
		modelID = m.deps.ActiveModel.ID
	}
	if modelID == "" {
		<-m.sem
		return "", nil, errors.New("subagent: no model configured")
	}

	worktree := m.deps.WorkDir
	if req.Isolation == IsolationWorktree && m.deps.Worktrees != nil && m.deps.Worktrees.IsRepo(m.deps.WorkDir) {
		m.spawnMu.Lock()
		path, werr := m.deps.Worktrees.Create(parentCtx, m.deps.WorkDir, id, req.Name)
		m.spawnMu.Unlock()
		if werr != nil {
			<-m.sem
			return "", nil, errors.New("subagent: worktree: " + werr.Error())
		}
		worktree = path
	}

	ctx, cancel := context.WithCancel(parentCtx)
	sa := &Subagent{
		Info: SubagentInfo{
			ID:          id,
			Name:        req.Name,
			Description: req.Description,
			Status:      StatusRunning,
			Isolation:   req.Isolation,
			Worktree:    worktree,
			ModelID:     modelID,
			StartedAt:   time.Now(),
		},
		cancel: cancel,
		done:   make(chan struct{}),
		req:    req,
	}
	m.agents.Store(id, sa)

	m.emit(SubagentEvent{
		Type:      EventSpawned,
		AgentID:   id,
		Name:      req.Name,
		Timestamp: time.Now(),
		Worktree:  worktree,
	})

	go m.runLoop(ctx, sa)

	if !req.Background {
		<-sa.done
	}
	return id, sa, nil
}

// Get returns a subagent by ID, or nil if unknown.
func (m *Manager) Get(id string) *Subagent {
	if v, ok := m.agents.Load(id); ok {
		return v.(*Subagent)
	}
	return nil
}

// List returns a snapshot of every known subagent (any status).
func (m *Manager) List() []SubagentInfo {
	var out []SubagentInfo
	m.agents.Range(func(_, value any) bool {
		sa := value.(*Subagent)
		sa.mu.Lock()
		out = append(out, sa.Info)
		sa.mu.Unlock()
		return true
	})
	return out
}

// Cancel requests cancellation of a specific subagent.
func (m *Manager) Cancel(id string) {
	if sa := m.Get(id); sa != nil {
		sa.cancel()
	}
}

// CancelAll requests cancellation of every running subagent.
func (m *Manager) CancelAll() {
	m.agents.Range(func(_, value any) bool {
		value.(*Subagent).cancel()
		return true
	})
}

// Cleanup removes a subagent's workspace (best-effort) and forgets it.
func (m *Manager) Cleanup(ctx context.Context, id string) error {
	sa := m.Get(id)
	if sa == nil {
		return nil
	}
	if sa.Info.Isolation == IsolationWorktree &&
		sa.Info.Worktree != "" && sa.Info.Worktree != m.deps.WorkDir &&
		m.deps.Worktrees != nil {
		if err := m.deps.Worktrees.Remove(ctx, sa.Info.Worktree); err != nil {
			m.deps.Logger.Warn("subagent: worktree cleanup failed", "id", id, "err", err)
		}
	}
	m.agents.Delete(id)
	return nil
}

// Shutdown cancels all subagents, waits for completion, and closes the event
// channel. Safe to call once at application exit.
func (m *Manager) Shutdown(ctx context.Context) {
	m.CancelAll()
	m.agents.Range(func(_, value any) bool {
		<-value.(*Subagent).done
		return true
	})
	close(m.eventCh)
}

// emit delivers an event to the outbound channel. Lifecycle events block
// briefly; verbose deltas are dropped when the channel is saturated.
func (m *Manager) emit(ev SubagentEvent) {
	if ev.Timestamp.IsZero() {
		ev.Timestamp = time.Now()
	}
	switch ev.Type {
	case EventDone, EventError, EventCancelled, EventSpawned:
		select {
		case m.eventCh <- ev:
		case <-time.After(500 * time.Millisecond):
			m.deps.Logger.Warn("subagent: lifecycle event dropped", "type", ev.Type, "id", ev.AgentID)
		}
	default:
		select {
		case m.eventCh <- ev:
		default:
		}
	}
}

func (m *Manager) runLoop(ctx context.Context, sa *Subagent) {
	defer close(sa.done)
	defer func() { <-m.sem }()

	if m.deps.NewDispatcher == nil {
		m.failAgent(sa, errors.New("subagent: no dispatcher factory configured"))
		return
	}
	dispatcher, err := m.deps.NewDispatcher(sa.Info.Worktree)
	if err != nil {
		m.failAgent(sa, errors.New("dispatcher: "+err.Error()))
		return
	}
	defer dispatcher.Stop()

	l := &loop{
		manager:    m,
		agent:      sa,
		dispatcher: dispatcher,
		provider:   m.resolveProvider(),
		modelID:    sa.Info.ModelID,
		maxTools:   sa.req.MaxTools,
		maxTokens:  sa.req.MaxTokens,
	}
	l.run(ctx)
}

func (m *Manager) resolveProvider() provider.LLMProvider {
	if m.deps.Registry == nil {
		return nil
	}
	return m.deps.Registry.ActiveProvider()
}

func (m *Manager) failAgent(sa *Subagent, err error) {
	sa.mu.Lock()
	sa.Info.Status = StatusError
	sa.Info.LastError = err.Error()
	sa.Info.FinishedAt = time.Now()
	sa.mu.Unlock()
	m.emit(SubagentEvent{
		Type:      EventError,
		AgentID:   sa.Info.ID,
		Name:      sa.Info.Name,
		Error:     err.Error(),
		Timestamp: time.Now(),
	})
}

func newAgentID() (string, error) {
	b := make([]byte, 3)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return hex.EncodeToString(b), nil
}
