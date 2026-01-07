package components

import (
	"fmt"
	"math"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ProgressBar renders a visual progress indicator.
type ProgressBar struct {
	Progress float64 // 0.0 to 1.0
	Width    int     // total width including percentage
	ShowPct  bool    // show percentage text
	Style    ProgressBarStyle
	Theme    theme.Theme
}

// ProgressBarStyle defines the visual style of the progress bar.
type ProgressBarStyle int

const (
	ProgressBarThin ProgressBarStyle = iota
	ProgressBarThick
	ProgressBarBlock
	ProgressBarRounded
)

// Render returns the progress bar as a string.
func (p ProgressBar) Render() string {
	if p.Width <= 0 {
		p.Width = 30
	}

	t := p.Theme
	if t.Text == "" {
		t = theme.Default()
	}
	pct := p.Progress
	if pct < 0 {
		pct = 0
	}
	if pct > 1 {
		pct = 1
	}

	filledWidth := int(math.Round(pct * float64(p.Width)))
	if filledWidth > p.Width {
		filledWidth = p.Width
	}
	emptyWidth := p.Width - filledWidth

	var bar string

	switch p.Style {
	case ProgressBarThin:
		filled := strings.Repeat("━", filledWidth)
		empty := strings.Repeat("─", emptyWidth)
		bar = filled + empty

	case ProgressBarThick:
		filled := strings.Repeat("█", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty

	case ProgressBarBlock:
		filled := strings.Repeat("█", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty

	case ProgressBarRounded:
		filled := strings.Repeat("▓", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty

	default:
		filled := strings.Repeat("█", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty
	}

	// Apply gradient fill for brand color
	if pct > 0 && pct < 1.0 {
		// Create gradient from brand to lighter shade
		gradientStyle := lipgloss.NewStyle().Foreground(t.Brand)
		bar = gradientStyle.Render(bar)
	} else if pct >= 1.0 {
		// Success color for completion
		progressStyle := lipgloss.NewStyle().Foreground(t.Success)
		bar = progressStyle.Render(bar)
	} else {
		// Empty state
		progressStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
		bar = progressStyle.Render(bar)
	}

	if p.ShowPct {
		pctStr := fmt.Sprintf("%d%%", int(math.Round(pct*100)))
		pctStyle := lipgloss.NewStyle().
			Foreground(t.TextSecondary)
		bar = bar + " " + pctStyle.Render(pctStr)
	}

	return bar
}

// RenderWithLabel returns the progress bar with a label inside the filled portion
func (p ProgressBar) RenderWithLabel(label string) string {
	if p.Width <= 0 {
		p.Width = 30
	}

	t := p.Theme
	if t.Text == "" {
		t = theme.Default()
	}
	pct := p.Progress
	if pct < 0 {
		pct = 0
	}
	if pct > 1 {
		pct = 1
	}

	filledWidth := int(math.Round(pct * float64(p.Width)))
	if filledWidth > p.Width {
		filledWidth = p.Width
	}
	emptyWidth := p.Width - filledWidth

	// Only show label if bar is wide enough
	showLabel := p.Width > 20 && label != ""

	var bar string

	// Create filled portion with optional label
	filled := ""
	if showLabel && filledWidth > len(label)+2 {
		// Center label in filled portion
		padding := (filledWidth - len(label)) / 2
		if padding < 0 {
			padding = 0
		}
		filled = strings.Repeat("█", padding) + label + strings.Repeat("█", filledWidth-padding-len(label))
	} else {
		filled = strings.Repeat("█", filledWidth)
	}

	empty := strings.Repeat("░", emptyWidth)
	bar = filled + empty

	// Apply gradient fill
	if pct > 0 && pct < 1.0 {
		gradientStyle := lipgloss.NewStyle().Foreground(t.Brand)
		bar = gradientStyle.Render(bar)
	} else if pct >= 1.0 {
		progressStyle := lipgloss.NewStyle().Foreground(t.Success)
		bar = progressStyle.Render(bar)
	} else {
		progressStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
		bar = progressStyle.Render(bar)
	}

	if p.ShowPct {
		pctStr := fmt.Sprintf("%d%%", int(math.Round(pct*100)))
		pctStyle := lipgloss.NewStyle().
			Foreground(t.TextSecondary)
		bar = bar + " " + pctStyle.Render(pctStr)
	}

	return bar
}

// SegmentedBar renders a bar segmented by different states.
type SegmentedBar struct {
	Segments []Segment
	Width    int
	Theme    theme.Theme
}

// Segment represents a portion of a segmented bar.
type Segment struct {
	Count int
	Color lipgloss.Color
	Label string
}

// Render returns the segmented bar as a string.
func (s SegmentedBar) Render() string {
	if s.Width <= 0 {
		s.Width = 40
	}

	total := 0
	for _, seg := range s.Segments {
		total += seg.Count
	}

	if total == 0 {
		return strings.Repeat("░", s.Width)
	}

	var bar strings.Builder
	remaining := s.Width

	for _, seg := range s.Segments {
		if remaining <= 0 {
			break
		}

		width := int(math.Round(float64(seg.Count) / float64(total) * float64(s.Width)))
		if width > remaining {
			width = remaining
		}

		style := lipgloss.NewStyle().Foreground(seg.Color)
		bar.WriteString(style.Render(strings.Repeat("█", width)))
		remaining -= width
	}

	// Fill remaining with background
	if remaining > 0 {
		t := s.Theme
		if t.Text == "" {
			t = theme.Default()
		}
		bar.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("░", remaining)))
	}

	return bar.String()
}

// AnimatedProgress provides a tick-driven progress bar fill animation.
// When progress advances, it smoothly ramps the displayed fill over 500ms.
// It also provides flash effects for completion (green) and failure (red).
type AnimatedProgress struct {
	Current   int               // actual current value
	Total     int               // total target value
	Displayed int               // animated display value (ramps from old→new)
	Animating bool              // true while animation is in progress
	Color     lipgloss.Color    // current bar color
	StartAt   time.Time         // timestamp when animation started
	fromVal   int               // displayed value at animation start
	toVal     int               // target value for current animation
}

// AnimatedProgressFlash tracks a transient color flash (e.g., green on completion).
type flashState struct {
	color     lipgloss.Color
	startedAt time.Time
	duration  time.Duration
	active    bool
}

// AnimatedProgressBar wraps AnimatedProgress with a render method.
type AnimatedProgressBar struct {
	Animated  AnimatedProgress
	Flash     flashState
	Width     int
	ShowPct   bool
	Theme     theme.Theme
}

// UpdateProgress starts an animation from oldCurrent to newCurrent.
func (p *AnimatedProgress) UpdateProgress(oldCurrent, newCurrent, total int) {
	if newCurrent > oldCurrent {
		p.fromVal = oldCurrent
		p.toVal = newCurrent
		p.Animating = true
		p.StartAt = time.Now()
	}
	p.Current = newCurrent
	p.Total = total
}

// Tick advances the animation by one frame. Call from Update() on each tick.
// Returns true when the animation is complete (reached target or timed out).
func (p *AnimatedProgress) Tick() bool {
	if !p.Animating {
		return true
	}
	elapsed := time.Since(p.StartAt)
	pct := math.Min(float64(elapsed)/float64(animationRampDuration), 1.0)
	diff := p.toVal - p.fromVal
	p.Displayed = p.fromVal + int(math.Round(float64(diff)*pct))
	if pct >= 1.0 {
		p.Displayed = p.toVal
		p.Animating = false
		return true
	}
	return false
}

// Progress returns the animated progress as a 0.0–1.0 float.
func (p *AnimatedProgress) Progress() float64 {
	if p.Total <= 0 {
		return 0
	}
	return float64(p.Displayed) / float64(p.Total)
}

// animationRampDuration is the duration of the progress bar fill animation.
const animationRampDuration = 500 * time.Millisecond

// flashDuration is the duration of a completion/failure flash.
const flashDuration = 300 * time.Millisecond

// StartFlash begins a transient color flash on the progress bar.
func (f *flashState) StartFlash(color lipgloss.Color) {
	f.color = color
	f.startedAt = time.Now()
	f.duration = flashDuration
	f.active = true
}

// Tick advances the flash animation. Returns true when the flash has expired.
func (f *flashState) Tick() bool {
	if !f.active {
		return true
	}
	if time.Since(f.startedAt) >= f.duration {
		f.active = false
		return true
	}
	return false
}

// Render renders the animated progress bar as a string.
func (b *AnimatedProgressBar) Render() string {
	t := b.Theme
	if t.Text == "" {
		t = theme.Default()
	}
	w := b.Width
	if w <= 0 {
		w = 30
	}

	pct := b.Animated.Progress()
	filledWidth := int(math.Round(pct * float64(w)))
	if filledWidth > w {
		filledWidth = w
	}
	if filledWidth < 0 {
		filledWidth = 0
	}
	emptyWidth := w - filledWidth

	// Determine bar color
	var barColor lipgloss.Color
	if b.Flash.active {
		barColor = b.Flash.color
	} else if pct >= 1.0 {
		barColor = t.Success
	} else {
		barColor = t.Brand
	}

	filled := strings.Repeat("█", filledWidth)
	empty := strings.Repeat("░", emptyWidth)
	bar := lipgloss.NewStyle().Foreground(barColor).Render(filled) +
		lipgloss.NewStyle().Foreground(t.Border).Render(empty)

	if b.ShowPct {
		pctStr := fmt.Sprintf("%d%%", int(math.Round(pct*100)))
		pctStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
		bar = bar + " " + pctStyle.Render(pctStr)
	}

	return bar
}

// CompactProgress renders a compact inline progress indicator.
type CompactProgress struct {
	Progress  float64
	ShowChars bool // show unicode progress chars
}

// Render returns a compact progress string.
func (c CompactProgress) Render() string {
	pct := c.Progress
	if pct < 0 {
		pct = 0
	}
	if pct > 1 {
		pct = 1
	}

	if c.ShowChars {
		// Show progress as fraction of 10 blocks
		blocks := int(math.Round(pct * 10))
		filled := strings.Repeat("█", blocks)
		empty := strings.Repeat("░", 10-blocks)
		return filled + empty
	}

	return fmt.Sprintf("[%d%%]", int(math.Round(pct*100)))
}
