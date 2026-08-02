package search

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestWebSearch_Name(t *testing.T) {
	ws := NewWebSearch("")
	if ws.Name() != "WebSearch" {
		t.Errorf("expected 'WebSearch', got %q", ws.Name())
	}
}

func TestWebSearch_RiskLevel(t *testing.T) {
	ws := NewWebSearch("")
	if ws.RiskLevel() != types.RiskMedium {
		t.Errorf("expected RiskMedium, got %v", ws.RiskLevel())
	}
}

func TestWebSearch_ParameterSchema(t *testing.T) {
	ws := NewWebSearch("")
	schema := ws.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !contains(schema, "query") {
		t.Error("schema missing query field")
	}
	if !contains(schema, "max_results") {
		t.Error("schema missing max_results field")
	}
	if !contains(schema, "engines") {
		t.Error("schema missing engines field")
	}
}

func TestWebSearch_ExecuteMissingQuery(t *testing.T) {
	ws := NewWebSearch("")
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing query")
	}
	if !contains(err.Error(), "missing parameter: query") {
		t.Errorf("expected 'missing parameter: query' error, got %q", err.Error())
	}
}

func TestWebSearch_ExecuteEmptyQuery(t *testing.T) {
	ws := NewWebSearch("")
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": "   ",
		},
	})
	if err == nil {
		t.Error("expected error for empty query")
	}
	if !contains(err.Error(), "query cannot be empty") {
		t.Errorf("expected 'query cannot be empty' error, got %q", err.Error())
	}
}

func TestWebSearch_ExecuteQueryTooLong(t *testing.T) {
	ws := NewWebSearch("")
	longQuery := ""
	for i := 0; i < 600; i++ {
		longQuery += "a"
	}
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": longQuery,
		},
	})
	if err == nil {
		t.Error("expected error for query too long")
	}
	if !contains(err.Error(), "query too long") {
		t.Errorf("expected 'query too long' error, got %q", err.Error())
	}
}

func TestWebSearch_ExecuteMaxResultsBounds(t *testing.T) {
	// Create a mock search server
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"results": [{"title": "Test", "url": "https://example.com", "content": "content", "engine": "google"}]}`))
	}))
	defer server.Close()

	ws := NewWebSearch(server.URL)
	ws.SetAllowPrivateIPs(true)

	// Test max_results < 1 (should clamp to 1)
	result, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query":       "test",
			"max_results": 0,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}

	// Test max_results > MaxSearchResults (should clamp to MaxSearchResults)
	result, err = ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query":       "test",
			"max_results": 100,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
}

func TestWebSearch_BaseURL(t *testing.T) {
	ws := NewWebSearch("https://custom.search.example")
	if ws.BaseURL() != "https://custom.search.example" {
		t.Errorf("expected 'https://custom.search.example', got %q", ws.BaseURL())
	}

	// Test default
	ws2 := NewWebSearch("")
	if ws2.BaseURL() != DefaultSearchBaseURL {
		t.Errorf("expected default base URL %q, got %q", DefaultSearchBaseURL, ws2.BaseURL())
	}
}

func TestWebSearch_BuildURL(t *testing.T) {
	ws := NewWebSearch("https://search.example.com")

	urlStr, err := ws.BuildURL("test query", map[string]any{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !contains(urlStr, "test+query") {
		t.Errorf("expected query in URL, got %q", urlStr)
	}
	if !contains(urlStr, "format=json") {
		t.Errorf("expected format=json in URL, got %q", urlStr)
	}
	if !contains(urlStr, "categories=general") {
		t.Errorf("expected categories=general in URL, got %q", urlStr)
	}
}

func TestWebSearch_BuildURLWithEngines(t *testing.T) {
	ws := NewWebSearch("https://search.example.com")

	urlStr, err := ws.BuildURL("test query", map[string]any{
		"engines": "google,bing",
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !contains(urlStr, "engines=google%2Cbing") {
		t.Errorf("expected engines in URL, got %q", urlStr)
	}
}

func TestWebSearch_AllowPrivateIPs(t *testing.T) {
	ws := NewWebSearch("")
	ws.SetAllowPrivateIPs(true)
	// Just verify it doesn't panic
}

func TestWebSearch_Close(t *testing.T) {
	ws := NewWebSearch("")
	ws.Close()
	// Just verify it doesn't panic
}

func TestWebSearch_DNSCache(t *testing.T) {
	ws := NewWebSearch("")
	// Verify DNSCache is initialized
	// We can't easily access the private field, but we can verify the object is created
	if ws == nil {
		t.Error("WebSearch should not be nil")
	}
}

func TestWebSearch_ExecuteWithMockServer(t *testing.T) {
	// Create a mock search server
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/search" {
			http.NotFound(w, r)
			return
		}
		// Return mock SearXNG response
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"results": [{"title": "Test Result", "url": "https://example.com", "content": "Test content", "engine": "google"}]}`))
	}))
	defer server.Close()

	ws := NewWebSearch(server.URL)
	ws.SetAllowPrivateIPs(true)

	result, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": "test",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "Test Result") {
		t.Errorf("expected 'Test Result' in output, got %q", result.Output)
	}
	if !contains(result.Output, "https://example.com") {
		t.Errorf("expected URL in output, got %q", result.Output)
	}
}

func TestWebSearch_ExecuteTruncatesResults(t *testing.T) {
	// Create a mock search server with many results
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		// Create 10 results but request max_results=3
		results := make([]map[string]string, 10)
		for i := 0; i < 10; i++ {
			results[i] = map[string]string{
				"title":   "Result " + string(rune('0'+i)),
				"url":     "https://example.com/" + string(rune('0'+i)),
				"content": "Content " + string(rune('0'+i)),
				"engine":  "google",
			}
		}
		w.Write([]byte(`{"results": ` + toJSON(results) + `}`))
	}))
	defer server.Close()

	ws := NewWebSearch(server.URL)
	ws.SetAllowPrivateIPs(true)

	result, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query":       "test",
			"max_results": float64(3),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "more results available (limit: 3)") {
		t.Errorf("expected truncation message, got %q", result.Output)
	}
}

func TestWebSearch_ExecuteNoResults(t *testing.T) {
	// Create a mock search server with no results
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"results": []}`))
	}))
	defer server.Close()

	ws := NewWebSearch(server.URL)
	ws.SetAllowPrivateIPs(true)

	result, err := ws.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"query": "test",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "No results found") {
		t.Errorf("expected 'No results found' message, got %q", result.Output)
	}
}

// Helper to convert to JSON
func toJSON(v any) string {
	// Simple JSON marshaling for test data
	if results, ok := v.([]map[string]string); ok {
		var b []byte
		b = append(b, '[')
		for i, r := range results {
			if i > 0 {
				b = append(b, ',')
			}
			b = append(b, '{')
			first := true
			for k, val := range r {
				if !first {
					b = append(b, ',')
				}
				b = append(b, '"')
				b = append(b, k...)
				b = append(b, '"')
				b = append(b, ':')
				b = append(b, '"')
				b = append(b, val...)
				b = append(b, '"')
				first = false
			}
			b = append(b, '}')
		}
		b = append(b, ']')
		return string(b)
	}
	return "[]"
}
