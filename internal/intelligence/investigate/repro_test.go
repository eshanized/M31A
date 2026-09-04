package investigate_test

import (
	"context"
	"os"
	"path/filepath"
	"regexp"
	"testing"

	"github.com/eshanized/M31A/internal/intelligence/investigate"
	"github.com/stretchr/testify/require"
)

func TestResolveReproCommand_PrecedenceFlagOverConfigOverAuto(t *testing.T) {
	repo := t.TempDir()
	require.NoError(t, createGoMod(repo))

	tests := []struct {
		name          string
		flagValue     string
		cfgValue      string
		expected      string
		expectError   bool
	}{
		{
			name:      "flag value wins",
			flagValue: "custom test cmd",
			cfgValue:  "config test cmd",
			expected:  "custom test cmd",
		},
		{
			name:      "config value wins when flag empty",
			flagValue: "",
			cfgValue:  "config test cmd",
			expected:  "config test cmd",
		},
		{
			name:     "auto-detect Go when both empty",
			flagValue: "",
			cfgValue:  "",
			expected:  "go test ./...",
		},
		{
			name:        "auto-detect npm when package.json present",
			flagValue:   "",
			cfgValue:    "",
			expected:    "npm test --silent",
			expectError: false,
		},
		{
			name:        "error when neither flag nor config nor auto-detect",
			flagValue:   "",
			cfgValue:    "",
			expected:    "",
			expectError: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			// Create temp dir with appropriate files
			workDir := t.TempDir()
			if tc.name == "auto-detect npm when package.json present" {
				require.NoError(t, os.WriteFile(filepath.Join(workDir, "package.json"), []byte(`{"name": "test"}`), 0o644))
			} else if tc.name != "error when neither flag nor config nor auto-detect" {
				require.NoError(t, createGoMod(workDir))
			}

			cmd, err := investigate.ResolveReproCommand(tc.flagValue, tc.cfgValue, workDir)
			if tc.expectError {
				require.Error(t, err)
			} else {
				require.NoError(t, err)
				require.Equal(t, tc.expected, cmd)
			}
		})
	}
}

func TestExecuteRepro_PassFailRegex(t *testing.T) {
	workDir := t.TempDir()

	// Test pass (exit 0)
	result, err := investigate.ExecuteRepro(context.Background(), workDir, "echo hello", nil)
	require.NoError(t, err)
	require.Equal(t, 0, result.ExitCode)
	require.Contains(t, result.Output, "hello")
	require.False(t, result.Matched)

	// Test fail (exit 1)
	result, err = investigate.ExecuteRepro(context.Background(), workDir, "false", nil)
	require.NoError(t, err)
	require.Equal(t, 1, result.ExitCode)
	require.False(t, result.Matched)

	// Test regex match
	result, err = investigate.ExecuteRepro(context.Background(), workDir, "echo 'FAIL: something broke'", regexp.MustCompile(`FAIL:`))
	require.NoError(t, err)
	require.Equal(t, 0, result.ExitCode)
	require.True(t, result.Matched)

	// Test regex no match
	result, err = investigate.ExecuteRepro(context.Background(), workDir, "echo 'everything ok'", regexp.MustCompile(`FAIL:`))
	require.NoError(t, err)
	require.Equal(t, 0, result.ExitCode)
	require.False(t, result.Matched)
}

func TestExecuteRepro_Cancellation(t *testing.T) {
	workDir := t.TempDir()

	ctx, cancel := context.WithCancel(context.Background())
	// Cancel immediately
	cancel()

	// The command should be killed promptly
	_, err := investigate.ExecuteRepro(ctx, workDir, "sleep 10", nil)
	require.Error(t, err)
	require.Contains(t, err.Error(), "context canceled")
}

func TestExecuteRepro_OutputBounded(t *testing.T) {
	workDir := t.TempDir()

	// Generate large output using a simple command that produces lots of output
	// Use printf with many repetitions
	result, err := investigate.ExecuteRepro(context.Background(), workDir, "printf 'line\\n%.0s' {1..10000}", nil)
	require.NoError(t, err)
	// Output should be bounded (~64KB)
	require.LessOrEqual(t, len(result.Output), 65536)
}

func TestReproResult_Fields(t *testing.T) {
	_ = investigate.ReproResult{
		ExitCode: 0,
		Output:   "test output",
		Matched:  true,
	}
}

func createGoMod(dir string) error {
	return os.WriteFile(filepath.Join(dir, "go.mod"), []byte("module test\n"), 0o644)
}