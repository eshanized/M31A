package codeintel

import (
	"context"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
)

// WatcherEventType represents the type of file system event.
type WatcherEventType string

const (
	WatcherCreated  WatcherEventType = "created"
	WatcherModified WatcherEventType = "modified"
	WatcherDeleted  WatcherEventType = "deleted"
)

// WatcherEvent represents a file system change event.
type WatcherEvent struct {
	Path      string
	EventType WatcherEventType
	Timestamp time.Time
}

// FileWatcher watches a directory tree for file changes using fsnotify
// with 500ms debounce, delivering events via a channel for background processing.
type FileWatcher struct {
	watcher *fsnotify.Watcher
	ctx     context.Context
	cancel  context.CancelFunc
	events  chan WatcherEvent
	debounce *time.Timer
	mu       sync.Mutex
	workDir  string
	logger   *slog.Logger
	parsers  []Parser
}

// NewFileWatcher creates a new file watcher for the given working directory.
// It watches the workDir and all immediate subdirectories (recursively).
// Events are debounced with a 500ms delay.
// The watcher is not started until Start() is called.
func NewFileWatcher(workDir string, parsers []Parser, logger *slog.Logger) (*FileWatcher, error) {
	w, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	ctx, cancel := context.WithCancel(context.Background())
	fw := &FileWatcher{
		watcher: w,
		ctx:     ctx,
		cancel:  cancel,
		events:  make(chan WatcherEvent, 100),
		workDir: workDir,
		logger:  logger,
		parsers: parsers,
	}

	// Add the root working directory
	if err := fw.watcher.Add(workDir); err != nil {
		cancel()
		_ = w.Close()
		return nil, err
	}

	// Walk subdirectories and add them (skip common ignored dirs)
	fw.walkAndAdd()

	return fw, nil
}

// Start starts the file watcher event loop with the given context.
// The provided context controls the lifetime of the watcher.
func (fw *FileWatcher) Start(ctx context.Context) {
	// Replace the internal context with the provided one
	fw.mu.Lock()
	fw.ctx = ctx
	fw.cancel = func() {} // no-op cancel since context is externally controlled
	fw.mu.Unlock()
	go fw.loop()
}

// walkAndAdd recursively adds directories to the watcher, skipping
// common build/dependency directories that should be ignored.
func (fw *FileWatcher) walkAndAdd() {
	visited := map[string]bool{fw.workDir: true}

	var walk func(dir string)
	walk = func(dir string) {
		select {
		case <-fw.ctx.Done():
			return
		default:
		}

		entries, err := os.ReadDir(dir)
		if err != nil {
			return
		}

		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			name := entry.Name()
			if isIgnoredDir(name) {
				continue
			}
			fullPath := filepath.Join(dir, name)
			if visited[fullPath] {
				continue
			}
			visited[fullPath] = true

			if err := fw.watcher.Add(fullPath); err != nil {
				fw.logger.Debug("file watcher: failed to add directory", "path", fullPath, "error", err)
				continue
			}
			walk(fullPath)
		}
	}

	walk(fw.workDir)
}

// loop reads fsnotify events and debounces them before sending to the events channel.
func (fw *FileWatcher) loop() {
	for {
		select {
		case <-fw.ctx.Done():
			close(fw.events)
			return
		case event, ok := <-fw.watcher.Events:
			if !ok {
				close(fw.events)
				return
			}
			if shouldIgnoreEvent(event.Name) {
				continue
			}
			fw.debounceEvent(event)
		case err, ok := <-fw.watcher.Errors:
			if !ok {
				return
			}
			fw.logger.Debug("file watcher error", "error", err)
		}
	}
}

// debounceEvent resets the debounce timer. When it fires (after 500ms of
// silence), we send the accumulated events to the events channel.
// For simplicity, this implementation debounces per-event; a more sophisticated
// version would batch multiple events.
func (fw *FileWatcher) debounceEvent(event fsnotify.Event) {
	fw.mu.Lock()
	if fw.debounce != nil {
		fw.debounce.Stop()
	}
	fw.debounce = time.AfterFunc(500*time.Millisecond, func() {
		fw.mu.Lock()
		defer fw.mu.Unlock()

		var eventType WatcherEventType
		switch {
		case event.Op&fsnotify.Create != 0:
			eventType = WatcherCreated
		case event.Op&fsnotify.Write != 0:
			eventType = WatcherModified
		case event.Op&fsnotify.Remove != 0:
			eventType = WatcherDeleted
		case event.Op&fsnotify.Rename != 0:
			// Treat rename as delete + create; check if file still exists
			if _, err := os.Stat(event.Name); err == nil {
				eventType = WatcherCreated
			} else {
				eventType = WatcherDeleted
			}
		default:
			eventType = WatcherModified
		}

		select {
		case fw.events <- WatcherEvent{
			Path:      event.Name,
			EventType: eventType,
			Timestamp: time.Now(),
		}:
		case <-fw.ctx.Done():
		}
	})
	fw.mu.Unlock()
}

// Events returns a read-only channel for receiving file watcher events.
func (fw *FileWatcher) Events() <-chan WatcherEvent {
	return fw.events
}

// Stop stops the file watcher and cleans up resources.
func (fw *FileWatcher) Stop() {
	fw.mu.Lock()
	if fw.debounce != nil {
		fw.debounce.Stop()
	}
	fw.mu.Unlock()
	if err := fw.watcher.Close(); err != nil {
		fw.logger.Debug("close file watcher", "error", err)
	}
}

// AddDirectory adds a directory to the watcher.
func (fw *FileWatcher) AddDirectory(path string) error {
	return fw.watcher.Add(path)
}

// RemoveDirectory removes a directory from the watcher.
func (fw *FileWatcher) RemoveDirectory(path string) error {
	return fw.watcher.Remove(path)
}

// isIgnoredDir returns true for directories that should not be watched.
func isIgnoredDir(name string) bool {
	switch name {
	case "node_modules", "vendor", ".next", "dist", "build", "target",
		".venv", "venv", "__pycache__", ".git", ".m31a", "tmp", ".cache":
		return true
	}
	return strings.HasPrefix(name, ".") && name != "."
}

// shouldIgnoreEvent returns true for file events that should be ignored.
func shouldIgnoreEvent(path string) bool {
	base := filepath.Base(path)
	// Ignore hidden files
	if strings.HasPrefix(base, ".") {
		return true
	}
	// Ignore common generated/temp files
	switch {
	case strings.HasSuffix(base, "~"):
		return true
	case strings.HasPrefix(base, ".#") || strings.HasPrefix(base, "#"):
		return true
	case strings.HasSuffix(base, ".swp") || strings.HasSuffix(base, ".swo"):
		return true
	}
	return false
}