package codeintel

import (
	"testing"
)

func makeTestFiles() []*FileInfo {
	return []*FileInfo{
		{
			Path: "engine.go", Language: "go",
			Imports: []ImportInfo{{Path: "types.go"}},
			Exports: []SymbolInfo{
				{Name: "Engine", Kind: "struct", Exported: true},
				{Name: "NewEngine", Kind: "func", Exported: true},
			},
			Funcs: []FuncSignature{
				{Name: "NewEngine", Exported: true, Returns: "*Engine"},
				{Name: "Run", Receiver: "*Engine", Exported: true, Params: "ctx context.Context"},
			},
			Types: []TypeInfo{
				{Name: "Engine", Kind: "struct", Fields: []string{"Name", "Config"}},
			},
		},
		{
			Path: "types.go", Language: "go",
			Exports: []SymbolInfo{
				{Name: "Config", Kind: "struct", Exported: true},
				{Name: "Status", Kind: "type", Exported: true},
			},
			Types: []TypeInfo{
				{Name: "Config", Kind: "struct", Fields: []string{"Name"}},
				{Name: "Status", Kind: "type"},
			},
		},
		{
			Path: "handler.go", Language: "go",
			Imports: []ImportInfo{{Path: "engine.go"}, {Path: "types.go"}},
			Exports: []SymbolInfo{
				{Name: "Handler", Kind: "interface", Exported: true},
			},
			Types: []TypeInfo{
				{Name: "Handler", Kind: "interface", Methods: []string{"Handle"}},
			},
		},
	}
}

func TestSymbolIndex_AddAndQuery(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	if idx.FileCount() != 3 {
		t.Errorf("FileCount: got %d, want 3", idx.FileCount())
	}

	locs := idx.Define("Engine")
	if len(locs) != 1 {
		t.Fatalf("Define(Engine): got %d locations, want 1", len(locs))
	}
	if locs[0].File != "engine.go" {
		t.Errorf("Engine defined in %q, want engine.go", locs[0].File)
	}
	if locs[0].Kind != "struct" {
		t.Errorf("Engine kind: got %q, want struct", locs[0].Kind)
	}
}

func TestSymbolIndex_DefineInFile(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	loc := idx.DefineInFile("Config", "types.go")
	if loc == nil {
		t.Fatal("expected Config to be defined in types.go")
	}
	if loc.Kind != "struct" {
		t.Errorf("Config kind: got %q, want struct", loc.Kind)
	}

	loc = idx.DefineInFile("Config", "engine.go")
	if loc != nil {
		t.Error("Config should not be in engine.go")
	}
}

func TestSymbolIndex_FileSymbols(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	syms := idx.FileSymbols("types.go")
	if len(syms) < 2 {
		t.Errorf("types.go symbols: got %d, want at least 2", len(syms))
	}

	names := make(map[string]bool)
	for _, s := range syms {
		names[s.Name] = true
	}
	if !names["Config"] {
		t.Error("missing Config in types.go symbols")
	}
	if !names["Status"] {
		t.Error("missing Status in types.go symbols")
	}
}

func TestSymbolIndex_FindTypes(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	types := idx.FindTypes("Engine")
	if len(types) != 1 {
		t.Errorf("FindTypes(Engine): got %d, want 1", len(types))
	}

	types = idx.FindTypes("Handler")
	if len(types) != 1 {
		t.Errorf("FindTypes(Handler): got %d, want 1", len(types))
	}
	if types[0].Kind != "interface" {
		t.Errorf("Handler kind: got %q, want interface", types[0].Kind)
	}
}

func TestSymbolIndex_FindFuncs(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	funcs := idx.FindFuncs("NewEngine")
	if len(funcs) != 1 {
		t.Errorf("FindFuncs(NewEngine): got %d, want 1", len(funcs))
	}
	if funcs[0].File != "engine.go" {
		t.Errorf("NewEngine in %q, want engine.go", funcs[0].File)
	}
}

func TestSymbolIndex_SymbolsMatching(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	matches := idx.SymbolsMatching("engi")
	if len(matches) == 0 {
		t.Error("expected matches for 'engi'")
	}
	found := false
	for _, m := range matches {
		if m == "Engine" {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected Engine in matches, got %v", matches)
	}
}

func TestSymbolIndex_NonExistent(t *testing.T) {
	files := makeTestFiles()
	idx := BuildIndex(files)

	locs := idx.Define("NonExistent")
	if len(locs) != 0 {
		t.Error("expected no locations for NonExistent")
	}

	syms := idx.FileSymbols("nonexistent.go")
	if len(syms) != 0 {
		t.Error("expected no symbols for nonexistent file")
	}
}
