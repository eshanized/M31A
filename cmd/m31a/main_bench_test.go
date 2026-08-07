package main

import (
	"os/exec"
	"path/filepath"
	"runtime"
	"testing"
)

func BenchmarkStartupVersion(b *testing.B) {
	binary := buildBinary(b)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		cmd := exec.Command(binary, "--version")
		if err := cmd.Run(); err != nil {
			b.Fatalf("--version failed: %v", err)
		}
	}
}

func BenchmarkStartupLazy(b *testing.B) {
	binary := buildBinary(b)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		cmd := exec.Command(binary, "--version")
		if err := cmd.Run(); err != nil {
			b.Fatalf("--version failed: %v", err)
		}
	}
}

func buildBinary(b *testing.B) string {
	b.Helper()
	binary := filepath.Join(b.TempDir(), "m31a-test")
	if runtime.GOOS == "windows" {
		binary += ".exe"
	}
	cmd := exec.Command("go", "build", "-o", binary, "./cmd/m31a")
	cmd.Dir = filepath.Join("..", "..")
	out, err := cmd.CombinedOutput()
	if err != nil {
		b.Fatalf("build failed: %v\n%s", err, out)
	}
	return binary
}
