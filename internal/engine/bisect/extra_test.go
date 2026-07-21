package bisect

import (
	"fmt"
	"log/slog"
	"os/exec"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
)

// ---------------------------------------------------------------------------
// mockGitRunner implements GitRunner for unit testing.
// ---------------------------------------------------------------------------

type mockGitRunner struct {
	responses []mockResponse
	calls     [][]string
}

type mockResponse struct {
	out string
	err error
}

func newMockGitRunner(responses ...mockResponse) *mockGitRunner {
	return &mockGitRunner{responses: responses}
}

func (m *mockGitRunner) Run(args ...string) (string, error) {
	m.calls = append(m.calls, args)
	if len(m.calls) <= len(m.responses) {
		resp := m.calls[len(m.calls)-1]
		_ = resp
		idx := len(m.calls) - 1
		return m.responses[idx].out, m.responses[idx].err
	}
	return "", fmt.Errorf("no mock response for call %d", len(m.calls))
}

// ---------------------------------------------------------------------------
// Constructor tests
// ---------------------------------------------------------------------------

func TestNew_CreatesInstance(t *testing.T) {
	b := New("/tmp/test", slog.Default())
	if b == nil {
		t.Fatal("New() returned nil")
	}
	if b.workDir != "/tmp/test" {
		t.Errorf("workDir = %q, want %q", b.workDir, "/tmp/test")
	}
}

func TestNew_NilLogger(t *testing.T) {
	b := New("/tmp/test", nil)
	if b == nil {
		t.Fatal("New() returned nil")
	}
	if b.logger != nil {
		t.Error("logger should be nil")
	}
}

// ---------------------------------------------------------------------------
// SetGit tests
// ---------------------------------------------------------------------------

func TestSetGit(t *testing.T) {
	b := New("/tmp/test", slog.Default())
	if b.git != nil {
		t.Error("git should be nil before SetGit")
	}

	mock := newMockGitRunner()
	b.SetGit(mock)
	if b.git == nil {
		t.Error("git should not be nil after SetGit")
	}
}

// ---------------------------------------------------------------------------
// run tests with mock
// ---------------------------------------------------------------------------

func TestRun_UsesWiredGit(t *testing.T) {
	b := New("/tmp/test", slog.Default())
	mock := newMockGitRunner(mockResponse{out: "ok\n", err: nil})
	b.SetGit(mock)

	out, err := b.run("status")
	if err != nil {
		t.Fatalf("run failed: %v", err)
	}
	if out != "ok" {
		t.Errorf("expected 'ok', got %q", out)
	}
	if len(mock.calls) != 1 {
		t.Fatalf("expected 1 call, got %d", len(mock.calls))
	}
	if mock.calls[0][0] != "status" {
		t.Errorf("expected 'status' call, got %v", mock.calls[0])
	}
}

func TestRun_WiredGitError(t *testing.T) {
	b := New("/tmp/test", slog.Default())
	mock := newMockGitRunner(mockResponse{out: "", err: fmt.Errorf("git error")})
	b.SetGit(mock)

	_, err := b.run("status")
	if err == nil {
		t.Fatal("expected error from mock")
	}
}

func TestRun_FallbackToExec(t *testing.T) {
	dir := t.TempDir()
	runGit(t, dir, "init")
	runGit(t, dir, "config", "user.name", "Test")
	runGit(t, dir, "config", "user.email", "test@test.com")
	writeFile(t, dir, "a.go", "package main\n")
	runGit(t, dir, "add", "-A")
	runGit(t, dir, "commit", "-m", "initial")

	b := New(dir, slog.Default())
	// No git wired — should fall back to execGit
	out, err := b.run("status")
	if err != nil {
		t.Fatalf("run fallback failed: %v", err)
	}
	if out == "" {
		t.Error("expected non-empty status output")
	}
}

func TestRun_FallbackError(t *testing.T) {
	dir := t.TempDir()
	b := New(dir, slog.Default())

	_, err := b.run("bisect", "start")
	if err == nil {
		t.Fatal("expected error from non-git directory")
	}
}

// ---------------------------------------------------------------------------
// New constructor tests
// ---------------------------------------------------------------------------

func TestNew_PreservesWorkDir(t *testing.T) {
	b := New("/some/path", slog.Default())
	if b.workDir != "/some/path" {
		t.Errorf("workDir = %q, want %q", b.workDir, "/some/path")
	}
}

func TestNew_PreservesLogger(t *testing.T) {
	logger := slog.Default()
	b := New("/tmp", logger)
	if b.logger != logger {
		t.Error("logger should be preserved")
	}
}

// ---------------------------------------------------------------------------
// BisectResult tests
// ---------------------------------------------------------------------------

func TestBisectResult_Struct(t *testing.T) {
	result := &BisectResult{}
	if result.Diff != "" {
		t.Error("expected empty Diff")
	}
	if result.OffendingCommit.Hash != "" {
		t.Error("expected empty Hash")
	}
}

// ---------------------------------------------------------------------------
// parseBisectLog additional tests
// ---------------------------------------------------------------------------

func TestParseBisectLog_MultipleFormats(t *testing.T) {
	tests := []struct {
		name string
		log  string
		want string
	}{
		{
			name: "no hash found",
			log:  "some random output",
			want: "",
		},
		{
			name: "partial hash",
			log:  "# first bad commit: [abc] short",
			want: "abc",
		},
		{
			name: "long hash",
			log:  "# first bad commit: [deadbeef1234567890abcdef1234567890abcdef] long message",
			want: "deadbeef1234567890abcdef1234567890abcdef",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseBisectLog(tt.log)
			if got != tt.want {
				t.Errorf("parseBisectLog: got %q, want %q", got, tt.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// execGit tests
// ---------------------------------------------------------------------------

func TestExecGit_Success(t *testing.T) {
	dir := t.TempDir()
	runGit(t, dir, "init")

	out, err := execGit(dir, "status")
	if err != nil {
		t.Fatalf("execGit failed: %v", err)
	}
	if out == "" {
		t.Error("expected non-empty output")
	}
}

func TestExecGit_Error(t *testing.T) {
	dir := t.TempDir()

	_, err := execGit(dir, "bisect", "start")
	if err == nil {
		t.Fatal("expected error in non-git directory")
	}
}

func TestExecGit_IncludesCommandInError(t *testing.T) {
	dir := t.TempDir()

	_, err := execGit(dir, "bisect", "start")
	if err == nil {
		t.Fatal("expected error")
	}
	errStr := err.Error()
	if !containsSubstring(errStr, "bisect") {
		t.Errorf("error should mention the command, got: %s", errStr)
	}
}

// ---------------------------------------------------------------------------
// Mock GitRunner bisect flow test
// ---------------------------------------------------------------------------

func TestBisect_WithMockGitRunner(t *testing.T) {
	b := New("/tmp", slog.Default())

	// Simulate a minimal bisect flow with mocked responses
	mock := newMockGitRunner(
		// bisect start
		mockResponse{out: "", err: nil},
		// bisect good <hash>
		mockResponse{out: "", err: nil},
		// bisect bad <hash>
		mockResponse{out: "", err: nil},
		// bisect log (in loop — first iteration)
		mockResponse{out: "# first bad commit: [abc123] break util", err: nil},
		// bisect log (after break — result parse)
		mockResponse{out: "# first bad commit: [abc123] break util", err: nil},
		// diff
		mockResponse{out: "diff --git a/util.go", err: nil},
		// bisect reset (deferred)
		mockResponse{out: "", err: nil},
	)
	b.SetGit(mock)

	// checkFn returns false (bad commit)
	result, err := b.Run("goodhash", "badhash", func() bool {
		return false
	})

	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.OffendingCommit.Hash != "abc123" {
		t.Errorf("expected offending commit abc123, got %s", result.OffendingCommit.Hash)
	}
}

// ---------------------------------------------------------------------------
// GitRunner interface compliance
// ---------------------------------------------------------------------------

func TestGitRunner_Interface(t *testing.T) {
	var _ GitRunner = (*mockGitRunner)(nil)
}

// ---------------------------------------------------------------------------
// ErrBisectResetFailed
// ---------------------------------------------------------------------------

func TestErrBisectResetFailed_IsDefined(t *testing.T) {
	err := m31errors.ErrBisectResetFailed
	if err == nil {
		t.Fatal("ErrBisectResetFailed should not be nil")
	}
}

// Helper: containsSubstring checks if s contains substr
func containsSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// Helper: runExecGit is a thin wrapper for exec.Command used in tests
func runExecGit(t *testing.T, dir string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %s failed: %v\n%s", args[0], err, string(out))
	}
	return string(out)
}
