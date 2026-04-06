package tokens

import (
	"fmt"
	"sync"
	"unicode/utf8"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
	"github.com/pkoukk/tiktoken-go"
)

// NOTE: tiktoken-go is unmaintained since 2024. New tokenizers (e.g. o200k_base
// for GPT-4o) may not be recognized, causing silent fallback to rune counting.
// Monitor for a maintained fork or official replacement.

// DefaultWarningThreshold is the default context usage ratio that triggers
// the context warning banner (80%).
const DefaultWarningThreshold = 0.80

// Estimator provides token counting and context usage estimation for a
// specific model. Uses tiktoken-go for supported OpenAI models and a rune-based
// fallback for unsupported models (Claude, OpenRouter aliases, etc.).
// EMA calibration corrects estimates against actual usage from API responses.
type Estimator struct {
	modelID   string
	tokenizer *tiktoken.Tiktoken
	mu        sync.Mutex
	emaAlpha  float64
	emaFactor float64
}

// NewEstimator creates an Estimator for the given model ID.
// If the model is not supported by tiktoken-go, the tokenizer is set to nil
// and the rune-based fallback is used for estimation.
func NewEstimator(modelID string) *Estimator {
	return NewEstimatorWithOpts(modelID, EstimatorOpts{})
}

// EstimatorOpts holds optional settings for the estimator.
type EstimatorOpts struct {
	EMAAlpha float64 // EMA calibration rate (0.0–1.0). 0 means default (0.3).
}

// NewEstimatorWithOpts creates an Estimator with explicit options.
func NewEstimatorWithOpts(modelID string, opts EstimatorOpts) *Estimator {
	alpha := opts.EMAAlpha
	if alpha <= 0 || alpha > 1 {
		alpha = types.EMACorrectionAlpha
	}
	e := &Estimator{
		modelID:   modelID,
		emaAlpha:  alpha,
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
// utf8.RuneCountInString(text) / 4 * 1.3. The emaFactor calibration is applied
// to all estimates.
func (e *Estimator) Estimate(text string) int {
	var estimated int

	if e.tokenizer != nil {
		estimated = len(e.tokenizer.Encode(text, nil, nil))
	} else {
		// Use utf8.RuneCountInString which is O(N) time but O(1) space
		// instead of len([]rune(text)) which allocates O(N) memory
		estimated = int((float64(utf8.RuneCountInString(text))/4.0 + 1.0) * 1.3)
	}

	e.mu.Lock()
	factor := e.emaFactor
	e.mu.Unlock()
	return int(float64(estimated) * factor)
}

// Calibrate updates the emaFactor using exponential moving average.
// ratio = actual / estimated
// newFactor = emaAlpha * ratio + (1 - emaAlpha) * previousFactor
// The factor is clamped to [0.1, 10.0] to prevent extreme values.
func (e *Estimator) Calibrate(estimated, actual int) {
	if estimated <= 0 {
		return
	}

	ratio := float64(actual) / float64(estimated)

	e.mu.Lock()
	e.emaFactor = e.emaAlpha*ratio + (1-e.emaAlpha)*e.emaFactor
	if e.emaFactor < 0.1 {
		e.emaFactor = 0.1
	}
	if e.emaFactor > 10.0 {
		e.emaFactor = 10.0
	}
	e.mu.Unlock()
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
	remaining := int(total) - used

	warning := lipgloss.NewStyle().
		Background(lipgloss.Color("#FDD663")).
		Foreground(lipgloss.Color("#000000")).
		Render(fmt.Sprintf("⚠ Context at %d%% — %d tokens remaining — consider using /compress", percent, remaining))

	return warning
}

// ModelID returns the model identifier for this estimator.
func (e *Estimator) ModelID() string {
	return e.modelID
}

// EstimateMessages returns the estimated total token count for a slice of
// messages. Accounts for:
//   - Content of each message
//   - Per-message role overhead (~4 tokens for role/separator metadata)
//   - Tool call input JSON (when present)
//
// Without these, preflight context checks underestimate usage on tool-heavy
// conversations and may allow requests that exceed the model's window (BUG-29).
func (e *Estimator) EstimateMessages(messages []types.Message) int {
	const perMessageOverhead = 4
	total := 0
	for _, msg := range messages {
		total += perMessageOverhead
		total += e.Estimate(msg.Content)
		for _, tc := range msg.ToolCalls {
			if len(tc.Input) > 0 {
				total += e.Estimate(string(tc.Input))
			}
			total += e.Estimate(tc.Name)
		}
	}
	return total
}
