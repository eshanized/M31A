package tools

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
)

// testDispatcher returns a new Dispatcher that will be stopped when the test ends.
func testDispatcher(t *testing.T) *Dispatcher {
	t.Helper()
	d, _ := DefaultDispatcher("", "", "", nil, nil)
	t.Cleanup(func() { d.Stop() })
	return d
}

// testDispatcherWithConfig returns a new Dispatcher with the given config that will be stopped when the test ends.
func testDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *Dispatcher {
	t.Helper()
	d, _ := DefaultDispatcher("", "", "", cfg, nil)
	t.Cleanup(func() { d.Stop() })
	return d
}
