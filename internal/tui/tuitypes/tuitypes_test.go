package tuitypes

import (
	"os"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/testutil"
	"github.com/eshanized/M31A/internal/types"
)

func TestScreen_Label(t *testing.T) {
	tests := []struct {
		screen Screen
		label  string
	}{
		{ScreenFirstRun, "Setup"},
		{ScreenREPL, "Chat"},
		{ScreenModelSelector, "Models"},
		{ScreenSettings, "Settings"},
		{ScreenResume, "Sessions"},
		{ScreenPermission, "Permission"},
		{ScreenPlan, "Plan"},
		{ScreenExecute, "Execute"},
		{ScreenVerify, "Verify"},
		{ScreenShip, "Ship"},
		{ScreenDiff, "Diff"},
		{ScreenLedger, "Ledger"},
		{ScreenRollback, "Rollback"},
		{ScreenGoalInput, "Goal"},
		{ScreenDiscuss, "Discuss"},
		{ScreenMetrics, "Metrics"},
		{ScreenConfig, "Config"},
		{ScreenHelp, "Help"},
		{ScreenBisect, "Bisect"},
		{ScreenNotifications, "Notifications"},
		{ScreenDashboard, "Dashboard"},
		{ScreenSessionDetail, "Session"},
		{ScreenFileExplorer, "Files"},
		{ScreenToolDetail, "Tool Output"},
		{ScreenPhaseModelPicker, "Model Setup"},
		{ScreenGhostPicker, "Ghost Picker"},
		{ScreenGhostOutput, "Ghost Output"},
		{ScreenConfirmQuit, "Confirm Quit"},
		{ScreenChatHistory, "Chat History"},
		{ScreenCommandPalette, "Commands"},
		{ScreenRuntimeCheck, "Runtime Check"},
		{ScreenHome, "Home"},
	}

	for _, tt := range tests {
		t.Run(tt.label, func(t *testing.T) {
			if got := tt.screen.Label(); got != tt.label {
				t.Errorf("Screen(%d).Label() = %q, want %q", tt.screen, got, tt.label)
			}
		})
	}
}

func TestScreen_Label_Unknown(t *testing.T) {
	s := Screen(999)
	if got := s.Label(); got != "Unknown" {
		t.Errorf("Screen(999).Label() = %q, want %q", got, "Unknown")
	}
}

func TestScreen_Constants(t *testing.T) {
	if ScreenFirstRun != 0 {
		t.Errorf("ScreenFirstRun = %d, want 0", ScreenFirstRun)
	}
	if ScreenREPL != 1 {
		t.Errorf("ScreenREPL = %d, want 1", ScreenREPL)
	}
	if ScreenHome != 32 {
		t.Errorf("ScreenHome = %d, want 32", ScreenHome)
	}
}

func TestAppMsg(t *testing.T) {
	msg := AppMsg{
		Screen:        ScreenREPL,
		Action:        "new_session",
		SessionID:     "session123",
		SaveKeychain:  true,
		ModelSelected: &ModelSelectedMsg{},
	}

	if msg.Screen != ScreenREPL {
		t.Errorf("Screen = %d, want %d", msg.Screen, ScreenREPL)
	}
	if msg.Action != "new_session" {
		t.Errorf("Action = %q, want %q", msg.Action, "new_session")
	}
	if msg.SessionID != "session123" {
		t.Errorf("SessionID = %q, want %q", msg.SessionID, "session123")
	}
	if !msg.SaveKeychain {
		t.Error("SaveKeychain = false, want true")
	}
}

func TestModelSelectedMsg(t *testing.T) {
	model := types.ModelInfo{
		ID:       "gpt-4",
		Provider: "openai",
		Name:     "GPT-4",
	}
	msg := ModelSelectedMsg{
		Model:    model,
		Provider: "openai",
	}

	if msg.Provider != "openai" {
		t.Errorf("Provider = %q, want %q", msg.Provider, "openai")
	}
	if msg.Model.ID != "gpt-4" {
		t.Errorf("Model.ID = %q, want %q", msg.Model.ID, "gpt-4")
	}
}

func TestProviderEntry(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		apiKey = "sk-123"
	}
	entry := ProviderEntry{
		ID:     "openai",
		APIKey: apiKey,
	}

	if entry.ID != "openai" {
		t.Errorf("ID = %q, want %q", entry.ID, "openai")
	}
	if entry.APIKey != apiKey {
		t.Errorf("APIKey = %q, want %q", entry.APIKey, apiKey)
	}
}

func TestFirstRunCompleteMsg(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		apiKey = "sk-123"
	}
	msg := FirstRunCompleteMsg{
		Providers: []ProviderEntry{
			{ID: "openai", APIKey: apiKey},
		},
		ModelID:         "gpt-4",
		SaveKeychain:    true,
		DefaultProvider: "openai",
	}

	if len(msg.Providers) != 1 {
		t.Errorf("len(Providers) = %d, want 1", len(msg.Providers))
	}
	if msg.ModelID != "gpt-4" {
		t.Errorf("ModelID = %q, want %q", msg.ModelID, "gpt-4")
	}
}

func TestHealthCheckTickMsg(t *testing.T) {
	now := time.Now()
	msg := HealthCheckTickMsg{Time: now}

	if !msg.Time.Equal(now) {
		t.Errorf("Time = %v, want %v", msg.Time, now)
	}
}

func TestHealthCheckResultMsg(t *testing.T) {
	msg := HealthCheckResultMsg{}

	if msg.Result.Status != "" {
		t.Errorf("Status = %q, want empty", msg.Result.Status)
	}
}

func TestRefreshCacheMsg(t *testing.T) {
	msg := RefreshCacheMsg{ProviderName: "openai"}

	if msg.ProviderName != "openai" {
		t.Errorf("ProviderName = %q, want %q", msg.ProviderName, "openai")
	}
}

func TestErrorMsg(t *testing.T) {
	err := &testError{msg: "test error"}
	msg := ErrorMsg{Err: err}

	if msg.Err == nil {
		t.Error("Err is nil")
	}
	if msg.Err.Error() != "test error" {
		t.Errorf("Err.Error() = %q, want %q", msg.Err.Error(), "test error")
	}
}

func TestPermissionRequestMsg(t *testing.T) {
	msg := PermissionRequestMsg{}

	if msg.Request.ToolName != "" {
		t.Errorf("Request.ToolName = %q, want empty", msg.Request.ToolName)
	}
}

func TestPermissionResponseMsg(t *testing.T) {
	msg := PermissionResponseMsg{}

	if msg.Response.RequestID != 0 {
		t.Errorf("Response.RequestID = %d, want 0", msg.Response.RequestID)
	}
}

func TestQuestionRequestMsg(t *testing.T) {
	msg := QuestionRequestMsg{
		ID:          1,
		Question:    "What is your name?",
		Header:      "Name",
		Options:     []string{"Alice", "Bob"},
		AllowCustom: true,
		TimeoutSecs: 30,
	}

	if msg.ID != 1 {
		t.Errorf("ID = %d, want 1", msg.ID)
	}
	if msg.Question != "What is your name?" {
		t.Errorf("Question = %q, want %q", msg.Question, "What is your name?")
	}
	if len(msg.Options) != 2 {
		t.Errorf("len(Options) = %d, want 2", len(msg.Options))
	}
}

func TestQuestionResponseMsg(t *testing.T) {
	msg := QuestionResponseMsg{Answer: "Alice"}

	if msg.Answer != "Alice" {
		t.Errorf("Answer = %q, want %q", msg.Answer, "Alice")
	}
}

func TestPhaseResultMsg(t *testing.T) {
	msg := PhaseResultMsg{
		Success: true,
		Error:   "",
	}

	if !msg.Success {
		t.Error("Success = false, want true")
	}
}

func TestPlanReadyMsg(t *testing.T) {
	msg := PlanReadyMsg{
		Tasks:        nil,
		CostEstimate: "$0.50",
		TimeEstimate: "5m",
	}

	if msg.CostEstimate != "$0.50" {
		t.Errorf("CostEstimate = %q, want %q", msg.CostEstimate, "$0.50")
	}
}

func TestGoalSubmittedMsg(t *testing.T) {
	msg := GoalSubmittedMsg{Goal: "Build a web app"}

	if msg.Goal != "Build a web app" {
		t.Errorf("Goal = %q, want %q", msg.Goal, "Build a web app")
	}
}

func TestPhaseModelPickedMsg(t *testing.T) {
	msg := PhaseModelPickedMsg{
		PlanningModelID:  "gpt-4",
		PlanningProvider: "openai",
		CodingModelID:    "claude-3",
		CodingProvider:   "anthropic",
	}

	if msg.PlanningModelID != "gpt-4" {
		t.Errorf("PlanningModelID = %q, want %q", msg.PlanningModelID, "gpt-4")
	}
	if msg.CodingModelID != "claude-3" {
		t.Errorf("CodingModelID = %q, want %q", msg.CodingModelID, "claude-3")
	}
}

func TestSlashCommandMsg(t *testing.T) {
	msg := SlashCommandMsg{
		Command:       "/help",
		AttachedFiles: 3,
	}

	if msg.Command != "/help" {
		t.Errorf("Command = %q, want %q", msg.Command, "/help")
	}
	if msg.AttachedFiles != 3 {
		t.Errorf("AttachedFiles = %d, want 3", msg.AttachedFiles)
	}
}

func TestHomeSubmitMsg(t *testing.T) {
	msg := HomeSubmitMsg{Text: "hello world"}

	if msg.Text != "hello world" {
		t.Errorf("Text = %q, want %q", msg.Text, "hello world")
	}
}

func TestToastMsg(t *testing.T) {
	msg := ToastMsg{
		Text:     "Operation successful",
		Duration: 5 * time.Second,
		Type:     "success",
	}

	if msg.Text != "Operation successful" {
		t.Errorf("Text = %q, want %q", msg.Text, "Operation successful")
	}
	if msg.Duration != 5*time.Second {
		t.Errorf("Duration = %v, want %v", msg.Duration, 5*time.Second)
	}
	if msg.Type != "success" {
		t.Errorf("Type = %q, want %q", msg.Type, "success")
	}
}

func TestToast(t *testing.T) {
	toast := Toast{
		ID:        1,
		Text:      "Test toast",
		Type:      "info",
		CreatedAt: time.Now(),
		Frame:     0,
		Duration:  5 * time.Second,
	}

	if toast.ID != 1 {
		t.Errorf("ID = %d, want 1", toast.ID)
	}
	if toast.Text != "Test toast" {
		t.Errorf("Text = %q, want %q", toast.Text, "Test toast")
	}
}

func TestFallbackEventMsg(t *testing.T) {
	msg := FallbackEventMsg{
		From:   "openai",
		To:     "anthropic",
		Reason: "rate limited",
	}

	if msg.From != "openai" {
		t.Errorf("From = %q, want %q", msg.From, "openai")
	}
	if msg.To != "anthropic" {
		t.Errorf("To = %q, want %q", msg.To, "anthropic")
	}
}

func TestSidebarFile(t *testing.T) {
	file := SidebarFile{
		Path:   "main.go",
		Status: "modified",
	}

	if file.Path != "main.go" {
		t.Errorf("Path = %q, want %q", file.Path, "main.go")
	}
	if file.Status != "modified" {
		t.Errorf("Status = %q, want %q", file.Status, "modified")
	}
}

func TestSidebarTodoItem(t *testing.T) {
	item := SidebarTodoItem{
		Content:  "Implement feature",
		Status:   "in_progress",
		Priority: "high",
		Source:   "task",
		TaskID:   1,
	}

	if item.Content != "Implement feature" {
		t.Errorf("Content = %q, want %q", item.Content, "Implement feature")
	}
	if item.Status != "in_progress" {
		t.Errorf("Status = %q, want %q", item.Status, "in_progress")
	}
}

func TestGhostResult(t *testing.T) {
	result := &GhostResult{
		Files: []GhostFile{
			{Path: "file1.go", Content: "content", Prompt: "prompt"},
		},
		Warnings: []string{"warning1"},
	}

	if len(result.Files) != 1 {
		t.Errorf("len(Files) = %d, want 1", len(result.Files))
	}
	if len(result.Warnings) != 1 {
		t.Errorf("len(Warnings) = %d, want 1", len(result.Warnings))
	}
}

func TestDiffScreenMsg(t *testing.T) {
	msg := DiffScreenMsg{
		Diff:  "diff content",
		Title: "My Diff",
		Lines: []string{"line1", "line2"},
	}

	if msg.Diff != "diff content" {
		t.Errorf("Diff = %q, want %q", msg.Diff, "diff content")
	}
	if msg.Title != "My Diff" {
		t.Errorf("Title = %q, want %q", msg.Title, "My Diff")
	}
}

func TestSessionDetailRequestMsg(t *testing.T) {
	msg := SessionDetailRequestMsg{}

	if msg.Session != nil {
		t.Error("Session should be nil")
	}
}

func TestIntentClassifiedMsg(t *testing.T) {
	msg := IntentClassifiedMsg{
		Input: "test input",
	}

	if msg.Input != "test input" {
		t.Errorf("Input = %q, want %q", msg.Input, "test input")
	}
}

type testError struct {
	msg string
}

func (e *testError) Error() string {
	return e.msg
}
