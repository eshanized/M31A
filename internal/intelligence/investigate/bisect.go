package investigate

import (
	"context"
	"fmt"
	"strings"
)

// BisectStep records a single probe during bisect.
type BisectStep struct {
	SHA      string
	ExitCode int
	Verdict  string // "good" or "bad"
}

// Candidates returns ordered candidate SHAs from rev-list --reverse baseline..head
// truncated to maxCommits after validating both refs.
// The returned slice is ordered from oldest to newest (reverse topological order).
func Candidates(git InvestigateGitRunner, baseline, head string, maxCommits int) ([]string, error) {
	if maxCommits < 1 {
		return nil, fmt.Errorf("maxCommits must be >= 1")
	}
	if !ValidateRef(baseline) {
		return nil, fmt.Errorf("invalid baseline ref: %q", baseline)
	}
	if !ValidateRef(head) {
		return nil, fmt.Errorf("invalid head ref: %q", head)
	}

	out, err := git.Run("rev-list", "--reverse", baseline+".."+head)
	if err != nil {
		return nil, fmt.Errorf("git rev-list --reverse %s..%s: %w", baseline, head, err)
	}

	lines := strings.Split(strings.TrimSpace(out), "\n")
	if len(lines) == 1 && lines[0] == "" {
		return []string{}, nil
	}

	if len(lines) > maxCommits {
		lines = lines[:maxCommits]
	}

	// Filter out empty lines
	var candidates []string
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line != "" {
			candidates = append(candidates, line)
		}
	}

	return candidates, nil
}

// FindCulprit performs a window-bounded binary search to find the first failing commit.
// pre-check: symptom must FAIL at head and PASS at baseline (window start).
// Returns culprit SHA, steps taken, and error.
// The check callback receives each probed SHA; manager performs checkout before invoking it.
func FindCulprit(
	ctx context.Context,
	mgr *WorktreeManager,
	git InvestigateGitRunner,
	baseline, head string,
	maxCommits int,
	check func(ctx context.Context, sha string) bool,
	logf func(string, ...any),
) (string, []BisectStep, error) {
	// Validate inputs
	if maxCommits < 1 {
		return "", nil, fmt.Errorf("maxCommits must be >= 1")
	}
	if !ValidateRef(baseline) {
		return "", nil, fmt.Errorf("invalid baseline ref: %q", baseline)
	}
	if !ValidateRef(head) {
		return "", nil, fmt.Errorf("invalid head ref: %q", head)
	}

	// Get candidates
	candidates, err := Candidates(git, baseline, head, maxCommits)
	if err != nil {
		return "", nil, err
	}
	if len(candidates) == 0 {
		return "", nil, fmt.Errorf("no candidates in range %s..%s", baseline, head)
	}

	// Pre-check: symptom must FAIL at head (last candidate)
	headSHA := candidates[len(candidates)-1]
	if logf != nil {
		logf("pre-check: testing HEAD %s", headSHA)
	}
	if check(ctx, headSHA) {
		// Symptom passes at head - not reproducible in window
		return "", nil, &NotReproducibleInWindowError{
			CheckedSHA:      headSHA,
			ObservedResult:  "pass",
			CandidateWindow: len(candidates),
		}
	}
	if logf != nil {
		logf("pre-check: HEAD %s fails (expected)", headSHA)
	}

	// Pre-check: symptom must PASS at baseline (first candidate)
	baselineSHA := candidates[0]
	if logf != nil {
		logf("pre-check: testing baseline %s", baselineSHA)
	}
	if !check(ctx, baselineSHA) {
		// Symptom fails at baseline - not reproducible in window
		return "", nil, &NotReproducibleInWindowError{
			CheckedSHA:      baselineSHA,
			ObservedResult:  "fail",
			CandidateWindow: len(candidates),
		}
	}
	if logf != nil {
		logf("pre-check: baseline %s passes (expected)", baselineSHA)
	}

	// Binary search
	var steps []BisectStep
	low := 0
	high := len(candidates) - 1
	culpritIdx := -1

	const maxIterations = 50
	iteration := 0

	for low <= high {
		iteration++
		if iteration > maxIterations {
			return "", steps, &IterationCapExceededError{
				MaxIterations: maxIterations,
				ProbesTaken:   steps,
			}
		}

		mid := (low + high) / 2
		sha := candidates[mid]

		if logf != nil {
			logf("bisect probe %d/%d: %s (low=%d high=%d mid=%d)", iteration, maxIterations, sha, low, high, mid)
		}

		result := check(ctx, sha)
		verdict := "good"
		exitCode := 0
		if !result {
			verdict = "bad"
			exitCode = 1
		}

		steps = append(steps, BisectStep{
			SHA:      sha,
			ExitCode: exitCode,
			Verdict:  verdict,
		})

		if logf != nil {
			logf("bisect probe %s: %s", sha, verdict)
		}

		if result {
			// Pass - move to upper half
			low = mid + 1
		} else {
			// Fail - this could be culprit, move to lower half
			culpritIdx = mid
			high = mid - 1
		}
	}

	if culpritIdx == -1 {
		return "", steps, fmt.Errorf("bisect did not find culprit")
	}

	culpritSHA := candidates[culpritIdx]

	// Confirmation: culprit must fail AND its parent must pass
	// Parent is the previous candidate in the list (or baseline if culprit is first)
	var parentSHA string
	if culpritIdx > 0 {
		parentSHA = candidates[culpritIdx-1]
	} else {
		parentSHA = baselineSHA
	}

	if logf != nil {
		logf("confirmation: testing culprit %s", culpritSHA)
	}
	if check(ctx, culpritSHA) {
		return "", steps, fmt.Errorf("confirmation failed: culprit %s unexpectedly passes", culpritSHA)
	}

	if logf != nil {
		logf("confirmation: testing parent %s", parentSHA)
	}
	if !check(ctx, parentSHA) {
		return "", steps, fmt.Errorf("confirmation failed: parent %s unexpectedly fails", parentSHA)
	}

	if logf != nil {
		logf("confirmation: culprit %s fails, parent %s passes - attribution verified", culpritSHA, parentSHA)
	}

	return culpritSHA, steps, nil
}

// NotReproducibleInWindowError is returned when the symptom doesn't reproduce at window boundaries.
type NotReproducibleInWindowError struct {
	CheckedSHA      string
	ObservedResult  string // "pass" or "fail"
	CandidateWindow int
}

func (e *NotReproducibleInWindowError) Error() string {
	return fmt.Sprintf("symptom not reproducible in window: checked %s, observed %s (window size %d)", e.CheckedSHA, e.ObservedResult, e.CandidateWindow)
}

// IsNotReproducibleInWindow checks if an error is a NotReproducibleInWindowError.
func IsNotReproducibleInWindow(err error) bool {
	_, ok := err.(*NotReproducibleInWindowError)
	return ok
}

// IterationCapExceededError is returned when bisect exceeds max iterations.
type IterationCapExceededError struct {
	MaxIterations int
	ProbesTaken   []BisectStep
}

func (e *IterationCapExceededError) Error() string {
	return fmt.Sprintf("bisect did not converge after %d iterations (probes: %d)", e.MaxIterations, len(e.ProbesTaken))
}

// IsIterationCapExceeded checks if an error is an IterationCapExceededError.
func IsIterationCapExceeded(err error) bool {
	_, ok := err.(*IterationCapExceededError)
	return ok
}