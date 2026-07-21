package subagent

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/core/types"
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
	// MaxTotalSubagents is the maximum total subagents that can be spawned
	// in a single session to prevent runaway resource consumption.
	MaxTotalSubagents = 50
	// MaxSpawnRate is the maximum number of subagents that can be spawned
	// per minute to prevent rapid resource exhaustion.
	MaxSpawnRate = 10
	// shutdownTimeout is the maximum time Shutdown waits for a subagent to
	// exit before force-cleaning it. Prevents indefinite blocking if a
	// subagent goroutine is stuck.
	shutdownTimeout = 5 * time.Second
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
	Worktrees     WorktreeOps                             // optional; nil = always use IsolationDefault
	NewDispatcher DispatcherFactory                       // required: builds a dispatcher per workspace
	Profiles      map[string]config.SubagentProfileConfig // optional: user profile overrides
}

// Manager orchestrates subagent lifecycles.
type Manager struct {
	deps         Dependencies
	sem          chan struct{}
	agents       sync.Map // id -> *Subagent
	eventCh      chan SubagentEvent
	spawnMu      sync.Mutex
	totalSpawned int32       // total subagents spawned in this session
	spawnTimes   []time.Time // timestamps of recent spawns for rate limiting
}

// Subagent is the live handle for a running child agent.
type Subagent struct {
	Info    SubagentInfo
	Profile AgentProfile // resolved agent profile
	cancel  context.CancelFunc
	done    chan struct{} // closed when the loop exits
	req     SpawnRequest
	mu      sync.Mutex // protects Info mutations from the loop goroutine
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
		req.Isolation = IsolationWorktree
	}
	if req.SubagentType == "" {
		req.SubagentType = "general"
	}

	// Check total spawn limit
	m.spawnMu.Lock()
	if m.totalSpawned >= MaxTotalSubagents {
		m.spawnMu.Unlock()
		return "", nil, fmt.Errorf("subagent: maximum total subagents (%d) reached", MaxTotalSubagents)
	}

	// Check spawn rate limit (max MaxSpawnRate per minute)
	now := time.Now()
	cutoff := now.Add(-1 * time.Minute)
	validTimes := make([]time.Time, 0, len(m.spawnTimes))
	for _, t := range m.spawnTimes {
		if t.After(cutoff) {
			validTimes = append(validTimes, t)
		}
	}
	if len(validTimes) >= MaxSpawnRate {
		m.spawnMu.Unlock()
		return "", nil, fmt.Errorf("subagent: spawn rate limit exceeded (%d per minute)", MaxSpawnRate)
	}
	m.spawnTimes = append(validTimes, now)
	m.totalSpawned++
	m.spawnMu.Unlock()

	// Resolve agent profile.
	profile, ok := ResolveProfile(req.SubagentType, m.deps.Profiles)
	if !ok {
		return "", nil, errors.New("subagent: unknown agent type: " + req.SubagentType)
	}

	// Apply profile budget overrides (request-level takes precedence).
	if req.MaxTools <= 0 {
		if profile.MaxTools > 0 {
			req.MaxTools = profile.MaxTools
		} else {
			req.MaxTools = DefaultMaxTools
		}
	}
	if req.MaxTokens <= 0 {
		if profile.MaxTokens > 0 {
			req.MaxTokens = profile.MaxTokens
		} else {
			req.MaxTokens = DefaultMaxTokens
		}
	}

	if m.resolveProvider() == nil {
		return "", nil, errors.New("subagent: no LLM provider configured")
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

	// Model: request > profile > parent active model.
	modelID := req.ModelID
	if modelID == "" && profile.Model != "" {
		modelID = profile.Model
	}
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
			// Degraded mode: log warning and emit event, continue without isolation.
			slog.Warn("subagent: worktree creation failed, running in degraded mode",
				"agent_id", id, "error", werr)
			m.emit(SubagentEvent{
				Type:      EventSpawnFailed,
				AgentID:   id,
				Name:      req.Name,
				Error:     fmt.Sprintf("worktree: %v", werr),
				Timestamp: time.Now(),
			})
		} else {
			worktree = path
		}
	}

	ctx, cancel := context.WithCancel(parentCtx)
	sa := &Subagent{
		Info: SubagentInfo{
			ID:           id,
			Name:         req.Name,
			SubagentType: req.SubagentType,
			Description:  req.Description,
			Status:       StatusRunning,
			Isolation:    req.Isolation,
			Worktree:     worktree,
			ModelID:      modelID,
			StartedAt:    time.Now(),
		},
		Profile: profile,
		cancel:  cancel,
		done:    make(chan struct{}),
		req:     req,
	}
	m.agents.Store(id, sa)

	m.emit(SubagentEvent{
		Type:         EventSpawned,
		AgentID:      id,
		Name:         req.Name,
		SubagentType: req.SubagentType,
		Timestamp:    time.Now(),
		Worktree:     worktree,
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
		if sa, ok := v.(*Subagent); ok {
			return sa
		}
	}
	return nil
}

// List returns a snapshot of every known subagent (any status).
func (m *Manager) List() []SubagentInfo {
	var out []SubagentInfo
	m.agents.Range(func(_, value any) bool {
		if sa, ok := value.(*Subagent); ok {
			sa.mu.Lock()
			out = append(out, sa.Info)
			sa.mu.Unlock()
		}
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
		if sa, ok := value.(*Subagent); ok {
			sa.cancel()
		}
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

// Shutdown cancels all subagents, waits for completion with a bounded
// timeout, cleans up their worktrees, and closes the event channel.
// Safe to call once at application exit.
func (m *Manager) Shutdown(ctx context.Context) {
	m.CancelAll()

	shutdownCtx, cancel := context.WithTimeout(ctx, shutdownTimeout)
	defer cancel()

	m.agents.Range(func(key, value any) bool {
		sa, ok := value.(*Subagent)
		if !ok {
			m.agents.Delete(key)
			return true
		}
		select {
		case <-sa.done:
			// Subagent exited within the timeout window.
		case <-shutdownCtx.Done():
			// Timeout exceeded — force cleanup to prevent indefinite blocking.
			m.deps.Logger.Warn("subagent: shutdown timeout exceeded, force-cleaning",
				"id", sa.Info.ID, "timeout", shutdownTimeout)
			m.forceCleanup(sa)
			return true
		}
		// Clean up worktree so orphaned directories don't accumulate.
		if sa.Info.Isolation == IsolationWorktree &&
			sa.Info.Worktree != "" && sa.Info.Worktree != m.deps.WorkDir &&
			m.deps.Worktrees != nil {
			if err := m.deps.Worktrees.Remove(ctx, sa.Info.Worktree); err != nil {
				m.deps.Logger.Warn("subagent: worktree cleanup failed on shutdown", "id", sa.Info.ID, "err", err)
			}
		}
		m.agents.Delete(key)
		return true
	})
	close(m.eventCh)
}

// forceCleanup forcibly removes a subagent from the manager when it fails to
// exit within the shutdown timeout. The subagent's done channel is left open
// (the goroutine may still be running) but the manager forgets it so the
// application can exit.
func (m *Manager) forceCleanup(sa *Subagent) {
	m.agents.Delete(sa.Info.ID)
}

// emit delivers an event to the outbound channel. Lifecycle events that
// trigger cleanup (Done, Error, Cancelled, Spawned) block with a generous
// timeout to ensure the TUI has a chance to call Cleanup(). Text deltas
// are dropped when the channel is saturated to avoid blocking the loop.
func (m *Manager) emit(ev SubagentEvent) {
	if ev.Timestamp.IsZero() {
		ev.Timestamp = time.Now()
	}
	switch ev.Type {
	case EventDone, EventError, EventCancelled, EventSpawned:
		// These events trigger worktree cleanup in the TUI; dropping them
		// would leak directories. Use a generous timeout (5s) so the
		// consumer can keep up even under heavy load.
		select {
		case m.eventCh <- ev:
		case <-time.After(5 * time.Second):
			m.deps.Logger.Warn("subagent: lifecycle event dropped (cleanup may be skipped)",
				"type", ev.Type, "id", ev.AgentID)
		}
	default:
		select {
		case m.eventCh <- ev:
		default:
			// Text deltas and progress events are dropped under backpressure.
			// Log at debug level to avoid noise during normal high-throughput
			// operation, but allow diagnostics when events are lost.
			m.deps.Logger.Debug("subagent: non-lifecycle event dropped (channel full)",
				"type", ev.Type, "id", ev.AgentID)
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

	// Apply profile-based tool filtering (allowlist/denylist).
	ApplyToolFilter(dispatcher, sa.Profile)

	effectiveMaxTurns := maxTurns
	if sa.Profile.MaxTurns > 0 {
		effectiveMaxTurns = sa.Profile.MaxTurns
	}

	l := &loop{
		manager:    m,
		agent:      sa,
		dispatcher: dispatcher,
		provider:   m.resolveProvider(),
		modelID:    sa.Info.ModelID,
		maxTools:   sa.req.MaxTools,
		maxTokens:  sa.req.MaxTokens,
		maxTurns_:  effectiveMaxTurns,
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
