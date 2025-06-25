package components

import (
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// SparklineChars are the Unicode block characters for sparklines.
var SparklineChars = []rune{'▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'}

// Sparkline renders a sparkline of values using Unicode block characters.
type Sparkline struct {
	Values []int
	Width  int // if 0, uses len(Values)
	Label  string
}

// Render returns the sparkline as a string.
func (s Sparkline) Render() string {
	if len(s.Values) == 0 {
		return ""
	}

	width := s.Width
	if width == 0 {
		width = len(s.Values)
	}

	// Resample values to fit width if needed
	sampled := s.Values
	if len(s.Values) > width {
		sampled = resample(s.Values, width)
	} else if len(s.Values) < width {
		// Pad with zeros on the left
		padding := width - len(s.Values)
		sampled = make([]int, width)
		copy(sampled[padding:], s.Values)
	}

	// Find min/max for scaling
	minVal := sampled[0]
	maxVal := sampled[0]
	for _, v := range sampled {
		if v < minVal {
			minVal = v
		}
		if v > maxVal {
			maxVal = v
		}
	}

	// Build sparkline
	var b strings.Builder
	for _, v := range sampled {
		if maxVal == minVal {
			b.WriteRune(SparklineChars[0])
		} else {
			idx := int(float64(v-minVal) / float64(maxVal-minVal) * float64(len(SparklineChars)-1))
			idx = int(math.Min(float64(idx), float64(len(SparklineChars)-1)))
			b.WriteRune(SparklineChars[idx])
		}
	}

	result := b.String()

	if s.Label != "" {
		labelStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#9AA0A6"))
		result = result + " " + labelStyle.Render(s.Label)
	}

	return result
}

// resample reduces values to target length by averaging.
func resample(values []int, target int) []int {
	if target <= 0 {
		return values
	}

	result := make([]int, target)
	step := float64(len(values)) / float64(target)

	for i := 0; i < target; i++ {
		start := int(float64(i) * step)
		end := int(float64(i+1) * step)
		if end > len(values) {
			end = len(values)
		}

		sum := 0
		count := 0
		for j := start; j < end; j++ {
			sum += values[j]
			count++
		}
		if count > 0 {
			result[i] = sum / count
		}
	}

	return result
}

// BarChart renders a horizontal bar chart using block characters.
type BarChart struct {
	Values    []int
	Labels    []string
	MaxWidth  int
	MaxValue  int // if 0, calculated from values
	ColorFunc func(int) string
}

// Render returns the bar chart as a string.
func (b BarChart) Render() string {
	if len(b.Values) == 0 || len(b.Labels) != len(b.Values) {
		return ""
	}

	maxWidth := b.MaxWidth
	if maxWidth == 0 {
		maxWidth = 40
	}

	maxValue := b.MaxValue
	if maxValue == 0 {
		maxValue = b.Values[0]
		for _, v := range b.Values {
			if v > maxValue {
				maxValue = v
			}
		}
	}

	var lines []string
	for i, v := range b.Values {
		barLen := 0
		if maxValue > 0 {
			barLen = int(float64(v) / float64(maxValue) * float64(maxWidth))
		}

		bar := strings.Repeat("█", barLen)

		if b.ColorFunc != nil {
			bar = b.ColorFunc(i)
			// Note: actual coloring requires lipgloss, keeping simple for now
		}

		line := b.Labels[i] + " " + bar
		lines = append(lines, line)
	}

	return strings.Join(lines, "\n")
}
