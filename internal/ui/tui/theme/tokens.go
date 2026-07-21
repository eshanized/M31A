package theme

// M31A Design System v2 — Color Palette
//
// Apple-inspired: calm, elegant, premium, understated.
// Colors communicate meaning, never decoration.
// Never fully saturated. Never pure white. Never pure black.
// Maintain excellent contrast while remaining comfortable during long sessions.

const (
	// ── Backgrounds ──────────────────────────────────────────────────────────
	// Very dark graphite hierarchy. Each level is slightly lighter.

	// BgBase is the deepest background color.
	BgBase = "#0F1117"
	// BgSurface is slightly elevated, used for main content areas.
	BgSurface = "#161822"
	// BgSurfaceHigh is for cards, panels, and elevated surfaces.
	BgSurfaceHigh = "#1C1F2E"
	// BgSurfaceHighest is for modals, popovers, and overlays.
	BgSurfaceHighest = "#222538"

	// ── Borders ──────────────────────────────────────────────────────────────
	// Almost invisible. Use spacing before borders.

	// BorderDefault is the standard border color for cards and panels.
	BorderDefault = "#2A2D3E"
	// BorderSubtle is barely visible, used for faint separators.
	BorderSubtle = "#1E2030"
	// BorderFocus is for focus rings and active states.
	BorderFocus = "#4A6CF7"

	// ── Text ─────────────────────────────────────────────────────────────────
	// Soft white hierarchy. Never pure white.

	// TextPrimary is the main text color — soft white.
	TextPrimary = "#E2E4E9"
	// TextSecondary is for secondary information.
	TextSecondary = "#8B8FA3"
	// TextMuted is for hints, metadata, and labels.
	TextMuted = "#5C5F73"
	// TextFaint is very dim, used for decorative metadata.
	TextFaint = "#3D3F52"

	// ── Accent ───────────────────────────────────────────────────────────────
	// Professional blue. The primary interactive color.

	// AccentPrimary is used for actions, links, and active states.
	AccentPrimary = "#4A6CF7"
	// AccentSoft is a subtle background tint of the accent color.
	AccentSoft = "#4A6CF720"

	// ── Semantic Colors ──────────────────────────────────────────────────────
	// Calm, desaturated. Each communicates a clear state.

	// Thinking is soft blue for AI thinking state.
	Thinking = "#6B9BF7"
	// ThinkingSoft is the background tint for thinking state.
	ThinkingSoft = "#6B9BF720"

	// Success is calm green for completed actions.
	Success = "#5CB88A"
	// SuccessSoft is the background tint for success state.
	SuccessSoft = "#5CB88A20"

	// Error is controlled red — never aggressive.
	Error = "#E05C5C"
	// ErrorSoft is the background tint for error state.
	ErrorSoft = "#E05C5C20"

	// Warning is warm amber for caution.
	Warning = "#D4A053"
	// WarningSoft is the background tint for warning state.
	WarningSoft = "#D4A05320"

	// ── Tool / Execution Colors ──────────────────────────────────────────────

	// ToolExec is muted cyan for tool execution indicators.
	ToolExec = "#5CBAD1"
	// ToolExecSoft is the background tint for tool execution.
	ToolExecSoft = "#5CBAD120"

	// Autonomous is muted purple for autonomous execution.
	Autonomous = "#9B7BF7"
	// AutonomousSoft is the background tint for autonomous state.
	AutonomousSoft = "#9B7BF720"

	// ── Diff Colors ──────────────────────────────────────────────────────────

	DiffAdded     = "#5CB88A"
	DiffRemoved   = "#E05C5C"
	DiffAddedBg   = "#5CB88A15"
	DiffRemovedBg = "#E05C5C15"
	DiffContextBg = "#1C1F2E"

	// ── Utility Colors ───────────────────────────────────────────────────────

	// Selection is the background for selected/highlighted items.
	Selection = "#4A6CF730"
	// Shadow is used for modal drop shadows.
	Shadow = "#00000050"
	// CodeBg is the background for code blocks.
	CodeBg = "#161822"
)
