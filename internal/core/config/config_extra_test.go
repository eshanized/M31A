package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// --- instructions.go tests ---

func TestRenderInstructions_Empty(t *testing.T) {
	t.Parallel()
	result := RenderInstructions(nil)
	if result != "" {
		t.Errorf("RenderInstructions(nil) = %q, want empty", result)
	}
}

func TestRenderInstructions_Single(t *testing.T) {
	t.Parallel()
	files := []InstructionFile{
		{Path: "/root/AGENTS.md", Content: "Always use tests"},
	}
	result := RenderInstructions(files)
	if !strings.Contains(result, "# Project Instructions") {
		t.Error("should contain header")
	}
	if !strings.Contains(result, "Always use tests") {
		t.Error("should contain content")
	}
	if !strings.Contains(result, "/root/AGENTS.md") {
		t.Error("should contain source path")
	}
}

func TestRenderInstructions_Multiple(t *testing.T) {
	t.Parallel()
	files := []InstructionFile{
		{Path: "/root/AGENTS.md", Content: "Root instructions"},
		{Path: "/root/src/AGENTS.md", Content: "Src instructions"},
	}
	result := RenderInstructions(files)
	if !strings.Contains(result, "Root instructions") {
		t.Error("should contain root content")
	}
	if !strings.Contains(result, "Src instructions") {
		t.Error("should contain src content")
	}
}

func TestDiscoverInstructions_NoFiles(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	result := DiscoverInstructions(tmpDir, tmpDir)
	if len(result) != 0 {
		t.Errorf("expected 0 files, got %d", len(result))
	}
}

func TestDiscoverInstructions_WithAGENTS(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	agentsPath := filepath.Join(tmpDir, "AGENTS.md")
	if err := os.WriteFile(agentsPath, []byte("# Instructions\nUse tests"), 0644); err != nil {
		t.Fatal(err)
	}
	result := DiscoverInstructions(tmpDir, tmpDir)
	if len(result) != 1 {
		t.Fatalf("expected 1 file, got %d", len(result))
	}
	if result[0].Content != "# Instructions\nUse tests" {
		t.Errorf("content = %q", result[0].Content)
	}
}

func TestDiscoverInstructions_Nested(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	rootAgents := filepath.Join(tmpDir, "AGENTS.md")
	srcDir := filepath.Join(tmpDir, "src")
	if err := os.MkdirAll(srcDir, 0755); err != nil {
		t.Fatal(err)
	}
	srcAgents := filepath.Join(srcDir, "AGENTS.md")
	if err := os.WriteFile(rootAgents, []byte("root"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(srcAgents, []byte("src"), 0644); err != nil {
		t.Fatal(err)
	}
	result := DiscoverInstructions(tmpDir, srcDir)
	if len(result) != 2 {
		t.Fatalf("expected 2 files, got %d", len(result))
	}
	// First should be root (outermost)
	if result[0].Content != "root" {
		t.Errorf("first file content = %q, want root", result[0].Content)
	}
	// Second should be src (innermost)
	if result[1].Content != "src" {
		t.Errorf("second file content = %q, want src", result[1].Content)
	}
}

func TestDiscoverInstructions_EmptyProjectRoot(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	agentsPath := filepath.Join(tmpDir, "AGENTS.md")
	if err := os.WriteFile(agentsPath, []byte("content"), 0644); err != nil {
		t.Fatal(err)
	}
	// Empty projectRoot should default to workDir
	result := DiscoverInstructions("", tmpDir)
	if len(result) != 1 {
		t.Errorf("expected 1 file, got %d", len(result))
	}
}

func TestDiscoverInstructions_EmptyAGENTS(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	agentsPath := filepath.Join(tmpDir, "AGENTS.md")
	if err := os.WriteFile(agentsPath, []byte("   \n  \n"), 0644); err != nil {
		t.Fatal(err)
	}
	result := DiscoverInstructions(tmpDir, tmpDir)
	if len(result) != 0 {
		t.Errorf("empty AGENTS.md should be skipped, got %d files", len(result))
	}
}

// --- loader.go validateConfig branch tests ---

func TestValidateConfig_NegativeArbitrageThreshold(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Model.ArbitrageThreshold = -0.5
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative arbitrage threshold")
	}
}

func TestValidateConfig_ArbitrageThresholdOverOne(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Model.ArbitrageThreshold = 1.5
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for arbitrage threshold > 1")
	}
}

func TestValidateConfig_NegativeDefaultContextLength(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Model.DefaultContextLength = -1000
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative default context length")
	}
}

func TestValidateConfig_NegativeTokenEMAAlpha(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Model.TokenEMAAlpha = -0.1
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative token EMA alpha")
	}
}

func TestValidateConfig_TokenEMAAlphaOverOne(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Model.TokenEMAAlpha = 1.5
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for token EMA alpha > 1")
	}
}

func TestValidateConfig_NegativeLeaderTimeoutMs(t *testing.T) {
	cfg := DefaultConfig()
	cfg.UI.LeaderTimeoutMs = -100
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative leader timeout")
	}
}

func TestValidateConfig_NegativePermissionTimeout(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Permissions.TimeoutSeconds = -10
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative permission timeout")
	}
}

func TestValidateConfig_NegativeMaxEntries(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Ledger.MaxEntries = -100
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative max entries")
	}
}

func TestValidateConfig_NegativeModelCacheTTL(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Features.ModelCacheTTLMinutes = -5
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative model cache TTL")
	}
}

func TestValidateConfig_NegativeModelCacheStale(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Features.ModelCacheStaleHours = -1
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative model cache stale hours")
	}
}

func TestValidateConfig_NegativeHealthCheckLive(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Features.HealthCheckLiveMs = -1000
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative healthcheck live ms")
	}
}

func TestValidateConfig_NegativeMaxRecentModels(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Features.MaxRecentModels = -5
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative max recent models")
	}
}

func TestValidateConfig_NegativeMaxGlobResults(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Tools.MaxGlobResults = -100
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative max glob results")
	}
}

func TestValidateConfig_NegativeMaxGrepResults(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Tools.MaxGrepResults = -100
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative max grep results")
	}
}

func TestValidateConfig_NegativeBashKillGrace(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Tools.BashKillGraceSecs = -5
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative bash kill grace")
	}
}

func TestValidateConfig_NegativeMaxBackupsPerFile(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Tools.MaxBackupsPerFile = -1
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative max backups per file")
	}
}

func TestValidateConfig_NegativeWebfetchMaxRedirects(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Tools.WebfetchMaxRedirects = -1
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for negative webfetch max redirects")
	}
}

func TestValidateConfig_InvalidRuleAction_Extra(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Permissions.Rules = []PermissionRule{
		{Action: "invalid", Tool: "Bash"},
	}
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for invalid rule action")
	}
}

func TestValidateConfig_EmptyRuleTool_Extra(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Permissions.Rules = []PermissionRule{
		{Action: "allow", Tool: ""},
	}
	if err := validateConfig(cfg); err == nil {
		t.Error("expected error for empty rule tool")
	}
}

// --- knownConfigKeys test ---

func TestKnownConfigKeys(t *testing.T) {
	keys := knownConfigKeys()
	if len(keys) == 0 {
		t.Error("knownConfigKeys() returned empty map")
	}
	// Check some expected keys
	for _, key := range []string{"provider", "model", "ui", "permissions", "tools", "features"} {
		if !keys[key] {
			t.Errorf("knownConfigKeys() missing key %q", key)
		}
	}
}

// --- Config struct tests ---

func TestCompactionConfig(t *testing.T) {
	t.Parallel()
	cfg := CompactionConfig{Auto: true, Buffer: 1000, KeepTokens: 500}
	if !cfg.Auto || cfg.Buffer != 1000 || cfg.KeepTokens != 500 {
		t.Errorf("CompactionConfig = %+v", cfg)
	}
}

func TestInstructionsConfig(t *testing.T) {
	t.Parallel()
	cfg := InstructionsConfig{Enabled: true, DisableProject: false}
	if !cfg.Enabled || cfg.DisableProject {
		t.Errorf("InstructionsConfig = %+v", cfg)
	}
}

func TestSkillsConfig(t *testing.T) {
	t.Parallel()
	cfg := SkillsConfig{Sources: []string{"a", "b"}}
	if len(cfg.Sources) != 2 {
		t.Errorf("SkillsConfig.Sources len = %d", len(cfg.Sources))
	}
}

// --- loader.go substituteVarsReport test ---

func TestSubstituteVarsReport(t *testing.T) {
	t.Parallel()
	val := "${TEST_VAR_FOR_REPORT}"
	result := substituteVarsReport(&val, "test_field")
	_ = result
}

// --- Config struct tests ---
