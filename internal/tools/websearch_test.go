package tools

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestWebSearch_Name(t *testing.T) {
	ws := NewWebSearch("")
	if ws.Name() != "WebSearch" {
		t.Fatalf("expected name WebSearch, got %s", ws.Name())
	}
}

func TestWebSearch_RiskLevel(t *testing.T) {
	ws := NewWebSearch("")
	if ws.RiskLevel() != types.RiskMedium {
		t.Fatalf("expected RiskMedium, got %s", ws.RiskLevel())
	}
}

func TestWebSearch_MissingQuery(t *testing.T) {
	ws := NewWebSearch("")
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{},
	})
	if err == nil {
		t.Fatal("expected error for missing query")
	}
	if !strings.Contains(err.Error(), "missing parameter") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWebSearch_EmptyQuery(t *testing.T) {
	ws := NewWebSearch("")
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{"query": "   "},
	})
	if err == nil {
		t.Fatal("expected error for empty query")
	}
	if !strings.Contains(err.Error(), "cannot be empty") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWebSearch_QueryTooLong(t *testing.T) {
	ws := NewWebSearch("")
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name: "WebSearch",
		Params: map[string]any{
			"query": strings.Repeat("a", MaxSearchQueryLength+1),
		},
	})
	if err == nil {
		t.Fatal("expected error for query too long")
	}
	if !strings.Contains(err.Error(), "too long") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWebSearch_QueryWrongType(t *testing.T) {
	ws := NewWebSearch("")
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{"query": 123},
	})
	if err == nil {
		t.Fatal("expected error for non-string query")
	}
	if !strings.Contains(err.Error(), "must be a string") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWebSearch_Success(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		q := r.URL.Query().Get("q")
		if q == "" {
			http.Error(w, "missing q", http.StatusBadRequest)
			return
		}
		if r.URL.Query().Get("format") != "json" {
			t.Errorf("expected format=json, got %s", r.URL.Query().Get("format"))
		}
		resp := searxngResponse{
			Results: []searxngResult{
				{Title: "Go Blog", URL: "https://go.dev/blog", Content: "Error handling in Go", Engine: "google"},
				{Title: "Go Wiki", URL: "https://go.dev/wiki", Content: "Error handling patterns", Engine: "bing"},
			},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	result, err := ws.Execute(context.Background(), types.ToolInput{
		Name: "WebSearch",
		Params: map[string]any{
			"query": "golang error handling",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Go Blog") {
		t.Errorf("expected output to contain 'Go Blog', got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "https://go.dev/blog") {
		t.Errorf("expected output to contain URL, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "Found 2 results") {
		t.Errorf("expected 'Found 2 results', got: %s", result.Output)
	}
	if result.Truncated {
		t.Error("expected not truncated")
	}
}

func TestWebSearch_Truncation(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		results := make([]searxngResult, 8)
		for i := range results {
			results[i] = searxngResult{
				Title:   "Result",
				URL:     "https://example.com",
				Content: "content",
			}
		}
		resp := searxngResponse{Results: results}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	result, err := ws.Execute(context.Background(), types.ToolInput{
		Name: "WebSearch",
		Params: map[string]any{
			"query":       "test",
			"max_results": 3.0,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !result.Truncated {
		t.Error("expected truncated")
	}
	if !strings.Contains(result.Output, "Found 3 results") {
		t.Errorf("expected 'Found 3 results', got: %s", result.Output)
	}
}

func TestWebSearch_EnginesParam(t *testing.T) {
	var gotEngines string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotEngines = r.URL.Query().Get("engines")
		resp := searxngResponse{Results: []searxngResult{}}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name: "WebSearch",
		Params: map[string]any{
			"query":   "test",
			"engines": "google,duckduckgo",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotEngines != "google,duckduckgo" {
		t.Errorf("expected engines=google,duckduckgo, got %s", gotEngines)
	}
}

func TestWebSearch_NoResults(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resp := searxngResponse{Results: []searxngResult{}}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	result, err := ws.Execute(context.Background(), types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{"query": "xyznonexistent"},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "No results found") {
		t.Errorf("expected 'No results found', got: %s", result.Output)
	}
}

func TestWebSearch_HTTPError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "service unavailable", http.StatusServiceUnavailable)
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{"query": "test"},
	})
	if err == nil {
		t.Fatal("expected error for HTTP 503")
	}
	if !strings.Contains(err.Error(), "503") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWebSearch_MalformedJSON(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.Write([]byte("not json"))
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	_, err := ws.Execute(context.Background(), types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{"query": "test"},
	})
	if err == nil {
		t.Fatal("expected error for malformed JSON")
	}
	if !strings.Contains(err.Error(), "parse") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWebSearch_ContextCancellation(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		<-r.Context().Done()
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := ws.Execute(ctx, types.ToolInput{
		Name:   "WebSearch",
		Params: map[string]any{"query": "test"},
	})
	if err == nil {
		t.Fatal("expected error for cancelled context")
	}
}

func TestWebSearch_MaxResultsClamp(t *testing.T) {
	var resultCount int
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		results := make([]searxngResult, 10)
		for i := range results {
			results[i] = searxngResult{Title: "R", URL: "https://x.com", Content: "c"}
		}
		resultCount = len(results)
		resp := searxngResponse{Results: results}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	ws := NewWebSearch(srv.URL)
	ws.allowPrivateIPs = true
	result, err := ws.Execute(context.Background(), types.ToolInput{
		Name: "WebSearch",
		Params: map[string]any{
			"query":       "test",
			"max_results": 999.0,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Found 10 results") {
		t.Errorf("expected clamped to MaxSearchResults=10, got: %s", result.Output)
	}
	_ = resultCount
}

func TestWebSearch_DefaultBaseURL(t *testing.T) {
	ws := NewWebSearch("")
	if ws.baseURL != DefaultSearchBaseURL {
		t.Errorf("expected default base URL %s, got %s", DefaultSearchBaseURL, ws.baseURL)
	}
}

func TestWebSearch_TrailingSlashStripped(t *testing.T) {
	ws := NewWebSearch("https://example.com/")
	if ws.baseURL != "https://example.com" {
		t.Errorf("expected trailing slash stripped, got %s", ws.baseURL)
	}
}
