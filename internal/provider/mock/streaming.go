package mock

import (
	"context"
	"io"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// StreamingMockProvider is a test LLM provider that streams configurable chunks
// with configurable delays between them. It supports concurrent calls to
// ChatCompletionStream to simulate parallel tool execution.
type StreamingMockProvider struct {
	// ChunksPerResponse is the number of chunks to emit per ChatCompletionStream call.
	// Default: 10.
	ChunksPerResponse int

	// ChunkDelay is the delay between emitting each chunk.
	// Default: 1ms.
	ChunkDelay time.Duration

	// ResponseContent is the base content for each chunk. The chunk index
	// is appended to make chunks distinguishable.
	// Default: "chunk-"
	ResponseContent string

	// ConcurrencyLimit is the maximum number of concurrent ChatCompletionStream
	// calls allowed. 0 means unlimited. Default: 0 (unlimited).
	ConcurrencyLimit int

	mu            sync.Mutex
	activeStreams int
}

func (m *StreamingMockProvider) chunksPerResponse() int {
	if m.ChunksPerResponse <= 0 {
		return 10
	}
	return m.ChunksPerResponse
}

func (m *StreamingMockProvider) chunkDelay() time.Duration {
	if m.ChunkDelay <= 0 {
		return 1 * time.Millisecond
	}
	return m.ChunkDelay
}

func (m *StreamingMockProvider) responseContent() string {
	if m.ResponseContent == "" {
		return "chunk-"
	}
	return m.ResponseContent
}

// Name returns the provider name.
func (m *StreamingMockProvider) Name() string { return "streaming-mock" }

// APIKey returns the test API key.
func (m *StreamingMockProvider) APIKey() string { return "test-key" }

// FetchModels returns an empty model list (not used in stress tests).
func (m *StreamingMockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}

// ChatCompletionStream streams a configurable number of chunks with configurable delay.
func (m *StreamingMockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	// Check concurrency limit
	if m.ConcurrencyLimit > 0 {
		m.mu.Lock()
		if m.activeStreams >= m.ConcurrencyLimit {
			m.mu.Unlock()
			return nil, io.EOF
		}
		m.activeStreams++
		m.mu.Unlock()

		defer func() {
			m.mu.Lock()
			m.activeStreams--
			m.mu.Unlock()
		}()
	}

	chunks := m.chunksPerResponse()
	delay := m.chunkDelay()
	content := m.responseContent()

	done := false
	chunkIdx := 0

	next := func() (*types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		if chunkIdx >= chunks {
			done = true
			return nil, io.EOF
		}

		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(delay):
		}

		chunkIdx++
		return &types.StreamChunk{
			Delta: content + string(rune('A'+(chunkIdx%26))),
		}, nil
	}

	closeFn := func() error {
		done = true
		return nil
	}

	return &types.StreamIterator{Next: next, Close: closeFn}, nil
}

// EstimateCost returns zero cost for testing.
func (m *StreamingMockProvider) EstimateCost(modelID string, usage types.Usage) float64 {
	return 0
}

// HealthCheck returns healthy status.
func (m *StreamingMockProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "live"}
}

// GetModel returns nil (not used in stress tests).
func (m *StreamingMockProvider) GetModel(id string) (*types.ModelInfo, error) {
	return nil, nil
}

// CachedModels returns nil (not used in stress tests).
func (m *StreamingMockProvider) CachedModels() []types.ModelInfo {
	return nil
}
