package tui

import "github.com/eshanized/M31A/internal/ui/tui/tuitypes"

// Screenable is the interface that all TUI screens must implement.
// This enables a router-based architecture where screens are
// registered and delegated to, rather than using giant switch statements.
type Screenable = tuitypes.Screenable
