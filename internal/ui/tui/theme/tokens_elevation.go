package theme

// M31A Design System v2 — Elevation
//
// Elevation communicates visual hierarchy.
// Higher elevation = more important = more visual weight.

const (
	// ── Elevation Levels ─────────────────────────────────────────────────────

	// ElevationNone is flat, no shadow — for inline content.
	ElevationNone = 0
	// ElevationLow is subtle shadow — for cards and panels.
	ElevationLow = 1
	// ElevationMedium is moderate shadow — for modals.
	ElevationMedium = 2
	// ElevationHighest is prominent shadow — for command palette and overlays.
	ElevationHighest = 3
)
