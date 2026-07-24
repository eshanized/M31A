package tokens

import (
	"fmt"
	"math"
	"strings"
	"sync/atomic"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/pkoukk/tiktoken-go"
)

// DefaultWarningThreshold is the default context usage ratio that triggers
// the context warning banner (80%).
const DefaultWarningThreshold = 0.80

// ProviderFamily identifies a group of models that share similar tokenization.
type ProviderFamily int

const (
	ProviderUnknown ProviderFamily = iota
	ProviderOpenAI
	ProviderAnthropic
	ProviderGoogle
	ProviderMeta
	ProviderMistral
	ProviderQwen
	ProviderDeepSeek
	ProviderCohere
)

// String returns the provider family name.
func (p ProviderFamily) String() string {
	switch p {
	case ProviderOpenAI:
		return "openai"
	case ProviderAnthropic:
		return "anthropic"
	case ProviderGoogle:
		return "google"
	case ProviderMeta:
		return "meta"
	case ProviderMistral:
		return "mistral"
	case ProviderQwen:
		return "qwen"
	case ProviderDeepSeek:
		return "deepseek"
	case ProviderCohere:
		return "cohere"
	default:
		return "unknown"
	}
}

// DetectProviderFamily identifies the provider family from a model ID.
func DetectProviderFamily(modelID string) ProviderFamily {
	id := strings.ToLower(modelID)

	switch {
	case strings.Contains(id, "gpt-") || strings.Contains(id, "o1") || strings.Contains(id, "o3") || strings.Contains(id, "o4"):
		return ProviderOpenAI
	case strings.Contains(id, "claude"):
		return ProviderAnthropic
	case strings.Contains(id, "gemini"):
		return ProviderGoogle
	case strings.Contains(id, "llama"):
		return ProviderMeta
	case strings.Contains(id, "mistral"):
		return ProviderMistral
	case strings.Contains(id, "qwen"):
		return ProviderQwen
	case strings.Contains(id, "deepseek"):
		return ProviderDeepSeek
	case strings.Contains(id, "command"):
		return ProviderCohere
	default:
		return ProviderUnknown
	}
}

// Estimator provides token counting and context usage estimation for a
// specific model. Uses tiktoken-go for supported OpenAI models and provider-
// specific approximations for other providers. Falls back to rune-based
// counting for unknown providers.
// EMA calibration corrects estimates against actual usage from API responses.
type Estimator struct {
	modelID       string
	provider      ProviderFamily
	tokenizer     *tiktoken.Tiktoken
	emaAlpha      float64
	emaFactorBits atomic.Uint64
}

// NewEstimator creates an Estimator for the given model ID.
// If the model is not supported by tiktoken-go, the tokenizer is set to nil
// and provider-specific or rune-based fallback is used for estimation.
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
	provider := DetectProviderFamily(modelID)
	e := &Estimator{
		modelID:  modelID,
		provider: provider,
		emaAlpha: alpha,
	}
	e.emaFactorBits.Store(math.Float64bits(1.0))

	// Only load tiktoken for OpenAI models
	if provider == ProviderOpenAI {
		tkm, err := tiktoken.EncodingForModel(modelID)
		if err == nil {
			e.tokenizer = tkm
		}
	}

	return e
}

// isCodeHeavy returns true when the text appears to be primarily code rather
// than natural language. Code tokenizes differently across providers because
// identifiers, operators, and whitespace carry different information density.
func isCodeHeavy(text string) bool {
	if len(text) < 20 {
		return false
	}
	// Simple heuristic: code tends to have high density of structural characters
	// (braces, parens, semicolons, colons, brackets) relative to word count.
	// Natural language has ~1 punctuation mark per 50+ words; code has many per line.
	structural := 0
	words := 0
	inWord := false
	for _, r := range text {
		switch {
		case r >= 'a' && r <= 'z', r >= 'A' && r <= 'Z', r >= '0' && r <= '9', r == '_':
			if !inWord {
				words++
				inWord = true
			}
		default:
			inWord = false
			switch r {
			case '{', '}', '(', ')', ';', '=', '[', ']', ':', ',', '<', '>', '!', '*', '#', '/', '|', '&', '^', '%', '~', '@', '$':
				structural++
			}
		}
	}
	if words == 0 {
		return false
	}
	// Code typically has >0.5 structural chars per word; prose has <0.1.
	return float64(structural)/float64(words) > 0.5
}

// estimateWithProvider applies provider-specific tokenization heuristics.
// These are approximations calibrated to each provider's known tokenizer
// characteristics. Code and natural language tokenize at different rates
// because code tokens tend to be shorter (more unique symbols).
func (e *Estimator) estimateWithProvider(text string) int {
	chars := len(text)
	words := strings.Fields(text)
	wordCount := len(words)

	// Use char-length for byte-oriented tokenizers (most BPE), rune count
	// only for rune-heavy text (CJK, emoji).
	// The threshold is 50 chars to avoid division-by-zero sensitivity on
	// very short strings where word count is more reliable.
	useCharLength := chars > 50

	code := isCodeHeavy(text)

	switch e.provider {
	case ProviderAnthropic:
		// Claude uses a byte-level BPE tokenizer.
		// English prose: ~3.8 chars/token. Code: ~3.0 chars/token.
		if useCharLength {
			ratio := 3.8
			if code {
				ratio = 3.0
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*3/2, 1)

	case ProviderGoogle:
		// Gemini uses SentencePiece with a large vocabulary.
		// English prose: ~3.5 chars/token. Code: ~2.8 chars/token.
		if useCharLength {
			ratio := 3.5
			if code {
				ratio = 2.8
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*3/2, 1)

	case ProviderMeta:
		// Llama models use SentencePiece BPE with a 32K vocabulary.
		// English prose: ~4.0 chars/token. Code: ~3.5 chars/token.
		if useCharLength {
			ratio := 4.0
			if code {
				ratio = 3.5
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*3/2, 1)

	case ProviderMistral:
		// Mistral uses a BPE tokenizer with a large vocabulary (~32K).
		// English prose: ~4.0 chars/token. Code: ~3.0 chars/token.
		if useCharLength {
			ratio := 4.0
			if code {
				ratio = 3.0
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*3/2, 1)

	case ProviderQwen:
		// Qwen uses a BPE tokenizer with extensive multilingual support.
		// The vocabulary is larger and includes many CJK merges.
		// English prose: ~3.0 chars/token. Code: ~2.5 chars/token.
		if useCharLength {
			ratio := 3.0
			if code {
				ratio = 2.5
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*4/3, 1)

	case ProviderDeepSeek:
		// DeepSeek uses a BPE tokenizer similar to Llama.
		// English prose: ~4.0 chars/token. Code: ~3.5 chars/token.
		if useCharLength {
			ratio := 4.0
			if code {
				ratio = 3.5
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*3/2, 1)

	case ProviderCohere:
		// Cohere uses a BPE tokenizer optimized for multilingual text.
		// English prose: ~4.0 chars/token. Code: ~3.0 chars/token.
		if useCharLength {
			ratio := 4.0
			if code {
				ratio = 3.0
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(wordCount*3/2, 1)

	default:
		// Unknown provider: use a conservative fallback.
		// ~3.8 chars/token for prose, slightly tighter for code.
		if useCharLength {
			ratio := 3.8
			if code {
				ratio = 3.0
			}
			return int(math.Ceil(float64(chars) / ratio))
		}
		return max(int(float64(wordCount)*1.5), 1)
	}
}

// Estimate returns the estimated token count for the given text.
// Uses tiktoken-go for OpenAI models, provider-specific heuristics for
// known providers, and rune-based fallback for unknown models.
// The emaFactor calibration is applied to all estimates.
func (e *Estimator) Estimate(text string) int {
	var estimated int

	if e.tokenizer != nil {
		estimated = len(e.tokenizer.Encode(text, nil, nil))
	} else {
		estimated = e.estimateWithProvider(text)
	}

	factor := math.Float64frombits(e.emaFactorBits.Load())
	return int(float64(estimated) * factor)
}

// EstimateTokensForProvider returns the estimated token count and the provider
// family name used for estimation. This is useful for diagnostics and fallback
// reporting.
func (e *Estimator) EstimateTokensForProvider(text string) (int, string) {
	count := e.Estimate(text)
	return count, e.provider.String()
}

// Calibrate updates the emaFactor using exponential moving average.
// ratio = actual / estimated
// newFactor = emaAlpha * ratio + (1 - emaAlpha) * previousFactor
// The factor is clamped to [0.1, 10.0] to prevent extreme values.
// Uses lock-free atomic CAS for concurrent safety.
func (e *Estimator) Calibrate(estimated, actual int) {
	if estimated <= 0 {
		return
	}

	ratio := float64(actual) / float64(estimated)

	for {
		oldBits := e.emaFactorBits.Load()
		oldFactor := math.Float64frombits(oldBits)
		newFactor := e.emaAlpha*ratio + (1-e.emaAlpha)*oldFactor
		if newFactor < 0.1 {
			newFactor = 0.1
		}
		if newFactor > 10.0 {
			newFactor = 10.0
		}
		newBits := math.Float64bits(newFactor)
		if e.emaFactorBits.CompareAndSwap(oldBits, newBits) {
			return
		}
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

// Provider returns the detected provider family for this estimator.
func (e *Estimator) Provider() ProviderFamily {
	return e.provider
}

// emaFactor returns the current calibration factor. Exported for testing only.
func (e *Estimator) emaFactor() float64 {
	return math.Float64frombits(e.emaFactorBits.Load())
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
