package components

import (
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// SparklineChars are the Unicode block characters for sparklines.
var SparklineChars = []rune{'▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'}

// BrailleChars are Unicode braille patterns for low-density sparklines.
var BrailleChars = []rune{'⠀', '⠁', '⠃', '⠇', '⠏', '⠟', '⠿', '⣿'}

// RenderSparkline renders a sparkline from float64 data using braille characters
// for low-density data and block elements for high-density data.
// It normalizes data to 0-1 range, maps to character density, and renders
// using theme.Brand for filled cells and theme.TextMuted for empty cells.
// Returns a single-line string of width characters.
func RenderSparkline(data []float64, width int, t theme.Theme) string {
	if width <= 0 || len(data) == 0 {
		return ""
	}

	// Cap data at 100 points for performance (threat T-24-02)
	if len(data) > 100 {
		data = data[len(data)-100:]
	}

	// Choose character set based on data density
	var chars []rune
	if len(data) <= width/2 {
		chars = BrailleChars
	} else {
		chars = SparklineChars
	}

	// Resample data to fit width
	resampled := resampleFloat64(data, width)

	// Find min/max for normalization
	minVal, maxVal := resampled[0], resampled[0]
	for _, v := range resampled {
		if v < minVal {
			minVal = v
		}
		if v > maxVal {
			maxVal = v
		}
	}

	// Build sparkline
	filledStyle := lipgloss.NewStyle().Foreground(t.Brand)
	emptyStyle := lipgloss.NewStyle().Foreground(t.TextMuted)

	var b strings.Builder
	for _, v := range resampled {
		var idx int
		if maxVal > minVal {
			idx = int((v - minVal) / (maxVal - minVal) * float64(len(chars)-1))
			if idx >= len(chars) {
				idx = len(chars) - 1
			}
		}

		if idx > 0 {
			b.WriteString(filledStyle.Render(string(chars[idx])))
		} else {
			b.WriteString(emptyStyle.Render(string(chars[0])))
		}
	}

	return b.String()
}

// resampleFloat64 reduces float64 values to target length by averaging.
func resampleFloat64(values []float64, target int) []float64 {
	if target <= 0 {
		return values
	}

	result := make([]float64, target)
	step := float64(len(values)) / float64(target)

	for i := 0; i < target; i++ {
		start := int(float64(i) * step)
		end := int(float64(i+1) * step)
		if end > len(values) {
			end = len(values)
		}

		sum := 0.0
		count := 0
		for j := start; j < end; j++ {
			sum += values[j]
			count++
		}
		if count > 0 {
			result[i] = sum / float64(count)
		}
	}

	return result
}

// Sparkline renders a sparkline of values using Unicode block characters.
type Sparkline struct {
	Values []int
	Width  int // if 0, uses len(Values)
	Label  string
	Theme  theme.Theme
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
		t := s.Theme
		if t.Text == "" {
			t = theme.Default()
		}
		labelStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
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
