package theme

// M31A Design System v2 — Spacing Scale
//
// Consistent spacing creates rhythm and calm.
// Every pixel must have a purpose.

const (
	// ── Spacing Units ────────────────────────────────────────────────────────
	// Used for logical spacing between elements.

	// Spacing0 is no space.
	Spacing0 = 0
	// Spacing1 is tight spacing — between icon and text.
	Spacing1 = 1
	// Spacing2 is small spacing — between related items.
	Spacing2 = 2
	// Spacing3 is medium spacing — between sections.
	Spacing3 = 3
	// Spacing4 is large spacing — between major sections.
	Spacing4 = 4

	// ── Padding Scale ────────────────────────────────────────────────────────
	// Used in lipgloss Padding() calls for component internal spacing.

	// PadNone is no padding.
	PadNone = 0
	// PadTight is minimal padding for compact components.
	PadTight = 1
	// PadNormal is standard padding for most components.
	PadNormal = 2
	// PadRelaxed is generous padding for spacious layouts.
	PadRelaxed = 3
	// PadLoose is maximum padding for emphasis.
	PadLoose = 4

	// ── Semantic Padding Aliases ─────────────────────────────────────────────
	// Semantic names for consistent usage across all screens.

	// PaddingCompact is alias for PadTight — dense UI elements.
	PaddingCompact = PadTight
	// PaddingStandard is alias for PadNormal — most components.
	PaddingStandard = PadNormal
	// PaddingRelaxed is alias for PadRelaxed — spacious layouts.
	PaddingRelaxed = PadRelaxed
)
