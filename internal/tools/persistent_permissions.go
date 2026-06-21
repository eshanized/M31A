package tools

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/types"
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

// Save adds a permission rule for the given project directory and persists it.
func (p *PersistentPermissions) Save(projectDir string, rule config.PermissionRule) error {
	if p.path == "" {
		return fmt.Errorf("no path configured for persistent permissions")
	}

	var pd persistentData

	if data, err := os.ReadFile(p.path); err == nil {
		_ = json.Unmarshal(data, &pd)
	}
	if pd.Projects == nil {
		pd.Projects = make(map[string][]config.PermissionRule)
	}

	pd.Projects[projectDir] = append(pd.Projects[projectDir], rule)

	out, err := json.MarshalIndent(pd, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal persistent permissions: %w", err)
	}

	if err := os.MkdirAll(filepath.Dir(p.path), types.DirPermission); err != nil {
		return fmt.Errorf("create persistent permissions dir: %w", err)
	}

	return os.WriteFile(p.path, out, types.FilePermission)
}

// Remove clears all persistent rules for the given project directory.
func (p *PersistentPermissions) Remove(projectDir string) error {
	if p.path == "" {
		return nil
	}

	var pd persistentData
	data, err := os.ReadFile(p.path)
	if err != nil {
		return nil
	}
	if unmarshalErr := json.Unmarshal(data, &pd); unmarshalErr != nil {
		return nil
	}

	delete(pd.Projects, projectDir)

	out, err := json.MarshalIndent(pd, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(p.path, out, types.FilePermission)
}
