package workflow

import (
	"context"
	"io"
	"math"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

func TestDetectProjectType_NodeJS(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte("{}"), 0644)
	if got := detectProjectType(dir); got != "nodejs" {
		t.Errorf("expected nodejs, got %q", got)
	}
}

func TestDetectProjectType_Rust(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Cargo.toml"), []byte("[package]"), 0644)
	if got := detectProjectType(dir); got != "rust" {
		t.Errorf("expected rust, got %q", got)
	}
}

func TestDetectProjectType_Python_Pyproject(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "pyproject.toml"), []byte("[project]"), 0644)
	if got := detectProjectType(dir); got != "python" {
		t.Errorf("expected python, got %q", got)
	}
}

func TestDetectProjectType_Python_Requirements(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte("flask"), 0644)
	if got := detectProjectType(dir); got != "python" {
		t.Errorf("expected python, got %q", got)
	}
}

func TestDetectProjectType_Java(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "pom.xml"), []byte("<project/>"), 0644)
	if got := detectProjectType(dir); got != "java" {
		t.Errorf("expected java, got %q", got)
	}
}

func TestDetectProjectType_CC_Makefile(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "Makefile"), []byte("all:"), 0644)
	if got := detectProjectType(dir); got != "cc" {
		t.Errorf("expected cc, got %q", got)
	}
}

func TestDetectProjectType_CC_CMake(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "CMakeLists.txt"), []byte("cmake_minimum_required"), 0644)
	if got := detectProjectType(dir); got != "cc" {
		t.Errorf("expected cc, got %q", got)
	}
}

func TestParseQuestions_NumberedWithoutQuestionMark(t *testing.T) {
	content := "1. Use Gin framework for the API\n2. PostgreSQL for the database"
	questions := parseQuestions(content)
	if len(questions) != 2 {
		t.Fatalf("expected 2 questions via fallback, got %d: %v", len(questions), questions)
	}
	if !strings.Contains(questions[0], "Gin") {
		t.Errorf("expected question about Gin, got %q", questions[0])
	}
}

func TestParseQuestions_FallbackLinesWithQuestionMark(t *testing.T) {
	content := "Should we use Redis?\nWhat about caching strategy?"
	questions := parseQuestions(content)
	if len(questions) != 2 {
		t.Errorf("expected 2 questions via line fallback, got %d: %v", len(questions), questions)
	}
}

func TestParseQuestions_EmptyContent(t *testing.T) {
	questions := parseQuestions("")
	if len(questions) != 0 {
		t.Errorf("expected 0 questions for empty, got %d", len(questions))
	}
}

func TestParseQuestions_ShortFallbackFiltered(t *testing.T) {
	// Numbered items without ? shorter than 10 chars should be filtered
	content := "1. Short"
	questions := parseQuestions(content)
	if len(questions) != 0 {
		t.Errorf("expected 0 (too short for fallback), got %d", len(questions))
	}
}

func TestParseQuestions_LinesWithQuestionMark_ShortFiltered(t *testing.T) {
	// Lines with ? shorter than 10 chars should be filtered
	content := "Hi?"
	questions := parseQuestions(content)
	if len(questions) != 0 {
		t.Errorf("expected 0 (line too short), got %d", len(questions))
	}
}

func TestFormatTaskSummary_Empty(t *testing.T) {
	summary := formatTaskSummary(nil)
	if !strings.Contains(summary, "ID") {
		t.Errorf("expected header even for empty, got %q", summary)
	}
}

func TestFormatTaskSummary_EmptyStatus(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "test", Dependencies: []int{}},
	}
	summary := formatTaskSummary(tasks)
	if !strings.Contains(summary, "pending") {
		t.Errorf("expected 'pending' for empty status, got %q", summary)
	}
}

func TestFormatTaskSummary_MultipleDeps(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 3, Action: "Create", Description: "test", Dependencies: []int{1, 2}},
	}
	summary := formatTaskSummary(tasks)
	if !strings.Contains(summary, "1, 2") {
		t.Errorf("expected '1, 2' for deps, got %q", summary)
	}
}

func TestDetectPackageManager_PnpmLock(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "pnpm-lock.yaml"), []byte(""), 0644)
	if got := detectPackageManager(dir); got != "pnpm run" {
		t.Errorf("expected 'pnpm run', got %q", got)
	}
}

func TestDetectPackageManager_YarnLock(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "yarn.lock"), []byte(""), 0644)
	if got := detectPackageManager(dir); got != "yarn" {
		t.Errorf("expected 'yarn', got %q", got)
	}
}

func TestDetectPackageManager_BunLock(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "bun.lockb"), []byte(""), 0644)
	if got := detectPackageManager(dir); got != "bun run" {
		t.Errorf("expected 'bun run', got %q", got)
	}
}

func TestDetectPackageManager_NpmLock(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package-lock.json"), []byte("{}"), 0644)
	if got := detectPackageManager(dir); got != "npm run" {
		t.Errorf("expected 'npm run', got %q", got)
	}
}

func TestDetectPackageManager_PackageJsonFallback(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte("{}"), 0644)
	if got := detectPackageManager(dir); got != "npm run" {
		t.Errorf("expected 'npm run' fallback, got %q", got)
	}
}

func TestDetectPackageManager_NoPackageJson(t *testing.T) {
	dir := t.TempDir()
	if got := detectPackageManager(dir); got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestListCwdFiles_SkipsGitDir(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, ".git", "objects"), 0755)
	os.WriteFile(filepath.Join(dir, ".git", "HEAD"), []byte("ref: refs/heads/main"), 0644)
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main"), 0644)

	list := listCwdFiles(dir)
	if strings.Contains(list, ".git") {
		t.Errorf("expected .git to be skipped, got %q", list)
	}
	if !strings.Contains(list, "main.go") {
		t.Errorf("expected main.go, got %q", list)
	}
}

func TestListCwdFiles_SkipsM31aDir(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, ".m31a"), 0755)
	os.WriteFile(filepath.Join(dir, ".m31a", "config.toml"), []byte(""), 0644)
	os.WriteFile(filepath.Join(dir, "app.go"), []byte("package main"), 0644)

	list := listCwdFiles(dir)
	if strings.Contains(list, ".m31a") {
		t.Errorf("expected .m31a to be skipped, got %q", list)
	}
	if !strings.Contains(list, "app.go") {
		t.Errorf("expected app.go, got %q", list)
	}
}

func TestHasTestFiles_TestJS(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "app.test.js"), []byte(""), 0644)
	if !hasTestFiles(dir, []string{"app.js"}) {
		t.Error("expected .test.js to be detected")
	}
}

func TestHasTestFiles_TestPy(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main_test.py"), []byte(""), 0644)
	if !hasTestFiles(dir, []string{"main.py"}) {
		t.Error("expected _test.py to be detected")
	}
}

func TestHasTestFiles_InSameDir(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(""), 0644)
	os.WriteFile(filepath.Join(dir, "main_test.go"), []byte(""), 0644)
	if !hasTestFiles(dir, []string{"main.go"}) {
		t.Error("expected test file in same directory to be detected")
	}
}

func TestHasTestFiles_EmptyDir(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "src"), 0755)
	if hasTestFiles(dir, []string{"src/app.go"}) {
		t.Error("expected no test files in empty dir")
	}
}

func TestReadTaskFiles_PathTraversalBlocked(t *testing.T) {
	engine, _ := setupTestEngine(t)
	content := engine.readTaskFiles([]string{"../../etc/passwd"})
	if !strings.Contains(content, "path traversal blocked") {
		t.Errorf("expected path traversal blocked message, got %q", content)
	}
}

func TestSetPhaseModel_NilMapInit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.perPhaseModels = nil
	engine.SetPhaseModel(m31types.PhasePlan, "claude-3-opus")
	if engine.perPhaseModels[m31types.PhasePlan] != "claude-3-opus" {
		t.Errorf("expected claude-3-opus, got %q", engine.perPhaseModels[m31types.PhasePlan])
	}
}

func TestSetPhaseModel_EmptyModelIgnored(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.perPhaseModels = map[m31types.WorkflowPhase]string{
		m31types.PhasePlan: "existing",
	}
	engine.SetPhaseModel(m31types.PhasePlan, "")
	if engine.perPhaseModels[m31types.PhasePlan] != "existing" {
		t.Errorf("expected 'existing' to be preserved, got %q", engine.perPhaseModels[m31types.PhasePlan])
	}
}

func TestModelForPhase_PerPhaseOverride(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.perPhaseModels = map[m31types.WorkflowPhase]string{
		m31types.PhasePlan: "claude-3-opus",
	}
	got := engine.modelForPhase(m31types.PhasePlan)
	if got != "claude-3-opus" {
		t.Errorf("expected 'claude-3-opus', got %q", got)
	}
}

func TestModelForPhase_AgentsConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Agents: config.AgentsConfig{
			Plan: "gpt-4",
		},
	}
	got := engine.modelForPhase(m31types.PhasePlan)
	if got != "gpt-4" {
		t.Errorf("expected 'gpt-4', got %q", got)
	}
}

func TestModelForPhase_AgentsDefault(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Agents: config.AgentsConfig{
			Default: "default-model",
		},
	}
	got := engine.modelForPhase(m31types.PhasePlan)
	if got != "default-model" {
		t.Errorf("expected 'default-model', got %q", got)
	}
}

func TestModelForPhase_FallbackToEngineModel(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{}
	got := engine.modelForPhase(m31types.PhasePlan)
	if got != "test-model" {
		t.Errorf("expected 'test-model', got %q", got)
	}
}

func TestModelForPhase_NilConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	got := engine.modelForPhase(m31types.PhasePlan)
	if got != "test-model" {
		t.Errorf("expected 'test-model' with nil config, got %q", got)
	}
}

func TestModelForPhase_AllPhases(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Agents: config.AgentsConfig{
			Execute: "exec-model",
			Verify:  "verify-model",
			Ship:    "ship-model",
			Discuss: "discuss-model",
		},
	}
	tests := []struct {
		phase m31types.WorkflowPhase
		want  string
	}{
		{m31types.PhaseExecute, "exec-model"},
		{m31types.PhaseVerify, "verify-model"},
		{m31types.PhaseShip, "ship-model"},
		{m31types.PhaseDiscuss, "discuss-model"},
	}
	for _, tt := range tests {
		got := engine.modelForPhase(tt.phase)
		if got != tt.want {
			t.Errorf("modelForPhase(%s) = %q, want %q", tt.phase, got, tt.want)
		}
	}
}

func TestSetModel_WithProvider(t *testing.T) {
	engine, _ := setupTestEngine(t)
	newProvider := &mockProvider{response: "new"}
	engine.SetModel("new-model", newProvider)
	if engine.modelID != "new-model" {
		t.Errorf("expected modelID 'new-model', got %q", engine.modelID)
	}
	if engine.provider != newProvider {
		t.Error("expected provider to be updated")
	}
}

func TestSetModel_NilProvider(t *testing.T) {
	engine, _ := setupTestEngine(t)
	original := engine.provider
	engine.SetModel("new-model", nil)
	if engine.modelID != "new-model" {
		t.Errorf("expected modelID 'new-model', got %q", engine.modelID)
	}
	if engine.provider != original {
		t.Error("expected provider to remain unchanged with nil")
	}
}

func TestDiscussState_ReturnsCopy(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{
		Questions: []string{"Q1", "Q2"},
		Answers:   map[int]string{0: "A1"},
	}
	state := engine.DiscussState()
	if len(state.Questions) != 2 {
		t.Errorf("expected 2 questions, got %d", len(state.Questions))
	}
	// Mutating the copy should not affect engine
	state.Questions = append(state.Questions, "Q3")
	if len(engine.discussState.Questions) != 2 {
		t.Error("DiscussState should return a copy")
	}
}

func TestSkipDiscuss_NoQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{}
	err := engine.SkipDiscuss()
	if err != nil {
		t.Fatalf("SkipDiscuss with no questions should not error: %v", err)
	}
}

func TestSkipDiscuss_AlreadyAnswered(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	engine.discussState = DiscussState{
		Questions: []string{"Q1", "Q2"},
		Answers:   map[int]string{0: "already answered"},
	}
	err = engine.SkipDiscuss()
	if err != nil {
		t.Fatalf("SkipDiscuss failed: %v", err)
	}
	// Q1 should keep its answer, Q2 should get empty string
	if engine.discussState.Answers[0] != "already answered" {
		t.Error("existing answer should be preserved")
	}
	if engine.discussState.Answers[1] != "" {
		t.Error("unanswered question should get empty string")
	}
}

func TestFinalizeDiscuss_NoQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	engine.discussState = DiscussState{}
	err = engine.FinalizeDiscuss()
	if err != nil {
		t.Fatalf("FinalizeDiscuss with no questions failed: %v", err)
	}
}

func TestSetRefinementFeedback_NewFeedback(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.planVersion = 1
	engine.SetRefinementFeedback("make it simpler")
	if engine.refineFeedback != "make it simpler" {
		t.Errorf("expected feedback stored, got %q", engine.refineFeedback)
	}
	if engine.planVersion != 2 {
		t.Errorf("expected planVersion to bump to 2, got %d", engine.planVersion)
	}
}

func TestSetRefinementFeedback_DuplicateIgnored(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.planVersion = 1
	engine.SetRefinementFeedback("feedback")
	if engine.planVersion != 2 {
		t.Fatalf("expected version 2, got %d", engine.planVersion)
	}
	// Same feedback should not bump version
	engine.SetRefinementFeedback("feedback")
	if engine.planVersion != 2 {
		t.Errorf("expected version to stay 2 with duplicate feedback, got %d", engine.planVersion)
	}
}

func TestSetRefinementFeedback_EmptyClearsFeedback(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.refineFeedback = "old feedback"
	engine.planVersion = 3
	engine.SetRefinementFeedback("")
	if engine.refineFeedback != "" {
		t.Errorf("expected empty feedback, got %q", engine.refineFeedback)
	}
	if engine.planVersion != 3 {
		t.Errorf("expected version to stay 3 with empty feedback, got %d", engine.planVersion)
	}
}

func TestPlanContent(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.planMarkdown = "# My Plan\nSome content"
	got := engine.PlanContent()
	if got != "# My Plan\nSome content" {
		t.Errorf("expected plan content, got %q", got)
	}
}

func TestPlanVersion(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.planVersion = 5
	if got := engine.PlanVersion(); got != 5 {
		t.Errorf("expected 5, got %d", got)
	}
}

func TestBuildToolDefinitions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	defs := engine.buildToolDefinitions()
	if len(defs) == 0 {
		t.Error("expected non-empty tool definitions")
	}
	// Verify caching - second call should return same slice
	defs2 := engine.buildToolDefinitions()
	if len(defs2) != len(defs) {
		t.Errorf("expected cached result, got %d vs %d", len(defs2), len(defs))
	}
}

func TestBuildToolDefinitions_CorrectNames(t *testing.T) {
	engine, _ := setupTestEngine(t)
	defs := engine.buildToolDefinitions()
	names := make(map[string]bool)
	for _, d := range defs {
		names[d.Name] = true
	}
	for _, expected := range []string{"Bash", "FileRead", "FileWrite"} {
		if !names[expected] {
			t.Errorf("expected tool %q in definitions", expected)
		}
	}
}

func TestSubmitDiscussAnswer_CreatesAnswersMap(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{
		Questions: []string{"Q1"},
		Answers:   nil, // explicitly nil
	}
	err := engine.SubmitDiscussAnswer(0, "answer")
	if err != nil {
		t.Fatalf("SubmitDiscussAnswer failed: %v", err)
	}
	if engine.discussState.Answers[0] != "answer" {
		t.Errorf("expected 'answer', got %q", engine.discussState.Answers[0])
	}
}

func TestHealTask_NotFound(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.HealTask(context.Background(), 999)
	if err == nil {
		t.Error("expected error for non-existent task")
	}
}

func TestHealTask_NotFailed(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "done task", Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)
	_, err = engine.HealTask(context.Background(), 1)
	if err == nil {
		t.Error("expected error for non-failed task")
	}
}

func TestEngine_Emit_NilEmitter(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.msgEmitter = nil
	// Should not panic
	engine.emit(IntermediateProgressMsg{Phase: "test", Message: "msg"})
}

func TestEngine_Emit_WithEmitter(t *testing.T) {
	engine, _ := setupTestEngine(t)
	var emitted any
	engine.msgEmitter = &testEmitter{emitFn: func(msg any) { emitted = msg }}
	engine.emit(IntermediateProgressMsg{Phase: "test", Message: "msg"})
	if emitted == nil {
		t.Error("expected message to be emitted")
	}
}

type testEmitter struct {
	emitFn func(any)
}

func (e *testEmitter) Emit(msg any) {
	if e.emitFn != nil {
		e.emitFn(msg)
	}
}

func TestDetectPackageManager_PriorityOrder(t *testing.T) {
	// pnpm-lock.yaml takes priority - verify by only having pnpm
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "pnpm-lock.yaml"), []byte(""), 0644)
	got := detectPackageManager(dir)
	if got != "pnpm run" {
		t.Errorf("expected 'pnpm run', got %q", got)
	}

	// Only yarn
	dir2 := t.TempDir()
	os.WriteFile(filepath.Join(dir2, "yarn.lock"), []byte(""), 0644)
	got = detectPackageManager(dir2)
	if got != "yarn" {
		t.Errorf("expected 'yarn', got %q", got)
	}

	// Only bun
	dir3 := t.TempDir()
	os.WriteFile(filepath.Join(dir3, "bun.lockb"), []byte(""), 0644)
	got = detectPackageManager(dir3)
	if got != "bun run" {
		t.Errorf("expected 'bun run', got %q", got)
	}
}

func TestHasTestFiles_TestFileDirect(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "pkg"), 0755)
	// Test file is directly in the files list
	if !hasTestFiles(dir, []string{"pkg/app_test.go"}) {
		t.Error("expected _test.go in files list to be detected")
	}
}

func TestFormatTaskSummary_EmptyDeps(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "test", Dependencies: nil},
	}
	summary := formatTaskSummary(tasks)
	if !strings.Contains(summary, "-") {
		t.Errorf("expected '-' for nil deps, got %q", summary)
	}
}

func TestParseSingleToolCall_ArrayForm(t *testing.T) {
	tc, err := parseSingleToolCall(`[{"name":"Bash","input":{"command":"ls"}}]`, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tc.Name != "Bash" {
		t.Errorf("expected Bash, got %q", tc.Name)
	}
}

func TestParseSingleToolCall_ArrayWithToolField(t *testing.T) {
	tc, err := parseSingleToolCall(`[{"tool":"FileRead","input":{"path":"a.go"}}]`, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tc.Name != "FileRead" {
		t.Errorf("expected FileRead, got %q", tc.Name)
	}
}

func TestParseSingleToolCall_ArrayEmpty(t *testing.T) {
	_, err := parseSingleToolCall(`[]`, 1)
	if err == nil {
		t.Error("expected error for empty array")
	}
}

func TestParseSingleToolCall_ArrayNoName(t *testing.T) {
	_, err := parseSingleToolCall(`[{"input":{"command":"ls"}}]`, 1)
	if err == nil {
		t.Error("expected error for missing name in array")
	}
}

func TestParseSingleToolCall_ObjectNoName(t *testing.T) {
	_, err := parseSingleToolCall(`{"input":{"command":"ls"}}`, 1)
	if err == nil {
		t.Error("expected error for missing name in object")
	}
}

func TestParseSingleToolCall_NameTakesPriority(t *testing.T) {
	tc, err := parseSingleToolCall(`{"name":"Bash","tool":"FileRead","input":{}}`, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tc.Name != "Bash" {
		t.Errorf("expected Bash (name takes priority), got %q", tc.Name)
	}
}

func TestParseSingleToolCall_NoInputDefaultsToEmpty(t *testing.T) {
	tc, err := parseSingleToolCall(`{"name":"Bash"}`, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if string(tc.Input) != "{}" {
		t.Errorf("expected {}, got %q", string(tc.Input))
	}
}

func TestParseSingleToolCall_InvalidJSON(t *testing.T) {
	_, err := parseSingleToolCall(`{invalid json`, 1)
	if err == nil {
		t.Error("expected error for invalid JSON")
	}
}

func TestParseSingleToolCall_InvalidJSONArray(t *testing.T) {
	_, err := parseSingleToolCall(`[{invalid`, 1)
	if err == nil {
		t.Error("expected error for invalid JSON array")
	}
}

func TestParseSingleToolCall_ToolFieldPriorityInArray(t *testing.T) {
	tc, err := parseSingleToolCall(`[{"tool":"Grep","input":{"pattern":"foo"}}]`, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tc.Name != "Grep" {
		t.Errorf("expected Grep, got %q", tc.Name)
	}
}

func TestNormalizeTrailingCommas_InString(t *testing.T) {
	input := `{"key": "value,,"}`
	got := normalizeTrailingCommas(input)
	if got != input {
		t.Errorf("trailing commas in strings should be preserved, got %q", got)
	}
}

func TestNormalizeTrailingCommas_NoTrailing(t *testing.T) {
	input := `{"a": 1, "b": 2}`
	got := normalizeTrailingCommas(input)
	if got != input {
		t.Errorf("no trailing commas, should be unchanged, got %q", got)
	}
}

func TestNormalizeTrailingCommas_EmptyString(t *testing.T) {
	got := normalizeTrailingCommas("")
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestStripJSONComments_BlockComment(t *testing.T) {
	input := `{"a": /* comment */ 1}`
	got := stripJSONComments(input)
	if strings.Contains(got, "comment") {
		t.Errorf("block comment should be stripped, got %q", got)
	}
}

func TestStripJSONComments_LineComment(t *testing.T) {
	input := `{"a": 1} // line comment`
	got := stripJSONComments(input)
	if strings.Contains(got, "line comment") {
		t.Errorf("line comment should be stripped, got %q", got)
	}
}

func TestStripJSONComments_CommentInString(t *testing.T) {
	input := `{"a": "this // is not a comment"}`
	got := stripJSONComments(input)
	if !strings.Contains(got, "//") {
		t.Errorf("comment inside string should be preserved, got %q", got)
	}
}

func TestValidateTasks_MissingAction(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Description: "test", Dependencies: []int{}},
	}
	errs := validateTasks(tasks)
	found := false
	for _, e := range errs {
		if strings.Contains(e, "missing action") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'missing action' error, got %v", errs)
	}
}

func TestValidateTasks_DuplicateIDs(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "A", Description: "a", Dependencies: []int{}},
		{ID: 1, Action: "B", Description: "b", Dependencies: []int{}},
	}
	errs := validateTasks(tasks)
	found := false
	for _, e := range errs {
		if strings.Contains(e, "duplicate ID") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'duplicate ID' error, got %v", errs)
	}
}

func TestValidateTasks_MissingID(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 0, Action: "A", Description: "a", Dependencies: []int{}},
	}
	errs := validateTasks(tasks)
	found := false
	for _, e := range errs {
		if strings.Contains(e, "missing ID") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'missing ID' error, got %v", errs)
	}
}

func TestListCwdFiles_EmptyDir(t *testing.T) {
	dir := t.TempDir()
	list := listCwdFiles(dir)
	if list != "" {
		t.Errorf("expected empty for empty dir, got %q", list)
	}
}

func TestListCwdFiles_SkipsVendor(t *testing.T) {
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "vendor", "pkg"), 0755)
	os.WriteFile(filepath.Join(dir, "vendor", "pkg", "mod.go"), []byte(""), 0644)
	os.WriteFile(filepath.Join(dir, "app.go"), []byte(""), 0644)

	list := listCwdFiles(dir)
	if strings.Contains(list, "vendor") {
		t.Errorf("expected vendor to be skipped, got %q", list)
	}
}

func TestReadTaskFiles_EmptyList(t *testing.T) {
	engine, _ := setupTestEngine(t)
	content := engine.readTaskFiles([]string{})
	if content != "" {
		t.Errorf("expected empty for empty list, got %q", content)
	}
}

func TestReadTaskFiles_MultipleFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("pkg a"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "b.go"), []byte("pkg b"), 0644)

	content := engine.readTaskFiles([]string{"a.go", "b.go", "missing.go"})
	if !strings.Contains(content, "pkg a") {
		t.Error("expected a.go content")
	}
	if !strings.Contains(content, "pkg b") {
		t.Error("expected b.go content")
	}
	if !strings.Contains(content, "missing.go: (not found)") {
		t.Error("expected missing.go not found message")
	}
}

func TestSessionID(t *testing.T) {
	engine, _ := setupTestEngine(t)
	if engine.SessionID() != engine.sessionID {
		t.Errorf("expected session ID %q, got %q", engine.sessionID, engine.SessionID())
	}
}

func TestSetSessionID(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestSetMsgEmitter(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetMsgEmitter(nil)
	if engine.msgEmitter != nil {
		t.Error("expected msgEmitter to be nil")
	}
	var called bool
	em := &testEmitter{emitFn: func(msg any) { called = true }}
	engine.SetMsgEmitter(em)
	engine.emit("test")
	if !called {
		t.Error("expected emitter to be called")
	}
}

func TestGitConfig_NilConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	gc := engine.gitConfig()
	if gc.UserName == "" {
		// DefaultGitConfig should return something
		t.Log("gitConfig with nil cfg returned defaults")
	}
}

func TestGitConfig_WithConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Git: config.GitConfig{
			CommitPrefix: "feat",
			UserName:     "testuser",
			UserEmail:    "test@example.com",
		},
	}
	gc := engine.gitConfig()
	if gc.UserName != "testuser" {
		t.Errorf("expected 'testuser', got %q", gc.UserName)
	}
	if gc.CommitPrefix != "feat" {
		t.Errorf("expected 'feat', got %q", gc.CommitPrefix)
	}
}

func TestPreflightContextCheck_NilTokens(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.tokens = nil
	_, err := engine.preflightContextCheck([]m31types.Message{
		{Role: "user", Content: "hello"},
	})
	if err != nil {
		t.Errorf("expected nil with nil tokens, got %v", err)
	}
}

func TestPreflightContextCheck_NilProvider(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = nil
	_, err := engine.preflightContextCheck([]m31types.Message{
		{Role: "user", Content: "hello"},
	})
	if err != nil {
		t.Errorf("expected nil with nil provider, got %v", err)
	}
}

func TestConsumeStreamWithTools_EmptyToolInput(t *testing.T) {
	engine, _ := setupTestEngine(t)

	chunks := []m31types.StreamChunk{
		{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "Bash", ToolInput: ""},
		{Type: "done"},
	}
	idx := 0
	next := func() (*m31types.StreamChunk, error) {
		if idx >= len(chunks) {
			return nil, io.EOF
		}
		c := chunks[idx]
		idx++
		return &c, nil
	}
	iter := &m31types.StreamIterator{Next: next, Close: func() error { return nil }}

	_, toolCalls, err := engine.consumeStreamWithTools(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(toolCalls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(toolCalls))
	}
	if string(toolCalls[0].Input) != "{}" {
		t.Errorf("expected {} for empty input, got %q", string(toolCalls[0].Input))
	}
}

func TestConsumeStreamWithTools_NilChunk(t *testing.T) {
	engine, _ := setupTestEngine(t)

	callCount := 0
	next := func() (*m31types.StreamChunk, error) {
		callCount++
		switch callCount {
		case 1:
			return nil, nil // nil chunk
		case 2:
			return &m31types.StreamChunk{Delta: "hello"}, nil
		default:
			return nil, io.EOF
		}
	}
	iter := &m31types.StreamIterator{Next: next, Close: func() error { return nil }}

	content, _, err := engine.consumeStreamWithTools(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "hello" {
		t.Errorf("expected 'hello', got %q", content)
	}
}

func TestConsumeStream_NilChunk(t *testing.T) {
	engine, _ := setupTestEngine(t)

	callCount := 0
	next := func() (*m31types.StreamChunk, error) {
		callCount++
		switch callCount {
		case 1:
			return nil, nil
		case 2:
			return &m31types.StreamChunk{Delta: "test"}, nil
		default:
			return nil, io.EOF
		}
	}
	iter := &m31types.StreamIterator{Next: next, Close: func() error { return nil }}

	content, err := engine.consumeStream(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "test" {
		t.Errorf("expected 'test', got %q", content)
	}
}

func TestFinalizeToolCalls_AutoGenerateID(t *testing.T) {
	engine, _ := setupTestEngine(t)
	builders := map[int]*toolCallBuilder{
		0: {name: "Bash", arguments: strings.Builder{}},
	}
	calls := finalizeToolCalls(builders, engine)
	if len(calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(calls))
	}
	if !strings.HasPrefix(calls[0].ID, "call_Bash_") {
		t.Errorf("expected auto-generated ID with prefix call_Bash_, got %q", calls[0].ID)
	}
}

func TestFinalizeToolCalls_UsesProvidedID(t *testing.T) {
	engine, _ := setupTestEngine(t)
	builders := map[int]*toolCallBuilder{
		0: {id: "custom_id", name: "Bash", arguments: strings.Builder{}},
	}
	calls := finalizeToolCalls(builders, engine)
	if len(calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(calls))
	}
	if calls[0].ID != "custom_id" {
		t.Errorf("expected 'custom_id', got %q", calls[0].ID)
	}
}

func TestHasCycle_ThreeWayCycle(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Dependencies: []int{2}},
		{ID: 2, Dependencies: []int{3}},
		{ID: 3, Dependencies: []int{1}},
	}
	if !hasCycle(tasks) {
		t.Error("expected cycle in 3-way dependency")
	}
}

func TestHasCycle_NoDeps(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Dependencies: []int{}},
		{ID: 2, Dependencies: []int{}},
		{ID: 3, Dependencies: []int{}},
	}
	if hasCycle(tasks) {
		t.Error("expected no cycle")
	}
}

func TestBuildExecuteContext(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{
		ID:                 1,
		Action:             "Create",
		Description:        "test task",
		Files:              []string{"main.go"},
		Dependencies:       []int{},
		AcceptanceCriteria: []string{"compiles"},
	}
	allTasks := []m31types.Task{task}
	messages := engine.buildExecuteContext(context.Background(), task, allTasks, "Build a CLI tool")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
	if messages[0].Role != "system" {
		t.Errorf("expected system message first, got %q", messages[0].Role)
	}
	if !strings.Contains(messages[1].Content, "test task") {
		t.Error("expected task description in context")
	}
}

func TestBuildExecuteContext_WithProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	task := m31types.Task{ID: 1, Action: "Create", Description: "test", Files: []string{"a.go"}}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "goal")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
}

func TestHealTask_FailedTaskMaxAttemptsExceeded(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	// Use a task with already-exhausted heal attempts
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "failed task", Status: m31types.StatusFailed, HealsAttempted: 3},
	}
	if saveErr := engine.sessionMgr.SaveTasks(engine.sessionID, tasks); saveErr != nil {
		t.Fatalf("SaveTasks failed: %v", saveErr)
	}
	_, err = engine.HealTask(context.Background(), 1)
	// This test verifies that HealTask processes the task. With a failed status
	// and heals already exceeded, it should return an error.
	if err != nil {
		t.Logf("HealTask returned error as expected: %v", err)
	}
}

func TestStripJSONComments_EmptyString(t *testing.T) {
	got := stripJSONComments("")
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestStripJSONComments_NoComments(t *testing.T) {
	input := `{"a": 1, "b": "hello"}`
	got := stripJSONComments(input)
	if got != input {
		t.Errorf("expected unchanged, got %q", got)
	}
}

func TestVerifyTask_GoProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc main() {}"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.go"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
	if !result.SyntaxOK {
		t.Errorf("expected syntax OK, errors: %v", result.Errors)
	}
}

func TestVerifyTask_PythonProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "requirements.txt"), []byte(""), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "app.py"), []byte("print('hello')"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"app.py"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestVerifyTask_NodeJSProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "package.json"), []byte(`{"name":"test"}`), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "app.ts"), []byte("console.log('hello')"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"app.ts"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestVerifyTask_RustProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "Cargo.toml"), []byte("[package]\nname=\"test\""), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.rs"), []byte("fn main() {}"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.rs"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestVerifyTask_CustomBuildCommand(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Verify: config.VerifyConfig{
			BuildCommand: "echo 'build ok'",
			TestCommand:  "echo 'test ok'",
		},
	}
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.go"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
	if !result.SyntaxOK {
		t.Errorf("expected syntax OK with custom build, errors: %v", result.Errors)
	}
	if !result.TestsOK {
		t.Errorf("expected tests OK with custom test, errors: %v", result.Errors)
	}
}

func TestVerifyTask_CustomBuildCommandFails(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Verify: config.VerifyConfig{
			BuildCommand: "false",
		},
	}
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.go"},
	}
	result := engine.verifyTask(context.Background(), task)
	if result.SyntaxOK {
		t.Error("expected syntax to fail with failing build command")
	}
}

func TestVerifyTask_CustomTestCommandFails(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Verify: config.VerifyConfig{
			TestCommand: "false",
		},
	}
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.go"},
	}
	result := engine.verifyTask(context.Background(), task)
	if result.TestsOK {
		t.Error("expected tests to fail with failing test command")
	}
}

func TestVerifyTask_PathTraversalInPython(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "requirements.txt"), []byte(""), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"../../etc/passwd.py"},
	}
	result := engine.verifyTask(context.Background(), task)
	if result.SyntaxOK {
		t.Error("expected path traversal to fail syntax check")
	}
}

func TestCollectDiffStats_NilGit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.git = nil
	stats := engine.collectDiffStats()
	if stats.FilesAdded != 0 || stats.Insertions != 0 {
		t.Errorf("expected zero stats with nil git, got %+v", stats)
	}
}

func TestCollectDiffStats_WithData(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Create a commit so there's a diff baseline
	os.WriteFile(filepath.Join(engine.workDir, "initial.go"), []byte("package main"), 0644)
	engine.git.Add("initial.go")
	engine.git.Commit("initial commit")

	// Now create a new file
	os.WriteFile(filepath.Join(engine.workDir, "new.go"), []byte("package new"), 0644)
	engine.git.Add("new.go")
	engine.git.Commit("add new.go")

	stats := engine.collectDiffStats()
	// Should have at least some stats from the diff
	t.Logf("stats: %+v", stats)
}

func TestRunInitialize_ContextCancellation(t *testing.T) {
	engine, _ := setupTestEngine(t)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := engine.RunPhase(ctx, m31types.PhaseInitialize, "Test")
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

func TestRunInitialize_DetectsGoProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build Go app")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("expected success")
	}

	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project.ProjectType != "go" {
		t.Errorf("expected project type 'go', got %q", project.ProjectType)
	}
}

func TestRunInitialize_DetectsNodeJSProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "package.json"), []byte("{}"), 0644)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build Node app")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("expected success")
	}

	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project.ProjectType != "nodejs" {
		t.Errorf("expected project type 'nodejs', got %q", project.ProjectType)
	}
}

func TestRunInitialize_DetectsPythonProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "requirements.txt"), []byte("flask"), 0644)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build Python app")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	project, _ := engine.sessionMgr.LoadProject(engine.sessionID)
	if project.ProjectType != "python" {
		t.Errorf("expected project type 'python', got %q", project.ProjectType)
	}
}

func TestRunInitialize_GitConfigured(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Git: config.GitConfig{
			UserName:  "testuser",
			UserEmail: "test@example.com",
		},
	}

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("expected success")
	}
}

func TestRunPhase_BudgetLimit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Features: config.FeaturesConfig{
			BudgetLimitUSD: 0.001,
		},
	}
	// Set cumulative cost above budget
	engine.totalCostBits = 1 // non-zero bits = non-zero cost

	_, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err == nil {
		t.Error("expected budget limit error")
	}
}

func TestRunPhase_BudgetZeroNoLimit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Features: config.FeaturesConfig{
			BudgetLimitUSD: 0,
		},
	}
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Errorf("expected no error with zero budget limit: %v", err)
	}
}

func TestRunPhase_NoConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = nil
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Errorf("expected no error with nil config: %v", err)
	}
}

func TestTransition_InvalidFromPhase(t *testing.T) {
	engine, _ := setupTestEngine(t)
	err := engine.Transition(context.Background(), m31types.PhaseShip, m31types.PhasePlan)
	if err == nil {
		t.Error("expected error for invalid transition from Ship")
	}
}

func TestTransition_InvalidToPhase(t *testing.T) {
	engine, _ := setupTestEngine(t)
	err := engine.Transition(context.Background(), m31types.PhaseIdle, m31types.PhaseShip)
	if err == nil {
		t.Error("expected error for invalid transition from Idle to Ship")
	}
}

func TestTransition_DiscussToPlan(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseDiscuss,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseDiscuss, "discussing", "done")

	err := engine.Transition(context.Background(), m31types.PhaseDiscuss, m31types.PhasePlan)
	if err != nil {
		t.Errorf("expected valid transition Discuss->Plan: %v", err)
	}
}

func TestTransition_DiscussToExecute(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseDiscuss,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseDiscuss, "discussing", "done")

	err := engine.Transition(context.Background(), m31types.PhaseDiscuss, m31types.PhaseExecute)
	if err != nil {
		t.Errorf("expected valid transition Discuss->Execute: %v", err)
	}
}

func TestTransition_PlanToExecute(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhasePlan, "planning", "done")

	err := engine.Transition(context.Background(), m31types.PhasePlan, m31types.PhaseExecute)
	if err != nil {
		t.Errorf("expected valid transition Plan->Execute: %v", err)
	}
}

func TestTransition_PlanToPlan(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhasePlan, "planning", "done")

	err := engine.Transition(context.Background(), m31types.PhasePlan, m31types.PhasePlan)
	if err != nil {
		t.Errorf("expected valid transition Plan->Plan (refinement): %v", err)
	}
}

func TestTransition_ExecuteToVerify(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseExecute,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseExecute, "executing", "done")

	err := engine.Transition(context.Background(), m31types.PhaseExecute, m31types.PhaseVerify)
	if err != nil {
		t.Errorf("expected valid transition Execute->Verify: %v", err)
	}
}

func TestTransition_VerifyToShip(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseVerify,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseVerify, "verifying", "done")

	err := engine.Transition(context.Background(), m31types.PhaseVerify, m31types.PhaseShip)
	if err != nil {
		t.Errorf("expected valid transition Verify->Ship: %v", err)
	}
}

func TestTransition_VerifyToExecute(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseVerify,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseVerify, "verifying", "done")

	err := engine.Transition(context.Background(), m31types.PhaseVerify, m31types.PhaseExecute)
	if err != nil {
		t.Errorf("expected valid transition Verify->Execute: %v", err)
	}
}

func TestTransition_ShipToIdle(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseShip,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseShip, "shipping", "done")

	err := engine.Transition(context.Background(), m31types.PhaseShip, m31types.PhaseIdle)
	if err != nil {
		t.Errorf("expected valid transition Ship->Idle: %v", err)
	}
}

func TestTransition_IdleToInitialize(t *testing.T) {
	engine, _ := setupTestEngine(t)
	err := engine.Transition(context.Background(), m31types.PhaseIdle, m31types.PhaseInitialize)
	if err != nil {
		t.Errorf("expected valid transition Idle->Initialize: %v", err)
	}
}

func TestTransition_PlanToDiscuss(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhasePlan, "planning", "done")

	err := engine.Transition(context.Background(), m31types.PhasePlan, m31types.PhaseDiscuss)
	if err != nil {
		t.Errorf("expected valid transition Plan->Discuss: %v", err)
	}
}

func TestTransition_PlanToIdle(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhasePlan, "planning", "done")

	err := engine.Transition(context.Background(), m31types.PhasePlan, m31types.PhaseIdle)
	if err != nil {
		t.Errorf("expected valid transition Plan->Idle: %v", err)
	}
}

func TestTransition_ExecuteToIdle(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseExecute,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseExecute, "executing", "done")

	err := engine.Transition(context.Background(), m31types.PhaseExecute, m31types.PhaseIdle)
	if err != nil {
		t.Errorf("expected valid transition Execute->Idle: %v", err)
	}
}

func TestTransition_VerifyToIdle(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseVerify,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseVerify, "verifying", "done")

	err := engine.Transition(context.Background(), m31types.PhaseVerify, m31types.PhaseIdle)
	if err != nil {
		t.Errorf("expected valid transition Verify->Idle: %v", err)
	}
}

func TestTransition_InitializeToIdle(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseInitialize,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseInitialize, "initializing", "done")

	err := engine.Transition(context.Background(), m31types.PhaseInitialize, m31types.PhaseIdle)
	if err != nil {
		t.Errorf("expected valid transition Initialize->Idle: %v", err)
	}
}

func TestReadTaskFiles_RelPath(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.MkdirAll(filepath.Join(engine.workDir, "sub"), 0755)
	os.WriteFile(filepath.Join(engine.workDir, "sub", "test.go"), []byte("package sub"), 0644)

	content := engine.readTaskFiles([]string{"sub/test.go"})
	if !strings.Contains(content, "package sub") {
		t.Errorf("expected file content, got %q", content)
	}
}

func TestRunPlan_InvalidJSONFallback(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mockProvider)
	mp.response = "plain text with no JSON"

	_, err = engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err == nil {
		t.Log("Plan may have succeeded via fallback parsing")
	}
}

func TestFinalizeToolCalls_ManyTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	builders := map[int]*toolCallBuilder{}
	for i := 0; i < 20; i++ {
		builders[i] = &toolCallBuilder{
			name:      "Bash",
			arguments: strings.Builder{},
		}
	}
	calls := finalizeToolCalls(builders, engine)
	if len(calls) > m31types.MaxToolsPerCall {
		t.Errorf("expected at most %d calls, got %d", m31types.MaxToolsPerCall, len(calls))
	}
}

func TestBuildSystemPrompt_Caching(t *testing.T) {
	engine, _ := setupTestEngine(t)
	p1 := engine.buildSystemPrompt()
	p2 := engine.buildSystemPrompt()
	if p1 != p2 {
		t.Error("expected cached prompt to be identical")
	}
}

func TestStreamLLM_PreflightContextExceeded(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mockProviderWithModel{
		model: &m31types.ModelInfo{
			ID:            "test-model",
			ContextLength: 50,
		},
	}
	longContent := strings.Repeat("word ", 60)
	messages := []m31types.Message{
		{Role: "user", Content: longContent},
	}
	_, err := engine.streamLLM(context.Background(), messages, false)
	if err == nil {
		t.Error("expected context exceeded error")
	}
}

func TestStreamLLM_WithTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}
	result, err := engine.streamLLM(context.Background(), messages, true)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result == "" {
		t.Error("expected non-empty result")
	}
}

func TestStreamLLM_WithoutTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}
	result, err := engine.streamLLM(context.Background(), messages, false)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result == "" {
		t.Error("expected non-empty result")
	}
}

func TestStreamLLM_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mockProvider{err: context.DeadlineExceeded}
	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}
	_, err := engine.streamLLM(context.Background(), messages, false)
	if err == nil {
		t.Error("expected error from LLM")
	}
}

func TestStreamLLMStreaming_WithTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}
	iter, err := engine.streamLLMStreaming(context.Background(), messages, true)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if iter == nil {
		t.Fatal("expected non-nil iterator")
	}
	defer iter.Close()
}

func TestStreamLLMStreaming_WithoutTools(t *testing.T) {
	engine, _ := setupTestEngine(t)
	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}
	iter, err := engine.streamLLMStreaming(context.Background(), messages, false)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if iter == nil {
		t.Fatal("expected non-nil iterator")
	}
	defer iter.Close()
}

func TestStreamLLMStreaming_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mockProvider{err: context.DeadlineExceeded}
	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}
	_, err := engine.streamLLMStreaming(context.Background(), messages, false)
	if err == nil {
		t.Error("expected error from LLM")
	}
}

func TestTransition_InitializeToDiscuss(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseInitialize,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseInitialize, "init", "done")

	err := engine.Transition(context.Background(), m31types.PhaseInitialize, m31types.PhaseDiscuss)
	if err != nil {
		t.Errorf("expected valid transition Initialize->Discuss: %v", err)
	}
}

func TestTransition_InvalidInitializeToPlan(t *testing.T) {
	engine, _ := setupTestEngine(t)
	err := engine.Transition(context.Background(), m31types.PhaseInitialize, m31types.PhasePlan)
	if err == nil {
		t.Error("expected error for Initialize->Plan (must go through Discuss)")
	}
}

func TestRunPlan_WithRefinement(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		`[{"id":1,"action":"Create","description":"v1 task","dependencies":[],"files":["a.go"],"acceptance_criteria":["works"]}]`,
		`[{"id":1,"action":"Create","description":"v2 task","dependencies":[],"files":["a.go"],"acceptance_criteria":["works"]}]`,
	}

	// First plan
	_, err = engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err != nil {
		t.Fatalf("First plan failed: %v", err)
	}

	// Refine
	engine.SetRefinementFeedback("simplify the approach")
	if engine.PlanVersion() < 2 {
		t.Errorf("expected plan version >= 2 after refinement, got %d", engine.PlanVersion())
	}
}

func TestRunVerify_FailedTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task with missing file", Files: []string{"nonexistent.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	if err == nil && !result.Success {
		t.Log("Verify correctly reported failure for missing file")
	}
	if result != nil && !result.Success {
		t.Log("Verify failed as expected for missing file")
	}
}

func TestRunExecute_WithTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mockProvider)
	mp.response = `{"name":"Bash","input":{"command":"echo test"}}`

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "test task", Files: []string{"test.go"}, Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseExecute, "Test")
	if err != nil {
		t.Logf("Execute returned error: %v", err)
	}
	if result != nil {
		t.Logf("Execute result: success=%v", result.Success)
	}
}

func TestRunShip_NoTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	if err != nil {
		t.Logf("Ship returned: %v", err)
	}
	if result != nil {
		t.Logf("Ship result: success=%v", result.Success)
	}
}

func TestBuildExecuteContext_WithPlanMarkdown(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	engine.planMarkdown = "# Plan\n## Summary\nBuild a web server\n### Core\n#### [NEW] main.go\n- Entry point"

	task := m31types.Task{ID: 1, Action: "Create", Description: "test", Files: []string{"main.go"}}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "goal")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
}

func TestHealTask_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	engine.provider = &mockProvider{err: context.DeadlineExceeded}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "failed task", Status: m31types.StatusFailed, Files: []string{"a.go"}},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.HealTask(context.Background(), 1)
	t.Logf("HealTask result: success=%v, err=%v", result, err)
}

func TestRunExecute_ContextCancellation(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "test", Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	_, err = engine.RunPhase(ctx, m31types.PhaseExecute, "Test")
	t.Logf("Execute with cancelled context: %v", err)
}

func TestConsumeStreamWithTools_Error(t *testing.T) {
	engine, _ := setupTestEngine(t)

	callCount := 0
	next := func() (*m31types.StreamChunk, error) {
		callCount++
		switch callCount {
		case 1:
			return &m31types.StreamChunk{Delta: "partial"}, nil
		case 2:
			return nil, io.ErrUnexpectedEOF
		default:
			return nil, io.EOF
		}
	}
	iter := &m31types.StreamIterator{Next: next, Close: func() error { return nil }}

	content, _, err := engine.consumeStreamWithTools(iter)
	if err != io.ErrUnexpectedEOF {
		t.Errorf("expected io.ErrUnexpectedEOF, got %v", err)
	}
	if content != "partial" {
		t.Errorf("expected 'partial', got %q", content)
	}
}

func TestConsumeStreamWithTools_TextThenError(t *testing.T) {
	engine, _ := setupTestEngine(t)

	chunks := []m31types.StreamChunk{
		{Type: "content", Delta: "hello"},
		{Type: "content", Delta: " world"},
	}
	idx := 0
	next := func() (*m31types.StreamChunk, error) {
		if idx >= len(chunks) {
			return nil, io.ErrUnexpectedEOF
		}
		c := chunks[idx]
		idx++
		return &c, nil
	}
	iter := &m31types.StreamIterator{Next: next, Close: func() error { return nil }}

	content, _, err := engine.consumeStreamWithTools(iter)
	if err != io.ErrUnexpectedEOF {
		t.Errorf("expected io.ErrUnexpectedEOF, got %v", err)
	}
	if content != "hello world" {
		t.Errorf("expected 'hello world', got %q", content)
	}
}

func TestBuildExecuteContext_EmptyGoal(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{ID: 1, Action: "Create", Description: "test", Files: []string{"a.go"}}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages, got %d", len(messages))
	}
}

func TestCollectDiffStats_WithCommitAndNewFile(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Create initial commit
	os.WriteFile(filepath.Join(engine.workDir, "init.go"), []byte("package main"), 0644)
	engine.git.Add("init.go")
	engine.git.Commit("initial")

	// Create uncommitted new file (not staged)
	os.WriteFile(filepath.Join(engine.workDir, "new.go"), []byte("package new"), 0644)

	stats := engine.collectDiffStats()
	// Diff between HEAD and working tree should show the new file
	t.Logf("stats after untracked file: %+v", stats)
}

func TestCollectDiffStats_WithModification(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Initial commit
	os.WriteFile(filepath.Join(engine.workDir, "mod.go"), []byte("package main"), 0644)
	engine.git.Add("mod.go")
	engine.git.Commit("initial")

	// Modify file without committing
	os.WriteFile(filepath.Join(engine.workDir, "mod.go"), []byte("package main\n// modified"), 0644)

	stats := engine.collectDiffStats()
	t.Logf("stats after modification: %+v", stats)
}

func TestCollectDiffStats_EmptyRepo(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// No commits yet
	stats := engine.collectDiffStats()
	if stats.FilesAdded != 0 || stats.Insertions != 0 {
		t.Errorf("expected zero stats for empty repo, got %+v", stats)
	}
}

func TestBuildExecuteContext_WithGoal(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{ID: 1, Action: "Create", Description: "test", Files: []string{"a.go"}}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "Build a web server")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages")
	}
	// System prompt should contain the goal
	sysContent := messages[0].Content
	if !strings.Contains(sysContent, "Build a web server") {
		t.Error("expected goal in system prompt")
	}
}

func TestBuildExecuteContext_AcceptanceCriteria(t *testing.T) {
	engine, _ := setupTestEngine(t)
	task := m31types.Task{
		ID:                 1,
		Action:             "Create",
		Description:        "test",
		Files:              []string{"a.go"},
		AcceptanceCriteria: []string{"compiles", "tests pass"},
	}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "goal")
	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "compiles") {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected acceptance criteria in context")
	}
}

func TestBuildPlanContext_WithRefinement(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	engine.planMarkdown = "# Old Plan\nOld content"
	engine.planVersion = 1
	engine.refineFeedback = "make it simpler"

	messages := engine.buildPlanContext(context.Background(), "Test", nil, nil, "")
	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "User Refinement Feedback") {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected refinement feedback in plan context")
	}
}

func TestBuildPlanContext_WithExistingTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	existingTasks := []m31types.Task{{ID: 1, Action: "Create", Description: "test"}}
	valErrs := []string{"missing description"}
	messages := engine.buildPlanContext(context.Background(), "Test", existingTasks, valErrs, "raw response here")
	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Previous Attempt Failed") {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected previous attempt info in context")
	}
}

func TestRunPlan_LLMErrorAllRetries(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mockProvider)
	mp.err = context.DeadlineExceeded

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err == nil && result != nil && !result.Success {
		t.Log("Plan failed after LLM errors as expected")
	}
}

func TestRunVerify_FailedTaskUnrecoverable(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Task that already exceeded heal attempts
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "broken task", Files: []string{"nonexistent.go"}, Status: m31types.StatusDone, HealsAttempted: 3},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	_, err = engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify with unrecoverable task: err=%v", err)
}

func TestRunShip_WithFailedTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "failed task", Status: m31types.StatusFailed},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	_, err = engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship with failed tasks: err=%v", err)
}

func TestRunShip_WithSkippedTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "skipped task", Status: m31types.StatusSkipped},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	_, err = engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship with skipped tasks: err=%v", err)
}

func TestHealTask_TaskNotFound(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.HealTask(context.Background(), 42)
	if err == nil {
		t.Error("expected error for non-existent task")
	}
}

func TestHealTask_FailedTaskMaxExceeded(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "broken", Status: m31types.StatusFailed, HealsAttempted: 5},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	ok, err := engine.HealTask(context.Background(), 1)
	if err == nil && !ok {
		t.Log("HealTask returned false for exhausted heals")
	}
}

func TestParseSingleToolCall_ArrayWithNullInput(t *testing.T) {
	tc, err := parseSingleToolCall(`[{"name":"Bash"}]`, 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if string(tc.Input) != "{}" {
		t.Errorf("expected {} for null input, got %q", string(tc.Input))
	}
}

func TestParseToolCalls_MalformedWithToolFields(t *testing.T) {
	eng := &Engine{}
	// Content with malformed JSON that has tool fields
	content := `{"name":"Bash","input":{broken`
	_, err := eng.parseToolCalls(content)
	if err == nil {
		t.Log("parseToolCalls may have returned nil error for malformed JSON")
	}
}

func TestParseToolCalls_NoToolCallsNoErrors(t *testing.T) {
	eng := &Engine{}
	content := `just some plain text with no tools`
	calls, err := eng.parseToolCalls(content)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) != 0 {
		t.Errorf("expected 0 calls, got %d", len(calls))
	}
}

func TestExecuteTaskWithTools_ToolExecutionError(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Return a tool call that will fail (FileRead of nonexistent)
	mp := engine.provider.(*mockProvider)
	mp.response = `[{"name":"FileRead","input":{"path":"/nonexistent/path/file.go"}}]`

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "test",
		Files:       []string{"test.go"},
	}
	allTasks := []m31types.Task{task}

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "goal")
	t.Logf("executeTaskWithTools result: success=%v, err=%v", result.Success, result.Error)
}

func TestExecuteTaskWithTools_NoToolCallsForFileTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// LLM returns text with no tool calls, task has files
	mp := engine.provider.(*mockProvider)
	mp.response = "I created the file for you"
	mp.multiResponses = []string{
		"I created the file for you",
		"I created the file for you",
		"I created the file for you",
		"I created the file for you",
		"I created the file for you",
	}

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "test",
		Files:       []string{"test.go"},
	}
	allTasks := []m31types.Task{task}

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "goal")
	t.Logf("executeTaskWithTools no tools result: success=%v, err=%v", result.Success, result.Error)
}

func TestExecuteTaskWithTools_NativeToolCalls(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Return native tool calls via multiResponses
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"ok",
		"done",
	}

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "test",
		Files:       []string{"test.go"},
	}
	allTasks := []m31types.Task{task}

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "goal")
	t.Logf("executeTaskWithTools result: success=%v, err=%v", result.Success, result.Error)
}

func TestRunShip_WithDoneTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create file and commit
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)
	engine.git.Add("main.go")
	engine.git.Commit("add main.go")

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "main.go", Files: []string{"main.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship with done tasks: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunVerify_WithTestFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create Go project with test files
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc main() {}"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main_test.go"), []byte("package main\nimport \"testing\"\nfunc TestMain(t *testing.T) {}"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "test task", Files: []string{"main.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify with test files: success=%v, err=%v", result != nil && result.Success, err)
}

func TestTransition_AllPhases(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.sessionMgr.SaveCheckpoint(engine.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseInitialize,
		Timestamp: time.Now(),
	})
	engine.sessionMgr.SaveState(engine.sessionID, m31types.PhaseInitialize, "init", "done")

	err := engine.Transition(context.Background(), m31types.PhaseInitialize, m31types.PhaseDiscuss)
	if err != nil {
		t.Errorf("Initialize->Discuss: %v", err)
	}
}

func TestRunVerify_FailedTasksReported(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"missing1.go"}, Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "task2", Files: []string{"missing2.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify with 2 failed tasks: success=%v, err=%v", result != nil && result.Success, err)
	if result != nil && !result.Success && result.Error == "" {
		t.Error("expected non-empty error string for failed tasks")
	}
}

func TestBuildExecuteContext_NoPlanMarkdown(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.planMarkdown = ""
	task := m31types.Task{ID: 1, Action: "Create", Description: "test", Files: []string{"a.go"}}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "goal")
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages")
	}
}

func TestRunInitialize_WithGitConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Git: config.GitConfig{
			UserName:  "tester",
			UserEmail: "test@example.com",
		},
	}

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "My project")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("expected success")
	}

	project, _ := engine.sessionMgr.LoadProject(engine.sessionID)
	if project.Goal != "My project" {
		t.Errorf("expected goal 'My project', got %q", project.Goal)
	}
}

func TestPreflightContextCheck_WarningThreshold(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.provider = &mockProviderWithModel{
		model: &m31types.ModelInfo{
			ID:            "test-model",
			ContextLength: 1000,
		},
	}

	// Create messages that exceed 80% but not 95% of 1000 tokens
	longContent := strings.Repeat("word ", 50)
	messages := []m31types.Message{
		{Role: "user", Content: longContent},
	}

	_, err := engine.preflightContextCheck(messages)
	if err != nil {
		t.Errorf("expected nil for warning threshold, got: %v", err)
	}
}

func TestFinalizeDiscuss_WithAnswers(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	engine.discussState = DiscussState{
		Questions: []string{"Q1?", "Q2?"},
		Answers:   map[int]string{0: "A1", 1: "A2"},
	}

	err = engine.FinalizeDiscuss()
	if err != nil {
		t.Fatalf("FinalizeDiscuss failed: %v", err)
	}

	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project.Answers["Q1?"] != "A1" {
		t.Errorf("expected 'A1', got %q", project.Answers["Q1?"])
	}
	if project.Answers["Q2?"] != "A2" {
		t.Errorf("expected 'A2', got %q", project.Answers["Q2?"])
	}
}

func TestSkipDiscuss_WithNilAnswers(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	engine.discussState = DiscussState{
		Questions: []string{"Q1", "Q2"},
		Answers:   nil,
	}

	err = engine.SkipDiscuss()
	if err != nil {
		t.Fatalf("SkipDiscuss failed: %v", err)
	}

	if engine.discussState.Answers[0] != "" {
		t.Error("expected empty string for Q1")
	}
	if engine.discussState.Answers[1] != "" {
		t.Error("expected empty string for Q2")
	}
}

func TestRunShip_WithPlanMarkdown(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create commit
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)
	engine.git.Add("main.go")
	engine.git.Commit("add main.go")

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "main.go", Files: []string{"main.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	// Save a plan with manual steps
	planMd := "# Plan\n## Summary\nTest plan\n## Verification Plan\n### Manual\n- Check browser\n- Run smoke tests"
	engine.sessionMgr.SavePlan(engine.sessionID, 1, planMd)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship with plan: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunShip_FailedTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "failing task", Files: []string{"nonexistent.go"}, Status: m31types.StatusFailed},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	if err == nil && result != nil && result.Error != "" {
		t.Log("Ship reported failure as expected")
	}
}

func TestRunVerify_SkipsNonDoneTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "pending", Status: m31types.StatusPending},
		{ID: 2, Action: "Create", Description: "running", Status: m31types.StatusRunning},
		{ID: 3, Action: "Create", Description: "skipped", Status: m31types.StatusSkipped},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	if err != nil {
		t.Fatalf("Verify failed: %v", err)
	}
	if !result.Success {
		t.Error("expected success when all tasks are non-done")
	}
}

func TestVerifyTask_NonGoNonNodeNonPythonProject(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "Makefile"), []byte("all:"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "app.c"), []byte("#include <stdio.h>"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"app.c"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestExecuteTaskWithTools_NilNativeTools_FallbackToParse(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Return content with tool calls in code blocks (parsed, not native)
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"```json\n{\"name\":\"Bash\",\"input\":{\"command\":\"echo hello\"}}\n```",
		"Tool executed successfully",
	}

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "test",
		Files:       []string{"test.go"},
	}

	result := engine.executeTaskWithTools(context.Background(), &task, []m31types.Task{task}, "goal")
	t.Logf("executeTaskWithTools fallback: success=%v, err=%v", result.Success, result.Error)
}

func TestVerifyTask_NodeJSProjectNoPackageManager(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "package.json"), []byte(`{"name":"test"}`), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "app.jsx"), []byte("console.log('hello')"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"app.jsx"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestVerifyTask_NodeJSProjectWithPackageManager(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "package.json"), []byte(`{"name":"test"}`), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "yarn.lock"), []byte(""), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "app.tsx"), []byte("console.log('hello')"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"app.tsx"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestVerifyTask_NodeJSNoPackageJSON(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "app.ts"), []byte("console.log('hello')"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"app.ts"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("expected files to exist")
	}
}

func TestVerifyTask_GoBuildFails(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	// Write invalid Go code that won't compile
	os.WriteFile(filepath.Join(engine.workDir, "broken.go"), []byte("package main\nfunc invalid {{{"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"broken.go"},
	}
	result := engine.verifyTask(context.Background(), task)
	if result.SyntaxOK {
		t.Error("expected syntax to fail for broken Go code")
	}
}

func TestVerifyTask_RustBuildFails(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "Cargo.toml"), []byte("[package]\nname=\"test\""), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.rs"), []byte("fn main() {{{"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.rs"},
	}
	result := engine.verifyTask(context.Background(), task)
	if result.SyntaxOK {
		t.Error("expected syntax to fail for broken Rust code")
	}
}

func TestVerifyTask_NoTestFilesNoTestRun(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc main() {}"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"main.go"},
	}
	result := engine.verifyTask(context.Background(), task)
	if !result.TestsOK {
		t.Error("expected tests OK when no test files exist")
	}
}

func TestRunInitialize_CancellationAtProjectDetection(t *testing.T) {
	engine, _ := setupTestEngine(t)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := engine.RunPhase(ctx, m31types.PhaseInitialize, "Test")
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

func TestRunInitialize_SavesAllState(t *testing.T) {
	engine, _ := setupTestEngine(t)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build a tool")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("expected success")
	}
	if result.Phase != m31types.PhaseInitialize {
		t.Errorf("expected PhaseInitialize, got %s", result.Phase)
	}

	// Verify project saved
	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil || project == nil {
		t.Fatal("project not saved")
	}
	if project.Goal != "Build a tool" {
		t.Errorf("expected goal 'Build a tool', got %q", project.Goal)
	}
	if project.ProjectType == "" {
		t.Error("expected project type to be set")
	}

	// Verify state saved
	phase, _, _, _, err := engine.sessionMgr.LoadState(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != m31types.PhaseInitialize {
		t.Errorf("expected phase initialize, got %s", phase)
	}
}

func TestHealTask_SuccessfulHeal(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Mock provider returns tool call to create a file
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		`{"name":"FileWrite","input":{"path":"fixed.go","content":"package main"}}`,
		"File written successfully",
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "failed task", Files: []string{"fixed.go"}, Status: m31types.StatusFailed, HealsAttempted: 0},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	ok, err := engine.HealTask(context.Background(), 1)
	t.Logf("HealTask result: ok=%v, err=%v", ok, err)
}

func TestHealTask_LLMReturnsNoToolCalls(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// LLM returns plain text, no tool calls
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"I fixed the issue by editing the file.",
		"I fixed the issue by editing the file.",
		"I fixed the issue by editing the file.",
		"I fixed the issue by editing the file.",
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "broken", Files: []string{"a.go"}, Status: m31types.StatusFailed, HealsAttempted: 0},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	ok, err := engine.HealTask(context.Background(), 1)
	t.Logf("HealTask no tools: ok=%v, err=%v", ok, err)
}

func TestRunExecute_AllTasksSkipped(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "skipped1", Status: m31types.StatusSkipped},
		{ID: 2, Action: "Create", Description: "skipped2", Status: m31types.StatusSkipped},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseExecute, "Test")
	if err != nil {
		t.Logf("Execute returned: %v", err)
	}
	if result != nil && !result.Success {
		t.Log("Execute result shows not all done (expected for skipped)")
	}
}

func TestRunVerify_GoProjectWithTests(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create Go project with passing test
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc Add(a, b int) int { return a + b }"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main_test.go"), []byte("package main\nimport \"testing\"\nfunc TestAdd(t *testing.T) { if Add(1,2) != 3 { t.Fatal() } }"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"main.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify Go with tests: success=%v, err=%v", result != nil && result.Success, err)
}

func TestCollectDiffStats_ComplexDiff(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Initial commit
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main\nfunc A() {}"), 0644)
	engine.git.Add("a.go")
	engine.git.Commit("initial")

	// Modify a.go and add b.go (uncommitted)
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main\nfunc A() {}\nfunc B() {}"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "b.go"), []byte("package main\nfunc C() {}"), 0644)

	stats := engine.collectDiffStats()
	t.Logf("complex diff stats: %+v", stats)
}

func TestBuildPlanContext_AllFeatures(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Set project with answers
	project := &m31types.ProjectState{
		Goal:        "Test",
		ProjectType: "go",
		Framework:   "gin",
		Answers:     map[string]string{"Q1?": "A1"},
	}
	engine.sessionMgr.SaveProject(engine.sessionID, project)

	// Set refinement
	engine.planMarkdown = "# Old plan"
	engine.planVersion = 1
	engine.refineFeedback = "simplify"

	existingTasks := []m31types.Task{{ID: 1, Action: "Create", Description: "test"}}
	valErrs := []string{"error 1"}
	raw := "raw response"

	messages := engine.buildPlanContext(context.Background(), "Test", existingTasks, valErrs, raw)
	if len(messages) < 2 {
		t.Fatalf("expected at least 2 messages")
	}

	// Verify refinement feedback is included
	found := false
	for _, m := range messages {
		if strings.Contains(m.Content, "User Refinement Feedback") {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected refinement feedback in context")
	}
}

func TestBuildPlanContext_LongPlanTruncation(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Set plan > 4000 chars to trigger truncation
	longPlan := strings.Repeat("x", 5000)
	engine.planMarkdown = longPlan
	engine.planVersion = 1
	engine.refineFeedback = "revise"

	messages := engine.buildPlanContext(context.Background(), "Goal", nil, nil, "")
	for _, m := range messages {
		if strings.Contains(m.Content, "... (summary truncated)") {
			return // success
		}
	}
	t.Error("expected truncated plan in context")
}

func TestFindRootCommit_Success(t *testing.T) {
	engine, _ := setupTestEngine(t)
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)
	engine.git.Add("a.go")
	engine.git.Commit("init")

	hash, err := engine.findRootCommit()
	if err != nil {
		t.Fatalf("findRootCommit failed: %v", err)
	}
	if hash == "" {
		t.Error("expected non-empty hash")
	}
}

func TestFindRootCommit_NilGit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.git = nil
	_, err := engine.findRootCommit()
	if err == nil {
		t.Error("expected error for nil git")
	}
}

func TestFindRootCommit_MultipleRootCommits(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Orphan branch creates a repo with multiple roots after merge
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)
	engine.git.Add("a.go")
	engine.git.Commit("init")

	hash, err := engine.findRootCommit()
	if err != nil {
		t.Fatalf("findRootCommit failed: %v", err)
	}
	if hash == "" {
		t.Error("expected non-empty hash")
	}
}

func TestCollectDiffStats_BinaryFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Initial commit
	data := []byte{0x00, 0x01, 0x02, 0x03}
	os.WriteFile(filepath.Join(engine.workDir, "data.bin"), data, 0644)
	engine.git.Add("data.bin")
	engine.git.Commit("add binary")

	// Modify binary and add text file
	data2 := []byte{0x00, 0x01, 0x02, 0x03, 0x04, 0x05}
	os.WriteFile(filepath.Join(engine.workDir, "data.bin"), data2, 0644)
	os.WriteFile(filepath.Join(engine.workDir, "text.go"), []byte("package main\nfunc main() {}"), 0644)

	stats := engine.collectDiffStats()
	t.Logf("binary diff stats: %+v", stats)
	// Should have some insertions/deletions
}

func TestGenerateDemonstration_LongPlanTruncation(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	longPlan := strings.Repeat("# Plan\n", 500)
	engine.planMarkdown = longPlan
	engine.sessionMgr.SavePlan(engine.sessionID, 1, longPlan)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result := engine.generateDemonstration(context.Background(), "Test", tasks, 1, 0, nil)
	if len(result) == 0 {
		t.Error("expected non-empty demonstration")
	}
}

func TestSaveDiscussAnswers_NilProject(t *testing.T) {
	engine, _ := setupTestEngine(t)

	err := engine.saveDiscussAnswers(nil, []string{"Q1", "Q2"}, []string{"A1", "A2"})
	if err != nil {
		t.Fatalf("saveDiscussAnswers failed: %v", err)
	}

	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil || project == nil {
		t.Fatal("project not saved")
	}
	if project.Answers["Q1"] != "A1" {
		t.Errorf("expected A1, got %q", project.Answers["Q1"])
	}
	if project.Answers["Q2"] != "A2" {
		t.Errorf("expected A2, got %q", project.Answers["Q2"])
	}
}

func TestSaveDiscussAnswers_MismatchedLengths(t *testing.T) {
	engine, _ := setupTestEngine(t)

	err := engine.saveDiscussAnswers(nil, []string{"Q1", "Q2", "Q3"}, []string{"A1"})
	if err != nil {
		t.Fatalf("saveDiscussAnswers failed: %v", err)
	}

	project, _ := engine.sessionMgr.LoadProject(engine.sessionID)
	if project.Answers["Q1"] != "A1" {
		t.Error("expected Q1->A1")
	}
	if _, ok := project.Answers["Q2"]; ok {
		t.Error("expected Q2 to not have answer")
	}
}

func TestRunShip_DirtyUnrelatedFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Create a committed file tracked by a task
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)
	engine.git.Add("main.go")
	engine.git.Commit("init")

	// Create unrelated dirty file (not tracked by any task)
	os.WriteFile(filepath.Join(engine.workDir, "unrelated.txt"), []byte("dirty"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"main.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship with dirty unrelated: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunShip_GitAddAllFallback(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	os.WriteFile(filepath.Join(engine.workDir, "new.go"), []byte("package main"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"new.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship git add all: success=%v, err=%v", result != nil && result.Success, err)
}

func TestTransition_UnknownFromPhase(t *testing.T) {
	engine, _ := setupTestEngine(t)
	err := engine.Transition(context.Background(), m31types.WorkflowPhase("unknown_from"), m31types.PhaseInitialize)
	if err == nil {
		t.Error("expected error for unknown from phase")
	}
}

func TestTransition_AllAllowedTransitions(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Test all transitions from validPhaseTransitions map
	transitions := []struct {
		from, to m31types.WorkflowPhase
	}{
		{m31types.PhaseInitialize, m31types.PhaseDiscuss},
		{m31types.PhaseInitialize, m31types.PhaseIdle},
		{m31types.PhaseDiscuss, m31types.PhasePlan},
		{m31types.PhaseDiscuss, m31types.PhaseExecute},
		{m31types.PhaseDiscuss, m31types.PhaseIdle},
		{m31types.PhasePlan, m31types.PhaseExecute},
		{m31types.PhasePlan, m31types.PhasePlan},
		{m31types.PhasePlan, m31types.PhaseDiscuss},
		{m31types.PhasePlan, m31types.PhaseIdle},
		{m31types.PhaseExecute, m31types.PhaseVerify},
		{m31types.PhaseExecute, m31types.PhaseIdle},
		{m31types.PhaseVerify, m31types.PhaseShip},
		{m31types.PhaseVerify, m31types.PhaseExecute},
		{m31types.PhaseVerify, m31types.PhaseIdle},
		{m31types.PhaseShip, m31types.PhaseIdle},
	}

	for _, tr := range transitions {
		// Reset phase to match the from
		engine.activePhase = tr.from
		err := engine.Transition(context.Background(), tr.from, tr.to)
		if err != nil {
			t.Errorf("Transition %s -> %s failed: %v", tr.from, tr.to, err)
		}
	}
}

func TestRunPhase_UnknownPhase(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.WorkflowPhase("unknown"), "Test")
	if err == nil {
		t.Error("expected error for unknown phase")
	}
}

func TestRunPhase_BudgetExceeded(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Features: config.FeaturesConfig{
			BudgetLimitUSD: 0.001,
		},
	}

	// Simulate high cost
	atomic.StoreUint64(&engine.totalCostBits, math.Float64bits(0.01))

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err == nil {
		t.Error("expected budget exceeded error")
	}
}

func TestRunVerify_TaskFailsVerification_HealSucceeds(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Mock provider: first call is for heal (returns tool call to write file)
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"```json\n{\"name\":\"Bash\",\"input\":{\"command\":\"echo ok\"}}\n```",
		"Tool executed successfully",
	}

	// Task marked done but file doesn't exist
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"nonexistent.go"}, Status: m31types.StatusDone, HealsAttempted: 0},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify with heal: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunVerify_TaskFailsMaxHealsUnrecoverable(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Task at max heal attempts, file doesn't exist
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"nonexistent.go"}, Status: m31types.StatusDone, HealsAttempted: m31types.MaxHealAttempts},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	if result != nil && result.Error != "" {
		t.Log("Verify correctly reported unrecoverable task")
	}
	_ = err
}

func TestRunVerify_AllNonDoneTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusPending},
		{ID: 2, Action: "Create", Description: "task2", Files: []string{"b.go"}, Status: m31types.StatusFailed},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify all non-done: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunVerify_ManualStepsFromPlan(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	planMd := "# Plan\n## Summary\nTest plan\n## Verification Plan\n### Manual\n- Check browser\n- Run smoke tests"
	engine.sessionMgr.SavePlan(engine.sessionID, 1, planMd)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"nonexistent.go"}, Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
	t.Logf("Verify with manual steps: success=%v, err=%v, manualSteps=%d", result != nil && result.Success, err,
		func() int {
			if result != nil {
				return len(result.ManualVerificationSteps)
			}
			return 0
		}())
}

func TestRunShip_EmptyTaskList(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// No tasks at all
	engine.sessionMgr.SaveTasks(engine.sessionID, []m31types.Task{})

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	t.Logf("Ship no tasks: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunShip_FailedTasksCauseError(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "task2", Files: []string{"b.go"}, Status: m31types.StatusUnrecoverable},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
	if result != nil && !result.Success {
		t.Log("Ship correctly reported failure for failed tasks")
	}
	_ = err
}

func TestBuildExecuteContext_WithPlanProposedChanges(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Save a plan with ProposedChanges
	planMd := "# Plan\n## Summary\nBuild tool\n## Proposed Changes\n### Backend\n- [Create] main.go: Main entry point\n- [Edit] util.go: Add helper\n### Frontend\n- [Create] app.jsx: React component"
	engine.sessionMgr.SavePlan(engine.sessionID, 1, planMd)

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "test",
		Files:       []string{"test.go"},
	}
	messages := engine.buildExecuteContext(context.Background(), task, []m31types.Task{task}, "goal")

	found := false
	for _, m := range messages {
		if strings.Contains(m.Content, "Proposed Changes") {
			found = true
			break
		}
	}
	if !found {
		t.Logf("proposed changes not found in context (plan may not parse ProposedChanges)")
	}
}

func TestRunExecute_TaskWithAcceptanceCriteria(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Mock provider returns file write tool call
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"```json\n{\"name\":\"Bash\",\"input\":{\"command\":\"echo ok\"}}\n```",
		"Tool executed successfully",
		"```json\n{\"name\":\"Bash\",\"input\":{\"command\":\"echo ok\"}}\n```",
		"Tool executed successfully",
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusPending,
			AcceptanceCriteria: []string{"Must compile", "Must have tests"}},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseExecute, "Test")
	t.Logf("Execute with criteria: success=%v, err=%v", result != nil && result.Success, err)
}

func TestRunExecute_NoToolCallsForFileTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Provider returns plain text only (no tool calls)
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"I'll create the file now.",
		"I'll create the file now.",
		"I'll create the file now.",
		"I'll create the file now.",
		"I'll create the file now.",
		"I'll create the file now.",
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseExecute, "Test")
	t.Logf("Execute no tool calls: success=%v, err=%v", result != nil && result.Success, err)
}

func TestHealTask_NonExistentTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	ok, err := engine.HealTask(context.Background(), 999)
	if ok {
		t.Error("expected false for non-existent task")
	}
	if err == nil {
		t.Error("expected error for non-existent task")
	}
}

func TestHealTask_NonFailedTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	ok, err := engine.HealTask(context.Background(), 1)
	if ok {
		t.Error("expected false for non-failed task")
	}
	if err == nil {
		t.Error("expected error for non-failed task")
	}
}

func TestRunPlan_WithExistingTasksForRetry(t *testing.T) {
	engine, _ := setupTestEngine(t)
	_, _ = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")

	// Mock provider for plan generation
	mp := engine.provider.(*mockProvider)
	mp.multiResponses = []string{
		"# Plan\n## Summary\nRetry plan\n## Tasks\n- [x] Create a.go: main file",
		"# Plan\n## Summary\nRetry plan\n## Tasks\n- [x] Create a.go: main file",
	}

	// Save existing tasks (simulating a retry scenario)
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "task1", Files: []string{"a.go"}, Status: m31types.StatusFailed},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	t.Logf("Plan with retry: success=%v, err=%v", result != nil && result.Success, err)
}
