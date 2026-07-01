package narrative

import (
	"testing"
	"time"
)

func TestTemplateResolverResolve(t *testing.T) {
	r := NewTemplateResolver()

	event := RawEvent{
		Type:      EventTaskStart,
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"description": "implement auth module",
		},
	}

	narrative := r.Resolve(NarrativeStartingTask, event)
	if narrative.Text == "" {
		t.Error("Resolve should produce non-empty text")
	}
	if narrative.Category != CategoryExecuting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryExecuting)
	}
	if narrative.Priority != PriorityTask {
		t.Errorf("Priority = %d, want %d", narrative.Priority, PriorityTask)
	}
}

func TestTemplateResolverReadFile(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"file": "main.go",
		},
	}

	narrative := r.Resolve(NarrativeReadingFile, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Display != DisplaySidebar {
		t.Errorf("Display = %q, want %q", narrative.Display, DisplaySidebar)
	}
}

func TestTemplateResolverWriteFile(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"file": "handler.go",
		},
	}

	narrative := r.Resolve(NarrativeWritingFile, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryExecuting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryExecuting)
	}
}

func TestTemplateResolverEditFile(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"file": "config.go",
		},
	}

	narrative := r.Resolve(NarrativeEditingFile, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
}

func TestTemplateResolverSearchingCode(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"pattern": "handleError",
		},
	}

	narrative := r.Resolve(NarrativeSearchingCode, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
}

func TestTemplateResolverRunningCommand(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"command": "go build ./...",
		},
	}

	narrative := r.Resolve(NarrativeRunningCommand, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryExecuting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryExecuting)
	}
}

func TestTemplateResolverSelfHealing(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"error": "build failed",
		},
	}

	narrative := r.Resolve(NarrativeSelfHealing, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryRecovering {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryRecovering)
	}
}

func TestTemplateResolverAgentStart(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"agent_type": "build",
		},
	}

	narrative := r.Resolve(NarrativeAgentStart, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Display != DisplaySidebar {
		t.Errorf("Display = %q, want %q", narrative.Display, DisplaySidebar)
	}
}

func TestTemplateResolverAgentDone(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"agent_type": "build",
		},
	}

	narrative := r.Resolve(NarrativeAgentDone, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryExecuting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryExecuting)
	}
}

func TestTemplateResolverGitCommit(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"message": "feat: add auth",
		},
	}

	narrative := r.Resolve(NarrativeCommitted, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryShipping {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryShipping)
	}
}

func TestTemplateResolverProviderError(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"error": "rate limited",
		},
	}

	narrative := r.Resolve(NarrativeProviderError, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryAlerting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryAlerting)
	}
	if narrative.Priority != PriorityActionRequired {
		t.Errorf("Priority = %d, want %d", narrative.Priority, PriorityActionRequired)
	}
}

func TestTemplateResolverProviderSwitch(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"from": "openrouter",
			"to":   "zen",
		},
	}

	narrative := r.Resolve(NarrativeProviderSwitch, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryAlerting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryAlerting)
	}
}

func TestTemplateResolverContextCompressed(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"tokens_saved": 5000,
		},
	}

	narrative := r.Resolve(NarrativeContextCompressed, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryLearning {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryLearning)
	}
}

func TestTemplateResolverAskingQuestion(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{},
	}

	narrative := r.Resolve(NarrativeAskingQuestion, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryDiscussing {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryDiscussing)
	}
}

func TestTemplateResolverPlanningWork(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{},
	}

	narrative := r.Resolve(NarrativePlanningWork, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryPlanning {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryPlanning)
	}
}

func TestTemplateResolverSearchingWeb(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"query": "Go bubbletea patterns",
		},
	}

	narrative := r.Resolve(NarrativeSearchingWeb, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
}

func TestTemplateResolverFetchingUrl(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"url": "https://example.com",
		},
	}

	narrative := r.Resolve(NarrativeFetchingUrl, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
}

func TestTemplateResolverTaskFailed(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"description": "test auth",
		},
	}

	narrative := r.Resolve(NarrativeTaskFailed, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryExecuting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryExecuting)
	}
}

func TestTemplateResolverLoopDetected(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data:     map[string]interface{}{},
	}

	narrative := r.Resolve(NarrativeLoopDetected, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Priority != PriorityActionRequired {
		t.Errorf("Priority = %d, want %d", narrative.Priority, PriorityActionRequired)
	}
}

func TestTemplateResolverChangelogReady(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"entry_count": 5,
		},
	}

	narrative := r.Resolve(NarrativeChangelogReady, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryShipping {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryShipping)
	}
}

func TestTemplateResolverSessionRestored(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"message_count": 10,
		},
	}

	narrative := r.Resolve(NarrativeSessionRestored, event)
	if narrative.Text == "" {
		t.Error("Text should not be empty")
	}
	if narrative.Category != CategoryOrienting {
		t.Errorf("Category = %q, want %q", narrative.Category, CategoryOrienting)
	}
}

func TestTemplateResolverAllTemplatesProduceOutput(t *testing.T) {
	r := NewTemplateResolver()
	event := RawEvent{
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"description": "test",
			"tool_name":   "Bash",
			"file":        "test.go",
			"pattern":     "TODO",
			"query":       "test query",
			"url":         "https://example.com",
			"agent_type":  "test-agent",
			"message":     "test message",
			"error":       "test error",
			"attempt":     1,
			"max_attempts": 3,
			"percentage":  50,
			"topic":       "test topic",
			"task_count":  5,
			"entry_count": 3,
			"tokens_saved": 100,
			"count":       5,
			"current":     1,
			"total":       3,
			"iteration":   1,
			"max_iterations": 3,
			"from":        "provider-a",
			"to":          "provider-b",
			"model":       "gpt-4",
			"lang":        "go",
			"file_count":  10,
			"test_count":  5,
			"passed":      4,
			"message_count": 10,
			"scope":       "test scope",
			"command":     "go build",
			"issue_count": 2,
		},
	}

	for _, nt := range AllNarrativeTypes() {
		t.Run(string(nt), func(t *testing.T) {
			narrative := r.Resolve(nt, event)
			if narrative.Text == "" {
				t.Errorf("NarrativeType %q produced empty text", nt)
			}
			if narrative.Type == "" {
				t.Errorf("NarrativeType %q produced empty Type field", nt)
			}
		})
	}
}
