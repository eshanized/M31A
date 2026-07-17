package builders

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tools"
)

// NewTestDispatcher creates a tools.Dispatcher configured for testing.
// The dispatcher is stopped automatically when the test ends.
func NewTestDispatcher(t *testing.T) *tools.Dispatcher {
	t.Helper()
	d, err := tools.DefaultDispatcher("", "", "", nil, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	t.Cleanup(func() { d.Stop() })
	return d
}

// NewTestDispatcherWithConfig creates a tools.Dispatcher with the given
// permission config for testing. The dispatcher is stopped automatically
// when the test ends.
func NewTestDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *tools.Dispatcher {
	t.Helper()
	d, err := tools.DefaultDispatcher("", "", "", cfg, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	t.Cleanup(func() { d.Stop() })
	return d
}
