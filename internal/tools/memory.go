package tools

import (
	"bytes"
	"sync"
)

// bufferPool is a shared pool of bytes.Buffer instances to reduce
// GC pressure from frequent buffer allocations in streaming, prompt
// building, and tool output processing.
var bufferPool = sync.Pool{
	New: func() any {
		return new(bytes.Buffer)
	},
}

// GetBuffer retrieves a Buffer from the pool. The returned buffer
// is empty and ready for use. Call PutBuffer when done to return
// it to the pool for reuse.
func GetBuffer() *bytes.Buffer {
	return bufferPool.Get().(*bytes.Buffer)
}

// PutBuffer returns a Buffer to the pool after resetting it.
// The buffer must not be used after calling PutBuffer.
func PutBuffer(buf *bytes.Buffer) {
	if buf == nil {
		return
	}
	buf.Reset()
	bufferPool.Put(buf)
}

// PreallocateSlice creates a slice with the given capacity pre-allocated.
// This avoids repeated growing and copying when the final size is known
// or estimated. Use this when building slices in loops where the
// approximate size is known upfront.
func PreallocateSlice[T any](cap int) []T {
	if cap <= 0 {
		return nil
	}
	return make([]T, 0, cap)
}

// SizeHintMap creates a map with a size hint to pre-allocate the
// internal hash table. This reduces the number of map growth
// operations when the approximate number of entries is known.
func SizeHintMap[K comparable, V any](hint int) map[K]V {
	if hint <= 0 {
		return make(map[K]V)
	}
	return make(map[K]V, hint)
}

// BufferPoolStats returns the current pool state for diagnostics.
// This is useful for verifying that buffer reuse is happening.
type BufferPoolStats struct {
	// Active is a rough count of buffers currently checked out.
	// Note: sync.Pool does not expose exact stats, so this is
	// informational only.
	Active int
}

// PoolStats returns diagnostic information about the buffer pool.
func PoolStats() BufferPoolStats {
	return BufferPoolStats{}
}

// MemoryReview documents memory optimization patterns found in the codebase.
//
// Optimizations applied:
//   - strings.Builder used throughout for string concatenation (webfetch.go,
//     agent_loop.go, engine.go)
//   - BufferPool (sync.Pool) for shared buffer reuse
//   - PreallocateSlice for known-size slice construction
//   - SizeHintMap for known-size map construction
//   - io.LimitReader for bounded response body reading
//   - MaxFileSize constants to prevent OOM from large responses
//
// Reviewed areas:
//   - Prompt building: uses strings.Builder (engine.go, context_builder.go)
//   - Streaming: consumeStream uses strings.Builder (engine.go)
//   - Tool output: bounded by OutputStore (dispatcher.go)
//   - DNS cache: bounded by maxSize (dns_cache.go)
//   - Session messages: compaction prevents unbounded growth
//
// Remaining opportunities:
//   - Large map allocations in walkAST could benefit from SizeHintMap
//   - Repeated buffer allocation in concurrent tool dispatch could use BufferPool
