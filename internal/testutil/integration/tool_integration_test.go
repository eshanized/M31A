// Package integration_test contains cross-package integration tests.
package integration_test

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// Edit.Execute() integration tests — real file I/O through the full pipeline
// ---------------------------------------------------------------------------

func TestEdit_Execute_ExactMatch_FileIO(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n"
	filePath := filepath.Join(dir, "main.go")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "main.go",
			"old_string": "fmt.Println(\"hello\")",
			"new_string": "fmt.Println(\"world\")",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	if !strings.Contains(string(got), "world") {
		t.Errorf("file content = %q, want contains 'world'", string(got))
	}
	if strings.Contains(string(got), "hello") {
		t.Errorf("file still contains 'hello'")
	}
}

func TestEdit_Execute_ReplaceAll(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "aaa bbb aaa ccc aaa"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":        "file.txt",
			"old_string":  "aaa",
			"new_string":  "zzz",
			"replace_all": true,
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	if string(got) != "zzz bbb zzz ccc zzz" {
		t.Errorf("file content = %q, want 'zzz bbb zzz ccc zzz'", string(got))
	}
}

func TestEdit_Execute_CRLF_RoundTrip(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "line1\r\nline2\r\nline3\r\n"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "line2",
			"new_string": "LINE2",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	gotStr := string(got)
	if !strings.Contains(gotStr, "LINE2") {
		t.Errorf("file content = %q, want contains 'LINE2'", gotStr)
	}
	if !strings.Contains(gotStr, "\r\n") {
		t.Errorf("CRLF endings not preserved, got %q", gotStr)
	}
}

func TestEdit_Execute_PathTraversal(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	// Create a file outside the workDir
	outsideDir := filepath.Join(dir, "outside")
	os.MkdirAll(outsideDir, 0755)
	os.WriteFile(filepath.Join(outsideDir, "secret.txt"), []byte("secret"), 0644)

	edit := tools.NewEdit(dir, backupDir)
	_, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "../outside/secret.txt",
			"old_string": "secret",
			"new_string": "pwned",
		},
	})
	if err == nil {
		t.Fatal("expected error for path traversal, got nil")
	}
}

func TestEdit_Execute_BinaryRejection(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte("hello world"), 0644)

	edit := tools.NewEdit(dir, backupDir)
	_, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "hello",
			"new_string": "hel\x00lo",
		},
	})
	if err == nil {
		t.Fatal("expected error for binary content, got nil")
	}
}

func TestEdit_Execute_ContextCancellation(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte("hello"), 0644)

	edit := tools.NewEdit(dir, backupDir)
	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	_, err := edit.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "hello",
			"new_string": "world",
		},
	})
	if err == nil {
		t.Fatal("expected error for cancelled context, got nil")
	}
}

func TestEdit_Execute_Integration_LineRange(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "line1\nline2\nline3\nline4\nline5"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"new_string": "REPLACED",
			"start_line": 2,
			"end_line":   4,
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	expected := "line1\nREPLACED\nline5"
	if string(got) != expected {
		t.Errorf("file content = %q, want %q", string(got), expected)
	}
}

func TestEdit_Execute_Integration_Fuzzy(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "func foo() {\n\treturn 1\n}\n"
	filePath := filepath.Join(dir, "file.go")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	// Use a slightly different old_string to trigger fuzzy match
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.go",
			"old_string": "func foo() {\n\treturn 2\n}", // line 2 differs
			"new_string": "func foo() {\n\treturn 42\n}",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	// Should succeed via fuzzy anchor match
	if result.Error != "" {
		t.Fatalf("Tool error (fuzzy match should have succeeded): %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	if !strings.Contains(string(got), "return 42") {
		t.Errorf("file content = %q, want contains 'return 42'", string(got))
	}
}

func TestEdit_Execute_NoMatchReturnsError(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "hello world"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "nonexistent_string",
			"new_string": "replacement",
		},
	})
	if err != nil {
		t.Fatalf("Execute returned Go error: %v", err)
	}
	// Error should be in ToolResult.Output, not a Go error
	if result.Error == "" && result.Output == "" {
		t.Error("expected error in ToolResult.Output")
	}
	// File should be unchanged
	got, _ := os.ReadFile(filePath)
	if string(got) != content {
		t.Errorf("file was modified despite no match: %q", string(got))
	}
}

func TestEdit_Execute_BackupCreated(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte("original"), 0644)

	edit := tools.NewEdit(dir, backupDir)
	_, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "original",
			"new_string": "modified",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}

	// Check backup was created
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		t.Fatalf("failed to read backup dir: %v", err)
	}
	// Filter for .bak files
	var bakFiles []string
	for _, e := range entries {
		if strings.HasSuffix(e.Name(), ".bak") {
			bakFiles = append(bakFiles, e.Name())
		}
	}
	if len(bakFiles) == 0 {
		t.Error("expected backup file to be created")
	}
}

func TestEdit_Execute_BackupPruning(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte("v0"), 0644)

	edit := tools.NewEdit(dir, backupDir)
	// Edit 15 times to trigger pruning (MaxBackupsPerFile = 10)
	for i := 0; i < 15; i++ {
		content, _ := os.ReadFile(filePath)
		newContent := strings.Replace(string(content), "v"+string(rune('0'+i%10)), "v"+string(rune('0'+(i+1)%10)), 1)
		os.WriteFile(filePath, []byte(newContent), 0644)
		_, err := edit.Execute(context.Background(), types.ToolInput{
			Params: map[string]any{
				"path":       "file.txt",
				"old_string": newContent,
				"new_string": "placeholder" + string(rune('A'+i)),
			},
		})
		if err != nil {
			t.Fatalf("Edit %d failed: %v", i, err)
		}
	}

	entries, err := os.ReadDir(backupDir)
	if err != nil {
		t.Fatalf("failed to read backup dir: %v", err)
	}
	var bakCount int
	for _, e := range entries {
		if strings.HasSuffix(e.Name(), ".bak") {
			bakCount++
		}
	}
	if bakCount > 10 {
		t.Errorf("backup count = %d, want <= 10", bakCount)
	}
}

func TestEdit_Execute_TempFileCleanup(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte("hello"), 0644)

	edit := tools.NewEdit(dir, backupDir)
	_, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "hello",
			"new_string": "world",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}

	// Check no temp files remain
	entries, _ := os.ReadDir(dir)
	for _, e := range entries {
		if strings.HasPrefix(e.Name(), ".m31a_tmp_") {
			t.Errorf("temp file not cleaned up: %s", e.Name())
		}
	}
}

func TestEdit_Execute_IndentNormalized(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	// Content with mixed tabs and spaces
	content := "func test() {\n\treturn 1\n}\n"
	filePath := filepath.Join(dir, "file.go")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	// old_string uses spaces instead of tabs
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.go",
			"old_string": "func test() {\n    return 1\n}",
			"new_string": "func test() {\n    return 42\n}",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	if !strings.Contains(string(got), "return 42") {
		t.Errorf("file content = %q, want contains 'return 42'", string(got))
	}
}

func TestEdit_Execute_BlankLineHandling(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	// Content WITH blank lines matching old_string
	content := "line1\nline2\nline3\n"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	// old_string matches content exactly (no blank lines in either)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "line1\nline2\nline3",
			"new_string": "A\nB\nC",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	if !strings.Contains(string(got), "A\nB\nC") {
		t.Errorf("file content = %q, want contains 'A\\nB\\nC'", string(got))
	}
}

func TestEdit_Execute_OldStringAndLineRange(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "line1\nline2\nline3\n"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	// Both old_string and line range provided — line range should take priority
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "line1",
			"new_string": "REPLACED_BY_LINE_RANGE",
			"start_line": 2,
			"end_line":   2,
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}

	got, _ := os.ReadFile(filePath)
	expected := "line1\nREPLACED_BY_LINE_RANGE\nline3\n"
	if string(got) != expected {
		t.Errorf("file content = %q, want %q", string(got), expected)
	}
}

func TestEdit_Execute_LargeFileRejection(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	// Create a file exceeding MaxFileSize (5MB)
	filePath := filepath.Join(dir, "large.txt")
	bigContent := strings.Repeat("x", types.MaxFileSize+1)
	os.WriteFile(filePath, []byte(bigContent), 0644)

	edit := tools.NewEdit(dir, backupDir)
	_, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "large.txt",
			"old_string": "x",
			"new_string": "y",
		},
	})
	if err == nil {
		t.Fatal("expected error for oversized file, got nil")
	}
}

func TestEdit_Execute_StrategyReported(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	os.MkdirAll(backupDir, 0755)

	content := "hello world"
	filePath := filepath.Join(dir, "file.txt")
	os.WriteFile(filePath, []byte(content), 0644)

	edit := tools.NewEdit(dir, backupDir)
	result, err := edit.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "file.txt",
			"old_string": "hello",
			"new_string": "goodbye",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Tool error: %s", result.Error)
	}
	// Output should contain strategy name
	if !strings.Contains(result.Output, "exact-match") {
		t.Errorf("output = %q, want contains 'exact-match'", result.Output)
	}
}
