package provider

import (
	"context"
	"io"
	"net/http"
	"strings"
	"testing"
)

func BenchmarkSSEParserNew(b *testing.B) {
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		pr, pw := io.Pipe()
		resp := &http.Response{Body: pr}
		ctx := context.Background()
		parser := NewSSEParserWithContext(resp, ctx)
		parser.Close()
		pw.Close()
	}
}

func BenchmarkSSEParserNext(b *testing.B) {
	pr, pw := io.Pipe()
	resp := &http.Response{Body: pr}
	ctx := context.Background()
	parser := NewSSEParserWithContext(resp, ctx)
	defer parser.Close()

	events := strings.Repeat("data: {\"test\": true}\n\n", 100)
	go func() {
		pw.Write([]byte(events))
		pw.Close()
	}()

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_, _, err := parser.Next()
		if err != nil {
			break
		}
	}
}

func BenchmarkSSEParserClose(b *testing.B) {
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		pr, pw := io.Pipe()
		resp := &http.Response{Body: pr}
		ctx := context.Background()
		parser := NewSSEParserWithContext(resp, ctx)
		parser.Close()
		pw.Close()
	}
}
