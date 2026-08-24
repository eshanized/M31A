package codeintel

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// WorkspaceType represents the type of workspace detected.
type WorkspaceType string

const (
	WorkspaceTypeGoWork     WorkspaceType = "go_work"
	WorkspaceTypeNPM        WorkspaceType = "npm_workspaces"
	WorkspaceTypeCargo      WorkspaceType = "cargo_workspace"
	WorkspaceTypeM31A       WorkspaceType = "m31a"
	WorkspaceTypeSingleRepo WorkspaceType = "single_repo"
)

// WorkspaceRoot represents the detected workspace root directory and its type.
type WorkspaceRoot struct {
	Path string
	Type WorkspaceType
}

// DetectWorkspaceRoot detects the workspace root by walking up from startDir.
// Returns the first match found in order: .m31a/, go.work, package.json (workspaces), Cargo.toml (workspace).
// If none found, returns nil, nil (single-repo workspace).
func DetectWorkspaceRoot(startDir string) (*WorkspaceRoot, error) {
	dir := startDir
	for {
		// Check for .m31a/ directory (highest priority for M31A projects)
		m31aPath := filepath.Join(dir, ".m31a")
		if info, err := os.Stat(m31aPath); err == nil && info.IsDir() {
			return &WorkspaceRoot{Path: dir, Type: WorkspaceTypeM31A}, nil
		}

		// Check for go.work
		goWorkPath := filepath.Join(dir, "go.work")
		if info, err := os.Stat(goWorkPath); err == nil && !info.IsDir() {
			return &WorkspaceRoot{Path: dir, Type: WorkspaceTypeGoWork}, nil
		}

		// Check for package.json with workspaces field
		pkgJSONPath := filepath.Join(dir, "package.json")
		if info, err := os.Stat(pkgJSONPath); err == nil && !info.IsDir() {
			content, err := os.ReadFile(pkgJSONPath)
			if err == nil && hasWorkspacesField(content) {
				return &WorkspaceRoot{Path: dir, Type: WorkspaceTypeNPM}, nil
			}
		}

		// Check for Cargo.toml with [workspace] section
		cargoPath := filepath.Join(dir, "Cargo.toml")
		if info, err := os.Stat(cargoPath); err == nil && !info.IsDir() {
			content, err := os.ReadFile(cargoPath)
			if err == nil && hasCargoWorkspace(content) {
				return &WorkspaceRoot{Path: dir, Type: WorkspaceTypeCargo}, nil
			}
		}

		// Walk up to parent directory
		parent := filepath.Dir(dir)
		if parent == dir {
			break // reached filesystem root
		}
		dir = parent
	}

	// No workspace root found - single repo
	return nil, nil
}

// hasWorkspacesField checks if package.json has a "workspaces" field.
func hasWorkspacesField(content []byte) bool {
	// Simple string search - could be improved with JSON parsing
	return strings.Contains(string(content), `"workspaces"`)
}

// hasCargoWorkspace checks if Cargo.toml has a [workspace] section.
func hasCargoWorkspace(content []byte) bool {
	return strings.Contains(string(content), "[workspace]")
}

// WorkspaceRepo represents a single repository within a workspace.
type WorkspaceRepo struct {
	Path       string
	ModulePath string
	Indexer    *Indexer
}

// Workspace manages multiple repositories in a workspace.
type Workspace struct {
	Root  *WorkspaceRoot
	Repos map[string]*WorkspaceRepo
	mu    sync.RWMutex
}

// NewWorkspace creates a new workspace.
func NewWorkspace(root *WorkspaceRoot) *Workspace {
	return &Workspace{
		Root:  root,
		Repos: make(map[string]*WorkspaceRepo),
	}
}

// AddRepo adds a repository to the workspace and builds its index.
func (w *Workspace) AddRepo(path string) error {
	modulePath, err := detectModulePath(path)
	if err != nil {
		return fmt.Errorf("detect module path: %w", err)
	}

	indexer := NewIndexerWithStore(path, nil)
	if err := indexer.Build(context.Background()); err != nil {
		return fmt.Errorf("build index: %w", err)
	}

	w.mu.Lock()
	defer w.mu.Unlock()

	w.Repos[path] = &WorkspaceRepo{
		Path:       path,
		ModulePath: modulePath,
		Indexer:    indexer,
	}

	return nil
}

// detectModulePath detects the module path for a repository based on its language.
func detectModulePath(path string) (string, error) {
	// Check for go.mod
	goModPath := filepath.Join(path, "go.mod")
	if info, err := os.Stat(goModPath); err == nil && !info.IsDir() {
		content, err := os.ReadFile(goModPath)
		if err != nil {
			return "", err
		}
		for _, line := range strings.Split(string(content), "\n") {
			line = strings.TrimSpace(line)
			if strings.HasPrefix(line, "module ") {
				return strings.TrimSpace(strings.TrimPrefix(line, "module")), nil
			}
		}
	}

	// Check for package.json
	pkgJSONPath := filepath.Join(path, "package.json")
	if info, err := os.Stat(pkgJSONPath); err == nil && !info.IsDir() {
		content, err := os.ReadFile(pkgJSONPath)
		if err != nil {
			return "", err
		}
		// Simple extraction of "name" field
		for _, line := range strings.Split(string(content), "\n") {
			line = strings.TrimSpace(line)
			if strings.HasPrefix(line, `"name"`) {
				parts := strings.Split(line, `"`)
				if len(parts) >= 4 {
					return parts[3], nil
				}
			}
		}
	}

	// Check for Cargo.toml
	cargoPath := filepath.Join(path, "Cargo.toml")
	if info, err := os.Stat(cargoPath); err == nil && !info.IsDir() {
		content, err := os.ReadFile(cargoPath)
		if err != nil {
			return "", err
		}
		inPackage := false
		for _, line := range strings.Split(string(content), "\n") {
			line = strings.TrimSpace(line)
			if line == "[package]" {
				inPackage = true
				continue
			}
			if strings.HasPrefix(line, "[") {
				inPackage = false
			}
			if inPackage && strings.HasPrefix(line, "name ") {
				parts := strings.Split(line, `"`)
				if len(parts) >= 3 {
					return parts[1], nil
				}
			}
		}
	}

	// Fallback: use directory name
	return filepath.Base(path), nil
}

// ResolveCrossRepoImport resolves an import path to a local file path in another workspace repo.
// For Go: checks if importPath starts with any repo's ModulePath.
// Returns (repo path, resolved file path) or ("", "") if not a cross-repo import.
func (w *Workspace) ResolveCrossRepoImport(importPath string, fromRepo string) (string, string) {
	w.mu.RLock()
	defer w.mu.RUnlock()

	for repoPath, repo := range w.Repos {
		if repoPath == fromRepo {
			continue
		}
		if strings.HasPrefix(importPath, repo.ModulePath) {
			// Strip module path prefix and resolve to local file
			relPath := strings.TrimPrefix(importPath, repo.ModulePath)
			relPath = strings.TrimPrefix(relPath, "/")
			if relPath == "" {
				continue
			}

			// Try as directory first (package)
			candidate := filepath.Join(repoPath, relPath)
			if info, err := os.Stat(candidate); err == nil && info.IsDir() {
				return repoPath, filepath.ToSlash(filepath.Join(relPath, "index.go"))
			}
			// Try as file
			if _, err := os.Stat(candidate + ".go"); err == nil {
				return repoPath, filepath.ToSlash(relPath + ".go")
			}
			// Return the relative path even if file doesn't exist (for resolution purposes)
			return repoPath, filepath.ToSlash(relPath)
		}
	}
	return "", ""
}

// QueryAll iterates over all repositories in the workspace and calls fn for each.
// Returns combined errors from all calls.
func (w *Workspace) QueryAll(fn func(repo *WorkspaceRepo, indexer *Indexer) error) error {
	w.mu.RLock()
	defer w.mu.RUnlock()

	var errs []error
	for _, repo := range w.Repos {
		if err := fn(repo, repo.Indexer); err != nil {
			errs = append(errs, err)
		}
	}
	if len(errs) > 0 {
		return fmt.Errorf("query all: %v", errs)
	}
	return nil
}

// ImpactAll performs impact analysis for a symbol across all workspace repositories.
// Returns a map of repo path -> ImpactResult for repos where the symbol is defined or has dependents.
func (w *Workspace) ImpactAll(symbol string, depth int) map[string]*ImpactResult {
	w.mu.RLock()
	defer w.mu.RUnlock()

	results := make(map[string]*ImpactResult)

	for repoPath, repo := range w.Repos {
		// Check if symbol is defined in this repo
		locs := repo.Indexer.Define(symbol)
		if len(locs) == 0 {
			// Check if any file in this repo imports the symbol from another repo
			// This is a simplified check - in practice would need cross-repo import tracking
			continue
		}

		// Symbol is defined in this repo, analyze impact
		graph := repo.Indexer.Projection().Graph()
		result := AnalyzeImpact(graph, repo.Indexer.index, symbol, depth)
		if result != nil {
			results[repoPath] = result
		}
	}

	return results
}