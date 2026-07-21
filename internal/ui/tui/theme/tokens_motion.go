package theme

// M31A Design System v2 — Motion
//
// Motion should feel invisible.
// Use subtle fades, smooth progress, gentle streaming.
// Avoid spinners everywhere, rapid blinking, bouncing.
// Motion communicates state, not decoration.

const (
	// ── Animation Durations (milliseconds) ───────────────────────────────────

	// MotionInstant is for immediate feedback — toggles, checkboxes.
	MotionInstant = 50
	// MotionFast is for quick transitions — fade in/out.
	MotionFast = 100
	// MotionNormal is for standard transitions — screen changes.
	MotionNormal = 150
	// MotionSlow is for complex animations.
	MotionSlow = 300
	// MotionGentle is for breathing indicators and loading states.
	MotionGentle = 500
)
