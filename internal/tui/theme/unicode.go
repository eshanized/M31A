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

// ── Symbol Override Support ──────────────────────────────────────────────────

// unicodeSymbols maps logical symbol names to their Unicode characters.
// Used by GetSymbol for override and ASCII fallback support.
var unicodeSymbols = map[string]string{
	"check":       StatusPass,
	"cross":       StatusFail,
	"warning":     StatusWarn,
	"pending":     StatusPending,
	"info":        "ℹ",
	"arrow_right": string(NavRight),
	"arrow_up":    string(ArrowUp),
	"arrow_down":  string(ArrowDown),
	"spinner":     SpinnerBraille,
	"block_full":  BlockFull,
	"block_high":  BlockHigh,
	"block_med":   BlockMed,
	"block_low":   BlockLow,
	"phase_init":  PhaseInit,
	"phase_plan":  PhasePlan,
	"phase_exec":  PhaseExecute,
	"phase_ship":  PhaseShip,
	"nav_cursor":  NavCursor,
	"nav_arrow":   NavArrow,
	"nav_play":    NavPlay,
	"section_1":   SectionMarker1,
	"section_2":   SectionMarker2,
	"section_3":   SectionMarker3,
	"section_4":   SectionMarker4,
	"file_go":     FileGo,
	"file_js":     FileJS,
	"file_md":     FileMD,
	"file_json":   FileJSON,
	"file_yaml":   FileYAML,
	"gradient_l":  GradientLight,
	"gradient_m":  GradientMedium,
	"gradient_h":  GradientHeavy,
	"gradient_f":  GradientFull,
}

// asciiSymbols maps symbol names to ASCII fallback characters.
// Used when ASCIIFallback is enabled for terminals with poor Unicode support.
var asciiSymbols = map[string]string{
	"check":       "[OK]",
	"cross":       "[FAIL]",
	"warning":     "[!]",
	"pending":     "[.]",
	"info":        "[i]",
	"arrow_right": "->",
	"arrow_up":    "^",
	"arrow_down":  "v",
	"spinner":     "*",
	"block_full":  "#",
	"block_high":  "#",
	"block_med":   "#",
	"block_low":   ".",
	"phase_init":  "O",
	"phase_plan":  "P",
	"phase_exec":  ">",
	"phase_ship":  "S",
	"nav_cursor":  ">",
	"nav_arrow":   ">",
	"nav_play":    ">",
	"section_1":   "*",
	"section_2":   "o",
	"section_3":   "*",
	"section_4":   "o",
	"file_go":     "G",
	"file_js":     "J",
	"file_md":     "M",
	"file_json":   "{",
	"file_yaml":   "Y",
	"gradient_l":  ".",
	"gradient_m":  "#",
	"gradient_h":  "#",
	"gradient_f":  "#",
}

// GetSymbol returns the symbol for a given name, applying overrides and
// ASCII fallback. Priority: override > ASCII fallback (when enabled) > Unicode default.
func GetSymbol(name string, overrides map[string]string, asciiFallback bool) string {
	// Check override first
	if override, ok := overrides[name]; ok && override != "" {
		return override
	}

	// ASCII fallback
	if asciiFallback {
		if ascii, ok := asciiSymbols[name]; ok {
			return ascii
		}
	}

	// Default Unicode symbol
	if sym, ok := unicodeSymbols[name]; ok {
		return sym
	}

	// Unknown symbol — return name as-is
	return name
}
