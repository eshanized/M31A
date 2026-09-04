package explain

import (
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
)

// RationaleSignals holds the deterministic signals used to compute the
// rationale-validity verdict class. All fields are populated from injected
// inputs — zero I/O inside the computation functions beyond reading deps.
type RationaleSignals struct {
	ConsumerCount         int      // number of direct callers from the call graph
	LastTouchAgeDays      int      // days since the last author-time commit touching the target
	HasDeprecationMarkers bool     // whether DEPRECATED/TODO/FIXME/XXX markers were found
	DeprecationMatches    []string // matched marker lines with file:line context
	TODOFixMECount        int      // count of TODO/FIXME/XXX markers found
	HasTestCoverage       bool     // whether AnalyzeImpact reports affected tests
	ADRStale              bool     // whether ADRs exist and are older than stale threshold
	ADRCount              int      // number of matching ADRs found
}

// markerKeywords are the case-insensitive substrings scanned for in source files.
var markerKeywords = []string{"DEPRECATED", "TODO", "FIXME", "XXX"}

// defaultStaleThresholdDays is the default staleness threshold (12 months).
// Matches the [intelligence] config default for stale_months.
const defaultStaleThresholdDays = 365

// ComputeRationaleSignals computes deterministic rationale signals from
// injected dependencies. It performs zero I/O beyond reading the provided
// in-memory dependencies and the target source file for marker scanning.
// All signals derive from:
//
// - ConsumerCount: graph.Callers(symbol) length
// - LastTouchAgeDays: max author-time from BlamePorcelain on targetPath
// - HasDeprecationMarkers/DeprecationMatches/TODOFixMECount: scanning target source lines
// - HasTestCoverage: AnalyzeImpact(graph, index, symbol, 1).AffectedTests non-empty
// - ADRStale/ADRCount: provided by caller (Task 2 wires this via ScanADRs)
func ComputeRationaleSignals(
	graph *codeintel.CodeGraph,
	index *codeintel.SymbolIndex,
	gitClient *git.Git,
	targetPath string,
	symbol string,
	now time.Time,
	staleThresholdDays int,
) RationaleSignals {
	var signals RationaleSignals

	if staleThresholdDays <= 0 {
		staleThresholdDays = defaultStaleThresholdDays
	}

	// ConsumerCount: number of direct callers from the call graph
	callers := graph.Callers(symbol)
	signals.ConsumerCount = len(callers)

	// LastTouchAgeDays: max author-time from blame on targetPath
	if gitClient != nil && targetPath != "" {
		_, commits, err := gitClient.BlamePorcelain(targetPath)
		if err == nil && len(commits) > 0 {
			var latest time.Time
			for _, c := range commits {
				if c.AuthorTime.After(latest) {
					latest = c.AuthorTime
				}
			}
			if !latest.IsZero() {
				signals.LastTouchAgeDays = int(now.Sub(latest).Hours() / 24)
			}
		}
	}

	// HasDeprecationMarkers / DeprecationMatches / TODOFixMECount:
	// scan target source lines for marker keywords (case-insensitive)
	if gitClient != nil && targetPath != "" {
		workDir := gitClient.WorkDir()
		if workDir != "" {
			absPath := filepath.Join(workDir, targetPath)
			content, err := os.ReadFile(absPath)
			if err == nil {
				hasMarkers, matches, count := scanMarkers(string(content), targetPath)
				signals.HasDeprecationMarkers = hasMarkers
				signals.DeprecationMatches = matches
				signals.TODOFixMECount = count
			}
		}
	}

	// HasTestCoverage: AnalyzeImpact reports affected tests
	impact := codeintel.AnalyzeImpact(graph, index, symbol, 1)
	signals.HasTestCoverage = len(impact.AffectedTests) > 0

	// ADR fields are populated by caller (Task 2 wires ScanADRs)
	// Default to zero values (ADRStale=false, ADRCount=0)

	return signals
}

// scanMarkers scans content for marker keywords.
// Returns (hasMarkers, matches, count).
func scanMarkers(content, filePath string) (bool, []string, int) {
	lines := strings.Split(content, "\n")
	var matches []string
	count := 0
	for i, line := range lines {
		upper := strings.ToUpper(line)
		for _, kw := range markerKeywords {
			if strings.Contains(upper, kw) {
				count++
				matches = append(matches, kw+" at "+filePath+":"+itoa(i+1))
				break // count each line once even if multiple keywords
			}
		}
	}
	return len(matches) > 0, matches, count
}

// itoa is a simple integer to string conversion to avoid importing strconv
// for this one use. In production, use strconv.Itoa.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	neg := n < 0
	if neg {
		n = -n
	}
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}

// VerdictClassFromSignals maps deterministic signals to the three-level
// Confidence enum. The mapping rules (per D-08):
//
// - verified: ConsumerCount >= 1 AND HasTestCoverage == true AND HasDeprecationMarkers == false
// - speculative: ConsumerCount == 0 AND LastTouchAgeDays >= staleThresholdDays AND HasDeprecationMarkers == true
// - likely: everything else
//
// Boundary convention: age equal to stale_threshold_days counts as stale (>=).
// Identical inputs always produce identical verdict class.
func VerdictClassFromSignals(s RationaleSignals) types.Confidence {
	// Verified: consumers exist, tests exist, no deprecation markers
	if s.ConsumerCount >= 1 && s.HasTestCoverage && !s.HasDeprecationMarkers {
		return types.ConfidenceVerified
	}

	// Speculative: zero consumers, stale last-touch, has deprecation markers
	isStale := s.LastTouchAgeDays >= defaultStaleThresholdDays

	if s.ConsumerCount == 0 && isStale && s.HasDeprecationMarkers {
		return types.ConfidenceSpeculative
	}

	// Everything else -> likely
	return types.ConfidenceLikely
}