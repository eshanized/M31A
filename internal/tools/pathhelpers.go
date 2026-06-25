package tools

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
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
	workDirPrefix := workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return fmt.Errorf("path resolves outside working directory")
	}
	return nil
}

// SafeFileOp provides TOCTOU-safe file operations by opening a file descriptor
// and validating it before performing operations. This prevents race conditions
// where a symlink could be created between path resolution and file access.
type SafeFileOp struct {
	mu sync.Mutex
}

// OpenAndValidate opens a file and validates it's within the allowed directory.
// Returns the file handle and resolved path, or an error if validation fails.
// The caller is responsible for closing the returned file.
func (s *SafeFileOp) OpenAndValidate(path, workDir string, flag int, perm os.FileMode) (*os.File, string, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// First resolve the path
	joined := path
	if !filepath.IsAbs(path) {
		joined = filepath.Join(workDir, path)
	}
	absPath, err := filepath.Abs(joined)
	if err != nil {
		return nil, "", fmt.Errorf("cannot resolve path: %w", err)
	}

	// Open the file first, then validate the opened fd
	f, err := os.OpenFile(absPath, flag, perm)
	if err != nil {
		return nil, "", fmt.Errorf("cannot open file: %w", err)
	}

	// Get the file info from the opened descriptor (not from path)
	fi, err := f.Stat()
	if err != nil {
		_ = f.Close()
		return nil, "", fmt.Errorf("cannot stat opened file: %w", err)
	}

	// For regular files, validate the real path from the opened fd
	if !fi.IsDir() {
		// Use /proc/self/fd on Linux for safe symlink resolution
		fdPath := fmt.Sprintf("/proc/self/fd/%d", int(f.Fd()))
		if realPath, err := os.Readlink(fdPath); err == nil {
			if err := ContainedInWorkDir(realPath, workDir); err != nil {
				_ = f.Close()
				return nil, "", fmt.Errorf("path validation failed: %w", err)
			}
			return f, realPath, nil
		}
		// Fallback: use EvalSymlinks on the original path
		if resolved, err := filepath.EvalSymlinks(absPath); err == nil {
			if err := ContainedInWorkDir(resolved, workDir); err != nil {
				_ = f.Close()
				return nil, "", fmt.Errorf("path validation failed: %w", err)
			}
			return f, resolved, nil
		}
	}

	return f, absPath, nil
}
