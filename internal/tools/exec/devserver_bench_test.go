package exec

import (
	"testing"
)

func BenchmarkDevServerBufferAlloc(b *testing.B) {
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		buf := devServerBufferPool.Get().(*ringBuffer)
		if buf != nil {
			buf.Reset()
			devServerBufferPool.Put(buf)
		}
	}
}

func BenchmarkDevServerBufferWrite(b *testing.B) {
	buf := NewRingBuffer(maxLogBytes)
	data := []byte("test log line for benchmarking ring buffer write performance\n")
	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_, _ = buf.Write(data)
	}
}

func BenchmarkDevServerBufferWritePooled(b *testing.B) {
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		buf := devServerBufferPool.Get().(*ringBuffer)
		if buf != nil {
			data := []byte("test log line for benchmarking pooled buffer write\n")
			_, _ = buf.Write(data)
			buf.Reset()
			devServerBufferPool.Put(buf)
		}
	}
}
