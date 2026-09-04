package deps_cli_test

import (
	"context"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/cmd/m31a"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/intelligence/deps"
	"github.com/eshanized/M31A/internal/memory/eventstore"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestRunDepsCheck_Usage tests that deps without subcommand prints usage
func TestRunDepsCheck_Usage(t *testing.T) {
	cfg := config.DefaultConfig()
	logger := slog.Default()

	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	exitCode := runDepsCheck([]string{}, "/tmp", cfg, logger)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "Usage: m31a deps check")
}

// TestRunDepsCheck_NoModuleArg tests that missing module arg returns error
func TestRunDepsCheck_NoModuleArg(t *testing.T) {
	cfg := config.DefaultConfig()
	logger := slog.Default()

	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	exitCode := runDepsCheck([]string{"check"}, "/tmp", cfg, logger)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "module argument required")
}

// TestRunDepsCheck_ApproveNoStore tests approve without event store
func TestRunDepsCheck_ApproveNoStore(t *testing.T) {
	cfg := config.DefaultConfig()
	logger := slog.Default()

	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	exitCode := runDepsCheck([]string{"check", "--approve", "github.com/test/mod"}, t.TempDir(), cfg, logger)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "no event store found")
}

// TestRunDepsCheck_ApproveNoPending tests approve with no pending checkpoint
func TestRunDepsCheck_ApproveNoPending(t *testing.T) {
	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")
	os.MkdirAll(m31aDir, 0755)
	eventsDB := filepath.Join(m31aDir, "events.db")
	store, _ := eventstore.NewEventStore(eventsDB)
	store.Close()

	cfg := config.DefaultConfig()
	logger := slog.Default()

	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	exitCode := runDepsCheck([]string{"check", "--approve", "github.com/test/mod"}, dir, cfg, logger)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "no pending checkpoint")
}

// TestComputePolicyHash tests policy hash computation
func TestComputePolicyHash(t *testing.T) {
	cfg := config.DefaultConfig()
	hash := computePolicyHash(cfg)
	assert.Len(t, hash, 16)
	assert.NotEmpty(t, hash)

	// Same config should produce same hash
	hash2 := computePolicyHash(cfg)
	assert.Equal(t, hash, hash2)

	// Different config should produce different hash
	cfg2 := config.DefaultConfig()
	cfg2.Intelligence.DepsRisk.StaleMonths = 24
	hash3 := computePolicyHash(cfg2)
	assert.NotEqual(t, hash, hash3)
}

// TestIsTerminal tests TTY detection
func TestIsTerminal(t *testing.T) {
	f, _ := os.Open("/dev/null")
	defer f.Close()
	result := isTerminal(f)
	assert.False(t, result)
}

// TestRenderVerdict_JSON tests JSON output format
func TestRenderVerdict_JSON(t *testing.T) {
	verdict := deps.Verdict{
		Module:         "github.com/test/mod",
		Version:        "v1.2.3",
		Exists:         true,
		AgeDays:        30,
		LatestRelease:  "2024-01-15T10:00:00Z",
		MaintenanceClass: "active",
		License:        "MIT",
		RiskClass:      "low",
		Confidence:     types.ConfidenceVerified,
	}

	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	exitCode := renderVerdict(verdict, "json", false)

	w.Close()
	os.Stdout = oldStdout

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 0, exitCode)

	var parsed map[string]interface{}
	err := json.Unmarshal([]byte(output), &parsed)
	require.NoError(t, err)
	assert.Equal(t, "github.com/test/mod", parsed["module"])
	assert.Equal(t, "v1.2.3", parsed["version"])
	assert.Equal(t, "low", parsed["risk_class"])
}

// TestRenderVerdict_JSON_Cached tests JSON output with cached flag
func TestRenderVerdict_JSON_Cached(t *testing.T) {
	verdict := deps.Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		Exists:     true,
		RiskClass:  "low",
		Confidence: types.ConfidenceVerified,
	}

	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	exitCode := renderVerdict(verdict, "json", true)

	w.Close()
	os.Stdout = oldStdout

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 0, exitCode)

	var parsed map[string]interface{}
	err := json.Unmarshal([]byte(output), &parsed)
	require.NoError(t, err)
	assert.Contains(t, parsed, "cached_at")
	assert.NotEmpty(t, parsed["cached_at"])
}

// TestRenderVerdict_Table tests table output format
func TestRenderVerdict_Table(t *testing.T) {
	verdict := deps.Verdict{
		Module:         "github.com/test/mod",
		Version:        "v1.2.3",
		Exists:         true,
		AgeDays:        30,
		LatestRelease:  "2024-01-15T10:00:00Z",
		MaintenanceClass: "active",
		License:        "MIT",
		RiskClass:      "low",
		Confidence:     types.ConfidenceVerified,
		Sources: []deps.SourceSnapshot{
			{Name: "depsdev", Status: "ok", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
		},
	}

	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	exitCode := renderVerdict(verdict, "table", false)

	w.Close()
	os.Stdout = oldStdout

	buf := new(strings.Builder)
	_, _ = buf.ReadFrom(r)
	output := buf.String()

	assert.Equal(t, 0, exitCode)
	assert.Contains(t, output, "Module:       github.com/test/mod")
	assert.Contains(t, output, "Risk Class:   low")
	assert.Contains(t, output, "Sources:")
	assert.Contains(t, output, "depsdev: ok")
}

// TestHandleInteractiveApproval_Cancel tests user cancellation
func TestHandleInteractiveApproval_Cancel(t *testing.T) {
	verdict := deps.Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
	}

	oldStdin := os.Stdin
	oldStdout := os.Stdout
	oldStderr := os.Stderr

	r, w, _ := os.Pipe()
	os.Stdin = r
	os.Stdout = w
	os.Stderr = w

	// Simulate user entering "n"
	go func() {
		time.Sleep(10 * time.Millisecond)
		w.Write([]byte("n\n"))
		w.Close()
	}()

	exitCode := handleInteractiveApproval(context.Background(), nil, verdict, "policy-hash", "table")

	os.Stdin = oldStdin
	os.Stdout = oldStdout
	os.Stderr = oldStderr

	assert.Equal(t, 1, exitCode)
}