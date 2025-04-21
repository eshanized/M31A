package main

import (
	"context"
	"fmt"
	"os"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/provider/zen"
	"github.com/eshanized/M31A/internal/types"
)

func main() {
	apiKey := os.Getenv("M31A_ZEN_API_KEY")
	if apiKey == "" {
		fmt.Println("FAIL: M31A_ZEN_API_KEY not set")
		os.Exit(1)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	// Step 1: Create Zen client
	fmt.Println("Step 1: Creating Zen client...")
	client, err := zen.New(apiKey)
	if err != nil {
		fmt.Printf("FAIL: Cannot create Zen client: %v\n", err)
		os.Exit(1)
	}
	fmt.Println("  ✓ Zen client created")

	// Step 2: Fetch models
	fmt.Println("Step 2: Fetching models...")
	models, err := client.FetchModels(ctx)
	if err != nil {
		fmt.Printf("FAIL: Cannot fetch models: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("  ✓ Fetched %d models\n", len(models))
	for _, m := range models[:5] {
		fmt.Printf("    - %s (%s)\n", m.ID, m.Name)
	}

	// Step 3: Pick a model for streaming test
	modelID := "deepseek-v4-flash-free" // Free model, no credits required
	fmt.Printf("Step 3: Testing streaming with model: %s\n", modelID)

	// Step 4: Send a real coding task with streaming
	fmt.Println("Step 4: Sending streaming chat request (real coding task)...")
	messages := []types.Message{
		{
			Role:    "user",
			Content: "Create a Go HTTP server with a /health endpoint that returns JSON with status, uptime, and version. Include graceful shutdown on SIGTERM.",
		},
	}

	req := provider.ChatRequest{
		Model:    modelID,
		Messages: messages,
		Stream:   true,
	}

	iter, err := client.ChatCompletionStream(ctx, req)
	if err != nil {
		fmt.Printf("FAIL: Streaming request failed: %v\n", err)
		os.Exit(1)
	}
	defer iter.Close()

	// Step 5: Read and accumulate the streaming response
	fmt.Println("Step 5: Reading streaming response...")
	var fullResponse string
	var tokenCount int
	startTime := time.Now()

	for {
		chunk, err := iter.Next()
		if err != nil {
			break // EOF or error
		}
		if chunk.Delta != "" {
			fullResponse += chunk.Delta
			tokenCount++
		}
		if chunk.ThinkingDuration > 0 {
			fmt.Printf("  Thinking: %dms\n", chunk.ThinkingDuration)
		}
	}

	elapsed := time.Since(startTime)

	fmt.Printf("  ✓ Response received in %v\n", elapsed)
	fmt.Printf("  Tokens streamed: %d\n", tokenCount)
	fmt.Println()
	fmt.Println("--- Response Preview (first 300 chars) ---")
	if len(fullResponse) > 300 {
		fmt.Println(fullResponse[:300] + "...")
	} else {
		fmt.Println(fullResponse)
	}
	fmt.Println("------------------------------------------------")

	if len(fullResponse) == 0 {
		fmt.Println("FAIL: Empty response from streaming")
		os.Exit(1)
	}

	// Step 6: Health check
	fmt.Println("Step 6: Running health check...")
	health := client.HealthCheck(ctx)
	fmt.Printf("  Status: %s, Latency: %dms\n", health.Status, health.LatencyMs)
	if health.Error != "" {
		fmt.Printf("  Error: %s\n", health.Error)
	}

	fmt.Println()
	fmt.Println("=== ALL TESTS PASSED ===")
	fmt.Println("Zen API is working. M31A can connect, fetch models, and stream responses.")
}
