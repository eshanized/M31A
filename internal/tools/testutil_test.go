package tools

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
)

// testDispatcher returns a new Dispatcher that will be stopped when the test ends.
// Uses a test decider that mimics the old permission behavior for testing.
func testDispatcher(t *testing.T) *Dispatcher {
	t.Helper()
	// Create dispatcher with nil policy first, then set test decider with dispatcher's channels
	d, _ := DefaultDispatcher("", "", "", nil, nil, nil)
	// Set test decider using dispatcher's own channels
	d.SetPermissionDecider(NewTestDecider(
		d.RequestCh(),
		d.ResponseCh(),
		types.DefaultPermissionTimeout,
	))
	t.Cleanup(func() { d.Stop() })
	return d
}

// testDispatcherWithConfig returns a new Dispatcher with the given config that will be stopped when the test ends.
func testDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *Dispatcher {
	t.Helper()
	d, _ := DefaultDispatcher("", "", "", cfg, nil, nil)
	d.SetPermissionDecider(NewTestDecider(
		d.RequestCh(),
		d.ResponseCh(),
		types.DefaultPermissionTimeout,
	))
	t.Cleanup(func() { d.Stop() })
	return d
}
