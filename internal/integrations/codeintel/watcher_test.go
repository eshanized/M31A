package codeintel

import (
	"context"
	"log/slog"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestFileWatcher_DetectsCreate(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(os.Stderr, nil))

	fw, err := NewFileWatcher(tmpDir, AllParsers(), logger)
	if err != nil {
		t.Fatalf("NewFileWatcher failed: %v", err)
	}
	fw.Start(context.Background())
	defer fw.Stop()

	// Create a test file
	testFile := filepath.Join(tmpDir, "test_file.go")
	content := []byte("package main\n\nfunc main() {}\n")
	if err := os.WriteFile(testFile, content, 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	// Wait for event with timeout
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	select {
	case event := <-fw.Events():
		// os.WriteFile may trigger Create then Write; accept either
		if event.EventType != WatcherCreated && event.EventType != WatcherModified {
			t.Errorf("expected WatcherCreated or WatcherModified, got %s", event.EventType)
		}
		if event.Path != testFile {
			t.Errorf("expected path %s, got %s", testFile, event.Path)
		}
	case <-ctx.Done():
		t.Fatal("timeout waiting for create event")
	}
}

func TestFileWatcher_DetectsModify(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(os.Stderr, nil))

	fw, err := NewFileWatcher(tmpDir, AllParsers(), logger)
	if err != nil {
		t.Fatalf("NewFileWatcher failed: %v", err)
	}
	fw.Start(context.Background())
	defer fw.Stop()

	// Create initial file
	testFile := filepath.Join(tmpDir, "test_file.go")
	content := []byte("package main\n\nfunc main() {}\n")
	if err := os.WriteFile(testFile, content, 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	// Wait for initial create event
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	select {
	case <-fw.Events():
	case <-ctx.Done():
		t.Fatal("timeout waiting for initial create event")
	}
	cancel()

	// Modify the file
	newContent := []byte("package main\n\nfunc main() { println(\"modified\") }\n")
	if err := os.WriteFile(testFile, newContent, 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	// Wait for modify event
	ctx, cancel = context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	select {
	case event := <-fw.Events():
		if event.EventType != WatcherModified {
			t.Errorf("expected WatcherModified, got %s", event.EventType)
		}
	case <-ctx.Done():
		t.Fatal("timeout waiting for modify event")
	}
}

func TestFileWatcher_DetectsDelete(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(os.Stderr, nil))

	fw, err := NewFileWatcher(tmpDir, AllParsers(), logger)
	if err != nil {
		t.Fatalf("NewFileWatcher failed: %v", err)
	}
	fw.Start(context.Background())
	defer fw.Stop()

	// Create initial file
	testFile := filepath.Join(tmpDir, "test_file.go")
	content := []byte("package main\n\nfunc main() {}\n")
	if err := os.WriteFile(testFile, content, 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	// Wait for initial create event
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	select {
	case <-fw.Events():
	case <-ctx.Done():
		t.Fatal("timeout waiting for initial create event")
	}
	cancel()

	// Delete the file
	if err := os.Remove(testFile); err != nil {
		t.Fatalf("Remove failed: %v", err)
	}

	// Wait for delete event
	ctx, cancel = context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	select {
	case event := <-fw.Events():
		if event.EventType != WatcherDeleted {
			t.Errorf("expected WatcherDeleted, got %s", event.EventType)
		}
	case <-ctx.Done():
		t.Fatal("timeout waiting for delete event")
	}
}

func TestFileWatcher_Debounce(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(os.Stderr, nil))

	fw, err := NewFileWatcher(tmpDir, AllParsers(), logger)
	if err != nil {
		t.Fatalf("NewFileWatcher failed: %v", err)
	}
	fw.Start(context.Background())
	defer fw.Stop()

	// Create 5 files rapidly
	for i := 0; i < 5; i++ {
		testFile := filepath.Join(tmpDir, "test_file_"+string(rune('0'+i))+".go")
		content := []byte("package main\n\nfunc main() {}\n")
		if err := os.WriteFile(testFile, content, 0o644); err != nil {
			t.Fatalf("WriteFile failed: %v", err)
		}
		// Small delay to ensure they're separate fsnotify events
		time.Sleep(10 * time.Millisecond)
	}

	// Wait for debounced events - should receive fewer than 5 events
	// due to 500ms debounce (all 5 files created within ~50ms)
	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	eventCount := 0
	for {
		select {
		case <-fw.Events():
			eventCount++
		case <-ctx.Done():
			goto done
		}
	}
done:

	// With 500ms debounce and rapid creation, we should get at most 2 events
	// (the debounce timer resets on each event)
	if eventCount > 2 {
		t.Errorf("expected <= 2 debounced events, got %d", eventCount)
	}
	if eventCount == 0 {
		t.Error("expected at least 1 event, got 0")
	}
}

func TestFileWatcher_Stop(t *testing.T) {
	tmpDir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(os.Stderr, nil))

	fw, err := NewFileWatcher(tmpDir, AllParsers(), logger)
	if err != nil {
		t.Fatalf("NewFileWatcher failed: %v", err)
	}
	fw.Start(context.Background())

	// Stop should not panic
	fw.Stop()

	// Calling Stop twice should not panic
	fw.Stop()

	// Verify events channel is closed
	select {
	case _, ok := <-fw.Events():
		if ok {
			t.Error("events channel should be closed after Stop")
		}
	default:
	}
}