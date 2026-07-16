package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/internal/types"
)

func TestVerifySetHealFunc(t *testing.T) {
	vm := &VerifyModel{healingTaskID: -1}
	vm.SetHealFunc(func(taskID int) tea.Cmd {
		return nil
	})
	if vm.healFunc == nil {
		t.Error("healFunc should be set")
	}
}

func TestVerifyCountFailedTasks(t *testing.T) {
	vm := &VerifyModel{
		tasks: []types.Task{
			{ID: 1, Description: "task1"},
			{ID: 2, Description: "task2"},
			{ID: 3, Description: "task3"},
		},
		results: map[int]workflow.VerificationResult{
			1: {FilesExist: true, SyntaxOK: true, TestsOK: true},
			2: {FilesExist: false, SyntaxOK: true, TestsOK: true},
			3: {FilesExist: true, SyntaxOK: false, TestsOK: true},
		},
	}
	if got := vm.countFailedTasks(); got != 2 {
		t.Errorf("want 2 failed, got %d", got)
	}
}

func TestVerifyCountFailedTasksNoResults(t *testing.T) {
	vm := &VerifyModel{
		tasks:   []types.Task{{ID: 1}},
		results: map[int]workflow.VerificationResult{},
	}
	if got := vm.countFailedTasks(); got != 0 {
		t.Errorf("want 0 failed, got %d", got)
	}
}

func TestVerifyGetFailedTasks(t *testing.T) {
	vm := &VerifyModel{
		tasks: []types.Task{
			{ID: 1, Description: "ok"},
			{ID: 2, Description: "fail"},
		},
		results: map[int]workflow.VerificationResult{
			1: {FilesExist: true, SyntaxOK: true, TestsOK: true},
			2: {FilesExist: false, SyntaxOK: true, TestsOK: false},
		},
	}
	failed := vm.getFailedTasks()
	if len(failed) != 1 || failed[0].ID != 2 {
		t.Errorf("want [2], got %v", failed)
	}
}

func TestVerifyGetFailedTasksNone(t *testing.T) {
	vm := &VerifyModel{
		tasks:   []types.Task{{ID: 1}},
		results: map[int]workflow.VerificationResult{},
	}
	failed := vm.getFailedTasks()
	if len(failed) != 0 {
		t.Error("want empty")
	}
}
