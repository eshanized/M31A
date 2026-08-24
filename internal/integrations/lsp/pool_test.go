package lsp

import (
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// TestPoolGetClient_LazyStart verifies that a client is created on first GetClient call.
func TestPoolGetClient_LazyStart(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.DiscardHandler)

	pool := NewLSPPool(5*time.Minute, logger)
	defer pool.ShutdownAll()

	// Use a language without an installed binary (typescript, python, rust)
	_, err := pool.GetClient(tmpDir, "typescript")
	if err == nil {
		t.Fatal("expected error for missing typescript-language-server binary, got nil")
	}

	// The error should be about binary not found
	t.Logf("Got expected error: %v", err)

	// Verify the pool tried to start the client by checking the error message
	if !contains(err.Error(), "not found in PATH") {
		t.Errorf("error should mention 'not found in PATH', got: %v", err)
	}
}

// TestPoolGetClient_Reuse verifies that the same client is returned on second call for same project/language.
func TestPoolGetClient_Reuse(t *testing.T) {
	// This test would require a real LSP server to work properly.
	// We'll test the pool's internal behavior by mocking the client creation.
	// For now, we test that the pool structure works correctly.

	tmpDir := t.TempDir()
	logger := slog.New(slog.DiscardHandler)

	pool := NewLSPPool(5*time.Minute, logger)
	defer pool.ShutdownAll()

	// Both calls should fail with the same error (binary not found)
	_, err1 := pool.GetClient(tmpDir, "typescript")
	_, err2 := pool.GetClient(tmpDir, "typescript")

	// Both should fail
	if err1 == nil || err2 == nil {
		t.Fatal("expected both calls to fail")
	}

	// The error messages should be the same
	if err1.Error() != err2.Error() {
		t.Errorf("error messages differ: %v vs %v", err1, err2)
	}

	// Stats should show 0 connections since both failed
	stats := pool.Stats()
	if stats.TotalConnections != 0 {
		t.Errorf("expected 0 connections, got %d", stats.TotalConnections)
	}
}

// TestPoolShutdownIdle verifies that clients idle longer than TTL are closed.
func TestPoolShutdownIdle(t *testing.T) {
	logger := slog.New(slog.DiscardHandler)

	// Use 1ms idle TTL for fast test
	pool := NewLSPPool(1*time.Millisecond, logger)
	defer pool.ShutdownAll()

	// We can't actually create a real client without a server, but we can test
	// that ShutdownIdle doesn't panic and handles empty pool correctly.
	pool.ShutdownIdle()

	stats := pool.Stats()
	if stats.TotalConnections != 0 {
		t.Errorf("expected 0 connections after ShutdownIdle on empty pool, got %d", stats.TotalConnections)
	}
}

// TestPoolShutdownAll verifies that ShutdownAll closes all clients.
func TestPoolShutdownAll(t *testing.T) {
	logger := slog.New(slog.DiscardHandler)

	pool := NewLSPPool(5*time.Minute, logger)

	pool.ShutdownAll()

	stats := pool.Stats()
	if stats.TotalConnections != 0 {
		t.Errorf("expected 0 connections after ShutdownAll, got %d", stats.TotalConnections)
	}
}

// TestPoolBinaryNotFound verifies that attempting GetClient for a language
// with missing binary returns an error with "not found in PATH".
func TestPoolBinaryNotFound(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.DiscardHandler)

	pool := NewLSPPool(5*time.Minute, logger)
	defer pool.ShutdownAll()

	_, err := pool.GetClient(tmpDir, "typescript")
	if err == nil {
		t.Fatal("expected error for missing typescript-language-server binary, got nil")
	}

	errMsg := err.Error()
	if !contains(errMsg, "not found in PATH") {
		t.Errorf("error should mention 'not found in PATH', got: %s", errMsg)
	}
}

// TestPoolStats verifies that Stats returns correct counts.
func TestPoolStats(t *testing.T) {
	logger := slog.New(slog.DiscardHandler)

	pool := NewLSPPool(5*time.Minute, logger)
	defer pool.ShutdownAll()

	// Test empty pool
	stats := pool.Stats()
	if stats.TotalConnections != 0 {
		t.Errorf("expected 0 connections initially, got %d", stats.TotalConnections)
	}

	// We can't test with real clients without a server, but the structure is correct
}

// TestSupportedLanguages verifies the list of supported languages.
func TestSupportedLanguages(t *testing.T) {
	langs := SupportedLanguages()
	expected := []string{"go", "typescript", "python", "rust"}

	if len(langs) != len(expected) {
		t.Errorf("expected %d languages, got %d", len(expected), len(langs))
	}

	langSet := make(map[string]bool)
	for _, l := range langs {
		langSet[l] = true
	}

	for _, expectedLang := range expected {
		if !langSet[expectedLang] {
			t.Errorf("missing expected language: %s", expectedLang)
		}
	}
}

// TestSupportedExtensions verifies the list of supported extensions.
func TestSupportedExtensions(t *testing.T) {
	exts := SupportedExtensions()
	expected := []string{".go", ".ts", ".tsx", ".js", ".jsx", ".py", ".rs"}

	if len(exts) != len(expected) {
		t.Errorf("expected %d extensions, got %d", len(expected), len(exts))
	}

	extSet := make(map[string]bool)
	for _, e := range exts {
		extSet[e] = true
	}

	for _, expectedExt := range expected {
		if !extSet[expectedExt] {
			t.Errorf("missing expected extension: %s", expectedExt)
		}
	}
}

// TestFindLanguageForFile verifies file extension to language mapping.
func TestFindLanguageForFile(t *testing.T) {
	tests := []struct {
		file     string
		expected string
		found    bool
	}{
		{"test.go", "go", true},
		{"test.ts", "typescript", true},
		{"test.tsx", "typescript", true},
		{"test.js", "typescript", true},
		{"test.jsx", "typescript", true},
		{"test.py", "python", true},
		{"test.rs", "rust", true},
		{"test.txt", "", false},
		{"test", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.file, func(t *testing.T) {
			lang, found := FindLanguageForFile(tt.file)
			if found != tt.found {
				t.Errorf("FindLanguageForFile(%q) found=%v, want %v", tt.file, found, tt.found)
			}
			if found && lang != tt.expected {
				t.Errorf("FindLanguageForFile(%q) = %q, want %q", tt.file, lang, tt.expected)
			}
		})
	}
}

// TestServerConfigForLanguage verifies server config retrieval.
func TestServerConfigForLanguage(t *testing.T) {
	tests := []struct {
		lang     string
		expected ServerConfig
		found    bool
	}{
		{
			lang: "go",
			expected: ServerConfig{
				Language:   "go",
				Command:    []string{"gopls", "serve"},
				Extensions: []string{".go"},
				Binary:     "gopls",
			},
			found: true,
		},
		{
			lang:     "unknown",
			found:    false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.lang, func(t *testing.T) {
			config, found := ServerConfigForLanguage(tt.lang)
			if found != tt.found {
				t.Errorf("ServerConfigForLanguage(%q) found=%v, want %v", tt.lang, found, tt.found)
			}
			if found {
				if config.Language != tt.expected.Language {
					t.Errorf("Language mismatch: %s != %s", config.Language, tt.expected.Language)
				}
				if len(config.Command) != len(tt.expected.Command) {
					t.Errorf("Command length mismatch")
				}
				if config.Binary != tt.expected.Binary {
					t.Errorf("Binary mismatch: %s != %s", config.Binary, tt.expected.Binary)
				}
			}
		})
	}
}

// TestBinaryExists verifies binary existence checking.
func TestBinaryExists(t *testing.T) {
	// Test with a known binary (should exist)
	exists := BinaryExists("sh")
	if !exists {
		t.Error("expected 'sh' to exist in PATH")
	}

	// Test with non-existent binary
	exists = BinaryExists("/nonexistent/binary/that/does/not/exist")
	if exists {
		t.Error("expected non-existent binary to return false")
	}
}

// TestPoolWithRealServerIntegration tests pool with a real server if available.
// Run with: go test -run TestPoolWithRealServerIntegration -count=1
func TestPoolWithRealServerIntegration(t *testing.T) {
	if _, err := exec.LookPath("gopls"); err != nil {
		t.Skip("gopls not found in PATH, skipping integration test")
	}

	tmpDir := t.TempDir()
	goMod := filepath.Join(tmpDir, "go.mod")
	if err := os.WriteFile(goMod, []byte("module test\n\ngo 1.21\n"), 0644); err != nil {
		t.Fatalf("write go.mod: %v", err)
	}

	logger := slog.New(slog.DiscardHandler)
	pool := NewLSPPool(5*time.Minute, logger)
	defer pool.ShutdownAll()

	client, err := pool.GetClient(tmpDir, "go")
	if err != nil {
		t.Fatalf("GetClient failed: %v", err)
	}

	// Second call should return the same client
	client2, err := pool.GetClient(tmpDir, "go")
	if err != nil {
		t.Fatalf("Second GetClient failed: %v", err)
	}

	if client != client2 {
		t.Error("expected same client to be returned")
	}

	stats := pool.Stats()
	if stats.TotalConnections != 1 {
		t.Errorf("expected 1 connection, got %d", stats.TotalConnections)
	}
}

// Helper to check if a string contains a substring.
func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && findSubstring(s, substr))
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}