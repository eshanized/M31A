package tools

import (
	"context"
	"errors"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/pkg/types"
)

func TestWebFetch_Blocks_PrivateIPv4(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	input := types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://10.0.0.1/",
		},
	}
	_, err := wf.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for private IPv4")
	}
	if !errors.Is(err, m31errors.ErrPrivateIPBlocked) {
		t.Fatalf("expected ErrPrivateIPBlocked, got: %v", err)
	}
}

func TestWebFetch_Blocks_Loopback(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	input := types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://127.0.0.1/",
		},
	}
	_, err := wf.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for loopback")
	}
	if !errors.Is(err, m31errors.ErrPrivateIPBlocked) {
		t.Fatalf("expected ErrPrivateIPBlocked, got: %v", err)
	}
}

func TestWebFetch_Blocks_PrivateIPv6_ULA(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	input := types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://[fc00::1]/",
		},
	}
	_, err := wf.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for IPv6 ULA")
	}
	if !errors.Is(err, m31errors.ErrPrivateIPBlocked) {
		t.Fatalf("expected ErrPrivateIPBlocked, got: %v", err)
	}
}

func TestWebFetch_Blocks_PrivateIPv6_LinkLocal(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	input := types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://[fe80::1]/",
		},
	}
	_, err := wf.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for IPv6 link-local")
	}
	if !errors.Is(err, m31errors.ErrPrivateIPBlocked) {
		t.Fatalf("expected ErrPrivateIPBlocked, got: %v", err)
	}
}

func TestWebFetch_Blocks_IPv4MappedIPv6(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	input := types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://[::ffff:10.0.0.1]/",
		},
	}
	_, err := wf.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for IPv4-mapped IPv6")
	}
	if !errors.Is(err, m31errors.ErrPrivateIPBlocked) {
		t.Fatalf("expected ErrPrivateIPBlocked, got: %v", err)
	}
}

func TestWebFetch_Allows_PublicDNS(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	input := types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "http://example.com/",
		},
	}
	_, err := wf.Execute(context.Background(), input)
	// The request may fail (network/DNS), but it should NOT be ErrPrivateIPBlocked
	if err != nil && errors.Is(err, m31errors.ErrPrivateIPBlocked) {
		t.Fatalf("public URL should not be blocked by SSRF, got: %v", err)
	}
}

func TestWebFetch_SharedClient(t *testing.T) {
	wf := NewWebFetch(t.TempDir(), false, 3, 100)
	if wf.client == nil {
		t.Fatal("expected non-nil client after NewWebFetch")
	}
	// Verify the same client is reused across calls
	client1 := wf.client
	client2 := wf.client
	if client1 != client2 {
		t.Error("expected same client instance to be reused")
	}
}
