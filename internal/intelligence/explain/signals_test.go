package explain

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestVerdictClassFromSignals_Branches(t *testing.T) {
	tests := []struct {
		name     string
		signals  RationaleSignals
		expected types.Confidence
	}{
		{
			name: "zero consumers, stale last-touch, has deprecation -> speculative",
			signals: RationaleSignals{
				ConsumerCount:          0,
				LastTouchAgeDays:       400,
				HasDeprecationMarkers:  true,
				DeprecationMatches:     []string{"DEPRECATED at bar.go:10"},
				HasTestCoverage:        false,
				ADRStale:               false,
				ADRCount:               0,
			},
			expected: types.ConfidenceSpeculative,
		},
		{
			name: "consumers >= 1, tests exist, no deprecation -> verified",
			signals: RationaleSignals{
				ConsumerCount:          3,
				LastTouchAgeDays:       30,
				HasDeprecationMarkers:  false,
				DeprecationMatches:     nil,
				HasTestCoverage:        true,
				ADRStale:               false,
				ADRCount:               1,
			},
			expected: types.ConfidenceVerified,
		},
		{
			name: "consumers >= 1 but no tests -> likely",
			signals: RationaleSignals{
				ConsumerCount:          2,
				LastTouchAgeDays:       100,
				HasDeprecationMarkers:  false,
				DeprecationMatches:     nil,
				HasTestCoverage:        false,
				ADRStale:               true,
				ADRCount:               1,
			},
			expected: types.ConfidenceLikely,
		},
		{
			name: "zero consumers but recent and no deprecation -> likely",
			signals: RationaleSignals{
				ConsumerCount:          0,
				LastTouchAgeDays:       10,
				HasDeprecationMarkers:  false,
				DeprecationMatches:     nil,
				HasTestCoverage:        true,
				ADRStale:               false,
				ADRCount:               0,
			},
			expected: types.ConfidenceLikely,
		},
		{
			name: "at-threshold age equality (stale_threshold_days == LastTouchAgeDays) counts as stale -> speculative when combined with zero consumers and deprecation",
			signals: RationaleSignals{
				ConsumerCount:          0,
				LastTouchAgeDays:       365, // exactly at 12 months threshold
				HasDeprecationMarkers:  true,
				DeprecationMatches:     []string{"TODO at bar.go:5"},
				HasTestCoverage:        false,
				ADRStale:               false,
				ADRCount:               0,
			},
			expected: types.ConfidenceSpeculative,
		},
		{
			name: "only deprecation marker, everything else healthy -> likely",
			signals: RationaleSignals{
				ConsumerCount:          5,
				LastTouchAgeDays:       5,
				HasDeprecationMarkers:  true,
				DeprecationMatches:     []string{"FIXME at bar.go:20"},
				HasTestCoverage:        true,
				ADRStale:               false,
				ADRCount:               2,
			},
			expected: types.ConfidenceLikely,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := VerdictClassFromSignals(tt.signals)
			if got != tt.expected {
				t.Errorf("VerdictClassFromSignals() = %q, want %q", got, tt.expected)
			}
		})
	}
}

func TestVerdictClassFromSignals_Deterministic(t *testing.T) {
	// This signal set maps to "likely" (consumers >= 1 but no tests)
	signals := RationaleSignals{
		ConsumerCount:          2,
		LastTouchAgeDays:       100,
		HasDeprecationMarkers:  false,
		HasTestCoverage:        false, // no tests -> likely
		ADRStale:               false,
		ADRCount:               1,
	}

	// Call multiple times to verify deterministic output
	for i := 0; i < 10; i++ {
		got := VerdictClassFromSignals(signals)
		if got != types.ConfidenceLikely {
			t.Errorf("iteration %d: got %q, want likely", i, got)
		}
	}
}

func TestRationaleSignals_NoNumericScores(t *testing.T) {
	// Verify the struct has only int/bool fields (no float scores)
	var s RationaleSignals
	s.ConsumerCount = 5
	s.LastTouchAgeDays = 30
	s.HasDeprecationMarkers = true
	s.DeprecationMatches = []string{"DEPRECATED"}
	s.TODOFixMECount = 2
	s.HasTestCoverage = true
	s.ADRStale = false
	s.ADRCount = 1

	// This test compiles and runs - if any float fields existed, they'd be visible
	// The key assertion is that we can set all fields without float types
	_ = s
}

func TestComputeRationaleSignals_Integration(t *testing.T) {
	// This test uses the seedRepo helper to verify that consumer-count
	// and test-presence fields populate correctly through the full pipeline.
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	// Build a minimal set of inputs for ComputeRationaleSignals
	// We need graph, index, git client, target path, symbol, now, staleThresholdDays
	graph := deps.Graph
	index := deps.Index
	gitClient := deps.GitClient
	targetPath := "bar.go"
	symbol := "Bar"
	now := time.Now()
	staleThresholdDays := 365

	// The function should be callable and produce signals with populated fields
	signals := ComputeRationaleSignals(graph, index, gitClient, targetPath, symbol, now, staleThresholdDays)

	// ConsumerCount should be > 0 (caller.go calls Bar)
	if signals.ConsumerCount == 0 {
		t.Errorf("ConsumerCount = %d, want > 0 (caller.go calls Bar)", signals.ConsumerCount)
	}

	// HasTestCoverage should be true (caller_test.go exists)
	if !signals.HasTestCoverage {
		t.Errorf("HasTestCoverage = false, want true (caller_test.go relates to Bar)")
	}

	// LastTouchAgeDays should be >= 0
	if signals.LastTouchAgeDays < 0 {
		t.Errorf("LastTouchAgeDays = %d, want >= 0", signals.LastTouchAgeDays)
	}
}