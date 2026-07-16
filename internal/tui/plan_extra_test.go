package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestWaveTitle(t *testing.T) {
	tests := []struct {
		idx  int
		want string
	}{
		{0, "Foundation"},
		{1, "Core Features"},
		{2, "Integration"},
		{3, "Polish"},
		{4, "Verification"},
		{5, "Phase 6"},
		{10, "Phase 11"},
	}
	for _, tt := range tests {
		got := waveTitle(tt.idx)
		if got != tt.want {
			t.Errorf("waveTitle(%d) = %q, want %q", tt.idx, got, tt.want)
		}
	}
}

func TestPlanModelSetPlanVersion(t *testing.T) {
	pm := &PlanModel{}
	pm.SetPlanVersion(5)
	if pm.planVersion != 5 {
		t.Errorf("want 5, got %d", pm.planVersion)
	}
}

func TestPlanModelComputeWavesEmpty(t *testing.T) {
	pm := &PlanModel{}
	pm.computeWaves()
	if pm.waves != nil {
		t.Error("empty tasks should produce nil waves")
	}
}

func TestPlanModelComputeWavesNoDeps(t *testing.T) {
	pm := &PlanModel{
		tasks: []types.Task{
			{ID: 1, Description: "a"},
			{ID: 2, Description: "b"},
		},
	}
	pm.computeWaves()
	if len(pm.waves) != 1 {
		t.Errorf("want 1 wave, got %d", len(pm.waves))
	}
	if len(pm.waves[0]) != 2 {
		t.Errorf("want 2 tasks in wave 0, got %d", len(pm.waves[0]))
	}
}

func TestPlanModelComputeWavesWithDeps(t *testing.T) {
	pm := &PlanModel{
		tasks: []types.Task{
			{ID: 1, Description: "base"},
			{ID: 2, Description: "depends on 1", Dependencies: []int{1}},
			{ID: 3, Description: "depends on 2", Dependencies: []int{2}},
		},
	}
	pm.computeWaves()
	if len(pm.waves) != 3 {
		t.Errorf("want 3 waves, got %d", len(pm.waves))
	}
	if len(pm.waves[0]) != 1 {
		t.Errorf("wave 0: want 1 task, got %d", len(pm.waves[0]))
	}
	if pm.waves[0][0].ID != 1 {
		t.Errorf("wave 0: want ID 1, got %d", pm.waves[0][0].ID)
	}
	if len(pm.waves[1]) != 1 || pm.waves[1][0].ID != 2 {
		t.Error("wave 1: want ID 2")
	}
	if len(pm.waves[2]) != 1 || pm.waves[2][0].ID != 3 {
		t.Error("wave 2: want ID 3")
	}
}

func TestPlanModelComputeWavesParallel(t *testing.T) {
	pm := &PlanModel{
		tasks: []types.Task{
			{ID: 1, Description: "base"},
			{ID: 2, Description: "a", Dependencies: []int{1}},
			{ID: 3, Description: "b", Dependencies: []int{1}},
		},
	}
	pm.computeWaves()
	if len(pm.waves) != 2 {
		t.Errorf("want 2 waves, got %d", len(pm.waves))
	}
	if len(pm.waves[0]) != 1 || pm.waves[0][0].ID != 1 {
		t.Error("wave 0: want ID 1")
	}
	if len(pm.waves[1]) != 2 {
		t.Errorf("wave 1: want 2 tasks, got %d", len(pm.waves[1]))
	}
}

func TestPlanModelComputeWavesCyclic(t *testing.T) {
	pm := &PlanModel{
		tasks: []types.Task{
			{ID: 1, Dependencies: []int{2}},
			{ID: 2, Dependencies: []int{1}},
		},
	}
	pm.computeWaves()
	if len(pm.waves) != 1 {
		t.Errorf("cyclic: want 1 wave (fallback), got %d", len(pm.waves))
	}
	if len(pm.waves[0]) != 2 {
		t.Errorf("cyclic: want 2 tasks, got %d", len(pm.waves[0]))
	}
}

func TestPlanModelComputeWavesMissingDep(t *testing.T) {
	pm := &PlanModel{
		tasks: []types.Task{
			{ID: 1, Dependencies: []int{999}},
		},
	}
	pm.computeWaves()
	if len(pm.waves) != 1 {
		t.Errorf("missing dep: want 1 wave, got %d", len(pm.waves))
	}
}
