package openrouter

import (
	"context"
	"testing"

	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/tests/testutil"
	"github.com/eshanized/M31A/internal/core/types"
)

func TestIntegration_HealthCheck(t *testing.T) {
	testutil.LoadTestDotEnv(t)
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_OPENROUTER_API_KEY", "OPENROUTER_API_KEY")

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
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_OPENROUTER_API_KEY", "OPENROUTER_API_KEY")

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
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_OPENROUTER_API_KEY", "OPENROUTER_API_KEY")

	c, err := New(apiKey, Options{})
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:     "openai/gpt-4o-mini",
		Messages:  []types.Message{{Role: "user", Content: "Say hello in exactly 3 words."}},
		MaxTokens: 50,
	})
	if err != nil {
		t.Fatalf("ChatCompletionStream failed: %v", err)
	}
	defer it.Close()

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
	if full == "" {
		t.Error("expected non-empty response")
	}
	t.Logf("Response: %s", full)
}

func TestIntegration_EstimateCost(t *testing.T) {
	testutil.LoadTestDotEnv(t)
	apiKey := testutil.RequireAnyAPIKey(t, "M31A_OPENROUTER_API_KEY", "OPENROUTER_API_KEY")

	c, err := New(apiKey, Options{})
	if err != nil {
		t.Fatalf("New failed: %v", err)
	}

	// Fetch models to populate cache
	_, err = c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("FetchModels failed: %v", err)
	}

	// Try to estimate cost for a known model
	cost := c.EstimateCost("openai/gpt-4o-mini", types.Usage{PromptTokens: 1000, CompletionTokens: 500})
	t.Logf("Estimated cost for gpt-4o-mini: $%f", cost)
}
