package components

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestTourInitial(t *testing.T) {
	tm := NewTourModel(theme.Default(), 80, 24)
	if tm.Current() != 0 {
		t.Errorf("expected Current() == 0, got %d", tm.Current())
	}
	if tm.Total() != 7 {
		t.Errorf("expected Total() == 7, got %d", tm.Total())
	}
	if tm.IsCompleted() {
		t.Error("expected IsCompleted() == false at start")
	}
}

func TestTourNavigation(t *testing.T) {
	tm := NewTourModel(theme.Default(), 80, 24)
	for i := 0; i < 6; i++ {
		if tm.Next() {
			t.Errorf("expected Next() to return false at step %d", i)
		}
		if tm.Current() != i+1 {
			t.Errorf("expected Current() == %d after Next(), got %d", i+1, tm.Current())
		}
	}
	if !tm.Next() {
		t.Error("expected Next() to return true on final step")
	}
	if !tm.IsCompleted() {
		t.Error("expected IsCompleted() == true after final Next()")
	}
}

func TestTourSkip(t *testing.T) {
	tm := NewTourModel(theme.Default(), 80, 24)
	tm.Skip()
	if !tm.IsCompleted() {
		t.Error("expected IsCompleted() == true after Skip()")
	}
	if tm.Current() != 0 {
		t.Errorf("expected Current() == 0 after Skip(), got %d", tm.Current())
	}
}

func TestTourSkipFromMiddle(t *testing.T) {
	tm := NewTourModel(theme.Default(), 80, 24)
	tm.Next()
	tm.Next()
	if tm.Current() != 2 {
		t.Fatalf("expected Current() == 2, got %d", tm.Current())
	}
	tm.Skip()
	if !tm.IsCompleted() {
		t.Error("expected IsCompleted() == true after Skip() from middle")
	}
}

func TestTourDimensions(t *testing.T) {
	tm := NewTourModel(theme.Default(), 80, 24)
	tm.SetDimensions(120, 40)
	result := tm.Render()
	if result == "" {
		t.Error("expected non-empty Render() output")
	}
}

func TestTourRender(t *testing.T) {
	tm := NewTourModel(theme.Default(), 80, 24)
	result := tm.Render()
	if result == "" {
		t.Error("expected non-empty Render() output")
	}
}
