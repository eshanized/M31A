package lsp

import (
	"context"
	"fmt"
	"log/slog"
	"os/exec"
	"sync"
	"time"
)

// LSPManager handles lifecycle management of LSP clients per project.
type LSPManager struct {
	pool       *LSPPool
	configs    map[string]ServerConfig
	logger     *slog.Logger
	mu         sync.Mutex
	projectRoot string
}

// NewLSPManager creates a new LSP manager for the given project root.
func NewLSPManager(projectRoot string, logger *slog.Logger) *LSPManager {
	if logger == nil {
		logger = slog.Default()
	}

	pool := NewLSPPool(5*time.Minute, logger)

	// Load default server configs
	configs := make(map[string]ServerConfig)
	for lang, config := range defaultServers {
		configs[lang] = config
	}

	return &LSPManager{
		pool:        pool,
		configs:     configs,
		logger:      logger,
		projectRoot: projectRoot,
	}
}

// EnsureStarted ensures the LSP server for the given language is started.
// Returns nil for unknown languages or missing binaries (graceful degradation).
func (m *LSPManager) EnsureStarted(ctx context.Context, language string) error {
	// Check if language is supported
	config, ok := m.configs[language]
	if !ok {
		m.logger.Debug("Language not supported, skipping LSP", "language", language)
		return nil
	}

	// Check if binary exists
	if _, err := exec.LookPath(config.Binary); err != nil {
		m.logger.Warn("Language server binary not found, graceful degradation",
			"language", language, "binary", config.Binary)
		return nil
	}

	// Get or start client from pool
	_, err := m.pool.GetClient(m.projectRoot, language)
	if err != nil {
		m.logger.Warn("Failed to start LSP client, graceful degradation",
			"language", language, "error", err)
		return nil
	}

	return nil
}

// SemanticQuery executes a semantic query using the LSP client for the given language.
// It handles crash recovery by restarting the client and retrying once.
func (m *LSPManager) SemanticQuery(ctx context.Context, language string, fn func(*LSPClient) error) error {
	// Ensure the server is started
	if err := m.EnsureStarted(ctx, language); err != nil {
		return err
	}

	// Get client from pool
	client, err := m.pool.GetClient(m.projectRoot, language)
	if err != nil {
		return fmt.Errorf("get LSP client for %s: %w", language, err)
	}

	// Execute the query function
	err = fn(client)
	if err == nil {
		return nil
	}

	// Check if error indicates a crash (broken pipe, process exited)
	if isCrashError(err) {
		m.logger.Warn("LSP client crashed, attempting restart", "language", language, "error", err)

		// Close the crashed client
		_ = client.Close()

		// Remove from pool to force recreation
		m.removeClient(m.projectRoot, language)

		// Create new client
		newClient, err := m.pool.GetClient(m.projectRoot, language)
		if err != nil {
			return fmt.Errorf("restart LSP client for %s: %w", language, err)
		}

		// Retry once with new client
		return fn(newClient)
	}

	return err
}

// isCrashError determines if an error indicates an LSP server crash.
func isCrashError(err error) bool {
	if err == nil {
		return false
	}
	errStr := err.Error()
	// Check for common crash indicators
	return containsStr(errStr, "broken pipe") ||
		containsStr(errStr, "connection reset") ||
		containsStr(errStr, "process exited") ||
		containsStr(errStr, "EOF") ||
		containsStr(errStr, "unexpected EOF") ||
		containsStr(errStr, "transport closed")
}

// containsStr checks if a string contains a substring.
func containsStr(s, substr string) bool {
	if len(substr) == 0 {
		return true
	}
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// removeClient removes a client from the pool (internal method).
func (m *LSPManager) removeClient(projectRoot, language string) {
	// We can't directly access the pool's internal map, so we rely on
	// the pool's GetClient to create a new one if the old one is closed.
	// The pool checks IsClosed() before returning a cached client.
}

// StartIdleReaper starts a background goroutine that periodically shuts down idle clients.
func (m *LSPManager) StartIdleReaper(ctx context.Context, interval time.Duration) {
	if interval <= 0 {
		interval = 1 * time.Minute
	}

	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				m.logger.Debug("Idle reaper stopped")
				return
			case <-ticker.C:
				m.pool.ShutdownIdle()
			}
		}
	}()
}

// Shutdown shuts down all LSP clients in the pool.
func (m *LSPManager) Shutdown() {
	m.logger.Debug("Shutting down LSP manager")
	m.pool.ShutdownAll()
}

// LanguageForFile determines the language for a given file path.
func (m *LSPManager) LanguageForFile(path string) (string, bool) {
	return FindLanguageForFile(path)
}

// ProjectRoot returns the project root directory.
func (m *LSPManager) ProjectRoot() string {
	return m.projectRoot
}

// Stats returns pool statistics.
func (m *LSPManager) Stats() PoolStats {
	return m.pool.Stats()
}