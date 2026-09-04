package cli

import (
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/intelligence/deps"
	"github.com/eshanized/M31A/internal/memory/eventstore"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestRunDepsCheck_Usage(t *testing.T) {
	cfg := config.DefaultConfig()
	logger := slog.Default()

	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	cliCfg := DepsCLIConfig{
		WorkDir: "/tmp",
		Config:  cfg,
		Logger:  logger,
		Format:  "table",
		Module:  "",
		Stdin:   os.Stdin,
		Stdout:  os.Stdout,
		Stderr:  os.Stderr,
	}

	ctx := context.Background()
	exitCode := RunDepsCheck(ctx, cliCfg)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = io.Copy(buf, r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "Usage: m31a deps check")
}

func TestRunDepsCheck_ApproveNoStore(t *testing.T) {
	cfg := config.DefaultConfig()
	logger := slog.Default()

	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	dir := t.TempDir()
	cliCfg := DepsCLIConfig{
		WorkDir: dir,
		Config:  cfg,
		Logger:  logger,
		Format:  "table",
		Approve: "github.com/test/mod",
		Stdin:   os.Stdin,
		Stdout:  os.Stdout,
		Stderr:  os.Stderr,
	}

	ctx := context.Background()
	exitCode := RunDepsCheck(ctx, cliCfg)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = io.Copy(buf, r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "no event store found")
}

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

	cliCfg := DepsCLIConfig{
		WorkDir: dir,
		Config:  cfg,
		Logger:  logger,
		Format:  "table",
		Approve: "github.com/test/mod",
		Stdin:   os.Stdin,
		Stdout:  os.Stdout,
		Stderr:  os.Stderr,
	}

	ctx := context.Background()
	exitCode := RunDepsCheck(ctx, cliCfg)

	w.Close()
	os.Stderr = oldStderr

	buf := new(strings.Builder)
	_, _ = io.Copy(buf, r)
	output := buf.String()

	assert.Equal(t, 1, exitCode)
	assert.Contains(t, output, "no pending checkpoint")
}

func TestComputePolicyHash(t *testing.T) {
	cfg := config.DefaultConfig()
	hash := computePolicyHash(cfg)
	assert.Len(t, hash, 16)
	assert.NotEmpty(t, hash)

	hash2 := computePolicyHash(cfg)
	assert.Equal(t, hash, hash2)

	cfg2 := config.DefaultConfig()
	cfg2.Intelligence.DepsRisk.StaleMonths = 24
	hash3 := computePolicyHash(cfg2)
	assert.NotEqual(t, hash, hash3)
}

func TestIsTerminal(t *testing.T) {
	// /dev/null is a character device, so use a regular file instead
	f, err := os.CreateTemp("", "test")
	require.NoError(t, err)
	defer os.Remove(f.Name())
	defer f.Close()

	result := isTerminal(f)
	assert.False(t, result)
}

func TestRenderVerdict_JSON(t *testing.T) {
	verdict := deps.Verdict{
		Module:           "github.com/test/mod",
		Version:          "v1.2.3",
		Exists:           true,
		AgeDays:          30,
		LatestRelease:    "2024-01-15T10:00:00Z",
		MaintenanceClass: "active",
		License:          "MIT",
		RiskClass:        "low",
		Confidence:       types.ConfidenceVerified,
	}

	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	cliCfg := DepsCLIConfig{
		WorkDir: "/tmp",
		Config:  config.DefaultConfig(),
		Logger:  slog.Default(),
		Format:  "json",
		Stdout:  w,
		Stderr:  os.Stderr,
	}

	exitCode := renderVerdict(cliCfg, verdict, false)

	w.Close()
	os.Stdout = oldStdout

	buf := new(strings.Builder)
	_, _ = io.Copy(buf, r)
	output := buf.String()

	assert.Equal(t, 0, exitCode)

	var parsed map[string]interface{}
	err := json.Unmarshal([]byte(output), &parsed)
	require.NoError(t, err)
	assert.Equal(t, "github.com/test/mod", parsed["module"])
	assert.Equal(t, "v1.2.3", parsed["version"])
	assert.Equal(t, "low", parsed["risk_class"])
}

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

	cliCfg := DepsCLIConfig{
		WorkDir: "/tmp",
		Config:  config.DefaultConfig(),
		Logger:  slog.Default(),
		Format:  "json",
		Stdout:  w,
		Stderr:  os.Stderr,
	}

	exitCode := renderVerdict(cliCfg, verdict, true)

	w.Close()
	os.Stdout = oldStdout

	buf := new(strings.Builder)
	_, _ = io.Copy(buf, r)
	output := buf.String()

	assert.Equal(t, 0, exitCode)

	var parsed map[string]interface{}
	err := json.Unmarshal([]byte(output), &parsed)
	require.NoError(t, err)
	assert.Contains(t, parsed, "cached_at")
	assert.NotEmpty(t, parsed["cached_at"])
}

func TestRenderVerdict_Table(t *testing.T) {
	verdict := deps.Verdict{
		Module:           "github.com/test/mod",
		Version:          "v1.2.3",
		Exists:           true,
		AgeDays:          30,
		LatestRelease:    "2024-01-15T10:00:00Z",
		MaintenanceClass: "active",
		License:          "MIT",
		RiskClass:        "low",
		Confidence:       types.ConfidenceVerified,
		Sources: []deps.SourceSnapshot{
			{Name: "depsdev", Status: "ok", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
		},
	}

	oldStdout := os.Stdout
	r, w, _ := os.Pipe()
	os.Stdout = w

	cliCfg := DepsCLIConfig{
		WorkDir: "/tmp",
		Config:  config.DefaultConfig(),
		Logger:  slog.Default(),
		Format:  "table",
		Stdout:  w,
		Stderr:  os.Stderr,
	}

	exitCode := renderVerdict(cliCfg, verdict, false)

	w.Close()
	os.Stdout = oldStdout

	buf := new(strings.Builder)
	_, _ = io.Copy(buf, r)
	output := buf.String()

	assert.Equal(t, 0, exitCode)
	assert.Contains(t, output, "Module:       github.com/test/mod")
	assert.Contains(t, output, "Risk Class:   low")
	assert.Contains(t, output, "Sources:")
	assert.Contains(t, output, "depsdev: ok")
}

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

	go func() {
		time.Sleep(10 * time.Millisecond)
		w.Write([]byte("n\n"))
		w.Close()
	}()

	cliCfg := DepsCLIConfig{
		WorkDir: "",
		Config:  config.DefaultConfig(),
		Logger:  slog.Default(),
		Format:  "table",
		Stdin:   r,
		Stdout:  w,
		Stderr:  w,
	}

	exitCode := handleInteractiveApproval(context.Background(), cliCfg, verdict, "policy-hash")

	os.Stdin = oldStdin
	os.Stdout = oldStdout
	os.Stderr = oldStderr

	assert.Equal(t, 1, exitCode)
}
