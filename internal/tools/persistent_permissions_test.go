package tools

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/types"
)

func setupTestPersistentPermissions(t *testing.T) (*PersistentPermissions, string) {
	t.Helper()
	dir := t.TempDir()
	pp := &PersistentPermissions{path: filepath.Join(dir, "permissions.json")}
	return pp, dir
}

func TestPersistentPermissions_SaveAndLoad(t *testing.T) {
	pp, _ := setupTestPersistentPermissions(t)
	projectDir := "/test/project"

	rules := []config.PermissionRule{
		{Tool: "bash", Pattern: "ls", Action: "allow", RiskLevel: types.RiskSafe},
		{Tool: "git", Pattern: "status", Action: "ask", RiskLevel: types.RiskSafe},
	}

	// Save rules
	if err := pp.Save(projectDir, rules); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Load rules
	loaded := pp.Load(projectDir)
	if len(loaded) != 2 {
		t.Fatalf("expected 2 rules, got %d", len(loaded))
	}
	if loaded[0].Tool != "bash" || loaded[0].Action != "allow" {
		t.Errorf("rule 0 mismatch: got %+v", loaded[0])
	}
	if loaded[1].Tool != "git" || loaded[1].Action != "ask" {
		t.Errorf("rule 1 mismatch: got %+v", loaded[1])
	}
}

func TestPersistentPermissions_MultipleProjects(t *testing.T) {
	pp, _ := setupTestPersistentPermissions(t)

	// Save rules for project A
	rulesA := []config.PermissionRule{
		{Tool: "bash", Pattern: "ls", Action: "allow", RiskLevel: types.RiskSafe},
	}
	if err := pp.Save("/project/a", rulesA); err != nil {
		t.Fatalf("Save project A failed: %v", err)
	}

	// Save rules for project B
	rulesB := []config.PermissionRule{
		{Tool: "git", Pattern: "status", Action: "ask", RiskLevel: types.RiskSafe},
	}
	if err := pp.Save("/project/b", rulesB); err != nil {
		t.Fatalf("Save project B failed: %v", err)
	}

	// Load project A
	loadedA := pp.Load("/project/a")
	if len(loadedA) != 1 || loadedA[0].Tool != "bash" {
		t.Errorf("project A rules mismatch: got %+v", loadedA)
	}

	// Load project B
	loadedB := pp.Load("/project/b")
	if len(loadedB) != 1 || loadedB[0].Tool != "git" {
		t.Errorf("project B rules mismatch: got %+v", loadedB)
	}
}

func TestPersistentPermissions_CorruptJSON(t *testing.T) {
	pp, _ := setupTestPersistentPermissions(t)

	// Write corrupt JSON
	if err := os.WriteFile(pp.path, []byte("not json"), 0644); err != nil {
		t.Fatal(err)
	}

	// Load should return nil for corrupt data
	loaded := pp.Load("/test/project")
	if loaded != nil {
		t.Errorf("expected nil for corrupt JSON, got %+v", loaded)
	}

	// Save should create a backup and return error
	err := pp.Save("/test/project", []config.PermissionRule{
		{Tool: "bash", Pattern: "ls", Action: "allow", RiskLevel: types.RiskSafe},
	})
	if err == nil {
		t.Error("expected error for corrupt JSON save")
	}

	// Verify backup was created
	dir := filepath.Dir(pp.path)
	entries, _ := os.ReadDir(dir)
	backupFound := false
	for _, e := range entries {
		if e.Name() != "permissions.json" && len(e.Name()) > len("permissions.json") {
			backupFound = true
			break
		}
	}
	if !backupFound {
		t.Error("expected backup file to be created")
	}
}

func TestPersistentPermissions_EmptyPath(t *testing.T) {
	pp := &PersistentPermissions{path: ""}

	// Load with empty path should return nil
	loaded := pp.Load("/test/project")
	if loaded != nil {
		t.Errorf("expected nil for empty path, got %+v", loaded)
	}

	// Save with empty path should not error
	if err := pp.Save("/test/project", nil); err != nil {
		t.Errorf("expected no error for empty path save, got: %v", err)
	}
}

func TestPersistentPermissions_NilRules(t *testing.T) {
	pp, _ := setupTestPersistentPermissions(t)
	projectDir := "/test/project"

	// Save nil rules
	if err := pp.Save(projectDir, nil); err != nil {
		t.Fatalf("Save nil rules failed: %v", err)
	}

	// Load should return empty slice
	loaded := pp.Load(projectDir)
	if loaded != nil {
		t.Errorf("expected nil/empty for nil rules save, got %+v", loaded)
	}
}

func TestPersistentPermissions_UpdateRules(t *testing.T) {
	pp, _ := setupTestPersistentPermissions(t)
	projectDir := "/test/project"

	// Save initial rules
	rules1 := []config.PermissionRule{
		{Tool: "bash", Pattern: "ls", Action: "allow", RiskLevel: types.RiskSafe},
	}
	if err := pp.Save(projectDir, rules1); err != nil {
		t.Fatal(err)
	}

	// Update rules
	rules2 := []config.PermissionRule{
		{Tool: "bash", Pattern: "ls", Action: "deny", RiskLevel: types.RiskSafe},
		{Tool: "git", Pattern: "status", Action: "ask", RiskLevel: types.RiskSafe},
	}
	if err := pp.Save(projectDir, rules2); err != nil {
		t.Fatal(err)
	}

	// Load should return updated rules
	loaded := pp.Load(projectDir)
	if len(loaded) != 2 {
		t.Fatalf("expected 2 rules, got %d", len(loaded))
	}
	if loaded[0].Action != "deny" {
		t.Errorf("expected deny, got %s", loaded[0].Action)
	}
}
