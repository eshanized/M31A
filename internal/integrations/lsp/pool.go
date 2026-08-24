package lsp

import (
	"fmt"
	"log/slog"
	"os/exec"
	"sync"
	"time"
)

// LSPError represents an LSP-related error.
type LSPError struct {
	Code    string
	Message string
	Details map[string]string
}

func (e *LSPError) Error() string {
	if e.Message != "" {
		return e.Message
	}
	return e.Code
}

// ErrLanguageNotSupported returns an error for unsupported languages.
func ErrLanguageNotSupported(lang string) error {
	return &LSPError{
		Code:    "LANGUAGE_NOT_SUPPORTED",
		Message: fmt.Sprintf("language %q is not supported", lang),
		Details: map[string]string{"language": lang},
	}
}

// ErrBinaryNotFound returns an error when a language server binary is not found.
func ErrBinaryNotFound(binary, language string) error {
	return &LSPError{
		Code:    "BINARY_NOT_FOUND",
		Message: fmt.Sprintf("language server %q not found in PATH — install %s for semantic analysis", binary, language),
		Details: map[string]string{"binary": binary, "language": language},
	}
}

// ErrClientClosed returns an error when the client is closed.
func ErrClientClosed() error {
	return &LSPError{
		Code:    "CLIENT_CLOSED",
		Message: "LSP client is closed",
	}
}

// poolEntry holds a client and its last used time.
type poolEntry struct {
	client   *LSPClient
	lastUsed time.Time
}

// LSPPool manages per-project per-language LSP connections with idle timeout.
type LSPPool struct {
	mu       sync.RWMutex
	clients  map[string]map[string]*poolEntry // projectRoot -> language -> entry
	idleTTL  time.Duration
	logger   *slog.Logger
}

// PoolStats contains statistics about the pool.
type PoolStats struct {
	TotalConnections int
	ByLanguage       map[string]int
	ByProject        map[string]int
}

// NewLSPPool creates a new LSP connection pool with the given idle TTL.
func NewLSPPool(idleTTL time.Duration, logger *slog.Logger) *LSPPool {
	if idleTTL <= 0 {
		idleTTL = 5 * time.Minute
	}
	if logger == nil {
		logger = slog.Default()
	}
	return &LSPPool{
		clients: make(map[string]map[string]*poolEntry),
		idleTTL: idleTTL,
		logger:  logger,
	}
}

// GetClient returns an LSP client for the given project and language.
// If a client already exists and is not idle, it is returned.
// Otherwise, a new client is started (lazy start).
func (p *LSPPool) GetClient(projectRoot, language string) (*LSPClient, error) {
	p.mu.RLock()
	if projClients, ok := p.clients[projectRoot]; ok {
		if entry, ok := projClients[language]; ok {
			// Check if client is still valid and not closed
			if !entry.client.IsClosed() {
				entry.client.Touch()
				entry.lastUsed = time.Now()
				p.mu.RUnlock()
				return entry.client, nil
			}
			// Client is closed, will create new one
		}
	}
	p.mu.RUnlock()

	// Need to create new client
	return p.startClient(projectRoot, language)
}

// startClient creates a new LSP client for the given project and language.
func (p *LSPPool) startClient(projectRoot, language string) (*LSPClient, error) {
	config, ok := ServerConfigForLanguage(language)
	if !ok {
		return nil, ErrLanguageNotSupported(language)
	}

	// Check if binary exists in PATH
	if _, err := exec.LookPath(config.Binary); err != nil {
		return nil, ErrBinaryNotFound(config.Binary, language)
	}

	client, err := NewLSPClient(config.Command, projectRoot)
	if err != nil {
		return nil, fmt.Errorf("start LSP client for %s: %w", language, err)
	}

	p.mu.Lock()
	if p.clients[projectRoot] == nil {
		p.clients[projectRoot] = make(map[string]*poolEntry)
	}
	entry := &poolEntry{
		client:   client,
		lastUsed: time.Now(),
	}
	p.clients[projectRoot][language] = entry
	p.mu.Unlock()

	p.logger.Debug("LSP client started", "project", projectRoot, "language", language)
	return client, nil
}

// ShutdownIdle closes clients that have been idle longer than the idle TTL.
func (p *LSPPool) ShutdownIdle() {
	p.mu.Lock()
	defer p.mu.Unlock()

	now := time.Now()
	for projectRoot, langClients := range p.clients {
		for language, entry := range langClients {
			if now.Sub(entry.lastUsed) > p.idleTTL {
				p.logger.Debug("Shutting down idle LSP client", "project", projectRoot, "language", language)
				_ = entry.client.Close()
				delete(langClients, language)
			}
		}
		if len(langClients) == 0 {
			delete(p.clients, projectRoot)
		}
	}
}

// ShutdownAll closes all clients in the pool.
func (p *LSPPool) ShutdownAll() {
	p.mu.Lock()
	defer p.mu.Unlock()

	for projectRoot, langClients := range p.clients {
		for language, entry := range langClients {
			p.logger.Debug("Shutting down LSP client", "project", projectRoot, "language", language)
			_ = entry.client.Close()
		}
	}
	p.clients = make(map[string]map[string]*poolEntry)
}

// Stats returns statistics about the current pool state.
func (p *LSPPool) Stats() PoolStats {
	p.mu.RLock()
	defer p.mu.RUnlock()

	stats := PoolStats{
		ByLanguage: make(map[string]int),
		ByProject:  make(map[string]int),
	}

	for projectRoot, langClients := range p.clients {
		stats.ByProject[projectRoot] = len(langClients)
		for language := range langClients {
			stats.ByLanguage[language]++
			stats.TotalConnections++
		}
	}

	return stats
}