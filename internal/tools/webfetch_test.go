package tools

import (
	"context"
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
