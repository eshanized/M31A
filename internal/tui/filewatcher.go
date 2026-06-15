package tui

import (
	"context"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/fsnotify/fsnotify"
	tea "github.com/charmbracelet/bubbletea"
)

// FileWatcher watches the working directory for file system changes and emits
// SidebarRefreshTickMsg to trigger immediate sidebar refreshes. This replaces
// the slow 5-second polling with near-instant detection of file changes.
type FileWatcher struct {
	watcher  *fsnotify.Watcher
	ctx      context.Context
	cancel   context.CancelFunc
	Events   chan tea.Msg
	workDir  string
	debounce *time.Timer
}

// NewFileWatcher creates a file watcher for the given working directory.
// It watches non-ignored directories recursively for create, write, remove,
// and rename events. Events are debounced (300ms) and emit a
// SidebarRefreshTickMsg to trigger an immediate sidebar refresh.
func NewFileWatcher(workDir string, events chan tea.Msg) (*FileWatcher, error) {
	w, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	ctx, cancel := context.WithCancel(context.Background())
	fw := &FileWatcher{
		watcher: w,
		ctx:     ctx,
		cancel:  cancel,
		Events:  events,
		workDir: workDir,
	}

	// Add the root working directory
	if err := fw.watcher.Add(workDir); err != nil {
		cancel()
		w.Close()
		return nil, err
	}

	// Walk subdirectories and add them (skip common ignored dirs)
	go fw.walkAndAdd()

	// Start the event loop
	go fw.loop()

	return fw, nil
}

// walkAndAdd recursively adds directories to the watcher, skipping
// common build/dependency directories that should be ignored.
func (fw *FileWatcher) walkAndAdd() {
	visited := map[string]bool{fw.workDir: true}

	var walk func(dir string)
	walk = func(dir string) {
		entries, err := os.ReadDir(dir)
		if err != nil {
			return
		}
		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			name := entry.Name()
			// Skip common ignored directories
			if isIgnoredDir(name) {
				continue
			}
			fullPath := filepath.Join(dir, name)
			if visited[fullPath] {
				continue
			}
			visited[fullPath] = true

			if err := fw.watcher.Add(fullPath); err != nil {
				continue
			}
			walk(fullPath)
		}
	}

	walk(fw.workDir)
}

// loop reads fsnotify events and debounce-emits a SidebarRefreshTickMsg.
func (fw *FileWatcher) loop() {
	for {
		select {
		case <-fw.ctx.Done():
			return
		case event, ok := <-fw.watcher.Events:
			if !ok {
				return
			}
			// Ignore hidden files and common non-project files
			if shouldIgnoreEvent(event.Name) {
				continue
			}
			// Debounce: reset timer on each event
			fw.debounceEvent()
		case err, ok := <-fw.watcher.Errors:
			if !ok {
				return
			}
			slog.Debug("file watcher error", "error", err)
		}
	}
}

// debounceEvent resets the debounce timer. When it fires (after 300ms of
// silence), we emit a SidebarRefreshTickMsg to trigger a sidebar refresh.
func (fw *FileWatcher) debounceEvent() {
	if fw.debounce != nil {
		fw.debounce.Stop()
	}
	fw.debounce = time.AfterFunc(300*time.Millisecond, func() {
		select {
		case fw.Events <- SidebarRefreshTickMsg{}:
		case <-fw.ctx.Done():
		}
	})
}

// Close stops the file watcher and cleans up resources.
func (fw *FileWatcher) Close() {
	fw.cancel()
	if fw.debounce != nil {
		fw.debounce.Stop()
	}
	fw.watcher.Close()
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
	// Ignore hidden files (except .m31a state files which we don't watch)
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
