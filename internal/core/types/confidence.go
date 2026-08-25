package types

import "fmt"

// Confidence is the shared three-level certainty vocabulary for all
// intelligence commands (explain, investigate, deps) per decision D-07:
// verified means machine-checked evidence directly supports the claim,
// likely means strong circumstantial evidence, and speculative means weak
// or contradictory evidence. It is a single vocabulary so future TUI
// badges and --format json output stay consistent across commands.
type Confidence string

const (
	// ConfidenceVerified indicates machine-checked evidence directly supports the claim.
	ConfidenceVerified Confidence = "verified"
	// ConfidenceLikely indicates strong circumstantial evidence supports the claim.
	ConfidenceLikely Confidence = "likely"
	// ConfidenceSpeculative indicates weak or contradictory evidence.
	ConfidenceSpeculative Confidence = "speculative"
)

// ParseConfidence converts s into a Confidence value. It rejects any string
// outside the {verified, likely, speculative} vocabulary with an error naming
// the valid values.
func ParseConfidence(s string) (Confidence, error) {
	switch Confidence(s) {
	case ConfidenceVerified:
		return ConfidenceVerified, nil
	case ConfidenceLikely:
		return ConfidenceLikely, nil
	case ConfidenceSpeculative:
		return ConfidenceSpeculative, nil
	default:
		return "", fmt.Errorf("invalid confidence %q: must be one of verified, likely, speculative", s)
	}
}

// Valid reports whether c is one of the three defined confidence levels.
func (c Confidence) Valid() bool {
	switch c {
	case ConfidenceVerified, ConfidenceLikely, ConfidenceSpeculative:
		return true
	default:
		return false
	}
}
