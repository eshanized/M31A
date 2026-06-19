package workflow

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDetectNodeFramework(t *testing.T) {
	dir := t.TempDir()

	// Write package.json with Next.js
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{
		"dependencies": {
			"next": "^14.0.0",
			"react": "^18.0.0"
		}
	}`), 0644)

	fw := detectNodeFramework(dir)
	if fw != "Next.js" {
		t.Errorf("detectNodeFramework() = %q, want Next.js", fw)
	}
}

func TestDetectNodeFrameworkExpress(t *testing.T) {
	dir := t.TempDir()

	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{
		"dependencies": {
			"express": "^4.18.0"
		}
	}`), 0644)

	fw := detectNodeFramework(dir)
	if fw != "Express" {
		t.Errorf("detectNodeFramework() = %q, want Express", fw)
	}
}

func TestDetectGoFramework(t *testing.T) {
	dir := t.TempDir()

	os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module example.com/app
go 1.21
require (
	github.com/gin-gonic/gin v1.9.0
)
`), 0644)

	fw := detectGoFramework(dir)
	if fw != "Gin" {
		t.Errorf("detectGoFramework() = %q, want Gin", fw)
	}
}

func TestDetectGoFrameworkNone(t *testing.T) {
	dir := t.TempDir()

	os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module example.com/app
go 1.21
`), 0644)

	fw := detectGoFramework(dir)
	if fw != "Go" {
		t.Errorf("detectGoFramework() = %q, want Go", fw)
	}
}

func TestDetectPythonFramework(t *testing.T) {
	dir := t.TempDir()

	os.WriteFile(filepath.Join(dir, "requirements.txt"), []byte(`fastapi==0.100.0
uvicorn==0.23.0
`), 0644)

	fw := detectPythonFramework(dir)
	if fw != "FastAPI" {
		t.Errorf("detectPythonFramework() = %q, want FastAPI", fw)
	}
}

func TestCountNodeDeps(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "package.json"), []byte(`{
		"dependencies": {"react": "18", "next": "14"},
		"devDependencies": {"typescript": "5"}
	}`), 0644)

	count := countNodeDeps(dir)
	if count != 3 {
		t.Errorf("countNodeDeps() = %d, want 3", count)
	}
}

func TestCountGoDeps(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module test
go 1.21
require (
	github.com/a/b v1.0.0
	github.com/c/d v2.0.0
	golang.org/x/sync v0.5.0
)
`), 0644)

	count := countGoDeps(dir)
	if count != 3 {
		t.Errorf("countGoDeps() = %d, want 3", count)
	}
}

func TestCalculateTestRatio(t *testing.T) {
	dir := t.TempDir()

	// Create source files
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(dir, "util.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(dir, "main_test.go"), []byte("package main"), 0644)

	ratio := calculateTestRatio(dir)
	// 1 test file out of 3 total = 0.333
	if ratio < 0.3 || ratio > 0.4 {
		t.Errorf("calculateTestRatio() = %f, want ~0.33", ratio)
	}
}

func TestCalculateTestRatioEmpty(t *testing.T) {
	dir := t.TempDir()
	ratio := calculateTestRatio(dir)
	if ratio != 0 {
		t.Errorf("calculateTestRatio() on empty dir = %f, want 0", ratio)
	}
}

func TestCalculateHealthScore(t *testing.T) {
	tests := []struct {
		name    string
		a       ProjectAnalysis
		minScore int
		maxScore int
	}{
		{
			name:     "good project",
			a:        ProjectAnalysis{TestFileRatio: 0.3, DependencyCount: 20, Framework: "Next.js", FileCount: 50},
			minScore: 70,
			maxScore: 100,
		},
		{
			name:     "no tests",
			a:        ProjectAnalysis{TestFileRatio: 0, DependencyCount: 10, FileCount: 20},
			minScore: 40,
			maxScore: 60,
		},
		{
			name:     "too many deps",
			a:        ProjectAnalysis{TestFileRatio: 0.1, DependencyCount: 150, FileCount: 100},
			minScore: 40,
			maxScore: 70,
		},
		{
			name:     "tiny project",
			a:        ProjectAnalysis{TestFileRatio: 0, DependencyCount: 0, FileCount: 2},
			minScore: 0,
			maxScore: 50,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			score := calculateHealthScore(tt.a)
			if score < tt.minScore || score > tt.maxScore {
				t.Errorf("calculateHealthScore() = %d, want [%d, %d]", score, tt.minScore, tt.maxScore)
			}
		})
	}
}

func TestDetectLanguage(t *testing.T) {
	tests := []struct {
		projectType string
		want        string
	}{
		{"go", "Go"},
		{"nodejs", "TypeScript/JavaScript"},
		{"python", "Python"},
		{"rust", "Rust"},
		{"unknown", "Unknown"},
	}

	for _, tt := range tests {
		got := detectLanguage(tt.projectType)
		if got != tt.want {
			t.Errorf("detectLanguage(%q) = %q, want %q", tt.projectType, got, tt.want)
		}
	}
}
