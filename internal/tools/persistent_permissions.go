package tools

import (
	"encoding/json"
	"os"
	"path/filepath"

	"github.com/eshanized/M31A/internal/config"
)

// PersistentPermissions manages saved permission rules that survive across
// sessions. Rules are scoped by project directory and stored in
// ~/.m31a/permissions.json.
type PersistentPermissions struct {
	path string
}

// persistentData is the on-disk format for persistent permissions.
type persistentData struct {
	Projects map[string][]config.PermissionRule `json:"projects"`
}

// NewPersistentPermissions creates a PersistentPermissions instance that
// reads/writes to ~/.m31a/permissions.json.
func NewPersistentPermissions() *PersistentPermissions {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return &PersistentPermissions{path: ""}
	}
	return &PersistentPermissions{
		path: filepath.Join(homeDir, ".m31a", "permissions.json"),
	}
}

// Load reads persistent permission rules for the given project directory.
func (p *PersistentPermissions) Load(projectDir string) []config.PermissionRule {
	if p.path == "" {
		return nil
	}

	data, err := os.ReadFile(p.path)
	if err != nil {
		return nil
	}

	var pd persistentData
	if err := json.Unmarshal(data, &pd); err != nil {
		return nil
	}

	return pd.Projects[projectDir]
}

// Save persists permission rules for the given project directory.
func (p *PersistentPermissions) Save(projectDir string, rules []config.PermissionRule) error {
	if p.path == "" {
		return nil
	}

	// Ensure directory exists
	dir := filepath.Dir(p.path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	// Read existing data to preserve other projects
	var pd persistentData
	data, err := os.ReadFile(p.path)
	if err == nil {
		_ = json.Unmarshal(data, &pd)
	}

	// Initialize map if needed
	if pd.Projects == nil {
		pd.Projects = make(map[string][]config.PermissionRule)
	}

	// Update rules for this project
	pd.Projects[projectDir] = rules

	// Write back to file
	data, err = json.MarshalIndent(pd, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(p.path, data, 0644)
}
