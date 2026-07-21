package metrics

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestNewCollector(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	if c == nil {
		t.Fatal("NewCollector() returned nil")
	}
	if !c.Enabled() {
		t.Error("expected Enabled() to be true")
	}
	if c.sessionID != "session1" {
		t.Errorf("sessionID = %q, want %q", c.sessionID, "session1")
	}
}

func TestNewCollector_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	if c.Enabled() {
		t.Error("expected Enabled() to be false")
	}
}

func TestCollector_RecordToolCall(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordToolCall("bash", true, 100)
	c.RecordToolCall("bash", true, 200)
	c.RecordToolCall("bash", false, 50)

	snap := c.Snapshot()
	if len(snap.Tools) != 1 {
		t.Fatalf("expected 1 tool, got %d", len(snap.Tools))
	}

	tool := snap.Tools[0]
	if tool.Name != "bash" {
		t.Errorf("tool Name = %q, want %q", tool.Name, "bash")
	}
	if tool.CallCount != 3 {
		t.Errorf("CallCount = %d, want 3", tool.CallCount)
	}
	if tool.SuccessCount != 2 {
		t.Errorf("SuccessCount = %d, want 2", tool.SuccessCount)
	}
	if tool.FailCount != 1 {
		t.Errorf("FailCount = %d, want 1", tool.FailCount)
	}
	if tool.TotalDurMs != 350 {
		t.Errorf("TotalDurMs = %d, want 350", tool.TotalDurMs)
	}
}

func TestCollector_RecordToolCall_MultipleTools(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordToolCall("bash", true, 100)
	c.RecordToolCall("read", true, 50)
	c.RecordToolCall("write", false, 200)

	snap := c.Snapshot()
	if len(snap.Tools) != 3 {
		t.Fatalf("expected 3 tools, got %d", len(snap.Tools))
	}
}

func TestCollector_RecordToolCall_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordToolCall("bash", true, 100)

	snap := c.Snapshot()
	if len(snap.Tools) != 0 {
		t.Errorf("expected 0 tools when disabled, got %d", len(snap.Tools))
	}
}

func TestCollector_RecordLLMInteraction(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	usage := &m31types.Usage{
		PromptTokens:     100,
		CompletionTokens: 50,
		TotalTokens:      150,
	}
	c.RecordLLMInteraction(m31types.PhasePlan, usage, 0.01)
	c.RecordLLMInteraction(m31types.PhasePlan, usage, 0.01)

	snap := c.Snapshot()
	if len(snap.LLMs) != 1 {
		t.Fatalf("expected 1 LLM metric, got %d", len(snap.LLMs))
	}

	llm := snap.LLMs[0]
	if llm.PromptTokens != 200 {
		t.Errorf("PromptTokens = %d, want 200", llm.PromptTokens)
	}
	if llm.CompletionTokens != 100 {
		t.Errorf("CompletionTokens = %d, want 100", llm.CompletionTokens)
	}
	if llm.TotalTokens != 300 {
		t.Errorf("TotalTokens = %d, want 300", llm.TotalTokens)
	}
	if llm.InteractionCount != 2 {
		t.Errorf("InteractionCount = %d, want 2", llm.InteractionCount)
	}
}

func TestCollector_RecordLLMInteraction_NilUsage(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordLLMInteraction(m31types.PhasePlan, nil, 0.01)

	snap := c.Snapshot()
	if len(snap.LLMs) != 0 {
		t.Errorf("expected 0 LLM metrics with nil usage, got %d", len(snap.LLMs))
	}
}

func TestCollector_RecordLLMInteraction_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	usage := &m31types.Usage{PromptTokens: 100}
	c.RecordLLMInteraction(m31types.PhasePlan, usage, 0.01)

	snap := c.Snapshot()
	if len(snap.LLMs) != 0 {
		t.Errorf("expected 0 LLM metrics when disabled, got %d", len(snap.LLMs))
	}
}

func TestCollector_RecordPhaseTransition(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordPhaseTransition(m31types.PhasePlan)
	c.RecordPhaseTransition(m31types.PhasePlan)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}

	phase := snap.Phases[0]
	if phase.TransitionCount != 2 {
		t.Errorf("TransitionCount = %d, want 2", phase.TransitionCount)
	}
}

func TestCollector_RecordPhaseTransition_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordPhaseTransition(m31types.PhasePlan)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

func TestCollector_RecordPhaseDuration(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordPhaseDuration(m31types.PhasePlan, 1000, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}

	phase := snap.Phases[0]
	if phase.DurationMs != 1000 {
		t.Errorf("DurationMs = %d, want 1000", phase.DurationMs)
	}
	if !phase.Success {
		t.Error("Success = false, want true")
	}
}

func TestCollector_RecordPhaseDuration_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordPhaseDuration(m31types.PhasePlan, 1000, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

func TestCollector_RecordHealTrigger(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordHealTrigger(m31types.PhaseExecute)
	c.RecordHealTrigger(m31types.PhaseExecute)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}

	phase := snap.Phases[0]
	if phase.HealTriggerCount != 2 {
		t.Errorf("HealTriggerCount = %d, want 2", phase.HealTriggerCount)
	}
}

func TestCollector_RecordHealTrigger_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordHealTrigger(m31types.PhaseExecute)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

func TestCollector_RecordBisectTrigger(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordBisectTrigger(m31types.PhaseExecute)
	c.RecordBisectTrigger(m31types.PhaseExecute)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}

	phase := snap.Phases[0]
	if phase.BisectTriggerCount != 2 {
		t.Errorf("BisectTriggerCount = %d, want 2", phase.BisectTriggerCount)
	}
}

func TestCollector_RecordBisectTrigger_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordBisectTrigger(m31types.PhaseExecute)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

func TestCollector_Snapshot(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordToolCall("bash", true, 100)
	c.RecordPhaseTransition(m31types.PhasePlan)

	snap1 := c.Snapshot()
	snap2 := c.Snapshot()

	// Verify deep copy - modifying snap1 should not affect snap2
	snap1.Tools[0].Name = "modified"
	if snap2.Tools[0].Name == "modified" {
		t.Error("Snapshot() did not return a deep copy")
	}
}

func TestCollector_Snapshot_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	snap := c.Snapshot()
	if snap == nil {
		t.Fatal("Snapshot() returned nil for disabled collector")
	}
	if len(snap.Tools) != 0 || len(snap.LLMs) != 0 || len(snap.Phases) != 0 {
		t.Error("expected empty metrics for disabled collector")
	}
}

func TestCollector_Flush(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordToolCall("bash", true, 100)

	if err := c.Flush(); err != nil {
		t.Fatalf("Flush() error = %v", err)
	}

	// Verify file was created
	filePath := filepath.Join(dir, "session1", "METRICS.json")
	if _, err := os.Stat(filePath); os.IsNotExist(err) {
		t.Error("metrics file was not created")
	}
}

func TestCollector_Flush_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	if err := c.Flush(); err != nil {
		t.Errorf("Flush() error = %v", err)
	}
}

func TestCollector_Stop(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordToolCall("bash", true, 100)

	// Stop should not panic
	c.Stop()

	// Verify file was created
	filePath := filepath.Join(dir, "session1", "METRICS.json")
	if _, err := os.Stat(filePath); os.IsNotExist(err) {
		t.Error("metrics file was not created after Stop")
	}
}

func TestCollector_Stop_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	// Stop should not panic when disabled
	c.Stop()
}

func TestLoad(t *testing.T) {
	dir := t.TempDir()
	sessionID := "test-session"

	// Create metrics file
	metrics := &SessionMetrics{
		SessionID: sessionID,
		StartedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	if err := Save(dir, sessionID, metrics); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	loaded, err := Load(dir, sessionID)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if loaded == nil {
		t.Fatal("Load() returned nil")
	}
	if loaded.SessionID != sessionID {
		t.Errorf("SessionID = %q, want %q", loaded.SessionID, sessionID)
	}
}

func TestLoad_NotExist(t *testing.T) {
	dir := t.TempDir()
	loaded, err := Load(dir, "nonexistent")
	if err != nil {
		t.Errorf("Load() error = %v", err)
	}
	if loaded != nil {
		t.Error("expected nil for nonexistent session")
	}
}

func TestLoad_InvalidJSON(t *testing.T) {
	dir := t.TempDir()
	sessionID := "test-session"

	sessionDir := filepath.Join(dir, sessionID)
	if err := os.MkdirAll(sessionDir, 0755); err != nil {
		t.Fatal(err)
	}
	filePath := filepath.Join(sessionDir, "METRICS.json")
	if err := os.WriteFile(filePath, []byte("invalid json"), 0644); err != nil {
		t.Fatal(err)
	}

	_, err := Load(dir, sessionID)
	if err == nil {
		t.Error("expected error for invalid JSON")
	}
}

func TestSave(t *testing.T) {
	dir := t.TempDir()
	sessionID := "test-session"

	metrics := &SessionMetrics{
		SessionID: sessionID,
		StartedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	if err := Save(dir, sessionID, metrics); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	// Verify file was created
	filePath := filepath.Join(dir, sessionID, "METRICS.json")
	if _, err := os.Stat(filePath); os.IsNotExist(err) {
		t.Error("metrics file was not created")
	}
}

func TestCollector_MultiplePhases(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordPhaseTransition(m31types.PhasePlan)
	c.RecordPhaseTransition(m31types.PhaseExecute)
	c.RecordPhaseDuration(m31types.PhasePlan, 1000, true)
	c.RecordPhaseDuration(m31types.PhaseExecute, 2000, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 2 {
		t.Fatalf("expected 2 phases, got %d", len(snap.Phases))
	}
}

func TestCollector_SessionID(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("my-session", dir, true)

	snap := c.Snapshot()
	if snap.SessionID != "my-session" {
		t.Errorf("SessionID = %q, want %q", snap.SessionID, "my-session")
	}
}

func TestCollector_Timestamps(t *testing.T) {
	dir := t.TempDir()
	before := time.Now()
	c := NewCollector("session1", dir, true)
	after := time.Now()

	snap := c.Snapshot()
	if snap.StartedAt.Before(before) || snap.StartedAt.After(after) {
		t.Error("StartedAt is not within expected range")
	}
}

// --- RecordEditStrategy tests ---

func TestCollector_RecordEditStrategy(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordEditStrategy("exact_match")
	c.RecordEditStrategy("exact_match")
	c.RecordEditStrategy("replace_all")

	snap := c.Snapshot()
	if len(snap.EditStrategies) != 2 {
		t.Fatalf("expected 2 strategies, got %d", len(snap.EditStrategies))
	}
	if snap.EditStrategies[0].Strategy != "exact_match" || snap.EditStrategies[0].Count != 2 {
		t.Errorf("exact_match: got %+v", snap.EditStrategies[0])
	}
	if snap.EditStrategies[1].Strategy != "replace_all" || snap.EditStrategies[1].Count != 1 {
		t.Errorf("replace_all: got %+v", snap.EditStrategies[1])
	}
}

func TestCollector_RecordEditStrategy_Empty(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordEditStrategy("")

	snap := c.Snapshot()
	if len(snap.EditStrategies) != 0 {
		t.Errorf("expected 0 strategies for empty input, got %d", len(snap.EditStrategies))
	}
}

func TestCollector_RecordEditStrategy_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordEditStrategy("exact_match")

	snap := c.Snapshot()
	if len(snap.EditStrategies) != 0 {
		t.Errorf("expected 0 strategies when disabled, got %d", len(snap.EditStrategies))
	}
}

// --- RecordHealOutcome tests ---

func TestCollector_RecordHealOutcome(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordHealOutcome(m31types.PhaseExecute, true)
	c.RecordHealOutcome(m31types.PhaseExecute, true)
	c.RecordHealOutcome(m31types.PhaseExecute, false)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	phase := snap.Phases[0]
	if phase.HealSuccessCount != 2 {
		t.Errorf("HealSuccessCount = %d, want 2", phase.HealSuccessCount)
	}
	if phase.HealFailCount != 1 {
		t.Errorf("HealFailCount = %d, want 1", phase.HealFailCount)
	}
}

func TestCollector_RecordHealOutcome_NewPhase(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	// No prior phase entry — should create one
	c.RecordHealOutcome(m31types.PhaseVerify, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	if snap.Phases[0].HealSuccessCount != 1 {
		t.Errorf("HealSuccessCount = %d, want 1", snap.Phases[0].HealSuccessCount)
	}
}

func TestCollector_RecordHealOutcome_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordHealOutcome(m31types.PhaseExecute, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

// --- RecordBisectOutcome tests ---

func TestCollector_RecordBisectOutcome(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordBisectOutcome(m31types.PhaseVerify, true)
	c.RecordBisectOutcome(m31types.PhaseVerify, false)
	c.RecordBisectOutcome(m31types.PhaseVerify, false)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	phase := snap.Phases[0]
	if phase.BisectSuccessCount != 1 {
		t.Errorf("BisectSuccessCount = %d, want 1", phase.BisectSuccessCount)
	}
	if phase.BisectFailCount != 2 {
		t.Errorf("BisectFailCount = %d, want 2", phase.BisectFailCount)
	}
}

func TestCollector_RecordBisectOutcome_NewPhase(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordBisectOutcome(m31types.PhaseVerify, false)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	if snap.Phases[0].BisectFailCount != 1 {
		t.Errorf("BisectFailCount = %d, want 1", snap.Phases[0].BisectFailCount)
	}
}

func TestCollector_RecordBisectOutcome_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordBisectOutcome(m31types.PhaseVerify, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

// --- Snapshot deep copy for EditStrategies ---

func TestCollector_Snapshot_EditStrategiesDeepCopy(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordEditStrategy("exact_match")

	snap1 := c.Snapshot()
	snap2 := c.Snapshot()

	snap1.EditStrategies[0].Strategy = "modified"
	if snap2.EditStrategies[0].Strategy == "modified" {
		t.Error("Snapshot did not deep-copy EditStrategies")
	}
}

// --- Combined scenario ---

func TestCollector_CombinedScenario(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordToolCall("Edit", true, 50)
	c.RecordEditStrategy("exact_match")
	c.RecordEditStrategy("cascading")
	c.RecordHealTrigger(m31types.PhaseExecute)
	c.RecordHealOutcome(m31types.PhaseExecute, true)
	c.RecordBisectTrigger(m31types.PhaseVerify)
	c.RecordBisectOutcome(m31types.PhaseVerify, false)

	snap := c.Snapshot()
	if len(snap.Tools) != 1 {
		t.Errorf("expected 1 tool, got %d", len(snap.Tools))
	}
	if len(snap.EditStrategies) != 2 {
		t.Errorf("expected 2 strategies, got %d", len(snap.EditStrategies))
	}
	if len(snap.Phases) != 2 {
		t.Errorf("expected 2 phases, got %d", len(snap.Phases))
	}
}

// --- HashPrompt tests ---

func TestHashPrompt_Deterministic(t *testing.T) {
	t.Parallel()
	h1 := HashPrompt("hello world")
	h2 := HashPrompt("hello world")
	if h1 != h2 {
		t.Errorf("HashPrompt not deterministic: %q != %q", h1, h2)
	}
	if len(h1) != 16 {
		t.Errorf("HashPrompt length = %d, want 16", len(h1))
	}
}

func TestHashPrompt_DifferentInputs(t *testing.T) {
	t.Parallel()
	h1 := HashPrompt("prompt A")
	h2 := HashPrompt("prompt B")
	if h1 == h2 {
		t.Errorf("different inputs produced same hash: %q", h1)
	}
}

func TestHashPrompt_Empty(t *testing.T) {
	t.Parallel()
	h := HashPrompt("")
	if len(h) != 16 {
		t.Errorf("HashPrompt empty length = %d, want 16", len(h))
	}
}

// --- RecordLLMInteractionWithPrompt tests ---

func TestCollector_RecordLLMInteractionWithPrompt(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	usage := &m31types.Usage{
		PromptTokens:     100,
		CompletionTokens: 50,
		TotalTokens:      150,
	}
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.01, "abc123", false)

	snap := c.Snapshot()
	if len(snap.LLMs) != 1 {
		t.Fatalf("expected 1 LLM metric, got %d", len(snap.LLMs))
	}
	llm := snap.LLMs[0]
	if llm.PromptHash != "abc123" {
		t.Errorf("PromptHash = %q, want abc123", llm.PromptHash)
	}
	if llm.TruncatedInput {
		t.Error("TruncatedInput should be false")
	}
}

func TestCollector_RecordLLMInteractionWithPrompt_Truncated(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	usage := &m31types.Usage{PromptTokens: 100, CompletionTokens: 50, TotalTokens: 150}
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.01, "hash1", true)

	snap := c.Snapshot()
	if !snap.LLMs[0].TruncatedInput {
		t.Error("TruncatedInput should be true")
	}
}

func TestCollector_RecordLLMInteractionWithPrompt_Accumulates(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	usage := &m31types.Usage{PromptTokens: 100, CompletionTokens: 50, TotalTokens: 150}
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.01, "hash1", false)
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.02, "hash2", true)

	snap := c.Snapshot()
	if len(snap.LLMs) != 1 {
		t.Fatalf("expected 1 LLM metric (accumulated), got %d", len(snap.LLMs))
	}
	llm := snap.LLMs[0]
	if llm.InteractionCount != 2 {
		t.Errorf("InteractionCount = %d, want 2", llm.InteractionCount)
	}
	if llm.PromptHash != "hash1" {
		t.Errorf("PromptHash should keep first: %q, want hash1", llm.PromptHash)
	}
	if !llm.TruncatedInput {
		t.Error("TruncatedInput should be true after truncation")
	}
}

func TestCollector_RecordLLMInteractionWithPrompt_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	usage := &m31types.Usage{PromptTokens: 100}
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.01, "hash1", true)

	snap := c.Snapshot()
	if len(snap.LLMs) != 0 {
		t.Errorf("expected 0 LLM metrics when disabled, got %d", len(snap.LLMs))
	}
}

func TestCollector_RecordLLMInteractionWithPrompt_NilUsage(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, nil, 0.01, "hash1", false)

	snap := c.Snapshot()
	if len(snap.LLMs) != 0 {
		t.Errorf("expected 0 LLM metrics with nil usage, got %d", len(snap.LLMs))
	}
}

func TestCollector_RecordLLMInteractionWithPrompt_EmptyHash(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	usage := &m31types.Usage{PromptTokens: 100, CompletionTokens: 50, TotalTokens: 150}
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.01, "", false)

	snap := c.Snapshot()
	if snap.LLMs[0].PromptHash != "" {
		t.Errorf("PromptHash should be empty, got %q", snap.LLMs[0].PromptHash)
	}
}

func TestCollector_Snapshot_LLMDeepCopy(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	usage := &m31types.Usage{PromptTokens: 100, CompletionTokens: 50, TotalTokens: 150}
	c.RecordLLMInteractionWithPrompt(m31types.PhasePlan, usage, 0.01, "hash1", true)

	snap1 := c.Snapshot()
	snap2 := c.Snapshot()

	snap1.LLMs[0].PromptHash = "modified"
	if snap2.LLMs[0].PromptHash == "modified" {
		t.Error("Snapshot did not deep-copy LLMs")
	}
}

// --- RecordHealDuration tests ---

func TestCollector_RecordHealDuration(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordHealDuration(m31types.PhaseExecute, 500)
	c.RecordHealDuration(m31types.PhaseExecute, 300)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	if snap.Phases[0].HealDurationMs != 800 {
		t.Errorf("HealDurationMs = %d, want 800", snap.Phases[0].HealDurationMs)
	}
}

func TestCollector_RecordHealDuration_NewPhase(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordHealDuration(m31types.PhaseVerify, 200)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	if snap.Phases[0].HealDurationMs != 200 {
		t.Errorf("HealDurationMs = %d, want 200", snap.Phases[0].HealDurationMs)
	}
}

func TestCollector_RecordHealDuration_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordHealDuration(m31types.PhaseExecute, 500)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

// --- RecordHealLoop tests ---

func TestCollector_RecordHealLoop(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordHealLoop(m31types.PhaseExecute)
	c.RecordHealLoop(m31types.PhaseExecute)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	if snap.Phases[0].HealLoopCount != 2 {
		t.Errorf("HealLoopCount = %d, want 2", snap.Phases[0].HealLoopCount)
	}
}

func TestCollector_RecordHealLoop_NewPhase(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	c.RecordHealLoop(m31types.PhaseVerify)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	if snap.Phases[0].HealLoopCount != 1 {
		t.Errorf("HealLoopCount = %d, want 1", snap.Phases[0].HealLoopCount)
	}
}

func TestCollector_RecordHealLoop_Disabled(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, false)

	c.RecordHealLoop(m31types.PhaseExecute)

	snap := c.Snapshot()
	if len(snap.Phases) != 0 {
		t.Errorf("expected 0 phases when disabled, got %d", len(snap.Phases))
	}
}

// --- Combined heal quality scenario ---

func TestCollector_HealQualityScenario(t *testing.T) {
	dir := t.TempDir()
	c := NewCollector("session1", dir, true)

	// Simulate heal sequence: trigger, duration, outcome, loop detection
	c.RecordHealTrigger(m31types.PhaseExecute)
	c.RecordHealDuration(m31types.PhaseExecute, 1500)
	c.RecordHealOutcome(m31types.PhaseExecute, false)
	c.RecordHealLoop(m31types.PhaseExecute)
	c.RecordHealDuration(m31types.PhaseExecute, 2000)
	c.RecordHealOutcome(m31types.PhaseExecute, true)

	snap := c.Snapshot()
	if len(snap.Phases) != 1 {
		t.Fatalf("expected 1 phase, got %d", len(snap.Phases))
	}
	p := snap.Phases[0]
	if p.HealTriggerCount != 1 {
		t.Errorf("HealTriggerCount = %d, want 1", p.HealTriggerCount)
	}
	if p.HealDurationMs != 3500 {
		t.Errorf("HealDurationMs = %d, want 3500", p.HealDurationMs)
	}
	if p.HealSuccessCount != 1 {
		t.Errorf("HealSuccessCount = %d, want 1", p.HealSuccessCount)
	}
	if p.HealFailCount != 1 {
		t.Errorf("HealFailCount = %d, want 1", p.HealFailCount)
	}
	if p.HealLoopCount != 1 {
		t.Errorf("HealLoopCount = %d, want 1", p.HealLoopCount)
	}
}
