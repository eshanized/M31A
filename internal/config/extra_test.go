package config

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"sync"
	"testing"
	"time"
)

// ── LocalConfigPath ──────────────────────────────────────────────────────────

func TestLocalConfigPath(t *testing.T) {
	got, err := LocalConfigPath()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if filepath.Base(got) != "m31a.toml" {
		t.Errorf("expected m31a.toml, got %s", filepath.Base(got))
	}
}

// ── SaveProject ──────────────────────────────────────────────────────────────

func TestSaveProject_ClearsAPIKeys(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "m31a.toml")

	cfg := DefaultConfig()
	cfg.Provider.OpenRouter.APIKey = "sk-or-secret123"
	cfg.Provider.Zen.APIKey = "sk-zen-secret456"
	cfg.Provider.Default = "openrouter"

	if err := cfg.SaveProject(path); err != nil {
		t.Fatalf("SaveProject failed: %v", err)
	}

	// Reload and verify keys are cleared
	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if loaded.Provider.OpenRouter.APIKey != "" {
		t.Errorf("expected OpenRouter API key to be cleared, got %q", loaded.Provider.OpenRouter.APIKey)
	}
	if loaded.Provider.Zen.APIKey != "" {
		t.Errorf("expected Zen API key to be cleared, got %q", loaded.Provider.Zen.APIKey)
	}
	if loaded.Provider.Default != "openrouter" {
		t.Errorf("expected provider 'openrouter', got %q", loaded.Provider.Default)
	}
}

func TestSaveProject_PreservesSliceAndMapFields(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "m31a.toml")

	cfg := DefaultConfig()
	cfg.Tools.SkipDirs = []string{"node_modules", "vendor"}
	cfg.Permissions.Rules = []PermissionRule{
		{Tool: "Bash", Action: "ask"},
	}
	cfg.Permissions.Agents = map[string]PermissionsAgentConfig{
		"build": {DefaultAction: "allow", Rules: []PermissionRule{{Tool: "Edit", Action: "deny"}}},
	}

	if err := cfg.SaveProject(path); err != nil {
		t.Fatalf("SaveProject failed: %v", err)
	}

	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if len(loaded.Tools.SkipDirs) != 2 {
		t.Errorf("expected 2 skip dirs, got %d", len(loaded.Tools.SkipDirs))
	}
	if len(loaded.Permissions.Rules) != 1 {
		t.Errorf("expected 1 rule, got %d", len(loaded.Permissions.Rules))
	}
	if loaded.Permissions.Agents == nil {
		t.Error("expected Agents to be non-nil")
	}
}

// ── mergeField ───────────────────────────────────────────────────────────────

func TestMergeField_String(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf("")).Elem()
	overlay = reflect.New(reflect.TypeOf("")).Elem()
	base.SetString("old")
	overlay.SetString("new")

	mergeField(base, overlay, reflect.TypeOf(""), nil, "test")
	if base.String() != "new" {
		t.Errorf("expected 'new', got %q", base.String())
	}
}

func TestMergeField_StringEmptyOverlay(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf("")).Elem()
	overlay = reflect.New(reflect.TypeOf("")).Elem()
	base.SetString("keep")

	mergeField(base, overlay, reflect.TypeOf(""), nil, "test")
	if base.String() != "keep" {
		t.Errorf("expected 'keep', got %q", base.String())
	}
}

func TestMergeField_Bool(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf(false)).Elem()
	overlay = reflect.New(reflect.TypeOf(false)).Elem()
	base.SetBool(false)
	overlay.SetBool(true)

	mergeField(base, overlay, reflect.TypeOf(false), nil, "test")
	if !base.Bool() {
		t.Error("expected true")
	}
}

func TestMergeField_BoolDefinedFalse(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf(false)).Elem()
	overlay = reflect.New(reflect.TypeOf(false)).Elem()
	base.SetBool(true)
	overlay.SetBool(false)

	defined := map[string]bool{"test": true}
	mergeField(base, overlay, reflect.TypeOf(false), defined, "test")
	if base.Bool() {
		t.Error("expected false when explicitly defined")
	}
}

func TestMergeField_BoolUndefinedFalse(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf(false)).Elem()
	overlay = reflect.New(reflect.TypeOf(false)).Elem()
	base.SetBool(true)
	overlay.SetBool(false)

	mergeField(base, overlay, reflect.TypeOf(false), nil, "test")
	if !base.Bool() {
		t.Error("expected true when overlay false and not defined")
	}
}

func TestMergeField_Int(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf(0)).Elem()
	overlay = reflect.New(reflect.TypeOf(0)).Elem()
	base.SetInt(10)
	overlay.SetInt(42)

	mergeField(base, overlay, reflect.TypeOf(0), nil, "test")
	if base.Int() != 42 {
		t.Errorf("expected 42, got %d", base.Int())
	}
}

func TestMergeField_IntZeroOverlay(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf(0)).Elem()
	overlay = reflect.New(reflect.TypeOf(0)).Elem()
	base.SetInt(10)

	mergeField(base, overlay, reflect.TypeOf(0), nil, "test")
	if base.Int() != 10 {
		t.Errorf("expected 10, got %d", base.Int())
	}
}

func TestMergeField_Float(t *testing.T) {
	var base, overlay reflect.Value
	base = reflect.New(reflect.TypeOf(0.0)).Elem()
	overlay = reflect.New(reflect.TypeOf(0.0)).Elem()
	base.SetFloat(1.5)
	overlay.SetFloat(2.5)

	mergeField(base, overlay, reflect.TypeOf(0.0), nil, "test")
	if base.Float() != 2.5 {
		t.Errorf("expected 2.5, got %f", base.Float())
	}
}

func TestMergeField_Slice(t *testing.T) {
	baseVal := []string{"a", "b"}
	overlayVal := []string{"c", "d", "e"}

	base := reflect.ValueOf(&baseVal).Elem()
	overlay := reflect.ValueOf(&overlayVal).Elem()

	mergeField(base, overlay, reflect.TypeOf([]string{}), nil, "test")
	if base.Len() != 3 {
		t.Errorf("expected length 3, got %d", base.Len())
	}
}

func TestMergeField_Map(t *testing.T) {
	baseVal := map[string]string{"a": "1"}
	overlayVal := map[string]string{"b": "2", "c": "3"}

	base := reflect.ValueOf(&baseVal).Elem()
	overlay := reflect.ValueOf(&overlayVal).Elem()

	mergeField(base, overlay, reflect.TypeOf(map[string]string{}), nil, "test")
	result := base.Interface().(map[string]string)
	if result["a"] != "1" {
		t.Error("expected base value preserved")
	}
	if result["b"] != "2" {
		t.Error("expected overlay value merged")
	}
}

func TestMergeField_MapNilBase(t *testing.T) {
	overlayVal := map[string]string{"a": "1"}
	base := reflect.ValueOf(&struct{ M map[string]string }{}).Elem().Field(0)
	overlay := reflect.ValueOf(&overlayVal).Elem()

	mergeField(base, overlay, reflect.TypeOf(map[string]string{}), nil, "test")
}

// ── substituteVars ───────────────────────────────────────────────────────────

func TestSubstituteVars_Replacement(t *testing.T) {
	t.Setenv("M31A_TEST_VAR_REPLACEMENT", "replaced_value")
	got := substituteVars("${M31A_TEST_VAR_REPLACEMENT}")
	if got != "replaced_value" {
		t.Errorf("expected 'replaced_value', got %q", got)
	}
}

func TestSubstituteVars_Empty(t *testing.T) {
	got := substituteVars("")
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

func TestSubstituteVars_NoVarPattern(t *testing.T) {
	got := substituteVars("plain-text")
	if got != "plain-text" {
		t.Errorf("expected 'plain-text', got %q", got)
	}
}

func TestSubstituteVars_UnsetVarPreserved(t *testing.T) {
	got := substituteVars("${M31A_TOTALLY_UNSET_VAR_XYZ}")
	if got != "${M31A_TOTALLY_UNSET_VAR_XYZ}" {
		t.Errorf("expected preserved pattern, got %q", got)
	}
}

func TestSubstituteVars_MultipleVars(t *testing.T) {
	t.Setenv("M31A_TESTVAR_A", "AAA")
	t.Setenv("M31A_TESTVAR_B", "BBB")
	got := substituteVars("${M31A_TESTVAR_A}-${M31A_TESTVAR_B}")
	if got != "AAA-BBB" {
		t.Errorf("expected 'AAA-BBB', got %q", got)
	}
}

func TestSubstituteVars_MixedResolved(t *testing.T) {
	t.Setenv("M31A_TESTVAR_C", "CCC")
	got := substituteVars("${M31A_TESTVAR_C}-${M31A_TESTVAR_NONEXISTENT}")
	if got != "CCC-${M31A_TESTVAR_NONEXISTENT}" {
		t.Errorf("expected mixed, got %q", got)
	}
}

// ── SaveWithKeychain ─────────────────────────────────────────────────────────

func TestSaveWithKeychain_NilKeychain(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	cfg := DefaultConfig()
	cfg.Provider.OpenRouter.APIKey = "sk-or-test"
	cfg.Provider.Zen.APIKey = "sk-zen-test"
	cfg.Provider.Default = "openrouter"

	if err := cfg.SaveWithKeychain(path, nil); err != nil {
		t.Fatalf("SaveWithKeychain failed: %v", err)
	}

	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if loaded.Provider.OpenRouter.APIKey != "sk-or-test" {
		t.Errorf("expected key preserved with nil keychain, got %q", loaded.Provider.OpenRouter.APIKey)
	}
}

func TestSaveWithKeychain_AvailableKeychain(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")

	cfg := DefaultConfig()
	cfg.Provider.OpenRouter.APIKey = "sk-or-key123"
	cfg.Provider.Zen.APIKey = "sk-zen-key456"
	cfg.Provider.Default = "openrouter"

	kc := newMockKeychain()
	if err := cfg.SaveWithKeychain(path, kc); err != nil {
		t.Fatalf("SaveWithKeychain failed: %v", err)
	}

	// Keys should be in keychain
	if v, _ := kc.Get("openrouter"); v != "sk-or-key123" {
		t.Errorf("expected key in keychain, got %q", v)
	}

	// Keys should NOT be in config file
	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if loaded.Provider.OpenRouter.APIKey != "" {
		t.Errorf("expected key cleared from config, got %q", loaded.Provider.OpenRouter.APIKey)
	}
}

func TestSaveWithKeychain_M31AConfigOverride(t *testing.T) {
	dir := t.TempDir()
	overridePath := filepath.Join(dir, "override.toml")

	cfg := DefaultConfig()
	cfg.UI.Theme = "light"

	t.Setenv("M31A_CONFIG", overridePath)

	if err := cfg.SaveWithKeychain(filepath.Join(dir, "other.toml"), nil); err != nil {
		t.Fatalf("SaveWithKeychain failed: %v", err)
	}

	if _, err := os.Stat(overridePath); os.IsNotExist(err) {
		t.Error("expected file at M31A_CONFIG path")
	}
}

// ── sendReload ───────────────────────────────────────────────────────────────

func TestSendReload_Success(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")
	if err := os.WriteFile(path, []byte("[ui]\ntheme=\"dark\"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	ch := make(chan ConfigReloadMsg, 1)
	ctx := context.Background()
	sendReload(ctx, ch, path)

	msg := <-ch
	if msg.Error != nil {
		t.Fatalf("unexpected error: %v", msg.Error)
	}
	if msg.Config == nil {
		t.Fatal("expected non-nil config")
	}
	if msg.Config.UI.Theme != "dark" {
		t.Errorf("expected 'dark', got %q", msg.Config.UI.Theme)
	}
}

func TestSendReload_ContextCancelled(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")
	if err := os.WriteFile(path, []byte("[ui]\ntheme=\"dark\"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	ch := make(chan ConfigReloadMsg, 1)
	sendReload(ctx, ch, path)

	// Should not block — either sends or exits via context
	select {
	case <-ch:
	case <-time.After(2 * time.Second):
		// acceptable: message was dropped
	}
}

// ── WatchConfig polling fallback ─────────────────────────────────────────────

func TestWatchConfig_PollingDetectsChange(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")
	if err := os.WriteFile(path, []byte("[ui]\ntheme=\"dark\"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	ch := make(chan ConfigReloadMsg, 1)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go watchConfigPolling(ctx, path, ch)

	time.Sleep(100 * time.Millisecond)

	// Modify file to trigger polling
	if err := os.WriteFile(path, []byte("[ui]\ntheme=\"light\"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	select {
	case msg := <-ch:
		if msg.Error != nil {
			t.Fatalf("unexpected error: %v", msg.Error)
		}
		if msg.Config.UI.Theme != "light" {
			t.Errorf("expected 'light', got %q", msg.Config.UI.Theme)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("timed out waiting for polling reload")
	}
}

func TestWatchConfig_PollingCancellation(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")
	if err := os.WriteFile(path, []byte("[ui]\ntheme=\"dark\"\n"), 0644); err != nil {
		t.Fatal(err)
	}

	ch := make(chan ConfigReloadMsg, 1)
	ctx, cancel := context.WithCancel(context.Background())

	done := make(chan struct{})
	go func() {
		watchConfigPolling(ctx, path, ch)
		close(done)
	}()

	cancel()

	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("watchConfigPolling did not exit after context cancellation")
	}
}

func TestWatchConfig_PollingNoFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "nonexistent.toml")

	ch := make(chan ConfigReloadMsg, 1)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	done := make(chan struct{})
	go func() {
		watchConfigPolling(ctx, path, ch)
		close(done)
	}()

	// Let one tick pass
	time.Sleep(100 * time.Millisecond)

	// Should not crash, just continue polling
	cancel()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("did not exit")
	}
}

// ── LoadDotEnv ───────────────────────────────────────────────────────────────

func TestLoadDotEnv_SetsVars(t *testing.T) {
	dir := t.TempDir()

	// Save and restore working directory
	origWd, _ := os.Getwd()
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	defer os.Chdir(origWd)

	envContent := "M31A_DOTENV_TEST_KEY=hello_from_dotenv\n# comment\n\nM31A_DOTENV_TEST_NUM=42\n"
	if err := os.WriteFile(filepath.Join(dir, ".env"), []byte(envContent), 0600); err != nil {
		t.Fatal(err)
	}

	// Reset the once guard to allow re-calling
	loadDotEnvOnce = sync.Once{}

	LoadDotEnv()

	if v := os.Getenv("M31A_DOTENV_TEST_KEY"); v != "hello_from_dotenv" {
		t.Errorf("expected 'hello_from_dotenv', got %q", v)
	}
	if v := os.Getenv("M31A_DOTENV_TEST_NUM"); v != "42" {
		t.Errorf("expected '42', got %q", v)
	}

	os.Unsetenv("M31A_DOTENV_TEST_KEY")
	os.Unsetenv("M31A_DOTENV_TEST_NUM")
}

func TestLoadDotEnv_DoesNotOverrideExisting(t *testing.T) {
	dir := t.TempDir()

	origWd, _ := os.Getwd()
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	defer os.Chdir(origWd)

	t.Setenv("M31A_DOTENV_EXISTING", "original")

	envContent := "M31A_DOTENV_EXISTING=overwritten\n"
	if err := os.WriteFile(filepath.Join(dir, ".env"), []byte(envContent), 0600); err != nil {
		t.Fatal(err)
	}

	loadDotEnvOnce = sync.Once{}
	LoadDotEnv()

	if v := os.Getenv("M31A_DOTENV_EXISTING"); v != "original" {
		t.Errorf("expected original value preserved, got %q", v)
	}
}

func TestLoadDotEnv_NoDotEnvFile(t *testing.T) {
	dir := t.TempDir()

	origWd, _ := os.Getwd()
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	defer os.Chdir(origWd)

	loadDotEnvOnce = sync.Once{}
	LoadDotEnv()
	// Should not panic
}

func TestLoadDotEnv_WritablePermSkipped(t *testing.T) {
	dir := t.TempDir()

	origWd, _ := os.Getwd()
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	defer os.Chdir(origWd)

	envContent := "M31A_DOTENV_SKIPPED=yes\n"
	envPath := filepath.Join(dir, ".env")
	if err := os.WriteFile(envPath, []byte(envContent), 0600); err != nil {
		t.Fatal(err)
	}
	// Set world-writable permissions (bypassing umask)
	if err := os.Chmod(envPath, 0622); err != nil {
		t.Fatal(err)
	}

	loadDotEnvOnce = sync.Once{}
	LoadDotEnv()

	if v := os.Getenv("M31A_DOTENV_SKIPPED"); v != "" {
		t.Errorf("expected env not set from world-writable .env, got %q", v)
	}
}

func TestLoadDotEnv_QuotedValues(t *testing.T) {
	dir := t.TempDir()

	origWd, _ := os.Getwd()
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	defer os.Chdir(origWd)

	envContent := "M31A_DOTENV_DQ=\"double_quoted\"\nM31A_DOTENV_SQ='single_quoted'\n"
	if err := os.WriteFile(filepath.Join(dir, ".env"), []byte(envContent), 0600); err != nil {
		t.Fatal(err)
	}

	loadDotEnvOnce = sync.Once{}
	LoadDotEnv()

	if v := os.Getenv("M31A_DOTENV_DQ"); v != "double_quoted" {
		t.Errorf("expected 'double_quoted', got %q", v)
	}
	if v := os.Getenv("M31A_DOTENV_SQ"); v != "single_quoted" {
		t.Errorf("expected 'single_quoted', got %q", v)
	}

	os.Unsetenv("M31A_DOTENV_DQ")
	os.Unsetenv("M31A_DOTENV_SQ")
}

// ── LoadProjectContext ───────────────────────────────────────────────────────

func TestLoadProjectContext_AgentsMD(t *testing.T) {
	dir := t.TempDir()
	content := "# Project Rules\nBe concise."
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	got, path := LoadProjectContext(dir)
	if got != content {
		t.Errorf("expected %q, got %q", content, got)
	}
	if filepath.Base(path) != "AGENTS.md" {
		t.Errorf("expected AGENTS.md, got %s", path)
	}
}

func TestLoadProjectContext_DotM31aAgentsMD(t *testing.T) {
	dir := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dir, ".m31a"), 0755); err != nil {
		t.Fatal(err)
	}
	content := "# Agent instructions"
	if err := os.WriteFile(filepath.Join(dir, ".m31a", "agents.md"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	got, path := LoadProjectContext(dir)
	if got != content {
		t.Errorf("expected %q, got %q", content, got)
	}
	if filepath.Base(path) != "agents.md" {
		t.Errorf("expected agents.md, got %s", path)
	}
}

func TestLoadProjectContext_MemoryMD(t *testing.T) {
	dir := t.TempDir()
	content := "# Memory file"
	if err := os.WriteFile(filepath.Join(dir, "MEMORY.md"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	got, path := LoadProjectContext(dir)
	if got != content {
		t.Errorf("expected %q, got %q", content, got)
	}
	if filepath.Base(path) != "MEMORY.md" {
		t.Errorf("expected MEMORY.md, got %s", path)
	}
}

func TestLoadProjectContext_NoFiles(t *testing.T) {
	dir := t.TempDir()
	got, path := LoadProjectContext(dir)
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
	if path != "" {
		t.Errorf("expected empty path, got %q", path)
	}
}

func TestLoadProjectContext_LargeFileTruncated(t *testing.T) {
	dir := t.TempDir()
	bigContent := make([]byte, maxProjectContextBytes+100)
	for i := range bigContent {
		bigContent[i] = 'A'
	}
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), bigContent, 0644); err != nil {
		t.Fatal(err)
	}

	got, _ := LoadProjectContext(dir)
	if len(got) != maxProjectContextBytes {
		t.Errorf("expected truncated to %d, got %d", maxProjectContextBytes, len(got))
	}
}


