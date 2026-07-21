package nvidia

import (
	"context"
	"errors"
	"strings"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/tests/testutil"
	"github.com/eshanized/M31A/internal/core/types"
)

func TestIntegration_HealthCheck(t *testing.T) {
	testutil.LoadTestDotEnv(t)
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_NVIDIA_API_KEY", "NVIDIA_API_KEY")

	c, err := New(apiKey, Options{})
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}

	status := c.HealthCheck(context.Background())
	if status.Status == "offline" {
		t.Errorf("health check returned offline: %s", status.Error)
	}
	t.Logf("Health status: %s (latency: %dms)", status.Status, status.LatencyMs)
}

func TestIntegration_FetchModels(t *testing.T) {
	testutil.LoadTestDotEnv(t)
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_NVIDIA_API_KEY", "NVIDIA_API_KEY")

	c, err := New(apiKey, Options{})
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}

	models, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("FetchModels failed: %v", err)
	}
	if len(models) == 0 {
		t.Fatal("expected at least 1 model")
	}
	t.Logf("Fetched %d models", len(models))
}

func TestIntegration_ChatCompletion(t *testing.T) {
	testutil.LoadTestDotEnv(t)
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_NVIDIA_API_KEY", "NVIDIA_API_KEY")

	c, err := New(apiKey, Options{})
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}

	// Try multiple models in case one isn't available
	models := []string{
		"meta/llama-3.1-8b-instruct",
		"meta/llama-3.1-70b-instruct",
		"google/gemma-2-9b-it",
	}

	var lastErr error
	for _, model := range models {
		it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
			Model:     model,
			Messages:  []types.Message{{Role: "user", Content: "Say hello in exactly 3 words."}},
			MaxTokens: 50,
		})
		if err != nil {
			lastErr = err
			t.Logf("model %s failed: %v", model, err)
			continue
		}

		var full string
		for {
			chunk, err := it.Next()
			if err != nil {
				break
			}
			if chunk.Type == "content" {
				full += chunk.Delta
			}
		}
		it.Close()

		if full != "" {
			t.Logf("model=%s response=%s", model, full)
			return
		}
	}

	// All models failed
	if lastErr != nil {
		if errors.Is(lastErr, m31errors.ErrInvalidKey) || strings.Contains(lastErr.Error(), "invalid") {
			t.Skipf("skipping: API key lacks chat completion permissions: %v", lastErr)
		}
		t.Fatalf("all models failed, last error: %v", lastErr)
	}
	t.Error("all models returned empty responses")
}
