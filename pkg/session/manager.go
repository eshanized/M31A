package session

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
)

// Manager provides CRUD operations for sessions stored on disk.
type Manager struct {
	baseDir string // path to ~/.m31a/sessions
}

// NewManager creates a Manager with the given base directory.
func NewManager(baseDir string) *Manager {
	return &Manager{baseDir: baseDir}
}

// basePathFor returns the directory path for the given session ID.
func (m *Manager) basePathFor(id string) string {
	return filepath.Join(m.baseDir, id)
}

// atomicWrite atomically writes data to path by writing to a temp file in the
// same directory then renaming. The temp file uses crypto/rand for a unique name.
func (m *Manager) atomicWrite(path string, data []byte) (err error) {
	dir := filepath.Dir(path)

	// Generate random temp name in the same directory (cross-device safety)
	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return fmt.Errorf("cannot generate temp name: %w", err)
	}
	tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))

	// Clean up temp file on any error
	cleanup := true
	defer func() {
		if cleanup {
			os.Remove(tmpPath) // best-effort cleanup
		}
	}()

	tmpFile, err := os.Create(tmpPath)
	if err != nil {
		return fmt.Errorf("cannot create temp file: %w", err)
	}

	if _, err := tmpFile.Write(data); err != nil {
		tmpFile.Close()
		return fmt.Errorf("temp write failed: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
		return fmt.Errorf("temp fsync failed: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("temp close failed: %w", err)
	}

	// Atomic rename
	if err := os.Rename(tmpPath, path); err != nil {
		return fmt.Errorf("rename failed: %w", err)
	}

	cleanup = false
	return nil
}

// ensureDir creates the directory at path (including parents) with 0755 perms.
func (m *Manager) ensureDir(path string) error {
	return os.MkdirAll(path, 0755)
}
