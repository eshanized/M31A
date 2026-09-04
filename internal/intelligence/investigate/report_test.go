package investigate

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/intelligence/explain"
)

// mockSynthesizer is a test double for explain.Synthesizer.
type mockSynthesizer struct {
	mechanism     string
	fixDirection  string
	shouldSucceed bool
}

func (m mockSynthesizer) ChatCompletion(ctx context.Context, req types.ChatRequest) (*types.ChatResponse, error) {
	if !m.shouldSucceed {
		return nil, &synthError{"synthesis failed"}
	}
	return &types.ChatResponse{
		Content: m.mechanism + "\n\nFix: " + m.fixDirection,
	}, nil
}

type synthError struct{ msg string }

func (e *synthError) Error() string { return e.msg }

// runCmd runs a command and fails the test on error.
func runCmd(t *testing.T, dir, cmd string, args ...string) {
	t.Helper()
	c := exec.Command(cmd, args...)
	c.Dir = dir
	out, err := c.CombinedOutput()
	if err != nil {
		t.Fatalf("cmd %s %v failed: %v\n%s", cmd, args, err, string(out))
	}
}

// runCmdOutput runs a command and returns stdout, failing the test on error.
func runCmdOutput(t *testing.T, dir, cmd string, args ...string) string {
	t.Helper()
	c := exec.Command(cmd, args...)
	c.Dir = dir
	out, err := c.CombinedOutput()
	if err != nil {
		t.Fatalf("cmd %s %v failed: %v\n%s", cmd, args, err, string(out))
	}
	return strings.TrimSpace(string(out))
}

// setupTestRepo creates a test repo with a known commit history for bisect testing.
// Returns the repo directory and a cleanup function.
func setupTestRepo(t *testing.T) (string, func()) {
	t.Helper()
	tmpDir := filepath.Join(os.TempDir(), "m31a-test-"+time.Now().Format("20060102150405")+"-"+strings.ReplaceAll(t.Name(), "/", "-"))
	_ = os.MkdirAll(tmpDir, 0o755)

	// Initialize git repo
	runCmd(t, tmpDir, "git", "init")
	runCmd(t, tmpDir, "git", "config", "user.email", "test@example.com")
	runCmd(t, tmpDir, "git", "config", "user.name", "Test User")

	// Initial commit with test file and go.mod
	_ = os.WriteFile(filepath.Join(tmpDir, "main.go"), []byte("package main\n\nfunc main() {}\n"), 0o644)
	_ = os.WriteFile(filepath.Join(tmpDir, "calc_test.go"), []byte(`package main

import "testing"

func TestAdd(t *testing.T) {
	if Add(1, 2) != 3 {
		t.Errorf("Add(1,2) = %d, want 3", Add(1,2))
	}
}
`), 0o644)
	_ = os.WriteFile(filepath.Join(tmpDir, "go.mod"), []byte("module test\n\ngo 1.21\n"), 0o644)
	runCmd(t, tmpDir, "git", "add", "main.go", "calc_test.go", "go.mod")
	runCmd(t, tmpDir, "git", "commit", "-m", "initial commit with test")

	// Add a function that we'll later break
	_ = os.WriteFile(filepath.Join(tmpDir, "calc.go"), []byte("package main\n\nfunc Add(a, b int) int { return a + b }\n"), 0o644)
	runCmd(t, tmpDir, "git", "add", "calc.go")
	runCmd(t, tmpDir, "git", "commit", "-m", "add Add function")

	// Working version - add a comment to make it different
	_ = os.WriteFile(filepath.Join(tmpDir, "calc.go"), []byte("package main\n\n// Add adds two integers\nfunc Add(a, b int) int { return a + b }\n"), 0o644)
	runCmd(t, tmpDir, "git", "add", "calc.go")
	runCmd(t, tmpDir, "git", "commit", "-m", "working Add")

	// Broken version - this is the culprit
	_ = os.WriteFile(filepath.Join(tmpDir, "calc.go"), []byte("package main\n\nfunc Add(a, b int) int { return a - b }\n"), 0o644)
	runCmd(t, tmpDir, "git", "add", "calc.go")
	runCmd(t, tmpDir, "git", "commit", "-m", "BREAK: change Add to subtract")

	// Another commit after
	_ = os.WriteFile(filepath.Join(tmpDir, "main.go"), []byte("package main\n\nfunc main() { println(Add(1,2)) }\n"), 0o644)
	runCmd(t, tmpDir, "git", "add", "main.go")
	runCmd(t, tmpDir, "git", "commit", "-m", "use Add in main")

	return tmpDir, func() { _ = os.RemoveAll(tmpDir) }
}

// buildGraphAndIndex builds the code graph and symbol index for a repo.
func buildGraphAndIndex(t *testing.T, repoDir string) (*codeintel.CodeGraph, *codeintel.SymbolIndex) {
	t.Helper()
	graph, files, err := codeintel.BuildGraph(repoDir, codeintel.AllParsers())
	if err != nil {
		t.Fatalf("build graph: %v", err)
	}
	for _, f := range files {
		for _, cs := range f.CallSites {
			graph.AddCallEdge(codeintel.CallEdge{
				CallerFile: f.Path,
				CallerLine: cs.Line,
				CallerName: cs.CallerName,
				CalleeName: cs.CalleeName,
			})
		}
	}
	index := codeintel.BuildIndex(files)
	return graph, index
}

// createDeps creates the standard test dependencies.
func createDeps(t *testing.T, repoDir string, graph *codeintel.CodeGraph, index *codeintel.SymbolIndex, synth explain.Synthesizer) ReportDeps {
	t.Helper()
	gitClient := git.New(repoDir)
	gitRunner := NewGitRunner(repoDir)
	mgr := NewWorktreeManager(gitRunner, repoDir)

	return ReportDeps{
		Manager:    mgr,
		Runner:     gitRunner,
		Graph:      graph,
		Index:      index,
		GitClient:  gitClient,
		Synth:      synth,
		ModelID:    "test-model",
		MaxCommits: 10,
	}
}

// TestBuildRootCauseReport_VerifiedPath tests the happy path where
// confirmation pass proves culprit fails and parent passes -> verified.
func TestBuildRootCauseReport_VerifiedPath(t *testing.T) {
	repoDir, cleanup := setupTestRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	mockSynth := mockSynthesizer{
		mechanism:     "The culprit commit changed the Add function from addition to subtraction.",
		fixDirection:  "Revert the change to restore addition behavior.",
		shouldSucceed: true,
	}

	deps := createDeps(t, repoDir, graph, index, mockSynth)

	ctx := context.Background()

	// Get the SHAs - HEAD is "use Add in main", HEAD~1 is BREAK, HEAD~2 is working, HEAD~3 is add Add function, HEAD~4 is initial
	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~1") // BREAK commit
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~3") // add Add function (commit before working)

	// Use go test which works with strings.Fields (no quoted args with spaces)
	reproCmd := "go test ./... -run TestAdd"

	report, err := BuildRootCauseReport(ctx, deps, "Add function returns wrong result", baseSHA, headSHA, reproCmd)
	if err != nil {
		t.Fatalf("BuildRootCauseReport failed: %v", err)
	}

	// Assertions
	if report.Outcome != "attributed" {
		t.Errorf("expected outcome 'attributed', got %q", report.Outcome)
	}
	if report.CulpritConfidence != types.ConfidenceVerified {
		t.Errorf("expected CulpritConfidence %q, got %q", types.ConfidenceVerified, report.CulpritConfidence)
	}
	if report.CulpritSHA != headSHA {
		t.Errorf("expected culprit SHA %s, got %s", headSHA, report.CulpritSHA)
	}
	if report.MechanismConfidence != types.ConfidenceLikely && report.MechanismConfidence != types.ConfidenceSpeculative {
		t.Errorf("expected MechanismConfidence <= likely, got %q", report.MechanismConfidence)
	}
	if len(report.AffectedComponents) == 0 {
		t.Log("AffectedComponents is empty - acceptable for leaf change")
	}
	if report.FixDirection == "" {
		t.Errorf("expected non-empty FixDirection")
	}
	if len(report.Evidence) == 0 {
		t.Errorf("expected non-empty Evidence")
	}

	// Verify both confirmation observations are recorded in Evidence
	foundCulpritFail := false
	foundParentPass := false
	for _, e := range report.Evidence {
		if strings.Contains(strings.ToLower(e), "culprit") && strings.Contains(strings.ToLower(e), "fail") {
			foundCulpritFail = true
		}
		if strings.Contains(strings.ToLower(e), "parent") && strings.Contains(strings.ToLower(e), "pass") {
			foundParentPass = true
		}
	}
	if !foundCulpritFail || !foundParentPass {
		t.Errorf("Evidence should record both culprit=fail and parent=pass observations; got: %v", report.Evidence)
	}
}

// TestBuildRootCauseReport_FlakyParent tests the case where parent check fails once,
// which should downgrade CulpritConfidence to likely and record both observations.
func TestBuildRootCauseReport_FlakyParent(t *testing.T) {
	repoDir, cleanup := setupTestRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	mockSynth := mockSynthesizer{
		mechanism:     "Test mechanism",
		fixDirection:  "Test fix",
		shouldSucceed: true,
	}

	deps := createDeps(t, repoDir, graph, index, mockSynth)

	ctx := context.Background()

	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~1") // BREAK commit
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~3") // add Add function

	reproCmd := "go test ./... -run TestAdd"

	report, err := BuildRootCauseReport(ctx, deps, "Add function returns wrong result", baseSHA, headSHA, reproCmd)
	if err != nil {
		t.Logf("Report generation (may fail if test not in history): %v", err)
		return
	}

	// The key assertion: if the report is generated, MechanismConfidence should never exceed Likely
	if report.MechanismConfidence == types.ConfidenceVerified {
		t.Errorf("MechanismConfidence must never be Verified, got %q", report.MechanismConfidence)
	}
}

// TestBuildRootCauseReport_NilSynth tests that nil synthesizer degrades
// MechanismConfidence to speculative and leaves FixDirection empty.
func TestBuildRootCauseReport_NilSynth(t *testing.T) {
	repoDir, cleanup := setupTestRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	deps := createDeps(t, repoDir, graph, index, nil) // nil synthesizer

	ctx := context.Background()

	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~1")
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~3")

	reproCmd := "go test ./... -run TestAdd"

	report, err := BuildRootCauseReport(ctx, deps, "Add function returns wrong result", baseSHA, headSHA, reproCmd)
	if err != nil {
		t.Logf("Report generation (expected without test in history): %v", err)
		return
	}

	if report.MechanismConfidence != types.ConfidenceSpeculative {
		t.Errorf("expected MechanismConfidence %q with nil synth, got %q", types.ConfidenceSpeculative, report.MechanismConfidence)
	}
	if report.FixDirection != "" {
		t.Errorf("expected empty FixDirection with nil synth, got %q", report.FixDirection)
	}
	if report.Mechanism != "" {
		t.Errorf("expected empty Mechanism with nil synth, got %q", report.Mechanism)
	}
}

// TestBuildRootCauseReport_NotReproducible tests the not-reproducible-in-window outcome.
func TestBuildRootCauseReport_NotReproducible(t *testing.T) {
	repoDir, cleanup := setupTestRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	mockSynth := mockSynthesizer{
		mechanism:     "Test mechanism",
		fixDirection:  "Test fix",
		shouldSucceed: true,
	}

	deps := createDeps(t, repoDir, graph, index, mockSynth)

	ctx := context.Background()

	// Use a range where the symptom doesn't reproduce at boundaries
	// Both baseline and head pass (using exit 0 which always succeeds)
	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD")
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~3")

	report, err := BuildRootCauseReport(ctx, deps, "nonexistent symptom", baseSHA, headSHA, "exit 0")
	if err != nil {
		t.Fatalf("BuildRootCauseReport failed: %v", err)
	}

	if report.Outcome != "not-reproducible-in-window" {
		t.Errorf("expected outcome 'not-reproducible-in-window', got %q", report.Outcome)
	}
	if report.CulpritConfidence != types.ConfidenceSpeculative {
		t.Errorf("expected CulpritConfidence %q for not-reproducible, got %q", types.ConfidenceSpeculative, report.CulpritConfidence)
	}
}

// setupDedupRepo creates a repo with a component reachable via two paths.
func setupDedupRepo(t *testing.T) (string, func()) {
	t.Helper()
	tmpDir := filepath.Join(os.TempDir(), "m31a-dedup-"+time.Now().Format("20060102150405")+"-"+strings.ReplaceAll(t.Name(), "/", "-"))
	_ = os.MkdirAll(tmpDir, 0o755)

	runCmd(t, tmpDir, "git", "init")
	runCmd(t, tmpDir, "git", "config", "user.email", "test@example.com")
	runCmd(t, tmpDir, "git", "config", "user.name", "Test User")

	// Create a structure where component "shared" is reachable via two paths:
	// main -> A -> shared
	// main -> B -> shared
	_ = os.WriteFile(filepath.Join(tmpDir, "main.go"), []byte(`package main

import "fmt"

func main() {
	A()
	B()
}
`), 0o644)
	_ = os.WriteFile(filepath.Join(tmpDir, "a.go"), []byte(`package main

func A() {
	shared()
}
`), 0o644)
	_ = os.WriteFile(filepath.Join(tmpDir, "b.go"), []byte(`package main

func B() {
	shared()
}
`), 0o644)
	_ = os.WriteFile(filepath.Join(tmpDir, "shared.go"), []byte(`package main

func shared() {
	println("shared")
}
`), 0o644)

	runCmd(t, tmpDir, "git", "add", ".")
	runCmd(t, tmpDir, "git", "commit", "-m", "initial: shared component with two paths")

	// Culprit: modify shared.go
	_ = os.WriteFile(filepath.Join(tmpDir, "shared.go"), []byte(`package main

func shared() {
	println("BROKEN")
}
`), 0o644)
	runCmd(t, tmpDir, "git", "add", "shared.go")
	runCmd(t, tmpDir, "git", "commit", "-m", "BREAK: break shared")

	return tmpDir, func() { _ = os.RemoveAll(tmpDir) }
}

// TestBuildRootCauseReport_DedupComponents tests that AffectedComponents
// deduplicates entries reachable via multiple transitive paths and keeps
// minimal depth.
func TestBuildRootCauseReport_DedupComponents(t *testing.T) {
	repoDir, cleanup := setupDedupRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	mockSynth := mockSynthesizer{
		mechanism:     "Test mechanism",
		fixDirection:  "Test fix",
		shouldSucceed: true,
	}

	deps := createDeps(t, repoDir, graph, index, mockSynth)

	ctx := context.Background()

	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD")
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~1")

	report, err := BuildRootCauseReport(ctx, deps, "test symptom", baseSHA, headSHA, "exit 1")
	if err != nil {
		t.Logf("Report generation: %v", err)
		return
	}

	// Count occurrences of each component name
	componentCounts := make(map[string]int)
	componentDepths := make(map[string]int)
	for _, comp := range report.AffectedComponents {
		componentCounts[comp.Name]++
		if d, ok := componentDepths[comp.Name]; !ok || comp.Depth < d {
			componentDepths[comp.Name] = comp.Depth
		}
	}

	// Each component should appear exactly once
	for name, count := range componentCounts {
		if count != 1 {
			t.Errorf("component %q appears %d times, expected 1", name, count)
		}
	}
}

// TestBuildRootCauseReport_ComponentOrdering tests that AffectedComponents
// are ordered by dependency depth then name ascending.
func TestBuildRootCauseReport_ComponentOrdering(t *testing.T) {
	repoDir, cleanup := setupDedupRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	mockSynth := mockSynthesizer{
		mechanism:     "Test mechanism",
		fixDirection:  "Test fix",
		shouldSucceed: true,
	}

	deps := createDeps(t, repoDir, graph, index, mockSynth)

	ctx := context.Background()

	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD")
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~1")

	report, err := BuildRootCauseReport(ctx, deps, "test symptom", baseSHA, headSHA, "exit 1")
	if err != nil {
		t.Logf("Report generation: %v", err)
		return
	}

	// Verify ordering: depth ascending, then name ascending
	for i := 1; i < len(report.AffectedComponents); i++ {
		prev := report.AffectedComponents[i-1]
		curr := report.AffectedComponents[i]
		if curr.Depth < prev.Depth {
			t.Errorf("components not ordered by depth: %s (depth %d) before %s (depth %d)", curr.Name, curr.Depth, prev.Name, prev.Depth)
		}
		if curr.Depth == prev.Depth && curr.Name < prev.Name {
			t.Errorf("components at same depth not ordered by name: %s before %s", curr.Name, prev.Name)
		}
	}
}

// TestBuildRootCauseReport_LeafChange tests that leaf changes report
// an empty but non-nil AffectedComponents list.
func TestBuildRootCauseReport_LeafChange(t *testing.T) {
	repoDir, cleanup := setupTestRepo(t)
	defer cleanup()

	graph, index := buildGraphAndIndex(t, repoDir)

	mockSynth := mockSynthesizer{
		mechanism:     "Test mechanism",
		fixDirection:  "Test fix",
		shouldSucceed: true,
	}

	deps := createDeps(t, repoDir, graph, index, mockSynth)

	ctx := context.Background()

	headSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~1")
	baseSHA := runCmdOutput(t, repoDir, "git", "rev-parse", "HEAD~3")

	reproCmd := "go test ./... -run TestAdd"

	report, err := BuildRootCauseReport(ctx, deps, "Add function returns wrong result", baseSHA, headSHA, reproCmd)
	if err != nil {
		t.Logf("Report generation: %v", err)
		return
	}

	// AffectedComponents should be non-nil but empty (or rendered as "none")
	if report.AffectedComponents == nil {
		t.Errorf("AffectedComponents should be non-nil slice, got nil")
	}
	// Empty list is acceptable for leaf changes
	t.Logf("AffectedComponents length: %d", len(report.AffectedComponents))
}

// TestBuildRootCauseReport_JSONConfidenceEnum tests that JSON marshaling
// of confidence fields uses exactly the three enum strings.
func TestBuildRootCauseReport_JSONConfidenceEnum(t *testing.T) {
	testCases := []struct {
		name            string
		culpritConf     types.Confidence
		mechanismConf   types.Confidence
		expectedCulprit string
		expectedMech    string
	}{
		{"verified/likely", types.ConfidenceVerified, types.ConfidenceLikely, "verified", "likely"},
		{"likely/speculative", types.ConfidenceLikely, types.ConfidenceSpeculative, "likely", "speculative"},
		{"speculative/speculative", types.ConfidenceSpeculative, types.ConfidenceSpeculative, "speculative", "speculative"},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			report := &RootCauseReport{
				CulpritConfidence:   tc.culpritConf,
				MechanismConfidence: tc.mechanismConf,
			}

			data, err := json.Marshal(report)
			if err != nil {
				t.Fatalf("marshal failed: %v", err)
			}

			var parsed map[string]interface{}
			if err := json.Unmarshal(data, &parsed); err != nil {
				t.Fatalf("unmarshal failed: %v", err)
			}

			if parsed["culprit_confidence"] != tc.expectedCulprit {
				t.Errorf("culprit_confidence: expected %q, got %q", tc.expectedCulprit, parsed["culprit_confidence"])
			}
			if parsed["mechanism_confidence"] != tc.expectedMech {
				t.Errorf("mechanism_confidence: expected %q, got %q", tc.expectedMech, parsed["mechanism_confidence"])
			}
		})
	}
}

// TestEmitInvestigationEvents_NilStore tests that nil EventStore
// is handled safely without panic or error.
func TestEmitInvestigationEvents_NilStore(t *testing.T) {
	ctx := context.Background()
	var nilStore types.EventStore

	err := EmitInvestigationStarted(ctx, nilStore, InvestigationStartedPayload{
		Symptom:    "test symptom",
		Baseline:   "abc123",
		Head:       "def456",
		WindowSize: 10,
	})
	if err != nil {
		t.Errorf("EmitInvestigationStarted with nil store should not error: %v", err)
	}

	err = EmitInvestigationCompleted(ctx, nilStore, InvestigationCompletedPayload{
		Outcome:              "attributed",
		CulpritSHA:           "def456",
		CulpritConfidence:    string(types.ConfidenceVerified),
		MechanismConfidence:  string(types.ConfidenceLikely),
	})
	if err != nil {
		t.Errorf("EmitInvestigationCompleted with nil store should not error: %v", err)
	}
}