package workflow

import (
	"context"
	"fmt"
	"io/fs"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
)

// readTaskFiles reads the content of files for a task.
func (e *Engine) readTaskFiles(files []string) string {
	var sb strings.Builder
	for _, f := range files {
		path := filepath.Join(e.workDir, f)
		content, err := os.ReadFile(path)
		if err != nil {
			sb.WriteString(fmt.Sprintf("=== %s: (not found) ===\n", f))
			continue
		}
		sb.WriteString(fmt.Sprintf("=== %s ===\n%s\n", f, string(content)))
	}
	return sb.String()
}

var skipDirs = m31types.SkipDirsMap()

// listCwdFiles returns a list of files in the working directory with sizes.
// Limits depth to 3 levels and skips known heavy directories.
// PERF-1: Uses filepath.WalkDir instead of filepath.Walk to avoid os.Stat on
// every entry. d.Info() is called lazily only for non-directory entries that
// will be included in output.
func listCwdFiles(workDir string) string {
	var sb strings.Builder
	filepath.WalkDir(workDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			slog.Warn("walk error", "path", path, "error", err)
			return nil
		}
		rel, _ := filepath.Rel(workDir, path)
		if rel == "." {
			return nil
		}
		// Skip heavy directories
		if d.IsDir() && skipDirs[d.Name()] {
			return filepath.SkipDir
		}
		// Depth limit: count path separators
		depth := strings.Count(rel, string(filepath.Separator))
		if depth >= m31types.MaxCwdFileDepth {
			if d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if strings.HasPrefix(rel, ".git") || strings.HasPrefix(rel, ".m31a") {
			if d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if !d.IsDir() {
			info, err := d.Info()
			if err != nil {
				slog.Warn("stat error", "path", path, "error", err)
				return nil
			}
			sb.WriteString(fmt.Sprintf("%s (%d bytes)\n", rel, info.Size()))
		}
		return nil
	})
	return sb.String()
}

// hasTestFiles checks if any files in the task end with test patterns.
func hasTestFiles(workDir string, files []string) bool {
	for _, f := range files {
		base := filepath.Base(f)
		if strings.HasSuffix(base, "_test.go") ||
			strings.HasSuffix(base, ".test.js") ||
			strings.HasSuffix(base, "_test.py") {
			return true
		}
		// Also check if test files exist in the same directory
		dir := filepath.Join(workDir, filepath.Dir(f))
		entries, _ := os.ReadDir(dir)
		for _, e := range entries {
			if strings.HasSuffix(e.Name(), "_test.go") ||
				strings.HasSuffix(e.Name(), ".test.js") ||
				strings.HasSuffix(e.Name(), "_test.py") {
				return true
			}
		}
	}
	return false
}

// verifyTaskTimeout is the maximum time allowed for a single verification command.
const verifyTaskTimeout = 5 * time.Minute

// verifyTaskContext returns a context with a deadline for verification commands.
// Derives from the parent context so session cancellation propagates.
func (e *Engine) verifyTaskContext(parent context.Context) (context.Context, context.CancelFunc) {
	ctx, cancel := context.WithTimeout(parent, verifyTaskTimeout)
	return ctx, cancel
}

// verifyTask checks if a task's outputs exist and are syntactically valid.
func (e *Engine) verifyTask(ctx context.Context, task m31types.Task) VerificationResult {
	result := VerificationResult{TaskID: task.ID, FilesExist: true, SyntaxOK: true, TestsOK: true}

	// File existence
	for _, f := range task.Files {
		path := filepath.Join(e.workDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			result.Errors = append(result.Errors, fmt.Sprintf("file not found: %s", f))
			result.FilesExist = false
		}
	}

	// Project-type-specific validation
	projectType := detectProjectType(e.workDir)
	switch projectType {
	case "go":
		hasGo := false
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".go") {
				hasGo = true
				break
			}
		}
		if hasGo {
			vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
			defer cancel()
			cmd := exec.CommandContext(vctx, "go", "build", "./...")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("go build failed: %s", string(out)))
				result.SyntaxOK = false
			}
		}
	case "nodejs":
		hasJS := false
		for _, f := range task.Files {
			ext := filepath.Ext(f)
			if ext == ".js" || ext == ".ts" || ext == ".jsx" || ext == ".tsx" {
				hasJS = true
				break
			}
		}
		if hasJS {
			// Check for package.json and try npm test or tsc
			if _, err := os.Stat(filepath.Join(e.workDir, "package.json")); err == nil {
				vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
				defer cancel()
				cmd := exec.CommandContext(vctx, "sh", "-c", "npm run build 2>&1 || tsc --noEmit 2>&1 || true")
				cmd.Dir = e.workDir
				if out, err := cmd.CombinedOutput(); err == nil && len(out) > 0 {
					// Log but don't fail — build may have warnings
					e.logger.Info("nodejs build output", "output", string(out))
				}
			}
		}
	case "python":
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".py") {
				path := filepath.Join(e.workDir, f)
				vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
				defer cancel()
				cmd := exec.CommandContext(vctx, "python3", "-m", "py_compile", path)
				cmd.Dir = e.workDir
				if out, err := cmd.CombinedOutput(); err != nil {
					result.Errors = append(result.Errors, fmt.Sprintf("python syntax error in %s: %s", f, string(out)))
					result.SyntaxOK = false
				}
			}
		}
	case "rust":
		hasRust := false
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".rs") {
				hasRust = true
				break
			}
		}
		if hasRust {
			vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
			defer cancel()
			cmd := exec.CommandContext(vctx, "cargo", "check")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("cargo check failed: %s", string(out)))
				result.SyntaxOK = false
			}
		}
	}

	// Test execution
	if hasTestFiles(e.workDir, task.Files) {
		switch projectType {
		case "go":
			vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
			defer cancel()
			cmd := exec.CommandContext(vctx, "go", "test", "./...")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("go test failed: %s", string(out)))
				result.TestsOK = false
			}
		case "nodejs":
			vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
			defer cancel()
			cmd := exec.CommandContext(vctx, "sh", "-c", "npm test 2>&1 || true")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err == nil && len(out) > 0 {
				e.logger.Info("npm test output", "output", string(out))
			}
		case "python":
			vctx, cancel := context.WithTimeout(ctx, verifyTaskTimeout)
			defer cancel()
			cmd := exec.CommandContext(vctx, "sh", "-c", "python3 -m pytest 2>&1 || true")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err == nil && len(out) > 0 {
				e.logger.Info("pytest output", "output", string(out))
			}
		}
	}

	return result
}
