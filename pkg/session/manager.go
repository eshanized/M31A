package session

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Manager provides CRUD operations for sessions stored on disk.
type Manager struct {
	baseDir         string // path to ~/.m31a/sessions
	sessionIDBytes  int    // number of random bytes for session IDs (default 4 = 8 hex chars)
	maxRecentModels int    // max recent models to track (default 10)

	// Session list cache (2-second TTL to avoid repeated filesystem walks)
	sessionCache     []SessionInfo
	sessionCacheTime time.Time
	cacheMu          sync.RWMutex
}

// ManagerOpts holds optional settings for the Manager.
type ManagerOpts struct {
	SessionIDBytes  int // Number of random bytes (4 = 8 hex chars). 0 = default.
	MaxRecentModels int // Max recent models. 0 = default (10).
}

// NewManager creates a Manager with the given base directory and optional settings.
func NewManager(baseDir string, opts ManagerOpts) *Manager {
	if opts.SessionIDBytes <= 0 {
		opts.SessionIDBytes = 4
	}
	if opts.MaxRecentModels <= 0 {
		opts.MaxRecentModels = 10
	}
	return &Manager{
		baseDir:         baseDir,
		sessionIDBytes:  opts.SessionIDBytes,
		maxRecentModels: opts.MaxRecentModels,
	}
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

	tmpFile, err := os.OpenFile(tmpPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0644)
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

// generateID generates a random hex session ID using crypto/rand.
// numBytes controls the length (4 bytes = 8 hex chars).
func generateID(numBytes int) (string, error) {
	b := make([]byte, numBytes)
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
		gid, err := generateID(m.sessionIDBytes)
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

	// L-13: Validate generated ID format
	if err := validateSessionID(id); err != nil {
		return nil, fmt.Errorf("generated invalid session ID: %w", err)
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

	// Invalidate list cache
	m.cacheMu.Lock()
	m.sessionCache = nil
	m.sessionCacheTime = time.Time{}
	m.cacheMu.Unlock()

	return session, nil
}

// LoadSession reads a session from disk and reconstructs it.
// Returns the session if found, or a specific error for each failure mode:
//   - ErrSessionNotFound: ID format invalid or directory missing
//   - ErrSessionCorrupted: JSON parse failure or missing required fields
//   - ErrSessionPermission: file access denied
//
// Sets ResumedAt to the current time on every successful load (M-6).
func (m *Manager) LoadSession(id string) (*Session, error) {
	if err := validateSessionID(id); err != nil {
		return nil, fmt.Errorf("invalid session ID %q: %w", id, m31errors.ErrSessionNotFound)
	}

	sessionPath := m.sessionJSONPath(id)

	data, err := os.ReadFile(sessionPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("session %s not found at %s: %w", id, sessionPath, m31errors.ErrSessionNotFound)
		}
		if os.IsPermission(err) {
			return nil, fmt.Errorf("cannot read session %s at %s: %w", id, sessionPath, m31errors.ErrSessionPermission)
		}
		return nil, fmt.Errorf("cannot read session %s: %w", id, err)
	}

	var session Session
	if err := json.Unmarshal(data, &session); err != nil {
		return nil, fmt.Errorf("corrupt JSON in session.json for session %s: %w", id, m31errors.ErrSessionCorrupted)
	}

	// H-17: Validate required fields.
	if session.ID == "" {
		return nil, fmt.Errorf("session %s has missing ID: %w", id, m31errors.ErrSessionCorrupted)
	}
	if session.StartedAt.IsZero() {
		return nil, fmt.Errorf("session %s has zero StartedAt: %w", id, m31errors.ErrSessionCorrupted)
	}
	// Validate WorkflowPhase is a known value.
	switch session.WorkflowPhase {
	case types.PhaseIdle, types.PhaseInitialize, types.PhaseDiscuss,
		types.PhasePlan, types.PhaseExecute, types.PhaseVerify, types.PhaseShip:
		// valid
	case "":
		// Empty phase defaults to idle — acceptable for legacy sessions
		session.WorkflowPhase = types.PhaseIdle
	default:
		return nil, fmt.Errorf("session %s has unknown WorkflowPhase %q: %w", id, session.WorkflowPhase, m31errors.ErrSessionCorrupted)
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

	// M-6: Record resume timestamp. ResumedAt is nil for first load,
	// updated to now on every subsequent load.
	now := time.Now()
	session.ResumedAt = &now

	return &session, nil
}

// UpdateWorkflowState persists the workflow's current goal, phase,
// and pending discuss questions to session.json. This is called
// on every phase transition by the TUI.
//
// Returns an error if the session doesn't exist or the write fails.
// The write is atomic (temp file + rename) so a crash mid-write
// leaves the existing session.json intact.
func (m *Manager) UpdateWorkflowState(id, goal string, phase types.WorkflowPhase, questions []string) error {
	session, err := m.LoadSession(id)
	if err != nil {
		return fmt.Errorf("UpdateWorkflowState: load session: %w", err)
	}
	session.SetWorkflowState(goal, phase, questions)
	return m.saveSessionAtomic(session)
}

// LoadWorkflowState reads just the workflow state fields from
// session.json. Returns the persisted values, or zero values
// (empty goal, PhaseIdle, empty questions) if the session
// doesn't exist or the workflow state is unset.
func (m *Manager) LoadWorkflowState(id string) (goal string, phase types.WorkflowPhase, questions []string, err error) {
	// Validate ID format — return zero values for invalid IDs
	if err := validateSessionID(id); err != nil {
		return "", types.PhaseIdle, nil, nil
	}
	session, err := m.LoadSession(id)
	if err != nil {
		if errors.Is(err, m31errors.ErrSessionCorrupted) {
			// Session doesn't exist yet — return zero values with no error
			return "", types.PhaseIdle, nil, nil
		}
		return "", types.PhaseIdle, nil, err
	}
	g, p, q := session.WorkflowState()
	return g, p, q, nil
}

// saveSessionAtomic writes the session metadata to session.json
// atomically (temp file + rename). Used by UpdateWorkflowState;
// also a building block for future session-modification operations.
// Note: this does NOT update messages.json — use SaveSession for that.
func (m *Manager) saveSessionAtomic(session *Session) error {
	data, err := json.Marshal(session)
	if err != nil {
		return fmt.Errorf("cannot marshal session: %w", err)
	}
	return m.atomicWrite(m.sessionJSONPath(session.ID), data)
}

// ListSessions returns all session directories sorted by last-modified descending.
// Session directories with missing or corrupt session.json are marked Corrupted=true.
func (m *Manager) ListSessions() ([]SessionInfo, error) {
	// Check cache first (2-second TTL)
	m.cacheMu.RLock()
	if time.Since(m.sessionCacheTime) < 2*time.Second && m.sessionCache != nil {
		result := make([]SessionInfo, len(m.sessionCache))
		copy(result, m.sessionCache)
		m.cacheMu.RUnlock()
		return result, nil
	}
	m.cacheMu.RUnlock()

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

	// Populate cache
	m.cacheMu.Lock()
	m.sessionCache = make([]SessionInfo, len(sessions))
	copy(m.sessionCache, sessions)
	m.sessionCacheTime = time.Now()
	m.cacheMu.Unlock()

	return sessions, nil
}

// DeleteSession removes the session directory and all its contents.
func (m *Manager) DeleteSession(id string) error {
	err := os.RemoveAll(m.basePathFor(id))
	if err == nil {
		m.cacheMu.Lock()
		m.sessionCache = nil
		m.sessionCacheTime = time.Time{}
		m.cacheMu.Unlock()
	}
	return err
}

// ArchiveSession moves the session directory into baseDir/archived/id.
func (m *Manager) ArchiveSession(id string) error {
	archiveDir := filepath.Join(m.baseDir, "archived")
	if err := m.ensureDir(archiveDir); err != nil {
		return fmt.Errorf("cannot create archive directory: %w", err)
	}
	err := os.Rename(m.basePathFor(id), filepath.Join(archiveDir, id))
	if err == nil {
		m.cacheMu.Lock()
		m.sessionCache = nil
		m.sessionCacheTime = time.Time{}
		m.cacheMu.Unlock()
	}
	return err
}

// ForkSession creates a child session from an existing parent session,
// copying the parent's messages and linking parent/child via ParentID/ChildrenIDs.
func (m *Manager) ForkSession(parentID string) (*Session, error) {
	// Load parent session
	parent, err := m.LoadSession(parentID)
	if err != nil {
		return nil, fmt.Errorf("cannot load parent session %q: %w", parentID, err)
	}

	// Generate unique ID (retry on collision)
	var newID string
	for i := 0; i < 10; i++ {
		gid, err := generateID(m.sessionIDBytes)
		if err != nil {
			return nil, err
		}
		if _, err := os.Stat(m.basePathFor(gid)); os.IsNotExist(err) {
			newID = gid
			break
		}
	}
	if newID == "" {
		return nil, fmt.Errorf("failed to generate unique session ID after 10 attempts")
	}

	// Create new session directory
	if err := m.ensureDir(m.basePathFor(newID)); err != nil {
		return nil, fmt.Errorf("cannot create session directory: %w", err)
	}

	// Build new session from parent
	newSession := NewSession(newID, parent.Model, parent.Provider)
	newSession.ParentID = parentID

	// Deep copy messages
	newSession.Messages = append([]types.Message{}, parent.Messages...)
	newSession.MessageCount = len(newSession.Messages)

	// Deep copy project state if present
	if parent.Project != nil {
		projData, err := json.Marshal(parent.Project)
		if err != nil {
			return nil, fmt.Errorf("cannot marshal parent project: %w", err)
		}
		var projCopy types.ProjectState
		if err := json.Unmarshal(projData, &projCopy); err != nil {
			return nil, fmt.Errorf("cannot unmarshal parent project: %w", err)
		}
		newSession.Project = &projCopy
	}

	// Add newID to parent ChildrenIDs if not already present
	found := false
	for _, cid := range parent.ChildrenIDs {
		if cid == newID {
			found = true
			break
		}
	}
	if !found {
		parent.ChildrenIDs = append(parent.ChildrenIDs, newID)
	}

	// Save both sessions atomically
	if err := m.SaveSession(newSession); err != nil {
		return nil, fmt.Errorf("cannot save child session: %w", err)
	}
	if err := m.SaveSession(parent); err != nil {
		return nil, fmt.Errorf("cannot save parent session: %w", err)
	}

	return newSession, nil
}

// SiblingSessions returns all child sessions of the parent (siblings of the
// given session including itself), plus the index of the given session within
// that list. Returns (nil, -1, nil) for root sessions (no parent).
func (m *Manager) SiblingSessions(sessionID string) ([]SessionInfo, int, error) {
	current, err := m.LoadSession(sessionID)
	if err != nil {
		return nil, -1, fmt.Errorf("cannot load session %q: %w", sessionID, err)
	}

	// Root sessions have no siblings
	if current.ParentID == "" {
		return nil, -1, nil
	}

	// ListChildren returns all children of the parent
	children, err := m.ListChildren(current.ParentID)
	if err != nil {
		return nil, -1, err
	}

	// Find index of sessionID in children list
	idx := -1
	for i, si := range children {
		if si.ID == sessionID {
			idx = i
			break
		}
	}

	return children, idx, nil
}

// ListChildren returns SessionInfo for all child sessions of the given parent session.
// Corrupt or deleted children are silently skipped.
func (m *Manager) ListChildren(parentID string) ([]SessionInfo, error) {
	parent, err := m.LoadSession(parentID)
	if err != nil {
		return nil, fmt.Errorf("cannot load parent session %q: %w", parentID, err)
	}

	var children []SessionInfo
	for _, cid := range parent.ChildrenIDs {
		child, err := m.LoadSession(cid)
		if err != nil {
			// Skip corrupt/deleted children gracefully
			continue
		}
		children = append(children, SessionInfo{
			ID:           child.ID,
			ParentID:     child.ParentID,
			ChildrenIDs:  child.ChildrenIDs,
			Model:        child.Model,
			Provider:     child.Provider,
			StartedAt:    child.StartedAt,
			MessageCount: child.MessageCount,
		})
	}

	return children, nil
}

// RecentModelsData stores the recent model list and favorites for quick model switching.
type RecentModelsData struct {
	Recent    []string        `json:"recent"`    // model IDs ordered most-recent-first, max 10
	Favorites map[string]bool `json:"favorites"` // model ID -> pinned
}

// recentModelsPath returns the path to the recent models file under ~/.m31a/.
func (m *Manager) recentModelsPath() string {
	return filepath.Join(filepath.Dir(m.baseDir), "recent_models.json")
}

// LoadRecentModels reads the recent models file from disk. If the file doesn't exist,
// it returns an empty RecentModelsData with initialized fields.
func (m *Manager) LoadRecentModels() (*RecentModelsData, error) {
	path := m.recentModelsPath()
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return &RecentModelsData{
				Recent:    []string{},
				Favorites: make(map[string]bool),
			}, nil
		}
		return nil, fmt.Errorf("cannot read recent models: %w", err)
	}

	var rmd RecentModelsData
	if err := json.Unmarshal(data, &rmd); err != nil {
		return nil, fmt.Errorf("cannot unmarshal recent models: %w", err)
	}
	if rmd.Recent == nil {
		rmd.Recent = []string{}
	}
	if rmd.Favorites == nil {
		rmd.Favorites = make(map[string]bool)
	}
	return &rmd, nil
}

// SaveRecentModels marshals and atomically writes the recent models data to disk.
// Recent slice is pruned to max recent models before saving.
func (m *Manager) SaveRecentModels(data *RecentModelsData) error {
	// Enforce max recent entries
	if len(data.Recent) > m.maxRecentModels {
		data.Recent = data.Recent[:m.maxRecentModels]
	}
	if data.Favorites == nil {
		data.Favorites = make(map[string]bool)
	}

	payload, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return fmt.Errorf("cannot marshal recent models: %w", err)
	}
	return m.atomicWrite(m.recentModelsPath(), payload)
}

// AddRecentModel adds a model ID to the top of the recent list. If the model
// already exists in the list, it is moved to the front (dedup). The list is
// capped at 10 entries.
func (m *Manager) AddRecentModel(modelID string) error {
	data, err := m.LoadRecentModels()
	if err != nil {
		return err
	}

	// Remove existing entry if present (dedup)
	var updated []string
	for _, id := range data.Recent {
		if id != modelID {
			updated = append(updated, id)
		}
	}
	// Prepend to front
	data.Recent = append([]string{modelID}, updated...)

	// Cap at max recent models
	if len(data.Recent) > m.maxRecentModels {
		data.Recent = data.Recent[:m.maxRecentModels]
	}

	return m.SaveRecentModels(data)
}

// ToggleFavorite toggles the favorite status for a model ID. If already
// favorited, it is removed; otherwise it is added.
func (m *Manager) ToggleFavorite(modelID string) error {
	data, err := m.LoadRecentModels()
	if err != nil {
		return err
	}

	if data.Favorites[modelID] {
		delete(data.Favorites, modelID)
	} else {
		data.Favorites[modelID] = true
	}

	return m.SaveRecentModels(data)
}

// IsFavorite checks whether a model ID is marked as a favorite.
func (m *Manager) IsFavorite(modelID string) bool {
	data, err := m.LoadRecentModels()
	if err != nil {
		return false
	}
	return data.Favorites[modelID]
}

// Cleanup removes session directories older than maxAge.
// Called on startup to prevent unbounded disk accumulation.
func (m *Manager) Cleanup(maxAge time.Duration) (int, error) {
	entries, err := os.ReadDir(m.baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, fmt.Errorf("read sessions dir: %w", err)
	}
	cutoff := time.Now().Add(-maxAge)
	removed := 0
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		if entry.Name() == "archived" {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		if info.ModTime().Before(cutoff) {
			path := filepath.Join(m.baseDir, entry.Name())
			if err := os.RemoveAll(path); err != nil {
				slog.Warn("failed to remove old session", "path", path, "error", err)
				continue
			}
			removed++
		}
	}
	if removed > 0 {
		m.cacheMu.Lock()
		m.sessionCache = nil
		m.sessionCacheTime = time.Time{}
		m.cacheMu.Unlock()
	}
	return removed, nil
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
