package archcheck

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDefaultArchRules(t *testing.T) {
	rules := DefaultArchRules()

	if rules == nil {
		t.Fatal("DefaultArchRules returned nil")
	}

	// Check default layers
	expectedLayers := []string{"domain", "engine", "intelligence", "interaction"}
	for _, layer := range expectedLayers {
		if _, ok := rules.Layers[layer]; !ok {
			t.Errorf("missing default layer: %s", layer)
		}
		if len(rules.Layers[layer]) == 0 {
			t.Errorf("default layer %s has no patterns", layer)
		}
	}

	// Check default allowed cross-layer
	if len(rules.AllowedCrossLayer) == 0 {
		t.Error("AllowedCrossLayer should not be empty")
	}

	// Check default severity
	expectedSeverities := []string{"forbidden_import", "circular_dep", "api_change"}
	for _, sevKey := range expectedSeverities {
		if _, ok := rules.Severity[sevKey]; !ok {
			t.Errorf("missing severity for: %s", sevKey)
		}
	}
}

func TestLoadArchRules_EmptyPath(t *testing.T) {
	rules, err := LoadArchRules("")
	if err != nil {
		t.Fatalf("LoadArchRules with empty path failed: %v", err)
	}
	if rules == nil {
		t.Fatal("LoadArchRules returned nil")
	}
	// Should return defaults
	if len(rules.Layers) == 0 {
		t.Error("expected default layers when config path is empty")
	}
}

func TestLoadArchRules_ValidConfig(t *testing.T) {
	// Create a temporary config file
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "arch.toml")

	configContent := `
[arch.layers]
domain = ["internal/core/**"]
engine = ["internal/engine/**"]

[arch]
allowed_cross_layer = ["engine -> domain"]
severity = { forbidden_import = "error", circular_dep = "warning", api_change = "info" }
`
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		t.Fatalf("failed to write config: %v", err)
	}

	rules, err := LoadArchRules(configPath)
	if err != nil {
		t.Fatalf("LoadArchRules failed: %v", err)
	}

	if rules == nil {
		t.Fatal("LoadArchRules returned nil")
	}

	// Check custom layers
	if _, ok := rules.Layers["domain"]; !ok {
		t.Error("domain layer not loaded")
	}
	if _, ok := rules.Layers["engine"]; !ok {
		t.Error("engine layer not loaded")
	}

	// Check allowed cross-layer
	if len(rules.AllowedCrossLayer) != 1 || rules.AllowedCrossLayer[0] != "engine -> domain" {
		t.Errorf("unexpected AllowedCrossLayer: %v", rules.AllowedCrossLayer)
	}

	// Check severity
	if rules.Severity["forbidden_import"] != "error" {
		t.Errorf("forbidden_import severity: expected error, got %s", rules.Severity["forbidden_import"])
	}
	if rules.Severity["circular_dep"] != "warning" {
		t.Errorf("circular_dep severity: expected warning, got %s", rules.Severity["circular_dep"])
	}
	if rules.Severity["api_change"] != "info" {
		t.Errorf("api_change severity: expected info, got %s", rules.Severity["api_change"])
	}
}

func TestLoadArchRules_NonExistentFile(t *testing.T) {
	rules, err := LoadArchRules("/non/existent/path/arch.toml")
	if err != nil {
		t.Fatalf("LoadArchRules should not error on non-existent file: %v", err)
	}
	if rules == nil {
		t.Fatal("LoadArchRules returned nil")
	}
	// Should return defaults
	if len(rules.Layers) == 0 {
		t.Error("expected default layers when config file doesn't exist")
	}
}

func TestLoadArchRules_InvalidSeverity(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "arch.toml")

	configContent := `
[arch]
severity = { forbidden_import = "invalid" }
`
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		t.Fatalf("failed to write config: %v", err)
	}

	_, err := LoadArchRules(configPath)
	if err == nil {
		t.Error("expected error for invalid severity")
	}
}

func TestLoadArchRules_UnknownSeverityKey(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "arch.toml")

	configContent := `
[arch]
severity = { unknown_violation = "error" }
`
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		t.Fatalf("failed to write config: %v", err)
	}

	_, err := LoadArchRules(configPath)
	if err == nil {
		t.Error("expected error for unknown severity key")
	}
}

func TestValidateRules_EmptyLayer(t *testing.T) {
	rules := &ArchRules{
		Layers: map[string][]string{
			"domain": {},
		},
	}
	err := ValidateRules(rules)
	if err == nil {
		t.Error("expected error for empty layer")
	}
}

func TestValidateRules_EmptyPattern(t *testing.T) {
	rules := &ArchRules{
		Layers: map[string][]string{
			"domain": {""},
		},
	}
	err := ValidateRules(rules)
	if err == nil {
		t.Error("expected error for empty pattern")
	}
}

func TestValidateRules_CircularAllowedCrossLayer(t *testing.T) {
	rules := &ArchRules{
		Layers: map[string][]string{
			"a": {"a/**"},
			"b": {"b/**"},
			"c": {"c/**"},
		},
		AllowedCrossLayer: []string{
			"a -> b",
			"b -> c",
			"c -> a", // circular
		},
		Severity: map[string]string{
			"forbidden_import": "error",
			"circular_dep":     "error",
			"api_change":       "warning",
		},
	}
	err := ValidateRules(rules)
	if err == nil {
		t.Error("expected error for circular allowed_cross_layer")
	}
}

func TestValidateRules_ValidConfig(t *testing.T) {
	rules := DefaultArchRules()
	err := ValidateRules(rules)
	if err != nil {
		t.Errorf("ValidateRules failed on default rules: %v", err)
	}
}

func TestForbiddenImportLayer(t *testing.T) {
	rules := DefaultArchRules()

	tests := []struct {
		file       string
		wantLayer  string
	}{
		{"internal/core/domain.go", "domain"},
		{"pkg/utils.go", "domain"},
		{"internal/engine/workflow.go", "engine"},
		{"internal/integrations/codeintel/parser.go", "intelligence"},
		{"internal/ui/tui/model.go", "interaction"},
		{"cmd/m31a/main.go", "interaction"},
		{"unknown/file.go", ""},
	}

	for _, tc := range tests {
		got := rules.ForbiddenImportLayer(tc.file)
		if got != tc.wantLayer {
			t.Errorf("ForbiddenImportLayer(%q) = %q, want %q", tc.file, got, tc.wantLayer)
		}
	}
}

func TestIsAllowedCrossLayer(t *testing.T) {
	rules := DefaultArchRules()

	tests := []struct {
		from, to   string
		wantAllowed bool
	}{
		{"engine", "domain", true},           // explicitly allowed
		{"intelligence", "domain", true},     // explicitly allowed
		{"interaction", "engine", true},      // explicitly allowed
		{"domain", "engine", false},          // reverse not allowed
		{"domain", "domain", true},           // same layer
		{"unknown", "domain", true},          // unknown layer
		{"domain", "unknown", true},          // unknown layer
	}

	for _, tc := range tests {
		got := rules.IsAllowedCrossLayer(tc.from, tc.to)
		if got != tc.wantAllowed {
			t.Errorf("IsAllowedCrossLayer(%q, %q) = %v, want %v", tc.from, tc.to, got, tc.wantAllowed)
		}
	}
}

func TestGetSeverity(t *testing.T) {
	rules := DefaultArchRules()

	tests := []struct {
		vType      ViolationType
		wantSeverity SeverityLevel
	}{
		{ViolationForbiddenImport, SeverityError},
		{ViolationCircularDep, SeverityError},
		{ViolationAPIChange, SeverityWarning},
		{ViolationType("unknown"), SeverityError}, // default
	}

	for _, tc := range tests {
		got := rules.GetSeverity(tc.vType)
		if got != tc.wantSeverity {
			t.Errorf("GetSeverity(%v) = %v, want %v", tc.vType, got, tc.wantSeverity)
		}
	}
}