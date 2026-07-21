package provider

import (
	"context"
	"io"
	"net/http"
	"strings"
	"testing"
)

func bodyReader(s string) *http.Response {
	return &http.Response{
		Body: io.NopCloser(strings.NewReader(s)),
	}
}

func TestSSEParser_SingleDataLine(t *testing.T) {
	resp := bodyReader("data: {\"key\":\"val\"}\n\n")
	p := NewSSEParser(resp)
	defer p.Close()

	eventType, data, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if eventType != "" {
		t.Fatalf("expected empty event type, got %q", eventType)
	}
	if data != `{"key":"val"}` {
		t.Fatalf("expected data %q, got %q", `{"key":"val"}`, data)
	}

	_, _, err = p.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF, got %v", err)
	}
}

func TestSSEParser_MultiLineData(t *testing.T) {
	resp := bodyReader("data: line1\ndata: line2\n\n")
	p := NewSSEParser(resp)
	defer p.Close()

	_, data, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if data != "line1\nline2" {
		t.Fatalf("expected %q, got %q", "line1\nline2", data)
	}
}

func TestSSEParser_EventType(t *testing.T) {
	resp := bodyReader("event: message\ndata: hello\n\n")
	p := NewSSEParser(resp)
	defer p.Close()

	eventType, data, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if eventType != "message" {
		t.Fatalf("expected event type %q, got %q", "message", eventType)
	}
	if data != "hello" {
		t.Fatalf("expected data %q, got %q", "hello", data)
	}
}

func TestSSEParser_DoneSentinel(t *testing.T) {
	resp := bodyReader("data: [DONE]\n\n")
	p := NewSSEParser(resp)
	defer p.Close()

	_, _, err := p.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF, got %v", err)
	}
}

func TestSSEParser_EmptyStream(t *testing.T) {
	resp := bodyReader("")
	p := NewSSEParser(resp)
	defer p.Close()

	_, _, err := p.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF, got %v", err)
	}
}

func TestSSEParser_DoneWithWhitespace(t *testing.T) {
	resp := bodyReader("data: [DONE] \n\n")
	p := NewSSEParser(resp)
	defer p.Close()

	_, _, err := p.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF for [DONE] with trailing whitespace, got %v", err)
	}
}

func TestSSEParser_DoneWithoutWhitespace(t *testing.T) {
	resp := bodyReader("data: [DONE]\n\n")
	p := NewSSEParser(resp)
	defer p.Close()

	_, _, err := p.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF for [DONE] without whitespace, got %v", err)
	}
}

func TestSSE_ContextCancel(t *testing.T) {
	// Create a parser with a cancellable context and a body that blocks
	ctx, cancel := context.WithCancel(context.Background())

	// Use a pipe so we can control when data is available
	pr, pw := io.Pipe()
	resp := &http.Response{
		Body: pr,
	}

	// Write the data line (no empty line yet) then cancel context
	go func() {
		// Write data line without the terminating empty line — scanner blocks
		pw.Write([]byte("data: {\"test\": true}\n"))
		// Give scanner time to start reading
		<-make(chan struct{})
		cancel()
		pw.Close()
	}()

	p := NewSSEParserWithContext(resp, ctx)
	defer p.Close()

	// Cancel the context before calling Next
	cancel()

	// Next should return context error since context is already cancelled
	_, _, err := p.Next()
	if err == nil {
		t.Fatal("expected error after context cancellation")
	}
	if err != context.Canceled {
		t.Fatalf("expected context.Canceled, got %v", err)
	}
}
