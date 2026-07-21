package codeintel

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

func TestIndexer_BuildAndQuery(t *testing.T) {
	dir := t.TempDir()

	writeFile(t, dir, "main.go", `package main

import (
	"fmt"
	"example.com/lib"
)

func main() {
	fmt.Println(lib.Greet())
}
`)

	writeFile(t, dir, "types.go", `package main

type Config struct {
	Name    string
	Verbose bool
}

type Handler interface {
	Handle(req string) error
}
`)

	writeFile(t, dir, "engine.go", `package main

import "fmt"

type Engine struct {
	Config Config
}

func NewEngine(cfg Config) *Engine {
	return &Engine{Config: cfg}
}

func (e *Engine) Run() error {
	fmt.Println("running")
	return nil
}
`)

	idx := NewIndexer(dir)
	if idx.IsBuilt() {
		t.Error("should not be built yet")
	}

	err := idx.Build(context.Background())
	if err != nil {
		t.Fatalf("Build: %v", err)
	}

	if !idx.IsBuilt() {
		t.Error("should be built now")
	}

	if idx.FileCount() != 3 {
		t.Errorf("FileCount: got %d, want 3", idx.FileCount())
	}

	if idx.SymbolCount() == 0 {
		t.Error("SymbolCount should be > 0")
	}

	locs := idx.Define("Engine")
	if len(locs) == 0 {
		t.Error("expected Engine to be defined")
	}

	syms := idx.FileSymbols("types.go")
	if len(syms) == 0 {
		t.Error("expected symbols in types.go")
	}

	relevant := idx.RelevantFiles([]string{"engine.go"}, "modify Config struct", 5)
	if len(relevant) == 0 {
		t.Error("expected relevant files")
	}

	ctx := idx.FormatContext([]string{"engine.go"}, "modify Config struct", 5, 5000)
	if ctx == "" {
		t.Error("expected non-empty context")
	}
	if !containsStr(ctx, "Recommended Files") {
		t.Error("context should contain 'Recommended Files'")
	}

	summary := idx.ProjectSummary(5000)
	if summary == "" {
		t.Error("expected non-empty summary")
	}
	if !containsStr(summary, "Codebase Overview") {
		t.Error("summary should contain 'Codebase Overview'")
	}
}

func TestIndexer_BuildEmptyDir(t *testing.T) {
	dir := t.TempDir()
	idx := NewIndexer(dir)
	err := idx.Build(context.Background())
	if err != nil {
		t.Fatalf("Build empty dir: %v", err)
	}
	if idx.FileCount() != 0 {
		t.Errorf("expected 0 files, got %d", idx.FileCount())
	}
}

func TestIndexer_BuildCancelled(t *testing.T) {
	dir := t.TempDir()
	idx := NewIndexer(dir)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	err := idx.Build(ctx)
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

func TestIndexer_QueryBeforeBuild(t *testing.T) {
	idx := NewIndexer("/nonexistent")

	if idx.FileCount() != 0 {
		t.Error("FileCount should be 0 before build")
	}
	if idx.SymbolCount() != 0 {
		t.Error("SymbolCount should be 0 before build")
	}
	if locs := idx.Define("X"); locs != nil {
		t.Error("Define should return nil before build")
	}
	if files := idx.RelevantFiles([]string{"a.go"}, "", 5); files != nil {
		t.Error("RelevantFiles should return nil before build")
	}
	if ctx := idx.FormatContext([]string{"a.go"}, "", 5, 1000); ctx != "" {
		t.Error("FormatContext should return empty before build")
	}
}

func TestIndexer_MultiLanguage(t *testing.T) {
	dir := t.TempDir()

	writeFile(t, dir, "main.go", `package main

import "fmt"

func main() {
	fmt.Println("hello")
}
`)

	writeFile(t, dir, "app.ts", `import { Config } from './config';

export function greet(name: string): string {
  return "hello " + name;
}
`)

	writeFile(t, dir, "config.ts", `export interface Config {
  port: number;
  host: string;
}
`)

	writeFile(t, dir, "main.py", `from pathlib import Path

def process(data: list) -> bool:
    return True
`)

	idx := NewIndexer(dir)
	err := idx.Build(context.Background())
	if err != nil {
		t.Fatalf("Build: %v", err)
	}

	if idx.FileCount() != 4 {
		t.Errorf("FileCount: got %d, want 4", idx.FileCount())
	}

	goDefined := idx.Define("main")
	foundGo := false
	for _, loc := range goDefined {
		if filepath.Ext(loc.File) == ".go" {
			foundGo = true
		}
	}
	if !foundGo {
		t.Error("expected main function in Go file")
	}

	tsDefined := idx.Define("greet")
	if len(tsDefined) == 0 {
		t.Error("expected greet function in TypeScript file")
	}

	pyDefined := idx.Define("process")
	if len(pyDefined) == 0 {
		t.Error("expected process function in Python file")
	}
}

func writeFile(t *testing.T, dir, name, content string) {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func containsStr(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsSubstr(s, substr))
}

func containsSubstr(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
