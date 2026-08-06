package observability

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"time"
)

// CrashReport captures all context about a panic for debugging and CI integration.
type CrashReport struct {
	Timestamp  string `json:"timestamp"`
	Version    string `json:"version"`
	Commit     string `json:"commit"`
	GoVersion  string `json:"go_version"`
	OS         string `json:"os"`
	Arch       string `json:"arch"`
	StackTrace string `json:"stack_trace"`
	PanicValue any    `json:"panic_value"`
}

// RecoverAndCapture recovers from a panic, captures a crash report, writes it to disk,
// logs to stderr for CI capture, and re-panics to preserve exit behavior.
// version and commit are the binary version and git commit (from build-time ldflags).
func RecoverAndCapture(version, commit string) {
	r := recover()
	if r == nil {
		return
	}

	report := CrashReport{
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		Version:    version,
		Commit:     commit,
		GoVersion:  runtime.Version(),
		OS:         runtime.GOOS,
		Arch:       runtime.GOARCH,
		StackTrace: string(debug.Stack()),
		PanicValue: r,
	}

	// Write crash report to ~/.m31a/crashes/
	homeDir, err := os.UserHomeDir()
	if err == nil {
		crashDir := filepath.Join(homeDir, ".m31a", "crashes")
		if mkdirErr := os.MkdirAll(crashDir, 0o700); mkdirErr == nil {
			filename := fmt.Sprintf("crash-%s.json", time.Now().UTC().Format("20060102-150405"))
			filepath := filepath.Join(crashDir, filename)
			if data, marshalErr := json.MarshalIndent(report, "", "  "); marshalErr == nil {
				_ = os.WriteFile(filepath, data, 0o600)
			}
		}
	}

	// Log to stderr for CI capture
	fmt.Fprintf(os.Stderr, "CRASH: %v\n%s\n", r, report.StackTrace)

	// Re-panic to preserve exit behavior
	panic(r)
}
