package explain

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/git"
)

// seedRepoWithADR creates a temp git repo like seedRepo but adds an ADR file
// in .m31a/decisions/ for testing ADR scanning.
func seedRepoWithADR(t *testing.T) string {
	dir := seedRepo(t)

	// Add .m31a/decisions/ directory with an ADR
	decisionsDir := filepath.Join(dir, ".m31a", "decisions")
	if err := os.MkdirAll(decisionsDir, 0755); err != nil {
		t.Fatalf("mkdir decisions: %v", err)
	}
	adrContent := `# ADR 001: Use Bar for Everything

This decision records that Bar is the canonical answer function.

## Context
We needed a function that returns 42.

## Decision
Use Bar.

## Consequences
All code must call Bar.`
	adrPath := filepath.Join(decisionsDir, "001-use-bar.md")
	if err := os.WriteFile(adrPath, []byte(adrContent), 0644); err != nil {
		t.Fatalf("write ADR: %v", err)
	}

	g := git.New(dir)
	if err := g.Add(".m31a/decisions/001-use-bar.md"); err != nil {
		t.Fatalf("add ADR: %v", err)
	}
	if _, err := g.CommitStaged("add ADR for Bar"); err != nil {
		t.Fatalf("commit ADR: %v", err)
	}

	return dir
}

func TestFileMode_IntroductionCommit(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack, err := Collect(context.Background(), deps, "bar.go", ModeFile, "test-model")
	if err != nil {
		t.Fatalf("Collect failed: %v", err)
	}

	// Find commit-kind evidence with introduction commit
	var commitRef string
	for _, s := range pack.Sections {
		if s.Kind == types.EvidenceCommit {
			commitRef = s.Ref
			break
		}
	}
	if commitRef == "" {
		t.Fatal("expected commit-kind evidence for introduction commit")
	}

	// The introduction commit should be the first commit (commit-1 SHA)
	// We need to verify it matches the first commit that touched bar.go
	// In seedRepo, commit 1 adds bar.go, so the introduction commit is commit 1
	g := git.New(dir)
	log, err := g.LogAll()
	if err != nil {
		t.Fatalf("log: %v", err)
	}
	if len(log) < 1 {
		t.Fatal("expected at least one commit")
	}
	firstCommitSHA := log[len(log)-1].Hash // LogAll returns newest first

	if commitRef != firstCommitSHA {
		t.Errorf("introduction commit Ref = %q, want first commit SHA %q", commitRef, firstCommitSHA)
	}

	// Verify the snippet contains the subject line
	for _, s := range pack.Sections {
		if s.Kind == types.EvidenceCommit && s.Ref == commitRef {
			if !strings.Contains(s.Snippet, "add Bar function") {
				t.Errorf("commit snippet missing subject: %q", s.Snippet)
			}
		}
	}
}

func TestFileMode_ConsumersAndRemovalImpact(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack, err := Collect(context.Background(), deps, "bar.go", ModeFile, "test-model")
	if err != nil {
		t.Fatalf("Collect failed: %v", err)
	}

	// Should have callers/consumers evidence
	hasCallers := false
	for _, s := range pack.Sections {
		if s.Kind == types.EvidenceCallers {
			hasCallers = true
			if !strings.Contains(s.Ref, "caller.go") {
				t.Errorf("callers ref should reference caller.go: %q", s.Ref)
			}
		}
	}
	if !hasCallers {
		t.Error("expected callers/consumers evidence in file mode")
	}

	// Should have test evidence (removal impact)
	hasTests := false
	for _, s := range pack.Sections {
		if s.Kind == types.EvidenceTest {
			hasTests = true
		}
	}
	if !hasTests {
		t.Error("expected test evidence for removal impact")
	}
}

func TestFileMode_Deterministic(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack1, err := Collect(context.Background(), deps, "bar.go", ModeFile, "test-model")
	if err != nil {
		t.Fatalf("first Collect failed: %v", err)
	}
	pack2, err := Collect(context.Background(), deps, "bar.go", ModeFile, "test-model")
	if err != nil {
		t.Fatalf("second Collect failed: %v", err)
	}

	if !reflect.DeepEqual(pack1.Sections, pack2.Sections) {
		t.Fatalf("identical inputs produced different Sections:\nfirst:  %+v\nsecond: %+v", pack1.Sections, pack2.Sections)
	}
}

func TestTopicMode_ProducesSourceSections(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack, err := Collect(context.Background(), deps, "Bar function", ModeTopic, "test-model")
	if err != nil {
		t.Fatalf("Collect failed: %v", err)
	}

	// Topic mode should produce at least one source-kind section
	hasSource := false
	for _, s := range pack.Sections {
		if s.Kind == types.EvidenceSource {
			hasSource = true
			if !strings.Contains(s.Snippet, "Bar") {
				t.Errorf("source snippet should mention Bar: %q", s.Snippet)
			}
		}
	}
	if !hasSource {
		t.Error("expected at least one source-kind section in topic mode")
	}
}

func TestScanADRs_Present(t *testing.T) {
	dir := seedRepoWithADR(t)

	adrs := ScanADRs(dir, "Bar")
	if len(adrs) == 0 {
		t.Fatal("expected at least one ADR for query 'Bar'")
	}

	for _, adr := range adrs {
		if adr.Title == "" {
			t.Error("ADR title should not be empty")
		}
		if adr.Path == "" {
			t.Error("ADR path should not be empty")
		}
		if adr.Snippet == "" {
			t.Error("ADR snippet should not be empty")
		}
		// Snippet should be capped at first heading + first paragraph
		if strings.Count(adr.Snippet, "\n") > 5 {
			t.Errorf("ADR snippet too long (should be capped): %q", adr.Snippet)
		}
	}
}

func TestScanADRs_Absent(t *testing.T) {
	dir := seedRepo(t) // no ADR directory

	adrs := ScanADRs(dir, "Bar")
	if len(adrs) != 0 {
		t.Errorf("expected zero ADRs when directory absent, got %d", len(adrs))
	}
}

func TestResolveMode_FileExists(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	mode := ResolveMode("bar.go", dir, deps.Index)
	if mode != ModeFile {
		t.Errorf("ResolveMode('bar.go') = %q, want %q", mode, ModeFile)
	}
}

func TestResolveMode_ExactSymbol(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	mode := ResolveMode("Bar", dir, deps.Index)
	if mode != ModeSymbol {
		t.Errorf("ResolveMode('Bar') = %q, want %q", mode, ModeSymbol)
	}
}

func TestResolveMode_TopicFallback(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	mode := ResolveMode("auth middleware", dir, deps.Index)
	if mode != ModeTopic {
		t.Errorf("ResolveMode('auth middleware') = %q, want %q", mode, ModeTopic)
	}
}