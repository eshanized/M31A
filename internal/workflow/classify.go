package workflow

import (
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
)

// trivialIndicators are keywords/phrases suggesting a trivial, single-action goal.
var trivialIndicators = []string{
	"add ", "create ", "delete ", "remove ", "rename ",
	"fix ", "update ", "bump ", "pin ", "chore",
}

// complexIndicators are keywords/phrases suggesting a complex, multi-step goal.
var complexIndicators = []string{
	"build ", "implement ", "design ", "architect ", "refactor ",
	"migrate ", "integrate ", "feature", "pipeline", "microservice",
	"full stack", "full-stack", "end to end", "e2e",
	"authentication", "authorization", "database", "api",
	"multi-step", "multi phase", "multi-phase",
}

// codeComplexitySignals are terms that indicate non-trivial code work even when
// the goal is short. Their presence prevents a goal from being classified as
// trivial, since trivial mode skips Plan and Verify phases.
var codeComplexitySignals = []string{
	"race", "deadlock", "data race", "leak", "memory leak",
	"security", "auth", "oauth", "jwt", "encryption",
	"concurrent", "parallel", "goroutine", "mutex",
	"payment", "transaction", "rollback", "migration",
	"vulnerability", "cve", "ssrf", "xss", "sqli", "sql injection",
	"timeout", "retry", "circuit breaker", "backpressure",
	"async", "await", "promise", "channel",
	"cache", "invalidation", "consistency",
	"idempotent", "atomic", "distributed",
	"schema", "index", "query optimization",
	"websocket", "grpc", "protobuf",
	"middleware", "interceptor", "hook",
}

// ClassifyPrompt estimates the complexity of a user's goal using heuristics.
// Factors considered:
//   - Keyword/phrase matching
//   - Goal length and sentence count
//   - Number of existing project files (proxy for project size)
func ClassifyPrompt(goal string, workDir string) m31types.ComplexityLevel {
	lower := strings.ToLower(strings.TrimSpace(goal))
	words := strings.Fields(lower)

	// Count complex indicators first (they take precedence)
	complexScore := 0
	for _, ind := range complexIndicators {
		if strings.Contains(lower, ind) {
			complexScore++
		}
	}

	// Count trivial indicators
	trivialScore := 0
	for _, ind := range trivialIndicators {
		if strings.Contains(lower, ind) {
			trivialScore++
		}
	}

	// Multi-sentence or long goals are at least moderate
	sentenceCount := strings.Count(lower, ".") + strings.Count(lower, "\n")
	if sentenceCount == 0 && strings.Contains(lower, ",") {
		sentenceCount = 1
	}
	wordCount := len(words)

	// Heuristic: complex indicator + many words + multi-sentence => complex
	if complexScore >= 2 && wordCount > 20 && sentenceCount > 1 {
		return m31types.ComplexityComplex
	}
	if complexScore >= 1 {
		return m31types.ComplexityModerate
	}

	// Check project size as a complexity signal
	fileCount := countProjectFiles(workDir)
	if fileCount > 30 {
		// Large project + non-trivial goal verb => moderate
		if strings.Contains(lower, "add ") || strings.Contains(lower, "implement ") || strings.Contains(lower, "create ") {
			return m31types.ComplexityModerate
		}
	}

	// Detect code-complexity signals that override trivial classification.
	// Short goals like "fix race condition" or "add auth bypass" involve
	// non-trivial code work even though they read like single-action verbs.
	// Position-weighted: signals at the start of the goal are stronger.
	codeSignalCount := 0
	for _, sig := range codeComplexitySignals {
		if idx := strings.Index(lower, sig); idx >= 0 {
			codeSignalCount++
			// Boost score if signal appears early in the goal (first 30%)
			if idx < len(lower)*3/10 {
				codeSignalCount++
			}
		}
	}
	if codeSignalCount > 0 {
		// At least moderate; if multiple signals or long goal, promote to complex.
		if codeSignalCount >= 2 || complexScore >= 1 || wordCount > 10 {
			return m31types.ComplexityComplex
		}
		return m31types.ComplexityModerate
	}

	// Trivial: very short, single-action, single-sentence
	if wordCount <= 8 && trivialScore > 0 && sentenceCount <= 1 {
		return m31types.ComplexityTrivial
	}

	// Simple: short goal with no complex indicators
	if wordCount <= 15 && complexScore == 0 {
		return m31types.ComplexitySimple
	}

	return m31types.ComplexityModerate
}

// WorkflowModeForComplexity maps complexity to a workflow mode.
// ModeAuto delegates to this mapping.
func WorkflowModeForComplexity(c m31types.ComplexityLevel) m31types.WorkflowMode {
	switch c {
	case m31types.ComplexityTrivial:
		return m31types.ModeDirect
	case m31types.ComplexitySimple:
		return m31types.ModeFast
	default:
		return m31types.ModeFull
	}
}

// countProjectFiles does a shallow count of non-vendor files in workDir.
func countProjectFiles(workDir string) int {
	count := 0
	_ = filepath.WalkDir(workDir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		rel, _ := filepath.Rel(workDir, path)
		if rel == "." {
			return nil
		}
		if d.IsDir() && (d.Name() == ".git" || d.Name() == "node_modules" || d.Name() == "vendor" || d.Name() == "target" || d.Name() == ".venv") {
			return filepath.SkipDir
		}
		if !d.IsDir() {
			count++
		}
		return nil
	})
	return count
}
