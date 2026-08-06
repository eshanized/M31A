package metrics

import (
	"encoding/json"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

const (
	// metricsFileName is the JSON file stored in the session directory.
	metricsFileName = "METRICS.json"
	// dirPermission for session directories.
	dirPermission = types.DirPermission
	// filePermission for metrics file.
	filePermission = types.FilePermission
)

// Collector provides thread-safe collection and persistence of session metrics.
// It is designed for a single-session lifetime: create once at session start,
// record events during the session, and flush/stop at session end.
type Collector struct {
	mu          sync.Mutex
	sessionID   string
	sessionsDir string
	metrics     *SessionMetrics
	enabled     bool
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
			SessionID:    sessionID,
			StartedAt:    time.Now(),
			UpdatedAt:    time.Now(),
			Tools:        []ToolMetric{},
			LLMs:         []LLMMetric{},
			Phases:       []PhaseMetric{},
			PlanOutcomes: []PlanOutcome{},
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
func (c *Collector) RecordLLMInteraction(phase types.WorkflowPhase, usage *types.Usage, cost float64) {
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

// RecordLLMInteractionWithPrompt records an LLM call with prompt hash and truncation info.
func (c *Collector) RecordLLMInteractionWithPrompt(phase types.WorkflowPhase, usage *types.Usage, cost float64, promptHash string, truncated bool) {
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
			if promptHash != "" && c.metrics.LLMs[i].PromptHash == "" {
				c.metrics.LLMs[i].PromptHash = promptHash
			}
			if truncated {
				c.metrics.LLMs[i].TruncatedInput = true
			}
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
		PromptHash:       promptHash,
		TruncatedInput:   truncated,
	}
	c.metrics.LLMs = append(c.metrics.LLMs, lm)
	c.metrics.UpdatedAt = time.Now()
}

// RecordPhaseTransition records a phase transition event.
func (c *Collector) RecordPhaseTransition(phase types.WorkflowPhase) {
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
func (c *Collector) RecordPhaseDuration(phase types.WorkflowPhase, durationMs int64, success bool) {
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
func (c *Collector) RecordHealTrigger(phase types.WorkflowPhase) {
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
func (c *Collector) RecordBisectTrigger(phase types.WorkflowPhase) {
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

// RecordEditStrategy records which edit replacement strategy was used.
func (c *Collector) RecordEditStrategy(strategy string) {
	if !c.enabled || strategy == "" {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.EditStrategies {
		if c.metrics.EditStrategies[i].Strategy == strategy {
			c.metrics.EditStrategies[i].Count++
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.EditStrategies = append(c.metrics.EditStrategies, EditStrategyMetric{
		Strategy: strategy,
		Count:    1,
	})
	c.metrics.UpdatedAt = time.Now()
}

// RecordHealOutcome records whether a self-heal attempt succeeded or failed.
func (c *Collector) RecordHealOutcome(phase types.WorkflowPhase, success bool) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			if success {
				c.metrics.Phases[i].HealSuccessCount++
			} else {
				c.metrics.Phases[i].HealFailCount++
			}
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	// New phase entry
	pm := PhaseMetric{Phase: phase}
	if success {
		pm.HealSuccessCount = 1
	} else {
		pm.HealFailCount = 1
	}
	c.metrics.Phases = append(c.metrics.Phases, pm)
	c.metrics.UpdatedAt = time.Now()
}

// RecordHealDuration adds heal duration to the phase metric.
func (c *Collector) RecordHealDuration(phase types.WorkflowPhase, durationMs int64) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			c.metrics.Phases[i].HealDurationMs += durationMs
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.Phases = append(c.metrics.Phases, PhaseMetric{
		Phase:          phase,
		HealDurationMs: durationMs,
	})
	c.metrics.UpdatedAt = time.Now()
}

// RecordHealLoop records a detected heal loop (same error repeating).
func (c *Collector) RecordHealLoop(phase types.WorkflowPhase) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			c.metrics.Phases[i].HealLoopCount++
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	c.metrics.Phases = append(c.metrics.Phases, PhaseMetric{
		Phase:         phase,
		HealLoopCount: 1,
	})
	c.metrics.UpdatedAt = time.Now()
}

// RecordBisectOutcome records whether a bisect heal attempt succeeded or failed.
func (c *Collector) RecordBisectOutcome(phase types.WorkflowPhase, success bool) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	for i := range c.metrics.Phases {
		if c.metrics.Phases[i].Phase == phase {
			if success {
				c.metrics.Phases[i].BisectSuccessCount++
			} else {
				c.metrics.Phases[i].BisectFailCount++
			}
			c.metrics.UpdatedAt = time.Now()
			return
		}
	}
	// New phase entry
	pm := PhaseMetric{Phase: phase}
	if success {
		pm.BisectSuccessCount = 1
	} else {
		pm.BisectFailCount = 1
	}
	c.metrics.Phases = append(c.metrics.Phases, pm)
	c.metrics.UpdatedAt = time.Now()
}

// RecordPlanOutcome records a completed plan task execution outcome.
func (c *Collector) RecordPlanOutcome(outcome PlanOutcome) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	c.metrics.PlanOutcomes = append(c.metrics.PlanOutcomes, outcome)
	c.metrics.UpdatedAt = time.Now()
}

// CurrentSessionCost returns the total LLM cost across all phases for this session.
func (c *Collector) CurrentSessionCost() float64 {
	if !c.enabled {
		return 0
	}
	snap := c.Snapshot()
	var total float64
	for _, llm := range snap.LLMs {
		total += llm.Cost
	}
	return total
}

// RecentPlanOutcomes returns the most recent n plan outcomes.
func (c *Collector) RecentPlanOutcomes(n int) []PlanOutcome {
	if !c.enabled {
		return nil
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	total := len(c.metrics.PlanOutcomes)
	if n <= 0 || n > total {
		n = total
	}
	start := total - n
	result := make([]PlanOutcome, n)
	copy(result, c.metrics.PlanOutcomes[start:])
	return result
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
		SessionID:      c.metrics.SessionID,
		StartedAt:      c.metrics.StartedAt,
		UpdatedAt:      c.metrics.UpdatedAt,
		Tools:          make([]ToolMetric, len(c.metrics.Tools)),
		EditStrategies: make([]EditStrategyMetric, len(c.metrics.EditStrategies)),
		LLMs:           make([]LLMMetric, len(c.metrics.LLMs)),
		Phases:         make([]PhaseMetric, len(c.metrics.Phases)),
		PlanOutcomes:   make([]PlanOutcome, len(c.metrics.PlanOutcomes)),
		Startup:        c.metrics.Startup,
		Completions:    make([]CompletionMetric, len(c.metrics.Completions)),
		Cancellations:  make([]CancellationMetric, len(c.metrics.Cancellations)),
	}
	copy(clone.Tools, c.metrics.Tools)
	copy(clone.EditStrategies, c.metrics.EditStrategies)
	copy(clone.LLMs, c.metrics.LLMs)
	copy(clone.Phases, c.metrics.Phases)
	copy(clone.PlanOutcomes, c.metrics.PlanOutcomes)
	copy(clone.Completions, c.metrics.Completions)
	copy(clone.Cancellations, c.metrics.Cancellations)
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

// RecordStartup records startup timing breakdown.
func (c *Collector) RecordStartup(durationMs, configLoadMs, providerMs, tuiMs int64) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	c.metrics.Startup = &StartupMetric{
		DurationMs:   durationMs,
		ConfigLoadMs: configLoadMs,
		ProviderMs:   providerMs,
		TUIMs:        tuiMs,
	}
	c.metrics.UpdatedAt = time.Now()
}

// RecordCompletion records a phase completion event.
func (c *Collector) RecordCompletion(phase types.WorkflowPhase, success bool, timeout bool) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	cm := CompletionMetric{
		Phase:     phase,
		Success:   success,
		Failure:   !success && !timeout,
		Timeout:   timeout,
		Timestamp: time.Now(),
	}
	c.metrics.Completions = append(c.metrics.Completions, cm)
	c.metrics.UpdatedAt = time.Now()
}

// RecordCancellation records a workflow cancellation event.
func (c *Collector) RecordCancellation(phase types.WorkflowPhase, reason string) {
	if !c.enabled {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	cm := CancellationMetric{
		Phase:     phase,
		Reason:    reason,
		Timestamp: time.Now(),
	}
	c.metrics.Cancellations = append(c.metrics.Cancellations, cm)
	c.metrics.UpdatedAt = time.Now()
}
