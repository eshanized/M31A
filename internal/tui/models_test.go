package tui

import (
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// ═══ plan_model.go ═══

func TestPlanModel(t *testing.T) {
	th := testTheme()
	tasks := []types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusPending},
		{ID: 2, Description: "task 2", Status: types.StatusPending, Dependencies: []int{1}},
	}
	pm := NewPlanModel(tasks, th, "gpt-4", "GPT-4", "openai", 0.05, "$0.05", 80, 24)
	if pm == nil {
		t.Fatal("NewPlanModel returned nil")
	}
	if len(pm.tasks) != 2 {
		t.Errorf("tasks=%d, want 2", len(pm.tasks))
	}
	if len(pm.waves) == 0 {
		t.Error("waves should not be empty")
	}
	pm.SetPlanContent("# Plan\n- Task 1\n- Task 2")
	if pm.planContent == "" {
		t.Error("planContent should be set")
	}
	pm.SetPlanVersion(2)
	if pm.planVersion != 2 {
		t.Errorf("planVersion=%d, want 2", pm.planVersion)
	}
	// WindowSizeMsg
	result, _ := pm.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	// View
	r := pm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestPlanModelEmptyTasks(t *testing.T) {
	th := testTheme()
	pm := NewPlanModel(nil, th, "", "", "", 0, "", 80, 24)
	if pm == nil {
		t.Fatal("nil")
	}
	if r := pm.View(); r == "" {
		t.Error("View should not be empty")
	}
}

func TestPlanModelSetDimensions(t *testing.T) {
	pm := NewPlanModel(nil, testTheme(), "", "", "", 0, "", 80, 24)
	pm.SetDimensions(100, 30)
	if pm.width != 100 || pm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestPlanModelUpdateEsc(t *testing.T) {
	pm := NewPlanModel(nil, testTheme(), "", "", "", 0, "", 80, 24)
	_, cmd := pm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

func TestPlanModelUpdateEnter(t *testing.T) {
	pm := NewPlanModel([]types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusPending},
	}, testTheme(), "", "", "", 0, "", 80, 24)
	pm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	pm.Update(tea.KeyMsg{Type: tea.KeyEnter})
}

func TestPlanModelComputeWaves(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "a"},
		{ID: 2, Description: "b"},
		{ID: 3, Description: "c", Dependencies: []int{1, 2}},
	}
	pm := NewPlanModel(tasks, testTheme(), "", "", "", 0, "", 80, 24)
	if len(pm.waves) < 2 {
		t.Errorf("waves=%d, want >= 2", len(pm.waves))
	}
}

func TestPlanModelUpdateTasks(t *testing.T) {
	pm := NewPlanModel([]types.Task{
		{ID: 1, Description: "a", Status: types.StatusPending},
	}, testTheme(), "", "", "", 0, "", 80, 24)
	pm.UpdateTasks([]types.Task{
		{ID: 1, Description: "a", Status: types.StatusDone},
		{ID: 2, Description: "b", Status: types.StatusPending},
	})
	if len(pm.tasks) != 2 {
		t.Errorf("tasks=%d, want 2", len(pm.tasks))
	}
}

// ═══ execute_model.go ═══

func TestExecuteModel(t *testing.T) {
	th := testTheme()
	tasks := []types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusPending},
		{ID: 2, Description: "task 2", Status: types.StatusPending},
	}
	em := NewExecuteModel(tasks, th, 80, 24)
	if em == nil {
		t.Fatal("NewExecuteModel returned nil")
	}
	if len(em.tasks) != 2 {
		t.Errorf("tasks=%d, want 2", len(em.tasks))
	}
	result, _ := em.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := em.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestExecuteModelUpdateTaskStatus(t *testing.T) {
	em := NewExecuteModel([]types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusPending},
	}, testTheme(), 80, 24)
	em.UpdateTaskStatus(1, types.StatusRunning)
	if em.tasks[0].Status != types.StatusRunning {
		t.Errorf("status=%s, want running", em.tasks[0].Status)
	}
	em.UpdateTaskStatus(1, types.StatusDone)
	if em.tasks[0].Status != types.StatusDone {
		t.Errorf("status=%s, want done", em.tasks[0].Status)
	}
}

func TestExecuteModelAppendLiveOutput(t *testing.T) {
	em := NewExecuteModel([]types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusRunning},
	}, testTheme(), 80, 24)
	em.currentTask = 0
	em.AppendLiveOutput([]string{"output line"})
	if len(em.liveOutput) != 1 {
		t.Errorf("liveOutput=%d, want 1", len(em.liveOutput))
	}
}

func TestExecuteModelAppendLiveOutputOverflow(t *testing.T) {
	em := NewExecuteModel([]types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusRunning},
	}, testTheme(), 80, 24)
	em.currentTask = 0
	lines := make([]string, 60)
	for i := range lines {
		lines[i] = "line"
	}
	em.AppendLiveOutput(lines)
	if len(em.liveOutput) > 50 {
		t.Errorf("liveOutput=%d, should be capped at 50", len(em.liveOutput))
	}
}

func TestExecuteModelSetCurrentTask(t *testing.T) {
	em := NewExecuteModel([]types.Task{
		{ID: 1, Description: "a"},
		{ID: 2, Description: "b"},
	}, testTheme(), 80, 24)
	em.SetCurrentTask(1)
	if em.currentTask != 1 {
		t.Errorf("currentTask=%d, want 1", em.currentTask)
	}
}

func TestExecuteModelUpdatePause(t *testing.T) {
	em := NewExecuteModel([]types.Task{
		{ID: 1, Description: "task 1", Status: types.StatusPending},
	}, testTheme(), 80, 24)
	em.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'p'}})
	if !em.paused {
		t.Error("p key should toggle pause")
	}
	em.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'p'}})
	if em.paused {
		t.Error("p key should toggle pause again")
	}
}

func TestExecuteModelUpdateEsc(t *testing.T) {
	em := NewExecuteModel(nil, testTheme(), 80, 24)
	_, cmd := em.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

func TestExecuteModelUpdateDown(t *testing.T) {
	em := NewExecuteModel(nil, testTheme(), 80, 24)
	em.Update(tea.KeyMsg{Type: tea.KeyDown})
}

func TestExecuteModelUpdateUp(t *testing.T) {
	em := NewExecuteModel(nil, testTheme(), 80, 24)
	em.Update(tea.KeyMsg{Type: tea.KeyUp})
}

// ═══ verify.go ═══

func TestVerifyModel(t *testing.T) {
	th := testTheme()
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: true, TestsOK: true},
	}
	vm := NewVerifyModel([]types.Task{
		{ID: 1, Description: "task 1"},
	}, results, th, 80, 24)
	if vm == nil {
		t.Fatal("NewVerifyModel returned nil")
	}
	r := vm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestVerifyModelEmpty(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	if vm == nil {
		t.Fatal("nil")
	}
	r := vm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestVerifyModelUpdateResults(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	newResults := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: false},
	}
	vm.UpdateResults(newResults)
	if len(vm.results) != 1 {
		t.Errorf("results=%d, want 1", len(vm.results))
	}
}

func TestVerifyModelSetManualSteps(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	vm.SetManualSteps([]string{"step 1", "step 2"})
	if len(vm.manualSteps) != 2 {
		t.Errorf("manualSteps=%d, want 2", len(vm.manualSteps))
	}
}

func TestVerifyModelSetHealFunc(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	vm.SetHealFunc(func(taskID int) tea.Cmd {
		return nil
	})
	if vm.healFunc == nil {
		t.Error("healFunc should be set")
	}
}

func TestVerifyModelStartHealing(t *testing.T) {
	vm := NewVerifyModel([]types.Task{
		{ID: 1, Description: "task 1"},
	}, map[int]workflow.VerificationResult{
		1: {TaskID: 1, TestsOK: false},
	}, testTheme(), 80, 24)
	vm.StartHealing(1, 1)
	if vm.healingTaskID != 1 {
		t.Errorf("healingTaskID=%d, want 1", vm.healingTaskID)
	}
}

func TestVerifyModelStopHealing(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	vm.healingTaskID = 5
	vm.StopHealing()
	if vm.healingTaskID != -1 {
		t.Errorf("healingTaskID=%d, want -1", vm.healingTaskID)
	}
}

func TestVerifyModelTickSpinner(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	vm.TickSpinner()
}

func TestVerifyModelUpdateEsc(t *testing.T) {
	vm := NewVerifyModel(nil, nil, testTheme(), 80, 24)
	_, cmd := vm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ ship_model.go ═══

func TestShipModel(t *testing.T) {
	th := testTheme()
	sm := NewShipModel(ShipSummary{
		TaskDone:  3,
		TaskTotal: 5,
	}, th, 80, 24)
	if sm == nil {
		t.Fatal("NewShipModel returned nil")
	}
	r := sm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestShipModelEmpty(t *testing.T) {
	sm := NewShipModel(ShipSummary{}, testTheme(), 80, 24)
	if sm == nil {
		t.Fatal("nil")
	}
	r := sm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestShipModelSetDemonstration(t *testing.T) {
	sm := NewShipModel(ShipSummary{}, testTheme(), 80, 24)
	sm.SetDemonstration("demo content")
	if sm.demonstration != "demo content" {
		t.Error("demonstration not set")
	}
}

func TestShipModelUpdateEsc(t *testing.T) {
	sm := NewShipModel(ShipSummary{}, testTheme(), 80, 24)
	_, cmd := sm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

func TestShipModelUpdateWindowSize(t *testing.T) {
	sm := NewShipModel(ShipSummary{}, testTheme(), 80, 24)
	result, _ := sm.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
}

// ═══ resume_model.go ═══

func TestResumeModel(t *testing.T) {
	th := testTheme()
	rm := NewResumeModel(nil, th)
	if rm == nil {
		t.Fatal("NewResumeModel returned nil")
	}
	if rm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := rm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestResumeModelSetTheme(t *testing.T) {
	rm := NewResumeModel(nil, testTheme())
	rm.SetTheme(testTheme())
}

func TestResumeModelSetDimensions(t *testing.T) {
	rm := NewResumeModel(nil, testTheme())
	rm.SetDimensions(100, 30)
	if rm.width != 100 || rm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestResumeModelUpdateUpDown(t *testing.T) {
	rm := NewResumeModel(nil, testTheme())
	rm.Update(tea.KeyMsg{Type: tea.KeyDown})
	rm.Update(tea.KeyMsg{Type: tea.KeyUp})
}

func TestResumeModelUpdateEsc(t *testing.T) {
	rm := NewResumeModel(nil, testTheme())
	_, cmd := rm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ fileexplorer_model.go ═══

func TestFileExplorerModel(t *testing.T) {
	th := testTheme()
	fem := NewFileExplorerModel(th, 80, 24)
	if fem == nil {
		t.Fatal("NewFileExplorerModel returned nil")
	}
	if fem.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := fem.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := fem.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestFileExplorerModelSetTheme(t *testing.T) {
	fem := NewFileExplorerModel(testTheme(), 80, 24)
	fem.SetTheme(testTheme())
}

func TestFileExplorerModelSetDimensions(t *testing.T) {
	fem := NewFileExplorerModel(testTheme(), 80, 24)
	fem.SetDimensions(100, 30)
	if fem.width != 100 || fem.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestFileExplorerModelUpdateUpDown(t *testing.T) {
	fem := NewFileExplorerModel(testTheme(), 80, 24)
	fem.Update(tea.KeyMsg{Type: tea.KeyDown})
	fem.Update(tea.KeyMsg{Type: tea.KeyUp})
}

func TestFileExplorerModelUpdateEsc(t *testing.T) {
	fem := NewFileExplorerModel(testTheme(), 80, 24)
	_, cmd := fem.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ sessiondetail_model.go ═══

func TestSessionDetailModel(t *testing.T) {
	th := testTheme()
	sdm := NewSessionDetailModel(th, 80, 24)
	if sdm == nil {
		t.Fatal("NewSessionDetailModel returned nil")
	}
	if sdm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := sdm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := sdm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestSessionDetailModelSetTheme(t *testing.T) {
	sdm := NewSessionDetailModel(testTheme(), 80, 24)
	sdm.SetTheme(testTheme())
}

func TestSessionDetailModelSetDimensions(t *testing.T) {
	sdm := NewSessionDetailModel(testTheme(), 80, 24)
	sdm.SetDimensions(100, 30)
	if sdm.width != 100 || sdm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestSessionDetailModelUpdateEsc(t *testing.T) {
	sdm := NewSessionDetailModel(testTheme(), 80, 24)
	_, cmd := sdm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ bisect_model.go ═══

func TestBisectModel(t *testing.T) {
	th := testTheme()
	bm := NewBisectModel(th, 80, 24)
	if bm == nil {
		t.Fatal("NewBisectModel returned nil")
	}
	if bm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := bm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := bm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestBisectModelSetTheme(t *testing.T) {
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetTheme(testTheme())
}

func TestBisectModelSetDimensions(t *testing.T) {
	bm := NewBisectModel(testTheme(), 80, 24)
	bm.SetDimensions(100, 30)
	if bm.width != 100 || bm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestBisectModelUpdateEsc(t *testing.T) {
	bm := NewBisectModel(testTheme(), 80, 24)
	_, cmd := bm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ theme_picker_model.go ═══

func TestThemePickerModel(t *testing.T) {
	th := testTheme()
	tpm := NewThemePickerModel(th, 80, 24)
	if tpm == nil {
		t.Fatal("NewThemePickerModel returned nil")
	}
	if tpm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := tpm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := tpm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestThemePickerSetTheme(t *testing.T) {
	tpm := NewThemePickerModel(testTheme(), 80, 24)
	tpm.SetTheme(testTheme())
}

func TestThemePickerSetDimensions(t *testing.T) {
	tpm := NewThemePickerModel(testTheme(), 80, 24)
	tpm.SetDimensions(100, 30)
	if tpm.width != 100 || tpm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestThemePickerUpdateUpDown(t *testing.T) {
	tpm := NewThemePickerModel(testTheme(), 80, 24)
	tpm.Update(tea.KeyMsg{Type: tea.KeyDown})
	tpm.Update(tea.KeyMsg{Type: tea.KeyUp})
}

func TestThemePickerUpdateEsc(t *testing.T) {
	tpm := NewThemePickerModel(testTheme(), 80, 24)
	_, cmd := tpm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ subagents_model.go ═══

func TestSubagentsModel(t *testing.T) {
	th := testTheme()
	sam := NewSubagentsModel(th)
	if sam == nil {
		t.Fatal("NewSubagentsModel returned nil")
	}
	if sam.IsEmpty() != true {
		t.Error("should be empty initially")
	}
}

func TestSubagentsModelSetTheme(t *testing.T) {
	sam := NewSubagentsModel(testTheme())
	sam.SetTheme(testTheme())
}

func TestSubagentsModelSetSize(t *testing.T) {
	sam := NewSubagentsModel(testTheme())
	sam.SetSize(100, 30)
	if sam.width != 100 || sam.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestSubagentsModelViewEmpty(t *testing.T) {
	sam := NewSubagentsModel(testTheme())
	sam.SetSize(80, 24)
	_ = sam.View()
}

func TestSubagentsModelMoveCursor(t *testing.T) {
	sam := NewSubagentsModel(testTheme())
	sam.MoveCursor(1)
	sam.MoveCursor(-1)
}

func TestSubagentsModelToggleExpand(t *testing.T) {
	sam := NewSubagentsModel(testTheme())
	sam.ToggleExpand()
}

func TestSubagentsModelSelected(t *testing.T) {
	sam := NewSubagentsModel(testTheme())
	s := sam.Selected()
	if s != nil {
		t.Error("Selected should be nil when empty")
	}
}

// ═══ tooldetail_model.go ═══

func TestToolDetailModel(t *testing.T) {
	th := testTheme()
	tdm := NewToolDetailModel(th, 80, 24)
	if tdm == nil {
		t.Fatal("NewToolDetailModel returned nil")
	}
	if tdm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := tdm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := tdm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestToolDetailModelSetTheme(t *testing.T) {
	tdm := NewToolDetailModel(testTheme(), 80, 24)
	tdm.SetTheme(testTheme())
}

func TestToolDetailModelSetDimensions(t *testing.T) {
	tdm := NewToolDetailModel(testTheme(), 80, 24)
	tdm.SetDimensions(100, 30)
	if tdm.width != 100 || tdm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestToolDetailModelSetContent(t *testing.T) {
	tdm := NewToolDetailModel(testTheme(), 80, 24)
	tdm.SetContent("title", "content")
	r := tdm.View()
	if r == "" {
		t.Error("View should not be empty after SetContent")
	}
}

func TestToolDetailModelUpdateEsc(t *testing.T) {
	tdm := NewToolDetailModel(testTheme(), 80, 24)
	_, cmd := tdm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ dashboard_model.go ═══

func TestDashboardModel(t *testing.T) {
	th := testTheme()
	dm := NewDashboardModel(th, 80, 24)
	if dm == nil {
		t.Fatal("NewDashboardModel returned nil")
	}
	if dm.Init() != nil {
		t.Error("Init should be nil")
	}
	result, _ := dm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := dm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestDashboardModelSetTheme(t *testing.T) {
	dm := NewDashboardModel(testTheme(), 80, 24)
	dm.SetTheme(testTheme())
}

func TestDashboardModelSetDimensions(t *testing.T) {
	dm := NewDashboardModel(testTheme(), 80, 24)
	dm.SetDimensions(100, 30)
	if dm.width != 100 || dm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestDashboardModelSetWorkflowState(t *testing.T) {
	dm := NewDashboardModel(testTheme(), 80, 24)
	dm.SetWorkflowState(types.PhaseExecute, "build app", "gpt-4", "openai")
	if dm.goal != "build app" {
		t.Error("goal not set")
	}
}

func TestDashboardModelUpdateUpDown(t *testing.T) {
	dm := NewDashboardModel(testTheme(), 80, 24)
	dm.Update(tea.KeyMsg{Type: tea.KeyDown})
	dm.Update(tea.KeyMsg{Type: tea.KeyUp})
}

func TestDashboardModelUpdateEsc(t *testing.T) {
	dm := NewDashboardModel(testTheme(), 80, 24)
	_, cmd := dm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ discuss.go ═══

func TestDiscussModel(t *testing.T) {
	th := testTheme()
	dm := NewDiscussModel(th, []string{"Q1?", "Q2?"}, 80, 24)
	if dm == nil {
		t.Fatal("NewDiscussModel returned nil")
	}
	if dm.Init() == nil {
		t.Error("Init should not be nil (blinks)")
	}
	result, _ := dm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	r := dm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestDiscussModelSetTheme(t *testing.T) {
	dm := NewDiscussModel(testTheme(), nil, 80, 24)
	dm.SetTheme(testTheme())
}

func TestDiscussModelSetDimensions(t *testing.T) {
	dm := NewDiscussModel(testTheme(), nil, 80, 24)
	dm.SetDimensions(100, 30)
	if dm.width != 100 || dm.height != 30 {
		t.Error("dimensions wrong")
	}
}

func TestDiscussModelSetTimeout(t *testing.T) {
	dm := NewDiscussModel(testTheme(), nil, 80, 24)
	dm.SetTimeout(30)
}

func TestDiscussModelUpdateDown(t *testing.T) {
	dm := NewDiscussModel(testTheme(), []string{"Q1?", "Q2?"}, 80, 24)
	dm.Update(tea.KeyMsg{Type: tea.KeyDown})
}

func TestDiscussModelUpdateEsc(t *testing.T) {
	dm := NewDiscussModel(testTheme(), nil, 80, 24)
	_, cmd := dm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Error("Esc should return cmd")
	}
}

// ═══ settings_tabs.go ═══

func TestSettingsTabConstants(t *testing.T) {
	tabs := []SettingsTab{TabProvider, TabModel, TabUI, TabKeys, TabWorkflow, TabAbout}
	for _, tab := range tabs {
		if tab < 0 || tab > TabAbout {
			t.Errorf("unexpected tab value: %d", tab)
		}
	}
}

func TestSettingsTabNames(t *testing.T) {
	if len(settingsTabNames) != 6 {
		t.Errorf("tab names count=%d, want 6", len(settingsTabNames))
	}
}

// ═══ repl_model.go / repl_state.go ═══

func TestReplModelInit(t *testing.T) {
	th := testTheme()
	rm := NewReplModel(th, "test-version")
	_ = rm.Init()
}

func TestReplModelSetTheme(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetTheme(testTheme())
}

func TestReplModelSetSidebarWidth(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetSidebarWidth(40)
	if rm.sidebarWidth != 40 {
		t.Errorf("sidebarWidth=%d, want 40", rm.sidebarWidth)
	}
}

func TestReplModelUpdateWindowSize(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	result, _ := rm.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
}

func TestReplModelView(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetSidebarWidth(0)
	result, _ := rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	_ = result
	r := rm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestReplModelAddMessage(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	msg := makeAssistantMsg("test message")
	rm.AddMessage(msg)
	if len(rm.messages) != 1 {
		t.Errorf("messages=%d, want 1", len(rm.messages))
	}
}

func TestReplModelClearMessages(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.AddMessage(makeAssistantMsg("msg"))
	rm.ClearMessages()
	if len(rm.messages) != 0 {
		t.Error("messages should be empty after clear")
	}
}

func TestReplModelSetSessionID(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetSessionID("abc-123")
	if rm.sessionID != "abc-123" {
		t.Error("sessionID wrong")
	}
}

func TestReplModelSetStreaming(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetStreaming(true)
	if !rm.streaming {
		t.Error("should be streaming after SetStreaming(true)")
	}
	rm.SetStreaming(false)
	if rm.streaming {
		t.Error("should not be streaming after SetStreaming(false)")
	}
}

func TestReplModelSetThinking(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetThinking(true)
	if !rm.thinking {
		t.Error("should be thinking after SetThinking(true)")
	}
	rm.SetThinking(false)
	if rm.thinking {
		t.Error("should not be thinking after SetThinking(false)")
	}
}

func TestReplModelInputValue(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	v := rm.InputValue()
	_ = v
}

func TestReplModelMessages(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	msgs := rm.Messages()
	if len(msgs) != 0 {
		t.Error("Messages should be empty initially")
	}
	rm.AddMessage(makeAssistantMsg("hello"))
	msgs = rm.Messages()
	if len(msgs) != 1 {
		t.Errorf("Messages count=%d, want 1", len(msgs))
	}
}

func TestReplModelGetStatusText(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	s := rm.GetStatusText()
	_ = s
}

func TestReplModelLastUsage(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	u := rm.LastUsage()
	if u != nil {
		t.Error("LastUsage should be nil initially")
	}
}

func TestReplModelLastCost(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	c := rm.LastCost()
	if c != 0 {
		t.Error("LastCost should be 0 initially")
	}
}

func TestReplModelSetDispatcher(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetDispatcher(nil)
}

func TestReplModelSetCommandRegistry(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetCommandRegistry(nil)
}

func TestReplModelSetFrecentHistory(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetFrecentHistory(nil)
}

func TestReplModelSetCwd(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetCwd("/tmp")
	if rm.cwd != "/tmp" {
		t.Error("cwd wrong")
	}
}

func TestReplModelSetChangedFiles(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetChangedFiles(5)
}

func TestReplModelSetKeyRegistry(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetKeyRegistry(nil)
}

func TestReplModelSetSessionSparkline(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetSessionSparkline("***")
}

func TestReplModelSetLastActivity(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetLastActivity(time.Now())
}

func TestReplModelSetMessages(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetMessages([]types.Message{
		makeAssistantMsg("msg1"),
		makeAssistantMsg("msg2"),
	})
	if len(rm.messages) != 2 {
		t.Errorf("messages=%d, want 2", len(rm.messages))
	}
}

func TestReplModelRefreshViewport(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.RefreshViewport()
}

func TestReplModelSpinnerTick(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	_ = rm.SpinnerTick()
}

func TestReplModelShowQuestion(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.ShowQuestion(QuestionRequestMsg{Question: "What?"})
}

func TestReplModelAppendStreamChunk(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.SetStreaming(true)
	rm.AppendStreamChunk(&types.StreamChunk{Delta: "hello"})
}

func TestReplModelUpdateEnter(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	rm.Update(tea.KeyMsg{Type: tea.KeyEnter})
}

func TestReplModelUpdateEsc(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	rm.Update(tea.KeyMsg{Type: tea.KeyEsc})
}

func TestReplModelUpdateDown(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	rm.Update(tea.KeyMsg{Type: tea.KeyDown})
}

func TestReplModelUpdateUp(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	rm.Update(tea.KeyMsg{Type: tea.KeyUp})
}

func TestReplModelUpdateTab(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	rm.Update(tea.KeyMsg{Type: tea.KeyTab})
}

// ═══ header.go renderProviderBadge ═══

func TestRenderProviderBadge(t *testing.T) {
	th := testTheme()
	r := renderProviderBadge(th, "openrouter")
	if r == "" {
		t.Error("renderProviderBadge should not be empty")
	}
	r = renderProviderBadge(th, "zen")
	if r == "" {
		t.Error("renderProviderBadge zen should not be empty")
	}
}

// ═══ header.go renderContextMeter ═══

func TestRenderContextMeter(t *testing.T) {
	th := testTheme()
	r := renderContextMeter(50, 100, th)
	if r == "" {
		t.Error("renderContextMeter should not be empty")
	}
	r = renderContextMeter(90, 100, th)
	if r == "" {
		t.Error("renderContextMeter high should not be empty")
	}
	r = renderContextMeter(70, 100, th)
	if r == "" {
		t.Error("renderContextMeter medium should not be empty")
	}
	r = renderContextMeter(0, 0, th)
	if r != "" {
		t.Error("renderContextMeter zero total should be empty")
	}
}

// ═══ diff_view.go (colorizeDiff) ═══

func TestColorizeDiff(t *testing.T) {
	th := testTheme()
	diff := "--- a/main.go\n+++ b/main.go\n@@ -1,3 +1,4 @@\n context\n+added\n-removed\n context2"
	r := colorizeDiff(diff, th)
	if r == "" {
		t.Error("colorizeDiff should not be empty")
	}
}

func TestColorizeDiffEmpty(t *testing.T) {
	th := testTheme()
	_ = colorizeDiff("", th)
}

func TestColorizeDiffSingleLine(t *testing.T) {
	th := testTheme()
	r := colorizeDiff("+added line", th)
	if r == "" {
		t.Error("single line should not be empty")
	}
}

func TestColorizeDiffRemoval(t *testing.T) {
	th := testTheme()
	r := colorizeDiff("-removed line", th)
	if r == "" {
		t.Error("removal line should not be empty")
	}
}

func TestColorizeDiffContext(t *testing.T) {
	th := testTheme()
	r := colorizeDiff(" context line", th)
	if r == "" {
		t.Error("context line should not be empty")
	}
}

func TestColorizeDiffHunk(t *testing.T) {
	th := testTheme()
	r := colorizeDiff("@@ -1,5 +1,6 @@", th)
	if r == "" {
		t.Error("hunk line should not be empty")
	}
}

func TestColorizeDiffBackslash(t *testing.T) {
	th := testTheme()
	r := colorizeDiff("\\ No newline at end of file", th)
	if r == "" {
		t.Error("backslash line should not be empty")
	}
}

// ═══ plan_refine.go ═══

func TestPlanRefineModel(t *testing.T) {
	th := testTheme()
	prm := NewPlanRefineModel(th, 80)
	if prm == nil {
		t.Fatal("NewPlanRefineModel returned nil")
	}
	result, _ := prm.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	if result == nil {
		t.Error("WindowSizeMsg should return non-nil")
	}
	_, _ = prm.Update(tea.KeyMsg{Type: tea.KeyEsc})
	v := prm.Value()
	_ = v
}

func TestPlanRefineModelView(t *testing.T) {
	prm := NewPlanRefineModel(testTheme(), 80)
	r := prm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}
