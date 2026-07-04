package tools

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestWebFetch_SSRFBlocksPrivateIP(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://127.0.0.1:80",
		},
	})
	if err == nil {
		t.Fatal("expected error for private IP, got nil")
	}
	if !strings.Contains(err.Error(), "private") && !strings.Contains(err.Error(), "SSRF") {
		t.Errorf("expected SSRF/private IP error, got: %v", err)
	}
}

func TestWebFetch_SSRFBlocksLinkLocal(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://169.254.169.254/latest/meta-data/",
		},
	})
	if err == nil {
		t.Fatal("expected error for link-local IP, got nil")
	}
	if !strings.Contains(err.Error(), "private") && !strings.Contains(err.Error(), "SSRF") {
		t.Errorf("expected SSRF/private IP error, got: %v", err)
	}
}

func TestWebFetch_TLSConnection(t *testing.T) {
	t.Parallel()
	// Start a test HTTPS server
	ts := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("OK"))
	}))
	defer ts.Close()

	// Create WebFetch with allowPrivateIPs to skip SSRF check on localhost
	wf := NewWebFetch(t.TempDir(), true)
	wf.client = ts.Client() // Use the test server's TLS-configured client

	result, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": ts.URL,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "OK") {
		t.Errorf("expected 'OK' in output, got: %s", result.Output)
	}
}
