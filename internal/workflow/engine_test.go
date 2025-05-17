package workflow

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

func setupTestEngine(t *testing.T) (*Engine, func()) {
	t.Helper()
	dir := t.TempDir()

	// Init git repo
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	// Create session
	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	// Create dispatcher with tools
	dispatcher := tools.NewDispatcher(nil)
	dispatcher.Register(tools.NewBash(dir))
	dispatcher.Register(tools.NewFileRead(dir))
	dispatcher.Register(tools.NewFileWrite(dir, filepath.Join(dir, "backups")))
	dispatcher.Register(tools.NewEdit(dir, filepath.Join(dir, "backups")))
	dispatcher.Register(tools.NewGlob(dir))
	dispatcher.Register(tools.NewGrep(dir))
	// Pre-approve all tools for tests
	dispatcher.SetPermission("Bash", true)
	dispatcher.SetPermission("FileRead", true)
	dispatcher.SetPermission("FileWrite", true)
	dispatcher.SetPermission("FileEdit", true)
	dispatcher.SetPermission("Glob", true)
	dispatcher.SetPermission("Grep", true)

	est := tokens.NewEstimator("test-model")

	engine, err := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		&mockProvider{}, "test-model", dispatcher, est, mgr)
	engine.git = g

	cleanup := func() {}
	return engine, cleanup
}

// Mock provider for testing
type mockProvider struct {
	response       string
	err            error
	callCount      int
	multiResponses []string // if set, returns responses[callCount] per call
}

func (m *mockProvider) Name() string                                       { return "mock" }
func (m *mockProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return nil, nil
}
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	m.callCount++
	content := m.response
	if len(m.multiResponses) > 0 {
		idx := m.callCount - 1
		if idx < len(m.multiResponses) {
			content = m.multiResponses[idx]
		}
	}
	if content == "" {
		content = "OK"
	}
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: content}, nil
	}
	close := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: close}, m.err
}
func (m *mockProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (m *mockProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return m31types.HealthStatus{Status: "live"}
}
func (m *mockProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }

func TestEngine_Initialization(t *testing.T) {
	engine, _ := setupTestEngine(t)

	if engine.sessionID == "" {
		t.Fatal("Expected non-empty session ID")
	}
	if engine.workDir == "" {
		t.Fatal("Expected non-empty workDir")
	}
}

func TestEngine_SessionDirSetup(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Planning directory should exist
	if _, err := os.Stat(engine.planningDir); os.IsNotExist(err) {
		t.Fatal("Planning directory should exist")
	}
}

func TestEngine_Transition(t *testing.T) {
	engine, _ := setupTestEngine(t)

	err := engine.Transition(context.Background(), m31types.PhaseIdle, m31types.PhaseInitialize)
	if err != nil {
		t.Fatalf("Transition failed: %v", err)
	}

	// STATE.md should be written
	phase, _, _, _, err := engine.sessionMgr.LoadState(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != m31types.PhaseInitialize {
		t.Errorf("Expected phase initialize, got %s", phase)
	}
}

func TestEngine_ContextPruning(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Each phase should build its own context
	messages := engine.buildDiscussContext("test goal")
	if len(messages) == 0 {
		t.Fatal("Expected non-empty messages")
	}

	// First message should be system prompt
	if messages[0].Role != "system" {
		t.Errorf("Expected system message first, got %s", messages[0].Role)
	}
	if !strings.Contains(messages[0].Content, "M31A") {
		t.Error("Expected system prompt to contain M31A identity")
	}
}

func TestEngine_SystemPromptInclusion(t *testing.T) {
	engine, _ := setupTestEngine(t)

	messages := engine.buildDiscussContext("goal")

	found := false
	for _, m := range messages {
		if m.Role == "system" && strings.Contains(m.Content, "M31A") {
			found = true
			break
		}
	}
	if !found {
		t.Error("System prompt not included in messages")
	}
}

func TestEngine_ErrorHandling(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Unknown phase should error
	_, err := engine.RunPhase(context.Background(), m31types.WorkflowPhase("unknown"), "goal")
	if err == nil {
		t.Fatal("Expected error for unknown phase")
	}
}

func TestEngine_ParseQuestions(t *testing.T) {
	tests := []struct {
		name     string
		content  string
		wantLen  int
		wantFirst string
	}{
		{
			name:     "numbered format",
			content:  "1. What framework?\n2. What language?",
			wantLen:  2,
			wantFirst: "What framework?",
		},
		{
			name:     "fallback question",
			content:  "What framework should we use?\nHow about testing?",
			wantLen:  2,
			wantFirst: "What framework should we use?",
		},
		{
			name:     "cap at 4",
			content:  "1. Q1?\n2. Q2?\n3. Q3?\n4. Q4?\n5. Q5?",
			wantLen:  4,
			wantFirst: "Q1?",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			questions := parseQuestions(tt.content)
			if len(questions) != tt.wantLen {
				t.Errorf("Expected %d questions, got %d: %v", tt.wantLen, len(questions), questions)
			}
			if len(questions) > 0 && questions[0] != tt.wantFirst {
				t.Errorf("Expected first question %q, got %q", tt.wantFirst, questions[0])
			}
		})
	}
}

func TestEngine_ParseTasksFromJSON(t *testing.T) {
	content := `[{"id":1,"action":"Create","description":"test","dependencies":[],"files":["a.go"],"acceptance_criteria":["works"]}]`

	tasks, err := parseTasksFromJSON(content)
	if err != nil {
		t.Fatalf("parseTasksFromJSON failed: %v", err)
	}
	if len(tasks) != 1 {
		t.Fatalf("Expected 1 task, got %d", len(tasks))
	}
	if tasks[0].ID != 1 {
		t.Errorf("Expected task ID 1, got %d", tasks[0].ID)
	}
}

func TestEngine_ParseToolCalls_NestedObjects(t *testing.T) {
	// Create a minimal engine for testing
	eng := &Engine{}

	// Tool call with nested input object — the old regex pattern 2 could not match this
	content := `{"name":"Bash","input":{"command":"echo hello"}}`
	calls := eng.parseToolCalls(content)
	if len(calls) != 1 {
		t.Fatalf("Expected 1 tool call, got %d", len(calls))
	}
	if calls[0].Name != "Bash" {
		t.Errorf("Expected Bash, got %s", calls[0].Name)
	}

	// Tool call with deeply nested input
	content2 := `Here is the call: {"name":"FileWrite","input":{"path":"test.go","content":"package main"}}`
	calls2 := eng.parseToolCalls(content2)
	if len(calls2) != 1 {
		t.Fatalf("Expected 1 tool call from text, got %d", len(calls2))
	}
	if calls2[0].Name != "FileWrite" {
		t.Errorf("Expected FileWrite, got %s", calls2[0].Name)
	}

	// No tool call — just text with braces
	content3 := `some {random} text without a name`
	calls3 := eng.parseToolCalls(content3)
	if len(calls3) != 0 {
		t.Errorf("Expected 0 tool calls, got %d", len(calls3))
	}
}

func TestEngine_StripCodeBlocks(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "json tag",
			input: "```json\n[{\"id\":1}]\n```",
			want:  "[{\"id\":1}]\n",
		},
		{
			name:  "go tag",
			input: "```go\nfunc main() {}\n```",
			want:  "func main() {}\n",
		},
		{
			name:  "python tag",
			input: "```python\nprint('hi')\n```",
			want:  "print('hi')\n",
		},
		{
			name:  "no tag",
			input: "```\nraw code\n```",
			want:  "raw code\n",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := stripCodeBlocks(tt.input)
			if got != tt.want {
				t.Errorf("Expected %q, got %q", tt.want, got)
			}
		})
	}
}

func TestEngine_ExtractJSONArray(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"direct", "[1,2,3]", "[1,2,3]"},
		{"wrapped", "Here is the list:\n[1,2,3]\nDone", "[1,2,3]"},
		{"empty", "", ""},
		{"brackets in string value", `[{"text":"use [brackets] here"}]`, `[{"text":"use [brackets] here"}]`},
		{"array after text", "Here are the tasks:\n[{\"id\":1}]\nDone", `[{"id":1}]`},
		{"nested array", `[{"items":[1,2,3]}]`, `[{"items":[1,2,3]}]`},
		{"escaped quotes", `[{"msg":"he said \"hi\""}]`, `[{"msg":"he said \"hi\""}]`},
		{"no brackets", "hello world", ""},
		{"unclosed array", "[1,2,3", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractJSONArray(tt.input)
			if got != tt.want {
				t.Errorf("Expected %q, got %q", tt.want, got)
			}
		})
	}
}

func TestEngine_ValidateTasks(t *testing.T) {
	tests := []struct {
		name      string
		tasks     []m31types.Task
		wantErrs  int
	}{
		{
			name:     "valid task",
			tasks:    []m31types.Task{{ID: 1, Action: "Create", Description: "test", Dependencies: []int{}}},
			wantErrs: 0,
		},
		{
			name:     "missing description",
			tasks:    []m31types.Task{{ID: 1, Action: "Create", Dependencies: []int{}}},
			wantErrs: 1,
		},
		{
			name:     "self reference",
			tasks:    []m31types.Task{{ID: 1, Action: "Create", Description: "test", Dependencies: []int{1}}},
			wantErrs: 2, // self-reference + circular
		},
		{
			name: "missing dep",
			tasks: []m31types.Task{
				{ID: 1, Action: "Create", Description: "a", Dependencies: []int{2}},
			},
			wantErrs: 1,
		},
		{
			name: "circular",
			tasks: []m31types.Task{
				{ID: 1, Action: "Create", Description: "a", Dependencies: []int{2}},
				{ID: 2, Action: "Create", Description: "b", Dependencies: []int{1}},
			},
			wantErrs: 1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			errs := validateTasks(tt.tasks)
			if len(errs) != tt.wantErrs {
				t.Errorf("Expected %d errors, got %d: %v", tt.wantErrs, len(errs), errs)
			}
		})
	}
}

func TestEngine_DetectProjectType(t *testing.T) {
	dir := t.TempDir()

	// Empty dir
	got := detectProjectType(dir)
	if got != "unknown" {
		t.Errorf("Expected unknown for empty dir, got %q", got)
	}

	// With go.mod
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte("module test"), 0644)
	got = detectProjectType(dir)
	if got != "go" {
		t.Errorf("Expected go, got %q", got)
	}
}

func TestEngine_FormatTaskSummary(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "test", Dependencies: []int{}, Status: m31types.StatusDone},
		{ID: 2, Action: "Add", Description: "test2", Dependencies: []int{1}, Status: m31types.StatusPending},
	}

	summary := formatTaskSummary(tasks)
	if !strings.Contains(summary, "test") {
		t.Errorf("Expected summary to contain task description, got %q", summary)
	}
	if !strings.Contains(summary, "done") {
		t.Errorf("Expected summary to contain status 'done', got %q", summary)
	}
}

func TestEngine_ListCwdFiles(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "a.go"), []byte("package main"), 0644)
	os.MkdirAll(filepath.Join(dir, "sub"), 0755)
	os.WriteFile(filepath.Join(dir, "sub", "b.go"), []byte("package sub"), 0644)

	list := listCwdFiles(dir)
	if !strings.Contains(list, "a.go") {
		t.Errorf("Expected a.go in file list, got %q", list)
	}
	if !strings.Contains(list, "sub/b.go") {
		t.Errorf("Expected sub/b.go in file list, got %q", list)
	}

	// Files beyond depth 3 should not appear
	os.MkdirAll(filepath.Join(dir, "d1", "d2", "d3"), 0755)
	os.WriteFile(filepath.Join(dir, "d1", "d2", "d3", "deep.go"), []byte("deep"), 0644)
	list = listCwdFiles(dir)
	if strings.Contains(list, "deep.go") {
		t.Errorf("Expected deep.go to be excluded (depth > 3), got %q", list)
	}

	// Files in node_modules should not appear
	os.MkdirAll(filepath.Join(dir, "node_modules", "pkg"), 0755)
	os.WriteFile(filepath.Join(dir, "node_modules", "pkg", "mod.go"), []byte("mod"), 0644)
	list = listCwdFiles(dir)
	if strings.Contains(list, "node_modules") {
		t.Errorf("Expected node_modules to be excluded, got %q", list)
	}
}

func TestEngine_HasCycle(t *testing.T) {
	// No cycle
	tasks := []m31types.Task{
		{ID: 1, Dependencies: []int{}},
		{ID: 2, Dependencies: []int{1}},
	}
	if hasCycle(tasks) {
		t.Error("Expected no cycle")
	}

	// Cycle
	tasks = []m31types.Task{
		{ID: 1, Dependencies: []int{2}},
		{ID: 2, Dependencies: []int{1}},
	}
	if !hasCycle(tasks) {
		t.Error("Expected cycle")
	}
}

func TestEngine_ReadTaskFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Create a file
	os.WriteFile(filepath.Join(engine.workDir, "test.go"), []byte("package main"), 0644)

	content := engine.readTaskFiles([]string{"test.go", "missing.go"})
	if !strings.Contains(content, "package main") {
		t.Errorf("Expected file content, got %q", content)
	}
	if !strings.Contains(content, "not found") {
		t.Errorf("Expected 'not found' for missing file, got %q", content)
	}
}

func TestEngine_HasTestFiles(t *testing.T) {
	dir := t.TempDir()

	// No test files
	if hasTestFiles(dir, []string{"main.go"}) {
		t.Error("Expected no test files")
	}

	// With test file
	os.WriteFile(filepath.Join(dir, "main_test.go"), []byte("package main"), 0644)
	if !hasTestFiles(dir, []string{"main.go"}) {
		t.Error("Expected test files found")
	}
}

func TestPromptRegistry_LoadPrompts(t *testing.T) {
	registry, err := LoadPrompts()
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	if registry.Base == "" {
		t.Error("Base prompt is empty")
	}
	if !strings.Contains(registry.Base, "M31A") {
		t.Error("Base prompt missing M31A identity")
	}
	if registry.ToolUse == "" {
		t.Error("ToolUse prompt is empty")
	}
	if registry.PlanFormat == "" {
		t.Error("PlanFormat prompt is empty")
	}
	if registry.ExecuteTask == "" {
		t.Error("ExecuteTask prompt is empty")
	}
	if registry.Discuss == "" {
		t.Error("Discuss prompt is empty")
	}
	if registry.SelfHeal == "" {
		t.Error("SelfHeal prompt is empty")
	}
	if registry.VerifyChecklist == "" {
		t.Error("VerifyChecklist prompt is empty")
	}
}

func TestEngine_BuildSystemPrompt(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Base only
	baseOnly := engine.buildSystemPrompt()
	if !strings.Contains(baseOnly, "M31A") {
		t.Error("Base-only prompt missing M31A identity")
	}

	// Base + one extra
	withExtra := engine.buildSystemPrompt("extra content")
	if !strings.Contains(withExtra, "extra content") {
		t.Error("Extra content not included")
	}
	if !strings.Contains(withExtra, "M31A") {
		t.Error("Base missing when extras added")
	}

	// Base + multiple extras, with empty string
	withMultiple := engine.buildSystemPrompt("first", "second", "")
	if !strings.Contains(withMultiple, "first") {
		t.Error("First extra not included")
	}
	if !strings.Contains(withMultiple, "second") {
		t.Error("Second extra not included")
	}
	if strings.Contains(withMultiple, "\n\n---\n\n---\n\n") {
		t.Error("Empty extra produced double separator")
	}
}

func TestEngine_PhasePromptComposition(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Discuss: base + discuss
	discussMsgs := engine.buildDiscussContext("test goal")
	if len(discussMsgs) == 0 || discussMsgs[0].Role != "system" {
		t.Fatal("Expected system message first")
	}
	sysContent := discussMsgs[0].Content
	if !strings.Contains(sysContent, "M31A") {
		t.Error("Discuss system prompt missing base identity")
	}
	if !strings.Contains(sysContent, "clarifying questions") {
		t.Error("Discuss system prompt missing discuss instructions")
	}

	// Plan: base + tool-use + plan-format
	planMsgs := engine.buildPlanContext("test goal", nil, nil, "")
	if len(planMsgs) == 0 || planMsgs[0].Role != "system" {
		t.Fatal("Expected system message first")
	}
	sysContent = planMsgs[0].Content
	if !strings.Contains(sysContent, "Bash") {
		t.Error("Plan system prompt missing tool-use instructions")
	}
	if !strings.Contains(sysContent, "JSON array") {
		t.Error("Plan system prompt missing plan format instructions")
	}
}
