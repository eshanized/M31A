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
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/coordinator"
)

// Manager provides session operations. Sessions are stored project-locally
// in <workDir>/.m31a/ as flat files (one session per project).
// Global config (recent models, etc.) remains in <baseDir> (~/.m31a/).
type Manager struct {
	baseDir         string        // path to ~/.m31a (global config)
	workDir         string        // path to project root
	sessionIDBytes  int           // number of random bytes for session IDs (default 4 = 8 hex chars)
	maxRecentModels int           // max recent models to track (default 10)
	sessionCacheTTL time.Duration // TTL for session list cache (kept for API compatibility)
	lock            *fileLock
	coordinator     *coordinator.Coordinator[string] // per-session concurrency control
	checkpointMu    sync.Mutex                       // protects checkpoint read-modify-write
}

// ManagerOpts holds optional settings for the Manager.
type ManagerOpts struct {
	SessionIDBytes         int           // Number of random bytes (4 = 8 hex chars). 0 = default.
	MaxRecentModels        int           // Max recent models. 0 = default (10).
	SessionCacheTTL        time.Duration // TTL for session list cache. 0 = default (2s).
	CoordinatorTimeoutSecs int           // Safety timeout for coordinator. 0 = default (300s).
}

// NewManager creates a Manager with a global config directory and a project
// working directory. Session data is stored in <workDir>/.m31a/.
func NewManager(baseDir, workDir string, opts ManagerOpts) *Manager {
	if opts.SessionIDBytes <= 0 {
		opts.SessionIDBytes = 4
	}
	if opts.MaxRecentModels <= 0 {
		opts.MaxRecentModels = types.DefaultMaxRecentModels
	}
	if opts.SessionCacheTTL <= 0 {
		opts.SessionCacheTTL = types.DefaultSessionCacheTTL
	}
	projectDir := filepath.Join(workDir, ".m31a")
	coord := coordinator.New[string]()
	if opts.CoordinatorTimeoutSecs > 0 {
		coord.SafetyTimeout = time.Duration(opts.CoordinatorTimeoutSecs) * time.Second
	}
	return &Manager{
		baseDir:         baseDir,
		workDir:         workDir,
		sessionIDBytes:  opts.SessionIDBytes,
		maxRecentModels: opts.MaxRecentModels,
		sessionCacheTTL: opts.SessionCacheTTL,
		lock:            newFileLock(filepath.Join(projectDir, "session.lock")),
		coordinator:     coord,
	}
}

// projectDir returns the project-local session directory: <workDir>/.m31a/.
func (m *Manager) projectDir() string {
	return filepath.Join(m.workDir, ".m31a")
}

// BaseDir returns the global config directory (~/.m31a/).
func (m *Manager) BaseDir() string {
	return m.baseDir
}

// WorkDir returns the project root directory.
func (m *Manager) WorkDir() string {
	return m.workDir
}

// Coordinator returns the session run coordinator for managing concurrent
// session execution. Use Run/Interrupt/AwaitIdle to control session drains.
func (m *Manager) Coordinator() *coordinator.Coordinator[string] {
	return m.coordinator
}

// sessionJSONPath returns the path to session.json in the project directory.
func (m *Manager) sessionJSONPath() string {
	return filepath.Join(m.projectDir(), "session.json")
}

// messagesJSONPath returns the path to messages.json in the project directory.
func (m *Manager) messagesJSONPath() string {
	return filepath.Join(m.projectDir(), "messages.json")
}

// planningDirPath returns the planning directory path (same as projectDir for flat layout).
func (m *Manager) planningDirPath() string {
	return m.projectDir()
}

// atomicWrite atomically writes data to path by writing to a temp file then renaming.
func (m *Manager) atomicWrite(path string, data []byte) error {
	return atomicWrite(path, data)
}

// ensureDir creates the directory at path (including parents) with DirPermission perms.
func (m *Manager) ensureDir(path string) error {
	return os.MkdirAll(path, types.DirPermission)
}

// readFileLimited reads a file with a size limit to prevent OOM.
func readFileLimited(path string, maxBytes int64) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer func() {
		if closeErr := f.Close(); closeErr != nil {
			slog.Debug("close file", "error", closeErr, "path", path)
		}
	}()

	fi, statErr := f.Stat()
	var data []byte
	if statErr == nil && fi.Size() > maxBytes {
		return nil, fmt.Errorf("file exceeds maximum size limit: %d bytes (max %d)", fi.Size(), maxBytes)
	}
	limited := io.LimitReader(f, maxBytes+1)
	data, err = io.ReadAll(limited)
	if err != nil {
		return nil, err
	}
	if int64(len(data)) > maxBytes {
		return nil, fmt.Errorf("file %s exceeds maximum size limit of %d bytes", path, maxBytes)
	}
	return data, nil
}

// generateID generates a random hex session ID using crypto/rand.
func generateID(numBytes int) (string, error) {
	b := make([]byte, numBytes)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("cannot generate session ID: %w", err)
	}
	return hex.EncodeToString(b), nil
}

// NewSession creates a new session in <workDir>/.m31a/. If a session already
// exists, the existing session.json is backed up to session.json.bak.
func (m *Manager) NewSession(model, provider string) (*Session, error) {
	if err := m.lock.Lock(); err != nil {
		return nil, fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	id, err := generateID(m.sessionIDBytes)
	if err != nil {
		return nil, err
	}

	dir := m.projectDir()
	if dirErr := m.ensureDir(dir); dirErr != nil {
		return nil, fmt.Errorf("cannot create project directory: %w", dirErr)
	}

	// Backup existing session if present
	sessPath := m.sessionJSONPath()
	if _, statErr := os.Stat(sessPath); statErr == nil {
		bakPath := sessPath + ".bak"
		_ = os.Rename(sessPath, bakPath)
	}

	session := NewSession(id, model, provider)

	data, err := json.Marshal(session)
	if err != nil {
		return nil, fmt.Errorf("cannot marshal session: %w", err)
	}
	if err := m.atomicWrite(sessPath, data); err != nil {
		return nil, fmt.Errorf("cannot write session.json: %w", err)
	}

	// Write empty messages.json
	msgData, jsonErr := json.Marshal([]types.Message{})
	if jsonErr != nil {
		return nil, fmt.Errorf("cannot marshal empty messages: %w", jsonErr)
	}
	if err := m.atomicWrite(m.messagesJSONPath(), msgData); err != nil {
		return nil, fmt.Errorf("cannot write messages.json: %w", err)
	}

	m.ensureGitIgnore()

	return session, nil
}

// LoadSession reads the project-local session from <workDir>/.m31a/session.json.
// The id parameter is accepted for API compatibility but ignored — the project
// directory contains at most one session.
func (m *Manager) LoadSession(id string) (*Session, error) {
	if err := m.lock.Lock(); err != nil {
		return nil, fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	sessPath := m.sessionJSONPath()
	data, err := readFileLimited(sessPath, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("no session found in project directory: %w", m31errors.ErrSessionNotFound)
		}
		if os.IsPermission(err) {
			return nil, fmt.Errorf("cannot read session: %w", m31errors.ErrSessionPermission)
		}
		return nil, fmt.Errorf("cannot read session.json: %w", err)
	}

	var session Session
	if err := json.Unmarshal(data, &session); err != nil {
		return nil, fmt.Errorf("corrupt session.json: %w", m31errors.ErrSessionCorrupted)
	}

	if session.ID == "" {
		return nil, fmt.Errorf("session has missing ID: %w", m31errors.ErrSessionCorrupted)
	}
	if session.StartedAt.IsZero() {
		return nil, fmt.Errorf("session has zero StartedAt: %w", m31errors.ErrSessionCorrupted)
	}

	switch session.WorkflowPhase {
	case types.PhaseIdle, types.PhaseInitialize, types.PhaseDiscuss,
		types.PhasePlan, types.PhaseExecute, types.PhaseVerify, types.PhaseRuntime, types.PhaseShip:
	case "":
		session.WorkflowPhase = types.PhaseIdle
	default:
		return nil, fmt.Errorf("session has unknown WorkflowPhase %q: %w", session.WorkflowPhase, m31errors.ErrSessionCorrupted)
	}

	// Load messages
	msgData, msgErr := readFileLimited(m.messagesJSONPath(), types.MaxSessionFileSize)
	if msgErr == nil {
		var messages []types.Message
		if err := json.Unmarshal(msgData, &messages); err == nil {
			session.Messages = messages
		} else {
			slog.Warn("messages.json parse failed, loading empty message history",
				"session", session.ID, "error", err)
		}
	} else if msgErr != nil && !os.IsNotExist(msgErr) {
		slog.Warn("messages.json read failed", "session", session.ID, "error", msgErr)
	}
	if session.Messages == nil {
		session.Messages = make([]types.Message, 0)
	}
	session.MessageCount = len(session.Messages)

	now := time.Now()
	session.ResumedAt = &now

	return &session, nil
}

// loadSessionMetadata reads only session.json without loading messages.
func (m *Manager) loadSessionMetadata() (*Session, error) {
	sessPath := m.sessionJSONPath()
	data, err := readFileLimited(sessPath, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("no session found: %w", m31errors.ErrSessionNotFound)
		}
		return nil, fmt.Errorf("cannot read session.json: %w", err)
	}

	var session Session
	if err := json.Unmarshal(data, &session); err != nil {
		return nil, fmt.Errorf("corrupt session.json: %w", m31errors.ErrSessionCorrupted)
	}

	if session.ID == "" {
		return nil, fmt.Errorf("session has missing ID: %w", m31errors.ErrSessionCorrupted)
	}
	if session.StartedAt.IsZero() {
		return nil, fmt.Errorf("session has zero StartedAt: %w", m31errors.ErrSessionCorrupted)
	}

	switch session.WorkflowPhase {
	case types.PhaseIdle, types.PhaseInitialize, types.PhaseDiscuss,
		types.PhasePlan, types.PhaseExecute, types.PhaseVerify, types.PhaseRuntime, types.PhaseShip:
	case "":
		session.WorkflowPhase = types.PhaseIdle
	default:
		return nil, fmt.Errorf("session has unknown WorkflowPhase %q: %w", session.WorkflowPhase, m31errors.ErrSessionCorrupted)
	}

	return &session, nil
}

// UpdateWorkflowState persists the workflow state to session.json.
func (m *Manager) UpdateWorkflowState(id, goal string, phase types.WorkflowPhase, questions []string) error {
	if err := m.lock.Lock(); err != nil {
		return fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	session, err := m.loadSessionMetadata()
	if err != nil {
		return fmt.Errorf("UpdateWorkflowState: load session: %w", err)
	}
	session.SetWorkflowState(goal, phase, questions)
	return m.saveSessionAtomic(session)
}

// LoadWorkflowState reads the persisted workflow state from session.json.
func (m *Manager) LoadWorkflowState(id string) (goal string, phase types.WorkflowPhase, questions []string, err error) {
	if err := m.lock.Lock(); err != nil {
		return "", types.PhaseIdle, nil, fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	session, loadErr := m.loadSessionMetadata()
	if loadErr != nil {
		if errors.Is(loadErr, m31errors.ErrSessionCorrupted) || errors.Is(loadErr, m31errors.ErrSessionNotFound) {
			return "", types.PhaseIdle, nil, nil
		}
		return "", types.PhaseIdle, nil, loadErr
	}
	g, p, q := session.WorkflowState()
	return g, p, q, nil
}

// sessionMetadata is a metadata-only view of Session for JSON serialization.
// It excludes Messages and Tasks to keep session.json lightweight.
type sessionMetadata struct {
	SchemaVersion    int                 `json:"schema_version"`
	ID               string              `json:"id"`
	ChildrenIDs      []string            `json:"children_ids"`
	Model            string              `json:"model"`
	Provider         string              `json:"provider"`
	StartedAt        time.Time           `json:"started_at"`
	MessageCount     int                 `json:"message_count"`
	WorkflowPhase    types.WorkflowPhase `json:"workflow_phase"`
	Project          *types.ProjectState `json:"project,omitempty"`
	ResumedAt        *time.Time          `json:"resumed_at,omitempty"`
	WorkflowGoal     string              `json:"workflow_goal,omitempty"`
	DiscussQuestions []string            `json:"discuss_questions,omitempty"`
	Label            string              `json:"label,omitempty"`
}

// saveSessionAtomic writes session metadata to session.json atomically.
// Only metadata fields are persisted — Messages and Tasks are excluded.
func (m *Manager) saveSessionAtomic(session *Session) error {
	meta := sessionMetadata{
		SchemaVersion:    session.SchemaVersion,
		ID:               session.ID,
		ChildrenIDs:      session.ChildrenIDs,
		Model:            session.Model,
		Provider:         session.Provider,
		StartedAt:        session.StartedAt,
		MessageCount:     session.MessageCount,
		WorkflowPhase:    session.WorkflowPhase,
		Project:          session.Project,
		ResumedAt:        session.ResumedAt,
		WorkflowGoal:     session.WorkflowGoal,
		DiscussQuestions: session.DiscussQuestions,
		Label:            session.Label,
	}
	data, err := json.Marshal(meta)
	if err != nil {
		return fmt.Errorf("cannot marshal session: %w", err)
	}
	return m.atomicWrite(m.sessionJSONPath(), data)
}

// ListSessions returns the current project session if one exists.
// With project-local sessions, there is at most one session per project.
func (m *Manager) ListSessions() ([]SessionInfo, error) {
	sessPath := m.sessionJSONPath()
	if _, err := os.Stat(sessPath); os.IsNotExist(err) {
		return []SessionInfo{}, nil
	}

	session, err := m.loadSessionMetadata()
	if err != nil {
		fi, statErr := os.Stat(m.projectDir())
		info := SessionInfo{Corrupted: true}
		if statErr == nil {
			info.LastModified = fi.ModTime()
		}
		return []SessionInfo{info}, nil
	}

	fi, _ := os.Stat(sessPath)
	info := SessionInfo{
		ID:            session.ID,
		Model:         session.Model,
		Provider:      session.Provider,
		StartedAt:     session.StartedAt,
		MessageCount:  session.MessageCount,
		WorkflowPhase: session.WorkflowPhase,
		Label:         session.Label,
	}
	if fi != nil {
		info.LastModified = fi.ModTime()
	}

	return []SessionInfo{info}, nil
}

// DeleteSession removes the project-local session files without deleting
// the entire .m31a/ directory (which may contain backups, planning data, etc.).
func (m *Manager) DeleteSession(id string) error {
	if err := m.lock.Lock(); err != nil {
		return fmt.Errorf("acquire lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck
	dir := m.projectDir()
	for _, name := range []string{"session.json", "session.json.bak", "messages.json"} {
		path := filepath.Join(dir, name)
		if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("remove %s: %w", name, err)
		}
	}
	return nil
}

// SaveSession writes session.json and messages.json to <workDir>/.m31a/.
func (m *Manager) SaveSession(s *Session) error {
	if err := m.lock.Lock(); err != nil {
		return fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	if s.Messages == nil {
		s.Messages = make([]types.Message, 0)
	}
	msgData, err := json.Marshal(s.Messages)
	if err != nil {
		return fmt.Errorf("cannot marshal messages: %w", err)
	}

	data, err := json.Marshal(s)
	if err != nil {
		return fmt.Errorf("cannot marshal session: %w", err)
	}

	if err := m.atomicWrite(m.sessionJSONPath(), data); err != nil {
		return fmt.Errorf("cannot write session.json: %w", err)
	}
	if err := m.atomicWrite(m.messagesJSONPath(), msgData); err != nil {
		return fmt.Errorf("cannot write messages.json: %w", err)
	}

	return nil
}

// SaveMessages writes messages.json to <workDir>/.m31a/.
func (m *Manager) SaveMessages(sessionID string, messages []types.Message) error {
	if err := m.lock.Lock(); err != nil {
		return fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	if messages == nil {
		messages = make([]types.Message, 0)
	}
	data, err := json.Marshal(messages)
	if err != nil {
		return fmt.Errorf("cannot marshal messages: %w", err)
	}
	return m.atomicWrite(m.messagesJSONPath(), data)
}

// LoadMessages reads messages.json from <workDir>/.m31a/.
func (m *Manager) LoadMessages(sessionID string) ([]types.Message, error) {
	data, err := readFileLimited(m.messagesJSONPath(), types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return []types.Message{}, nil
		}
		return nil, fmt.Errorf("cannot read messages.json: %w", err)
	}
	var messages []types.Message
	if err := json.Unmarshal(data, &messages); err != nil {
		return nil, fmt.Errorf("cannot unmarshal messages.json: %w", err)
	}
	return messages, nil
}

// FilterSessions returns sessions matching a query string.
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

// RenameSession updates the session's label.
func (m *Manager) RenameSession(id, label string) error {
	if err := m.lock.Lock(); err != nil {
		return fmt.Errorf("session lock: %w", err)
	}
	defer m.lock.Unlock() //nolint:errcheck

	sess, err := m.loadSessionMetadata()
	if err != nil {
		return fmt.Errorf("load session: %w", err)
	}
	sess.Label = label
	return m.saveSessionAtomic(sess)
}

// ExportSessionMarkdown exports the session's messages as a markdown file.
func (m *Manager) ExportSessionMarkdown(id, path string) error {
	sess, err := m.LoadSession(id)
	if err != nil {
		return fmt.Errorf("load session: %w", err)
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "# Session: %s\n\n", sess.ID)
	fmt.Fprintf(&sb, "**Model:** %s  \n**Provider:** %s  \n**Started:** %s  \n**Messages:** %d\n\n---\n\n",
		sess.Model, sess.Provider, sess.StartedAt.Format(time.RFC3339), sess.MessageCount)
	for _, msg := range sess.Messages {
		fmt.Fprintf(&sb, "## %s\n\n%s\n\n---\n\n", msg.Role, msg.Content)
	}
	return m.atomicWrite(path, []byte(sb.String()))
}

// ExportSessionJSON exports the session's full data as a JSON file.
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

// Cleanup is a no-op for project-local sessions (don't auto-delete project data).
func (m *Manager) Cleanup(maxAge time.Duration) (int, error) {
	return 0, nil
}

// ensureGitIgnore adds .m31a/ to the project's .gitignore if not already present.
func (m *Manager) ensureGitIgnore() {
	gitignorePath := filepath.Join(m.workDir, ".gitignore")

	existing, err := os.ReadFile(gitignorePath)
	if err != nil {
		if os.IsNotExist(err) {
			if writeErr := os.WriteFile(gitignorePath, []byte("# M31A session data\n.m31a/\n"), types.FilePermission); writeErr != nil {
				slog.Warn("failed to create .gitignore", "error", writeErr)
			}
		}
		return
	}

	if strings.Contains(string(existing), ".m31a/") {
		return
	}

	f, err := os.OpenFile(gitignorePath, os.O_APPEND|os.O_WRONLY, types.FilePermission)
	if err != nil {
		slog.Warn("failed to update .gitignore", "error", err)
		return
	}
	defer func() {
		if closeErr := f.Close(); closeErr != nil {
			slog.Debug("close file", "error", closeErr, "path", gitignorePath)
		}
	}()

	content := string(existing)
	if !strings.HasSuffix(content, "\n") {
		if _, err := f.WriteString("\n"); err != nil {
			slog.Warn("failed to update .gitignore", "error", err)
			return
		}
	}
	if _, err := f.WriteString("# M31A session data\n.m31a/\n"); err != nil {
		slog.Warn("failed to update .gitignore", "error", err)
	}
}

// ── Recent Models (global, stored in ~/.m31a/) ─────────────────────────────

// RecentModelsData stores the recent model list and favorites.
type RecentModelsData struct {
	Recent    []string        `json:"recent"`
	Favorites map[string]bool `json:"favorites"`
}

func (m *Manager) recentModelsPath() string {
	return filepath.Join(m.baseDir, "recent_models.json")
}

// LoadRecentModels reads the recent models file from ~/.m31a/.
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

// SaveRecentModels writes the recent models data to ~/.m31a/.
func (m *Manager) SaveRecentModels(data *RecentModelsData) error {
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
	if err := m.ensureDir(m.baseDir); err != nil {
		return fmt.Errorf("cannot create config directory: %w", err)
	}
	return m.atomicWrite(m.recentModelsPath(), payload)
}

// AddRecentModel adds a model ID to the top of the recent list.
func (m *Manager) AddRecentModel(modelID string) error {
	data, err := m.LoadRecentModels()
	if err != nil {
		return err
	}

	var updated []string
	for _, id := range data.Recent {
		if id != modelID {
			updated = append(updated, id)
		}
	}
	data.Recent = append([]string{modelID}, updated...)

	if len(data.Recent) > m.maxRecentModels {
		data.Recent = data.Recent[:m.maxRecentModels]
	}

	return m.SaveRecentModels(data)
}

// ToggleFavorite toggles the favorite status for a model ID.
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
