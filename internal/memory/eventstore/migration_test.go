package eventstore

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestMigrateRequirements(t *testing.T) {
	dir := t.TempDir()
	planningDir := filepath.Join(dir, ".planning")
	m31aDir := filepath.Join(dir, ".m31a")

	if err := os.MkdirAll(planningDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Create test REQUIREMENTS.md
	reqContent := `# Requirements

## Phase 1

- [x] REQ-01: First requirement
- [ ] REQ-02: Second requirement
  Description for REQ-02
- [/] REQ-03: Active requirement
`
	if err := os.WriteFile(filepath.Join(planningDir, "REQUIREMENTS.md"), []byte(reqContent), 0644); err != nil {
		t.Fatal(err)
	}

	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	require.NoError(t, err)
	defer store.Close()

	ctx := context.Background()
	count, err := migrateRequirements(ctx, store, planningDir, m31aDir)
	require.NoError(t, err)
	assert.Equal(t, 3, count)

	// Verify events in store
	events, err := store.Query(ctx, types.Query{Type: func() *types.EventType { t := types.EventRequirementCreated; return &t }()})
	require.NoError(t, err)
	assert.Len(t, events, 3)

	// Verify projection written
	_, err = os.Stat(filepath.Join(m31aDir, "requirements.md"))
	require.NoError(t, err)
}

func TestMigrateRoadmap(t *testing.T) {
	dir := t.TempDir()
	planningDir := filepath.Join(dir, ".planning")
	m31aDir := filepath.Join(dir, ".m31a")

	if err := os.MkdirAll(planningDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	roadmapContent := `# Roadmap

## Phase 1: Foundation
**Goal:** Establish core infrastructure

  - REQ-01
  - REQ-02

## Phase 2: Core Features
**Goal:** Build core features
`
	if err := os.WriteFile(filepath.Join(planningDir, "ROADMAP.md"), []byte(roadmapContent), 0644); err != nil {
		t.Fatal(err)
	}

	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	require.NoError(t, err)
	defer store.Close()

	ctx := context.Background()
	count, err := migrateRoadmap(ctx, store, planningDir, m31aDir)
	require.NoError(t, err)
	assert.GreaterOrEqual(t, count, 1)

	_, err = os.Stat(filepath.Join(m31aDir, "roadmap.md"))
	require.NoError(t, err)
}

func TestMigrateDecisions(t *testing.T) {
	dir := t.TempDir()
	planningDir := filepath.Join(dir, ".planning")
	m31aDir := filepath.Join(dir, ".m31a")

	decisionsDir := filepath.Join(planningDir, "decisions")
	if err := os.MkdirAll(decisionsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	decisionContent := `# Decision: Use SQLite

**Status:** accepted

**Timestamp:** 2026-08-24T10:00:00Z

## Rationale
SQLite is embedded, no separate server needed.
`
	if err := os.WriteFile(filepath.Join(decisionsDir, "abc12345-use-sqlite.md"), []byte(decisionContent), 0644); err != nil {
		t.Fatal(err)
	}

	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	require.NoError(t, err)
	defer store.Close()

	ctx := context.Background()
	count, err := migrateDecisions(ctx, store, planningDir, m31aDir)
	require.NoError(t, err)
	assert.Equal(t, 1, count)

	// Verify projection written
	files, err := os.ReadDir(filepath.Join(m31aDir, "decisions"))
	require.NoError(t, err)
	assert.Len(t, files, 1)
}

func TestMigrateResearch(t *testing.T) {
	dir := t.TempDir()
	planningDir := filepath.Join(dir, ".planning")
	m31aDir := filepath.Join(dir, ".m31a")

	researchDir := filepath.Join(planningDir, "research")
	if err := os.MkdirAll(researchDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	researchContent := `# Research: How to implement X?

**Confidence:** 0.90

**Created:** 2026-08-24T10:00:00Z

**Created By:** 00000000-0000-0000-0000-000000000000

## Sources
- source1
- source2

## Findings
Found that Y works best because Z.
`
	if err := os.WriteFile(filepath.Join(researchDir, "def45678-research-x.md"), []byte(researchContent), 0644); err != nil {
		t.Fatal(err)
	}

	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	require.NoError(t, err)
	defer store.Close()

	ctx := context.Background()
	count, err := migrateResearch(ctx, store, planningDir, m31aDir)
	require.NoError(t, err)
	assert.Equal(t, 1, count)

	files, err := os.ReadDir(filepath.Join(m31aDir, "research"))
	require.NoError(t, err)
	assert.Len(t, files, 1)
}

func TestMigrateCodebase(t *testing.T) {
	dir := t.TempDir()
	planningDir := filepath.Join(dir, ".planning")
	m31aDir := filepath.Join(dir, ".m31a")

	codebaseDir := filepath.Join(planningDir, "codebase")
	if err := os.MkdirAll(codebaseDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	if err := os.WriteFile(filepath.Join(codebaseDir, "analysis.md"), []byte("# Analysis\n\nContent"), 0644); err != nil {
		t.Fatal(err)
	}

	store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
	require.NoError(t, err)
	defer store.Close()

	ctx := context.Background()
	count, err := migrateCodebase(ctx, store, planningDir, m31aDir)
	require.NoError(t, err)
	assert.Equal(t, 1, count)

	// Verify projection written
	_, err = os.Stat(filepath.Join(m31aDir, "codebase", "analysis.md"))
	require.NoError(t, err)
}

func TestArchivePlanning(t *testing.T) {
	dir := t.TempDir()
	planningDir := filepath.Join(dir, ".planning")
	m31aDir := filepath.Join(dir, ".m31a")

	if err := os.MkdirAll(planningDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Create some files
	if err := os.WriteFile(filepath.Join(planningDir, "test.txt"), []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}

	err := archivePlanning(planningDir)
	require.NoError(t, err)

	// Original should be gone
	_, err = os.Stat(planningDir)
	assert.True(t, errors.Is(err, os.ErrNotExist))

	// Archive should exist
	entries, err := os.ReadDir(dir)
	require.NoError(t, err)
	found := false
	for _, entry := range entries {
		if entry.Name() == ".planning.archived" || len(entry.Name()) > len(".planning.archived.") && entry.Name()[:len(".planning.archived.")] == ".planning.archived." {
			found = true
			break
		}
	}
	assert.True(t, found, "archive directory not found")
}

func TestMigrateFull(t *testing.T) {
	// This test requires the actual .planning/ directory
	planningDir := ".planning"
	if _, err := os.Stat(planningDir); errors.Is(err, os.ErrNotExist) {
		t.Skip("Actual .planning/ directory not found, skipping full migration test")
	}

	dir := t.TempDir()
	m31aDir := filepath.Join(dir, ".m31a")

	if err := os.MkdirAll(m31aDir, 0755); err != nil {
		t.Fatal(err)
	}

	counts, err := Migrate(context.Background(), planningDir, m31aDir)
	require.NoError(t, err)

	t.Logf("Migrated: %+v", counts)
	assert.Greater(t, counts.Requirements, 0)
	assert.Greater(t, counts.Phases, 0)
	assert.GreaterOrEqual(t, counts.EventsTotal, counts.Requirements+counts.Phases)

	// Verify key projections exist
	projections := []string{"project.md", "requirements.md", "roadmap.md", "config.toml"}
	for _, p := range projections {
		_, err := os.Stat(filepath.Join(m31aDir, p))
		require.NoError(t, err, "projection %s should exist", p)
	}

	// Verify archive exists
	entries, err := os.ReadDir(dir)
	require.NoError(t, err)
	found := false
	for _, entry := range entries {
		if len(entry.Name()) > len(".planning.archived.") && entry.Name()[:len(".planning.archived.")] == ".planning.archived." {
			found = true
			break
		}
	}
	assert.True(t, found, "archive directory not found")

	// Verify events in store
	eventsDB := filepath.Join(m31aDir, "events.db")
	store, err := NewEventStore(eventsDB)
	require.NoError(t, err)
	defer store.Close()

	// Verify we have events
	events, err := store.Query(context.Background(), types.Query{})
	require.NoError(t, err)
	assert.Greater(t, len(events), 0)

	// Verify MigrationStarted and MigrationCompleted events
	migrationEvents := 0
	for _, evt := range events {
		if evt.Type == types.EventMigrationStarted || evt.Type == types.EventMigrationCompleted {
			migrationEvents++
		}
	}
	assert.Equal(t, 2, migrationEvents, "should have MigrationStarted and MigrationCompleted events")
}