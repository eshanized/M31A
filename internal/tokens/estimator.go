package tokens

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/pkoukk/tiktoken-go"
)

// DefaultWarningThreshold is the default context usage ratio that triggers
// the context warning banner (80%).
const DefaultWarningThreshold = 0.80

// Estimator provides token counting and context usage estimation for a
// specific model. Uses tiktoken-go for supported OpenAI models and a rune-based
// fallback for unsupported models (Claude, OpenRouter aliases, etc.).
// EMA calibration corrects estimates against actual usage from API responses.
type Estimator struct {
	modelID   string
	tokenizer *tiktoken.Tiktoken // nil if model unsupported by tiktoken-go
	emaAlpha  float64
	emaFactor float64 // running calibration factor, starts at 1.0
}

// NewEstimator creates an Estimator for the given model ID.
// If the model is not supported by tiktoken-go, the tokenizer is set to nil
// and the rune-based fallback is used for estimation.
func NewEstimator(modelID string) *Estimator {
	e := &Estimator{
		modelID:   modelID,
		emaAlpha:  0.3,
		emaFactor: 1.0,
	}

	tkm, err := tiktoken.EncodingForModel(modelID)
	if err == nil {
		e.tokenizer = tkm
	}

	return e
}

// Estimate returns the estimated token count for the given text.
// Uses tiktoken-go if the model is supported; otherwise falls back to
// len([]rune(text)) / 4 * 1.3. The emaFactor calibration is applied
// to all estimates.
func (e *Estimator) Estimate(text string) int {
	var estimated int

	if e.tokenizer != nil {
		estimated = len(e.tokenizer.Encode(text, nil, nil))
	} else {
		// Fallback for unsupported models (Claude, etc.)
		// Use rune count for multibyte character handling
		estimated = int(float64(len([]rune(text))) / 4 * 1.3)
	}

	// Apply EMA calibration factor
	return int(float64(estimated) * e.emaFactor)
}

// Calibrate updates the emaFactor using exponential moving average.
// ratio = actual / estimated
// newFactor = emaAlpha * ratio + (1 - emaAlpha) * previousFactor
// The factor is clamped to [0.1, 10.0] to prevent extreme values.
func (e *Estimator) Calibrate(estimated, actual int) {
	if estimated <= 0 {
		return // avoid division by zero
	}

	ratio := float64(actual) / float64(estimated)
	e.emaFactor = e.emaAlpha*ratio + (1-e.emaAlpha)*e.emaFactor

	// Clamp to prevent extreme values
	if e.emaFactor < 0.1 {
		e.emaFactor = 0.1
	}
	if e.emaFactor > 10.0 {
		e.emaFactor = 10.0
	}
}

// FormatUsage returns a formatted string showing used/total context with
// percentage: "used / total (XX%)".
// If total is 0 or negative, returns "-- / --".
func (e *Estimator) FormatUsage(used int, total int64) string {
	if total <= 0 {
		return "-- / --"
	}

	percent := int(float64(used) / float64(total) * 100)
	return fmt.Sprintf("%d / %d (%d%%)", used, total, percent)
}

// ContextWarningBanner returns a styled warning string when context usage
// exceeds the threshold. Returns empty string if usage is below the threshold
// or total is 0.
//
// The returned string uses lipgloss with yellow background (FDD663) and
// black foreground for visibility.
func (e *Estimator) ContextWarningBanner(used int, total int64, threshold float64) string {
	if total <= 0 {
		return ""
	}

	ratio := float64(used) / float64(total)
	if ratio < threshold {
		return ""
	}

	percent := int(ratio * 100)

	warning := lipgloss.NewStyle().
		Background(lipgloss.Color("#FDD663")).
		Foreground(lipgloss.Color("#000000")).
		Render(fmt.Sprintf("⚠ Context at %d%% — consider using /compress", percent))

	return warning
}

// ModelID returns the model identifier for this estimator.
func (e *Estimator) ModelID() string {
	return e.modelID
}
