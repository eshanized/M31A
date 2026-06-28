package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/types"
)

// ═══ Pure functions ═══

func TestOverlayToastOnContent(t *testing.T) {
	r := overlayToastOnContent("content", "toast", 80)
	if r == "" {
		t.Error("should not be empty")
	}
}

func TestOverlayToastOnContentEmpty(t *testing.T) {
	r := overlayToastOnContent("content", "", 80)
	if r == "" {
		t.Error("should not be empty")
	}
}

func TestTruncateToVisibleWidth(t *testing.T) {
	r := truncateToVisibleWidth("hello world", 5)
	if len(r) > 10 {
		t.Error("should truncate")
	}
}

func TestTruncateToVisibleWidthShort(t *testing.T) {
	r := truncateToVisibleWidth("hi", 80)
	if r != "hi" {
		t.Errorf("r=%s, want hi", r)
	}
}

func TestMaxInt(t *testing.T) {
	if maxInt(3, 5) != 5 {
		t.Error("maxInt(3,5) != 5")
	}
	if maxInt(7, 2) != 7 {
		t.Error("maxInt(7,2) != 7")
	}
	if maxInt(4, 4) != 4 {
		t.Error("maxInt(4,4) != 4")
	}
}

// ═══ View() paths ═══

func TestViewZeroSize(t *testing.T) {
	m := testAppState()
	m.width = 0
	m.height = 0
	r := m.View()
	// When dimensions are not yet known, View() should return "" and wait
	// for the first tea.WindowSizeMsg before rendering. This is the correct
	// BubbleTea pattern — an empty initial frame prevents layout artifacts.
	if r != "" {
		t.Error("View should return empty string for zero size (dimensions not yet set)")
	}
}

func TestViewUltraNarrow(t *testing.T) {
	m := testAppState()
	m.width = 39
	m.height = 24
	r := m.View()
	if r == "" {
		t.Error("View should not be empty for ultra narrow")
	}
}

func TestViewHeightTooSmall(t *testing.T) {
	m := testAppState()
	m.width = 80
	m.height = 9
	r := m.View()
	if r == "" {
		t.Error("View should not be empty for small height")
	}
}

func TestViewNormalREPL(t *testing.T) {
	m := testAppState()
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewSettings(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSettings
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewPlan(t *testing.T) {
	m := testAppState()
	m.screen = ScreenPlan
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewExecute(t *testing.T) {
	m := testAppState()
	m.screen = ScreenExecute
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewVerify(t *testing.T) {
	m := testAppState()
	m.screen = ScreenVerify
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewShip(t *testing.T) {
	m := testAppState()
	m.screen = ScreenShip
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewResume(t *testing.T) {
	m := testAppState()
	m.screen = ScreenResume
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewGoalInput(t *testing.T) {
	m := testAppState()
	m.screen = ScreenGoalInput
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewFirstRun(t *testing.T) {
	m := testAppState()
	m.screen = ScreenFirstRun
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewLedger(t *testing.T) {
	m := testAppState()
	m.screen = ScreenLedger
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewRollback(t *testing.T) {
	m := testAppState()
	m.screen = ScreenRollback
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewMetrics(t *testing.T) {
	m := testAppState()
	m.screen = ScreenMetrics
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewDiscuss(t *testing.T) {
	m := testAppState()
	m.screen = ScreenDiscuss
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewConfig(t *testing.T) {
	m := testAppState()
	m.screen = ScreenConfig
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewDiff(t *testing.T) {
	m := testAppState()
	m.screen = ScreenDiff
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewHelp(t *testing.T) {
	m := testAppState()
	m.screen = ScreenHelp
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewBisect(t *testing.T) {
	m := testAppState()
	m.screen = ScreenBisect
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewNotifications(t *testing.T) {
	m := testAppState()
	m.screen = ScreenNotifications
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewDashboard(t *testing.T) {
	m := testAppState()
	m.screen = ScreenDashboard
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewSessionDetail(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSessionDetail
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewFileExplorer(t *testing.T) {
	m := testAppState()
	m.screen = ScreenFileExplorer
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewToolDetail(t *testing.T) {
	m := testAppState()
	m.screen = ScreenToolDetail
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewPhaseModelPicker(t *testing.T) {
	m := testAppState()
	m.screen = ScreenPhaseModelPicker
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestViewPermission(t *testing.T) {
	m := testAppState()
	m.screen = ScreenPermission
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

// ═══ buildHeaderInfo ═══

func TestBuildHeaderInfo(t *testing.T) {
	m := testAppState()
	info := m.buildHeaderInfo()
	if info.Breadcrumb == "" {
		t.Error("Breadcrumb should not be empty")
	}
}

func TestBuildHeaderInfoWithModel(t *testing.T) {
	m := testAppState()
	m.activeModel = &types.ModelInfo{ID: "gpt-4", Name: "GPT-4"}
	m.activeProvider = "openai"
	info := m.buildHeaderInfo()
	if info.ModelName == "" {
		t.Error("ModelName should not be empty")
	}
}

func TestBuildHeaderInfoWithWorkflow(t *testing.T) {
	m := testAppState()
	m.workflowPhase = types.PhaseExecute
	info := m.buildHeaderInfo()
	_ = info
}

func TestBuildHeaderInfoWithPlanningModel(t *testing.T) {
	m := testAppState()
	m.planningModelID = "gpt-4"
	info := m.buildHeaderInfo()
	_ = info
}

func TestBuildHeaderInfoWithCodingModel(t *testing.T) {
	m := testAppState()
	m.codingModelID = "claude-3"
	info := m.buildHeaderInfo()
	_ = info
}

// ═══ buildFooterInfo ═══

func TestBuildFooterInfo(t *testing.T) {
	m := testAppState()
	info := m.buildFooterInfo()
	_ = info
}

func TestBuildFooterInfoWithThinking(t *testing.T) {
	m := testAppState()
	m.replModel.thinking = true
	info := m.buildFooterInfo()
	_ = info
}

func TestBuildFooterInfoWithStreaming(t *testing.T) {
	m := testAppState()
	m.replModel.streaming = true
	info := m.buildFooterInfo()
	_ = info
}

func TestBuildFooterInfoSettingsScreen(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSettings
	info := m.buildFooterInfo()
	_ = info
}

func TestBuildFooterInfoWithWorkflow(t *testing.T) {
	m := testAppState()
	m.workflowPhase = types.PhaseExecute
	info := m.buildFooterInfo()
	_ = info
}

func TestBuildFooterInfoWithCwd(t *testing.T) {
	m := testAppState()
	m.replModel.cwd = "/home/user/project"
	info := m.buildFooterInfo()
	_ = info
}

// ═══ renderPermissionModal ═══

func TestRenderPermissionModalNil(t *testing.T) {
	m := testAppState()
	m.screen = ScreenPermission
	r := m.renderPermissionModal()
	_ = r
}

func TestRenderPermissionModalWithRequest(t *testing.T) {
	m := testAppState()
	m.screen = ScreenPermission
	m.permRequest = &tools.PermissionRequest{
		ID:       1,
		ToolName: "bash",
		Command:  "rm -rf /",
	}
	m.permCountdown = 10
	m.permModalWidth = 60
	r := m.renderPermissionModal()
	if r == "" {
		t.Error("should not be empty")
	}
}

func TestRenderPermissionModalUrgent(t *testing.T) {
	m := testAppState()
	m.screen = ScreenPermission
	m.permRequest = &tools.PermissionRequest{
		ID:       1,
		ToolName: "bash",
		Command:  "rm -rf /",
	}
	m.permCountdown = 3
	m.permModalWidth = 60
	r := m.renderPermissionModal()
	if r == "" {
		t.Error("should not be empty")
	}
}

// ═══ renderQuestionModal ═══

func TestRenderQuestionModalNil(t *testing.T) {
	m := testAppState()
	r := m.renderQuestionModal()
	_ = r
}

func TestRenderQuestionModalWithRequest(t *testing.T) {
	m := testAppState()
	m.questionRequest = &QuestionRequestMsg{
		Question:    "What framework?",
		Header:      "Framework Selection",
		TimeoutSecs: 30,
	}
	m.permModalWidth = 60
	r := m.renderQuestionModal()
	if r == "" {
		t.Error("should not be empty")
	}
}

// ═══ ensureReplModel ═══

func TestEnsureReplModel(t *testing.T) {
	m := testAppState()
	m.replModel = nil
	m.ensureReplModel()
	if m.replModel == nil {
		t.Error("replModel should be created")
	}
}

func TestEnsureReplModelAlreadySet(t *testing.T) {
	m := testAppState()
	m.ensureReplModel()
	if m.replModel == nil {
		t.Error("replModel should still be set")
	}
}

// ═══ syncReplSize ═══

func TestSyncReplSize(t *testing.T) {
	m := testAppState()
	chrome := layout.PageChrome{}
	m.syncReplSize(chrome)
}

// ═══ with toast overlay ═══

func TestViewWithToasts(t *testing.T) {
	m := testAppState()
	m.addToast("test toast", "success")
	r := m.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}
