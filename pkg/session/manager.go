package session

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
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

// sessionJSONPath returns the path to the session.json file for the given ID.
func (m *Manager) sessionJSONPath(id string) string {
	return filepath.Join(m.basePathFor(id), "session.json")
}

// messagesJSONPath returns the path to the messages.json file for the given ID.
func (m *Manager) messagesJSONPath(id string) string {
	return filepath.Join(m.basePathFor(id), "messages.json")
}

// planningDirPath returns the path to the planning/ subdirectory for the given ID.
func (m *Manager) planningDirPath(id string) string {
	return filepath.Join(m.basePathFor(id), "planning")
}

// generateID generates a random 8-character hex session ID using crypto/rand.
// Returns 4 bytes hex-encoded for 8 hex characters.
func generateID() (string, error) {
	b := make([]byte, 4)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("cannot generate session ID: %w", err)
	}
	return hex.EncodeToString(b), nil
}

// NewSession creates a new session with a generated ID, writes it to disk
// atomically, and returns the session.
func (m *Manager) NewSession(model, provider string) (*Session, error) {
	// Generate unique ID (retry on collision, astronomically unlikely)
	var id string
	for i := 0; i < 10; i++ {
		gid, err := generateID()
		if err != nil {
			return nil, err
		}
		// Check for collision
		if _, err := os.Stat(m.basePathFor(gid)); os.IsNotExist(err) {
			id = gid
			break
		}
	}
	if id == "" {
		return nil, fmt.Errorf("failed to generate unique session ID after 10 attempts")
	}

	// Create session directory
	sessionDir := m.basePathFor(id)
	if err := m.ensureDir(sessionDir); err != nil {
		return nil, fmt.Errorf("cannot create session directory: %w", err)
	}

	// Create session struct
	session := NewSession(id, model, provider)

	// Write session.json atomically
	data, err := json.Marshal(session)
	if err != nil {
		return nil, fmt.Errorf("cannot marshal session: %w", err)
	}
	if err := m.atomicWrite(m.sessionJSONPath(id), data); err != nil {
		return nil, fmt.Errorf("cannot write session.json: %w", err)
	}

	// Create planning/ subdirectory
	if err := m.ensureDir(m.planningDirPath(id)); err != nil {
		return nil, fmt.Errorf("cannot create planning directory: %w", err)
	}

	return session, nil
}

// LoadSession reads a session from disk and reconstructs it.
// Returns the session if found, or ErrSessionCorrupted if session.json is missing.
func (m *Manager) LoadSession(id string) (*Session, error) {
	sessionPath := m.sessionJSONPath(id)

	data, err := os.ReadFile(sessionPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, m31errors.ErrSessionCorrupted
		}
		return nil, fmt.Errorf("cannot read session.json: %w", err)
	}

	var session Session
	if err := json.Unmarshal(data, &session); err != nil {
		return nil, m31errors.ErrSessionCorrupted
	}

	// Try to load messages (graceful degradation if missing)
	msgData, err := os.ReadFile(m.messagesJSONPath(id))
	if err == nil {
		var messages []types.Message
		if err := json.Unmarshal(msgData, &messages); err == nil {
			session.Messages = messages
		}
	}
	// Update MessageCount to reflect loaded messages
	if session.Messages == nil {
		session.Messages = make([]types.Message, 0)
	}
	session.MessageCount = len(session.Messages)

	return &session, nil
}

// ListSessions returns all session directories sorted by last-modified descending.
// Session directories with missing or corrupt session.json are marked Corrupted=true.
func (m *Manager) ListSessions() ([]SessionInfo, error) {
	entries, err := os.ReadDir(m.baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			return []SessionInfo{}, nil
		}
		return nil, fmt.Errorf("cannot read sessions directory: %w", err)
	}

	var sessions []SessionInfo
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		if entry.Name() == "archived" {
			continue
		}

		info := SessionInfo{
			ID: entry.Name(),
		}

		// Try to read session.json
		sessionPath := m.sessionJSONPath(entry.Name())
		data, err := os.ReadFile(sessionPath)
		if err != nil {
			info.Corrupted = true
			// Use directory modtime as fallback
			if fi, statErr := entry.Info(); statErr == nil {
				info.LastModified = fi.ModTime()
			}
			sessions = append(sessions, info)
			continue
		}

		var s Session
		if err := json.Unmarshal(data, &s); err != nil {
			info.Corrupted = true
			if fi, statErr := entry.Info(); statErr == nil {
				info.LastModified = fi.ModTime()
			}
			sessions = append(sessions, info)
			continue
		}

		// Get last-modified from session.json file stat
		if fi, statErr := os.Stat(sessionPath); statErr == nil {
			info.LastModified = fi.ModTime()
		}

		info.Model = s.Model
		info.Provider = s.Provider
		info.StartedAt = s.StartedAt
		info.MessageCount = s.MessageCount
		sessions = append(sessions, info)
	}

	// Sort by LastModified descending
	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].LastModified.After(sessions[j].LastModified)
	})

	return sessions, nil
}

// DeleteSession removes the session directory and all its contents.
func (m *Manager) DeleteSession(id string) error {
	return os.RemoveAll(m.basePathFor(id))
}

// ArchiveSession moves the session directory into baseDir/archived/id.
func (m *Manager) ArchiveSession(id string) error {
	archiveDir := filepath.Join(m.baseDir, "archived")
	if err := m.ensureDir(archiveDir); err != nil {
		return fmt.Errorf("cannot create archive directory: %w", err)
	}
	return os.Rename(m.basePathFor(id), filepath.Join(archiveDir, id))
}

// SaveSession writes session.json and messages.json to disk atomically.
func (m *Manager) SaveSession(s *Session) error {
	// Save session metadata
	data, err := json.Marshal(s)
	if err != nil {
		return fmt.Errorf("cannot marshal session: %w", err)
	}
	if err := m.atomicWrite(m.sessionJSONPath(s.ID), data); err != nil {
		return fmt.Errorf("cannot write session.json: %w", err)
	}

	// Save messages
	if s.Messages == nil {
		s.Messages = make([]types.Message, 0)
	}
	msgData, err := json.Marshal(s.Messages)
	if err != nil {
		return fmt.Errorf("cannot marshal messages: %w", err)
	}
	if err := m.atomicWrite(m.messagesJSONPath(s.ID), msgData); err != nil {
		return fmt.Errorf("cannot write messages.json: %w", err)
	}

	return nil
}
