package archcheck

import (
	"fmt"
	"os"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/bmatcuk/doublestar/v4"
)

// ArchRules defines the architecture layer rules for violation detection.
type ArchRules struct {
	Layers            map[string][]string `toml:"layers"`
	AllowedCrossLayer []string            `toml:"allowed_cross_layer"`
	Severity          map[string]string   `toml:"severity"`
}

// archConfig is the TOML config structure with [arch] section.
type archConfig struct {
	Arch ArchRules `toml:"arch"`
}

// ViolationType represents the type of architecture violation.
type ViolationType string

const (
	ViolationForbiddenImport ViolationType = "forbidden_import"
	ViolationCircularDep     ViolationType = "circular_dep"
	ViolationAPIChange       ViolationType = "api_change"
)

// SeverityLevel represents the severity of a violation.
type SeverityLevel string

const (
	SeverityError   SeverityLevel = "error"
	SeverityWarning SeverityLevel = "warning"
	SeverityInfo    SeverityLevel = "info"
)

// DefaultArchRules returns sensible default architecture rules.
func DefaultArchRules() *ArchRules {
	return &ArchRules{
		Layers: map[string][]string{
			"domain":       {"internal/core/**", "pkg/**"},
			"engine":       {"internal/engine/**"},
			"intelligence": {"internal/integrations/**"},
			"interaction":  {"internal/ui/**", "cmd/**"},
		},
		AllowedCrossLayer: []string{
			"engine -> domain",
			"intelligence -> domain",
			"interaction -> engine",
		},
		Severity: map[string]string{
			"forbidden_import": "error",
			"circular_dep":     "error",
			"api_change":       "warning",
		},
	}
}

// LoadArchRules loads architecture rules from a TOML config file.
// If configPath is empty, returns DefaultArchRules.
func LoadArchRules(configPath string) (*ArchRules, error) {
	if configPath == "" {
		return DefaultArchRules(), nil
	}

	var cfg archConfig
	_, err := toml.DecodeFile(configPath, &cfg)
	if err != nil {
		if os.IsNotExist(err) {
			return DefaultArchRules(), nil
		}
		return nil, fmt.Errorf("decode arch rules: %w", err)
	}

	rules := cfg.Arch

	// Set defaults for missing fields
	if rules.Layers == nil {
		rules.Layers = DefaultArchRules().Layers
	}
	if rules.AllowedCrossLayer == nil {
		rules.AllowedCrossLayer = DefaultArchRules().AllowedCrossLayer
	}
	if rules.Severity == nil {
		rules.Severity = DefaultArchRules().Severity
	}

	// Validate rules
	if err := ValidateRules(&rules); err != nil {
		return nil, err
	}

	return &rules, nil
}

// ValidateRules validates the architecture rules for consistency.
func ValidateRules(rules *ArchRules) error {
	// Check for empty layers
	for layer, patterns := range rules.Layers {
		if len(patterns) == 0 {
			return fmt.Errorf("layer %q has no patterns", layer)
		}
		for _, pattern := range patterns {
			if strings.TrimSpace(pattern) == "" {
				return fmt.Errorf("layer %q contains empty pattern", layer)
			}
		}
	}

	// Check for circular layer references in AllowedCrossLayer
	// Build a graph of allowed cross-layer dependencies
	allowedGraph := make(map[string]map[string]bool)
	for _, allowed := range rules.AllowedCrossLayer {
		parts := strings.Split(allowed, "->")
		if len(parts) != 2 {
			continue
		}
		from := strings.TrimSpace(parts[0])
		to := strings.TrimSpace(parts[1])
		if allowedGraph[from] == nil {
			allowedGraph[from] = make(map[string]bool)
		}
		allowedGraph[from][to] = true
	}

	// Detect cycles in allowed cross-layer graph using DFS
	visited := make(map[string]bool)
	recStack := make(map[string]bool)

	var dfs func(string) bool
	dfs = func(node string) bool {
		visited[node] = true
		recStack[node] = true

		for neighbor := range allowedGraph[node] {
			if !visited[neighbor] {
				if dfs(neighbor) {
					return true
				}
			} else if recStack[neighbor] {
				return true // cycle detected
			}
		}

		recStack[node] = false
		return false
	}

	for node := range allowedGraph {
		if !visited[node] {
			if dfs(node) {
				return fmt.Errorf("circular reference detected in allowed_cross_layer")
			}
		}
	}

	// Validate severity values
	validSeverities := map[string]bool{
		"error":   true,
		"warning": true,
		"info":    true,
	}
	for key, severity := range rules.Severity {
		if !validSeverities[severity] {
			return fmt.Errorf("invalid severity %q for %s: must be error, warning, or info", severity, key)
		}
		// Check that the key is a known violation type
		knownKeys := map[string]bool{
			"forbidden_import": true,
			"circular_dep":     true,
			"api_change":       true,
		}
		if !knownKeys[key] {
			return fmt.Errorf("unknown severity key %q", key)
		}
	}

	// Also validate that all required severity keys have values
	requiredKeys := []string{"forbidden_import", "circular_dep", "api_change"}
	for _, k := range requiredKeys {
		if _, ok := rules.Severity[k]; !ok {
			rules.Severity[k] = DefaultArchRules().Severity[k]
		}
	}

	return nil
}

// ForbiddenImportLayer returns the layer a file belongs to based on glob matching.
func (r *ArchRules) ForbiddenImportLayer(file string) string {
	for layer, patterns := range r.Layers {
		for _, pattern := range patterns {
			matched, _ := doublestar.Match(pattern, file)
			if matched {
				return layer
			}
		}
	}
	return ""
}

// IsAllowedCrossLayer checks if a cross-layer import is explicitly allowed.
func (r *ArchRules) IsAllowedCrossLayer(fromLayer, toLayer string) bool {
	// Unknown layers or same layer are not cross-layer violations
	if fromLayer == "" || toLayer == "" || fromLayer == toLayer {
		return true
	}

	// Check if both layers are known
	_, fromKnown := r.Layers[fromLayer]
	_, toKnown := r.Layers[toLayer]
	if !fromKnown || !toKnown {
		return true // unknown layer is not a cross-layer violation
	}

	allowedKey := fmt.Sprintf("%s -> %s", fromLayer, toLayer)
	for _, allowed := range r.AllowedCrossLayer {
		if allowed == allowedKey {
			return true
		}
	}
	return false
}

// GetSeverity returns the severity level for a violation type.
func (r *ArchRules) GetSeverity(vType ViolationType) SeverityLevel {
	if sev, ok := r.Severity[string(vType)]; ok {
		return SeverityLevel(sev)
	}
	return SeverityError // default to error
}

// fileExists checks if any file matches the given glob pattern.
func fileExists(pattern string) bool {
	matches, _ := doublestar.Glob(nil, pattern)
	return len(matches) > 0
}