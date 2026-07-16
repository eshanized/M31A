package tools

import (
	"context"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/types"
)

type mockToolTimeout struct {
	name      string
	riskLevel types.RiskLevel
}

func (m *mockToolTimeout) Name() string               { return m.name }
func (m *mockToolTimeout) Description() string        { return "mock tool for timeout test" }
func (m *mockToolTimeout) RiskLevel() types.RiskLevel { return m.riskLevel }
func (m *mockToolTimeout) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	return types.ToolResult{Output: "success"}, nil
}

func TestPermissionTimeout(t *testing.T) {
	t.Run("Permission request times out", func(t *testing.T) {
		d := testDispatcherWithConfig(t, &config.PermissionsConfig{
			TimeoutSeconds: 1,
			Rules: []config.PermissionRule{
				{
					Tool:      "bash",
					RiskLevel: types.RiskDangerous,
					Action:    "ask",
				},
			},
		})
		d.Register(&mockToolTimeout{name: "bash", riskLevel: types.RiskDangerous})

		errCh := make(chan error, 1)
		go func() {
			_, err := d.Execute(context.Background(), types.ToolCall{
				ID:    "call1",
				Name:  "bash",
				Input: []byte(`{"name": "bash", "params": {"command": "echo hello"}}`),
			})
			errCh <- err
		}()

		// Wait for permission request
		req := <-d.RequestCh()
		if req.ToolName != "bash" {
			t.Errorf("expected request for 'bash', got %q", req.ToolName)
		}

		// Don't respond - should timeout after 1 second
		select {
		case err := <-errCh:
			if err == nil {
				t.Error("expected timeout error, got nil")
			} else {
				t.Logf("Got expected timeout error: %v", err)
			}
		case <-time.After(3 * time.Second):
			t.Error("timed out waiting for permission timeout")
		}
	})

	t.Run("Permission request succeeds before timeout", func(t *testing.T) {
		d := testDispatcherWithConfig(t, &config.PermissionsConfig{
			TimeoutSeconds: 2,
			Rules: []config.PermissionRule{
				{
					Tool:      "bash",
					RiskLevel: types.RiskDangerous,
					Action:    "ask",
				},
			},
		})
		d.Register(&mockToolTimeout{name: "bash", riskLevel: types.RiskDangerous})

		errCh := make(chan error, 1)
		go func() {
			_, err := d.Execute(context.Background(), types.ToolCall{
				ID:    "call1",
				Name:  "bash",
				Input: []byte(`{"name": "bash", "params": {"command": "echo hello"}}`),
			})
			errCh <- err
		}()

		// Wait for permission request
		req := <-d.RequestCh()
		if req.ToolName != "bash" {
			t.Errorf("expected request for 'bash', got %q", req.ToolName)
		}

		// Respond before timeout
		d.ApprovePermission(req.ID, true, false)

		select {
		case err := <-errCh:
			if err != nil {
				t.Errorf("expected nil error after approval, got: %v", err)
			}
		case <-time.After(3 * time.Second):
			t.Error("timed out waiting for permission response")
		}
	})
}
