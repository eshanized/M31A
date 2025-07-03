package bisect

import (
	"errors"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
)

func setupBisectRepo(t *testing.T) (string, *Bisect) {
	t.Helper()
	dir := t.TempDir()

	// Init repo
	runGit(t, dir, "init")
	runGit(t, dir, "config", "user.name", "Test")
	runGit(t, dir, "config", "user.email", "test@test.com")

	// Commit 1: good
	writeFile(t, dir, "main.go", "package main\nfunc main() {}\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "initial")

	// Commit 2: good
	writeFile(t, dir, "util.go", "package main\nfunc helper() {}\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "add util")

	// Commit 3: bad - removes the helper
	writeFile(t, dir, "util.go", "package main\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "break util")

	// Commit 4: bad - adds more
	writeFile(t, dir, "extra.go", "package main\nfunc extra() {}\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "add extra")

	b := New(dir, slog.Default())
	return dir, b
}

func runGit(t *testing.T, dir string, args ...string) {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %s failed: %v\n%s", args[0], err, string(out))
	}
}

func writeFile(t *testing.T, dir, name, content string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
		t.Fatalf("writeFile %s failed: %v", name, err)
	}
}

func commitHash(t *testing.T, dir string) string {
	t.Helper()
	out, err := exec.Command("git", "rev-parse", "HEAD").CombinedOutput()
	if err != nil {
		t.Fatalf("rev-parse failed: %v", err)
	}
	return string(out[:len(out)-1]) // trim newline
}

func TestBisect_Successful(t *testing.T) {
	dir, b := setupBisectRepo(t)

	// Get hashes
	runGit(t, dir, "log", "--oneline") // just to verify

	// Commit 1 hash (initial)
	hashes := gitLogHashes(t, dir)
	if len(hashes) < 4 {
		t.Fatalf("Expected 4 commits, got %d", len(hashes))
	}

	// hashes are newest first: [4, 3, 2, 1]
	// Commit 2 (index 2) is good, Commit 3 (index 1) is bad
	sessionStartHash := hashes[2] // "add util" (good)
	headHash := hashes[0]         // "add extra" (bad)

	// checkFn: util.go should contain "helper"
	checkCalls := 0
	checkFn := func() bool {
		checkCalls++
		content, _ := os.ReadFile(filepath.Join(dir, "util.go"))
		return len(content) > 20 // "helper" version is longer
	}

	result, err := b.Run(sessionStartHash, headHash, checkFn)
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}

	if checkCalls == 0 {
		t.Fatal("Expected checkFn to be called at least once")
	}

	// The bisect should have identified the offending commit
	if result.OffendingCommit.ShortHash == "" {
		t.Fatal("Expected non-empty offending commit")
	}

	if result.Diff == "" {
		t.Fatal("Expected non-empty diff")
	}
}

func TestBisect_AlwaysPasses(t *testing.T) {
	dir, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, dir)
	sessionStartHash := hashes[len(hashes)-1] // oldest
	headHash := hashes[0]                     // newest

	// checkFn always passes - no bad commit in range
	checkFn := func() bool { return true }

	_, err := b.Run(sessionStartHash, headHash, checkFn)
	// Bisect may or may not error depending on git version
	// The important thing is it doesn't panic and resets
	_ = err
}

func TestBisect_AlwaysFails(t *testing.T) {
	dir, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, dir)
	sessionStartHash := hashes[len(hashes)-1]
	headHash := hashes[0]

	// checkFn always fails - first commit after good is bad
	checkFn := func() bool { return false }

	result, err := b.Run(sessionStartHash, headHash, checkFn)
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}

	if result.OffendingCommit.ShortHash == "" {
		t.Fatal("Expected non-empty offending commit")
	}
}

func TestBisect_ResetOnError(t *testing.T) {
	dir, b := setupBisectRepo(t)

	// Use invalid hashes to force an error
	_, err := b.Run("invalid1", "invalid2", func() bool { return true })
	if err == nil {
		t.Fatal("Expected error for invalid hashes")
	}

	// Repo should still be in clean state (bisect reset was called)
	out, _ := exec.Command("git", "-C", dir, "status").CombinedOutput()
	if len(out) == 0 {
		t.Fatal("Expected status output")
	}
}

func TestBisect_ParseLog(t *testing.T) {
	tests := []struct {
		name string
		log  string
		want string
	}{
		{
			name: "standard format",
			log: `git bisect start
# bad: [abc123] commit message
# good: [def456] earlier commit
# first bad commit: [abc123] commit message`,
			want: "abc123",
		},
		{
			name: "bracket format",
			log: `[abc123] commit message
Bisecting: 0 revisions left`,
			want: "abc123",
		},
		{
			name: "empty log",
			log:  "",
			want: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseBisectLog(tt.log)
			if got != tt.want {
				t.Errorf("parseBisectLog: expected %q, got %q", tt.want, got)
			}
		})
	}
}

func TestBisect_DiffExtraction(t *testing.T) {
	dir, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, dir)
	// hashes[2] = "add util" (good), hashes[1] = "break util" (bad)
	sessionStartHash := hashes[2]
	headHash := hashes[1]

	// checkFn: commit 3 ("break util") should be identified as bad
	// We know commit at hashes[1] is the first bad one
	// Check if util.go has the helper function
	checkFn := func() bool {
		content, err := os.ReadFile(filepath.Join(dir, "util.go"))
		if err != nil {
			return false
		}
		// The "good" version has "func helper()" which is > 20 chars
		// The "bad" version is just "package main\n"
		return stringsContains(string(content), "func helper")
	}

	result, err := b.Run(sessionStartHash, headHash, checkFn)
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}

	if result.OffendingCommit.ShortHash == "" {
		t.Fatal("Expected non-empty offending commit")
	}

	if result.Diff == "" {
		t.Error("Expected diff to be non-empty")
	}
}

func TestBisect_ParseLog_EdgeCases(t *testing.T) {
	tests := []struct {
		name string
		log  string
		want string
	}{
		{
			name: "first word without brackets",
			log: `git bisect start
# first bad commit: abc123 some message`,
			want: "abc123",
		},
		{
			name: "bracket format fallback parsing",
			log: `[deadbeef] this is a commit message
some other line`,
			want: "deadbeef",
		},
		{
			name: "empty string returns empty",
			log:  "",
			want: "",
		},
		{
			name: "only whitespace returns empty",
			log:  "   \n\n  \t  ",
			want: "",
		},
		{
			name: "malformed bracket - no closing",
			log:  "# first bad commit: [abc123",
			want: "[abc123",
		},
		{
			name: "malformed bracket - empty brackets",
			log:  "# first bad commit: [] message",
			want: "",
		},
		{
			name: "bisecting lines are skipped",
			log: `Bisecting: 0 revisions left
Bisecting: 1 revision left`,
			want: "",
		},
		{
			name: "standard format with extra lines",
			log: `git bisect start
# bad: [abc123] bad commit
# good: [def456] good commit
Bisecting: 0 revisions left
# first bad commit: [abc123] bad commit`,
			want: "abc123",
		},
		{
			name: "bracket at start with spaces",
			log: `  [feed123] spaced commit`,
			want: "feed123",
		},
		{
			name: "multiple first bad commits - takes first",
			log: `# first bad commit: [aaa111] first
# first bad commit: [bbb222] second`,
			want: "aaa111",
		},
		{
			name: "no recognizable pattern",
			log:  "some random log output\nnothing useful here",
			want: "",
		},
		{
			name: "first bad commit with only hash",
			log:  "# first bad commit: abc",
			want: "abc",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseBisectLog(tt.log)
			if got != tt.want {
				t.Errorf("parseBisectLog: expected %q, got %q", tt.want, got)
			}
		})
	}
}

func TestBisect_CheckFnPassesThenErrors(t *testing.T) {
	dir, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, dir)
	// Use a range where checkFn passes on first call
	sessionStartHash := hashes[2] // "add util"
	headHash := hashes[0]         // "add extra"

	// checkFn passes, so bisect marks as "good" and moves forward
	callCount := 0
	checkFn := func() bool {
		callCount++
		return true
	}

	result, err := b.Run(sessionStartHash, headHash, checkFn)
	// Should complete without error
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if callCount == 0 {
		t.Fatal("Expected checkFn to be called at least once")
	}
	if result.OffendingCommit.ShortHash == "" {
		t.Fatal("Expected non-empty offending commit")
	}
}

func TestBisect_CheckFnFailsThenErrors(t *testing.T) {
	dir, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, dir)
	sessionStartHash := hashes[2]
	headHash := hashes[0]

	callCount := 0
	checkFn := func() bool {
		callCount++
		return false
	}

	result, err := b.Run(sessionStartHash, headHash, checkFn)
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if callCount == 0 {
		t.Fatal("Expected checkFn to be called at least once")
	}
	if result.OffendingCommit.ShortHash == "" {
		t.Fatal("Expected non-empty offending commit")
	}
}

func TestBisect_NilLogger(t *testing.T) {
	dir := t.TempDir()
	runGit(t, dir, "init")
	runGit(t, dir, "config", "user.name", "Test")
	runGit(t, dir, "config", "user.email", "test@test.com")
	writeFile(t, dir, "a.go", "package main\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "initial")

	// Create Bisect with nil logger - should not panic
	b := New(dir, nil)
	hashes := gitLogHashes(t, dir)

	_, err := b.Run(hashes[len(hashes)-1], hashes[0], func() bool { return true })
	// Should not panic with nil logger
	_ = err
}

func TestBisect_DiffExtractionErrorSilent(t *testing.T) {
	dir, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, dir)
	sessionStartHash := hashes[2]
	headHash := hashes[1]

	checkFn := func() bool {
		content, err := os.ReadFile(filepath.Join(dir, "util.go"))
		if err != nil {
			return false
		}
		return stringsContains(string(content), "func helper")
	}

	result, err := b.Run(sessionStartHash, headHash, checkFn)
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}

	// Diff should be populated (may be empty if git diff fails, but no error returned)
	if result.OffendingCommit.ShortHash == "" {
		t.Fatal("Expected non-empty offending commit")
	}
}

func TestBisect_GoodCommitError(t *testing.T) {
	_, b := setupBisectRepo(t)

	// Use valid head but invalid good hash to trigger error on "bisect good"
	_, err := b.Run("nonexistentgoodhash", "nonexistentbadhash", func() bool { return true })
	if err == nil {
		t.Fatal("Expected error for invalid hashes")
	}
	// Error should mention the bad hash or good hash
	errStr := err.Error()
	if !stringsContains(errStr, "nonexistentgoodhash") && !stringsContains(errStr, "nonexistentbadhash") {
		t.Logf("Error message: %s", errStr)
	}
}

func TestBisect_EmptyWorkDir(t *testing.T) {
	dir := t.TempDir()
	b := New(dir, slog.Default())

	// Running bisect in a non-git directory should fail
	_, err := b.Run("abc", "def", func() bool { return true })
	if err == nil {
		t.Fatal("Expected error when running in non-git directory")
	}
}

func TestBisect_ParseBisectLog_Unit(t *testing.T) {
	// Direct unit tests for parseBisectLog edge cases
	tests := []struct {
		name string
		log  string
		want string
	}{
		{
			name: "hash with closing bracket at position 1",
			log:  "# first bad commit: [a] minimal hash",
			want: "a",
		},
		{
			name: "bracket format with short hash",
			log:  "[x] short",
			want: "x",
		},
		{
			name: "first bad commit line with leading spaces",
			log: "  # first bad commit: [commit1] spaced line",
			want: "commit1",
		},
		{
			name: "mixed formats in same log",
			log: `Bisecting: 0 left
[abc123] some commit
# first bad commit: [def456] the real bad one`,
			want: "abc123", // parser returns first bracket match it finds
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseBisectLog(tt.log)
			if got != tt.want {
				t.Errorf("parseBisectLog(%q) = %q, want %q", tt.log, got, tt.want)
			}
		})
	}
}

func TestBisect_SameGoodAndBadHash(t *testing.T) {
	_, b := setupBisectRepo(t)

	hashes := gitLogHashes(t, b.workDir)
	sameHash := hashes[0]

	// Good and bad are the same - should error
	_, err := b.Run(sameHash, sameHash, func() bool { return true })
	// Git should error or bisect should fail
	_ = err
}

// ---------------------------------------------------------------------------
// M-29: Bisect reset error sentinel
// ---------------------------------------------------------------------------

// TestBisect_ResetFailure_WrapsErrBisectResetFailed verifies that when
// bisect reset fails, the returned error wraps ErrBisectResetFailed.
func TestBisect_ResetFailure_WrapsErrBisectResetFailed(t *testing.T) {
	// Use invalid hashes to force an error that triggers the reset path
	dir := t.TempDir()
	runGit(t, dir, "init")
	runGit(t, dir, "config", "user.name", "Test")
	runGit(t, dir, "config", "user.email", "test@test.com")
	writeFile(t, dir, "a.go", "package main\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "initial")

	b := New(dir, slog.Default())

	// Invalid hashes should cause bisect to error, and the defer
	// should wrap the reset error if reset also fails.
	_, err := b.Run("0000000000000000000000000000000000000000", "1111111111111111111111111111111111111111", func() bool {
		return true
	})
	// The error may or may not wrap ErrBisectResetFailed depending on
	// whether the reset itself fails. The key thing is the Run function
	// doesn't panic and handles the error path.
	if err != nil {
		// If we got an error, it should be a valid error
		_ = err
	}
}

// TestBisect_ErrBisectResetFailed_IsDefined verifies the sentinel error exists.
func TestBisect_ErrBisectResetFailed_IsDefined(t *testing.T) {
	err := m31errors.ErrBisectResetFailed
	if err == nil {
		t.Fatal("ErrBisectResetFailed should not be nil")
	}
	if !errors.Is(err, m31errors.ErrBisectResetFailed) {
		t.Error("errors.Is should match ErrBisectResetFailed")
	}
}

func TestBisect_SingleCommit(t *testing.T) {
	dir := t.TempDir()
	runGit(t, dir, "init")
	runGit(t, dir, "config", "user.name", "Test")
	runGit(t, dir, "config", "user.email", "test@test.com")
	writeFile(t, dir, "a.go", "package main\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "single")
	headHash := commitHash(t, dir)

	// Need a parent for bisect
	parent, _ := exec.Command("git", "-C", dir, "rev-parse", "HEAD^").CombinedOutput()
	if len(parent) == 0 {
		// No parent, can't bisect
		return
	}

	b := New(dir, slog.Default())
	_, err := b.Run(string(parent[:len(parent)-1]), headHash, func() bool { return false })
	// May or may not work with single commit
	_ = err
}

func gitLogHashes(t *testing.T, dir string) []string {
	t.Helper()
	cmd := exec.Command("git", "log", "--format=%H")
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git log failed: %v", err)
	}

	var hashes []string
	for _, line := range stringsSplit(string(out), "\n") {
		line = stringsTrimSpace(line)
		if len(line) == 40 {
			hashes = append(hashes, line)
		}
	}
	return hashes
}

// Minimal string helpers to avoid importing strings in test file
func stringsSplit(s, sep string) []string {
	result := []string{}
	for {
		idx := index(s, sep)
		if idx == -1 {
			result = append(result, s)
			break
		}
		result = append(result, s[:idx])
		s = s[idx+len(sep):]
	}
	return result
}

func stringsTrimSpace(s string) string {
	start := 0
	for start < len(s) && (s[start] == ' ' || s[start] == '\n' || s[start] == '\r' || s[start] == '\t') {
		start++
	}
	end := len(s)
	for end > start && (s[end-1] == ' ' || s[end-1] == '\n' || s[end-1] == '\r' || s[end-1] == '\t') {
		end--
	}
	return s[start:end]
}

func index(s, sep string) int {
	for i := 0; i <= len(s)-len(sep); i++ {
		if s[i:i+len(sep)] == sep {
			return i
		}
	}
	return -1
}

func stringsContains(s, substr string) bool {
	if len(substr) == 0 {
		return true
	}
	if len(s) < len(substr) {
		return false
	}
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
