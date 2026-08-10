package fileops

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// ResolveAndContainPath resolves a path (joining relative to workDir if needed),
// resolves symlinks, and verifies the result is within workDir. If the file does
// not exist, it attempts to resolve the parent directory through symlinks instead.
// Returns the resolved absolute path or an error if it escapes the workDir.
func ResolveAndContainPath(path, workDir string) (string, error) {
	joined := path
	if !filepath.IsAbs(path) {
		joined = filepath.Join(workDir, path)
	}
	targetPath, err := filepath.Abs(joined)
	if err != nil {
		return "", fmt.Errorf("cannot resolve path: %w", err)
	}

	resolved := targetPath
	if _, err := os.Stat(targetPath); err == nil {
		resolved, err = filepath.EvalSymlinks(targetPath)
		if err != nil {
			return "", fmt.Errorf("cannot resolve symlinks: %w", err)
		}
	} else if os.IsNotExist(err) {
		// File doesn't exist — resolve parent directory through symlinks
		parentDir := filepath.Dir(targetPath)
		if resolvedParent, parentErr := filepath.EvalSymlinks(parentDir); parentErr == nil {
			resolved = filepath.Join(resolvedParent, filepath.Base(targetPath))
		}
	} else {
		return "", fmt.Errorf("cannot stat path: %w", err)
	}

	if err := ContainedInWorkDir(resolved, workDir); err != nil {
		return "", err
	}
	return resolved, nil
}

// ResolveAndContainPathExists resolves a path and verifies it is within workDir.
// Unlike ResolveAndContainPath, this requires the path to already exist on disk.
func ResolveAndContainPathExists(path, workDir string) (string, error) {
	joined := path
	if !filepath.IsAbs(path) {
		joined = filepath.Join(workDir, path)
	}
	absPath, err := filepath.Abs(joined)
	if err != nil {
		return "", fmt.Errorf("cannot resolve path: %w", err)
	}

	resolved, err := filepath.EvalSymlinks(absPath)
	if err != nil {
		if os.IsNotExist(err) {
			return "", fmt.Errorf("file not found: %s", path)
		}
		return "", fmt.Errorf("cannot resolve path: %w", err)
	}

	if err := ContainedInWorkDir(resolved, workDir); err != nil {
		return "", err
	}
	return resolved, nil
}

// ContainedInWorkDir checks that resolved is either equal to workDir or is
// contained within it (with a trailing separator guard to prevent prefix attacks).
func ContainedInWorkDir(resolved, workDir string) error {
	// macOS t.TempDir() uses symlinks (e.g. /var -> /private/var)
	// We need to resolve workDir symlinks before comparing prefixes
	evalWorkDir := workDir
	if wd, err := filepath.EvalSymlinks(workDir); err == nil {
		evalWorkDir = wd
	}

	workDirPrefix := evalWorkDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != evalWorkDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return fmt.Errorf("path resolves outside working directory")
	}
	return nil
}
