package metrics

import (
	"encoding/json"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
)

const (
	// metricsFileName is the JSON file stored in the session directory.
	metricsFileName = "METRICS.json"
	// dirPermission for session directories.
	dirPermission = m31types.DirPermission
	// filePermission for metrics file.
	filePermission = m31types.FilePermission
)

// Collector provides thread-safe collection and persistence of session metrics.
// It is designed for a single-session lifetime: create once at session start,
// record events during the session, and flush/stop at session end.
type Collector struct {
	mu           sync.Mutex
	sessionID    string
	sessionsDir  string
	metrics      *SessionMetrics
	enabled      bool
}

// NewCollector creates a new Collector scoped to the given session.
// If enabled is false, all recording methods become no-ops.
func NewCollector(sessionID, sessionsDir string, enabled bool) *Collector {
	c := &Collector{
		sessionID:   sessionID,
		sessionsDir: sessionsDir,
		enabled:     enabled,
	}
	if enabled {
		c.metrics = &SessionMetrics{
			SessionID: sessionID,
			StartedAt: time.Now(),
			UpdatedAt: time.Now(),
			Tools:     []ToolMetric{},
			LLMs:      []LLMMetric{},
			Phases:    []PhaseMetric{},
		}
	}
	return c
}

// Enabled reports whether metrics collection is active.
func (c *Collector) Enabled() bool {
	return c.enabled
}

// RecordToolCall records a completed tool execution.
func (c *Collector) RecordToolCall(name string, success bool, durationMs int64) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Tools {
		if c.metrics.Tools[i].Name == name {
			c.metrics.Tools[i].CallCount++
			c.metrics.Tools[i].TotalDurMs += durationMs
			if success {
				c.metrics.Tools[i].SuccessCount++
			} else {
				c.metrics.Tools[i].FailCount++
			}
			c.metrics.Tools[i].AvgDurMs = float64(c.metrics.Tools[i].TotalDurMs) / float64(c.metrics.Tools[i].CallCount)
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	// New tool entry
	tm := ToolMetric{
		Name:       name,
		CallCount:  1,
		TotalDurMs: durationMs,
		AvgDurMs:   float64(durationMs),
	}
	if success {
		tm.SuccessCount = 1
	} else {
		tm.FailCount = 1
	}
	c.metrics.Tools = append(c.metrics.Tools, tm)
	c.metrics.UpdatedAt = time.Now()
}

// RecordLLMInteraction records an LLM call's token usage and cost.
func (c *Collector) RecordLLMInteraction(phase m31types.WorkflowPhase, usage *m31types.Usage, cost float64) {
	if !c.enabled || usage == nil {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.LLMs {
		if c.metrics.LLMs[i].Phase == phase {
			c.metrics.LLMs[i].PromptTokens += int64(usage.PromptTokens)
			c.metrics.LLMs[i].CompletionTokens += int64(usage.CompletionTokens)
			c.metrics.LLMs[i].TotalTokens += int64(usage.TotalTokens)
			c.metrics.LLMs[i].Cost += cost
			c.metrics.LLMs[i].InteractionCount++
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	// New phase entry
	lm := LLMMetric{
		Phase:            phase,
		PromptTokens:     int64(usage.PromptTokens),
		CompletionTokens: int64(usage.CompletionTokens),
		TotalTokens:      int64(usage.TotalTokens),
		Cost:             cost,
		InteractionCount: 1,
	}
	c.metrics.LLMs = append(c.metrics.LLMs, lm)
	c.metrics.UpdatedAt = time.Now()
}

// RecordPhaseTransition records a phase transition event.
func (c *Collector) RecordPhaseTransition(phase m31types.WorkflowPhase) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			c.metrics.Phases[i].TransitionCount++
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.Phases = append(c.metrics.Phases, PhaseMetric{
		Phase:           phase,
		TransitionCount: 1,
	})
	c.metrics.UpdatedAt = time.Now()
}

// RecordPhaseDuration records the final duration of a completed phase.
func (c *Collector) RecordPhaseDuration(phase m31types.WorkflowPhase, durationMs int64, success bool) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			c.metrics.Phases[i].DurationMs = durationMs
			c.metrics.Phases[i].Success = success
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.Phases = append(c.metrics.Phases, PhaseMetric{
		Phase:      phase,
		DurationMs: durationMs,
		Success:    success,
	})
	c.metrics.UpdatedAt = time.Now()
}

// RecordHealTrigger records a self-heal trigger event.
func (c *Collector) RecordHealTrigger(phase m31types.WorkflowPhase) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			c.metrics.Phases[i].HealTriggerCount++
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.Phases = append(c.metrics.Phases, PhaseMetric{
		Phase:            phase,
		HealTriggerCount: 1,
	})
	c.metrics.UpdatedAt = time.Now()
}

// RecordBisectTrigger records a bisect trigger event.
func (c *Collector) RecordBisectTrigger(phase m31types.WorkflowPhase) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			c.metrics.Phases[i].BisectTriggerCount++
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.Phases = append(c.metrics.Phases, PhaseMetric{
		Phase:              phase,
		BisectTriggerCount: 1,
	})
	c.metrics.UpdatedAt = time.Now()
}

// Snapshot returns a deep copy of the current metrics state.
// The returned copy is safe for concurrent reads without holding the lock.
func (c *Collector) Snapshot() *SessionMetrics {
	if !c.enabled {
		return &SessionMetrics{}
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	clone := &SessionMetrics{
		SessionID: c.metrics.SessionID,
		StartedAt: c.metrics.StartedAt,
		UpdatedAt: c.metrics.UpdatedAt,
		Tools:     make([]ToolMetric, len(c.metrics.Tools)),
		LLMs:      make([]LLMMetric, len(c.metrics.LLMs)),
		Phases:    make([]PhaseMetric, len(c.metrics.Phases)),
	}
	copy(clone.Tools, c.metrics.Tools)
	copy(clone.LLMs, c.metrics.LLMs)
	copy(clone.Phases, c.metrics.Phases)
	return clone
}

// Flush persists the current metrics state to the session directory as JSON.
// Creates the directory and file if they do not exist.
func (c *Collector) Flush() error {
	if !c.enabled {
		return nil
	}
	snap := c.Snapshot()

	sessionDir := filepath.Join(c.sessionsDir, c.sessionID)
	if err := os.MkdirAll(sessionDir, dirPermission); err != nil {
		return err
	}

	filePath := filepath.Join(sessionDir, metricsFileName)

	data, err := json.MarshalIndent(snap, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(filePath, data, filePermission)
}

// Load reads a SessionMetrics from the session directory.
// Returns nil and a nil error if the file does not exist.
func Load(sessionsDir, sessionID string) (*SessionMetrics, error) {
	filePath := filepath.Join(sessionsDir, sessionID, metricsFileName)

	data, err := os.ReadFile(filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var m SessionMetrics
	if err := json.Unmarshal(data, &m); err != nil {
		return nil, err
	}
	return &m, nil
}

// Save persists the given SessionMetrics to the session directory.
func Save(sessionsDir, sessionID string, m *SessionMetrics) error {
	sessionDir := filepath.Join(sessionsDir, sessionID)
	if err := os.MkdirAll(sessionDir, dirPermission); err != nil {
		return err
	}

	filePath := filepath.Join(sessionDir, metricsFileName)

	data, err := json.MarshalIndent(m, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(filePath, data, filePermission)
}

// Stop persists metrics and logs a summary. Intended for session cleanup.
func (c *Collector) Stop() {
	if !c.enabled {
		return
	}
	if err := c.Flush(); err != nil {
		slog.Warn("metrics flush failed", "error", err)
	}
	snap := c.Snapshot()
	slog.Info("session metrics",
		"tools", len(snap.Tools),
		"phases", len(snap.Phases),
		"llm_interactions", len(snap.LLMs),
	)
}
