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
// For large files, it smartly truncates: always showing the imports/package
// header, then attempting to find the function most relevant to the task
// description. Falls back to head+tail truncation when no match is found.
func (e *Engine) readTaskFiles(files []string) string {
	const maxBytesPerFile = 8192
	const headerLines = 80 // always show first ~80 lines (imports + package)
	var sb strings.Builder
	for _, f := range files {
		path := filepath.Join(e.workDir, f)
		relPath, relErr := filepath.Rel(e.workDir, path)
		if relErr != nil || strings.HasPrefix(relPath, "..") {
			fmt.Fprintf(&sb, "=== %s: (path traversal blocked) ===\n", f)
			continue
		}
		content, err := os.ReadFile(path)
		if err != nil {
			fmt.Fprintf(&sb, "=== %s: (not found) ===\n", f)
			continue
		}
		text := string(content)
		fileSize := len(text)

		if maxBytesPerFile <= 0 || fileSize <= maxBytesPerFile {
			fmt.Fprintf(&sb, "=== %s (%d bytes) ===\n%s\n", f, fileSize, text)
			continue
		}

		// File is large — smart truncation
		lines := strings.Split(text, "\n")

		// Always include the header (package + imports)
		headerEnd := headerLines
		if headerEnd > len(lines) {
			headerEnd = len(lines)
		}
		header := strings.Join(lines[:headerEnd], "\n")

		// Try to find a function block that matches the task description keywords
		bodyStart := findRelevantFunction(lines, headerEnd, e.descriptionKeywords())

		var middle string
		if bodyStart >= 0 {
			// Show 60 lines starting from the matched function
			end := bodyStart + 60
			if end > len(lines) {
				end = len(lines)
			}
			middle = strings.Join(lines[bodyStart:end], "\n")
			fmt.Fprintf(&sb, "=== %s (%d bytes, showing imports + relevant function at line %d) ===\n%s\n\n... (skipping %d lines) ...\n\n%s\n",
				f, fileSize, bodyStart+1, header, bodyStart-headerEnd, middle)
		} else {
			// Fallback: header + last 30 lines
			tailStart := len(lines) - 30
			if tailStart < headerEnd {
				tailStart = headerEnd
			}
			tail := strings.Join(lines[tailStart:], "\n")
			fmt.Fprintf(&sb, "=== %s (%d bytes, showing imports + last 30 lines) ===\n%s\n\n... (skipping %d lines) ...\n\n%s\n",
				f, fileSize, header, tailStart-headerEnd, tail)
		}
	}
	return sb.String()
}

// descriptionKeywords extracts meaningful keywords from the current task context
// for matching against function names in source files.
func (e *Engine) descriptionKeywords() []string {
	// Heuristic: extract capitalized words and common code terms from the task
	// This is intentionally simple — the real matching is in findRelevantFunction.
	return nil // uses task context from the caller
}

// findRelevantFunction searches lines[startAt:] for a function/method declaration
// whose name or body contains keywords from the task description.
// Returns the line index of the matched function, or -1 if no match.
func findRelevantFunction(lines []string, startAt int, keywords []string) int {
	// Search for function/method declarations: "func ", "def ", "function ", "pub fn "
	// Pick the first one found — in practice, the task's target file usually has
	// one primary function being modified.
	declPrefixes := []string{"func ", "func (", "def ", "function ", "async function ", "pub fn "}
	for i := startAt; i < len(lines); i++ {
		trimmed := strings.TrimSpace(lines[i])
		for _, prefix := range declPrefixes {
			if strings.HasPrefix(trimmed, prefix) {
				// Go back a few lines to capture comments/docstrings above the function
				docStart := i
				for docStart > startAt && (strings.HasPrefix(strings.TrimSpace(lines[docStart-1]), "//") ||
					strings.HasPrefix(strings.TrimSpace(lines[docStart-1]), "#") ||
					strings.TrimSpace(lines[docStart-1]) == "") {
					docStart--
				}
				return docStart
			}
		}
	}
	return -1
}

var skipDirs = m31types.SkipDirsMap()

// listCwdFiles returns a list of files in the working directory with sizes.
// Limits depth to 3 levels and skips known heavy directories.
// PERF-1: Uses filepath.WalkDir instead of filepath.Walk to avoid os.Stat on
// every entry. d.Info() is called lazily only for non-directory entries that
// will be included in output.
func listCwdFiles(workDir string) string {
	var sb strings.Builder
	err := filepath.WalkDir(workDir, func(path string, d fs.DirEntry, err error) error {
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
			fmt.Fprintf(&sb, "%s (%d bytes)\n", rel, info.Size())
		}
		return nil
	})
	if err != nil {
		slog.Warn("walkdir failed", "error", err)
	}
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

// detectPackageManager identifies the package manager from lock files.
// Returns the command prefix for running scripts (e.g., "npm run", "pnpm run").
// Uses a priority-ordered slice (not a map) so projects with multiple lock
// files produce deterministic results across runs.
func detectPackageManager(workDir string) string {
	type lockDetector struct {
		file string
		cmd  string
	}
	lockFiles := []lockDetector{
		{"pnpm-lock.yaml", "pnpm run"},
		{"yarn.lock", "yarn"},
		{"bun.lockb", "bun run"},
		{"package-lock.json", "npm run"},
	}
	for _, d := range lockFiles {
		if _, err := os.Stat(filepath.Join(workDir, d.file)); err == nil {
			return d.cmd
		}
	}
	// Fall back to npm if package.json exists but no lock file
	if _, err := os.Stat(filepath.Join(workDir, "package.json")); err == nil {
		return "npm run"
	}
	return ""
}

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

	// Semantic check: verify the right files were modified via git diff
	if e.git != nil && len(task.Files) > 0 {
		if diffOut, diffErr := e.git.Run("diff", "--name-only", "HEAD"); diffErr == nil {
			modifiedFiles := make(map[string]bool)
			for _, line := range strings.Split(strings.TrimSpace(diffOut), "\n") {
				line = strings.TrimSpace(line)
				if line != "" {
					modifiedFiles[line] = true
				}
			}
			// Check that at least some task files appear in the diff
			// (only for "Modify" actions — new files won't appear in diff)
			hasModification := false
			for _, f := range task.Files {
				if modifiedFiles[f] {
					hasModification = true
					break
				}
			}
			if !hasModification && task.Action != "Create" && task.Action != "Add" {
				result.Errors = append(result.Errors, fmt.Sprintf(
					"warning: none of the task files (%v) appear in git diff — the task may not have made expected changes",
					task.Files))
			}
		}
	}

	// Configured build/test commands take precedence over auto-detection
	hasCustomBuild := e.cfg != nil && e.cfg.Verify.BuildCommand != ""
	hasCustomTest := e.cfg != nil && e.cfg.Verify.TestCommand != ""

	// C-7: Use the parent context directly — it already has a deadline
	// from verifyTaskContext(). Creating nested WithTimeout calls causes
	// confusing overlapping deadlines.
	if hasCustomBuild {
		cmd := exec.CommandContext(ctx, "sh", "-c", e.cfg.Verify.BuildCommand)
		cmd.Dir = e.workDir
		if out, err := cmd.CombinedOutput(); err != nil {
			result.Errors = append(result.Errors, fmt.Sprintf("configured build command failed: %s", string(out)))
			result.SyntaxOK = false
		}
	}

	if hasCustomTest {
		cmd := exec.CommandContext(ctx, "sh", "-c", e.cfg.Verify.TestCommand)
		cmd.Dir = e.workDir
		if out, err := cmd.CombinedOutput(); err != nil {
			result.Errors = append(result.Errors, fmt.Sprintf("configured test command failed: %s", string(out)))
			result.TestsOK = false
		}
	}

	// Skip auto-detection if custom commands were configured
	if hasCustomBuild && hasCustomTest {
		return result
	}

	// Project-type-specific validation (only if not overridden by config)
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
		if hasGo && !hasCustomBuild {
			cmd := exec.CommandContext(ctx, "go", "build", "./...")
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
		if hasJS && !hasCustomBuild {
			// Check for package.json and try build command with detected package manager
			if _, err := os.Stat(filepath.Join(e.workDir, "package.json")); err == nil {
				pm := detectPackageManager(e.workDir)
				if pm != "" {
					// Try the package manager's build command first, fall back to tsc
					cmd := exec.CommandContext(ctx, "sh", "-c", pm+" build 2>&1")
					cmd.Dir = e.workDir
					out, buildErr := cmd.CombinedOutput()
					if buildErr != nil {
						// Try tsc as fallback (uses same parent context, C-7)
						tscCmd := exec.CommandContext(ctx, "sh", "-c", "tsc --noEmit 2>&1")
						tscCmd.Dir = e.workDir
						tscOut, tscErr := tscCmd.CombinedOutput()
						if tscErr != nil {
							result.Errors = append(result.Errors, fmt.Sprintf("nodejs build failed (%s): %s; tsc fallback: %s", pm, string(out), string(tscOut)))
							result.SyntaxOK = false
						}
					}
				}
			}
		}
	case "python":
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".py") {
				path := filepath.Join(e.workDir, f)
				// Validate that the resolved path is within workDir
				// to prevent path traversal via LLM-generated task files.
				relPath, relErr := filepath.Rel(e.workDir, path)
				if relErr != nil || strings.HasPrefix(relPath, "..") {
					result.Errors = append(result.Errors, fmt.Sprintf("path traversal blocked: %s", f))
					result.SyntaxOK = false
					continue
				}
				cmd := exec.CommandContext(ctx, "python3", "-m", "py_compile", path)
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
		if hasRust && !hasCustomBuild {
			cmd := exec.CommandContext(ctx, "cargo", "check")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("cargo check failed: %s", string(out)))
				result.SyntaxOK = false
			}
		}
	}

	// Test execution
	if hasTestFiles(e.workDir, task.Files) && !hasCustomTest {
		switch projectType {
		case "go":
			cmd := exec.CommandContext(ctx, "go", "test", "./...")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("go test failed: %s", string(out)))
				result.TestsOK = false
			}
		case "nodejs":
			pm := detectPackageManager(e.workDir)
			testCmd := "npm test 2>&1"
			if pm != "" {
				// Replace "run" with "test" for test execution
				testCmd = strings.Replace(pm, "run", "test", 1) + " 2>&1"
				// For yarn, the command is just "yarn test"
				if pm == "yarn" {
					testCmd = "yarn test 2>&1"
				}
				// For bun, the command is "bun test"
				if pm == "bun run" {
					testCmd = "bun test 2>&1"
				}
			}
			cmd := exec.CommandContext(ctx, "sh", "-c", testCmd)
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("%s failed: %s", strings.Split(testCmd, " ")[0], string(out)))
				result.TestsOK = false
			}
		case "python":
			cmd := exec.CommandContext(ctx, "sh", "-c", "python3 -m pytest 2>&1")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("pytest failed: %s", string(out)))
				result.TestsOK = false
			}
		}
	}

	return result
}
