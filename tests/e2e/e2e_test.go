package m31a_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/testutil/ci"
)

// TestBinary_Version verifies the binary prints version info.
func TestBinary_Version(t *testing.T) {
	bin := buildBinary(t)
	out := runBinary(t, bin, "--version")
	if !strings.Contains(out, "m31a") {
		t.Errorf("expected version output to contain 'm31a', got: %s", out)
	}
	expectedPlatform := runtime.GOOS + "/" + runtime.GOARCH
	if !strings.Contains(out, expectedPlatform) {
		t.Errorf("expected version output to contain '%s', got: %s", expectedPlatform, out)
	}
}

// TestBinary_Help verifies the binary prints help info.
func TestBinary_Help(t *testing.T) {
	bin := buildBinary(t)
	out := runBinary(t, bin, "--help")
	if !strings.Contains(out, "Terminal AI Coding Agent") {
		t.Errorf("expected help output to contain 'Terminal AI Coding Agent', got: %s", out)
	}
	if !strings.Contains(out, "-prompt") {
		t.Errorf("expected help output to contain '-prompt' flag, got: %s", out)
	}
}

// TestBinary_Prompt_NoProvider verifies headless mode fails gracefully without API keys.
func TestBinary_Prompt_NoProvider(t *testing.T) {
	bin := buildBinary(t)
	cmd := exec.Command(bin, "--prompt", "hello")
	cmd.Env = cleanEnv()
	cmd.Dir = t.TempDir()
	out, err := cmd.CombinedOutput()
	if err == nil {
		t.Fatal("expected error when no provider configured")
	}
	if !strings.Contains(string(out), "no provider") {
		t.Errorf("expected 'no provider' error, got: %s", string(out))
	}
}

// TestBinary_Prompt_NvidiaRealAPI tests real NVIDIA API call end-to-end.
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
	ci.SkipIfCI(t, "requires NVIDIA_API_KEY and network access not available in CI")
	apiKey := os.Getenv("NVIDIA_API_KEY")
	if apiKey == "" {
		t.Skip("NVIDIA_API_KEY not set — skipping real API test")
	}

	bin := buildBinary(t)
	cmd := exec.Command(bin, "--prompt", "What is 2+2? Reply with just the number.", "--model", "meta/llama-3.1-8b-instruct")
	cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY="+apiKey)
	cmd.Dir = t.TempDir()

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("headless mode failed: %v\noutput: %s", err, string(out))
	}

	resp := strings.TrimSpace(string(out))
	if resp == "" {
		t.Fatal("expected non-empty response from NVIDIA API")
	}
	t.Logf("NVIDIA API response: %q", resp)
}

// TestBinary_Prompt_ZenRealAPI tests real Zen API call end-to-end.
func TestBinary_Prompt_ZenRealAPI(t *testing.T) {
	ci.SkipIfCI(t, "requires ZEN_API_KEY and network access not available in CI")
	apiKey := os.Getenv("ZEN_API_KEY")
	if apiKey == "" {
		t.Skip("ZEN_API_KEY not set — skipping real API test")
	}

	bin := buildBinary(t)
	cmd := exec.Command(bin, "--prompt", "What is the capital of France? Reply with just the city name.", "--model", "openai/gpt-3.5-turbo")
	cmd.Env = append(cleanEnv(), "ZEN_API_KEY="+apiKey)
	cmd.Dir = t.TempDir()

	out, err := cmd.CombinedOutput()
	if err != nil {
		output := string(out)
		// Zen API key may not have chat permissions — skip gracefully
		if strings.Contains(output, "invalid API key") {
			t.Skip("Zen API key lacks chat permissions — skipping chat test")
		}
		t.Fatalf("headless mode failed: %v\noutput: %s", err, output)
	}

	resp := strings.TrimSpace(string(out))
	if resp == "" {
		t.Fatal("expected non-empty response from Zen API")
	}
	t.Logf("Zen API response: %q", resp)
}

// TestBinary_Prompt_OpenRouterRealAPI tests real OpenRouter API call end-to-end.
func TestBinary_Prompt_OpenRouterRealAPI(t *testing.T) {
	ci.SkipIfCI(t, "requires OPENROUTER_API_KEY and network access not available in CI")
	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		t.Skip("OPENROUTER_API_KEY not set — skipping real API test")
	}

	bin := buildBinary(t)
	cmd := exec.Command(bin, "--prompt", "What is 3*7? Reply with just the number.", "--model", "openai/gpt-3.5-turbo")
	cmd.Env = append(cleanEnv(), "OPENROUTER_API_KEY="+apiKey)
	cmd.Dir = t.TempDir()

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("headless mode failed: %v\noutput: %s", err, string(out))
	}

	resp := strings.TrimSpace(string(out))
	if resp == "" {
		t.Fatal("expected non-empty response from OpenRouter API")
	}
	t.Logf("OpenRouter API response: %q", resp)
}

// TestBinary_Prompt_Timeout verifies that a long prompt times out gracefully.
func TestBinary_Prompt_Timeout(t *testing.T) {
	bin := buildBinary(t)
	cmd := exec.Command(bin, "--prompt", "Write a 10000 word essay about quantum computing")
	cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY=dummy-key-for-timeout-test")
	cmd.Dir = t.TempDir()

	done := make(chan error, 1)
	go func() {
		_, err := cmd.CombinedOutput()
		done <- err
	}()

	select {
	case <-done:
		// Expected — either timeout error or auth error
	case <-time.After(30 * time.Second):
		cmd.Process.Kill()
		t.Fatal("binary did not exit within 30 seconds")
	}
}

// buildBinary compiles the M31A binary and returns the path.
func buildBinary(t *testing.T) string {
	t.Helper()
	bin := filepath.Join(t.TempDir(), "m31a")
	// Find project root by looking for go.mod
	dir := mustGetwd(t)
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			break
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("could not find project root (go.mod)")
		}
		dir = parent
	}
	cmd := exec.Command("go", "build", "-o", bin, "./cmd/m31a")
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("build failed: %v\n%s", err, string(out))
	}
	return bin
}

// runBinary executes the binary with args and returns stdout.
func runBinary(t *testing.T, bin string, args ...string) string {
	t.Helper()
	cmd := exec.Command(bin, args...)
	cmd.Env = cleanEnv()
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("run failed: %v\noutput: %s", err, string(out))
	}
	return string(out)
}

// cleanEnv returns a minimal environment without any API keys or project config.
func cleanEnv() []string {
	return []string{
		"PATH=" + os.Getenv("PATH"),
		"HOME=" + os.TempDir(),
		"M31A_CONFIG=" + filepath.Join(os.TempDir(), "m31a-test-config.toml"),
	}
}

func mustGetwd(t *testing.T) string {
	t.Helper()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	return wd
}
