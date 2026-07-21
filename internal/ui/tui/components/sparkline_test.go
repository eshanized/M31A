package components

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestRenderSparkline_EmptyData(t *testing.T) {
	result := RenderSparkline([]float64{}, 10, theme.Dark())
	if result != "" {
		t.Errorf("empty data should return empty string, got %q", result)
	}
}

func TestRenderSparkline_ZeroWidth(t *testing.T) {
	result := RenderSparkline([]float64{0.1, 0.5}, 0, theme.Dark())
	if result != "" {
		t.Errorf("zero width should return empty string, got %q", result)
	}
}

func TestRenderSparkline_BasicData(t *testing.T) {
	data := []float64{0.1, 0.5, 0.8, 1.0, 0.3}
	result := RenderSparkline(data, 10, theme.Dark())
	if result == "" {
		t.Error("sparkline with data should not be empty")
	}
}

func TestRenderSparkline_SingleValue(t *testing.T) {
	data := []float64{0.5}
	result := RenderSparkline(data, 5, theme.Dark())
	if result == "" {
		t.Error("single value sparkline should not be empty")
	}
}

func TestRenderSparkline_AllSameValues(t *testing.T) {
	data := []float64{0.5, 0.5, 0.5, 0.5}
	result := RenderSparkline(data, 8, theme.Dark())
	if result == "" {
		t.Error("uniform values sparkline should not be empty")
	}
}

func TestRenderSparkline_LargeDataCapped(t *testing.T) {
	// Create 150 data points — should be capped to 100
	data := make([]float64, 150)
	for i := range data {
		data[i] = float64(i) / 150.0
	}
	result := RenderSparkline(data, 20, theme.Dark())
	if result == "" {
		t.Error("large data sparkline should not be empty")
	}
}
