package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestCodeComplexity_Name(t *testing.T) {
	t.Parallel()
	cc := NewCodeComplexity(t.TempDir(), nil)
	if cc.Name() != "CodeComplexity" {
		t.Errorf("expected 'CodeComplexity', got %s", cc.Name())
	}
}

func TestCodeComplexity_Description(t *testing.T) {
	t.Parallel()
	cc := NewCodeComplexity(t.TempDir(), nil)
	if cc.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestCodeComplexity_RiskLevel(t *testing.T) {
	t.Parallel()
	cc := NewCodeComplexity(t.TempDir(), nil)
	if cc.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", cc.RiskLevel())
	}
}

func TestCodeComplexity_ParameterSchema(t *testing.T) {
	t.Parallel()
	cc := NewCodeComplexity(t.TempDir(), nil)
	schema := cc.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
}

func TestCodeComplexity_EmptyDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output")
	}
	if !strings.Contains(result.Output, "Source files") {
		t.Error("expected 'Source files' in output")
	}
}

func TestCodeComplexity_CountsGoFiles(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	createFile(t, filepath.Join(dir, "main.go"), "package main\n\nfunc main() {}\n")
	createFile(t, filepath.Join(dir, "util.go"), "package main\n\nfunc helper() {}\n")
	// Test file — should be excluded
	createFile(t, filepath.Join(dir, "main_test.go"), "package main\n\nfunc TestMain(t *testing.T) {}\n")
	// Non-Go file — should be excluded
	createFile(t, filepath.Join(dir, "README.md"), "# Hello\n")

	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "2") {
		t.Errorf("expected '2' in output for 2 source files, output:\n%s", result.Output)
	}
}

func TestCodeComplexity_SkipsTestFiles(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	createFile(t, filepath.Join(dir, "app.go"), "package app\n\nfunc Start() {}\n")
	createFile(t, filepath.Join(dir, "app_test.go"), "package app\n\nfunc TestStart(t *testing.T) {}\n")

	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "1") {
		t.Errorf("expected '1' for 1 source file (test excluded), output:\n%s", result.Output)
	}
}

func TestCodeComplexity_SkipsWorktrees(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	createFile(t, filepath.Join(dir, "app.go"), "package app\n\nfunc Start() {}\n")
	wtDir := filepath.Join(dir, ".m31a-worktrees", "abc123")
	if err := os.MkdirAll(wtDir, 0o755); err != nil {
		t.Fatal(err)
	}
	createFile(t, filepath.Join(wtDir, "hidden.go"), "package hidden\n\nfunc X() {}\n")

	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "1") {
		t.Errorf("expected '1' for 1 source file (worktrees excluded), output:\n%s", result.Output)
	}
}

func TestCodeComplexity_SkipsDefaultDirs(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	createFile(t, filepath.Join(dir, "app.go"), "package app\n\nfunc Start() {}\n")
	vendorDir := filepath.Join(dir, "vendor")
	if err := os.MkdirAll(vendorDir, 0o755); err != nil {
		t.Fatal(err)
	}
	createFile(t, filepath.Join(vendorDir, "dep.go"), "package dep\n\nfunc Dep() {}\n")

	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "1") {
		t.Errorf("expected '1' for 1 source file (vendor excluded), output:\n%s", result.Output)
	}
}

func TestCodeComplexity_ExtraSkipDirsViaParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	createFile(t, filepath.Join(dir, "app.go"), "package app\n\nfunc Start() {}\n")
	customDir := filepath.Join(dir, "custom")
	if err := os.MkdirAll(customDir, 0o755); err != nil {
		t.Fatal(err)
	}
	createFile(t, filepath.Join(customDir, "stuff.go"), "package stuff\n\nfunc Stuff() {}\n")

	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"skip_dirs": []any{"custom"},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "1") {
		t.Errorf("expected '1' for 1 source file (custom excluded via param), output:\n%s", result.Output)
	}
}

func TestCodeComplexity_ConstructorSkipDirs(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	createFile(t, filepath.Join(dir, "app.go"), "package app\n\nfunc Start() {}\n")
	specialDir := filepath.Join(dir, "special")
	if err := os.MkdirAll(specialDir, 0o755); err != nil {
		t.Fatal(err)
	}
	createFile(t, filepath.Join(specialDir, "code.go"), "package special\n\nfunc Go() {}\n")

	cc := NewCodeComplexity(dir, []string{"special"})
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "1") {
		t.Errorf("expected '1' for 1 source file (special excluded via constructor), output:\n%s", result.Output)
	}
}

func TestCodeComplexity_TopPackages(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	// Create a "big" package with many lines
	bigDir := filepath.Join(dir, "big")
	if err := os.MkdirAll(bigDir, 0o755); err != nil {
		t.Fatal(err)
	}
	lines := strings.Repeat("func F() {}\n", 50)
	createFile(t, filepath.Join(bigDir, "file1.go"), "package big\n"+lines)
	createFile(t, filepath.Join(bigDir, "file2.go"), "package big\n"+lines)

	// Create a "small" package
	smallDir := filepath.Join(dir, "small")
	if err := os.MkdirAll(smallDir, 0o755); err != nil {
		t.Fatal(err)
	}
	createFile(t, filepath.Join(smallDir, "file.go"), "package small\n")

	cc := NewCodeComplexity(dir, nil)
	result, err := cc.Execute(context.Background(), types.ToolInput{Params: map[string]any{}})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !strings.Contains(result.Output, "Top") {
		t.Errorf("expected 'Top' in output for packages section, output:\n%s", result.Output)
	}
	if !strings.Contains(result.Output, "big") {
		t.Errorf("expected 'big' package in output, output:\n%s", result.Output)
	}
}

func TestCodeComplexity_ComplexityScore(t *testing.T) {
	t.Parallel()
	tests := []struct {
		lines int
		want  string
	}{
		{0, "simple (< 10K lines)"},
		{5000, "simple (< 10K lines)"},
		{9999, "simple (< 10K lines)"},
		{10000, "moderate (10K\u201350K lines)"},
		{30000, "moderate (10K\u201350K lines)"},
		{49999, "moderate (10K\u201350K lines)"},
		{50000, "complex (50K+ lines)"},
		{100000, "complex (50K+ lines)"},
	}
	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			t.Parallel()
			got := complexityScore(tt.lines)
			if got != tt.want {
				t.Errorf("complexityScore(%d) = %q, want %q", tt.lines, got, tt.want)
			}
		})
	}
}

func TestCodeComplexity_FormatNumber(t *testing.T) {
	t.Parallel()
	tests := []struct {
		n    int
		want string
	}{
		{0, "0"},
		{999, "999"},
		{1000, "1,000"},
		{12345, "12,345"},
		{1234567, "1,234,567"},
	}
	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			t.Parallel()
			got := formatNumber(tt.n)
			if got != tt.want {
				t.Errorf("formatNumber(%d) = %q, want %q", tt.n, got, tt.want)
			}
		})
	}
}

func TestCodeComplexity_FormatComplexityReport(t *testing.T) {
	t.Parallel()
	r := &ComplexityReport{
		TotalFiles: 42,
		TotalLines: 12345,
		Packages: []PackageStat{
			{Path: "internal/tools", Files: 10, Lines: 5000},
			{Path: "internal/tui", Files: 20, Lines: 3000},
		},
		Files: []FileStat{
			{Path: "internal/tools/edit.go", Lines: 1000},
			{Path: "internal/tui/repl.go", Lines: 800},
		},
		Score: "moderate (10K\u201350K lines)",
	}

	output := formatComplexityReport(r)

	checks := []string{
		"Complexity Report",
		"Source files",
		"42",
		"12,345",
		"moderate",
		"internal/tools",
		"edit.go",
		"Top 2 Packages",
		"Top 2 Files",
	}
	for _, check := range checks {
		if !strings.Contains(output, check) {
			t.Errorf("expected %q in output, got:\n%s", check, output)
		}
	}
}

func TestCodeComplexity_ContextCancellation(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	createFile(t, filepath.Join(dir, "app.go"), "package app\n")

	cc := NewCodeComplexity(dir, nil)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	// On a small dir the walk may complete before the cancel check fires,
	// so we just verify it doesn't panic.
	_, _ = cc.Execute(ctx, types.ToolInput{Params: map[string]any{}})
}

func TestCodeComplexity_InterfaceCompliance(t *testing.T) {
	t.Parallel()
	cc := NewCodeComplexity(t.TempDir(), nil)
	var _ types.Tool = cc
}

// createFile writes a file at the given path, creating parent directories.
func createFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
