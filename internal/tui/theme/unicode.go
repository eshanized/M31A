package theme

// Section markers
const (
	SectionMarker1 = "◈"
	SectionMarker2 = "◇"
	SectionMarker3 = "◆"
	SectionMarker4 = "◉"
	SectionMarker5 = "⊕"
	SectionMarker6 = "⊗"
)

// File type icons
const (
	FileGo      = "◇"
	FileJS      = "◆"
	FileMD      = "▤"
	FileJSON    = "◈"
	FileYAML    = "▥"
	FileDefault = "▣"
)

// Workflow phase icons
const (
	PhaseInit    = "⬡"
	PhaseDiscuss = "⬢"
	PhasePlan    = "◬"
	PhaseExecute = "△"
	PhaseVerify  = "▽"
	PhaseShip    = "◈"
)

// Navigation icons
const (
	NavCursor  = "▸"
	NavArrow   = "▹"
	NavPlay    = "►"
	NavForward = "▻"
	NavRight   = "❯"
	NavLeft    = "❮"
)

// Status icons
const (
	StatusPass    = "✓"
	StatusFail    = "✗"
	StatusWarn    = "⚠"
	StatusPending = "●"
	StatusHalf    = "◐"
	StatusCircle  = "◑"
	StatusQuarter = "◒"
	StatusThreeQ  = "◓"
	StatusRetry   = "⟲"
	StatusRefresh = "⟳"
)

// Arrow icons
const (
	ArrowUpRight   = "↗"
	ArrowDownRight = "↘"
	ArrowDownLeft  = "↙"
	ArrowUpLeft    = "↖"
	ArrowUp        = "⇡"
	ArrowDown      = "⇣"
)

// Decorative lines
const (
	LineDashed1 = "╌"
	LineDashed2 = "╎"
	LineDashed3 = "┄"
	LineDashed4 = "┈"
)

// Block characters for progress bars and density indicators
const (
	BlockFull = "█"
	BlockHigh = "▓"
	BlockMed  = "▒"
	BlockLow  = "░"
)

// Spinner characters
const (
	SpinnerBraille  = "⠋"
	SpinnerDots     = "⣾"
	SpinnerArc      = "◜"
	SpinnerBouncing = "⠁"
	SpinnerLine     = "|"
	SpinnerGrow     = "▏"
	SpinnerPulse    = "◐"
)

// Card and border characters
const (
	CardTopLeft     = "╭"
	CardTopRight    = "╮"
	CardBottomLeft  = "╰"
	CardBottomRight = "╯"
	CardHorizontal  = "─"
	CardVertical    = "│"
)

// Gradient characters
const (
	GradientLight  = "░"
	GradientMedium = "▒"
	GradientHeavy  = "▓"
	GradientFull   = "█"
)

// GetFileTypeIcon returns the icon for a given file extension
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

// GetPhaseIcon returns the icon for a workflow phase
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
	case "ship":
		return PhaseShip
	default:
		return PhaseInit
	}
}

// GetStatusIcon returns the icon for a status
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
