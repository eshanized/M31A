package subagent

import (
	"context"
	"encoding/json"
	"log/slog"
	"os"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/eshanized/M31A/pkg/types"
)

// --- worktree.go pure functions ---

func TestSanitizePath(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input, want string
	}{
		{"abc123", "abc123"},
		{"hello-world", "hello-world"},
		{"hello_world", "hello_world"},
		{"Hello World!", "Hello_World_"},
		{"a/b/c", "a_b_c"},
		{"..", "__"},
		{".", "_"},
		{"...", "___"},
		{"", "_invalid"},
		{"@#$%", "____"},
		{"agent-abc-123", "agent-abc-123"},
	}
	for _, tt := range tests {
		got := sanitizePath(tt.input)
		if got != tt.want {
			t.Errorf("sanitizePath(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestWorktreeBranchName(t *testing.T) {
	t.Parallel()
	tests := []struct {
		agentID, suffix, want string
	}{
		{"abc", "", "m31a/agent-abc"},
		{"abc", "fix", "m31a/agent-abc-fix"},
		{"abc", "my branch!", "m31a/agent-abc-my_branch_"},
	}
	for _, tt := range tests {
		got := worktreeBranchName(tt.agentID, tt.suffix)
		if got != tt.want {
			t.Errorf("worktreeBranchName(%q, %q) = %q, want %q", tt.agentID, tt.suffix, got, tt.want)
		}
	}
}

func TestParseWorktreeList(t *testing.T) {
	t.Parallel()
	porcelain := `worktree /path/to/worktree1
HEAD abc123
branch refs/heads/m31a/agent-abc
worktree /path/to/worktree2
HEAD def456
branch refs/heads/m31a/agent-def-fix
`
	branches, paths := parseWorktreeList(porcelain)
	if len(branches) != 2 {
		t.Fatalf("expected 2 branches, got %d: %v", len(branches), branches)
	}
	if !branches["m31a/agent-abc"] {
		t.Error("missing m31a/agent-abc")
	}
	if !branches["m31a/agent-def-fix"] {
		t.Error("missing m31a/agent-def-fix")
	}
	if len(paths) != 2 {
		t.Fatalf("expected 2 paths, got %d: %v", len(paths), paths)
	}
	if !paths["/path/to/worktree1"] {
		t.Error("missing /path/to/worktree1")
	}
	if !paths["/path/to/worktree2"] {
		t.Error("missing /path/to/worktree2")
	}
}

func TestParseWorktreeList_Empty(t *testing.T) {
	t.Parallel()
	branches, paths := parseWorktreeList("")
	if len(branches) != 0 {
		t.Errorf("expected empty branches, got %v", branches)
	}
	if len(paths) != 0 {
		t.Errorf("expected empty paths, got %v", paths)
	}
}

func TestParseWorktreeList_NoBranches(t *testing.T) {
	t.Parallel()
	porcelain := "worktree /some/path\nHEAD abc123\n\n"
	branches, paths := parseWorktreeList(porcelain)
	if len(branches) != 0 {
		t.Errorf("expected empty branches, got %v", branches)
	}
	if len(paths) != 1 {
		t.Errorf("expected 1 path, got %d: %v", len(paths), paths)
	}
}

func TestGitWorktrees_RootFor_Empty(t *testing.T) {
	t.Parallel()
	g := &GitWorktrees{}
	got := g.rootFor("/parent/dir")
	want := "/parent/dir/.m31a-worktrees"
	if got != want {
		t.Errorf("rootFor = %q, want %q", got, want)
	}
}

func TestGitWorktrees_RootFor_Custom(t *testing.T) {
	t.Parallel()
	g := &GitWorktrees{Root: "/custom/root"}
	got := g.rootFor("/parent/dir")
	if got != "/custom/root" {
		t.Errorf("rootFor = %q, want /custom/root", got)
	}
}

// --- manager.go methods ---

func testManager(t *testing.T) *Manager {
	t.Helper()
	return NewManager(Dependencies{
		WorkDir: t.TempDir(),
		Logger:  slog.Default(),
	})
}

func TestNewAgentID(t *testing.T) {
	t.Parallel()
	id1, err := newAgentID()
	if err != nil {
		t.Fatalf("newAgentID: %v", err)
	}
	id2, err := newAgentID()
	if err != nil {
		t.Fatalf("newAgentID: %v", err)
	}
	if id1 == id2 {
		t.Error("two agent IDs should be different")
	}
	if len(id1) != 6 {
		t.Errorf("agent ID length = %d, want 6", len(id1))
	}
}

func TestManager_Events(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	ch := m.Events()
	if ch == nil {
		t.Error("Events() returned nil channel")
	}
}

func TestManager_Get_NotFound(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	sa := m.Get("nonexistent")
	if sa != nil {
		t.Error("Get should return nil for unknown ID")
	}
}

func TestManager_Get_Found(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	id, _ := newAgentID()
	sa := &Subagent{Info: SubagentInfo{ID: id, Status: StatusRunning}}
	m.agents.Store(id, sa)
	got := m.Get(id)
	if got != sa {
		t.Error("Get should return the stored subagent")
	}
}

func TestManager_List_Empty(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	list := m.List()
	if len(list) != 0 {
		t.Errorf("List() returned %d items, want 0", len(list))
	}
}

func TestManager_List_WithData(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	id1, _ := newAgentID()
	id2, _ := newAgentID()
	m.agents.Store(id1, &Subagent{Info: SubagentInfo{ID: id1, Name: "a1"}})
	m.agents.Store(id2, &Subagent{Info: SubagentInfo{ID: id2, Name: "a2"}})
	list := m.List()
	if len(list) != 2 {
		t.Errorf("List() returned %d items, want 2", len(list))
	}
}

func TestManager_Cancel(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	ctx, cancel := context.WithCancel(context.Background())
	sa := &Subagent{
		Info:   SubagentInfo{ID: "test-cancel", Status: StatusRunning},
		cancel: cancel,
		done:   make(chan struct{}),
	}
	m.agents.Store("test-cancel", sa)
	m.Cancel("test-cancel")
	select {
	case <-ctx.Done():
	case <-time.After(time.Second):
		t.Error("Cancel should cancel the context")
	}
	m.Cancel("nonexistent") // should not panic
}

func TestManager_CancelAll(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	ctx1, cancel1 := context.WithCancel(context.Background())
	ctx2, cancel2 := context.WithCancel(context.Background())
	m.agents.Store("a1", &Subagent{cancel: cancel1, done: make(chan struct{})})
	m.agents.Store("a2", &Subagent{cancel: cancel2, done: make(chan struct{})})
	m.CancelAll()
	select {
	case <-ctx1.Done():
	case <-time.After(time.Second):
		t.Error("CancelAll should cancel first context")
	}
	_ = ctx2.Done()
}

func TestManager_Cleanup_NotFound(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	err := m.Cleanup(context.Background(), "nonexistent")
	if err != nil {
		t.Errorf("Cleanup should return nil for unknown ID, got %v", err)
	}
}

func TestManager_Cleanup_Found(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	id, _ := newAgentID()
	m.agents.Store(id, &Subagent{Info: SubagentInfo{ID: id, Status: StatusDone}})
	err := m.Cleanup(context.Background(), id)
	if err != nil {
		t.Errorf("Cleanup should return nil, got %v", err)
	}
	if m.Get(id) != nil {
		t.Error("Cleanup should remove the subagent")
	}
}

func TestManager_Cleanup_Worktree(t *testing.T) {
	t.Parallel()
	mock := &mockWorktreeOps{removed: make([]string, 0)}
	m := NewManager(Dependencies{
		WorkDir:   "/test/workdir",
		Logger:    slog.Default(),
		Worktrees: mock,
	})
	id, _ := newAgentID()
	m.agents.Store(id, &Subagent{
		Info: SubagentInfo{
			ID:        id,
			Status:    StatusDone,
			Isolation: IsolationWorktree,
			Worktree:  "/test/workdir/worktree1",
		},
	})
	err := m.Cleanup(context.Background(), id)
	if err != nil {
		t.Errorf("Cleanup should return nil, got %v", err)
	}
}

func TestManager_Cleanup_Worktree_SameAsWorkDir(t *testing.T) {
	t.Parallel()
	mock := &mockWorktreeOps{}
	m := NewManager(Dependencies{
		WorkDir:   "/test/workdir",
		Logger:    slog.Default(),
		Worktrees: mock,
	})
	id, _ := newAgentID()
	m.agents.Store(id, &Subagent{
		Info: SubagentInfo{
			ID:        id,
			Isolation: IsolationWorktree,
			Worktree:  "/test/workdir", // same as WorkDir
		},
	})
	_ = m.Cleanup(context.Background(), id)
	if len(mock.removed) != 0 {
		t.Error("should not remove worktree when path equals WorkDir")
	}
}

func TestManager_emit(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	ev := SubagentEvent{Type: EventTextDelta, AgentID: "test", Delta: "hello"}
	m.emit(ev)
	select {
	case got := <-m.Events():
		if got.AgentID != "test" || got.Delta != "hello" {
			t.Errorf("unexpected event: %+v", got)
		}
	case <-time.After(time.Second):
		t.Error("emit should deliver event")
	}
}

func TestManager_emit_Lifecycle(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	ev := SubagentEvent{Type: EventDone, AgentID: "test"}
	m.emit(ev)
	select {
	case got := <-m.Events():
		if got.Type != EventDone {
			t.Errorf("unexpected event type: %v", got.Type)
		}
	case <-time.After(time.Second):
		t.Error("lifecycle emit should deliver event")
	}
}

func TestManager_failAgent(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	sa := &Subagent{Info: SubagentInfo{ID: "f1", Name: "fail-test"}}
	m.failAgent(sa, context.DeadlineExceeded)
	if sa.Info.Status != StatusError {
		t.Errorf("status = %v, want StatusError", sa.Info.Status)
	}
	if sa.Info.LastError != context.DeadlineExceeded.Error() {
		t.Errorf("error = %q, want %q", sa.Info.LastError, context.DeadlineExceeded.Error())
	}
	select {
	case ev := <-m.Events():
		if ev.Type != EventError {
			t.Errorf("event type = %v, want EventError", ev.Type)
		}
	case <-time.After(time.Second):
		t.Error("failAgent should emit EventError")
	}
}

func TestManager_resolveProvider_NilRegistry(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	got := m.resolveProvider()
	if got != nil {
		t.Error("resolveProvider should return nil with nil registry")
	}
}

func TestManager_Spawn_NoDispatcher(t *testing.T) {
	t.Parallel()
	m := NewManager(Dependencies{
		WorkDir: t.TempDir(),
		Logger:  slog.Default(),
		ActiveModel: &types.ModelInfo{
			ID:   "test-model",
			Name: "Test Model",
		},
		Registry: newTestRegistry(),
	})
	_, _, err := m.Spawn(context.Background(), SpawnRequest{
		Description: "test",
		Prompt:      "test prompt",
		Background:  true,
	})
	if err != nil {
		t.Fatalf("Spawn should succeed: %v", err)
	}
	// The loop will fail because NewDispatcher is nil, but Spawn itself succeeds
	time.Sleep(50 * time.Millisecond)
}

func TestManager_Spawn_NoDescription(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	_, _, err := m.Spawn(context.Background(), SpawnRequest{
		Prompt: "test prompt",
	})
	if err == nil {
		t.Error("Spawn should fail without description")
	}
}

func TestManager_Spawn_NoPrompt(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	_, _, err := m.Spawn(context.Background(), SpawnRequest{
		Description: "test desc",
	})
	if err == nil {
		t.Error("Spawn should fail without prompt")
	}
}

func TestManager_Shutdown(t *testing.T) {
	t.Parallel()
	m := testManager(t)
	done := make(chan struct{})
	go func() {
		m.Shutdown(context.Background())
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Error("Shutdown should complete quickly with no agents")
	}
}

// --- loop.go methods ---

func TestLoop_ExtractSummary(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "test"}}
	m := &Manager{
		eventCh: make(chan SubagentEvent, 256),
		deps:    Dependencies{Logger: slog.Default()},
	}
	l := &loop{
		manager: m,
		agent:   sa,
		messages: []types.Message{
			{Role: "system", Content: "system prompt"},
			{Role: "user", Content: "user prompt"},
			{Role: "assistant", Content: "This is the final summary of findings."},
		},
	}
	got := l.extractSummary()
	if got != "This is the final summary of findings." {
		t.Errorf("extractSummary = %q", got)
	}
}

func TestLoop_ExtractSummary_Empty(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "test"}}
	l := &loop{
		agent:    sa,
		messages: []types.Message{},
	}
	got := l.extractSummary()
	if got != "" {
		t.Errorf("extractSummary should be empty, got %q", got)
	}
}

func TestLoop_ExtractSummary_NoAssistant(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "test"}}
	l := &loop{
		agent: sa,
		messages: []types.Message{
			{Role: "system", Content: "system"},
			{Role: "user", Content: "user"},
		},
	}
	got := l.extractSummary()
	if got != "" {
		t.Errorf("extractSummary should be empty, got %q", got)
	}
}

func TestLoop_BuildSystemPrompt(t *testing.T) {
	t.Parallel()
	sa := &Subagent{
		Info: SubagentInfo{ID: "test"},
		req:  SpawnRequest{Description: "test task", Name: "test-name"},
	}
	l := &loop{agent: sa, maxTools: 10}
	prompt := l.buildSystemPrompt()
	if !strings.Contains(prompt, "subagent") {
		t.Error("system prompt should mention subagent")
	}
	if !strings.Contains(prompt, "test task") {
		t.Error("system prompt should contain description")
	}
	if !strings.Contains(prompt, "test-name") {
		t.Error("system prompt should contain name")
	}
	if !strings.Contains(prompt, "10") {
		t.Error("system prompt should contain tool budget")
	}
}

func TestLoop_BuildSystemPrompt_NoName(t *testing.T) {
	t.Parallel()
	sa := &Subagent{
		Info: SubagentInfo{ID: "test"},
		req:  SpawnRequest{Description: "test task"},
	}
	l := &loop{agent: sa, maxTools: 5}
	prompt := l.buildSystemPrompt()
	if strings.Contains(prompt, "Name:") {
		t.Error("system prompt should not contain Name line when empty")
	}
}

func TestLoop_BuildToolDefinitions(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "test"}}
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "FileRead", Description: "read files", ParameterSchema: `{"type":"object"}`},
			{Name: "AskUserQuestion", Description: "ask user"},
			{Name: "Bash", Description: "run bash"},
		},
	}
	l := &loop{agent: sa, dispatcher: disp}
	defs := l.buildToolDefinitions()
	if len(defs) != 2 {
		t.Fatalf("expected 2 tool defs, got %d", len(defs))
	}
	if defs[0].Name != "FileRead" || defs[1].Name != "Bash" {
		t.Error("AskUserQuestion should be filtered out")
	}
}

func TestLoop_BuildToolDefinitions_EmptyParams(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "test"}}
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "Glob", Description: "glob files"},
		},
	}
	l := &loop{agent: sa, dispatcher: disp}
	defs := l.buildToolDefinitions()
	if len(defs) != 1 {
		t.Fatalf("expected 1 tool def, got %d", len(defs))
	}
	if defs[0].Parameters != "{}" {
		t.Errorf("empty params should default to '{}', got %q", defs[0].Parameters)
	}
}

func TestLoop_UpdateLastTool(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "test"}}
	l := &loop{
		agent:        sa,
		toolCallsRun: 3,
		inputToks:    100,
		outputToks:   50,
	}
	l.updateLastTool("Bash", "running")
	if sa.Info.LastToolName != "Bash" {
		t.Errorf("LastToolName = %q, want Bash", sa.Info.LastToolName)
	}
	if sa.Info.LastToolStatus != "running" {
		t.Errorf("LastToolStatus = %q, want running", sa.Info.LastToolStatus)
	}
	if sa.Info.ToolCalls != 3 {
		t.Errorf("ToolCalls = %d, want 3", sa.Info.ToolCalls)
	}
}

func TestLoop_FinishDone(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "done-test"}}
	m := &Manager{
		eventCh: make(chan SubagentEvent, 256),
		deps:    Dependencies{Logger: slog.Default()},
	}
	l := &loop{
		manager:      m,
		agent:        sa,
		messages:     []types.Message{{Role: "assistant", Content: "done summary"}},
		toolCallsRun: 5,
		inputToks:    200,
		outputToks:   100,
	}
	l.finishDone()
	if sa.Info.Status != StatusDone {
		t.Errorf("status = %v, want StatusDone", sa.Info.Status)
	}
	if sa.Info.LastSummary != "done summary" {
		t.Errorf("summary = %q, want done summary", sa.Info.LastSummary)
	}
	select {
	case ev := <-m.Events():
		if ev.Type != EventDone {
			t.Errorf("event type = %v, want EventDone", ev.Type)
		}
	case <-time.After(time.Second):
		t.Error("finishDone should emit EventDone")
	}
}

func TestLoop_FinishCancelled(t *testing.T) {
	t.Parallel()
	sa := &Subagent{Info: SubagentInfo{ID: "cancel-test"}}
	m := &Manager{
		eventCh: make(chan SubagentEvent, 256),
		deps:    Dependencies{Logger: slog.Default()},
	}
	l := &loop{manager: m, agent: sa}
	l.finishCancelled(context.Canceled)
	if sa.Info.Status != StatusCancel {
		t.Errorf("status = %v, want StatusCancel", sa.Info.Status)
	}
	if sa.Info.LastError != context.Canceled.Error() {
		t.Errorf("error = %q", sa.Info.LastError)
	}
	select {
	case ev := <-m.Events():
		if ev.Type != EventCancelled {
			t.Errorf("event type = %v, want EventCancelled", ev.Type)
		}
	case <-time.After(time.Second):
		t.Error("finishCancelled should emit EventCancelled")
	}
}

// --- events.go ---

func TestSubagentEvent_MarshalJSON(t *testing.T) {
	t.Parallel()
	ev := SubagentEvent{
		Type:      EventDone,
		AgentID:   "abc",
		Timestamp: time.Date(2025, 1, 1, 12, 0, 0, 0, time.UTC),
	}
	data, err := ev.MarshalJSON()
	if err != nil {
		t.Fatalf("MarshalJSON: %v", err)
	}
	var parsed SubagentEvent
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}
	if parsed.AgentID != "abc" {
		t.Errorf("agent_id = %q, want abc", parsed.AgentID)
	}
}

// --- abbreviate ---

func TestAbbreviate_Short(t *testing.T) {
	t.Parallel()
	got := abbreviate("hello", 10)
	if got != "hello" {
		t.Errorf("abbreviate short = %q, want hello", got)
	}
}

func TestAbbreviate_Long(t *testing.T) {
	t.Parallel()
	got := abbreviate("hello world", 5)
	if got != "hell…" {
		t.Errorf("abbreviate long = %q, want hell…", got)
	}
}

func TestAbbreviate_ZeroN(t *testing.T) {
	t.Parallel()
	got := abbreviate("hello", 0)
	if got != "" {
		t.Errorf("abbreviate zero = %q, want empty", got)
	}
}

// --- mock types ---

type mockDispatcher struct {
	tools []ToolDescriptor
}

func (d *mockDispatcher) Execute(_ context.Context, _ ToolCallInput) (ToolCallOutput, error) {
	return ToolCallOutput{Output: "ok"}, nil
}

func (d *mockDispatcher) ListTools() []ToolDescriptor {
	return d.tools
}

func (d *mockDispatcher) UnregisterTool(name string) {
	newTools := make([]ToolDescriptor, 0, len(d.tools))
	for _, t := range d.tools {
		if t.Name != name {
			newTools = append(newTools, t)
		}
	}
	d.tools = newTools
}

func (d *mockDispatcher) Stop() {}

type mockWorktreeOps struct {
	removed []string
	mu      sync.Mutex
}

func (m *mockWorktreeOps) Create(_ context.Context, _, _, _ string) (string, error) {
	dir, err := os.MkdirTemp("", "mock-worktree-*")
	if err != nil {
		return "", err
	}
	return dir, nil
}

func (m *mockWorktreeOps) Remove(_ context.Context, path string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.removed = append(m.removed, path)
	return nil
}

func (m *mockWorktreeOps) IsRepo(_ string) bool {
	return true
}
