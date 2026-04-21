package session

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/fileutil"
	"github.com/eshanized/M31A/internal/types"
)

// Manager provides CRUD operations for sessions stored on disk.
type Manager struct {
	baseDir         string        // path to ~/.m31a/sessions
	sessionIDBytes  int           // number of random bytes for session IDs (default 4 = 8 hex chars)
	maxRecentModels int           // max recent models to track (default 10)
	sessionCacheTTL time.Duration // TTL for session list cache

	// Session list cache (configurable TTL to avoid repeated filesystem walks)
	sessionCache     []SessionInfo
	sessionCacheTime time.Time
	cacheMu          sync.RWMutex
	refreshMu        sync.Mutex // serializes cache refresh to prevent redundant walks
}

// ManagerOpts holds optional settings for the Manager.
type ManagerOpts struct {
	SessionIDBytes  int           // Number of random bytes (4 = 8 hex chars). 0 = default.
	MaxRecentModels int           // Max recent models. 0 = default (10).
	SessionCacheTTL time.Duration // TTL for session list cache. 0 = default (2s).
}

// NewManager creates a Manager with the given base directory and optional settings.
func NewManager(baseDir string, opts ManagerOpts) *Manager {
	if opts.SessionIDBytes <= 0 {
		opts.SessionIDBytes = 4
	}
	if opts.MaxRecentModels <= 0 {
		opts.MaxRecentModels = types.DefaultMaxRecentModels
	}
	if opts.SessionCacheTTL <= 0 {
		opts.SessionCacheTTL = types.DefaultSessionCacheTTL
	}
	return &Manager{
		baseDir:         baseDir,
		sessionIDBytes:  opts.SessionIDBytes,
		maxRecentModels: opts.MaxRecentModels,
		sessionCacheTTL: opts.SessionCacheTTL,
	}
}

// basePathFor returns the directory path for the given session ID.
func (m *Manager) basePathFor(id string) string {
	return filepath.Join(m.baseDir, id)
}

// BaseDir returns the directory path that holds all session folders.
// Used by callers that need to locate session data on disk (e.g. the
// auto-backup feature copies the session tree to a sibling directory).
func (m *Manager) BaseDir() string {
	return m.baseDir
}

// atomicWrite atomically writes data to path by writing to a temp file in the
// same directory then renaming. Delegates to fileutil.AtomicWrite.
func (m *Manager) atomicWrite(path string, data []byte) error {
	return fileutil.AtomicWrite(path, data)
}

// ensureDir creates the directory at path (including parents) with DirPermission perms.
func (m *Manager) ensureDir(path string) error {
	return os.MkdirAll(path, types.DirPermission)
}

// readFileLimited reads a file with a size limit to prevent OOM from corrupted
// or maliciously crafted session files. Returns the file contents or an error.
// Pre-allocates the buffer when file size is known to avoid repeated reallocation.
func readFileLimited(path string, maxBytes int64) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	// Pre-allocate buffer based on file size when known (A5)
	fi, statErr := f.Stat()
	var data []byte
	if statErr == nil && fi.Size() > 0 && fi.Size() <= maxBytes {
		data = make([]byte, 0, fi.Size())
		limited := io.LimitReader(f, maxBytes+1)
		data, err = io.ReadAll(limited)
	} else {
		limited := io.LimitReader(f, maxBytes+1)
		data, err = io.ReadAll(limited)
	}
	if err != nil {
		return nil, err
	}
	if int64(len(data)) > maxBytes {
		return nil, fmt.Errorf("file %s exceeds maximum size limit of %d bytes", path, maxBytes)
	}
	return data, nil
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

	// Validate generated ID format
	if err := validateSessionID(id, m.sessionIDBytes*2); err != nil {
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
	if err := validateSessionID(id, m.sessionIDBytes*2); err != nil {
		return nil, fmt.Errorf("invalid session ID %q: %w", id, m31errors.ErrSessionNotFound)
	}

	sessionPath := m.sessionJSONPath(id)

	data, err := readFileLimited(sessionPath, types.MaxSessionFileSize)
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

	// Validate required fields.
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
	msgData, err := readFileLimited(m.messagesJSONPath(id), types.MaxSessionFileSize)
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

	// Record resume timestamp. ResumedAt is nil for first load,
	// updated to now on every subsequent load.
	now := time.Now()
	session.ResumedAt = &now

	return &session, nil
}

// loadSessionMetadata reads only session.json without loading messages.json.
// This is an optimized path for operations that only need session metadata
// (e.g., workflow state updates, session list).
func (m *Manager) loadSessionMetadata(id string) (*Session, error) {
	if err := validateSessionID(id, m.sessionIDBytes*2); err != nil {
		return nil, fmt.Errorf("invalid session ID %q: %w", id, m31errors.ErrSessionNotFound)
	}

	sessionPath := m.sessionJSONPath(id)
	data, err := readFileLimited(sessionPath, types.MaxSessionFileSize)
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

	if session.ID == "" {
		return nil, fmt.Errorf("session %s has missing ID: %w", id, m31errors.ErrSessionCorrupted)
	}
	if session.StartedAt.IsZero() {
		return nil, fmt.Errorf("session %s has zero StartedAt: %w", id, m31errors.ErrSessionCorrupted)
	}

	switch session.WorkflowPhase {
	case types.PhaseIdle, types.PhaseInitialize, types.PhaseDiscuss,
		types.PhasePlan, types.PhaseExecute, types.PhaseVerify, types.PhaseShip:
	case "":
		session.WorkflowPhase = types.PhaseIdle
	default:
		return nil, fmt.Errorf("session %s has unknown WorkflowPhase %q: %w", id, session.WorkflowPhase, m31errors.ErrSessionCorrupted)
	}

	return &session, nil
}

// UpdateWorkflowState persists the workflow's current goal, phase,
// and pending discuss questions to session.json. This is called
// on every phase transition by the TUI.
//
// Returns an error if the session doesn't exist or the write fails.
// The write is atomic (temp file + rename) so a crash mid-write
// leaves the existing session.json intact.
// Optimized to load only session.json metadata, not messages.
func (m *Manager) UpdateWorkflowState(id, goal string, phase types.WorkflowPhase, questions []string) error {
	session, err := m.loadSessionMetadata(id)
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
// Optimized to load only session.json metadata, not messages.
func (m *Manager) LoadWorkflowState(id string) (goal string, phase types.WorkflowPhase, questions []string, err error) {
	// Validate ID format — return zero values for invalid IDs
	if err := validateSessionID(id, m.sessionIDBytes*2); err != nil {
		return "", types.PhaseIdle, nil, nil
	}
	session, err := m.loadSessionMetadata(id)
	if err != nil {
		if errors.Is(err, m31errors.ErrSessionCorrupted) || errors.Is(err, m31errors.ErrSessionNotFound) {
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
	// Check cache first (configurable TTL)
	m.cacheMu.RLock()
	if time.Since(m.sessionCacheTime) < m.sessionCacheTTL && m.sessionCache != nil {
		result := make([]SessionInfo, len(m.sessionCache))
		copy(result, m.sessionCache)
		m.cacheMu.RUnlock()
		return result, nil
	}
	m.cacheMu.RUnlock()

	// Serialize refreshes so concurrent callers don't all do filesystem walks
	m.refreshMu.Lock()
	defer m.refreshMu.Unlock()

	// Re-check cache: another goroutine may have refreshed while we waited
	m.cacheMu.RLock()
	if time.Since(m.sessionCacheTime) < m.sessionCacheTTL && m.sessionCache != nil {
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

		// Try to read session.json using optimized metadata-only path (H8 fix)
		sess, err := m.loadSessionMetadata(entry.Name())
		if err != nil {
			info.Corrupted = true
			// Use directory modtime as fallback
			if fi, statErr := entry.Info(); statErr == nil {
				info.LastModified = fi.ModTime()
			}
			sessions = append(sessions, info)
			continue
		}

		// Get last-modified from session.json file stat
		// Use DirEntry.Info() which may be cached by the OS, avoiding a separate stat syscall
		if fi, statErr := entry.Info(); statErr == nil {
			info.LastModified = fi.ModTime()
		}

		info.Model = sess.Model
		info.Provider = sess.Provider
		info.StartedAt = sess.StartedAt
		info.MessageCount = sess.MessageCount
		info.WorkflowPhase = sess.WorkflowPhase
		info.Label = sess.Label
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
// If the destination already exists (e.g. rerunning ship on the same ID),
// a timestamp suffix is appended to avoid overwriting the previous archive.
func (m *Manager) ArchiveSession(id string) error {
	archiveDir := filepath.Join(m.baseDir, "archived")
	if err := m.ensureDir(archiveDir); err != nil {
		return fmt.Errorf("cannot create archive directory: %w", err)
	}
	src := m.basePathFor(id)

	// Bail out early when the source doesn't exist — os.Rename would fail
	// anyway, but this gives a clearer error message.
	if _, statErr := os.Stat(src); os.IsNotExist(statErr) {
		return fmt.Errorf("session %q does not exist: cannot archive", id)
	}

	dst := filepath.Join(archiveDir, id)
	if _, err := os.Stat(dst); err == nil {
		// Destination exists — disambiguate with a timestamp suffix.
		dst = filepath.Join(archiveDir, fmt.Sprintf("%s.%s", id, time.Now().Format("20060102T150405")))
	}
	err := os.Rename(src, dst)
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

	// C-4: Marshal both sessions before writing either to avoid orphaned child
	// if a crash occurs between the two writes.
	if newSession.Messages == nil {
		newSession.Messages = make([]types.Message, 0)
	}
	childMsgData, err := json.Marshal(newSession.Messages)
	if err != nil {
		return nil, fmt.Errorf("cannot marshal child messages: %w", err)
	}
	childData, err := json.Marshal(newSession)
	if err != nil {
		return nil, fmt.Errorf("cannot marshal child session: %w", err)
	}

	if parent.Messages == nil {
		parent.Messages = make([]types.Message, 0)
	}
	parentMsgData, err := json.Marshal(parent.Messages)
	if err != nil {
		return nil, fmt.Errorf("cannot marshal parent messages: %w", err)
	}
	parentData, err := json.Marshal(parent)
	if err != nil {
		return nil, fmt.Errorf("cannot marshal parent session: %w", err)
	}

	// Write child session files
	if err := m.atomicWrite(m.sessionJSONPath(newSession.ID), childData); err != nil {
		return nil, fmt.Errorf("cannot save child session: %w", err)
	}
	if err := m.atomicWrite(m.messagesJSONPath(newSession.ID), childMsgData); err != nil {
		return nil, fmt.Errorf("cannot save child messages: %w", err)
	}

	// Write parent session files
	if err := m.atomicWrite(m.sessionJSONPath(parent.ID), parentData); err != nil {
		return nil, fmt.Errorf("cannot save parent session: %w", err)
	}
	if err := m.atomicWrite(m.messagesJSONPath(parent.ID), parentMsgData); err != nil {
		return nil, fmt.Errorf("cannot save parent messages: %w", err)
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
	data, err := readFileLimited(path, types.MaxSessionFileSize)
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
// C-3: Both payloads are marshalled before any writes so a crash between
// writes never leaves session.json with a stale messages.json.
func (m *Manager) SaveSession(s *Session) error {
	if s.Messages == nil {
		s.Messages = make([]types.Message, 0)
	}
	msgData, err := json.Marshal(s.Messages)
	if err != nil {
		return fmt.Errorf("cannot marshal messages: %w", err)
	}

	// Marshal session metadata (contains MessageCount derived from Messages)
	data, err := json.Marshal(s)
	if err != nil {
		return fmt.Errorf("cannot marshal session: %w", err)
	}

	// Write both atomically: temp files first, then rename
	if err := m.atomicWrite(m.sessionJSONPath(s.ID), data); err != nil {
		return fmt.Errorf("cannot write session.json: %w", err)
	}
	if err := m.atomicWrite(m.messagesJSONPath(s.ID), msgData); err != nil {
		return fmt.Errorf("cannot write messages.json: %w", err)
	}

	return nil
}

// FilterSessions returns sessions matching a query string (searches ID, label, and model).
func (m *Manager) FilterSessions(query string) ([]SessionInfo, error) {
	all, err := m.ListSessions()
	if err != nil {
		return nil, err
	}
	if query == "" {
		return all, nil
	}
	q := strings.ToLower(query)
	var filtered []SessionInfo
	for _, s := range all {
		if strings.Contains(strings.ToLower(s.ID), q) ||
			strings.Contains(strings.ToLower(s.Label), q) ||
			strings.Contains(strings.ToLower(s.Model), q) {
			filtered = append(filtered, s)
		}
	}
	return filtered, nil
}

// ExportSessionMarkdown exports a session's messages as a markdown file.
func (m *Manager) ExportSessionMarkdown(id, path string) error {
	sess, err := m.LoadSession(id)
	if err != nil {
		return fmt.Errorf("load session: %w", err)
	}
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("# Session: %s\n\n", id))
	sb.WriteString(fmt.Sprintf("**Model:** %s  \n**Provider:** %s  \n**Started:** %s  \n**Messages:** %d\n\n---\n\n",
		sess.Model, sess.Provider, sess.StartedAt.Format(time.RFC3339), sess.MessageCount))
	for _, msg := range sess.Messages {
		sb.WriteString(fmt.Sprintf("## %s\n\n%s\n\n---\n\n", msg.Role, msg.Content))
	}
	return m.atomicWrite(path, []byte(sb.String()))
}

// ExportSessionJSON exports a session's full data as a JSON file.
func (m *Manager) ExportSessionJSON(id, path string) error {
	sess, err := m.LoadSession(id)
	if err != nil {
		return fmt.Errorf("load session: %w", err)
	}
	data, err := json.MarshalIndent(sess, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal session: %w", err)
	}
	return m.atomicWrite(path, data)
}

// RenameSession updates a session's label.
func (m *Manager) RenameSession(id, label string) error {
	sess, err := m.LoadSession(id)
	if err != nil {
		return fmt.Errorf("load session: %w", err)
	}
	sess.Label = label
	return m.saveSessionAtomic(sess)
}
