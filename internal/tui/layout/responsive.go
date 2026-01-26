package layout

// Breakpoint classifies terminal width for responsive layout decisions.
type Breakpoint int

const (
	// UltraNarrow: < 40 cols — show "resize terminal" message only.
	UltraNarrow Breakpoint = iota
	// Compact: 40–59 cols — minimal chrome, no sidebar, no hints.
	Compact
	// Standard: 60–79 cols — full chrome, no sidebar.
	Standard
	// Full: ≥ 80 cols — sidebar visible, all chrome, full key hints.
	Full
)

// Width thresholds.
const (
	MinWidth    = 40
	CompactMax  = 59
	StandardMax = 79
)

// Detect returns the breakpoint for a given terminal width.
func Detect(width int) Breakpoint {
	switch {
	case width < MinWidth:
		return UltraNarrow
	case width <= CompactMax:
		return Compact
	case width <= StandardMax:
		return Standard
	default:
		return Full
	}
}

// ShowSidebar returns true when the sidebar should be displayed alongside
// content (width ≥ 80). Below this threshold the sidebar is hidden and can
// only be shown as an overlay via ctrl+b.
func ShowSidebar(width int) bool {
	return width >= MinWidth*2 // 80
}

// ShowHeaderRight returns true when the header right zone (model badge,
// context meter) should be rendered. Hidden below 60 cols.
func ShowHeaderRight(width int) bool {
	return width >= 60
}

// ShowFooterHints returns true when keyboard shortcut hints should appear
// in the footer. Hidden below 60 cols.
func ShowFooterHints(width int) bool {
	return width >= 60
}

// ShowFooterCost returns true when token/cost info should appear in the
// footer. Hidden below 80 cols.
func ShowFooterCost(width int) bool {
	return width >= 80
}

// ChromeHeight is the total number of rows consumed by the unified header
// and footer. Content height = terminal height - ChromeHeight.
const ChromeHeight = 2
