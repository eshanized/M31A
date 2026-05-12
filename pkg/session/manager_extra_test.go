package session

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// newTestManagerIsolated creates a Manager whose base directory is nested
// inside a fresh t.TempDir(). This ensures that recentModelsPath()
// (which uses filepath.Dir of the base dir) resolves to a unique parent,
// avoiding shared-file conflicts between parallel tests.
func newTestManagerIsolated(t *testing.T) (*Manager, string) {
	t.Helper()
	parent := t.TempDir()
	baseDir := filepath.Join(parent, "sessions")
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		t.Fatalf("Failed to create base dir: %v", err)
	}
	return NewManager(baseDir, baseDir, ManagerOpts{}), baseDir
}

// ---------------------------------------------------------------------------
// ExportSessionJSON
// ---------------------------------------------------------------------------

func TestManager_ExportSessionJSON(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}
	s.Messages = append(s.Messages, types.Message{Role: "user", Content: "hello"})
	s.MessageCount = 1
	if err := mgr.SaveSession(s); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	exportPath := filepath.Join(t.TempDir(), "export.json")
	if err := mgr.ExportSessionJSON(s.ID, exportPath); err != nil {
		t.Fatalf("ExportSessionJSON failed: %v", err)
	}

	data, err := os.ReadFile(exportPath)
	if err != nil {
		t.Fatalf("Failed to read export file: %v", err)
	}

	var exported Session
	if err := json.Unmarshal(data, &exported); err != nil {
		t.Fatalf("Export file is not valid JSON: %v", err)
	}
	if exported.ID != s.ID {
		t.Errorf("Expected ID %s, got %s", s.ID, exported.ID)
	}
	if exported.Model != "gpt-4o" {
		t.Errorf("Expected model gpt-4o, got %s", exported.Model)
	}
	if len(exported.Messages) != 1 {
		t.Errorf("Expected 1 message, got %d", len(exported.Messages))
	}
}

func TestManager_ExportSessionJSON_NonexistentSession(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	exportPath := filepath.Join(t.TempDir(), "export.json")
	err := mgr.ExportSessionJSON("nonexistent", exportPath)
	if err == nil {
		t.Error("Expected error for nonexistent session")
	}
}

// ---------------------------------------------------------------------------
// RenameSession
// ---------------------------------------------------------------------------

func TestManager_RenameSession(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	if err := mgr.RenameSession(s.ID, "my cool session"); err != nil {
		t.Fatalf("RenameSession failed: %v", err)
	}

	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession failed: %v", err)
	}
	if loaded.Label != "my cool session" {
		t.Errorf("Expected label 'my cool session', got %q", loaded.Label)
	}
}

func TestManager_RenameSession_ClearLabel(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	if err := mgr.RenameSession(s.ID, "initial name"); err != nil {
		t.Fatalf("RenameSession failed: %v", err)
	}
	if err := mgr.RenameSession(s.ID, ""); err != nil {
		t.Fatalf("RenameSession clear failed: %v", err)
	}

	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession failed: %v", err)
	}
	if loaded.Label != "" {
		t.Errorf("Expected empty label after clear, got %q", loaded.Label)
	}
}

func TestManager_RenameSession_Nonexistent(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	err := mgr.RenameSession("deadbeef", "name")
	if err == nil {
		t.Error("Expected error for nonexistent session")
	}
}

// ---------------------------------------------------------------------------
// FilterSessions
// ---------------------------------------------------------------------------

func TestManager_FilterSessions_EmptyQuery(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_FilterSessions_ByModel(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_FilterSessions_ByLabel(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_FilterSessions_CaseInsensitive(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, _ = mgr.NewSession("GPT-4o", "openrouter")

	filtered, err := mgr.FilterSessions("gpt")
	if err != nil {
		t.Fatalf("FilterSessions failed: %v", err)
	}
	if len(filtered) != 1 {
		t.Errorf("Expected case-insensitive match, got %d results", len(filtered))
	}
}

func TestManager_FilterSessions_NoMatch(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, _ = mgr.NewSession("gpt-4o", "openrouter")

	filtered, err := mgr.FilterSessions("nonexistent-model")
	if err != nil {
		t.Fatalf("FilterSessions failed: %v", err)
	}
	if len(filtered) != 0 {
		t.Errorf("Expected 0 matches, got %d", len(filtered))
	}
}

// ---------------------------------------------------------------------------
// LoadRecentModels / SaveRecentModels
// (use newTestManagerIsolated to avoid shared /tmp/recent_models.json)
// ---------------------------------------------------------------------------

func TestManager_LoadRecentModels_Missing(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	rmd, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if rmd.Recent == nil {
		t.Error("Expected non-nil Recent slice")
	}
	if len(rmd.Recent) != 0 {
		t.Errorf("Expected empty Recent, got %d", len(rmd.Recent))
	}
	if rmd.Favorites == nil {
		t.Error("Expected non-nil Favorites map")
	}
	if len(rmd.Favorites) != 0 {
		t.Errorf("Expected empty Favorites, got %d", len(rmd.Favorites))
	}
}

func TestManager_SaveAndLoadRecentModels(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	rmd := &RecentModelsData{
		Recent:    []string{"model-a", "model-b", "model-c"},
		Favorites: map[string]bool{"model-a": true},
	}
	if err := mgr.SaveRecentModels(rmd); err != nil {
		t.Fatalf("SaveRecentModels failed: %v", err)
	}

	loaded, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if len(loaded.Recent) != 3 {
		t.Errorf("Expected 3 recent models, got %d", len(loaded.Recent))
	}
	if loaded.Recent[0] != "model-a" || loaded.Recent[1] != "model-b" || loaded.Recent[2] != "model-c" {
		t.Errorf("Recent models mismatch: %v", loaded.Recent)
	}
	if !loaded.Favorites["model-a"] {
		t.Error("Expected model-a to be favorited")
	}
}

func TestManager_SaveRecentModels_PruneToMax(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	rmd := &RecentModelsData{
		Recent:    []string{"m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9", "m10", "m11", "m12"},
		Favorites: make(map[string]bool),
	}
	if err := mgr.SaveRecentModels(rmd); err != nil {
		t.Fatalf("SaveRecentModels failed: %v", err)
	}

	loaded, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if len(loaded.Recent) != 10 {
		t.Errorf("Expected 10 recent models (pruned), got %d", len(loaded.Recent))
	}
}

func TestManager_SaveRecentModels_NilFavorites(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	rmd := &RecentModelsData{
		Recent:    []string{"model-a"},
		Favorites: nil,
	}
	if err := mgr.SaveRecentModels(rmd); err != nil {
		t.Fatalf("SaveRecentModels failed: %v", err)
	}

	loaded, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if loaded.Favorites == nil {
		t.Error("Expected Favorites to be initialized even if saved as nil")
	}
}

func TestManager_LoadRecentModels_CorruptJSON(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	path := mgr.recentModelsPath()
	if err := os.WriteFile(path, []byte("{corrupt"), 0644); err != nil {
		t.Fatal(err)
	}

	_, err := mgr.LoadRecentModels()
	if err == nil {
		t.Error("Expected error for corrupt recent models JSON")
	}
}

// ---------------------------------------------------------------------------
// AddRecentModel
// ---------------------------------------------------------------------------

func TestManager_AddRecentModel(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	if err := mgr.AddRecentModel("model-a"); err != nil {
		t.Fatalf("AddRecentModel failed: %v", err)
	}
	if err := mgr.AddRecentModel("model-b"); err != nil {
		t.Fatalf("AddRecentModel failed: %v", err)
	}

	rmd, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if len(rmd.Recent) != 2 {
		t.Fatalf("Expected 2 recent models, got %d", len(rmd.Recent))
	}
	if rmd.Recent[0] != "model-b" {
		t.Errorf("Expected model-b first, got %s", rmd.Recent[0])
	}
	if rmd.Recent[1] != "model-a" {
		t.Errorf("Expected model-a second, got %s", rmd.Recent[1])
	}
}

func TestManager_AddRecentModel_Dedup(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	mgr.AddRecentModel("model-a")
	mgr.AddRecentModel("model-b")
	mgr.AddRecentModel("model-a") // should move to front

	rmd, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if len(rmd.Recent) != 2 {
		t.Fatalf("Expected 2 (deduped), got %d", len(rmd.Recent))
	}
	if rmd.Recent[0] != "model-a" {
		t.Errorf("Expected model-a at front after dedup, got %s", rmd.Recent[0])
	}
}

func TestManager_AddRecentModel_CapsAtMax(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	for i := 0; i < 15; i++ {
		model := "model-" + string(rune('a'+i%26))
		if err := mgr.AddRecentModel(model); err != nil {
			t.Fatalf("AddRecentModel %d failed: %v", i, err)
		}
	}

	rmd, err := mgr.LoadRecentModels()
	if err != nil {
		t.Fatalf("LoadRecentModels failed: %v", err)
	}
	if len(rmd.Recent) != 10 {
		t.Errorf("Expected capped at 10, got %d", len(rmd.Recent))
	}
}

// ---------------------------------------------------------------------------
// ToggleFavorite / IsFavorite
// ---------------------------------------------------------------------------

func TestManager_ToggleFavorite(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	if mgr.IsFavorite("model-a") {
		t.Error("model-a should not be favorited initially")
	}

	if err := mgr.ToggleFavorite("model-a"); err != nil {
		t.Fatalf("ToggleFavorite failed: %v", err)
	}
	if !mgr.IsFavorite("model-a") {
		t.Error("model-a should be favorited after toggle on")
	}

	if err := mgr.ToggleFavorite("model-a"); err != nil {
		t.Fatalf("ToggleFavorite failed: %v", err)
	}
	if mgr.IsFavorite("model-a") {
		t.Error("model-a should not be favorited after toggle off")
	}
}

func TestManager_IsFavorite_Multiple(t *testing.T) {
	t.Parallel()
	mgr, _ := newTestManagerIsolated(t)

	mgr.ToggleFavorite("model-a")
	mgr.ToggleFavorite("model-c")

	if !mgr.IsFavorite("model-a") {
		t.Error("model-a should be favorited")
	}
	if mgr.IsFavorite("model-b") {
		t.Error("model-b should not be favorited")
	}
	if !mgr.IsFavorite("model-c") {
		t.Error("model-c should be favorited")
	}
}

// ---------------------------------------------------------------------------
// Cleanup edge cases
// ---------------------------------------------------------------------------

func TestManager_Cleanup_NoOldSessions(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	removed, err := mgr.Cleanup(30 * 24 * time.Hour)
	if err != nil {
		t.Fatalf("Cleanup failed: %v", err)
	}
	if removed != 0 {
		t.Errorf("Expected 0 removed, got %d", removed)
	}
}

func TestManager_Cleanup_AllOld(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_Cleanup_InvalidatesCache(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_Cleanup_SkipsFiles(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	oldTime := time.Now().Add(-60 * 24 * time.Hour)
	filePath := filepath.Join(dir, "not-a-session.txt")
	os.WriteFile(filePath, []byte("garbage"), 0644)
	os.Chtimes(filePath, oldTime, oldTime)

	removed, err := mgr.Cleanup(30 * 24 * time.Hour)
	if err != nil {
		t.Fatalf("Cleanup failed: %v", err)
	}
	if removed != 0 {
		t.Errorf("Expected 0 removed (files should be skipped), got %d", removed)
	}
	if _, err := os.Stat(filePath); os.IsNotExist(err) {
		t.Error("Regular file should not be removed by cleanup")
	}
}

func TestManager_Cleanup_SkipsSubdirsOfArchived(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_Cleanup_ExactCutoff(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessionDir := filepath.Join(dir, "cutoff001")
	os.MkdirAll(sessionDir, 0755)
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte(`{"id":"cutoff001"}`), 0644)

	// Set modtime 1 second AFTER the cutoff so it is NOT before the cutoff
	// Cleanup computes cutoff = time.Now().Add(-maxAge). By setting modtime
	// to cutoff+1s we guarantee it survives. We use the actual maxAge value
	// to compute the right boundary.
	maxAge := 30 * 24 * time.Hour
	cutoffCandidate := time.Now().Add(-maxAge)
	modTime := cutoffCandidate.Add(1 * time.Second)
	os.Chtimes(sessionDir, modTime, modTime)

	removed, err := mgr.Cleanup(maxAge)
	if err != nil {
		t.Fatalf("Cleanup failed: %v", err)
	}
	if removed != 0 {
		t.Errorf("Session just after cutoff should be kept, but %d were removed", removed)
	}
}

// ---------------------------------------------------------------------------
// saveSessionAtomic / loadSessionMetadata
// ---------------------------------------------------------------------------

func TestManager_saveSessionAtomic(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// saveSessionAtomic writes session.json only (not messages.json).
	// Verify that the label persists correctly via session.json.
	s.Label = "atomic test"
	s.WorkflowPhase = types.PhasePlan
	if err := mgr.saveSessionAtomic(s); err != nil {
		t.Fatalf("saveSessionAtomic failed: %v", err)
	}

	// Verify via loadSessionMetadata (reads only session.json)
	meta, err := mgr.loadSessionMetadata()
	if err != nil {
		t.Fatalf("loadSessionMetadata failed: %v", err)
	}
	if meta.Label != "atomic test" {
		t.Errorf("Expected label 'atomic test', got %q", meta.Label)
	}
	if meta.WorkflowPhase != types.PhasePlan {
		t.Errorf("Expected PhasePlan, got %s", meta.WorkflowPhase)
	}
}

func TestManager_loadSessionMetadata(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save with messages — session.json will contain the full Session
	// including the Messages slice (serialized by json.Marshal in SaveSession).
	s.Messages = append(s.Messages, types.Message{Role: "user", Content: "hi"})
	s.MessageCount = 1
	if err := mgr.SaveSession(s); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	meta, err := mgr.loadSessionMetadata()
	if err != nil {
		t.Fatalf("loadSessionMetadata failed: %v", err)
	}
	if meta.ID != s.ID {
		t.Errorf("Expected ID %s, got %s", s.ID, meta.ID)
	}
	if meta.MessageCount != 1 {
		t.Errorf("Expected MessageCount 1, got %d", meta.MessageCount)
	}
	// loadSessionMetadata deserializes session.json which embeds the full
	// Session struct — so Messages are present from the JSON payload.
	if len(meta.Messages) != 1 {
		t.Errorf("Expected 1 message from session.json, got %d", len(meta.Messages))
	}
}

func TestManager_loadSessionMetadata_InvalidID(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, err := mgr.loadSessionMetadata()
	if err == nil {
		t.Error("Expected error for invalid session ID")
	}
}

func TestManager_loadSessionMetadata_Missing(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, err := mgr.loadSessionMetadata()
	if err == nil {
		t.Error("Expected error for missing session")
	}
}

func TestManager_loadSessionMetadata_CorruptJSON(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessionDir := filepath.Join(dir, "aabbccdd")
	os.MkdirAll(sessionDir, 0755)
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte("{corrupt"), 0644)

	_, err := mgr.loadSessionMetadata()
	if err == nil {
		t.Error("Expected error for corrupt session JSON")
	}
}

func TestManager_loadSessionMetadata_MissingID(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessionDir := filepath.Join(dir, "aabbccdd")
	os.MkdirAll(sessionDir, 0755)
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte(`{"model":"gpt-4o"}`), 0644)

	_, err := mgr.loadSessionMetadata()
	if err == nil {
		t.Error("Expected error for session missing ID field")
	}
}

func TestManager_loadSessionMetadata_ZeroStartedAt(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessionDir := filepath.Join(dir, "aabbccdd")
	os.MkdirAll(sessionDir, 0755)
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte(`{"id":"aabbccdd","started_at":"0001-01-01T00:00:00Z"}`), 0644)

	_, err := mgr.loadSessionMetadata()
	if err == nil {
		t.Error("Expected error for session with zero StartedAt")
	}
}

func TestManager_loadSessionMetadata_UnknownPhase(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessionDir := filepath.Join(dir, "aabbccdd")
	os.MkdirAll(sessionDir, 0755)
	now := time.Now().UTC().Format(time.RFC3339)
	sessionJSON := `{"id":"aabbccdd","started_at":"` + now + `","workflow_phase":"bogus"}`
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte(sessionJSON), 0644)

	_, err := mgr.loadSessionMetadata()
	if err == nil {
		t.Error("Expected error for unknown WorkflowPhase")
	}
}

// ---------------------------------------------------------------------------
// LoadWorkflowState with invalid ID
// ---------------------------------------------------------------------------

func TestManager_LoadWorkflowState_InvalidID(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	goal, phase, questions, err := mgr.LoadWorkflowState("INVALID!")
	if err != nil {
		t.Fatalf("Expected nil error for invalid ID, got %v", err)
	}
	if goal != "" {
		t.Errorf("Expected empty goal, got %q", goal)
	}
	if phase != types.PhaseIdle {
		t.Errorf("Expected PhaseIdle, got %s", phase)
	}
	if questions != nil {
		t.Errorf("Expected nil questions, got %v", questions)
	}
}

// ---------------------------------------------------------------------------
// ListSessions cache behavior
// ---------------------------------------------------------------------------

func TestManager_ListSessions_CacheReuse(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, _ = mgr.NewSession("gpt-4o", "openrouter")

	sessions1, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions 1 failed: %v", err)
	}

	sessions2, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions 2 failed: %v", err)
	}

	if len(sessions1) != len(sessions2) {
		t.Errorf("Cache mismatch: %d vs %d", len(sessions1), len(sessions2))
	}
}

// ---------------------------------------------------------------------------
// NewSession creates planning directory
// ---------------------------------------------------------------------------

func TestManager_NewSession_CreatesPlanningDir(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := mgr.planningDirPath()
	if _, err := os.Stat(planningDir); os.IsNotExist(err) {
		t.Error("NewSession should create planning directory")
	}
}

// ---------------------------------------------------------------------------
// ExportSessionMarkdown
// ---------------------------------------------------------------------------

func TestManager_ExportSessionMarkdown(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}
	s.Messages = append(s.Messages,
		types.Message{Role: "user", Content: "Hello"},
		types.Message{Role: "assistant", Content: "Hi there!"},
	)
	s.MessageCount = 2
	if err := mgr.SaveSession(s); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	exportPath := filepath.Join(t.TempDir(), "export.md")
	if err := mgr.ExportSessionMarkdown(s.ID, exportPath); err != nil {
		t.Fatalf("ExportSessionMarkdown failed: %v", err)
	}

	data, err := os.ReadFile(exportPath)
	if err != nil {
		t.Fatalf("Failed to read export file: %v", err)
	}

	content := string(data)
	if !strings.Contains(content, "# Session: "+s.ID) {
		t.Error("Expected session ID in header")
	}
	if !strings.Contains(content, "**Model:** gpt-4o") {
		t.Error("Expected model in header")
	}
	if !strings.Contains(content, "**Provider:** openrouter") {
		t.Error("Expected provider in header")
	}
	if !strings.Contains(content, "## user") {
		t.Error("Expected user message role")
	}
	if !strings.Contains(content, "Hello") {
		t.Error("Expected user message content")
	}
	if !strings.Contains(content, "## assistant") {
		t.Error("Expected assistant message role")
	}
	if !strings.Contains(content, "Hi there!") {
		t.Error("Expected assistant message content")
	}
}

func TestManager_ExportSessionMarkdown_NonexistentSession(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	exportPath := filepath.Join(t.TempDir(), "export.md")
	err := mgr.ExportSessionMarkdown("nonexistent", exportPath)
	if err == nil {
		t.Error("Expected error for nonexistent session")
	}
}

// ---------------------------------------------------------------------------
// LoadWorkflowState error paths
// ---------------------------------------------------------------------------

func TestManager_LoadWorkflowState_CorruptSession(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Create corrupt session
	sessionDir := filepath.Join(dir, "aabbccdd")
	os.MkdirAll(sessionDir, 0755)
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte("{corrupt"), 0644)

	goal, phase, questions, err := mgr.LoadWorkflowState("aabbccdd")
	if err != nil {
		t.Fatalf("Expected nil error for corrupt session, got %v", err)
	}
	if goal != "" {
		t.Errorf("Expected empty goal, got %q", goal)
	}
	if phase != types.PhaseIdle {
		t.Errorf("Expected PhaseIdle, got %s", phase)
	}
	if questions != nil {
		t.Errorf("Expected nil questions, got %v", questions)
	}
}

// ---------------------------------------------------------------------------
// LoadSession permission error (simulated via read-only dir)
// ---------------------------------------------------------------------------

func TestManager_LoadSession_PermissionDenied(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessionDir := filepath.Join(dir, "aabbccdd")
	os.MkdirAll(sessionDir, 0755)
	os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte(`{"id":"aabbccdd"}`), 0644)
	os.Chmod(sessionDir, 0000)
	defer os.Chmod(sessionDir, 0755)

	_, err := mgr.LoadSession("aabbccdd")
	// On some systems (root), permission errors may not occur
	if err != nil {
		if !errors.Is(err, m31errors.ErrSessionPermission) {
			// If we get a different error that's ok for root
			t.Logf("Got error (may be root): %v", err)
		}
	}
}

// ---------------------------------------------------------------------------
// saveSessionAtomic writes valid JSON
// ---------------------------------------------------------------------------

func TestManager_saveSessionAtomic_ProducesValidJSON(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	s.Label = "test-label"
	s.WorkflowGoal = "test-goal"
	s.DiscussQuestions = []string{"Q1", "Q2"}
	s.MessageCount = 5

	if err := mgr.saveSessionAtomic(s); err != nil {
		t.Fatalf("saveSessionAtomic failed: %v", err)
	}

	// Verify the file is valid JSON
	path := mgr.sessionJSONPath()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("Failed to read session.json: %v", err)
	}

	var decoded Session
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("session.json is not valid JSON: %v", err)
	}
	if decoded.Label != "test-label" {
		t.Errorf("Expected label 'test-label', got %q", decoded.Label)
	}
	if decoded.WorkflowGoal != "test-goal" {
		t.Errorf("Expected workflow_goal 'test-goal', got %q", decoded.WorkflowGoal)
	}
}

// ---------------------------------------------------------------------------
// SaveSession marshals messages correctly
// ---------------------------------------------------------------------------

func TestManager_SaveSession_NilMessagesBecomesEmptySlice(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	s.Messages = nil
	if err := mgr.SaveSession(s); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	// Read messages.json directly
	msgPath := mgr.messagesJSONPath()
	data, err := os.ReadFile(msgPath)
	if err != nil {
		t.Fatalf("Failed to read messages.json: %v", err)
	}
	var messages []types.Message
	if err := json.Unmarshal(data, &messages); err != nil {
		t.Fatalf("messages.json is not valid JSON: %v", err)
	}
	if messages == nil {
		t.Error("Expected non-nil messages slice after save with nil messages")
	}
	if len(messages) != 0 {
		t.Errorf("Expected 0 messages, got %d", len(messages))
	}
}

// ---------------------------------------------------------------------------
// LoadCheckpoints archived path fallback
// ---------------------------------------------------------------------------

func TestManager_LoadCheckpoints_FallsBackToArchived(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_LoadCheckpoints_ArchivedPathAlsoMissing(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

// ---------------------------------------------------------------------------
// LoadCheckpoints corrupt JSON
// ---------------------------------------------------------------------------

func TestManager_LoadCheckpoints_CorruptJSON(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Write corrupt checkpoint.json
	path := filepath.Join(mgr.projectDir(), "checkpoint.json")
	os.WriteFile(path, []byte("{corrupt"), 0644)

	_, err = mgr.LoadCheckpoints(s.ID)
	if err == nil {
		t.Error("Expected error for corrupt checkpoint JSON")
	}
}

// ---------------------------------------------------------------------------
// ListChildren with nonexistent parent
// ---------------------------------------------------------------------------

func TestManager_ListChildren_NonexistentParent(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

// ---------------------------------------------------------------------------
// SiblingSessions with nonexistent session
// ---------------------------------------------------------------------------

func TestManager_SiblingSessions_NonexistentSession(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

// ---------------------------------------------------------------------------
// ArchiveSession nonexistent
// ---------------------------------------------------------------------------

func TestManager_ArchiveSession_Nonexistent(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

// ---------------------------------------------------------------------------
// ListSessions with file entries (non-directory)
// ---------------------------------------------------------------------------

func TestManager_ListSessions_SkipsFiles(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Create a regular file in the base dir
	os.WriteFile(filepath.Join(dir, "random-file.txt"), []byte("data"), 0644)

	// Also create a valid session
	_, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}
	if len(sessions) != 1 {
		t.Errorf("Expected 1 session (file skipped), got %d", len(sessions))
	}
}
