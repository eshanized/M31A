package tuitypes

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/integrations/arbitrage"
	"github.com/eshanized/M31A/internal/core/types"
)

// --- Screen Name() method ---

func TestScreen_Name(t *testing.T) {
	t.Parallel()
	tests := []struct {
		screen Screen
		name   string
	}{
		{ScreenREPL, "repl"},
		{ScreenExecute, "execute"},
		{ScreenPlan, "plan"},
		{ScreenVerify, "verify"},
		{ScreenRuntimeCheck, "runtime"},
		{ScreenShip, "ship"},
		{ScreenDiscuss, "discuss"},
		{ScreenSettings, "settings"},
		{ScreenHelp, "help"},
		{ScreenChatHistory, "chathistory"},
		{ScreenConfig, "config"},
		{ScreenResume, "resume"},
		{ScreenRollback, "rollback"},
		{ScreenDiff, "diff"},
		{ScreenModelSelector, "modelselector"},
		{ScreenCommandPalette, "cmdpalette"},
		{ScreenPhaseModelPicker, "phasempicker"},
		{ScreenSessionDetail, "session"},
		{ScreenFileExplorer, "fileexplorer"},
		{ScreenConfirmQuit, "confirmquit"},
		{ScreenDashboard, "dashboard"},
		{ScreenMetrics, "metrics"},
		{ScreenLedger, "ledger"},
		{ScreenHome, "home"},
		{ScreenFirstRun, "firstrun"},
		{ScreenGoalInput, "goalinput"},
		{ScreenGhostPicker, "ghostpicker"},
		{ScreenGhostOutput, "ghostoutput"},
		{ScreenToolDetail, "tooldetail"},
		{ScreenNotifications, "notifications"},
		{ScreenBisect, "bisect"},
		{ScreenPermission, "permission"},
		{ScreenDecisions, "decisions"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			if got := tt.screen.Name(); got != tt.name {
				t.Errorf("Screen(%d).Name() = %q, want %q", tt.screen, got, tt.name)
			}
		})
	}
}

func TestScreen_Name_Unknown(t *testing.T) {
	t.Parallel()
	s := Screen(999)
	if got := s.Name(); got != "" {
		t.Errorf("Screen(999).Name() = %q, want empty", got)
	}
}

// --- Screen Label() edge cases ---

func TestScreen_Label_AllScreens(t *testing.T) {
	t.Parallel()
	screens := []Screen{
		ScreenFirstRun, ScreenREPL, ScreenModelSelector, ScreenSettings,
		ScreenResume, ScreenPermission, ScreenPlan, ScreenExecute,
		ScreenVerify, ScreenShip, ScreenDiff, ScreenLedger,
		ScreenRollback, ScreenGoalInput, ScreenDiscuss, ScreenMetrics,
		ScreenConfig, ScreenHelp, ScreenBisect, ScreenNotifications,
		ScreenDashboard, ScreenSessionDetail, ScreenFileExplorer,
		ScreenToolDetail, ScreenPhaseModelPicker, ScreenGhostPicker,
		ScreenGhostOutput, ScreenConfirmQuit, ScreenChatHistory,
		ScreenCommandPalette, ScreenRuntimeCheck, ScreenHome,
		ScreenDecisions,
	}
	for _, s := range screens {
		label := s.Label()
		if label == "" {
			t.Errorf("Screen(%d).Label() should not be empty", s)
		}
	}
}

// --- Screen constants ---

func TestScreen_Constants_Values(t *testing.T) {
	t.Parallel()
	if ScreenFirstRun != 0 {
		t.Errorf("ScreenFirstRun = %d, want 0", ScreenFirstRun)
	}
	if ScreenREPL != 1 {
		t.Errorf("ScreenREPL = %d, want 1", ScreenREPL)
	}
	if ScreenHome != 32 {
		t.Errorf("ScreenHome = %d, want 32", ScreenHome)
	}
	if ScreenDecisions != 33 {
		t.Errorf("ScreenDecisions = %d, want 33", ScreenDecisions)
	}
}

func TestScreen_UniqueValues(t *testing.T) {
	t.Parallel()
	screens := []Screen{
		ScreenFirstRun, ScreenREPL, ScreenModelSelector, ScreenSettings,
		ScreenResume, ScreenPermission, ScreenPlan, ScreenExecute,
		ScreenVerify, ScreenShip, ScreenDiff, ScreenLedger,
		ScreenRollback, ScreenGoalInput, ScreenDiscuss, ScreenMetrics,
		ScreenConfig, ScreenHelp, ScreenBisect, ScreenNotifications,
		ScreenDashboard, ScreenSessionDetail, ScreenFileExplorer,
		ScreenToolDetail, ScreenPhaseModelPicker, ScreenGhostPicker,
		ScreenGhostOutput, ScreenConfirmQuit, ScreenChatHistory,
		ScreenCommandPalette, ScreenRuntimeCheck, ScreenHome,
		ScreenDecisions,
	}
	seen := make(map[Screen]bool)
	for _, s := range screens {
		if seen[s] {
			t.Errorf("duplicate screen value: %d", s)
		}
		seen[s] = true
	}
}

// --- AppMsg ---

func TestAppMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := AppMsg{}
	if msg.Screen != 0 {
		t.Error("empty AppMsg Screen should be 0")
	}
	if msg.Action != "" {
		t.Error("empty AppMsg Action should be empty")
	}
	if msg.SessionID != "" {
		t.Error("empty AppMsg SessionID should be empty")
	}
	if msg.SaveKeychain {
		t.Error("empty AppMsg SaveKeychain should be false")
	}
}

func TestAppMsg_AllFields(t *testing.T) {
	t.Parallel()
	modelMsg := &ModelSelectedMsg{
		Model:    types.ModelInfo{ID: "gpt-4"},
		Provider: "openai",
	}
	msg := AppMsg{
		Screen:        ScreenSettings,
		Action:        "save_settings",
		SessionID:     "sess_123",
		SaveKeychain:  true,
		ModelSelected: modelMsg,
	}
	if msg.Screen != ScreenSettings {
		t.Errorf("Screen = %d, want %d", msg.Screen, ScreenSettings)
	}
	if msg.Action != "save_settings" {
		t.Errorf("Action = %q, want save_settings", msg.Action)
	}
	if !msg.SaveKeychain {
		t.Error("SaveKeychain should be true")
	}
	if msg.ModelSelected == nil {
		t.Error("ModelSelected should not be nil")
	}
}

// --- ModelSelectedMsg ---

func TestModelSelectedMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := ModelSelectedMsg{}
	if msg.Provider != "" {
		t.Error("empty ModelSelectedMsg Provider should be empty")
	}
}

func TestModelSelectedMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ModelSelectedMsg{
		Model:    types.ModelInfo{ID: "claude-3", Name: "Claude 3"},
		Provider: "anthropic",
	}
	if msg.Model.ID != "claude-3" {
		t.Errorf("Model.ID = %q, want claude-3", msg.Model.ID)
	}
	if msg.Provider != "anthropic" {
		t.Errorf("Provider = %q, want anthropic", msg.Provider)
	}
}

// --- ProviderEntry ---

func TestProviderEntry_Empty(t *testing.T) {
	t.Parallel()
	entry := ProviderEntry{}
	if entry.ID != "" {
		t.Error("empty ProviderEntry ID should be empty")
	}
	if entry.APIKey != "" {
		t.Error("empty ProviderEntry APIKey should be empty")
	}
}

func TestProviderEntry_Fields(t *testing.T) {
	t.Parallel()
	entry := ProviderEntry{ID: "openrouter", APIKey: "sk-test"}
	if entry.ID != "openrouter" {
		t.Errorf("ID = %q, want openrouter", entry.ID)
	}
	if entry.APIKey != "sk-test" {
		t.Errorf("APIKey = %q, want sk-test", entry.APIKey)
	}
}

// --- FirstRunCompleteMsg ---

func TestFirstRunCompleteMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := FirstRunCompleteMsg{}
	if len(msg.Providers) != 0 {
		t.Error("empty FirstRunCompleteMsg Providers should be empty")
	}
	if msg.ModelID != "" {
		t.Error("empty FirstRunCompleteMsg ModelID should be empty")
	}
}

func TestFirstRunCompleteMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := FirstRunCompleteMsg{
		Providers: []ProviderEntry{
			{ID: "openrouter", APIKey: "sk-test"},
			{ID: "zen", APIKey: "sk-zen"},
		},
		ModelID:         "gpt-4",
		SaveKeychain:    true,
		DefaultProvider: "openrouter",
	}
	if len(msg.Providers) != 2 {
		t.Errorf("len(Providers) = %d, want 2", len(msg.Providers))
	}
	if msg.ModelID != "gpt-4" {
		t.Errorf("ModelID = %q, want gpt-4", msg.ModelID)
	}
	if !msg.SaveKeychain {
		t.Error("SaveKeychain should be true")
	}
	if msg.DefaultProvider != "openrouter" {
		t.Errorf("DefaultProvider = %q, want openrouter", msg.DefaultProvider)
	}
}

// --- HealthCheckTickMsg ---

func TestHealthCheckTickMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := HealthCheckTickMsg{}
	if !msg.Time.IsZero() {
		t.Error("empty HealthCheckTickMsg Time should be zero")
	}
}

func TestHealthCheckTickMsg_Fields(t *testing.T) {
	t.Parallel()
	now := time.Now()
	msg := HealthCheckTickMsg{Time: now}
	if !msg.Time.Equal(now) {
		t.Error("Time should match")
	}
}

// --- HealthCheckResultMsg ---

func TestHealthCheckResultMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := HealthCheckResultMsg{}
	if msg.Result.Status != "" {
		t.Error("empty HealthCheckResultMsg Result.Status should be empty")
	}
}

// --- RefreshCacheMsg ---

func TestRefreshCacheMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := RefreshCacheMsg{}
	if msg.ProviderName != "" {
		t.Error("empty RefreshCacheMsg ProviderName should be empty")
	}
}

func TestRefreshCacheMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := RefreshCacheMsg{ProviderName: "openrouter"}
	if msg.ProviderName != "openrouter" {
		t.Errorf("ProviderName = %q, want openrouter", msg.ProviderName)
	}
}

// --- CacheRefreshResultMsg ---

func TestCacheRefreshResultMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := CacheRefreshResultMsg{ErrMsg: "timeout"}
	if msg.ErrMsg != "timeout" {
		t.Errorf("ErrMsg = %q, want timeout", msg.ErrMsg)
	}
}

// --- ErrorMsg ---

func TestErrorMsg_Nil(t *testing.T) {
	t.Parallel()
	msg := ErrorMsg{}
	if msg.Err != nil {
		t.Error("empty ErrorMsg Err should be nil")
	}
}

// --- PermissionRequestMsg ---

func TestPermissionRequestMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PermissionRequestMsg{}
	if msg.Request.ToolName != "" {
		t.Error("empty PermissionRequestMsg Request.ToolName should be empty")
	}
}

// --- PermissionResponseMsg ---

func TestPermissionResponseMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PermissionResponseMsg{}
	if msg.Response.RequestID != 0 {
		t.Error("empty PermissionResponseMsg Response.RequestID should be 0")
	}
}

// --- PermissionTickMsg ---

func TestPermissionTickMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PermissionTickMsg{}
	_ = msg // just verify it exists
}

// --- QuestionRequestMsg ---

func TestQuestionRequestMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := QuestionRequestMsg{}
	if msg.ID != 0 {
		t.Error("empty QuestionRequestMsg ID should be 0")
	}
	if msg.Question != "" {
		t.Error("empty QuestionRequestMsg Question should be empty")
	}
}

func TestQuestionRequestMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := QuestionRequestMsg{
		ID:          42,
		Question:    "What is your name?",
		Header:      "Name",
		Options:     []string{"Alice", "Bob", "Custom"},
		AllowCustom: true,
		TimeoutSecs: 30,
	}
	if msg.ID != 42 {
		t.Errorf("ID = %d, want 42", msg.ID)
	}
	if len(msg.Options) != 3 {
		t.Errorf("len(Options) = %d, want 3", len(msg.Options))
	}
	if !msg.AllowCustom {
		t.Error("AllowCustom should be true")
	}
	if msg.TimeoutSecs != 30 {
		t.Errorf("TimeoutSecs = %d, want 30", msg.TimeoutSecs)
	}
}

// --- QuestionResponseMsg ---

func TestQuestionResponseMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := QuestionResponseMsg{}
	if msg.Answer != "" {
		t.Error("empty QuestionResponseMsg Answer should be empty")
	}
}

func TestQuestionResponseMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := QuestionResponseMsg{Answer: "Alice"}
	if msg.Answer != "Alice" {
		t.Errorf("Answer = %q, want Alice", msg.Answer)
	}
}

// --- DiscussAnswerTimeoutMsg ---

func TestDiscussAnswerTimeoutMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := DiscussAnswerTimeoutMsg{QuestionIndex: 3}
	if msg.QuestionIndex != 3 {
		t.Errorf("QuestionIndex = %d, want 3", msg.QuestionIndex)
	}
}

// --- DiscussAnswerMsg ---

func TestDiscussAnswerMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := DiscussAnswerMsg{Index: 1, Answer: "yes"}
	if msg.Index != 1 {
		t.Errorf("Index = %d, want 1", msg.Index)
	}
	if msg.Answer != "yes" {
		t.Errorf("Answer = %q, want yes", msg.Answer)
	}
}

// --- DiscussCompleteMsg ---

func TestDiscussCompleteMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := DiscussCompleteMsg{}
	_ = msg
}

// --- PhaseResultMsg ---

func TestPhaseResultMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PhaseResultMsg{}
	if msg.Success {
		t.Error("empty PhaseResultMsg Success should be false")
	}
}

func TestPhaseResultMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := PhaseResultMsg{
		Phase:      types.PhaseExecute,
		Success:    true,
		DurationMs: 5000,
		Cost:       0.25,
		ToolCalls:  10,
	}
	if msg.Phase != types.PhaseExecute {
		t.Errorf("Phase = %v, want PhaseExecute", msg.Phase)
	}
	if !msg.Success {
		t.Error("Success should be true")
	}
	if msg.DurationMs != 5000 {
		t.Errorf("DurationMs = %d, want 5000", msg.DurationMs)
	}
	if msg.Cost != 0.25 {
		t.Errorf("Cost = %f, want 0.25", msg.Cost)
	}
	if msg.ToolCalls != 10 {
		t.Errorf("ToolCalls = %d, want 10", msg.ToolCalls)
	}
}

// --- PlanReadyMsg ---

func TestPlanReadyMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PlanReadyMsg{}
	if msg.CostEstimate != "" {
		t.Error("empty PlanReadyMsg CostEstimate should be empty")
	}
}

func TestPlanReadyMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := PlanReadyMsg{
		Tasks:        []types.Task{{ID: 1}},
		CostEstimate: "$0.50",
		TimeEstimate: "5m",
	}
	if len(msg.Tasks) != 1 {
		t.Errorf("len(Tasks) = %d, want 1", len(msg.Tasks))
	}
	if msg.CostEstimate != "$0.50" {
		t.Errorf("CostEstimate = %q, want $0.50", msg.CostEstimate)
	}
}

// --- PlanApproveMsg ---

func TestPlanApproveMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PlanApproveMsg{}
	_ = msg
}

// --- PlanRefineMsg ---

func TestPlanRefineMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := PlanRefineMsg{Feedback: "make it better"}
	if msg.Feedback != "make it better" {
		t.Errorf("Feedback = %q, want make it better", msg.Feedback)
	}
}

// --- ExecutePauseMsg ---

func TestExecutePauseMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ExecutePauseMsg{Paused: true}
	if !msg.Paused {
		t.Error("Paused should be true")
	}
}

// --- HealResultMsg ---

func TestHealResultMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := HealResultMsg{TaskID: 3, Success: true}
	if msg.TaskID != 3 {
		t.Errorf("TaskID = %d, want 3", msg.TaskID)
	}
	if !msg.Success {
		t.Error("Success should be true")
	}
}

// --- GoalSubmittedMsg ---

func TestGoalSubmittedMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := GoalSubmittedMsg{}
	if msg.Goal != "" {
		t.Error("empty GoalSubmittedMsg Goal should be empty")
	}
}

func TestGoalSubmittedMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := GoalSubmittedMsg{Goal: "Build a web app"}
	if msg.Goal != "Build a web app" {
		t.Errorf("Goal = %q, want Build a web app", msg.Goal)
	}
}

// --- PhaseModelPickedMsg ---

func TestPhaseModelPickedMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PhaseModelPickedMsg{}
	if msg.PlanningModelID != "" {
		t.Error("empty PhaseModelPickedMsg PlanningModelID should be empty")
	}
}

func TestPhaseModelPickedMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := PhaseModelPickedMsg{
		PlanningModelID:  "gpt-4",
		PlanningProvider: "openai",
		CodingModelID:    "claude-3",
		CodingProvider:   "anthropic",
	}
	if msg.PlanningModelID != "gpt-4" {
		t.Errorf("PlanningModelID = %q, want gpt-4", msg.PlanningModelID)
	}
	if msg.CodingModelID != "claude-3" {
		t.Errorf("CodingModelID = %q, want claude-3", msg.CodingModelID)
	}
}

// --- SlashCommandMsg ---

func TestSlashCommandMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := SlashCommandMsg{}
	if msg.Command != "" {
		t.Error("empty SlashCommandMsg Command should be empty")
	}
	if msg.AttachedFiles != 0 {
		t.Error("empty SlashCommandMsg AttachedFiles should be 0")
	}
}

func TestSlashCommandMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SlashCommandMsg{Command: "/help", AttachedFiles: 5}
	if msg.Command != "/help" {
		t.Errorf("Command = %q, want /help", msg.Command)
	}
	if msg.AttachedFiles != 5 {
		t.Errorf("AttachedFiles = %d, want 5", msg.AttachedFiles)
	}
}

// --- HomeSubmitMsg ---

func TestHomeSubmitMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := HomeSubmitMsg{}
	if msg.Text != "" {
		t.Error("empty HomeSubmitMsg Text should be empty")
	}
}

func TestHomeSubmitMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := HomeSubmitMsg{Text: "hello world"}
	if msg.Text != "hello world" {
		t.Errorf("Text = %q, want hello world", msg.Text)
	}
}

// --- ToastMsg ---

func TestToastMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := ToastMsg{}
	if msg.Text != "" {
		t.Error("empty ToastMsg Text should be empty")
	}
}

func TestToastMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ToastMsg{
		Text:     "Success!",
		Duration: 5 * time.Second,
		Type:     "success",
	}
	if msg.Text != "Success!" {
		t.Errorf("Text = %q, want Success!", msg.Text)
	}
	if msg.Duration != 5*time.Second {
		t.Errorf("Duration = %v, want 5s", msg.Duration)
	}
	if msg.Type != "success" {
		t.Errorf("Type = %q, want success", msg.Type)
	}
}

// --- Toast ---

func TestToast_Fields(t *testing.T) {
	t.Parallel()
	now := time.Now()
	toast := Toast{
		ID:        42,
		Text:      "Test toast",
		Type:      "info",
		CreatedAt: now,
		Frame:     2,
		Duration:  3 * time.Second,
	}
	if toast.ID != 42 {
		t.Errorf("ID = %d, want 42", toast.ID)
	}
	if toast.Text != "Test toast" {
		t.Errorf("Text = %q, want Test toast", toast.Text)
	}
	if toast.Frame != 2 {
		t.Errorf("Frame = %d, want 2", toast.Frame)
	}
}

// --- ToastExpiryMsg ---

func TestToastExpiryMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ToastExpiryMsg{ToastID: 5}
	if msg.ToastID != 5 {
		t.Errorf("ToastID = %d, want 5", msg.ToastID)
	}
}

// --- DismissToastMsg ---

func TestDismissToastMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := DismissToastMsg{ToastID: 3}
	if msg.ToastID != 3 {
		t.Errorf("ToastID = %d, want 3", msg.ToastID)
	}
}

// --- FallbackEventMsg ---

func TestFallbackEventMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := FallbackEventMsg{}
	if msg.From != "" {
		t.Error("empty FallbackEventMsg From should be empty")
	}
}

func TestFallbackEventMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := FallbackEventMsg{
		From:   "openrouter",
		To:     "zen",
		Reason: "rate limited",
	}
	if msg.From != "openrouter" {
		t.Errorf("From = %q, want openrouter", msg.From)
	}
	if msg.To != "zen" {
		t.Errorf("To = %q, want zen", msg.To)
	}
	if msg.Reason != "rate limited" {
		t.Errorf("Reason = %q, want rate limited", msg.Reason)
	}
}

// --- SettingsSavedMsg ---

func TestSettingsSavedMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := SettingsSavedMsg{}
	_ = msg
}

// --- ResetCompleteMsg ---

func TestResetCompleteMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := ResetCompleteMsg{}
	_ = msg
}

// --- OptimizedMsg ---

func TestOptimizedMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := OptimizedMsg{}
	if len(msg.Recommendations) != 0 {
		t.Error("empty OptimizedMsg Recommendations should be empty")
	}
}

func TestOptimizedMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := OptimizedMsg{
		Recommendations: []arbitrage.ArbitrageRecommendation{{}},
		TaskID:          1,
	}
	if len(msg.Recommendations) != 1 {
		t.Errorf("len(Recommendations) = %d, want 1", len(msg.Recommendations))
	}
	if msg.TaskID != 1 {
		t.Errorf("TaskID = %d, want 1", msg.TaskID)
	}
}

// --- BisectStartMsg ---

func TestBisectStartMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := BisectStartMsg{
		GoodCommit: "abc123",
		BadCommit:  "def456",
	}
	if msg.GoodCommit != "abc123" {
		t.Errorf("GoodCommit = %q, want abc123", msg.GoodCommit)
	}
	if msg.BadCommit != "def456" {
		t.Errorf("BadCommit = %q, want def456", msg.BadCommit)
	}
}

// --- SessionDetailRequestMsg ---

func TestSessionDetailRequestMsg_Nil(t *testing.T) {
	t.Parallel()
	msg := SessionDetailRequestMsg{}
	if msg.Session != nil {
		t.Error("empty SessionDetailRequestMsg Session should be nil")
	}
}

// --- DiffScreenMsg ---

func TestDiffScreenMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := DiffScreenMsg{}
	if msg.Diff != "" {
		t.Error("empty DiffScreenMsg Diff should be empty")
	}
}

func TestDiffScreenMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := DiffScreenMsg{
		Diff:  "+line1\n-line2",
		Title: "My Diff",
		Lines: []string{"+line1", "-line2"},
	}
	if msg.Diff != "+line1\n-line2" {
		t.Errorf("Diff = %q, want +line1\\n-line2", msg.Diff)
	}
	if msg.Title != "My Diff" {
		t.Errorf("Title = %q, want My Diff", msg.Title)
	}
	if len(msg.Lines) != 2 {
		t.Errorf("len(Lines) = %d, want 2", len(msg.Lines))
	}
}

// --- DiffCloseMsg ---

func TestDiffCloseMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := DiffCloseMsg{}
	_ = msg
}

// --- SidebarRefreshMsg ---

func TestSidebarRefreshMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := SidebarRefreshMsg{}
	if len(msg.Files) != 0 {
		t.Error("empty SidebarRefreshMsg Files should be empty")
	}
}

func TestSidebarRefreshMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SidebarRefreshMsg{
		Files:  []SidebarFile{{Path: "main.go", Status: "modified"}},
		Branch: "main",
		Remote: "origin",
	}
	if len(msg.Files) != 1 {
		t.Errorf("len(Files) = %d, want 1", len(msg.Files))
	}
	if msg.Branch != "main" {
		t.Errorf("Branch = %q, want main", msg.Branch)
	}
}

// --- SidebarFile ---

func TestSidebarFile_Fields(t *testing.T) {
	t.Parallel()
	file := SidebarFile{Path: "go.mod", Status: "added"}
	if file.Path != "go.mod" {
		t.Errorf("Path = %q, want go.mod", file.Path)
	}
	if file.Status != "added" {
		t.Errorf("Status = %q, want added", file.Status)
	}
}

// --- SidebarRefreshTickMsg ---

func TestSidebarRefreshTickMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := SidebarRefreshTickMsg{}
	_ = msg
}

// --- SidebarTodoUpdateMsg ---

func TestSidebarTodoUpdateMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SidebarTodoUpdateMsg{
		Items: []SidebarTodoItem{
			{Content: "Task 1", Status: "pending", Priority: "high"},
		},
	}
	if len(msg.Items) != 1 {
		t.Errorf("len(Items) = %d, want 1", len(msg.Items))
	}
}

// --- SidebarTodoItem ---

func TestSidebarTodoItem_Fields(t *testing.T) {
	t.Parallel()
	item := SidebarTodoItem{
		Content:  "Implement feature",
		Status:   "in_progress",
		Priority: "high",
		Source:   "task",
		TaskID:   1,
	}
	if item.Content != "Implement feature" {
		t.Errorf("Content = %q, want Implement feature", item.Content)
	}
	if item.Status != "in_progress" {
		t.Errorf("Status = %q, want in_progress", item.Status)
	}
	if item.Priority != "high" {
		t.Errorf("Priority = %q, want high", item.Priority)
	}
	if item.Source != "task" {
		t.Errorf("Source = %q, want task", item.Source)
	}
	if item.TaskID != 1 {
		t.Errorf("TaskID = %d, want 1", item.TaskID)
	}
}

// --- SidebarRevertMsg ---

func TestSidebarRevertMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := SidebarRevertMsg{}
	_ = msg
}

// --- SessionRenameMsg ---

func TestSessionRenameMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SessionRenameMsg{SessionID: "sess_123"}
	if msg.SessionID != "sess_123" {
		t.Errorf("SessionID = %q, want sess_123", msg.SessionID)
	}
}

// --- SessionExportMsg ---

func TestSessionExportMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SessionExportMsg{SessionID: "sess_456"}
	if msg.SessionID != "sess_456" {
		t.Errorf("SessionID = %q, want sess_456", msg.SessionID)
	}
}

// --- PopScreenMsg ---

func TestPopScreenMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := PopScreenMsg{}
	_ = msg
}

// --- ChatHistoryContinueMsg ---

func TestChatHistoryContinueMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ChatHistoryContinueMsg{MessageIndex: 5}
	if msg.MessageIndex != 5 {
		t.Errorf("MessageIndex = %d, want 5", msg.MessageIndex)
	}
}

// --- GhostWriteRequestMsg ---

func TestGhostWriteRequestMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := GhostWriteRequestMsg{Files: []string{"a.go", "b.go"}}
	if len(msg.Files) != 2 {
		t.Errorf("len(Files) = %d, want 2", len(msg.Files))
	}
}

// --- GhostWriteResultMsg ---

func TestGhostWriteResultMsg_Nil(t *testing.T) {
	t.Parallel()
	msg := GhostWriteResultMsg{}
	if msg.Result != nil {
		t.Error("empty GhostWriteResultMsg Result should be nil")
	}
}

func TestGhostWriteResultMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := GhostWriteResultMsg{
		Result: &GhostResult{
			Files:    []GhostFile{{Path: "out.go", Content: "code", Prompt: "prompt"}},
			Warnings: []string{"warn1"},
		},
	}
	if msg.Result == nil {
		t.Fatal("Result should not be nil")
	}
	if len(msg.Result.Files) != 1 {
		t.Errorf("len(Files) = %d, want 1", len(msg.Result.Files))
	}
	if len(msg.Result.Warnings) != 1 {
		t.Errorf("len(Warnings) = %d, want 1", len(msg.Result.Warnings))
	}
}

// --- GhostResult ---

func TestGhostResult_Empty(t *testing.T) {
	t.Parallel()
	result := &GhostResult{}
	if len(result.Files) != 0 {
		t.Error("empty GhostResult Files should be empty")
	}
	if len(result.Warnings) != 0 {
		t.Error("empty GhostResult Warnings should be empty")
	}
}

// --- GhostFile ---

func TestGhostFile_Fields(t *testing.T) {
	t.Parallel()
	file := GhostFile{
		Path:    "output.go",
		Content: "package main",
		Prompt:  "generate main",
	}
	if file.Path != "output.go" {
		t.Errorf("Path = %q, want output.go", file.Path)
	}
	if file.Content != "package main" {
		t.Errorf("Content = %q, want package main", file.Content)
	}
}

// --- IntentClassifiedMsg ---

func TestIntentClassifiedMsg_Empty(t *testing.T) {
	t.Parallel()
	msg := IntentClassifiedMsg{}
	if msg.Input != "" {
		t.Error("empty IntentClassifiedMsg Input should be empty")
	}
}

func TestIntentClassifiedMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := IntentClassifiedMsg{
		Input: "build a web app",
		Err:   nil,
	}
	if msg.Input != "build a web app" {
		t.Errorf("Input = %q, want build a web app", msg.Input)
	}
}
