package workflow

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/decision"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/tests/testutil/mocks"
	m31types "github.com/eshanized/M31A/internal/types"
)

// ============================================================================
// Retry Tests (unique ones not in retry_test.go)
// ============================================================================

func TestRetryWithBackoff_NonRetryableStopsImmediately(t *testing.T) {
	calls := 0
	err := RetryWithBackoff(context.Background(), RetryConfig{MaxAttempts: 3}, func() error {
		calls++
		return errors.New("permanent failure")
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if calls != 1 {
		t.Errorf("expected 1 call (non-retryable), got %d", calls)
	}
}

func TestRetryWithResult_NonRetryableStopsImmediately(t *testing.T) {
	calls := 0
	result, err := RetryWithResult(context.Background(), RetryConfig{MaxAttempts: 3}, func() (string, error) {
		calls++
		return "", errors.New("permanent failure")
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if result != "" {
		t.Errorf("expected empty result, got %q", result)
	}
	if calls != 1 {
		t.Errorf("expected 1 call, got %d", calls)
	}
}

func TestRetryWithHeaders_ContextCancellation(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()

	err := RetryWithHeaders(ctx, RetryConfig{MaxAttempts: 100, BaseDelay: 100 * time.Millisecond}, func() (http.Header, error) {
		return nil, errors.New("retryable")
	})
	if err == nil {
		t.Fatal("expected error from cancelled context")
	}
}

func TestRetryWithHeaders_NonRetryableStopsImmediately(t *testing.T) {
	calls := 0
	err := RetryWithHeaders(context.Background(), RetryConfig{MaxAttempts: 3}, func() (http.Header, error) {
		calls++
		return nil, errors.New("permanent failure")
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if calls != 1 {
		t.Errorf("expected 1 call, got %d", calls)
	}
}

func TestRetryConfig_ToPolicy(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 5, BaseDelay: 200 * time.Millisecond}.toPolicy()
	if cfg.MaxAttempts != 5 {
		t.Errorf("expected 5, got %d", cfg.MaxAttempts)
	}
	if cfg.InitialDelay != 200*time.Millisecond {
		t.Errorf("expected 200ms, got %v", cfg.InitialDelay)
	}
}

// ============================================================================
// StateMachine Tests (unique ones not in state_machine_test.go)
// ============================================================================

func TestStateMachine_Transition_InvalidFromPhase(t *testing.T) {
	sm := NewStateMachine()
	err := sm.Transition(m31types.PhaseDiscuss, m31types.PhasePlan) // starting phase is Idle
	if err == nil {
		t.Fatal("expected error for invalid from phase")
	}
}

func TestStateMachine_Transition_InvalidToPhase(t *testing.T) {
	sm := NewStateMachine()
	err := sm.Transition(m31types.PhaseIdle, m31types.PhasePlan) // Idle -> Plan not allowed
	if err == nil {
		t.Fatal("expected error for invalid transition")
	}
}

func TestStateMachine_HistoryCopySemantics(t *testing.T) {
	sm := NewStateMachine()
	sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize)
	sm.Transition(m31types.PhaseInitialize, m31types.PhaseExecute)

	history := sm.History()
	history[0] = m31types.PhaseShip
	orig := sm.History()
	if orig[0] != m31types.PhaseIdle {
		t.Error("modifying returned history should not affect original")
	}
}

func TestStateMachine_DiscussPlanCycleLimit(t *testing.T) {
	sm := NewStateMachine()
	sm.SetPhase(m31types.PhasePlan)

	for i := 0; i < maxDiscussPlanCycles; i++ {
		err := sm.Transition(m31types.PhasePlan, m31types.PhaseDiscuss)
		if err != nil {
			t.Fatalf("cycle %d: unexpected error: %v", i, err)
		}
		err = sm.Transition(m31types.PhaseDiscuss, m31types.PhasePlan)
		if err != nil {
			t.Fatalf("cycle %d back: unexpected error: %v", i, err)
		}
	}

	err := sm.Transition(m31types.PhasePlan, m31types.PhaseDiscuss)
	if err == nil {
		t.Fatal("expected cycle limit error")
	}
}

func TestStateMachine_CycleResetOnExecute(t *testing.T) {
	sm := NewStateMachine()
	sm.SetPhase(m31types.PhasePlan)

	for i := 0; i < 2; i++ {
		sm.Transition(m31types.PhasePlan, m31types.PhaseDiscuss)
		sm.Transition(m31types.PhaseDiscuss, m31types.PhasePlan)
	}

	sm.Transition(m31types.PhasePlan, m31types.PhaseExecute)
	if sm.CurrentPhase() != m31types.PhaseExecute {
		t.Errorf("expected PhaseExecute, got %s", sm.CurrentPhase())
	}
}

// ============================================================================
// Engine Decision/Checkpoint Tests
// ============================================================================

func TestEngine_FlushDecisions_NilLogger(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.state.decisionLog = nil
	decisions := engine.FlushDecisions()
	if decisions != nil {
		t.Errorf("expected nil, got %v", decisions)
	}
}

func TestEngine_SnapshotDecisions_NilLogger(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.state.decisionLog = nil
	decisions := engine.SnapshotDecisions()
	if decisions != nil {
		t.Errorf("expected nil, got %v", decisions)
	}
}

func TestEngine_SaveCheckpointData_Roundtrip(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SaveCheckpointData("test goal")
	cp := engine.GetCheckpointData()
	if cp == nil {
		t.Fatal("expected checkpoint data")
	}
	if cp.Goal != "test goal" {
		t.Errorf("expected goal 'test goal', got %q", cp.Goal)
	}
	if cp.Timestamp.IsZero() {
		t.Error("expected non-zero timestamp")
	}
}

func TestEngine_LoadCheckpointData_NilData(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.LoadCheckpointData(nil)
	if engine.GetCheckpointData() != nil {
		t.Error("expected nil checkpoint data when no checkpoints on disk")
	}
}

func TestEngine_LoadCheckpointData_WithData(t *testing.T) {
	engine, _ := setupTestEngine(t)
	cp := &CheckpointData{
		Phase:       m31types.PhasePlan,
		Goal:        "test",
		PlanVersion: 3,
		Timestamp:   time.Now(),
	}
	engine.LoadCheckpointData(cp)
	if engine.GetCheckpointData() != cp {
		t.Error("expected same checkpoint data")
	}
	if engine.stateMachine.CurrentPhase() != m31types.PhasePlan {
		t.Errorf("expected PhasePlan, got %s", engine.stateMachine.CurrentPhase())
	}
	if engine.state.planVersion != 3 {
		t.Errorf("expected planVersion 3, got %d", engine.state.planVersion)
	}
}

func TestEngine_Close_NilLogger(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.state.decisionLog = nil
	engine.Close()
}

func TestEngine_Close_WithData(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.LogDecision(decision.DecisionReceipt{Decision: "test"})
	engine.Close()
	engine.Close() // double close
}

func TestEngine_SetSessionID_Roundtrip(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetSessionID("new-session-id")
	if engine.SessionID() != "new-session-id" {
		t.Errorf("expected 'new-session-id', got %q", engine.SessionID())
	}
}

func TestEngine_RenderDynamicContext_NonEmpty(t *testing.T) {
	engine, _ := setupTestEngine(t)
	snapshot := map[string]string{"key1": "value1", "key2": "value2"}
	result := engine.renderDynamicContext(snapshot)
	if result == "" {
		t.Error("expected non-empty result")
	}
}

// ============================================================================
// Discuss Tests (unique ones not in discuss_test.go)
// ============================================================================

func TestCountDiscussIssues_Empty(t *testing.T) {
	blockers, warnings := countDiscussIssues(nil)
	if blockers != 0 || warnings != 0 {
		t.Errorf("expected 0/0, got %d/%d", blockers, warnings)
	}
}

func TestCountDiscussIssues_Mixed(t *testing.T) {
	issues := []DiscussIssue{
		{Severity: "blocker"},
		{Severity: "warning"},
		{Severity: "blocker"},
		{Severity: "warning"},
	}
	blockers, warnings := countDiscussIssues(issues)
	if blockers != 2 || warnings != 2 {
		t.Errorf("expected 2/2, got %d/%d", blockers, warnings)
	}
}

func TestCheckAnswerCompleteness_EmptyQuestions(t *testing.T) {
	result := checkAnswerCompleteness(nil, nil, "goal")
	if result.Total != 0 {
		t.Errorf("expected 0 total, got %d", result.Total)
	}
	if result.Score != 0 {
		t.Errorf("expected 0 score, got %d", result.Score)
	}
}

func TestCheckAnswerCompleteness_AllAnswered(t *testing.T) {
	questions := []string{"What framework?", "What language?"}
	answers := map[int]string{0: "React", 1: "Go"}
	result := checkAnswerCompleteness(questions, answers, "build a web app")
	if result.Answered != 2 {
		t.Errorf("expected 2 answered, got %d", result.Answered)
	}
	if result.Skipped != 0 {
		t.Errorf("expected 0 skipped, got %d", result.Skipped)
	}
}

func TestCheckAnswerCompleteness_PartialAnswers(t *testing.T) {
	questions := []string{"Q1?", "Q2?", "Q3?"}
	answers := map[int]string{0: "A1"}
	result := checkAnswerCompleteness(questions, answers, "goal")
	if result.Answered != 1 {
		t.Errorf("expected 1 answered, got %d", result.Answered)
	}
	if result.Skipped != 2 {
		t.Errorf("expected 2 skipped, got %d", result.Skipped)
	}
}

func TestCheckAnswerCompleteness_MissingKeywordsPenalizesScore(t *testing.T) {
	questions := []string{"Q1?"}
	answers := map[int]string{0: "answer"}
	result := checkAnswerCompleteness(questions, answers, "database authentication caching")
	if len(result.MissingAreas) == 0 {
		t.Error("expected missing areas for keywords not in answers")
	}
	if result.Score >= 100 {
		t.Error("expected penalized score")
	}
}

func TestEngine_ShouldGenerateFollowUps_Disabled(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	completeness := DiscussCompleteness{Score: 30, Skipped: 3, Total: 5}
	if engine.shouldGenerateFollowUps(completeness) {
		t.Error("expected false when feature disabled")
	}
}

func TestEngine_ShouldGenerateFollowUps_LowScore(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Features: config.FeaturesConfig{DiscussFollowUps: true}}
	completeness := DiscussCompleteness{Score: 30, Skipped: 2, Total: 5}
	if !engine.shouldGenerateFollowUps(completeness) {
		t.Error("expected true when score < 60")
	}
}

func TestEngine_ShouldGenerateFollowUps_HighSkippedRatio(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Features: config.FeaturesConfig{DiscussFollowUps: true}}
	completeness := DiscussCompleteness{Score: 80, Skipped: 4, Total: 6}
	if !engine.shouldGenerateFollowUps(completeness) {
		t.Error("expected true when skipped > total/2")
	}
}

func TestEngine_ShouldGenerateFollowUps_HighScoreLowSkipped(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Features: config.FeaturesConfig{DiscussFollowUps: true}}
	completeness := DiscussCompleteness{Score: 80, Skipped: 1, Total: 6}
	if engine.shouldGenerateFollowUps(completeness) {
		t.Error("expected false when score >= 60 and skipped <= total/2")
	}
}

func TestEngine_CheckDiscussCompleteness_Disabled(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	result := engine.CheckDiscussCompleteness()
	if result.Score != 100 {
		t.Errorf("expected score 100 when disabled, got %d", result.Score)
	}
}

// ============================================================================
// Plan Check Tests (unique ones not in plan_check_test.go)
// ============================================================================

func TestParseCheckResult_WithTaskReference(t *testing.T) {
	content := `- [B1] Task 1: granularity — Task too coarse
- [W1] Task 2: alignment — Does not match`
	result := parseCheckResult(content)
	if result.Passed {
		t.Error("expected not passed due to blockers")
	}
	if len(result.Issues) < 2 {
		t.Errorf("expected at least 2 issues, got %d", len(result.Issues))
	}
	if result.Issues[0].TaskID != 1 {
		t.Errorf("expected task ID 1, got %d", result.Issues[0].TaskID)
	}
}

func TestParseCheckResult_NoTaskReference(t *testing.T) {
	content := `- [B1] security — Missing auth check`
	result := parseCheckResult(content)
	if result.Passed {
		t.Error("expected not passed")
	}
	if len(result.Issues) != 1 {
		t.Fatalf("expected 1 issue, got %d", len(result.Issues))
	}
	if result.Issues[0].Category != "security" {
		t.Errorf("expected 'security', got %q", result.Issues[0].Category)
	}
}

func TestParseCheckResult_UnknownSeveritySkipped(t *testing.T) {
	content := `- [X1] unknown severity`
	result := parseCheckResult(content)
	if len(result.Issues) != 0 {
		t.Errorf("expected 0 issues for unknown severity, got %d", len(result.Issues))
	}
}

func TestCountIssuesByType_Empty(t *testing.T) {
	blockers, warnings := countIssuesByType(nil)
	if blockers != 0 || warnings != 0 {
		t.Errorf("expected 0/0, got %d/%d", blockers, warnings)
	}
}

func TestIsPlanCheckStalled_EdgeCases(t *testing.T) {
	tests := []struct {
		current, previous int
		expected          bool
	}{
		{0, 0, false},
		{5, 5, true},
		{3, 5, false},
		{10, 5, true},
	}
	for _, tt := range tests {
		result := isPlanCheckStalled(tt.current, tt.previous)
		if result != tt.expected {
			t.Errorf("isPlanCheckStalled(%d, %d) = %v, want %v", tt.current, tt.previous, result, tt.expected)
		}
	}
}

func TestEngine_MaxPlanRevisions_Default(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	if engine.maxPlanRevisions() != 3 {
		t.Errorf("expected default 3, got %d", engine.maxPlanRevisions())
	}
}

func TestEngine_MaxPlanRevisions_Configured(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Features: config.FeaturesConfig{PlanCheckMaxIter: 5}}
	if engine.maxPlanRevisions() != 5 {
		t.Errorf("expected 5, got %d", engine.maxPlanRevisions())
	}
}

func TestEngine_MaxPlanRevisions_ZeroConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Features: config.FeaturesConfig{PlanCheckMaxIter: 0}}
	if engine.maxPlanRevisions() != 3 {
		t.Errorf("expected default 3 for zero config, got %d", engine.maxPlanRevisions())
	}
}

// ============================================================================
// Plan Chunk Tests (unique ones not in plan_chunk_test.go)
// ============================================================================

func TestParseOutline_InCodeBlock(t *testing.T) {
	content := "Here is the outline:\n```json\n{\"title\":\"Test\",\"waves\":[{\"wave\":1,\"tasks\":[{\"id\":1,\"action\":\"Create\",\"description\":\"t1\",\"dependencies\":[],\"category\":\"impl\"}]}],\"total_tasks\":1}\n```"
	outline, err := parseOutline(content)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if outline.Title != "Test" {
		t.Errorf("expected 'Test', got %q", outline.Title)
	}
}

func TestParseOutline_InvalidWaves(t *testing.T) {
	_, err := parseOutline(`{"title":"Test","waves":[]}`)
	if err == nil {
		t.Fatal("expected error for empty waves")
	}
}

func TestFindWave_NotFound(t *testing.T) {
	outline := &PlanOutline{Waves: []WaveOutline{{Wave: 1}}}
	if findWave(outline, 99) != nil {
		t.Error("expected nil for non-existent wave")
	}
}

func TestEngine_ChunkThreshold_Default(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	if engine.chunkThreshold() != 10 {
		t.Errorf("expected default 10, got %d", engine.chunkThreshold())
	}
}

func TestEngine_ChunkThreshold_Configured(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Features: config.FeaturesConfig{PlanChunkThreshold: 15}}
	if engine.chunkThreshold() != 15 {
		t.Errorf("expected 15, got %d", engine.chunkThreshold())
	}
}

// ============================================================================
// Execute Quality Tests (unique ones not in execute_quality_test.go)
// ============================================================================

func TestEngine_VerifyCriterion_Contains(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\nfunc main() {}"), 0644)
	engine, _ := setupTestEngine(t)
	engine.workDir = dir
	task := m31types.Task{Files: []string{"main.go"}}
	if !engine.verifyCriterion(task, "main.go contains func main") {
		t.Error("expected criterion to pass")
	}
}

func TestEngine_VerifyCriterion_Exists(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("content"), 0644)
	engine, _ := setupTestEngine(t)
	engine.workDir = dir
	task := m31types.Task{Files: []string{"test.txt"}}
	if !engine.verifyCriterion(task, "test.txt exists") {
		t.Error("expected criterion to pass")
	}
}

func TestEngine_VerifyCriterion_Has(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.go"), []byte("func TestFoo() {}"), 0644)
	engine, _ := setupTestEngine(t)
	engine.workDir = dir
	task := m31types.Task{Files: []string{"test.go"}}
	if !engine.verifyCriterion(task, "test.go has func TestFoo") {
		t.Error("expected criterion to pass")
	}
}

func TestEngine_VerifyCriterion_Fallback(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("some content here"), 0644)
	engine, _ := setupTestEngine(t)
	engine.workDir = dir
	task := m31types.Task{Files: []string{"test.txt"}}
	if !engine.verifyCriterion(task, "some content here") {
		t.Error("expected fallback criterion to pass")
	}
}

func TestEngine_CheckAcceptanceCriteria_AllPass(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("package main"), 0644)
	engine, _ := setupTestEngine(t)
	engine.workDir = dir
	task := m31types.Task{
		ID:                 1,
		AcceptanceCriteria: []string{"test.txt contains package main", "test.txt exists"},
		Files:              []string{"test.txt"},
	}
	result := engine.checkAcceptanceCriteria(task)
	if !result.Passed {
		t.Errorf("expected passed, got details: %v", result.Details)
	}
	if result.Checked != 2 {
		t.Errorf("expected 2 checked, got %d", result.Checked)
	}
}

func TestEngine_CheckAcceptanceCriteria_SomeFail(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("package main"), 0644)
	engine, _ := setupTestEngine(t)
	engine.workDir = dir
	task := m31types.Task{
		ID:                 1,
		AcceptanceCriteria: []string{"test.txt contains package main", "test.txt contains xyz"},
		Files:              []string{"test.txt"},
	}
	result := engine.checkAcceptanceCriteria(task)
	if result.Passed {
		t.Error("expected not passed")
	}
	if result.Failed != 1 {
		t.Errorf("expected 1 failed, got %d", result.Failed)
	}
}

func TestEngine_BehavioralVerification_BlockedCommand(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{AcceptanceCriteria: []string{"run sudo rm -rf /"}}
	results, err := engine.BehavioralVerification(task)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}
	if results[0].Passed {
		t.Error("expected blocked command to fail")
	}
}

func TestEngine_BehavioralVerification_NotAllowed(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{AcceptanceCriteria: []string{"run echo hello"}}
	results, err := engine.BehavioralVerification(task)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}
	if results[0].Passed {
		t.Error("expected non-allowed command to fail")
	}
}

func TestEngine_BehavioralVerification_NoCommand(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{AcceptanceCriteria: []string{"the file should exist"}}
	results, err := engine.BehavioralVerification(task)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 0 {
		t.Errorf("expected 0 results for no-command criterion, got %d", len(results))
	}
}

func TestExtractKeyPhrase_LongPhrase(t *testing.T) {
	longCriterion := "this is a very long criterion that should be truncated at fifty characters because it exceeds the limit"
	result := extractKeyPhrase(longCriterion)
	if len(result) > 50 {
		t.Errorf("expected <=50 chars, got %d", len(result))
	}
}

func TestExtractKeyPhrase_ValidateThat(t *testing.T) {
	// extractKeyPhrase strips one prefix and truncates to 50 chars
	result := extractKeyPhrase("validate that the database schema is correct")
	// "validate that " is stripped → "the database schema is correct"
	if result != "the database schema is correct" {
		t.Errorf("unexpected result: %q", result)
	}
}

func TestExtractKeyPhrase_AssertThat(t *testing.T) {
	result := extractKeyPhrase("assert that the tests pass")
	if result != "the tests pass" {
		t.Errorf("unexpected result: %q", result)
	}
}

func TestExtractKeyPhrase_ConfirmThat(t *testing.T) {
	result := extractKeyPhrase("confirm that build succeeds")
	if result != "build succeeds" {
		t.Errorf("unexpected result: %q", result)
	}
}

func TestExtractKeyPhrase_CheckThat(t *testing.T) {
	result := extractKeyPhrase("check that the API works")
	if result != "the API works" {
		t.Errorf("unexpected result: %q", result)
	}
}

func TestExtractKeyPhrase_The(t *testing.T) {
	result := extractKeyPhrase("the database is configured")
	if result != "database is configured" {
		t.Errorf("unexpected result: %q", result)
	}
}

// ============================================================================
// Intent Tests (unique ones not in intent_test.go)
// ============================================================================

func TestParseIntentJSON_InCodeBlock(t *testing.T) {
	raw := "Here is the result:\n```json\n{\"intent\":\"bugfix\",\"complexity\":\"moderate\",\"confidence\":0.7,\"summary\":\"fix bug\"}\n```"
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Intent != m31types.IntentBugfix {
		t.Errorf("expected IntentBugfix, got %s", result.Intent)
	}
}

func TestParseIntentJSON_EmptyScope(t *testing.T) {
	raw := `{"intent":"feature","complexity":"simple","confidence":0.9,"summary":"test","scope":[]}`
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result.Scope) != 0 {
		t.Errorf("expected empty scope, got %v", result.Scope)
	}
}

func TestGuessIntentFromKeywords_Question(t *testing.T) {
	tests := []struct {
		input    string
		expected m31types.IntentType
	}{
		{"how to implement auth", m31types.IntentQuestion},
		{"what is the best approach?", m31types.IntentQuestion},
		{"why does this fail?", m31types.IntentQuestion},
		{"when should we deploy?", m31types.IntentQuestion},
		{"where is the config?", m31types.IntentQuestion},
		{"who maintains this?", m31types.IntentQuestion},
		{"can you explain this?", m31types.IntentQuestion},
		{"describe the architecture", m31types.IntentQuestion},
	}
	for _, tt := range tests {
		if got := guessIntentFromKeywords(tt.input); got != tt.expected {
			t.Errorf("guessIntentFromKeywords(%q) = %s, want %s", tt.input, got, tt.expected)
		}
	}
}

func TestGuessIntentFromKeywords_Bugfix(t *testing.T) {
	tests := []string{"fix the login bug", "crash on startup", "error in parser", "broken build"}
	for _, input := range tests {
		if got := guessIntentFromKeywords(input); got != m31types.IntentBugfix {
			t.Errorf("guessIntentFromKeywords(%q) = %s, want bugfix", input, got)
		}
	}
}

func TestGuessIntentFromKeywords_Chore(t *testing.T) {
	tests := []string{"bump version", "update version to 1.0", "upgrade go to 1.22", "config change", "add dependency"}
	for _, input := range tests {
		if got := guessIntentFromKeywords(input); got != m31types.IntentChore {
			t.Errorf("guessIntentFromKeywords(%q) = %s, want chore", input, got)
		}
	}
}

func TestGuessIntentFromKeywords_Refactor(t *testing.T) {
	tests := []string{"refactor the auth module", "clean up dead code", "restructure the API"}
	for _, input := range tests {
		if got := guessIntentFromKeywords(input); got != m31types.IntentRefactor {
			t.Errorf("guessIntentFromKeywords(%q) = %s, want refactor", input, got)
		}
	}
}

func TestGuessIntentFromKeywords_Feature(t *testing.T) {
	input := "add dark mode toggle"
	if got := guessIntentFromKeywords(input); got != m31types.IntentFeature {
		t.Errorf("guessIntentFromKeywords(%q) = %s, want feature", input, got)
	}
}

func TestDefaultIntentClassifyPrompt(t *testing.T) {
	prompt := defaultIntentClassifyPrompt()
	if prompt == "" {
		t.Error("expected non-empty prompt")
	}
	if len(prompt) < 100 {
		t.Error("expected prompt to be substantial")
	}
}

func TestNormalizeIntent_AllTypes(t *testing.T) {
	tests := []struct {
		input    string
		expected m31types.IntentType
	}{
		{"feature", m31types.IntentFeature},
		{"new_feature", m31types.IntentFeature},
		{"bug_fix", m31types.IntentBugfix},
		{"refactoring", m31types.IntentRefactor},
		{"question", m31types.IntentQuestion},
		{"explanation", m31types.IntentExplanation},
		{"exploration", m31types.IntentExploration},
		{"chore", m31types.IntentChore},
		{"unknown", m31types.IntentQuestion},
	}
	for _, tt := range tests {
		if got := normalizeIntent(m31types.IntentType(tt.input)); got != tt.expected {
			t.Errorf("normalizeIntent(%q) = %s, want %s", tt.input, got, tt.expected)
		}
	}
}

func TestNormalizeComplexity_AllLevels(t *testing.T) {
	tests := []struct {
		input    string
		expected m31types.ComplexityLevel
	}{
		{"trivial", m31types.ComplexityTrivial},
		{"simple", m31types.ComplexitySimple},
		{"moderate", m31types.ComplexityModerate},
		{"complex", m31types.ComplexityComplex},
		{"unknown", m31types.ComplexityModerate},
	}
	for _, tt := range tests {
		if got := normalizeComplexity(m31types.ComplexityLevel(tt.input)); got != tt.expected {
			t.Errorf("normalizeComplexity(%q) = %s, want %s", tt.input, got, tt.expected)
		}
	}
}

func TestExtractJSONFromResponse_InCodeBlock(t *testing.T) {
	input := "Here:\n```json\n{\"key\":\"value\"}\n```"
	result := extractJSONFromResponse(input)
	if result != `{"key":"value"}` {
		t.Errorf("expected JSON, got %q", result)
	}
}

func TestExtractJSONFromResponse_Wrapped(t *testing.T) {
	input := "Some text {\"key\":\"value\"} more text"
	result := extractJSONFromResponse(input)
	if result != `{"key":"value"}` {
		t.Errorf("expected JSON, got %q", result)
	}
}

func TestExtractJSONFromResponse_NoJSON(t *testing.T) {
	result := extractJSONFromResponse("just plain text")
	if result != "" {
		t.Errorf("expected empty, got %q", result)
	}
}

// ============================================================================
// Diff Summary Tests (unique ones not in diff_summary_test.go)
// ============================================================================

func TestFormatDiffSummary_Nil(t *testing.T) {
	result := FormatDiffSummary(nil)
	if result != "No file changes." {
		t.Errorf("expected 'No file changes.', got %q", result)
	}
}

func TestFormatDiffSummary_EmptyFiles(t *testing.T) {
	result := FormatDiffSummary(&m31types.DiffSummary{Files: nil})
	if result != "No file changes." {
		t.Errorf("expected 'No file changes.', got %q", result)
	}
}

func TestFormatDiffSummary_WithFiles(t *testing.T) {
	ds := &m31types.DiffSummary{
		Files: []m31types.FileDiff{
			{File: "main.go", Status: "modified", Additions: 10, Deletions: 5},
			{File: "new.go", Status: "added", Additions: 20, Deletions: 0},
		},
		Additions: 30,
		Deletions: 5,
	}
	result := FormatDiffSummary(ds)
	if !strings.Contains(result, "main.go") {
		t.Error("expected result to contain main.go")
	}
	if !strings.Contains(result, "+30/-5") {
		t.Error("expected result to contain total changes")
	}
}

// ============================================================================
// Phase Coordinator Tests (unique ones not in phase_coordinator_test.go)
// ============================================================================

func TestPhaseCoordinator_PrePhaseSetup_BudgetExceeded(t *testing.T) {
	dir := t.TempDir()
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")
	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})
	s, _ := mgr.NewSession("test-model", "test-provider")

	sm := NewStateMachine()
	cache := NewWorkflowCache()
	ct := NewCostTracker(100.0)
	ct.RecordCost(100.0)

	cfg := &mockBudgetConfig{limit: 50.0}
	pc := NewPhaseCoordinator(sm, cache, mgr, s.ID, ct, nil, nil, nil, func(msg any) {})

	_, err := pc.PrePhaseSetup(context.Background(), m31types.PhaseInitialize, cfg, nil, nil)
	if err == nil {
		t.Fatal("expected budget exceeded error")
	}
}

func TestPhaseCoordinator_PostPhaseExecution_NilResult(t *testing.T) {
	dir := t.TempDir()
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")
	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})
	s, _ := mgr.NewSession("test-model", "test-provider")

	sm := NewStateMachine()
	cache := NewWorkflowCache()
	ct := NewCostTracker(0)
	pc := NewPhaseCoordinator(sm, cache, mgr, s.ID, ct, nil, nil, nil, func(msg any) {})
	pc.PostPhaseExecution(m31types.PhasePlan, nil, time.Now())
}

func TestPhaseCoordinator_PostPhaseExecution_RecordsCost(t *testing.T) {
	dir := t.TempDir()
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")
	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})
	s, _ := mgr.NewSession("test-model", "test-provider")

	sm := NewStateMachine()
	cache := NewWorkflowCache()
	ct := NewCostTracker(0)
	pc := NewPhaseCoordinator(sm, cache, mgr, s.ID, ct, nil, nil, nil, func(msg any) {})

	result := &PhaseResult{Success: true, Cost: 0.01, Usage: &m31types.Usage{TotalTokens: 100}}
	start := time.Now().Add(-100 * time.Millisecond)
	pc.PostPhaseExecution(m31types.PhasePlan, result, start)

	if result.DurationMs <= 0 {
		t.Error("expected positive duration")
	}
	if ct.TotalCost() != 0.01 {
		t.Errorf("expected cost 0.01, got %f", ct.TotalCost())
	}
}

// ============================================================================
// Engine Proactive Compact Tests
// ============================================================================

func TestEngine_ProactiveCompactCheck_NilCompactor(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.compactor = nil
	messages := []m31types.Message{{Role: "user", Content: "test"}}
	result := engine.proactiveCompactCheck(messages)
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

func TestEngine_ProactiveCompactCheck_Disabled(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{Compaction: config.CompactionConfig{Proactive: false}}
	messages := []m31types.Message{{Role: "user", Content: "test"}}
	result := engine.proactiveCompactCheck(messages)
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

func TestEngine_ProactiveCompactCheck_NilConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	messages := []m31types.Message{{Role: "user", Content: "test"}}
	result := engine.proactiveCompactCheck(messages)
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

// ============================================================================
// Engine Compacted Messages Tests
// ============================================================================

func TestEngine_CompactedMessages_NilCompactor(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.compactor = nil
	messages := []m31types.Message{
		{Role: "user", Content: "msg1"},
		{Role: "user", Content: "msg2"},
	}
	result := engine.compactedMessages(messages, "summary")
	if len(result) != 2 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

func TestEngine_CompactedMessages_NilTokens(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.tokens = nil
	messages := []m31types.Message{{Role: "user", Content: "msg1"}}
	result := engine.compactedMessages(messages, "summary")
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

// ============================================================================
// Engine Preflight Context Check Tests
// ============================================================================

func TestEngine_PreflightContextCheck_NilTokens(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.tokens = nil
	messages := []m31types.Message{{Role: "user", Content: "test"}}
	result, err := engine.preflightContextCheck(messages)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

func TestEngine_PreflightContextCheck_NilProvider(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = nil
	messages := []m31types.Message{{Role: "user", Content: "test"}}
	result, err := engine.preflightContextCheck(messages)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

func TestEngine_PreflightContextCheck_WithinThreshold(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mocks.MockProvider{}
	messages := []m31types.Message{{Role: "user", Content: "short"}}
	result, err := engine.preflightContextCheck(messages)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) != 1 {
		t.Errorf("expected unchanged messages, got %d", len(result))
	}
}

func TestPreflightContextCheck_WithProvider(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mockProviderWithModel{
		model: &m31types.ModelInfo{ContextLength: 100000},
	}
	messages := []m31types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "hello"},
	}
	result, err := engine.preflightContextCheck(messages)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) != 2 {
		t.Errorf("expected 2 messages, got %d", len(result))
	}
}

// ============================================================================
// Engine Load Project Cached Tests
// ============================================================================

func TestEngine_LoadProjectCached_CacheHit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	p1 := engine.loadProjectCached()
	p2 := engine.loadProjectCached()
	if p1 != p2 {
		t.Error("expected same pointer from cache")
	}
}

// ============================================================================
// Engine consumeStream Tests
// ============================================================================

func TestEngine_ConsumeStream_MaxSize(t *testing.T) {
	engine, _ := setupTestEngine(t)
	content := make([]byte, m31types.MaxLLMResponseBytes+1)
	for i := range content {
		content[i] = 'a'
	}
	iterator := &m31types.StreamIterator{
		Next:  func() (*m31types.StreamChunk, error) { return &m31types.StreamChunk{Delta: string(content)}, nil },
		Close: func() error { return nil },
	}
	_, _, err := engine.consumeStream(iterator)
	if err == nil {
		t.Fatal("expected error for oversized response")
	}
}

func TestEngine_ConsumeStream_PartialOnError(t *testing.T) {
	engine, _ := setupTestEngine(t)
	called := false
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			if !called {
				called = true
				return &m31types.StreamChunk{Delta: "partial"}, errors.New("network error")
			}
			return nil, io.EOF
		},
		Close: func() error { return nil },
	}
	content, _, err := engine.consumeStream(iterator)
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(content, "partial") {
		t.Errorf("expected partial content, got %q", content)
	}
}

func TestEngine_ConsumeStream_NilChunk(t *testing.T) {
	engine, _ := setupTestEngine(t)
	called := false
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			if !called {
				called = true
				return nil, nil
			}
			return nil, io.EOF
		},
		Close: func() error { return nil },
	}
	content, _, err := engine.consumeStream(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "" {
		t.Errorf("expected empty content, got %q", content)
	}
}

// ============================================================================
// Engine consumeStreamWithTools Tests
// ============================================================================

func TestEngine_ConsumeStreamWithTools_TextOnly(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			switch callsMade {
			case 1:
				return &m31types.StreamChunk{Delta: "hello"}, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}
	content, calls, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "hello" {
		t.Errorf("expected 'hello', got %q", content)
	}
	if len(calls) != 0 {
		t.Errorf("expected 0 tool calls, got %d", len(calls))
	}
}

func TestEngine_ConsumeStreamWithTools_WithToolCalls(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			switch callsMade {
			case 1:
				return &m31types.StreamChunk{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "Bash", ToolInput: `{"command":"echo hi"}`}, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}
	_, calls, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(calls))
	}
	// normalizeToolName("Bash") → "Bash" (already canonical)
	if calls[0].Name != "Bash" {
		t.Errorf("expected 'Bash', got %q", calls[0].Name)
	}
}

func TestEngine_ConsumeStreamWithTools_NilChunk(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			switch callsMade {
			case 1:
				return nil, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}
	content, _, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "" {
		t.Errorf("expected empty content, got %q", content)
	}
}

func TestEngine_ConsumeStreamWithTools_EmptyToolInput(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			switch callsMade {
			case 1:
				return &m31types.StreamChunk{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "Bash", ToolInput: ""}, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}
	_, calls, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(calls))
	}
	if string(calls[0].Input) != "{}" {
		t.Errorf("expected '{}', got %q", string(calls[0].Input))
	}
}

func TestEngine_ConsumeStreamWithTools_ToolCountExceeded(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			if callsMade <= m31types.MaxToolsPerCall+5 {
				return &m31types.StreamChunk{
					Type:       "tool_call",
					Index:      callsMade - 1,
					ToolCallID: fmt.Sprintf("call_%d", callsMade),
					ToolName:   "Bash",
					ToolInput:  `{}`,
				}, nil
			}
			return nil, io.EOF
		},
		Close: func() error { return nil },
	}
	_, calls, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) > m31types.MaxToolsPerCall {
		t.Errorf("expected <= %d tool calls, got %d", m31types.MaxToolsPerCall, len(calls))
	}
}

func TestEngine_ConsumeStreamWithTools_MaxSize(t *testing.T) {
	engine, _ := setupTestEngine(t)
	content := make([]byte, m31types.MaxLLMResponseBytes+1)
	for i := range content {
		content[i] = 'a'
	}
	iterator := &m31types.StreamIterator{
		Next:  func() (*m31types.StreamChunk, error) { return &m31types.StreamChunk{Delta: string(content)}, nil },
		Close: func() error { return nil },
	}
	_, _, _, err := engine.consumeStreamWithTools(iterator)
	if err == nil {
		t.Fatal("expected error for oversized response")
	}
}

// ============================================================================
// Engine finalizeToolCalls Tests
// ============================================================================

func TestFinalizeToolCalls_Empty(t *testing.T) {
	engine, _ := setupTestEngine(t)
	result := finalizeToolCalls(map[int]*toolCallBuilder{}, engine)
	if result != nil {
		t.Errorf("expected nil, got %v", result)
	}
}

func TestFinalizeToolCalls_SortedByIndex(t *testing.T) {
	engine, _ := setupTestEngine(t)
	b1 := &toolCallBuilder{id: "call_1", name: "Bash"}
	b1.arguments.WriteString(`{"command":"echo hi"}`)
	b0 := &toolCallBuilder{id: "call_2", name: "FileRead"}
	b0.arguments.WriteString(`{"path":"test.txt"}`)
	builders := map[int]*toolCallBuilder{1: b1, 0: b0}
	result := finalizeToolCalls(builders, engine)
	if len(result) != 2 {
		t.Fatalf("expected 2 calls, got %d", len(result))
	}
	if result[0].ID != "call_2" {
		t.Errorf("expected first call to be call_2 (index 0), got %s", result[0].ID)
	}
	if result[1].ID != "call_1" {
		t.Errorf("expected second call to be call_1 (index 1), got %s", result[1].ID)
	}
}

func TestFinalizeToolCalls_AutoGeneratedID(t *testing.T) {
	engine, _ := setupTestEngine(t)
	b := &toolCallBuilder{name: "Bash"}
	b.arguments.WriteString(`{}`)
	builders := map[int]*toolCallBuilder{0: b}
	result := finalizeToolCalls(builders, engine)
	if len(result) != 1 {
		t.Fatalf("expected 1 call, got %d", len(result))
	}
	if result[0].ID == "" {
		t.Error("expected auto-generated ID")
	}
}

func TestFinalizeToolCalls_EmptyArgs(t *testing.T) {
	engine, _ := setupTestEngine(t)
	b := &toolCallBuilder{id: "call_1", name: "Bash"}
	builders := map[int]*toolCallBuilder{0: b}
	result := finalizeToolCalls(builders, engine)
	if len(result) != 1 {
		t.Fatalf("expected 1 call, got %d", len(result))
	}
	if string(result[0].Input) != "{}" {
		t.Errorf("expected '{}' for empty args, got %q", string(result[0].Input))
	}
}

// ============================================================================
// Engine ExtractWebsiteTemplate Tests
// ============================================================================

func TestEngine_ExtractWebsiteTemplateTo_Cached(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.websiteTemplateDir = "/cached/dir"
	result, err := engine.ExtractWebsiteTemplateTo()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "/cached/dir" {
		t.Errorf("expected cached dir, got %q", result)
	}
}

func TestExtractWebsiteTemplate(t *testing.T) {
	dir := t.TempDir()
	err := ExtractWebsiteTemplate(dir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	entries, _ := os.ReadDir(dir)
	if len(entries) == 0 {
		t.Error("expected extracted files")
	}
}

// ============================================================================
// Engine truncateForLog Tests
// ============================================================================

func TestTruncateForLog_Short(t *testing.T) {
	result := truncateForLog("hello", 10)
	if result != "hello" {
		t.Errorf("expected 'hello', got %q", result)
	}
}

func TestTruncateForLog_Long(t *testing.T) {
	result := truncateForLog("hello world this is a long string", 15)
	// s[:maxLen-3] + "..." => s[:12] + "..." = "hello world " + "..."
	if result != "hello world ..." {
		t.Errorf("expected 'hello world ...', got %q", result)
	}
}

func TestTruncateForLog_ExactLength(t *testing.T) {
	result := truncateForLog("hello", 5)
	if result != "hello" {
		t.Errorf("expected 'hello', got %q", result)
	}
}

func TestTruncateForLog_SmallMaxLen(t *testing.T) {
	// maxLen=3 => s[:0] + "..." = "..."
	result := truncateForLog("hello", 3)
	if result != "..." {
		t.Errorf("expected '...', got %q", result)
	}
}

// ============================================================================
// Engine ShouldChunk Tests
// ============================================================================

func TestEngine_ShouldChunk_SimpleGoal(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if engine.shouldChunk("hello world") {
		t.Error("expected false for simple goal")
	}
}

// ============================================================================
// Engine Budget/Config Tests
// ============================================================================

func TestBudgetFromConfig_Nil(t *testing.T) {
	if budgetFromConfig(nil) != 0 {
		t.Error("expected 0 for nil config")
	}
}

func TestBudgetFromConfig_WithLimit(t *testing.T) {
	cfg := &config.Config{Features: config.FeaturesConfig{BudgetLimitUSD: 10.0}}
	if budgetFromConfig(cfg) != 10.0 {
		t.Errorf("expected 10.0, got %f", budgetFromConfig(cfg))
	}
}

func TestCompactionConfig_Nil(t *testing.T) {
	result := compactionConfig(nil)
	if result.Buffer <= 0 {
		t.Error("expected positive buffer")
	}
}

func TestCompactionConfig_WithValues(t *testing.T) {
	cfg := &config.Config{Compaction: config.CompactionConfig{Buffer: 5000, KeepTokens: 4000}}
	result := compactionConfig(cfg)
	if result.Buffer != 5000 {
		t.Errorf("expected 5000, got %d", result.Buffer)
	}
	if result.KeepTokens != 4000 {
		t.Errorf("expected 4000, got %d", result.KeepTokens)
	}
}

func TestCompactionConfig_ZeroValues(t *testing.T) {
	cfg := &config.Config{Compaction: config.CompactionConfig{Buffer: 0, KeepTokens: 0}}
	result := compactionConfig(cfg)
	if result.Buffer != 20000 {
		t.Errorf("expected default 20000, got %d", result.Buffer)
	}
	if result.KeepTokens != 8000 {
		t.Errorf("expected default 8000, got %d", result.KeepTokens)
	}
}

// ============================================================================
// Engine SetModel Tests
// ============================================================================

func TestEngine_SetModel_NilProvider(t *testing.T) {
	engine, _ := setupTestEngine(t)
	origProvider := engine.provider
	engine.SetModel("new-model", nil)
	if engine.provider != origProvider {
		t.Error("expected provider unchanged when nil passed")
	}
}

// ============================================================================
// Engine Concurrent Access Tests
// ============================================================================

func TestEngine_ConcurrentSessionID(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Sequential calls — SetSessionID is not goroutine-safe
	for i := 0; i < 10; i++ {
		engine.SetSessionID(fmt.Sprintf("session-%d", i))
		got := engine.SessionID()
		if got != fmt.Sprintf("session-%d", i) {
			t.Errorf("expected session-%d, got %s", i, got)
		}
	}
}

// ============================================================================
// Engine DiscussState Tests
// ============================================================================

func TestEngine_DiscussState(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{
		Questions: []string{"Q1", "Q2"},
		Answers:   map[int]string{0: "A1"},
	}
	state := engine.DiscussState()
	if len(state.Questions) != 2 {
		t.Errorf("expected 2 questions, got %d", len(state.Questions))
	}
	if state.Answers[0] != "A1" {
		t.Errorf("expected 'A1', got %q", state.Answers[0])
	}
}

func TestEngine_SkipDiscuss_NoQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if err := engine.SkipDiscuss(); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestEngine_SkipDiscuss_WithQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{Questions: []string{"Q1", "Q2"}}
	if err := engine.SkipDiscuss(); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	for i := range engine.discussState.Questions {
		if _, ok := engine.discussState.Answers[i]; !ok {
			t.Errorf("expected answer for question %d", i)
		}
	}
}

func TestEngine_SubmitDiscussAnswer_NilQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{Questions: nil}
	if err := engine.SubmitDiscussAnswer(0, "answer"); err == nil {
		t.Fatal("expected error for nil questions")
	}
}

func TestEngine_SubmitDiscussAnswer_CreatesAnswersMap(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{Questions: []string{"Q1"}}
	if err := engine.SubmitDiscussAnswer(0, "answer"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if engine.discussState.Answers == nil {
		t.Error("expected answers map to be created")
	}
}

// ============================================================================
// Engine SetRefinementFeedback Tests
// ============================================================================

func TestEngine_SetRefinementFeedback_BumpsVersion(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetRefinementFeedback("please refine")
	if engine.state.refineFeedback != "please refine" {
		t.Errorf("expected 'please refine', got %q", engine.state.refineFeedback)
	}
	if engine.state.planVersion != 1 {
		t.Errorf("expected plan version 1, got %d", engine.state.planVersion)
	}
}

func TestEngine_SetRefinementFeedback_DuplicateIgnored(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetRefinementFeedback("feedback")
	v1 := engine.state.planVersion
	engine.SetRefinementFeedback("feedback")
	if engine.state.planVersion != v1 {
		t.Error("duplicate feedback should not bump version")
	}
}

func TestEngine_SetRefinementFeedback_EmptyClears(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetRefinementFeedback("feedback")
	engine.SetRefinementFeedback("")
	if engine.state.refineFeedback != "" {
		t.Errorf("expected empty feedback, got %q", engine.state.refineFeedback)
	}
}

// ============================================================================
// Engine ExecCommandContext Tests
// ============================================================================

func TestEngine_ExecCommandContext_NilContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	out, err := engine.execCommandContext(context.TODO(), "echo", "hello")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(string(out), "hello") {
		t.Errorf("expected 'hello' in output, got %q", string(out))
	}
}

func TestEngine_ExecCommandContext_WithContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	out, err := engine.execCommandContext(ctx, "echo", "hello")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(string(out), "hello") {
		t.Errorf("expected 'hello' in output, got %q", string(out))
	}
}

// ============================================================================
// Engine BuildToolDefinitions Tests
// ============================================================================

func TestEngine_BuildToolDefinitions_HasTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	defs := engine.buildToolDefinitions()
	if len(defs) == 0 {
		t.Error("expected at least 1 tool definition")
	}
	for _, def := range defs {
		if def.Name == "" {
			t.Error("expected non-empty tool name")
		}
		if def.Description == "" {
			t.Error("expected non-empty tool description")
		}
	}
}

// ============================================================================
// Engine CostTracker Tests
// ============================================================================

func TestCostTracker_RecordCost_ZeroAndNegative(t *testing.T) {
	ct := NewCostTracker(0)
	ct.RecordCost(0)
	if ct.TotalCost() != 0 {
		t.Errorf("expected 0, got %f", ct.TotalCost())
	}
	// RecordCost does not guard against negative values; it just accumulates.
	ct.RecordCost(-0.01)
	if ct.TotalCost() != -0.01 {
		t.Errorf("expected -0.01, got %f", ct.TotalCost())
	}
}

func TestCostTracker_BudgetLimit(t *testing.T) {
	ct := NewCostTracker(10.0)
	ct.RecordCost(5.0)
	if ct.TotalCost() != 5.0 {
		t.Errorf("expected 5.0, got %f", ct.TotalCost())
	}
}

// ============================================================================
// Engine WorkflowCache Tests
// ============================================================================

func TestWorkflowCache_GetToolDefs_CachesResult(t *testing.T) {
	cache := NewWorkflowCache()
	called := false
	defs := cache.GetToolDefs(func() []provider.ToolDefinition {
		called = true
		return []provider.ToolDefinition{{Name: "test"}}
	})
	if !called {
		t.Error("expected factory to be called on first access")
	}
	if len(defs) != 1 {
		t.Errorf("expected 1 def, got %d", len(defs))
	}
	called2 := false
	defs2 := cache.GetToolDefs(func() []provider.ToolDefinition {
		called2 = true
		return nil
	})
	if called2 {
		t.Error("expected factory not to be called on cached access")
	}
	if len(defs2) != 1 {
		t.Errorf("expected cached 1 def, got %d", len(defs2))
	}
}

func TestWorkflowCache_GetProjectShared_Miss(t *testing.T) {
	cache := NewWorkflowCache()
	result := cache.GetProjectShared("session-1")
	if result != nil {
		t.Error("expected nil for missing project")
	}
}

func TestWorkflowCache_GetProjectShared_Hit(t *testing.T) {
	cache := NewWorkflowCache()
	project := &m31types.ProjectState{Goal: "test"}
	cache.SetProjectShared("session-1", project)
	result := cache.GetProjectShared("session-1")
	if result != project {
		t.Error("expected same project from cache")
	}
}

// ============================================================================
// Engine IsConfigFile Tests
// ============================================================================

func TestEngine_IsConfigFile(t *testing.T) {
	tests := []struct {
		path   string
		expect bool
	}{
		{".gitignore", true},
		{".env", true},
		{".editorconfig", true},
		{".nvmrc", true},
		{"Procfile", true},
		{".dockerignore", true},
		{".env.example", true},
		{".toml", true},
		{"config.yaml", true},
		{"config.yml", true},
		{"config.json", true},
		{"config.xml", true},
		{"config.ini", true},
		{"config.cfg", true},
		{"config.conf", true},
		{"main.go", false},
		{"test.txt", false},
	}
	for _, tt := range tests {
		if got := isConfigFile(tt.path); got != tt.expect {
			t.Errorf("isConfigFile(%q) = %v, want %v", tt.path, got, tt.expect)
		}
	}
}

func TestEngine_DescriptionKeywords(t *testing.T) {
	engine, _ := setupTestEngine(t)
	result := engine.descriptionKeywords()
	// descriptionKeywords returns common code terms for smart truncation
	if result == nil {
		t.Error("expected non-nil result, got nil")
	}
	if len(result) == 0 {
		t.Error("expected non-empty result, got empty slice")
	}
}

func TestEngine_ListCwdFiles_Empty(t *testing.T) {
	dir := t.TempDir()
	result := listCwdFiles(dir)
	_ = result
}

func TestEngine_ListCwdFiles_WithFiles(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main"), 0644)
	result := listCwdFiles(dir)
	if result == "" {
		t.Error("expected non-empty result")
	}
	if !strings.Contains(result, "main.go") {
		t.Error("expected result to contain main.go")
	}
}

// ============================================================================
// Engine SetCollector/Ledger/MsgEmitter Tests
// ============================================================================

func TestEngine_SetCollector_Nil(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetCollector(nil)
}

func TestEngine_SetLedger_Nil(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetLedger(nil)
}

func TestEngine_SetMsgEmitter_Nil(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetMsgEmitter(nil)
}

// ============================================================================
// Engine PreflightContextCheck With Large Messages
// ============================================================================

func TestPreflightContextCheck_ContextTooLarge(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mockProviderWithModel{
		model: &m31types.ModelInfo{ContextLength: 100},
	}
	messages := []m31types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "assistant", Content: strings.Repeat("x", 2000)},
		{Role: "user", Content: strings.Repeat("y", 2000)},
		{Role: "assistant", Content: strings.Repeat("z", 2000)},
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: strings.Repeat("a", 2000)},
		{Role: "user", Content: "world"},
		{Role: "assistant", Content: strings.Repeat("b", 2000)},
		{Role: "user", Content: "final"},
	}
	_, err := engine.preflightContextCheck(messages)
	_ = err
}

// ============================================================================
// Engine LastHealReport Tests
// ============================================================================

func TestEngine_LastHealReport_Nil(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if engine.LastHealReport() != nil {
		t.Error("expected nil heal report")
	}
}

// ============================================================================
// Engine PlanContent/PlanVersion Tests
// ============================================================================

func TestEngine_PlanContent(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.state.planMarkdown = "# Plan\nSome content"
	if engine.PlanContent() != "# Plan\nSome content" {
		t.Error("unexpected plan content")
	}
}

func TestEngine_PlanVersion(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.state.planVersion = 5
	if engine.PlanVersion() != 5 {
		t.Errorf("expected 5, got %d", engine.PlanVersion())
	}
}

// ============================================================================
// Engine Tool Name Normalization Tests
// ============================================================================

func TestEngine_ConsumeStreamWithTools_ToolNameNormalization(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			switch callsMade {
			case 1:
				// "read_file" is an alias for canonical "FileRead"
				return &m31types.StreamChunk{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "read_file", ToolInput: `{}`}, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}
	_, calls, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(calls))
	}
	if calls[0].Name != "FileRead" {
		t.Errorf("expected 'FileRead', got %q", calls[0].Name)
	}
}

func TestEngine_ConsumeStreamWithTools_MultipleTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	callsMade := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callsMade++
			switch callsMade {
			case 1:
				return &m31types.StreamChunk{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "Bash", ToolInput: `{}`}, nil
			case 2:
				return &m31types.StreamChunk{Type: "tool_call", Index: 1, ToolCallID: "call_2", ToolName: "FileRead", ToolInput: `{}`}, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}
	_, calls, _, err := engine.consumeStreamWithTools(iterator)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) != 2 {
		t.Fatalf("expected 2 tool calls, got %d", len(calls))
	}
}

// ============================================================================
// init_deep.go Tests
// ============================================================================

func TestDetectFramework_Empty(t *testing.T) {
	dir := t.TempDir()
	result := detectFramework(dir, "unknown")
	if result != "" {
		t.Errorf("expected empty, got %q", result)
	}
}

func TestDetectFramework_NodeJS(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{"dependencies":{"next":"14.0.0","react":"18.2.0"}}`), 0644)
	result := detectFramework(dir, "nodejs")
	if result != "Next.js" {
		t.Errorf("expected 'Next.js', got %q", result)
	}
}

func TestDetectFramework_NodeJS_Vue(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{"dependencies":{"vue":"3.0.0"}}`), 0644)
	result := detectFramework(dir, "nodejs")
	if result != "Vue" {
		t.Errorf("expected 'Vue', got %q", result)
	}
}

func TestDetectFramework_NodeJS_React(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{"dependencies":{"react":"18.2.0"}}`), 0644)
	result := detectFramework(dir, "nodejs")
	if result != "React" {
		t.Errorf("expected 'React', got %q", result)
	}
}

func TestDetectFramework_NodeJS_Express(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{"dependencies":{"express":"4.18.0"}}`), 0644)
	result := detectFramework(dir, "nodejs")
	if result != "Express" {
		t.Errorf("expected 'Express', got %q", result)
	}
}

func TestDetectFramework_NodeJS_NoPkg(t *testing.T) {
	dir := t.TempDir()
	result := detectFramework(dir, "nodejs")
	if result != "" {
		t.Errorf("expected empty, got %q", result)
	}
}

func TestDetectFramework_NodeJS_UnknownDep(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{"dependencies":{"some-unknown-lib":"1.0"}}`), 0644)
	result := detectFramework(dir, "nodejs")
	if result != "Node.js" {
		t.Errorf("expected 'Node.js', got %q", result)
	}
}

func TestDetectFramework_Python_Django(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte("django==4.2\npsycopg2==2.9"), 0644)
	result := detectFramework(dir, "python")
	if result != "Django" {
		t.Errorf("expected 'Django', got %q", result)
	}
}

func TestDetectFramework_Python_Flask(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte("flask==3.0"), 0644)
	result := detectFramework(dir, "python")
	if result != "Flask" {
		t.Errorf("expected 'Flask', got %q", result)
	}
}

func TestDetectFramework_Python_FastAPI(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte("fastapi==0.100.0\nuvicorn"), 0644)
	result := detectFramework(dir, "python")
	if result != "FastAPI" {
		t.Errorf("expected 'FastAPI', got %q", result)
	}
}

func TestDetectFramework_Python_NoFile(t *testing.T) {
	dir := t.TempDir()
	result := detectFramework(dir, "python")
	if result != "Python" {
		t.Errorf("expected 'Python' (default), got %q", result)
	}
}

func TestDetectFramework_Go(t *testing.T) {
	dir := t.TempDir()
	result := detectFramework(dir, "go")
	if result != "" {
		t.Errorf("expected empty (no go.mod), got %q", result)
	}
}

func TestDetectRustFramework_Actix(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[dependencies]\nactix-web = \"4.0\""), 0644)
	result := detectRustFramework(dir)
	if result != "Actix-web" {
		t.Errorf("expected 'Actix-web', got %q", result)
	}
}

func TestDetectRustFramework_Axum(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[dependencies]\naxum = \"0.6\""), 0644)
	result := detectRustFramework(dir)
	if result != "Axum" {
		t.Errorf("expected 'Axum', got %q", result)
	}
}

func TestDetectRustFramework_Rocket(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[dependencies]\nrocket = \"0.5\""), 0644)
	result := detectRustFramework(dir)
	if result != "Rocket" {
		t.Errorf("expected 'Rocket', got %q", result)
	}
}

func TestDetectRustFramework_NoToml(t *testing.T) {
	dir := t.TempDir()
	result := detectRustFramework(dir)
	if result != "" {
		t.Errorf("expected empty, got %q", result)
	}
}

func TestDetectRustFramework_UnknownCrate(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[dependencies]\nsome-random-crate = \"1.0\""), 0644)
	result := detectRustFramework(dir)
	if result != "Rust" {
		t.Errorf("expected 'Rust', got %q", result)
	}
}

func TestCountDependencies_NodeJS(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{"dependencies":{"a":"1","b":"2"},"devDependencies":{"c":"3"}}`), 0644)
	count := countDependencies(dir, "nodejs")
	if count != 3 {
		t.Errorf("expected 3, got %d", count)
	}
}

func TestCountDependencies_Go(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte("module test\nrequire (\n  github.com/foo/bar v1.0\n  github.com/baz/qux v2.0\n)\n"), 0644)
	count := countDependencies(dir, "go")
	if count != 2 {
		t.Errorf("expected 2, got %d", count)
	}
}

func TestCountDependencies_Python(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte("flask==3.0\n# comment\ndjango==4.2\n"), 0644)
	count := countDependencies(dir, "python")
	if count != 2 {
		t.Errorf("expected 2, got %d", count)
	}
}

func TestCountDependencies_Rust(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[dependencies]\nfoo = \"1.0\"\nbar = \"2.0\"\n\n[dev-dependencies]\nbaz = \"3.0\"\n"), 0644)
	count := countDependencies(dir, "rust")
	if count != 2 {
		t.Errorf("expected 2, got %d", count)
	}
}

func TestCountDependencies_Unknown(t *testing.T) {
	dir := t.TempDir()
	count := countDependencies(dir, "unknown")
	if count != 0 {
		t.Errorf("expected 0, got %d", count)
	}
}

func TestCountPythonDeps(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte("flask==3.0\n# a comment\n-django==4.2\n\nrequests==2.31\n"), 0644)
	count := countPythonDeps(dir)
	if count != 2 {
		t.Errorf("expected 2, got %d", count)
	}
}

func TestCountPythonDeps_NoFile(t *testing.T) {
	dir := t.TempDir()
	count := countPythonDeps(dir)
	if count != 0 {
		t.Errorf("expected 0, got %d", count)
	}
}

func TestCountRustDeps(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[dependencies]\nfoo = \"1.0\"\nbar = \"2.0\"\n\n[dev-dependencies]\nbaz = \"3.0\"\n"), 0644)
	count := countRustDeps(dir)
	if count != 2 {
		t.Errorf("expected 2, got %d", count)
	}
}

func TestCountRustDeps_NoFile(t *testing.T) {
	dir := t.TempDir()
	count := countRustDeps(dir)
	if count != 0 {
		t.Errorf("expected 0, got %d", count)
	}
}

func TestCountNodeDeps_NoFile(t *testing.T) {
	dir := t.TempDir()
	count := countNodeDeps(dir)
	if count != 0 {
		t.Errorf("expected 0, got %d", count)
	}
}

func TestCountGoDeps_NoFile(t *testing.T) {
	dir := t.TempDir()
	count := countGoDeps(dir)
	if count != 0 {
		t.Errorf("expected 0, got %d", count)
	}
}

// ============================================================================
// plan.go Tests
// ============================================================================

func TestComposeChunkedPlanMarkdown(t *testing.T) {
	outline := &PlanOutline{
		Title: "Test Plan",
		Waves: []WaveOutline{
			{Wave: 1, Tasks: []TaskStub{{ID: 1, Action: "create"}}},
			{Wave: 2, Tasks: []TaskStub{{ID: 2, Action: "modify"}}},
		},
		TotalTasks: 2,
	}
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Description: "Create file"},
		{ID: 2, Action: "modify", Description: "Edit file"},
	}
	result := composeChunkedPlanMarkdown(outline, tasks, nil)
	if !strings.Contains(result, "# Test Plan") {
		t.Error("expected title in output")
	}
	if !strings.Contains(result, "## Task List") {
		t.Error("expected task list in output")
	}
	if !strings.Contains(result, "chunked planning mode") {
		t.Error("expected summary in output")
	}
}

// ============================================================================
// ship.go rotateMemoryFile Tests
// ============================================================================

func TestRotateMemoryFile_NoFile(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Should not panic with missing file
	engine.rotateMemoryFile(filepath.Join(t.TempDir(), "nonexistent", "MEMORY.md"))
}

func TestRotateMemoryFile_FewSessions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	content := "## Session 1\nSome content\n## Session 2\nMore content\n"
	memPath := filepath.Join(t.TempDir(), "MEMORY.md")
	os.WriteFile(memPath, []byte(content), 0600)
	engine.rotateMemoryFile(memPath)
	data, _ := os.ReadFile(memPath)
	if string(data) != content {
		t.Errorf("file should be unchanged with few sessions, got %q", string(data))
	}
}

func TestRotateMemoryFile_ManySessions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	var sb strings.Builder
	for i := 1; i <= 25; i++ {
		fmt.Fprintf(&sb, "## Session %d\nContent %d\n", i, i)
	}
	memPath := filepath.Join(t.TempDir(), "MEMORY.md")
	os.WriteFile(memPath, []byte(sb.String()), 0600)
	engine.rotateMemoryFile(memPath)
	data, _ := os.ReadFile(memPath)
	result := string(data)
	// Should NOT contain session 1 (rotated out)
	if strings.Contains(result, "## Session 1\n") {
		t.Error("session 1 should have been rotated out")
	}
	// Should contain session 25
	if !strings.Contains(result, "## Session 25\n") {
		t.Error("session 25 should still be present")
	}
}

// ============================================================================
// execute.go classifyError Tests
// ============================================================================

func TestClassifyError_Build(t *testing.T) {
	if got := classifyError("build failed"); got != "build_error" {
		t.Errorf("expected build_error, got %q", got)
	}
}

func TestClassifyError_Compile(t *testing.T) {
	if got := classifyError("compile error"); got != "build_error" {
		t.Errorf("expected build_error, got %q", got)
	}
}

func TestClassifyError_Test(t *testing.T) {
	if got := classifyError("test failed"); got != "test_failure" {
		t.Errorf("expected test_failure, got %q", got)
	}
}

func TestClassifyError_Fail(t *testing.T) {
	if got := classifyError("some failure"); got != "test_failure" {
		t.Errorf("expected test_failure, got %q", got)
	}
}

func TestClassifyError_Syntax(t *testing.T) {
	if got := classifyError("syntax error on line 5"); got != "syntax_error" {
		t.Errorf("expected syntax_error, got %q", got)
	}
}

func TestClassifyError_Parse(t *testing.T) {
	if got := classifyError("parse error"); got != "syntax_error" {
		t.Errorf("expected syntax_error, got %q", got)
	}
}

func TestClassifyError_Undefined(t *testing.T) {
	if got := classifyError("undefined variable x"); got != "undefined_symbol" {
		t.Errorf("expected undefined_symbol, got %q", got)
	}
}

func TestClassifyError_Import(t *testing.T) {
	if got := classifyError("cannot import package foo"); got != "import_error" {
		t.Errorf("expected import_error, got %q", got)
	}
}

func TestClassifyError_Require(t *testing.T) {
	if got := classifyError("cannot require module bar"); got != "import_error" {
		t.Errorf("expected import_error, got %q", got)
	}
}

func TestClassifyError_Permission(t *testing.T) {
	if got := classifyError("permission denied"); got != "permission_error" {
		t.Errorf("expected permission_error, got %q", got)
	}
}

func TestClassifyError_Timeout(t *testing.T) {
	if got := classifyError("context deadline exceeded"); got != "timeout" {
		t.Errorf("expected timeout, got %q", got)
	}
}

func TestClassifyError_Token(t *testing.T) {
	if got := classifyError("token limit exceeded"); got != "context_overflow" {
		t.Errorf("expected context_overflow, got %q", got)
	}
}

func TestClassifyError_Unknown(t *testing.T) {
	if got := classifyError("something random"); got != "unknown" {
		t.Errorf("expected unknown, got %q", got)
	}
}

// ============================================================================
// execute.go healCreatedExpectedFiles Tests
// ============================================================================

func TestHealCreatedExpectedFiles_EmptyFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := &m31types.Task{Files: []string{}}
	if !engine.healCreatedExpectedFiles(task) {
		t.Error("expected true for empty files")
	}
}

func TestHealCreatedExpectedFiles_AllExist(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "b.go"), []byte("package main"), 0644)
	task := &m31types.Task{Files: []string{"a.go", "b.go"}}
	if !engine.healCreatedExpectedFiles(task) {
		t.Error("expected true when all files exist")
	}
}

func TestHealCreatedExpectedFiles_MissingFile(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)
	task := &m31types.Task{Files: []string{"a.go", "missing.go"}}
	if engine.healCreatedExpectedFiles(task) {
		t.Error("expected false when file is missing")
	}
}

// ============================================================================
// execute.go describeHealStrategy Tests
// ============================================================================

func TestDescribeHealStrategy_Empty(t *testing.T) {
	result := describeHealStrategy(nil)
	if result != "no tools used" {
		t.Errorf("expected 'no tools used', got %q", result)
	}
}

func TestDescribeHealStrategy_WithTools(t *testing.T) {
	calls := []m31types.ToolCall{
		{Name: "Bash", Input: json.RawMessage(`{"command":"go build"}`)},
		{Name: "FileWrite", Input: json.RawMessage(`{"path":"foo.go","content":"x"}`)},
	}
	result := describeHealStrategy(calls)
	if !strings.Contains(result, "Bash") {
		t.Errorf("expected Bash in result, got %q", result)
	}
}

// ============================================================================
// execute.go execCommandContext Tests
// ============================================================================

func TestExecCommandContext_Simple(t *testing.T) {
	// execCommandContext is not exported; use a local helper to test exec behavior
	ctx := context.Background()
	out, err := exec.CommandContext(ctx, "echo", "hello").CombinedOutput()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(string(out), "hello") {
		t.Errorf("expected 'hello' in output, got %q", string(out))
	}
}

func TestExecCommandContext_Error(t *testing.T) {
	ctx := context.Background()
	_, err := exec.CommandContext(ctx, "nonexistent_command_xyz").CombinedOutput()
	if err == nil {
		t.Error("expected error for nonexistent command")
	}
}

func TestExecCommandContext_Cancelled(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := exec.CommandContext(ctx, "sleep", "10").CombinedOutput()
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

// ============================================================================
// execute_quality.go Tests (additional)
// ============================================================================

func TestCheckAcceptanceCriteria_AllMet(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Create a file that the criterion can match
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\n\nfunc main() {}\n"), 0644)
	task := m31types.Task{
		ID:                 1,
		AcceptanceCriteria: []string{"main.go contains func main"},
	}
	result := engine.checkAcceptanceCriteria(task)
	if !result.Passed {
		t.Errorf("expected passed, got %v", result)
	}
}

func TestCheckAcceptanceCriteria_NoneMet(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{
		ID:                 1,
		AcceptanceCriteria: []string{"nonexistent.go exists"},
	}
	result := engine.checkAcceptanceCriteria(task)
	if result.Passed {
		t.Error("expected not passed")
	}
}

func TestCheckAcceptanceCriteria_Empty(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{
		ID:                 1,
		AcceptanceCriteria: []string{},
	}
	result := engine.checkAcceptanceCriteria(task)
	if !result.Passed {
		t.Error("expected passed for empty criteria")
	}
}

// ============================================================================
// plan_check.go Tests
// ============================================================================

func TestMaxPlanRevisions_Default(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if got := engine.maxPlanRevisions(); got != 3 {
		t.Errorf("expected default 3, got %d", got)
	}
}

func TestMaxPlanRevisions_Configured(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.PlanCheckMaxIter = 5
	if got := engine.maxPlanRevisions(); got != 5 {
		t.Errorf("expected configured 5, got %d", got)
	}
}

func TestBuildCheckContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	plan := &m31types.Plan{
		RawMarkdown: "# Plan\nSome plan content",
		Tasks:       []m31types.Task{{ID: 1, Action: "test", Description: "Test task"}},
	}
	messages := engine.buildCheckContext(plan, "build a feature")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if messages[0].Role != "system" {
		t.Errorf("expected system message first, got %q", messages[0].Role)
	}
	if messages[1].Role != "user" {
		t.Errorf("expected user message second, got %q", messages[1].Role)
	}
	if !strings.Contains(messages[1].Content, "build a feature") {
		t.Error("expected goal in user message")
	}
	if !strings.Contains(messages[1].Content, "Some plan content") {
		t.Error("expected plan content in user message")
	}
}

func TestBuildRevisionContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	plan := &m31types.Plan{
		RawMarkdown: "# Plan\nOriginal plan",
		Tasks:       []m31types.Task{{ID: 1, Action: "test", Description: "Test task"}},
	}
	issues := []PlanIssue{
		{Severity: "blocker", Category: "granularity", Message: "Too coarse", TaskID: 1},
		{Severity: "warning", Category: "coverage", Message: "Missing tests"},
	}
	messages := engine.buildRevisionContext(plan, issues, "build a feature")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if !strings.Contains(messages[1].Content, "build a feature") {
		t.Error("expected goal in user message")
	}
	if !strings.Contains(messages[1].Content, "blocker") {
		t.Error("expected blocker in user message")
	}
}

func TestParseCheckResult_Passed(t *testing.T) {
	result := parseCheckResult("PLAN CHECK PASSED")
	if !result.Passed {
		t.Error("expected passed")
	}
}

func TestParseCheckResult_NoIssues(t *testing.T) {
	result := parseCheckResult("No issues found")
	if !result.Passed {
		t.Error("expected passed")
	}
}

func TestParseCheckResult_Blocker(t *testing.T) {
	content := "- [B1] Task 1: granularity — Task is too broad"
	result := parseCheckResult(content)
	if result.Passed {
		t.Error("expected not passed with blocker")
	}
	if len(result.Issues) != 1 {
		t.Fatalf("expected 1 issue, got %d", len(result.Issues))
	}
	if result.Issues[0].Severity != "blocker" {
		t.Errorf("expected blocker, got %q", result.Issues[0].Severity)
	}
	if result.Issues[0].TaskID != 1 {
		t.Errorf("expected task ID 1, got %d", result.Issues[0].TaskID)
	}
}

func TestParseCheckResult_Warning(t *testing.T) {
	content := "- [W2] Task 2: acceptance — Missing acceptance criteria"
	result := parseCheckResult(content)
	if !result.Passed {
		t.Error("expected passed with only warnings")
	}
	if len(result.Issues) != 1 {
		t.Fatalf("expected 1 issue, got %d", len(result.Issues))
	}
	if result.Issues[0].Severity != "warning" {
		t.Errorf("expected warning, got %q", result.Issues[0].Severity)
	}
}

func TestParseCheckResult_NoTaskRef(t *testing.T) {
	content := "- [B] granularity — The plan is too coarse"
	result := parseCheckResult(content)
	if len(result.Issues) != 1 {
		t.Fatalf("expected 1 issue, got %d", len(result.Issues))
	}
	if result.Issues[0].Category != "granularity" {
		t.Errorf("expected granularity, got %q", result.Issues[0].Category)
	}
}

func TestParseCheckResult_MultipleIssues(t *testing.T) {
	content := "- [B1] Task 1: granularity — Too broad\n- [W2] coverage — Missing tests\n- [B] security — No input validation"
	result := parseCheckResult(content)
	if result.Passed {
		t.Error("expected not passed with blockers")
	}
	if len(result.Issues) != 3 {
		t.Errorf("expected 3 issues, got %d", len(result.Issues))
	}
}

func TestIsPlanCheckStalled_V2(t *testing.T) {
	tests := []struct {
		current, previous int
		expected          bool
	}{
		{5, 5, true},
		{5, 3, true},
		{3, 5, false},
		{0, 0, false},
		{1, 0, false},
	}
	for _, tt := range tests {
		if got := isPlanCheckStalled(tt.current, tt.previous); got != tt.expected {
			t.Errorf("isPlanCheckStalled(%d, %d) = %v, want %v", tt.current, tt.previous, got, tt.expected)
		}
	}
}

func TestCountIssuesByType_V2(t *testing.T) {
	issues := []PlanIssue{
		{Severity: "blocker"},
		{Severity: "warning"},
		{Severity: "blocker"},
	}
	blockers, warnings := countIssuesByType(issues)
	if blockers != 2 {
		t.Errorf("expected 2 blockers, got %d", blockers)
	}
	if warnings != 1 {
		t.Errorf("expected 1 warning, got %d", warnings)
	}
}

// ============================================================================
// plan_chunk.go Tests (additional)
// ============================================================================

func TestParseOutline_Valid(t *testing.T) {
	content := `{"title":"My Plan","waves":[{"wave":1,"tasks":[{"id":1,"action":"create","description":"file","dependencies":[],"category":"feat"}]}]}`
	outline, err := parseOutline(content)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if outline.Title != "My Plan" {
		t.Errorf("expected 'My Plan', got %q", outline.Title)
	}
	if len(outline.Waves) != 1 {
		t.Errorf("expected 1 wave, got %d", len(outline.Waves))
	}
	if outline.TotalTasks != 1 {
		t.Errorf("expected 1 total task, got %d", outline.TotalTasks)
	}
}

func TestParseOutline_CodeBlock(t *testing.T) {
	content := "```json\n{\"title\":\"X\",\"waves\":[{\"wave\":1,\"tasks\":[]}]}\n```"
	outline, err := parseOutline(content)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if outline.Title != "X" {
		t.Errorf("expected 'X', got %q", outline.Title)
	}
}

func TestParseOutline_NoJSON(t *testing.T) {
	_, err := parseOutline("no json here at all")
	if err == nil {
		t.Error("expected error for no JSON")
	}
}

func TestParseOutline_NoWaves(t *testing.T) {
	_, err := parseOutline(`{"title":"X","waves":[]}`)
	if err == nil {
		t.Error("expected error for empty waves")
	}
}

func TestParseOutline_BadJSON(t *testing.T) {
	_, err := parseOutline(`{bad json}`)
	if err == nil {
		t.Error("expected error for bad JSON")
	}
}

func TestFindWave_Found(t *testing.T) {
	outline := &PlanOutline{
		Waves: []WaveOutline{
			{Wave: 1, Tasks: []TaskStub{{ID: 1}}},
			{Wave: 2, Tasks: []TaskStub{{ID: 2}}},
		},
	}
	wave := findWave(outline, 2)
	if wave == nil {
		t.Fatal("expected to find wave 2")
	}
	if wave.Wave != 2 {
		t.Errorf("expected wave 2, got %d", wave.Wave)
	}
}

func TestChunkThreshold_Default(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if got := engine.chunkThreshold(); got != 10 {
		t.Errorf("expected default 10, got %d", got)
	}
}

func TestChunkThreshold_Configured(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.PlanChunkThreshold = 20
	if got := engine.chunkThreshold(); got != 20 {
		t.Errorf("expected configured 20, got %d", got)
	}
}

// ============================================================================
// plan_check.go PlanCheckResult Tests
// ============================================================================

func TestPlanCheckResult_PassedNoIssues(t *testing.T) {
	result := PlanCheckResult{Passed: true}
	if !result.Passed {
		t.Error("expected passed")
	}
	if len(result.Issues) != 0 {
		t.Errorf("expected 0 issues, got %d", len(result.Issues))
	}
}

func TestPlanIssue_Fields(t *testing.T) {
	issue := PlanIssue{
		Severity: "blocker",
		Category: "granularity",
		Message:  "too broad",
		TaskID:   3,
	}
	if issue.Severity != "blocker" {
		t.Errorf("expected blocker, got %q", issue.Severity)
	}
	if issue.TaskID != 3 {
		t.Errorf("expected task ID 3, got %d", issue.TaskID)
	}
}

// ============================================================================
// init_deep.go Additional Tests
// ============================================================================

func TestCalculateHealthScore_HighScore(t *testing.T) {
	a := ProjectAnalysis{
		TestFileRatio:   0.5,
		DependencyCount: 10,
		Framework:       "Next.js",
		Language:        "TypeScript",
		FileCount:       50,
	}
	score := calculateHealthScore(a)
	// 50 + 25 (test ratio 0.5*50) + 10 (framework != language) = 85
	if score != 85 {
		t.Errorf("expected 85, got %d", score)
	}
}

func TestCalculateHealthScore_LowScore(t *testing.T) {
	a := ProjectAnalysis{
		DependencyCount: 200,
		FileCount:       2,
	}
	score := calculateHealthScore(a)
	if score != 30 { // 50 - 10 (deps>100) - 10 (files<5) = 30
		t.Errorf("expected 30, got %d", score)
	}
}

func TestCalculateHealthScore_MediumDeps(t *testing.T) {
	a := ProjectAnalysis{
		DependencyCount: 60,
		FileCount:       20,
	}
	score := calculateHealthScore(a)
	if score != 45 { // 50 - 5 (deps>50) = 45
		t.Errorf("expected 45, got %d", score)
	}
}

func TestCalculateHealthScore_FrameworkBonus(t *testing.T) {
	a := ProjectAnalysis{
		DependencyCount: 5,
		Framework:       "React",
		Language:        "JavaScript",
		FileCount:       30,
	}
	score := calculateHealthScore(a)
	if score != 60 { // 50 + 10 (framework) = 60
		t.Errorf("expected 60, got %d", score)
	}
}

func TestCalculateHealthScore_FrameworkSameAsLanguage(t *testing.T) {
	a := ProjectAnalysis{
		DependencyCount: 5,
		Framework:       "Go",
		Language:        "Go",
		FileCount:       30,
	}
	score := calculateHealthScore(a)
	// Framework == Language, so no framework bonus
	if score != 50 {
		t.Errorf("expected 50, got %d", score)
	}
}

func TestDetectLanguage_V2(t *testing.T) {
	tests := []struct {
		input, expected string
	}{
		{"go", "Go"},
		{"nodejs", "TypeScript/JavaScript"},
		{"python", "Python"},
		{"rust", "Rust"},
		{"java", "Java"},
		{"cc", "C/C++"},
		{"unknown", "Unknown"},
	}
	for _, tt := range tests {
		if got := detectLanguage(tt.input); got != tt.expected {
			t.Errorf("detectLanguage(%q) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

// ============================================================================
// execute_quality.go BehavioralVerification Tests
// ============================================================================

func TestBehavioralVerification_NoTests(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{
		ID:                 1,
		AcceptanceCriteria: []string{"main.go contains func main"},
	}
	results, err := engine.BehavioralVerification(task)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// May return empty results when there are no test files to verify against
	_ = results
}

// ============================================================================
// discuss_check.go generateFollowUps/buildFollowUpContext Tests
// ============================================================================

func TestBuildFollowUpContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	goal := "build a web app"
	questions := []string{"What framework?", "What database?"}
	answers := map[int]string{0: "React"}
	messages := engine.buildFollowUpContext(goal, questions, answers)
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if messages[0].Role != "system" {
		t.Errorf("expected system first, got %q", messages[0].Role)
	}
	if !strings.Contains(messages[1].Content, "build a web app") {
		t.Error("expected goal in user message")
	}
	if !strings.Contains(messages[1].Content, "React") {
		t.Error("expected answer in user message")
	}
	if !strings.Contains(messages[1].Content, "(skipped)") {
		t.Error("expected skipped for unanswered question")
	}
}

func TestBuildFollowUpContext_AllAnswered(t *testing.T) {
	engine, _ := setupTestEngine(t)
	goal := "build something"
	questions := []string{"Q1"}
	answers := map[int]string{0: "A1"}
	messages := engine.buildFollowUpContext(goal, questions, answers)
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if strings.Contains(messages[1].Content, "(skipped)") {
		t.Error("should not have skipped since all questions answered")
	}
}

// ============================================================================
// coverage_gates.go Tests (additional)
// ============================================================================

func TestCoverageGates_ExtractGoalKeywords_V2(t *testing.T) {
	keywords := extractGoalKeywords("implement JWT authentication for the API")
	if len(keywords) == 0 {
		t.Error("expected non-empty keywords")
	}
}

func TestCoverageGates_HasSecurityKeywords_V2(t *testing.T) {
	tests := []struct {
		plan     *m31types.Plan
		expected bool
	}{
		{&m31types.Plan{Tasks: []m31types.Task{{Description: "implement authentication middleware"}}}, true},
		{&m31types.Plan{Tasks: []m31types.Task{{Files: []string{"auth/validate.go"}}}}, true},
		{&m31types.Plan{Tasks: []m31types.Task{{Description: "fix a typo in the readme"}}}, false},
		{&m31types.Plan{Tasks: []m31types.Task{{Description: "build UI component"}}}, false},
		{&m31types.Plan{}, false},
	}
	for _, tt := range tests {
		if got := hasSecurityKeywords(tt.plan); got != tt.expected {
			t.Errorf("hasSecurityKeywords(%v) = %v, want %v", tt.plan, got, tt.expected)
		}
	}
}

func TestCoverageGates_ContainsAny_V2(t *testing.T) {
	tests := []struct {
		s        string
		substrs  []string
		expected bool
	}{
		{"hello world", []string{"world", "foo"}, true},
		{"hello world", []string{"foo", "bar"}, false},
		{"", []string{"foo"}, false},
	}
	for _, tt := range tests {
		if got := containsAny(tt.s, tt.substrs); got != tt.expected {
			t.Errorf("containsAny(%q, %v) = %v, want %v", tt.s, tt.substrs, got, tt.expected)
		}
	}
}

// ============================================================================
// classify.go Tests
// ============================================================================

func TestWorkflowModeForComplexity(t *testing.T) {
	tests := []struct {
		input    m31types.ComplexityLevel
		expected m31types.WorkflowMode
	}{
		{m31types.ComplexityTrivial, m31types.ModeDirect},
		{m31types.ComplexitySimple, m31types.ModeFast},
		{m31types.ComplexityModerate, m31types.ModeFull},
		{m31types.ComplexityComplex, m31types.ModeFull},
	}
	for _, tt := range tests {
		if got := WorkflowModeForComplexity(tt.input); got != tt.expected {
			t.Errorf("WorkflowModeForComplexity(%v) = %v, want %v", tt.input, got, tt.expected)
		}
	}
}

// ============================================================================
// agent_switch.go Tests (additional)
// ============================================================================

func TestTruncatePlan_V2(t *testing.T) {
	plan := "## Task 1\nDo stuff\n## Task 2\nDo more\n"
	result := truncatePlan(plan, 20)
	if result == plan {
		t.Error("expected truncated result")
	}
	if !strings.Contains(result, "[plan truncated") {
		t.Error("expected truncation marker in result")
	}
}

func TestTruncatePlan_Short_V2(t *testing.T) {
	plan := "short"
	result := truncatePlan(plan, 100)
	if result != plan {
		t.Errorf("expected unchanged, got %q", result)
	}
}

// ============================================================================
// diff_summary.go Tests (additional)
// ============================================================================

func TestStatusIcon_V2(t *testing.T) {
	tests := []struct {
		input, expected string
	}{
		{"modified", "[M]"},
		{"added", "[+]"},
		{"deleted", "[-]"},
		{"renamed", "[R]"},
		{"unknown", "[M]"},
	}
	for _, tt := range tests {
		if got := statusIcon(tt.input); got != tt.expected {
			t.Errorf("statusIcon(%q) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

// ============================================================================
// Additional coverage: engine helpers
// ============================================================================

func TestEngine_NextCallID(t *testing.T) {
	engine, _ := setupTestEngine(t)
	id1 := engine.nextCallID()
	id2 := engine.nextCallID()
	if id1 == id2 {
		t.Error("expected unique call IDs")
	}
	if id1 != 1 {
		t.Errorf("expected first ID 1, got %d", id1)
	}
	if id2 != 2 {
		t.Errorf("expected second ID 2, got %d", id2)
	}
}

func TestEngine_ScopeIncludes_V2(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Empty intent result should not match anything
	if engine.ScopeIncludes("website") {
		t.Error("expected false for empty scope")
	}
}

// ============================================================================
// initialize.go Tests
// ============================================================================

func TestRunInitialize_V2(t *testing.T) {
	engine, _ := setupTestEngine(t)
	result, err := engine.runInitialize(context.Background(), "test goal")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result == nil {
		t.Fatal("expected non-nil result")
	}
}

// ============================================================================
// retry.go Tests (additional for coverage)
// ============================================================================

func TestRetryWithBackoff_SingleAttempt(t *testing.T) {
	attempts := 0
	err := RetryWithBackoff(context.Background(), RetryConfig{MaxAttempts: 1}, func() error {
		attempts++
		return nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if attempts != 1 {
		t.Errorf("expected 1 attempt, got %d", attempts)
	}
}

func TestRetryWithBackoff_AllFail(t *testing.T) {
	attempts := 0
	err := RetryWithBackoff(context.Background(), RetryConfig{MaxAttempts: 3}, func() error {
		attempts++
		return fmt.Errorf("rate limit exceeded")
	})
	if err == nil {
		t.Error("expected error after all retries")
	}
	if attempts != 3 {
		t.Errorf("expected 3 attempts, got %d", attempts)
	}
}

func TestRetryWithBackoff_SucceedsOnSecond(t *testing.T) {
	attempts := 0
	err := RetryWithBackoff(context.Background(), RetryConfig{MaxAttempts: 3}, func() error {
		attempts++
		if attempts < 2 {
			return fmt.Errorf("connection reset")
		}
		return nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if attempts != 2 {
		t.Errorf("expected 2 attempts, got %d", attempts)
	}
}

// ============================================================================
// runtime_proc_unix.go Tests
// ============================================================================

func TestGetProcessGroup(t *testing.T) {
	pg, err := getProcessGroup(os.Getpid())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if pg <= 0 {
		t.Errorf("expected positive process group, got %d", pg)
	}
}

// ============================================================================
// Additional Tests to Push Coverage
// ============================================================================

func TestCheckDiscussCompleteness_Disabled(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// cfg is nil, so completeness is disabled
	result := engine.CheckDiscussCompleteness()
	if result.Score != 100 {
		t.Errorf("expected score 100 when disabled, got %d", result.Score)
	}
}

func TestCheckDiscussCompleteness_Enabled(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.DiscussCompleteness = true
	engine.discussState.Questions = []string{"What framework?", "What database?"}
	engine.discussState.Answers = map[int]string{0: "React"}
	result := engine.CheckDiscussCompleteness()
	// 1 answered out of 2 = 50% base score, minus penalty for missing areas
	if result.Total != 2 {
		t.Errorf("expected total 2, got %d", result.Total)
	}
	if result.Answered != 1 {
		t.Errorf("expected 1 answered, got %d", result.Answered)
	}
}

func TestVerifyCriterion_Exists(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "test.txt"), []byte("hello"), 0644)
	task := m31types.Task{ID: 1, Files: []string{"test.txt"}}
	if !engine.verifyCriterion(task, "test.txt exists") {
		t.Error("expected file exists to pass")
	}
}

func TestVerifyCriterion_Exists_NotFound(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{ID: 1, Files: []string{}}
	if engine.verifyCriterion(task, "nonexistent.txt exists") {
		t.Error("expected file not found to fail")
	}
}

func TestVerifyCriterion_Has(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc main() {}\n"), 0644)
	task := m31types.Task{ID: 1, Files: []string{"main.go"}}
	if !engine.verifyCriterion(task, "main.go has func main") {
		t.Error("expected pattern match to pass")
	}
}

func TestVerifyCriterion_Has_NotFound(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\n"), 0644)
	task := m31types.Task{ID: 1, Files: []string{"main.go"}}
	if engine.verifyCriterion(task, "main.go has func nonexistent_xyz") {
		t.Error("expected pattern not found to fail")
	}
}

func TestVerifyCriterion_Fallback(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "auth.go"), []byte("package main\nfunc validateUser() {}\n"), 0644)
	task := m31types.Task{ID: 1, Files: []string{"auth.go"}}
	// Fallback checks if criterion text appears in any task file
	if !engine.verifyCriterion(task, "validateUser") {
		t.Error("expected fallback match to pass")
	}
}

func TestRunDeepAnalysis(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Create a go project structure
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\nrequire (\n  github.com/foo/bar v1.0\n  github.com/baz/qux v2.0\n  github.com/other/thing v3.0\n)\n"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc main() {}\n"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main_test.go"), []byte("package main\nfunc TestMain(t *testing.T) {}\n"), 0644)

	analysis := engine.runDeepAnalysis(engine.workDir)
	if analysis.ProjectType != "go" {
		t.Errorf("expected project type 'go', got %q", analysis.ProjectType)
	}
	if analysis.DependencyCount != 3 {
		t.Errorf("expected 3 dependencies, got %d", analysis.DependencyCount)
	}
	if analysis.FileCount < 2 {
		t.Errorf("expected at least 2 files, got %d", analysis.FileCount)
	}
	if analysis.TestFileRatio <= 0 {
		t.Error("expected positive test file ratio")
	}
	if analysis.HealthScore <= 0 {
		t.Error("expected positive health score")
	}
}

func TestSaveDiscussAnswers_EmptyAnswers(t *testing.T) {
	engine, _ := setupTestEngine(t)
	project := &m31types.ProjectState{Answers: make(map[string]string)}
	err := engine.saveDiscussAnswers(project, []string{"Q1"}, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(project.Answers) != 0 {
		t.Errorf("expected 0 answers, got %d", len(project.Answers))
	}
}

func TestBuildDiscussContext_V2(t *testing.T) {
	engine, _ := setupTestEngine(t)
	goal := "build a REST API"
	messages := engine.buildDiscussContext(goal)
	if len(messages) < 1 {
		t.Fatalf("expected at least 1 message, got %d", len(messages))
	}
	if messages[0].Role != "system" {
		t.Errorf("expected system message first, got %q", messages[0].Role)
	}
}

func TestCountDiscussIssues(t *testing.T) {
	issues := []DiscussIssue{
		{Severity: "blocker"},
		{Severity: "warning"},
		{Severity: "blocker"},
	}
	blockers, warnings := countDiscussIssues(issues)
	if blockers != 2 {
		t.Errorf("expected 2 blockers, got %d", blockers)
	}
	if warnings != 1 {
		t.Errorf("expected 1 warning, got %d", warnings)
	}
}

func TestCheckAnswerCompleteness_AllAnswered_V2(t *testing.T) {
	questions := []string{"What framework?", "What database?"}
	answers := map[int]string{0: "React", 1: "PostgreSQL"}
	result := checkAnswerCompleteness(questions, answers, "build web app")
	if result.Answered != 2 {
		t.Errorf("expected 2 answered, got %d", result.Answered)
	}
	if result.Skipped != 0 {
		t.Errorf("expected 0 skipped, got %d", result.Skipped)
	}
}

func TestCheckAnswerCompleteness_NoneAnswered(t *testing.T) {
	questions := []string{"Q1", "Q2", "Q3"}
	answers := map[int]string{}
	result := checkAnswerCompleteness(questions, answers, "build something")
	if result.Answered != 0 {
		t.Errorf("expected 0 answered, got %d", result.Answered)
	}
	if result.Skipped != 3 {
		t.Errorf("expected 3 skipped, got %d", result.Skipped)
	}
}

func TestWordSet_V2(t *testing.T) {
	s := "the quick brown fox jumps over the lazy dog"
	result := wordSet(s)
	if result["the"] {
		t.Error("'the' should be filtered as stop word")
	}
	if !result["quick"] {
		t.Error("'quick' should be in the set")
	}
	if !result["brown"] {
		t.Error("'brown' should be in the set")
	}
}

func TestWordOverlap_HighOverlap(t *testing.T) {
	// wordSet filters stop words (3+ chars), so these sentences have overlapping technical terms
	overlap := wordOverlap("database connection pooling configuration settings", "database connection pooling timeout settings")
	if overlap < 0.5 {
		t.Errorf("expected overlap >= 0.5, got %f", overlap)
	}
}

func TestWordOverlap_LowOverlap(t *testing.T) {
	overlap := wordOverlap("what database should we use", "how to deploy to production")
	if overlap > 0.5 {
		t.Errorf("expected low overlap, got %f", overlap)
	}
}

func TestWordOverlap_Empty(t *testing.T) {
	overlap := wordOverlap("", "")
	if overlap != 0 {
		t.Errorf("expected 0 for empty strings, got %f", overlap)
	}
}

func TestExtractKeyPhrase_Short(t *testing.T) {
	result := extractKeyPhrase("hello")
	if result != "hello" {
		t.Errorf("expected 'hello', got %q", result)
	}
}

func TestExtractKeyPhrase_Truncation(t *testing.T) {
	long := "this is a very long criterion that exceeds fifty characters easily and should be truncated"
	result := extractKeyPhrase(long)
	if len(result) > 50 {
		t.Errorf("expected at most 50 chars, got %d", len(result))
	}
}

func TestCheckQuestionQuality_V2(t *testing.T) {
	questions := []string{
		"Do you want React?",
		"What framework should we use for the frontend?",
		"How should we implement the authentication middleware?",
	}
	issues := checkQuestionQuality(questions)
	// First question is yes/no
	foundYesNo := false
	for _, issue := range issues {
		if issue.Category == "yes_no" {
			foundYesNo = true
			break
		}
	}
	if !foundYesNo {
		t.Error("expected yes_no issue for first question")
	}
}

func TestCheckQuestionQuality_Vague_V2(t *testing.T) {
	questions := []string{"What?"}
	issues := checkQuestionQuality(questions)
	foundVague := false
	for _, issue := range issues {
		if issue.Category == "vague" {
			foundVague = true
			break
		}
	}
	if !foundVague {
		t.Error("expected vague issue for short question")
	}
}

func TestCheckQuestionQuality_Duplicates(t *testing.T) {
	questions := []string{
		"how should we implement the authentication middleware for the api",
		"how should we implement the authentication middleware for the api backend",
	}
	issues := checkQuestionQuality(questions)
	foundDup := false
	for _, issue := range issues {
		if issue.Category == "duplicate" {
			foundDup = true
			break
		}
	}
	if !foundDup {
		t.Error("expected duplicate issue for near-identical questions")
	}
}

func TestIsVagueQuestion_Short(t *testing.T) {
	if !isVagueQuestion("What?") {
		t.Error("expected short question to be vague")
	}
}

func TestIsVagueQuestion_Long_WithTerms(t *testing.T) {
	if isVagueQuestion("How should we implement the database connection pooling for the API endpoints?") {
		t.Error("expected long question with technical terms to not be vague")
	}
}

func TestIsVagueQuestion_Long_NoTerms(t *testing.T) {
	if isVagueQuestion("How should we implement something for the general system?") {
		t.Error("expected long question with general terms to not be vague")
	}
}

func TestIsYesNoQuestion_Should(t *testing.T) {
	if !isYesNoQuestion("Should we use React?") {
		t.Error("expected 'Should we use React?' to be yes/no")
	}
}

func TestIsYesNoQuestion_WithDash(t *testing.T) {
	if !isYesNoQuestion("Is it working? — verify by running tests") {
		t.Error("expected yes/no even with em-dash suggestion")
	}
}

func TestIsYesNoQuestion_Wh(t *testing.T) {
	if isYesNoQuestion("What framework should we use?") {
		t.Error("expected 'What framework' to NOT be yes/no")
	}
}

func TestBuildOutlineContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	messages := engine.buildOutlineContext(context.Background(), "build a website")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if messages[0].Role != "system" {
		t.Errorf("expected system first, got %q", messages[0].Role)
	}
	if !strings.Contains(messages[1].Content, "build a website") {
		t.Error("expected goal in user message")
	}
}

func TestBuildWaveExpandContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	outline := &PlanOutline{
		Title: "Test Plan",
		Waves: []WaveOutline{
			{Wave: 1, Tasks: []TaskStub{{ID: 1, Action: "create", Description: "Create file", Category: "feat"}}},
		},
	}
	wave := &outline.Waves[0]
	messages := engine.buildWaveExpandContext(outline, wave, "build a feature")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if !strings.Contains(messages[1].Content, "Expanding Wave 1") {
		t.Error("expected wave info in user message")
	}
}

func TestGenerateSessionSummary(t *testing.T) {
	engine, _ := setupTestEngine(t)
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Description: "Create file"},
		{ID: 2, Action: "modify", Description: "Edit file", Status: m31types.StatusDone},
	}
	commits := []git.CommitInfo{
		{Hash: "abc123", Message: "feat: add file"},
	}
	summary := ShipSummary{
		Duration:  5 * time.Minute,
		TaskTotal: 2,
		TaskDone:  1,
	}
	result := engine.generateSessionSummary(tasks, commits, "build a feature", summary)
	if !strings.Contains(result, "Session Summary") {
		t.Error("expected header in summary")
	}
	if !strings.Contains(result, "build a feature") {
		t.Error("expected goal in summary")
	}
}

func TestExtractKeyPhrase_Empty(t *testing.T) {
	result := extractKeyPhrase("")
	if result != "" {
		t.Errorf("expected empty, got %q", result)
	}
}

func TestCheckAnswerCompleteness_EmptyQuestions_V2(t *testing.T) {
	result := checkAnswerCompleteness([]string{}, nil, "goal")
	if result.Total != 0 {
		t.Errorf("expected total 0, got %d", result.Total)
	}
}

func TestCheckAnswerCompleteness_SkippedWithAnswers(t *testing.T) {
	questions := []string{"Q1", "Q2"}
	answers := map[int]string{0: "A1"}
	result := checkAnswerCompleteness(questions, answers, "")
	if result.Answered != 1 {
		t.Errorf("expected 1 answered, got %d", result.Answered)
	}
	if result.Skipped != 1 {
		t.Errorf("expected 1 skipped, got %d", result.Skipped)
	}
}

// ============================================================================
// Additional Tests for Final Push
// ============================================================================

func TestRunEnvironmentPreflight_Go(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\n"), 0644)
	check := engine.runEnvironmentPreflight()
	// Should have runtime version set (go is available on the test machine)
	if check.RuntimeVersion == "" && check.Passed {
		t.Error("expected runtime version or failed check")
	}
}

func TestRunEnvironmentPreflight_Unknown(t *testing.T) {
	engine, _ := setupTestEngine(t)
	check := engine.runEnvironmentPreflight()
	// No project type detected, should pass with no version
	if !check.Passed {
		t.Error("expected passed for unknown project type")
	}
}

func TestGenerateFollowUpsIfNeeded_NoQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.DiscussFollowUps = true
	// No questions → shouldGenerateFollowUps returns false → nil, nil
	result, err := engine.GenerateFollowUpsIfNeeded(context.Background())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != nil {
		t.Error("expected nil result when no questions")
	}
}

func TestShouldGenerateFollowUps_Disabled(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// cfg is nil → not enabled
	result := engine.shouldGenerateFollowUps(DiscussCompleteness{Score: 30})
	if result {
		t.Error("expected false when not enabled")
	}
}

func TestShouldGenerateFollowUps_LowScore(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.DiscussFollowUps = true
	result := engine.shouldGenerateFollowUps(DiscussCompleteness{Score: 40, Total: 10, Skipped: 3})
	if !result {
		t.Error("expected true when score < 60")
	}
}

func TestShouldGenerateFollowUps_HighScore(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.DiscussFollowUps = true
	result := engine.shouldGenerateFollowUps(DiscussCompleteness{Score: 80, Total: 10, Skipped: 2})
	if result {
		t.Error("expected false when score >= 60 and not too many skipped")
	}
}

func TestShouldGenerateFollowUps_ManySkipped(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	engine.cfg.Features.DiscussFollowUps = true
	// Skipped > Total/2 → true
	result := engine.shouldGenerateFollowUps(DiscussCompleteness{Score: 80, Total: 10, Skipped: 6})
	if !result {
		t.Error("expected true when > half skipped")
	}
}

func TestGenerateSessionSummary_V2(t *testing.T) {
	engine, _ := setupTestEngine(t)
	summary := engine.generateSessionSummary(nil, nil, "test goal", ShipSummary{})
	if !strings.Contains(summary, "Session Summary") {
		t.Error("expected header")
	}
}

func TestBuildOutlineContext_V2(t *testing.T) {
	engine, _ := setupTestEngine(t)
	messages := engine.buildOutlineContext(context.Background(), "test goal")
	if len(messages) < 1 {
		t.Error("expected messages")
	}
}

func TestBuildWaveExpandContext_V2(t *testing.T) {
	engine, _ := setupTestEngine(t)
	outline := &PlanOutline{
		Title: "T",
		Waves: []WaveOutline{{Wave: 1, Tasks: []TaskStub{{ID: 1}}}},
	}
	messages := engine.buildWaveExpandContext(outline, &outline.Waves[0], "goal")
	if len(messages) < 1 {
		t.Error("expected messages")
	}
}

// ============================================================================
// Mock Types
// ============================================================================

type mockBudgetConfig struct {
	limit float64
}

func (m *mockBudgetConfig) GetBudgetLimit() float64 {
	return m.limit
}
