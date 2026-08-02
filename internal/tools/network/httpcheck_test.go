package network

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestHTTPCheck_ExecuteBlocksPrivateIP(t *testing.T) {
	t.Parallel()
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	}))
	defer server.Close()

	h := NewHTTPCheck()

	result, err := h.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url": server.URL,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// SSRF protection should block localhost
	if result.Error == "" {
		t.Errorf("expected SSRF error for localhost, got no error")
	}
	if result.Output == "" {
		t.Errorf("expected non-empty output")
	}
}

func TestHTTPCheck_ExecuteMissingURL(t *testing.T) {
	t.Parallel()
	h := NewHTTPCheck()

	result, err := h.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for missing URL")
	}
}

func TestHTTPCheck_ExecuteWrongStatus(t *testing.T) {
	t.Parallel()
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound)
		w.Write([]byte("Not Found"))
	}))
	defer server.Close()

	h := NewHTTPCheck()

	result, err := h.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"url":             server.URL,
			"expected_status": 200,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for wrong status")
	}
}

func TestHTTPCheck_RiskLevel(t *testing.T) {
	t.Parallel()
	h := NewHTTPCheck()
	if h.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %v", h.RiskLevel())
	}
}

func TestHTTPCheck_Name(t *testing.T) {
	t.Parallel()
	h := NewHTTPCheck()
	if h.Name() != "HTTPCheck" {
		t.Errorf("expected 'HTTPCheck', got %q", h.Name())
	}
}

func TestHTTPCheck_Description(t *testing.T) {
	t.Parallel()
	h := NewHTTPCheck()
	desc := h.Description()
	if desc == "" {
		t.Errorf("expected non-empty description")
	}
}

func TestHTTPCheck_ParameterSchema(t *testing.T) {
	t.Parallel()
	h := NewHTTPCheck()
	schema := h.ParameterSchema()
	if schema == "" {
		t.Errorf("expected non-empty schema")
	}
}
