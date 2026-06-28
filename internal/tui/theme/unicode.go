package theme

// ── Section Markers ──────────────────────────────────────────────────────────
// Simple, monochrome, meaningful.
const (
	SectionMarker1 = "◆"
	SectionMarker2 = "◇"
	SectionMarker3 = "●"
	SectionMarker4 = "○"
)

// ── File Type Icons ──────────────────────────────────────────────────────────
// Monochrome indicators for file extensions.
const (
	FileGo      = "◆"
	FileJS      = "◇"
	FileMD      = "▬"
	FileJSON    = "●"
	FileYAML    = "▭"
	FileDefault = "□"
)

// ── Workflow Phase Icons ─────────────────────────────────────────────────────
// Each phase has a distinct shape for pipeline visualization.
const (
	PhaseInit    = "○"
	PhaseDiscuss = "◎"
	PhasePlan    = "◇"
	PhaseExecute = "▶"
	PhaseVerify  = "✓"
	PhaseRuntime = "●"
	PhaseShip    = "◆"
)

// ── Navigation Icons ─────────────────────────────────────────────────────────
// Directional indicators.
const (
	NavCursor  = "▸"
	NavArrow   = "▹"
	NavPlay    = "▶"
	NavForward = "▻"
	NavRight   = "❯"
	NavLeft    = "❮"
)

// ── Status Icons ─────────────────────────────────────────────────────────────
// Clear, universally understood status indicators.
const (
	StatusPass    = "✓"
	StatusFail    = "✗"
	StatusWarn    = "⚠"
	StatusPending = "○"
	StatusHalf    = "◐"
	StatusCircle  = "◑"
	StatusQuarter = "◒"
	StatusThreeQ  = "◓"
	StatusRetry   = "⟲"
	StatusRefresh = "⟳"
)

// ── Arrow Icons ──────────────────────────────────────────────────────────────
const (
	ArrowUpRight   = "↗"
	ArrowDownRight = "↘"
	ArrowDownLeft  = "↙"
	ArrowUpLeft    = "↖"
	ArrowUp        = "⇡"
	ArrowDown      = "⇣"
)

// ── Line Characters ──────────────────────────────────────────────────────────
const (
	LineDashed1 = "╌"
	LineDashed2 = "╎"
	LineDashed3 = "┄"
	LineDashed4 = "┈"
)

// ── Block Characters ─────────────────────────────────────────────────────────
// Density indicators for progress bars and sparklines.
const (
	BlockFull = "█"
	BlockHigh = "▓"
	BlockMed  = "▒"
	BlockLow  = "░"
)

// ── Spinner Characters ───────────────────────────────────────────────────────
// Reduced to two: active (braille) and idle (dots).
const (
	SpinnerBraille = "⠋"
	SpinnerDots    = "⣾"
)

// ── Card and Border Characters ───────────────────────────────────────────────
// Use standard box-drawing characters, not decorative ones.
const (
	CardHorizontal = "─"
	CardVertical   = "│"
)

// ── Gradient Characters ──────────────────────────────────────────────────────
const (
	GradientLight  = "░"
	GradientMedium = "▒"
	GradientHeavy  = "▓"
	GradientFull   = "█"
)

// GetFileTypeIcon returns the icon for a given file extension.
func GetFileTypeIcon(ext string) string {
	switch ext {
	case ".go":
		return FileGo
	case ".js", ".ts", ".jsx", ".tsx":
		return FileJS
	case ".md", ".markdown":
		return FileMD
	case ".json":
		return FileJSON
	case ".yaml", ".yml":
		return FileYAML
	default:
		return FileDefault
	}
}

// GetPhaseIcon returns the icon for a workflow phase.
func GetPhaseIcon(phase string) string {
	switch phase {
	case "initialize":
		return PhaseInit
	case "discuss":
		return PhaseDiscuss
	case "plan":
		return PhasePlan
	case "execute":
		return PhaseExecute
	case "verify":
		return PhaseVerify
	case "runtime":
		return PhaseRuntime
	case "ship":
		return PhaseShip
	default:
		return PhaseInit
	}
}

// GetStatusIcon returns the icon for a status.
func GetStatusIcon(status string) string {
	switch status {
	case "done", "pass", "complete":
		return StatusPass
	case "failed", "error":
		return StatusFail
	case "warning", "skipped":
		return StatusWarn
	case "pending":
		return StatusPending
	case "running":
		return StatusRefresh
	default:
		return StatusPending
	}
}
