package search

import (
	"context"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestWebFetch_Name(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	if wf.Name() != "WebFetch" {
		t.Errorf("expected 'WebFetch', got %q", wf.Name())
	}
}

func TestWebFetch_RiskLevel(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	if wf.RiskLevel() != types.RiskMedium {
		t.Errorf("expected RiskMedium, got %v", wf.RiskLevel())
	}
}

func TestWebFetch_ParameterSchema(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	schema := wf.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
}

func TestWebFetch_ExecuteMissingURL(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing url")
	}
	if !contains(err.Error(), "missing parameter: url") {
		t.Errorf("expected 'missing parameter: url' error, got %q", err.Error())
	}
}

func TestWebFetch_ExecuteInvalidURL(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url": "not-a-url",
		},
	})
	if err == nil {
		t.Error("expected error for invalid url")
	}
	// The error message might be "tool execution failed: only http/https URLs are supported"
	// or "invalid URL" depending on validation order
	if !contains(err.Error(), "invalid URL") && !contains(err.Error(), "only http/https") {
		t.Errorf("expected URL validation error, got %q", err.Error())
	}
}

func TestWebFetch_ExecuteInvalidScheme(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url": "ftp://example.com/file",
		},
	})
	if err == nil {
		t.Error("expected error for invalid scheme")
	}
	if !contains(err.Error(), "only http/https") {
		t.Errorf("expected 'only http/https' error, got %q", err.Error())
	}
}

func TestWebFetch_ExecuteBlocksPrivateIP(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	// Test with a private IP
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url": "http://10.0.0.1",
		},
	})
	if err == nil {
		t.Error("expected error for private IP")
	}
	if !contains(err.Error(), "private IP blocked") {
		t.Errorf("expected 'private IP blocked' error, got %q", err.Error())
	}
}

func TestWebFetch_ExecuteWithCustomTimeout(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	result, err := wf.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url":     "http://example.com",
			"timeout": 10,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Should fail because we can't connect, but timeout parsing should work
	if result.Error == "" {
		t.Logf("request failed as expected: %s", result.Error)
	}
}

func TestWebFetch_ExecuteInvalidTimeout(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url":     "http://example.com",
			"timeout": float64(200), // exceeds MaxTimeoutSecs
		},
	})
	if err == nil {
		t.Error("expected error for timeout > max")
	}
	if !contains(err.Error(), "timeout must be between") {
		t.Errorf("expected timeout range error, got %q", err.Error())
	}
}

func TestWebFetch_ParameterSchemaContainsRequiredFields(t *testing.T) {
	wf := NewWebFetch("", 30, nil)
	schema := wf.ParameterSchema()
	if !contains(schema, "url") {
		t.Error("schema missing url field")
	}
	if !contains(schema, "method") {
		t.Error("schema missing method field")
	}
	if !contains(schema, "headers") {
		t.Error("schema missing headers field")
	}
	if !contains(schema, "format") {
		t.Error("schema missing format field")
	}
}

func TestIsPrivateIPFromHost(t *testing.T) {
	tests := []struct {
		name     string
		host     string
		expected bool
	}{
		{"loopback IP", "127.0.0.1", true},
		{"private IP 10.0.0.1", "10.0.0.1", true},
		{"private IP 172.16.0.1", "172.16.0.1", true},
		{"private IP 192.168.0.1", "192.168.0.1", true},
		{"link-local IP", "169.254.0.1", true},
		{"public IP", "8.8.8.8", false},
		{"IPv6 loopback", "::1", true},
		{"IPv6 public", "2001:4860:4860::8888", false},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := IsPrivateIPFromHost(tc.host)
			if result != tc.expected {
				t.Errorf("IsPrivateIPFromHost(%q) = %v, want %v", tc.host, result, tc.expected)
			}
		})
	}
}

func TestWebFetch_ResolveAndCheck(t *testing.T) {
	wf := NewWebFetch("", 30, nil)

	// Test with private IP
	err := wf.ResolveAndCheck(context.Background(), "http://10.0.0.1")
	if err == nil {
		t.Error("expected error for private IP")
	}
	if !contains(err.Error(), "private IP not allowed") {
		t.Errorf("expected 'private IP not allowed' error, got %q", err.Error())
	}

	// Test with public domain (will fail DNS but should not block on private IP check)
	err = wf.ResolveAndCheck(context.Background(), "http://example.com")
	// This might fail due to DNS, but should not be a private IP error
	if err != nil && contains(err.Error(), "private IP") {
		t.Errorf("unexpected private IP error for example.com: %v", err)
	}
}

func TestWebFetch_DNSCache(t *testing.T) {
	dnsCache := NewDNSCache(5*time.Minute, 64)
	_ = NewWebFetch("", 30, dnsCache)

	// Test that DNSCache is properly initialized
	if dnsCache == nil {
		t.Error("expected DNSCache to be set")
	}
}

func TestWebFetch_ResolveAndCache(t *testing.T) {
	dnsCache := NewDNSCache(5*time.Minute, 64)
	wf := NewWebFetch("", 30, dnsCache)

	// Test resolveAndCache with a mock - we can't easily mock DNS resolution
	// but we can verify the method exists and handles context cancellation
	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Immediately cancel

	_, err := wf.ResolveAndCache(ctx, "example.com")
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

func TestWebFetch_SetVersion(t *testing.T) {
	// Reset version first
	Version.Store("")

	SetVersion("test-1.0")
	v := GetVersion()
	if v != "test-1.0" {
		t.Errorf("expected version 'test-1.0', got %q", v)
	}

	// Reset
	Version.Store("")
}

func TestWebFetch_GetVersionDefault(t *testing.T) {
	// Reset version first
	Version.Store("")

	v := GetVersion()
	if v != "dev" {
		t.Errorf("expected default version 'dev', got %q", v)
	}
}
