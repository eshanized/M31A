package tokens

import (
	"strings"
	"testing"
)

func TestContextWarning(t *testing.T) {
	t.Run("ContextWarningBanner shows remaining tokens", func(t *testing.T) {
		e := &Estimator{
			modelID: "test-model",
		}
		used := 80000
		total := int64(100000)
		threshold := 0.8
		banner := e.ContextWarningBanner(used, total, threshold)
		if banner == "" {
			t.Error("expected non-empty banner")
		}
		// Check that banner contains remaining tokens
		if !strings.Contains(banner, "20000 tokens remaining") {
			t.Errorf("expected banner to contain '20000 tokens remaining', got '%s'", banner)
		}
	})

	t.Run("ContextWarningBanner shows percentage", func(t *testing.T) {
		e := &Estimator{
			modelID: "test-model",
		}
		used := 80000
		total := int64(100000)
		threshold := 0.8
		banner := e.ContextWarningBanner(used, total, threshold)
		if banner == "" {
			t.Error("expected non-empty banner")
		}
		// Check that banner contains percentage
		if !strings.Contains(banner, "80%") {
			t.Errorf("expected banner to contain '80%%', got '%s'", banner)
		}
	})

	t.Run("ContextWarningBanner returns empty when under threshold", func(t *testing.T) {
		e := &Estimator{
			modelID: "test-model",
		}
		used := 50000
		total := int64(100000)
		threshold := 0.8
		banner := e.ContextWarningBanner(used, total, threshold)
		if banner != "" {
			t.Errorf("expected empty banner, got '%s'", banner)
		}
	})

	t.Run("ContextWarningBanner returns empty when total is zero", func(t *testing.T) {
		e := &Estimator{
			modelID: "test-model",
		}
		used := 50000
		total := int64(0)
		threshold := 0.8
		banner := e.ContextWarningBanner(used, total, threshold)
		if banner != "" {
			t.Errorf("expected empty banner, got '%s'", banner)
		}
	})

	t.Run("ContextWarningBanner includes compress suggestion", func(t *testing.T) {
		e := &Estimator{
			modelID: "test-model",
		}
		used := 90000
		total := int64(100000)
		threshold := 0.8
		banner := e.ContextWarningBanner(used, total, threshold)
		if banner == "" {
			t.Error("expected non-empty banner")
		}
		// Check that banner contains compress suggestion
		if !strings.Contains(banner, "/compress") {
			t.Errorf("expected banner to contain '/compress', got '%s'", banner)
		}
	})
}
