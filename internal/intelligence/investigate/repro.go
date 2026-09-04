package investigate

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

// ReproResult holds the result of a repro command execution.
type ReproResult struct {
	ExitCode int
	Output   string
	Matched  bool
}

// maxOutputBytes caps the combined output captured from repro commands.
const maxOutputBytes = 64 * 1024 // 64KB

// ResolveReproCommand resolves the repro command following strict precedence:
// 1. flagValue (--repro flag) when non-empty
// 2. cfgValue (intelligence.repro_command config) when non-empty
// 3. Auto-detect based on project files in workDir:
//    - go.mod present -> "go test ./..."
//    - package.json present -> "npm test --silent"
//    - neither -> error instructing user to pass --repro
func ResolveReproCommand(flagValue, cfgValue string, workDir string) (string, error) {
	if strings.TrimSpace(flagValue) != "" {
		return strings.TrimSpace(flagValue), nil
	}
	if strings.TrimSpace(cfgValue) != "" {
		return strings.TrimSpace(cfgValue), nil
	}

	// Auto-detect
	if _, err := os.Stat(filepath.Join(workDir, "go.mod")); err == nil {
		return "go test ./...", nil
	}
	if _, err := os.Stat(filepath.Join(workDir, "package.json")); err == nil {
		return "npm test --silent", nil
	}

	return "", ErrNoReproCommand
}

// ErrNoReproCommand is returned when no repro command can be resolved.
var ErrNoReproCommand = &reproError{"no repro command: pass --repro flag, set intelligence.repro_command in config, or run in a Go/npm project"}

type reproError struct {
	msg string
}

func (e *reproError) Error() string {
	return e.msg
}

// ExecuteRepro runs the repro command in the given directory with context cancellation.
// Uses exec.CommandContext with whitespace-split arguments (no shell).
// Captures combined output bounded to maxOutputBytes.
// If symptomRegex is provided, Matched is set based on regex search of combined output.
func ExecuteRepro(ctx context.Context, dir, command string, symptomRegex *regexp.Regexp) (ReproResult, error) {
	// Split command by whitespace (no shell)
	args := strings.Fields(command)
	if len(args) == 0 {
		return ReproResult{}, &reproError{"empty repro command"}
	}

	cmd := exec.CommandContext(ctx, args[0], args[1:]...)
	cmd.Dir = dir

	// Capture combined output with size limit
	outputBuf := make([]byte, 0, maxOutputBytes+1)
	cmd.Stdout = &limitedWriter{buf: &outputBuf, limit: maxOutputBytes}
	cmd.Stderr = &limitedWriter{buf: &outputBuf, limit: maxOutputBytes}

	err := cmd.Run()
	exitCode := 0
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			exitCode = exitErr.ExitCode()
		} else if ctx.Err() != nil {
			// Context cancellation
			return ReproResult{
				ExitCode: -1,
				Output:   string(outputBuf),
				Matched:  false,
			}, ctx.Err()
		} else {
			return ReproResult{}, err
		}
	}

	output := string(outputBuf)
	matched := false
	if symptomRegex != nil {
		matched = symptomRegex.MatchString(output)
	}

	return ReproResult{
		ExitCode: exitCode,
		Output:   output,
		Matched:  matched,
	}, nil
}

// limitedWriter writes to a byte slice with a size limit.
type limitedWriter struct {
	buf   *[]byte
	limit int
}

func (w *limitedWriter) Write(p []byte) (n int, err error) {
	remaining := w.limit - len(*w.buf)
	if remaining <= 0 {
		return len(p), nil // Discard excess but return success
	}
	if len(p) > remaining {
		p = p[:remaining]
	}
	*w.buf = append(*w.buf, p...)
	return len(p), nil
}