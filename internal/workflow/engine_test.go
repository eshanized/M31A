package workflow

import (
	"context"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/tests/testutil/mocks"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
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
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	// Create dispatcher with tools
	dispatcher, err := tools.DefaultDispatcher(dir, filepath.Join(dir, "backups"), sessionBaseDir, nil, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	t.Cleanup(func() { dispatcher.Stop() })

	dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
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

	engine, _ := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		mocks.NewMockProvider("mock"), "test-model", dispatcher, est, mgr, nil)
	engine.SetGit(g)

	cleanup := func() {}
	return engine, cleanup
}

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
	t.Skip("removed: project-local sessions")
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
		name      string
		content   string
		wantLen   int
		wantFirst string
	}{
		{
			name:      "numbered format",
			content:   "1. What framework?\n2. What language?",
			wantLen:   2,
			wantFirst: "What framework?",
		},
		{
			name:      "fallback question",
			content:   "What framework should we use?\nHow about testing?",
			wantLen:   2,
			wantFirst: "What framework should we use?",
		},
		{
			name:      "cap at 4",
			content:   "1. Q1?\n2. Q2?\n3. Q3?\n4. Q4?\n5. Q5?",
			wantLen:   4,
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
	calls, err := eng.parseToolCalls(content)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("Expected 1 tool call, got %d", len(calls))
	}
	if calls[0].Name != "Bash" {
		t.Errorf("Expected Bash, got %s", calls[0].Name)
	}

	// Tool call with deeply nested input
	content2 := `Here is the call: {"name":"FileWrite","input":{"path":"test.go","content":"package main"}}`
	calls2, err := eng.parseToolCalls(content2)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(calls2) != 1 {
		t.Fatalf("Expected 1 tool call from text, got %d", len(calls2))
	}
	if calls2[0].Name != "FileWrite" {
		t.Errorf("Expected FileWrite, got %s", calls2[0].Name)
	}

	// No tool call — just text with braces
	content3 := `some {random} text without a name`
	calls3, err := eng.parseToolCalls(content3)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
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
		name     string
		tasks    []m31types.Task
		wantErrs int
	}{
		{
			name:     "valid task",
			tasks:    []m31types.Task{{ID: 1, Action: "Create", Description: "test", Dependencies: []int{}, AcceptanceCriteria: []string{"works"}}},
			wantErrs: 0,
		},
		{
			name:     "missing description",
			tasks:    []m31types.Task{{ID: 1, Action: "Create", Dependencies: []int{}, AcceptanceCriteria: []string{"works"}}},
			wantErrs: 1,
		},
		{
			name:     "self reference",
			tasks:    []m31types.Task{{ID: 1, Action: "Create", Description: "test", Dependencies: []int{1}, AcceptanceCriteria: []string{"works"}}},
			wantErrs: 2, // self-reference + circular
		},
		{
			name: "missing dep",
			tasks: []m31types.Task{
				{ID: 1, Action: "Create", Description: "a", Dependencies: []int{2}, AcceptanceCriteria: []string{"works"}},
			},
			wantErrs: 1,
		},
		{
			name: "circular",
			tasks: []m31types.Task{
				{ID: 1, Action: "Create", Description: "a", Dependencies: []int{2}, AcceptanceCriteria: []string{"works"}},
				{ID: 2, Action: "Create", Description: "b", Dependencies: []int{1}, AcceptanceCriteria: []string{"works"}},
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
	registry, err := LoadPrompts(config.PromptConfig{}, "")
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
}

// TestAutonomousPrompt_ReadOnlyGuidance locks in the read-only-task guidance
// in autonomous.md so the agent is told to respond in text (not call FileWrite
// for scratch artifacts) when asked to explain/summarize/review code.
// Regression here re-triggers the DESTRUCTIVE permission dialog on "explain
// the codebase" style requests.
func TestAutonomousPrompt_ReadOnlyGuidance(t *testing.T) {
	registry, err := LoadPrompts(config.PromptConfig{}, "")
	if err != nil {
		t.Fatalf("LoadPrompts failed: %v", err)
	}
	for _, want := range []string{
		"Read-only tasks",
		"Respond directly in text",
		"Never call FileWrite",
		"/tmp/",
	} {
		if !strings.Contains(registry.Autonomous, want) {
			t.Errorf("autonomous prompt missing %q", want)
		}
	}
	if !strings.Contains(registry.ToolUse, "Never use for") {
		t.Error("tool-use FileWrite section missing 'Never use for' guidance")
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
	planMsgs := engine.buildPlanContext(context.Background(), "test goal", nil, nil, "")
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

func TestConsumeStream_ErrorBeforeEOF(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Create a mock iterator that returns content then a non-EOF error
	callCount := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callCount++
			switch callCount {
			case 1:
				return &m31types.StreamChunk{Delta: "hello "}, nil
			case 2:
				return &m31types.StreamChunk{Delta: "world"}, io.ErrUnexpectedEOF
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}

	result, _, err := engine.consumeStream(iterator)
	if err != io.ErrUnexpectedEOF {
		t.Fatalf("expected io.ErrUnexpectedEOF, got %v", err)
	}
	// Partial content before error should be preserved
	if result != "hello world" {
		t.Errorf("expected 'hello world', got %q", result)
	}
}

func TestConsumeStream_NormalEOF(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	callCount := 0
	iterator := &m31types.StreamIterator{
		Next: func() (*m31types.StreamChunk, error) {
			callCount++
			switch callCount {
			case 1:
				return &m31types.StreamChunk{Delta: "foo"}, nil
			case 2:
				return &m31types.StreamChunk{Delta: "bar"}, nil
			default:
				return nil, io.EOF
			}
		},
		Close: func() error { return nil },
	}

	result, _, err := engine.consumeStream(iterator)
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if result != "foobar" {
		t.Errorf("expected 'foobar', got %q", result)
	}
}

func TestVerifyTask_ContextTimeout(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Create a Go file that compiles
	goFile := filepath.Join(engine.workDir, "main.go")
	os.WriteFile(goFile, []byte("package main\nfunc main() {}\n"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module testmod\ngo 1.21\n"), 0644)

	task := m31types.Task{
		ID:          1,
		Description: "test task",
		Action:      "create",
		Files:       []string{"main.go"},
	}

	result := engine.verifyTask(context.Background(), task)
	if !result.SyntaxOK {
		t.Errorf("expected SyntaxOK=true, got errors: %v", result.Errors)
	}
}

// mockProviderWithModel returns a provider that serves a specific ModelInfo.
type mockProviderWithModel struct {
	mocks.MockProvider
	model *m31types.ModelInfo
}

func (m *mockProviderWithModel) GetModel(id string) (*m31types.ModelInfo, error) {
	return m.model, nil
}
func (m *mockProviderWithModel) CachedModels() []m31types.ModelInfo {
	if m.model != nil {
		return []m31types.ModelInfo{*m.model}
	}
	return nil
}

func TestEngine_PreflightContextCheck(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Set up a provider that returns a model with small context window (100 tokens)
	engine.provider = &mockProviderWithModel{
		model: &m31types.ModelInfo{
			ID:            "test-model",
			ContextLength: 100,
		},
	}

	// Create messages that will exceed 95% of 100 tokens (95 tokens)
	// New heuristic: ~3.8 chars/token for unknown providers (prose)
	// Need > 95 * 3.8 = 361 chars to exceed the threshold.
	longContent := strings.Repeat("word ", 80) // 400 chars → ~105 tokens
	messages := []m31types.Message{
		{Role: "user", Content: longContent},
	}

	_, err := engine.preflightContextCheck(messages)
	if err == nil {
		t.Fatal("expected ErrContextExceeded, got nil")
	}
	if !errors.Is(err, m31errors.ErrContextExceeded) {
		t.Errorf("expected errors.Is(err, ErrContextExceeded), got: %v", err)
	}
}

func TestEngine_PreflightContextCheck_BelowThreshold(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Set up a provider that returns a model with large context window
	engine.provider = &mockProviderWithModel{
		model: &m31types.ModelInfo{
			ID:            "test-model",
			ContextLength: 128000,
		},
	}

	messages := []m31types.Message{
		{Role: "user", Content: "hello"},
	}

	_, err := engine.preflightContextCheck(messages)
	if err != nil {
		t.Errorf("expected nil error for small context, got: %v", err)
	}
}

func TestConsumeStreamWithTools_NativeToolCalls(t *testing.T) {
	engine, _ := setupTestEngine(t)

	chunks := []m31types.StreamChunk{
		{Type: "content", Delta: "I'll create the file."},
		{Type: "tool_call", Index: 0, ToolCallID: "call_abc", ToolName: "FileWrite", ToolInput: `{"path": "main.go"`},
		{Type: "tool_call", Index: 0, ToolCallID: "call_abc", ToolName: "FileWrite", ToolInput: `, "content": "package main"}`},
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

	content, toolCalls, _, err := engine.consumeStreamWithTools(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "I'll create the file." {
		t.Errorf("unexpected content: %q", content)
	}
	if len(toolCalls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(toolCalls))
	}
	if toolCalls[0].Name != "FileWrite" {
		t.Errorf("expected tool name FileWrite, got %q", toolCalls[0].Name)
	}
	if toolCalls[0].ID != "call_abc" {
		t.Errorf("expected tool call ID call_abc, got %q", toolCalls[0].ID)
	}
	if !strings.Contains(string(toolCalls[0].Input), `"path"`) {
		t.Errorf("expected input to contain path, got %q", string(toolCalls[0].Input))
	}
}

func TestConsumeStreamWithTools_MultipleTools(t *testing.T) {
	engine, _ := setupTestEngine(t)

	chunks := []m31types.StreamChunk{
		{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "Bash", ToolInput: `{"command": "ls"}`},
		{Type: "tool_call", Index: 1, ToolCallID: "call_2", ToolName: "FileRead", ToolInput: `{"path": "README.md"}`},
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

	content, toolCalls, _, err := engine.consumeStreamWithTools(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "" {
		t.Errorf("expected empty content, got %q", content)
	}
	if len(toolCalls) != 2 {
		t.Fatalf("expected 2 tool calls, got %d", len(toolCalls))
	}
	if toolCalls[0].Name != "Bash" {
		t.Errorf("expected first tool Bash, got %q", toolCalls[0].Name)
	}
	if toolCalls[1].Name != "FileRead" {
		t.Errorf("expected second tool FileRead, got %q", toolCalls[1].Name)
	}
}

func TestConsumeStreamWithTools_NoToolCalls(t *testing.T) {
	engine, _ := setupTestEngine(t)

	chunks := []m31types.StreamChunk{
		{Type: "content", Delta: "Just text, no tools."},
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

	content, toolCalls, _, err := engine.consumeStreamWithTools(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if content != "Just text, no tools." {
		t.Errorf("unexpected content: %q", content)
	}
	if len(toolCalls) != 0 {
		t.Errorf("expected 0 tool calls, got %d", len(toolCalls))
	}
}

func TestConsumeStreamWithTools_ToolNameNormalization(t *testing.T) {
	engine, _ := setupTestEngine(t)

	chunks := []m31types.StreamChunk{
		{Type: "tool_call", Index: 0, ToolCallID: "call_1", ToolName: "bash", ToolInput: `{"command": "echo hi"}`},
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

	_, toolCalls, _, err := engine.consumeStreamWithTools(iter)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(toolCalls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(toolCalls))
	}
	if toolCalls[0].Name != "Bash" {
		t.Errorf("expected normalized name Bash, got %q", toolCalls[0].Name)
	}
}

func TestFinalizeToolCalls_EmptyBuilders(t *testing.T) {
	engine, _ := setupTestEngine(t)
	result := finalizeToolCalls(map[int]*toolCallBuilder{}, engine)
	if result != nil {
		t.Errorf("expected nil for empty builders, got %v", result)
	}
}

func TestStreamLLMWithTools_SendsToolsInRequest(t *testing.T) {
	engine, _ := setupTestEngine(t)

	var capturedReq provider.ChatRequest
	mp := engine.provider.(*mocks.MockProvider)

	mp2 := &capturingProvider{
		inner: mp,
		onCall: func(req provider.ChatRequest) {
			capturedReq = req
		},
	}
	engine.provider = mp2

	_, _, _ = engine.streamLLMWithTools(context.Background(), []m31types.Message{
		{Role: "user", Content: "hello"},
	})

	if len(capturedReq.Tools) == 0 {
		t.Error("expected tools to be sent in request, got none")
	}
}

type capturingProvider struct {
	inner  *mocks.MockProvider
	onCall func(req provider.ChatRequest)
}

func (c *capturingProvider) Name() string   { return c.inner.Name() }
func (c *capturingProvider) APIKey() string { return c.inner.APIKey() }
func (c *capturingProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return c.inner.FetchModels(ctx)
}
func (c *capturingProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	c.onCall(req)
	return c.inner.ChatCompletionStream(ctx, req)
}
func (c *capturingProvider) EstimateCost(modelID string, usage m31types.Usage) float64 {
	return c.inner.EstimateCost(modelID, usage)
}
func (c *capturingProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return c.inner.HealthCheck(ctx)
}
func (c *capturingProvider) GetModel(id string) (*m31types.ModelInfo, error) {
	return c.inner.GetModel(id)
}
func (c *capturingProvider) CachedModels() []m31types.ModelInfo {
	return c.inner.CachedModels()
}

func TestEngine_SetWorkflowMode(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Default mode should be zero value
	if engine.WorkflowMode() != "" {
		t.Errorf("expected default workflow mode to be empty, got %v", engine.WorkflowMode())
	}

	// Set mode
	engine.SetWorkflowMode(m31types.ModeAuto)
	if engine.WorkflowMode() != m31types.ModeAuto {
		t.Errorf("expected workflow mode %v, got %v", m31types.ModeAuto, engine.WorkflowMode())
	}
}

func TestEngine_IntentResult(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Default should be nil
	if engine.IntentResult() != nil {
		t.Error("expected nil intent result by default")
	}

	// Set intent result
	ir := &m31types.IntentResult{
		Intent:     "feature",
		Complexity: "medium",
		Confidence: 0.85,
		Scope:      []string{"auth", "api"},
	}
	engine.SetIntentResult(ir)

	got := engine.IntentResult()
	if got == nil {
		t.Fatal("expected non-nil intent result")
	}
	if got.Intent != "feature" {
		t.Errorf("expected intent 'feature', got %q", got.Intent)
	}
}

func TestEngine_ScopeIncludes(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// No intent result — should return false
	if engine.ScopeIncludes("auth") {
		t.Error("ScopeIncludes should return false with nil intent result")
	}

	// Set intent with scope
	ir := &m31types.IntentResult{
		Scope: []string{"auth", "api", "database"},
	}
	engine.SetIntentResult(ir)

	if !engine.ScopeIncludes("auth") {
		t.Error("ScopeIncludes should return true for 'auth'")
	}
	if !engine.ScopeIncludes("api") {
		t.Error("ScopeIncludes should return true for 'api'")
	}
	if engine.ScopeIncludes("frontend") {
		t.Error("ScopeIncludes should return false for 'frontend'")
	}
}

func TestEngine_SetCollector(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Setting nil should be safe
	engine.SetCollector(nil)

	// Setting a collector should not panic
	// (We can't easily test the collector without mocking metrics)
}

func TestEngine_CompactedMessages(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Empty messages should return empty
	msgs := []m31types.Message{
		{Role: "system", Content: "You are a helpful assistant."},
		{Role: "user", Content: "Hello"},
		{Role: "assistant", Content: "Hi there!"},
	}

	result := engine.compactedMessages(msgs, "Previous conversation summarized.")
	if len(result) == 0 {
		t.Error("compactedMessages should return non-empty result")
	}
}
