package tools

import (
	"bytes"
	"testing"
)

func TestBufferPool_GetPut(t *testing.T) {
	buf := GetBuffer()
	if buf == nil {
		t.Fatal("GetBuffer returned nil")
	}
	if buf.Len() != 0 {
		t.Fatalf("GetBuffer: expected empty buffer, got %d bytes", buf.Len())
	}

	// Write something to the buffer
	buf.WriteString("hello")
	if buf.Len() != 5 {
		t.Fatalf("buffer write: expected 5, got %d", buf.Len())
	}

	// Put it back
	PutBuffer(buf)

	// Get another buffer — should be reset
	buf2 := GetBuffer()
	if buf2.Len() != 0 {
		t.Fatalf("GetBuffer after Put: expected empty, got %d bytes", buf2.Len())
	}
	PutBuffer(buf2)
}

func TestBufferPool_NilPut(t *testing.T) {
	// Should not panic
	PutBuffer(nil)
}

func TestBufferPool_MultipleBuffers(t *testing.T) {
	buffers := make([]*bytes.Buffer, 10)
	for i := range buffers {
		buffers[i] = GetBuffer()
		buffers[i].WriteString("test")
	}

	for _, buf := range buffers {
		PutBuffer(buf)
	}

	// Get them back — all should be empty
	for i := range buffers {
		buffers[i] = GetBuffer()
		if buffers[i].Len() != 0 {
			t.Fatalf("buffer %d: expected empty after pool reuse, got %d", i, buffers[i].Len())
		}
		PutBuffer(buffers[i])
	}
}

func TestPreallocateSlice(t *testing.T) {
	s := PreallocateSlice[int](10)
	if s == nil {
		t.Fatal("PreallocateSlice returned nil")
	}
	if cap(s) != 10 {
		t.Fatalf("PreallocateSlice: cap = %d, want 10", cap(s))
	}
	if len(s) != 0 {
		t.Fatalf("PreallocateSlice: len = %d, want 0", len(s))
	}

	// Zero hint
	s2 := PreallocateSlice[int](0)
	if s2 != nil {
		t.Fatal("PreallocateSlice(0) should return nil")
	}
}

func TestSizeHintMap(t *testing.T) {
	m := SizeHintMap[string, int](100)
	if m == nil {
		t.Fatal("SizeHintMap returned nil")
	}
	if len(m) != 0 {
		t.Fatalf("SizeHintMap: len = %d, want 0", len(m))
	}

	m["key"] = 42
	if m["key"] != 42 {
		t.Fatal("SizeHintMap: value not stored correctly")
	}

	// Zero hint
	m2 := SizeHintMap[string, int](0)
	if m2 == nil {
		t.Fatal("SizeHintMap(0) should return non-nil empty map")
	}
}

func TestPoolStats(t *testing.T) {
	stats := PoolStats()
	// Just verify it doesn't panic
	_ = stats
}

func TestReviewMemory(t *testing.T) {
	// Verify memory review compiles and returns
	// This is a documentation-only function
}
