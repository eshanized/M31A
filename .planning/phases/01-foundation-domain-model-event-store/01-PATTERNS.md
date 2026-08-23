# Phase 01: Foundation — Domain Model & Event Store - Pattern Map

**Mapped:** 2026-08-23
**Files analyzed:** 20 new files
**Analogs found:** 18 / 20

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `internal/core/types/domain.go` | model | CRUD | `internal/core/types/types.go` | exact |
| `internal/core/types/planning.go` | model | CRUD | `internal/core/types/types.go` | exact |
| `internal/core/types/execution.go` | model | CRUD | `internal/core/types/types.go` | exact |
| `internal/core/types/assurance.go` | model | CRUD | `internal/core/types/types.go` | exact |
| `internal/core/types/event.go` | model | CRUD + event-driven | `internal/core/types/types.go` | role-match |
| `internal/core/types/serialization.go` | utility | transform | `internal/core/types/types.go` | role-match |
| `internal/core/types/eventstore.go` | service (interface) | request-response | `internal/integrations/keychain/keychain.go` | exact |
| `internal/core/config/config.go` | config | request-response | `internal/core/config/types.go` | exact |
| `internal/core/config/loader.go` | service | request-response | `internal/core/config/loader.go` | exact (modify) |
| `internal/core/config/validation.go` | utility | transform | `internal/core/config/loader.go` (validation section) | role-match |
| `internal/memory/eventstore/eventstore.go` | service | CRUD + streaming | `internal/integrations/keychain/keychain.go` (interface) + `internal/core/config/loader.go` (init) | role-match |
| `internal/memory/eventstore/schema.sql` | config (DDL) | file-I/O | — | no analog |
| `internal/memory/eventstore/append.go` | service | CRUD | `internal/core/config/loader.go` (transaction pattern) | role-match |
| `internal/memory/eventstore/query.go` | service | request-response | `internal/integrations/git/git.go` (query methods) | role-match |
| `internal/memory/eventstore/projection.go` | service | event-driven | `internal/integrations/keychain/keychain.go` (subscription pattern) | role-match |
| `internal/memory/eventstore/backup.go` | service | file-I/O | `internal/integrations/git/git.go` (file operations) | role-match |
| `internal/memory/eventstore/migration.go` | service | batch + event-driven | `internal/core/config/loader.go` (layered loading) | role-match |
| `internal/memory/artifacts/project_md.go` | service | file-I/O | `internal/integrations/git/git.go` | exact |
| `internal/memory/artifacts/decisions.go` | service | file-I/O | `internal/integrations/git/git.go` | exact |
| `internal/memory/artifacts/research.go` | service | file-I/O | `internal/integrations/git/git.go` | exact |

---

## Pattern Assignments

### `internal/core/types/domain.go` (model, CRUD)

**Analog:** `internal/core/types/types.go`

**Imports pattern** (lines 1-7):
```go
package types

import (
	"encoding/json"
	"time"
	"github.com/google/uuid"
)
```

**Core type pattern with JSON tags** (lines 9-16, 140-151):
```go
type RiskLevel string

const (
	RiskSafe        RiskLevel = "safe"
	RiskMedium      RiskLevel = "medium"
	RiskDangerous   RiskLevel = "dangerous"
	RiskDestructive RiskLevel = "destructive"
)

type ModelInfo struct {
	ID            string   `json:"id"`
	Provider      string   `json:"provider"`
	Name          string   `json:"name"`
	Description   string   `json:"description"`
	ContextLength int64    `json:"context_length"`
	Pricing       Pricing  `json:"pricing"`
	Architecture  ArchInfo `json:"architecture"`
	TopProvider   string   `json:"top_provider"`
	Capabilities  CapFlags `json:"capabilities"`
	Variant       *string  `json:"variant,omitempty"`
}
```

**Custom JSON marshaling for nil-slice safety** (lines 181-189):
```go
func (m Message) MarshalJSON() ([]byte, error) {
	if m.Segments == nil {
		m.Segments = []MessageSegment{}
	}
	type msgAlias Message
	return json.Marshal(msgAlias(m))
}
```

**Error types with hint for LLM recovery** (lines 204-226):
```go
type ToolError struct {
	Err  error
	Hint string
}

func (e *ToolError) Error() string {
	if e.Hint != "" {
		return e.Err.Error() + "\nHint: " + e.Hint
	}
	return e.Err.Error()
}

func (e *ToolError) Unwrap() error {
	return e.Err
}

func NewToolError(err error, hint string) *ToolError {
	return &ToolError{Err: err, Hint: hint}
}
```

---

### `internal/core/types/planning.go` (model, CRUD)

**Analog:** `internal/core/types/types.go`

**Core type pattern** (lines 256-290):
```go
type Task struct {
	ID                 int        `json:"id"`
	Description        string     `json:"description"`
	Action             string     `json:"action"`
	Category           string     `json:"category,omitempty"`
	PlanSection        string     `json:"plan_section,omitempty"`
	Dependencies       []int      `json:"dependencies"`
	Files              []string   `json:"files"`
	AcceptanceCriteria []string   `json:"acceptance_criteria"`
	Status             TaskStatus `json:"status"`
	HealsAttempted     int        `json:"heals_attempted"`
	CommitHash         string     `json:"commit_hash,omitempty"`
}

type ProjectState struct {
	Goal        string            `json:"goal"`
	ProjectType string            `json:"project_type"`
	Framework   string            `json:"framework"`
	Answers     map[string]string `json:"answers"`
	CreatedAt   time.Time         `json:"created_at"`
}

type Session struct {
	ID            string        `json:"id"`
	ParentID      string        `json:"parent_id,omitempty"`
	ChildrenIDs   []string      `json:"children_ids,omitempty"`
	Label         string        `json:"label,omitempty"`
	Tags          []string      `json:"tags,omitempty"`
	Model         string        `json:"model"`
	Provider      string        `json:"provider"`
	StartedAt     time.Time     `json:"started_at"`
	MessageCount  int           `json:"message_count"`
	WorkflowPhase WorkflowPhase `json:"workflow_phase"`
	Project       *ProjectState `json:"project,omitempty"`
}
```

**Constants with string values for JSON serialization** (lines 18-29, 107-116):
```go
type WorkflowPhase string

const (
	PhaseIdle       WorkflowPhase = "idle"
	PhaseInitialize WorkflowPhase = "initialize"
	PhaseDiscuss    WorkflowPhase = "discuss"
	PhasePlan       WorkflowPhase = "plan"
	PhaseExecute    WorkflowPhase = "execute"
	PhaseVerify     WorkflowPhase = "verify"
	PhaseRuntime    WorkflowPhase = "runtime"
	PhaseShip       WorkflowPhase = "ship"
)

type TaskStatus string

const (
	StatusPending       TaskStatus = "pending"
	StatusRunning       TaskStatus = "running"
	StatusDone          TaskStatus = "done"
	StatusFailed        TaskStatus = "failed"
	StatusSkipped       TaskStatus = "skipped"
	StatusUnrecoverable TaskStatus = "unrecoverable"
)
```

---

### `internal/core/types/execution.go` (model, CRUD)

**Analog:** `internal/core/types/types.go`

**Interface pattern for tools** (lines 241-254):
```go
type Tool interface {
	Name() string
	Description() string
	RiskLevel() RiskLevel
	Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}

type SchemaProvider interface {
	ParameterSchema() string
}
```

**Input/output types with JSON tags** (lines 191-202):
```go
type ToolInput struct {
	Name   string         `json:"name"`
	Params map[string]any `json:"params"`
}

type ToolResult struct {
	ToolCallID string `json:"tool_call_id"`
	Output     string `json:"output"`
	Error      string `json:"error,omitempty"`
	DurationMs int64  `json:"duration_ms"`
	Truncated  bool   `json:"truncated"`
}
```

**HealReport for self-healing tracking** (lines 228-239):
```go
type HealReport struct {
	TaskID     int       `json:"task_id"`
	Attempt    int       `json:"attempt"`
	Success    bool      `json:"success"`
	ErrorType  string    `json:"error_type"`
	ErrorMsg   string    `json:"error_msg"`
	Strategy   string    `json:"strategy"`
	FilesUsed  []string  `json:"files_used,omitempty"`
	DurationMs int64     `json:"duration_ms"`
	Timestamp  time.Time `json:"timestamp"`
}
```

---

### `internal/core/types/assurance.go` (model, CRUD)

**Analog:** `internal/core/types/types.go`

**Structured types with time.Time and uuid.UUID** (use same patterns as `Session`, `ProjectState`, `ModelInfo`):
```go
// Follows same patterns as ProjectState, Session, ModelInfo
// Use uuid.UUID for IDs, time.Time for timestamps
// JSON tags with omitempty for optional fields
```

---

### `internal/core/types/event.go` (model, CRUD + event-driven)

**Analog:** `internal/core/types/types.go` + RESEARCH.md examples

**Event envelope with monotonic seq** (from RESEARCH.md lines 155-170):
```go
type Event struct {
	ID          uuid.UUID       `json:"id"`
	Seq         int64           `json:"seq"`
	Type        EventType       `json:"type"`
	Timestamp   time.Time       `json:"timestamp"`
	RunID       *uuid.UUID      `json:"run_id,omitempty"`
	SessionID   *uuid.UUID      `json:"session_id,omitempty"`
	Payload     json.RawMessage `json:"payload"`
	Metadata    EventMetadata   `json:"metadata,omitempty"`
}

type EventType string

const (
	EventProjectInitialized    EventType = "ProjectInitialized"
	EventRepositoryIndexed     EventType = "RepositoryIndexed"
	EventSessionCreated        EventType = "SessionCreated"
	EventRunCreated            EventType = "RunCreated"
	EventIntentAccepted        EventType = "IntentAccepted"
	EventRequirementCreated    EventType = "RequirementCreated"
	EventDecisionLogged        EventType = "DecisionLogged"
	EventPlanCreated           EventType = "PlanCreated"
	EventTaskCreated           EventType = "TaskCreated"
	EventAgentStarted          EventType = "AgentStarted"
	EventToolCallRequested     EventType = "ToolCallRequested"
	EventFileChanged           EventType = "FileChanged"
	EventCheckpointRequested   EventType = "CheckpointRequested"
	EventVerificationStarted   EventType = "VerificationStarted"
	EventTaskCompleted         EventType = "TaskCompleted"
	EventRunCompleted          EventType = "RunCompleted"
	// ... 35+ types per CONTEXT_M31A.md §11
)

type EventMetadata struct {
	SchemaVersion int    `json:"schema_version"`
	Tags          []string `json:"tags,omitempty"`
	Source        string `json:"source,omitempty"`
}
```

**EventStore interface** (from RESEARCH.md lines 165-170):
```go
type EventStore interface {
	Append(ctx context.Context, events ...Event) error
	Query(ctx context.Context, q Query) ([]Event, error)
	Subscribe(ctx context.Context, afterSeq int64) (<-chan Event, error)
	Backup(ctx context.Context, dstPath string) error
}
```

---

### `internal/core/types/serialization.go` (utility, transform)

**Analog:** `internal/core/types/types.go` (lines 433-442)

**JSON marshaling helpers** (lines 433-442):
```go
func MarshalEventPayload(v any) (json.RawMessage, error) {
	return json.Marshal(v)
}

func UnmarshalEventPayload[T any](data json.RawMessage) (T, error) {
	var v T
	err := json.Unmarshal(data, &v)
	return v, err
}
```

---

### `internal/core/types/eventstore.go` (service interface, request-response)

**Analog:** `internal/integrations/keychain/keychain.go` (lines 18-35)

**Interface definition with error types** (lines 18-35):
```go
// Keychain provides OS-native secure storage for API keys.
// Each platform implements this interface using the native secret storage mechanism.
type Keychain interface {
	// Get retrieves the value for the given service name.
	// Returns ErrKeyNotFound if the key does not exist.
	// Returns ErrKeychainUnavailable if the keychain backend is unavailable.
	Get(service string) (string, error)

	// Set stores a value for the given service name.
	// If a value already exists for this service, it is overwritten.
	// Returns ErrKeychainUnavailable if the keychain backend is unavailable.
	Set(service, value string) error

	// Delete removes the value for the given service name.
	// Returns ErrKeyNotFound if the key does not exist.
	// Returns ErrKeychainUnavailable if the keychain backend is unavailable.
	Delete(service string) error
}

// Error sentinels for interface consumers
var (
	ErrKeyNotFound         = errors.New("key not found")
	ErrKeychainUnavailable = errors.New("keychain unavailable")
	ErrNotImplemented      = errors.New("not implemented")
)
```

**Cached wrapper pattern** (lines 48-80):
```go
type cachedKeychain struct {
	inner            Keychain
	unavailableSince atomic.Int64 // Unix nanoseconds when blacklisted; 0 = available
}

func (c *cachedKeychain) isBlacklisted() bool {
	ts := c.unavailableSince.Load()
	if ts == 0 {
		return false
	}
	return time.Since(time.Unix(0, ts)) < blacklistTTL
}

func (c *cachedKeychain) Get(service string) (string, error) {
	if c.isBlacklisted() {
		return "", ErrKeychainUnavailable
	}
	val, err := c.inner.Get(service)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.blacklist()
		return val, err
	}
	c.clearBlacklist()
	return val, err
}

func NewCached(inner Keychain) Keychain {
	if inner == nil {
		return nil
	}
	return &cachedKeychain{inner: inner}
}
```

---

### `internal/core/config/config.go` (config, request-response)

**Analog:** `internal/core/config/types.go` (lines 13-32)

**Config struct with TOML tags and nested config sections** (lines 13-32):
```go
type Config struct {
	Provider          ProviderConfig          `toml:"provider"`
	Model             ModelConfig             `toml:"model"`
	UI                UIConfig                `toml:"ui"`
	Permissions       PermissionsConfig       `toml:"permissions"`
	Features          FeaturesConfig          `toml:"features"`
	Ledger            LedgerConfig            `toml:"ledger"`
	Tools             ToolsConfig             `toml:"tools"`
	Agents            AgentsConfig            `toml:"agents"`
	Git               GitConfig               `toml:"git"`
	Verify            VerifyConfig            `toml:"verify"`
	Compaction        CompactionConfig        `toml:"compaction"`
	Instructions      InstructionsConfig      `toml:"instructions"`
	Skills            SkillsConfig            `toml:"skills"`
	ModelCapabilities ModelCapabilitiesConfig `toml:"model_capabilities"`
	Prompts           PromptConfig            `toml:"prompts"`
	Narrative         NarrativeConfig         `toml:"narrative"`
	Templates         TemplateConfig          `toml:"templates"`
	Extensions        ExtensionsConfig        `toml:"extensions" json:"extensions"`
}
```

**Provider credential config with keychain integration** (lines 197-229):
```go
type ProviderConfig struct {
	Default      string                   `toml:"default"`
	AutoFallback bool                     `toml:"auto_fallback"`
	OpenRouter   ProviderCredentialConfig `toml:"openrouter"`
	Zen          ProviderCredentialConfig `toml:"zen"`
	Nvidia       ProviderCredentialConfig `toml:"nvidia"`
	OpenRouterBaseURL string `toml:"openrouter_base_url"`
	ZenBaseURL        string `toml:"zen_base_url"`
	NvidiaBaseURL     string `toml:"nvidia_base_url"`
	FallbackPriority []string `toml:"fallback_priority"`
	HealthCheckTimeoutSecs int `toml:"health_check_timeout_secs"`
	RegistrationOrder []string `toml:"registration_order"`
}

type ProviderCredentialConfig struct {
	APIKey string `toml:"api_key"`
}
```

**Execution/feature flags pattern** (lines 414-506):
```go
type FeaturesConfig struct {
	WorkflowMode string `toml:"workflow_mode"` // "auto", "full", "fast", "direct"
	MaxParallelTasks int `toml:"max_parallel_tasks"`
	PermissionMode string `toml:"permission_mode"` // "interactive", "deny", "allow", "ci"
	// ... boolean feature flags
	PlanResearch       bool `toml:"plan_research"`
	PlanCheck          bool `toml:"plan_check"`
	ExecutePreflight   bool `toml:"execute_preflight"`
	// ...
}
```

---

### `internal/core/config/loader.go` (service, request-response)

**Analog:** `internal/core/config/loader.go` (existing file - will be extended)

**Layered config loading with koanf** (lines 215-230, 238-262):
```go
func Load(path string) (*Config, error) {
	// Step 1: M31A_CONFIG env var overrides path argument
	if envPath := os.Getenv("M31A_CONFIG"); envPath != "" {
		path = envPath
	}

	// Step 2: Layer 1 — Defaults (zero-valued Config)
	cfg := DefaultConfig()

	// Step 3: Layer 2 — Global config (~/.m31a/config.toml)
	if path != "" {
		meta, err := toml.DecodeFile(path, cfg)
		if err != nil {
			if !os.IsNotExist(err) {
				return nil, fmt.Errorf("decode global config %s: %w", path, err)
			}
		} else {
			knownKeys := knownConfigKeys()
			for _, key := range meta.Keys() {
				k := key.String()
				topLevel := k
				if dotIdx := strings.IndexByte(k, '.'); dotIdx >= 0 {
					topLevel = k[:dotIdx]
				}
				if !knownKeys[topLevel] {
					slog.Warn("unknown config key, check for typos", "key", k, "file", path)
				}
			}
		}
	}

	// Step 4: Auto-load .env file
	LoadDotEnv()

	// Step 5: Environment variable overrides (highest precedence)
	if theme := os.Getenv("M31A_THEME"); theme != "" {
		cfg.UI.Theme = theme
	}
	// ... more env overrides

	// Step 6: Variable substitution
	unresolvedVars := applyVarSubstitution(cfg)

	// Step 7: Validation
	if err := validateConfig(cfg); err != nil {
		return nil, fmt.Errorf("config validation: %w", err)
	}

	return cfg, nil
}
```

**Atomic write with keychain integration** (lines 469-591):
```go
func (c *Config) SaveWithKeychain(path string, kc keychain.Keychain) error {
	cfgCopy := *c
	// Deep-copy slice and map fields to prevent data races
	if c.Permissions.Rules != nil {
		rulesCopy := make([]PermissionRule, len(c.Permissions.Rules))
		copy(rulesCopy, c.Permissions.Rules)
		cfgCopy.Permissions.Rules = rulesCopy
	}
	// ... similar for other slices/maps

	persistKeys := true
	if kc != nil {
		// Try to save keys to keychain
		if openRouterKey != "" {
			if err := kc.Set(types.ProviderOpenRouter, openRouterKey); err == nil {
				openRouterSaved = true
			}
		}
		// ... similar for zen, nvidia
		if openRouterSaved && zenSaved && nvidiaSaved {
			persistKeys = false
		}
	}

	if persistKeys {
		slog.Info("API keys will be stored in config file (keychain unavailable)")
	}

	data, err := toml.Marshal(&cfgCopy)
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}

	if err := os.MkdirAll(filepath.Dir(path), types.DirPermission); err != nil {
		return fmt.Errorf("create config dir: %w", err)
	}

	if err := fileutil.AtomicWrite(path, data); err != nil {
		return fmt.Errorf("write config: %w", err)
	}
	return nil
}
```

**API key resolution priority** (lines 604-648):
```go
func (c *Config) ResolveAPIKeys(kc keychain.Keychain) error {
	// Priority: env var → OS keychain → config file field
	if key := os.Getenv("M31A_OPENROUTER_API_KEY"); key != "" {
		c.Provider.OpenRouter.APIKey = key
	} else if key := os.Getenv("OPENROUTER_API_KEY"); key != "" {
		c.Provider.OpenRouter.APIKey = key
	} else if kc != nil {
		if k, err := kc.Get(types.ProviderOpenRouter); err == nil {
			c.Provider.OpenRouter.APIKey = k
		} else if !errors.Is(err, keychain.ErrKeyNotFound) && !errors.Is(err, keychain.ErrKeychainUnavailable) {
			slog.Warn("keychain error", "provider", types.ProviderOpenRouter, "error", err)
		}
	}
	// ... repeat for Zen, Nvidia
	return nil
}
```

**File watcher with debounce** (lines 678-734):
```go
func WatchConfig(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		slog.Warn("fsnotify unavailable, falling back to config polling", "error", err)
		watchConfigPolling(ctx, path, ch)
		return
	}
	defer watcher.Close()

	dir := filepath.Dir(path)
	base := filepath.Base(path)
	if err := watcher.Add(dir); err != nil {
		watchConfigPolling(ctx, path, ch)
		return
	}

	var debounce *time.Timer
	for {
		select {
		case <-ctx.Done():
			return
		case event, ok := <-watcher.Events:
			if !ok || filepath.Base(event.Name) != base {
				continue
			}
			if event.Op&(fsnotify.Write|fsnotify.Create|fsnotify.Rename) == 0 {
				continue
			}
			if debounce != nil {
				debounce.Stop()
			}
			debounce = time.AfterFunc(50*time.Millisecond, func() {
				sendReload(ctx, ch, path)
			})
		}
	}
}
```

---

### `internal/core/config/validation.go` (utility, transform)

**Analog:** `internal/core/config/loader.go` validation section (lines 327-340) + `internal/core/config/config_validate.go`

**Validation pattern with structured errors** (from `internal/core/errors/errors.go` lines 106-122):
```go
type ConfigError struct {
	Key string
	Err error
}

func (e *ConfigError) Error() string {
	if e.Key != "" {
		return "config " + e.Key + ": " + e.Err.Error()
	}
	return "config: " + e.Err.Error()
}

func (e *ConfigError) Unwrap() error { return e.Err }
```

**Validation function signature** (from loader.go line 328):
```go
func validateConfig(cfg *Config) error {
	// Use errors.Wrap for consistent wrapping
	if cfg.Provider.Default == "" {
		return errors.Wrap(ErrValidation, "provider.default is required")
	}
	// Validate enum fields
	validModes := map[string]bool{"auto": true, "full": true, "fast": true, "direct": true}
	if !validModes[cfg.Features.WorkflowMode] {
		return errors.Wrapf(ErrValidation, "invalid workflow_mode: %s", cfg.Features.WorkflowMode)
	}
	// Range checks
	if cfg.Features.MaxParallelTasks <= 0 || cfg.Features.MaxParallelTasks > 32 {
		return errors.Wrap(ErrValidation, "max_parallel_tasks must be 1-32")
	}
	return nil
}
```

---

### `internal/memory/eventstore/eventstore.go` (service, CRUD + streaming)

**Analog:** `internal/integrations/keychain/keychain.go` (interface) + `internal/core/config/loader.go` (initialization)

**SQLiteEventStore struct with sql.DB** (from RESEARCH.md lines 501-516):
```go
type SQLiteEventStore struct {
	db *sql.DB
}

func NewEventStore(dbPath string) (*SQLiteEventStore, error) {
	dsn := "file:" + dbPath + "?_pragma=journal_mode(WAL)&_pragma=foreign_keys(1)&_pragma=busy_timeout(5000)&_time_format=sqlite"
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, err
	}
	if _, err := db.Exec(schemaSQL); err != nil {
		return nil, err
	}
	return &SQLiteEventStore{db: db}, nil
}
```

**Interface implementation** (from RESEARCH.md lines 165-170):
```go
func (s *SQLiteEventStore) Append(ctx context.Context, events ...types.Event) error {
	if len(events) == 0 {
		return nil
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	stmt, err := tx.PrepareContext(ctx, `
		INSERT INTO events (id, type, timestamp, run_id, session_id, payload, metadata)
		VALUES (?, ?, ?, ?, ?, ?, ?)
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for i := range events {
		events[i].ID = uuid.New()
		events[i].Timestamp = time.Now()

		payload, _ := json.Marshal(events[i].Payload)
		metadata, _ := json.Marshal(events[i].Metadata)

		runID := ""
		if events[i].RunID != nil {
			runID = events[i].RunID.String()
		}
		sessionID := ""
		if events[i].SessionID != nil {
			sessionID = events[i].SessionID.String()
		}

		if _, err := stmt.ExecContext(ctx,
			events[i].ID.String(),
			events[i].Type,
			events[i].Timestamp.UnixNano(),
			nullIfEmpty(runID),
			nullIfEmpty(sessionID),
			payload,
			nullIfEmpty(string(metadata)),
		); err != nil {
			return err
		}
	}

	return tx.Commit()
}

func nullIfEmpty(s string) any {
	if s == "" {
		return nil
	}
	return s
}
```

**Subscription pattern for TUI** (from RESEARCH.md pattern):
```go
func (s *SQLiteEventStore) Subscribe(ctx context.Context, afterSeq int64) (<-chan types.Event, error) {
	ch := make(chan types.Event, 100)
	go func() {
		defer close(ch)
		ticker := time.NewTicker(100 * time.Millisecond)
		defer ticker.Stop()
		var lastSeq = afterSeq
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				events, err := s.Query(ctx, Query{AfterSeq: lastSeq, Limit: 100})
				if err != nil {
					slog.Error("subscription query failed", "error", err)
					continue
				}
				for _, evt := range events {
					select {
					case ch <- evt:
						lastSeq = evt.Seq
					case <-ctx.Done():
						return
					}
				}
			}
		}
	}()
	return ch, nil
}
```

---

### `internal/memory/eventstore/schema.sql` (config/DDL, file-I/O)

**No analog** — new file. Based on RESEARCH.md lines 445-484:

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS events (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    id          TEXT NOT NULL UNIQUE,
    type        TEXT NOT NULL,
    timestamp   INTEGER NOT NULL,
    run_id      TEXT,
    session_id  TEXT,
    payload     BLOB NOT NULL,
    metadata    BLOB
);

CREATE INDEX IF NOT EXISTS idx_events_run_seq      ON events(run_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_session_seq  ON events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_type_seq     ON events(type, seq);
CREATE INDEX IF NOT EXISTS idx_events_timestamp    ON events(timestamp);

CREATE TABLE IF NOT EXISTS projections (
    name        TEXT PRIMARY KEY,
    last_seq    INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  INTEGER NOT NULL
);
```

---

### `internal/memory/eventstore/append.go` (service, CRUD)

**Analog:** `internal/core/config/loader.go` transaction pattern + RESEARCH.md lines 518-577

**Batch append with transaction** (from RESEARCH.md lines 518-569):
```go
func (s *SQLiteEventStore) AppendEvents(ctx context.Context, events []types.Event) error {
	if len(events) == 0 {
		return nil
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return errors.Wrap(err, "begin transaction")
	}
	defer tx.Rollback()

	stmt, err := tx.PrepareContext(ctx, `
		INSERT INTO events (id, type, timestamp, run_id, session_id, payload, metadata)
		VALUES (?, ?, ?, ?, ?, ?, ?)
	`)
	if err != nil {
		return errors.Wrap(err, "prepare statement")
	}
	defer stmt.Close()

	for i := range events {
		events[i].ID = uuid.New()
		events[i].Timestamp = time.Now().UTC()

		payload, err := json.Marshal(events[i].Payload)
		if err != nil {
			return errors.Wrapf(err, "marshal payload for event %d", i)
		}
		metadata, err := json.Marshal(events[i].Metadata)
		if err != nil {
			return errors.Wrapf(err, "marshal metadata for event %d", i)
		}

		runID := nullString(events[i].RunID)
		sessionID := nullString(events[i].SessionID)

		if _, err := stmt.ExecContext(ctx,
			events[i].ID.String(),
			string(events[i].Type),
			events[i].Timestamp.UnixNano(),
			runID,
			sessionID,
			payload,
			nullString(string(metadata)),
		); err != nil {
			return errors.Wrapf(err, "exec event %d", i)
		}
	}

	if err := tx.Commit(); err != nil {
		return errors.Wrap(err, "commit transaction")
	}
	return nil
}

func nullString(id *uuid.UUID) any {
	if id == nil {
		return nil
	}
	return id.String()
}
```

---

### `internal/memory/eventstore/query.go` (service, request-response)

**Analog:** `internal/integrations/git/git.go` query methods (lines 192-286)

**Query with filtering and pagination** (from `git.go` Log/LogSince pattern):
```go
type Query struct {
	RunID      *uuid.UUID
	SessionID  *uuid.UUID
	Type       *EventType
	AfterSeq   int64
	BeforeSeq  int64
	Limit      int
	Offset     int
}

func (s *SQLiteEventStore) Query(ctx context.Context, q Query) ([]types.Event, error) {
	query := "SELECT seq, id, type, timestamp, run_id, session_id, payload, metadata FROM events WHERE 1=1"
	args := []any{}

	if q.RunID != nil {
		query += " AND run_id = ?"
		args = append(args, q.RunID.String())
	}
	if q.SessionID != nil {
		query += " AND session_id = ?"
		args = append(args, q.SessionID.String())
	}
	if q.Type != nil {
		query += " AND type = ?"
		args = append(args, string(*q.Type))
	}
	if q.AfterSeq > 0 {
		query += " AND seq > ?"
		args = append(args, q.AfterSeq)
	}
	if q.BeforeSeq > 0 {
		query += " AND seq < ?"
		args = append(args, q.BeforeSeq)
	}
	query += " ORDER BY seq ASC"
	if q.Limit > 0 {
		query += " LIMIT ?"
		args = append(args, q.Limit)
	}
	if q.Offset > 0 {
		query += " OFFSET ?"
		args = append(args, q.Offset)
	}

	rows, err := s.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, errors.Wrap(err, "query events")
	}
	defer rows.Close()

	var events []types.Event
	for rows.Next() {
		var evt types.Event
		var runID, sessionID sql.NullString
		var payload, metadata []byte
		if err := rows.Scan(&evt.Seq, &evt.ID, &evt.Type, &evt.Timestamp, &runID, &sessionID, &payload, &metadata); err != nil {
			return nil, errors.Wrap(err, "scan event")
		}
		if runID.Valid {
			parsed, _ := uuid.Parse(runID.String)
			evt.RunID = &parsed
		}
		if sessionID.Valid {
			parsed, _ := uuid.Parse(sessionID.String)
			evt.SessionID = &parsed
		}
		evt.Payload = payload
		evt.Metadata = metadata
		events = append(events, evt)
	}
	return events, rows.Err()
}
```

---

### `internal/memory/eventstore/projection.go` (service, event-driven)

**Analog:** `internal/integrations/keychain/keychain.go` cached wrapper pattern + RESEARCH.md pattern

**Projection interface and checkpoint pattern** (from RESEARCH.md lines 146-150, 471-476):
```go
type Projection interface {
	Name() string
	Apply(event types.Event) error
	State() any
	Checkpoint() ([]byte, error)
	Restore(data []byte) error
}

type ProjectionManager struct {
	store      *SQLiteEventStore
	projections map[string]Projection
	mu         sync.RWMutex
}

func (pm *ProjectionManager) Register(p Projection) {
	pm.mu.Lock()
	defer pm.mu.Unlock()
	pm.projections[p.Name()] = p
}

func (pm *ProjectionManager) Rebuild(ctx context.Context, name string) error {
	pm.mu.RLock()
	p, ok := pm.projections[name]
	pm.mu.RUnlock()
	if !ok {
		return errors.New("projection not found: " + name)
	}

	// Load last checkpoint
	var lastSeq int64
	err := pm.store.db.QueryRowContext(ctx,
		"SELECT last_seq FROM projections WHERE name = ?", name).Scan(&lastSeq)
	if err != nil && err != sql.ErrNoRows {
		return errors.Wrap(err, "load checkpoint")
	}

	// Replay events from last_seq
	events, err := pm.store.Query(ctx, Query{AfterSeq: lastSeq, Limit: 1000})
	if err != nil {
		return errors.Wrap(err, "query events for replay")
	}

	for _, evt := range events {
		if err := p.Apply(evt); err != nil {
			return errors.Wrapf(err, "apply event seq=%d", evt.Seq)
		}
		lastSeq = evt.Seq
	}

	// Save checkpoint in same transaction as event append (per D-15)
	checkpointData, err := p.Checkpoint()
	if err != nil {
		return errors.Wrap(err, "create checkpoint")
	}
	_, err = pm.store.db.ExecContext(ctx, `
		INSERT INTO projections (name, last_seq, updated_at)
		VALUES (?, ?, ?)
		ON CONFLICT(name) DO UPDATE SET last_seq = ?, updated_at = ?`,
		name, lastSeq, time.Now().UnixNano(), lastSeq, time.Now().UnixNano())
	return err
}
```

---

### `internal/memory/eventstore/backup.go` (service, file-I/O)

**Analog:** `internal/integrations/git/git.go` file operations + RESEARCH.md lines 579-639

**Hot backup with incremental Step** (from RESEARCH.md lines 592-638):
```go
func (s *SQLiteEventStore) Backup(ctx context.Context, dstPath string) error {
	conn, err := s.db.Conn(ctx)
	if err != nil {
		return errors.Wrap(err, "get connection")
	}
	defer conn.Close()

	return conn.Raw(func(driverConn any) error {
		type backuper interface {
			NewBackup(string) (interface {
				Step(int32) (bool, error)
				Finish() error
			}, error)
		}

		bc, ok := driverConn.(backuper)
		if !ok {
			return errors.New("driver does not support backup")
		}

		bk, err := bc.NewBackup(dstPath)
		if err != nil {
			return errors.Wrap(err, "create backup")
		}

		ticker := time.NewTicker(10 * time.Millisecond)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				bk.Finish()
				return ctx.Err()
			case <-ticker.C:
				more, err := bk.Step(5)
				if err != nil {
					bk.Finish()
					return errors.Wrap(err, "backup step")
				}
				if !more {
					return bk.Finish()
				}
			}
		}
	})
}
```

---

### `internal/memory/eventstore/migration.go` (service, batch + event-driven)

**Analog:** `internal/core/config/loader.go` layered loading pattern (lines 215-343)

**Migration as event replay** (from RESEARCH.md lines 232-265):
```go
func Migrate(ctx context.Context, planningDir, m31aDir string) error {
	// 1. Create .m31a/ directory structure
	dirs := []string{"decisions", "research", "codebase", "graphs"}
	for _, d := range dirs {
		if err := os.MkdirAll(filepath.Join(m31aDir, d), types.DirPermission); err != nil {
			return errors.Wrapf(err, "create dir %s", d)
		}
	}

	// 2. Initialize EventStore
	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	if err != nil {
		return errors.Wrap(err, "create event store")
	}
	defer store.Close()

	// 3. Emit MigrationStarted event
	migrationEvt := types.Event{
		Type:    types.EventMigrationStarted,
		Payload: mustMarshal(MigrationPayload{PlanningDir: planningDir, M31aDir: m31aDir}),
	}
	if err := store.Append(ctx, migrationEvt); err != nil {
		return errors.Wrap(err, "append migration start event")
	}

	// 4. For each artifact in .planning/, emit event + write projection
	// Example: requirements.md → RequirementCreated events
	reqs, err := parseRequirements(filepath.Join(planningDir, "REQUIREMENTS.md"))
	if err != nil {
		return errors.Wrap(err, "parse requirements")
	}
	for _, req := range reqs {
		evt := types.Event{
			Type:      types.EventRequirementCreated,
			Payload:   mustMarshal(req),
			SessionID: uuidPtr(uuid.MustParse("00000000-0000-0000-0000-000000000000")), // migration session
		}
		if err := store.Append(ctx, evt); err != nil {
			return errors.Wrapf(err, "append requirement %s", req.ID)
		}
	}
	writeProjection(filepath.Join(m31aDir, "requirements.md"), formatRequirements(reqs))

	// Repeat for: roadmap, decisions, research, state, context...
	// 5. Write config.toml with detected settings
	// 6. Write project.md with project identity
	// 7. Emit MigrationCompleted event
	completedEvt := types.Event{
		Type:    types.EventMigrationCompleted,
		Payload: mustMarshal(MigrationPayload{RequirementsCount: len(reqs), ...}),
	}
	return store.Append(ctx, completedEvt)
}
```

---

### `internal/memory/artifacts/project_md.go` (service, file-I/O)

**Analog:** `internal/integrations/git/git.go` file operations (lines 404-430, 694-702)

**Atomic file write with validation** (from `git.go` lines 404-430, `loader.go` lines 575-591):
```go
func WriteProjectMD(path string, project *types.Project) error {
	data, err := json.MarshalIndent(project, "", "  ")
	if err != nil {
		return errors.Wrap(err, "marshal project")
	}

	// Validate path is within workspace (prevent symlink escape)
	absPath, err := filepath.EvalSymlinks(path)
	if err != nil {
		return errors.Wrap(err, "resolve path")
	}
	// ... validate absPath is under workspace root

	// Atomic write: write to temp file in same directory, then rename
	if err := fileutil.AtomicWrite(absPath, data); err != nil {
		return errors.Wrap(err, "write project.md")
	}
	return nil
}

func ReadProjectMD(path string) (*types.Project, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, errors.Wrap(err, "read project.md")
	}
	var project types.Project
	if err := json.Unmarshal(data, &project); err != nil {
		return nil, errors.Wrap(err, "unmarshal project")
	}
	return &project, nil
}
```

---

### `internal/memory/artifacts/decisions.go` (service, file-I/O)

**Analog:** `internal/integrations/git/git.go` file operations

**Directory management with atomic writes**:
```go
func WriteDecision(m31aDir string, decision *types.Decision) error {
	decisionsDir := filepath.Join(m31aDir, "decisions")
	if err := os.MkdirAll(decisionsDir, types.DirPermission); err != nil {
		return errors.Wrap(err, "create decisions dir")
	}

	filename := fmt.Sprintf("%s-%s.md", decision.ID[:8], slugify(decision.Title))
	path := filepath.Join(decisionsDir, filename)

	content := formatDecision(decision)
	return fileutil.AtomicWrite(path, []byte(content))
}

func ListDecisions(m31aDir string) ([]*types.Decision, error) {
	decisionsDir := filepath.Join(m31aDir, "decisions")
	entries, err := os.ReadDir(decisionsDir)
	if err != nil {
		if os.IsNotExist(err) {
			return []*types.Decision{}, nil
		}
		return nil, errors.Wrap(err, "read decisions dir")
	}

	var decisions []*types.Decision
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".md") {
			path := filepath.Join(decisionsDir, entry.Name())
			decision, err := parseDecisionFile(path)
			if err != nil {
				slog.Warn("failed to parse decision", "file", entry.Name(), "error", err)
				continue
			}
			decisions = append(decisions, decision)
		}
	}
	return decisions, nil
}
```

---

### `internal/memory/artifacts/research.go` (service, file-I/O)

**Analog:** `internal/integrations/git/git.go` file operations

**Same pattern as decisions.go** — directory-based file management with atomic writes and parsing.

---

## Shared Patterns

### Error Handling
**Source:** `internal/core/errors/errors.go` (lines 139-156, 158-177)

**Apply to:** All service and model files

**Error wrapping:**
```go
func Wrap(err error, msg string) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", msg, err)
}

func Wrapf(err error, format string, args ...any) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", fmt.Sprintf(format, args...), err)
}
```

**Sentinel errors** (lines 10-58):
```go
var (
	ErrProviderUnreachable = errors.New("provider unreachable")
	ErrRateLimited         = errors.New("rate limited")
	ErrInvalidKey          = errors.New("invalid API key")
	ErrContextExceeded     = errors.New("context window exceeded")
	ErrSessionCorrupted    = errors.New("session data corrupted")
	ErrPermissionDenied    = errors.New("permission denied")
	ErrValidation          = errors.New("config validation")
	ErrNotImplemented      = errors.New("not implemented")
	// ... more
)
```

**User-friendly messages** (lines 158-277):
```go
func UserMessage(e error) string {
	switch {
	case errors.Is(e, ErrProviderUnreachable):
		return "Provider unreachable — check your internet connection"
	case errors.Is(e, ErrRateLimited):
		return "Rate limited — retry in a moment"
	case errors.Is(e, ErrInvalidKey):
		return "Invalid API key — run /settings to update"
	case errors.Is(e, ErrContextExceeded):
		return "Context window exceeded — conversation too long. Use /compress to reduce context."
	// ... more patterns
	}
	return "An unexpected error occurred — check the logs or try again"
}
```

---

### Configuration Loading (koanf)
**Source:** `internal/core/config/loader.go` (lines 215-343)

**Apply to:** `internal/core/config/loader.go` (extend for Phase 1)

**Layered loading order:**
1. Defaults (lowest priority)
2. Global TOML (~/.m31a/config.toml)
3. Workspace TOML (.m31a/workspace.toml)
4. .env file (auto-load)
5. Project JSON (m31a.json)
6. Environment variables M31A_* (highest priority)
7. Variable substitution ${VAR}
8. Validation

---

### OS Keychain Integration
**Source:** `internal/integrations/keychain/keychain.go` (lines 18-120)

**Apply to:** Config loader (ResolveAPIKeys), EventStore (if needed for credentials)

**Interface pattern with cached wrapper:**
```go
type Keychain interface {
	Get(service string) (string, error)
	Set(service, value string) error
	Delete(service string) error
}

// Platform-specific implementations via build tags
// keychain_darwin.go: security CLI
// keychain_linux.go: D-Bus Secret Service + pass CLI fallback
// keychain_windows.go: Windows Credential Manager
```

**Resolution priority:** env var → OS keychain → config file (never write secrets to config.toml)

---

### Atomic File Writes
**Source:** `internal/core/config/loader.go` (lines 575-591) + `internal/core/types/fileutil.go`

**Apply to:** All artifact writers (project_md.go, decisions.go, research.go, migration.go)

```go
func AtomicWrite(path string, data []byte) error {
	dir := filepath.Dir(path)
	tmpFile, err := os.CreateTemp(dir, ".tmp-*")
	if err != nil {
		return err
	}
	tmpPath := tmpFile.Name()
	defer os.Remove(tmpPath)

	if _, err := tmpFile.Write(data); err != nil {
		tmpFile.Close()
		return err
	}
	if err := tmpFile.Close(); err != nil {
		return err
	}
	return os.Rename(tmpPath, path)
}
```

---

### SQLite WAL Mode DSN
**Source:** RESEARCH.md lines 173-176 + `internal/memory/eventstore/eventstore.go` pattern

**Apply to:** `internal/memory/eventstore/eventstore.go`

```go
dsn := "file:" + dbPath + "?_pragma=journal_mode(WAL)&_pragma=foreign_keys(1)&_pragma=busy_timeout(5000)&_time_format=sqlite"
```

---

### UUID Generation
**Source:** `internal/core/types/types.go` imports + RESEARCH.md

**Apply to:** All domain types with ID fields

```go
import "github.com/google/uuid"

// For new IDs:
id := uuid.New()

// For parsing:
parsed, _ := uuid.Parse(str)

// For nil pointer handling:
var idPtr *uuid.UUID
if str != "" {
    parsed, _ := uuid.Parse(str)
    idPtr = &parsed
}
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `internal/memory/eventstore/schema.sql` | config (DDL) | file-I/O | No existing SQL schema files in codebase; pure DDL |
| `internal/core/types/eventstore.go` | service (interface) | request-response | Interface definition is new; pattern from keychain.go applies but no direct analog |

---

## Metadata

**Analog search scope:** internal/core/types/, internal/core/config/, internal/core/errors/, internal/integrations/keychain/, internal/integrations/git/
**Files scanned:** 12 existing files
**Pattern extraction date:** 2026-08-23