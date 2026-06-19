package tui

import (
	"strconv"
	"strings"
)

// RenderDiff performs incremental rendering by comparing frames and emitting
// only changed lines using ANSI cursor positioning.
type RenderDiff struct {
	previousFrame string
	currentWidth  int
	currentHeight int
}

// NewRenderDiff creates a new RenderDiff tracker.
func NewRenderDiff() *RenderDiff {
	return &RenderDiff{}
}

// Update compares the new frame with the previous one and returns an optimized
// string with ANSI escape codes to update only changed lines.
// If the frame dimensions changed, it returns the full frame.
func (d *RenderDiff) Update(newFrame string, width, height int) string {
	if d.previousFrame == "" || d.currentWidth != width || d.currentHeight != height {
		d.previousFrame = newFrame
		d.currentWidth = width
		d.currentHeight = height
		return newFrame
	}

	oldLines := strings.Split(d.previousFrame, "\n")
	newLines := strings.Split(newFrame, "\n")

	var b strings.Builder
	changed := false

	maxLines := len(newLines)
	if len(oldLines) > maxLines {
		maxLines = len(oldLines)
	}

	for i := 0; i < maxLines; i++ {
		oldLine := ""
		newLine := ""
		if i < len(oldLines) {
			oldLine = oldLines[i]
		}
		if i < len(newLines) {
			newLine = newLines[i]
		}

		if oldLine != newLine {
			changed = true
			// Move cursor to line i+1 (1-indexed), column 1
			b.WriteString("\x1b[" + strconv.Itoa(i+1) + ";1H")
			// Clear the line
			b.WriteString("\x1b[2K")
			// Write new content
			b.WriteString(newLine)
		}
	}

	d.previousFrame = newFrame

	if !changed {
		return ""
	}

	// Reset cursor to bottom
	b.WriteString("\x1b[" + strconv.Itoa(height) + ";1H")
	return b.String()
}

// Reset clears the previous frame, forcing a full redraw on next Update.
func (d *RenderDiff) Reset() {
	d.previousFrame = ""
}
