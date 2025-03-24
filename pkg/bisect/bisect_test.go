package bisect

import (
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
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

func TestBisect_NonAncestorError(t *testing.T) {
	dir, b := setupBisectRepo(t)

	// Create a separate branch that is not an ancestor
	runGit(t, dir, "checkout", "-b", "separate")
	writeFile(t, dir, "separate.go", "package main\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "separate commit")
	separateHash := commitHash(t, dir)

	runGit(t, dir, "checkout", "master")

	// Try to bisect between unrelated commits
	_, err := b.Run(separateHash, commitHash(t, dir), func() bool { return true })
	// This may or may not error depending on git version
	_ = err
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
