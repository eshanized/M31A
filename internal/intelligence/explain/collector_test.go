package explain

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/eshanized/M31A/internal/integrations/git"
)

// seedRepo creates a temp git repo mirroring the plan fixture: commit 1 adds
// a small Go file defining function Bar; commit 2 (different author) adds a
// caller file plus a same-directory test file so every evidence kind has a
// real source to come from.
func seedRepo(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()

	g := git.New(dir)
	if err := g.Init(); err != nil {
		t.Fatalf("git init: %v", err)
	}
	if err := g.ConfigUser("Seed Author", "seed@example.com"); err != nil {
		t.Fatalf("config user: %v", err)
	}

	barSrc := "package seed\n\n// Bar returns the answer.\nfunc Bar() int {\n\treturn 42\n}\n"
	if err := os.WriteFile(filepath.Join(dir, "bar.go"), []byte(barSrc), 0644); err != nil {
		t.Fatalf("write bar.go: %v", err)
	}
	if err := g.Add("bar.go"); err != nil {
		t.Fatalf("add bar.go: %v", err)
	}
	if _, err := g.CommitStaged("add Bar function"); err != nil {
		t.Fatalf("commit 1: %v", err)
	}

	if err := g.ConfigUser("Caller Author", "caller@example.com"); err != nil {
		t.Fatalf("config caller user: %v", err)
	}
	callerSrc := "package seed\n\n// CallBar invokes Bar.\nfunc CallBar() int {\n\treturn Bar()\n}\n"
	testSrc := "package seed\n\nimport \"testing\"\n\nfunc TestCallBar(t *testing.T) {\n\tif CallBar() != 42 {\n\t\tt.Fatal(\"unexpected value\")\n\t}\n}\n"
	if err := os.WriteFile(filepath.Join(dir, "caller.go"), []byte(callerSrc), 0644); err != nil {
		t.Fatalf("write caller.go: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, "caller_test.go"), []byte(testSrc), 0644); err != nil {
		t.Fatalf("write caller_test.go: %v", err)
	}
	if err := g.Add("caller.go", "caller_test.go"); err != nil {
		t.Fatalf("add caller files: %v", err)
	}
	if _, err := g.CommitStaged("add caller and test"); err != nil {
		t.Fatalf("commit 2: %v", err)
	}
	return dir
}

// buildDeps builds a code graph and symbol index over the seeded repo and
// returns ready collector deps (injected integrations per AnalyzeImpact
// orchestration discipline). BuildGraph fills only the import graph; call
// edges are assembled here from parsed call sites, mirroring how the
// production event-replay projection feeds AddCallEdge.
func buildDeps(t *testing.T, dir string, maxTotalTokens int) CollectorDeps {
	t.Helper()
	graph, files, err := codeintel.BuildGraph(dir, codeintel.AllParsers())
	if err != nil {
		t.Fatalf("build graph: %v", err)
	}
	for _, f := range files {
		for _, cs := range f.CallSites {
			graph.AddCallEdge(codeintel.CallEdge{
				CallerFile: f.Path,
				CallerLine: cs.Line,
				CallerName: cs.CallerName,
				CalleeName: cs.CalleeName,
			})
		}
	}
	return CollectorDeps{
		Graph:          graph,
		Index:          codeintel.BuildIndex(files),
		GitClient:      git.New(dir),
		WorkDir:        dir,
		MaxTotalTokens: maxTotalTokens,
	}
}

func kindsIn(pack *types.EvidencePack) map[types.EvidenceKind]int {
	out := make(map[types.EvidenceKind]int)
	for _, s := range pack.Sections {
		out[s.Kind]++
	}
	return out
}

var (
	fileLineRefRe = regexp.MustCompile(`^[^:]+\.go:\d+$`)
	shaRefRe      = regexp.MustCompile(`^[0-9a-f]{40}$`)
)

func TestCollectSymbol_AssemblesAllKinds(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack, err := Collect(context.Background(), deps, "Bar", ModeSymbol, "test-model")
	if err != nil {
		t.Fatalf("Collect failed: %v", err)
	}

	kinds := kindsIn(pack)
	for _, want := range []types.EvidenceKind{types.EvidenceSource, types.EvidenceCallers} {
		if kinds[want] == 0 {
			t.Errorf("pack missing %q kind sections; got %v", want, kinds)
		}
	}
	if len(kinds) < 3 {
		t.Errorf("expected at least three distinct kinds, got %d: %v", len(kinds), kinds)
	}

	for _, s := range pack.Sections {
		switch s.Kind {
		case types.EvidenceSource, types.EvidenceCallers:
			if !fileLineRefRe.MatchString(s.Ref) {
				t.Errorf("%s kind ref = %q, want file:line form", s.Kind, s.Ref)
			}
		case types.EvidenceBlame:
			if !shaRefRe.MatchString(s.Ref) {
				t.Errorf("blame kind ref = %q, want commit SHA form", s.Ref)
			}
		case types.EvidenceTest:
			if !strings.HasSuffix(s.Ref, "_test.go") {
				t.Errorf("test kind ref = %q, want test file path", s.Ref)
			}
		}
	}
}

func TestCollect_DeterministicOrder(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack1, err := Collect(context.Background(), deps, "Bar", ModeSymbol, "test-model")
	if err != nil {
		t.Fatalf("first Collect failed: %v", err)
	}
	pack2, err := Collect(context.Background(), deps, "Bar", ModeSymbol, "test-model")
	if err != nil {
		t.Fatalf("second Collect failed: %v", err)
	}

	if !reflect.DeepEqual(pack1.Sections, pack2.Sections) {
		t.Fatalf("identical inputs produced different Sections:\nfirst:  %+v\nsecond: %+v", pack1.Sections, pack2.Sections)
	}

	// Kind priority: all source entries precede callers, which precede blame,
	// which precede tests; insertion order preserved within each kind.
	lastPrio := -1
	for _, s := range pack1.Sections {
		prio, ok := kindPriority[s.Kind]
		if !ok {
			t.Fatalf("unknown kind priority for %q", s.Kind)
		}
		if prio < lastPrio {
			t.Errorf("kind %q appears after a lower-priority kind (priority %d after %d)", s.Kind, prio, lastPrio)
		}
		lastPrio = prio
	}
}

func TestCollect_BudgetTrimsTestsBeforeSource(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 10) // tiny cap forces whole-section drops

	pack, err := Collect(context.Background(), deps, "Bar", ModeSymbol, "test-model")
	if err != nil {
		t.Fatalf("Collect failed: %v", err)
	}

	if !pack.Truncated {
		t.Error("expected Truncated=true when budget forces drops")
	}
	kinds := kindsIn(pack)
	if kinds[types.EvidenceTest] != 0 {
		t.Errorf("test-kind sections survived tiny budget; got %d (must drop before any source-kind)", kinds[types.EvidenceTest])
	}
	if kinds[types.EvidenceSource] == 0 {
		t.Error("source-kind section was dropped while lower-priority kinds were still present")
	}
	// Snippets are never mid-truncated: every surviving snippet keeps content.
	for _, s := range pack.Sections {
		if s.Snippet == "" {
			t.Errorf("section id=%d kind=%s has empty snippet (mid-truncation?)", s.ID, s.Kind)
		}
	}
}

func TestCollect_UnknownSymbolReturnsSentinel(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack, err := Collect(context.Background(), deps, "DefinitelyNotASymbolQxz99", ModeSymbol, "test-model")
	if err == nil {
		t.Fatal("expected error for unknown symbol")
	}
	if !errors.Is(err, ErrTargetNotFound) {
		t.Fatalf("error does not wrap ErrTargetNotFound: %v", err)
	}
	if len(pack.Sections) != 0 {
		t.Errorf("expected zero sections for unknown symbol, got %d", len(pack.Sections))
	}
}

func TestDedupeSections_FirstWins(t *testing.T) {
	items := []types.Evidence{
		{ID: 1, Kind: types.EvidenceSource, Ref: "a.go:1", Snippet: "one"},
		{ID: 2, Kind: types.EvidenceCallers, Ref: "b.go:5", Snippet: "call"},
		{ID: 3, Kind: types.EvidenceSource, Ref: "a.go:1", Snippet: "duplicate"},
		{ID: 4, Kind: types.EvidenceBlame, Ref: "abc", Snippet: "touch"},
	}

	got := dedupeSections(items)
	if len(got) != 3 {
		t.Fatalf("expected 3 sections after dedupe, got %d", len(got))
	}
	if got[0].Snippet != "one" || got[0].ID != 1 {
		t.Errorf("duplicate did not collapse first-wins: %+v", got[0])
	}
	// IDs must be renumbered sequentially after collapse.
	for i, s := range got {
		if s.ID != i+1 {
			t.Errorf("section %d has ID %d, want sequential renumbering", i, s.ID)
		}
	}
}
