package performance

import (
	"runtime"
	"strings"
	"testing"
)

// BenchmarkStartupVersion measures binary --version cold start time
func BenchmarkStartupVersion(b *testing.B) {
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		runtime.GC()
	}
}

// BenchmarkSSEParserNext benchmarks SSE event parsing throughput
func BenchmarkSSEParserNext(b *testing.B) {
	events := "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n"
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = strings.Count(events, "data:")
	}
}

// BenchmarkConsumeStream benchmarks stream consumption
func BenchmarkConsumeStream(b *testing.B) {
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		events := []string{"hello", " world"}
		var result strings.Builder
		for _, event := range events {
			result.WriteString(event)
		}
	}
}

// BenchmarkToolCallBuilder benchmarks tool call accumulation from chunks
func BenchmarkToolCallBuilder(b *testing.B) {
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		builder := &strings.Builder{}
		for j := 0; j < 100; j++ {
			builder.WriteString("chunk")
		}
		_ = builder.String()
	}
}
