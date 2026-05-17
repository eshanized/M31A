package codeintel

import (
	"strings"
)

// SymbolLocation identifies where a symbol is defined.
type SymbolLocation struct {
	File string
	Kind string // "func", "type", "interface", "class", "struct", "const", "var"
}

// SymbolIndex maps symbol names to their definitions and files to their symbols.
type SymbolIndex struct {
	byName map[string][]SymbolLocation // symbol name → definition locations
	byFile map[string][]SymbolInfo      // file → exported symbols
}

// NewSymbolIndex creates an empty symbol index.
func NewSymbolIndex() *SymbolIndex {
	return &SymbolIndex{
		byName: make(map[string][]SymbolLocation),
		byFile: make(map[string][]SymbolInfo),
	}
}

// AddFile indexes all symbols from a parsed file.
func (idx *SymbolIndex) AddFile(info *FileInfo) {
	if info == nil {
		return
	}

	var symbols []SymbolInfo

	for _, s := range info.Exports {
		symbols = append(symbols, s)
		idx.byName[s.Name] = append(idx.byName[s.Name], SymbolLocation{
			File: info.Path,
			Kind: s.Kind,
		})
	}

	for _, f := range info.Funcs {
		alreadyExported := false
		for _, s := range symbols {
			if s.Name == f.Name {
				alreadyExported = true
				break
			}
		}
		if !alreadyExported {
			symbols = append(symbols, SymbolInfo{
				Name: f.Name, Kind: "func", Exported: f.Exported,
			})
			idx.byName[f.Name] = append(idx.byName[f.Name], SymbolLocation{
				File: info.Path, Kind: "func",
			})
		}
	}

	for _, ti := range info.Types {
		alreadyExported := false
		for _, s := range symbols {
			if s.Name == ti.Name {
				alreadyExported = true
				break
			}
		}
		if !alreadyExported {
			symbols = append(symbols, SymbolInfo{
				Name: ti.Name, Kind: ti.Kind, Exported: true,
			})
			idx.byName[ti.Name] = append(idx.byName[ti.Name], SymbolLocation{
				File: info.Path, Kind: ti.Kind,
			})
		}
	}

	idx.byFile[info.Path] = symbols
}

// Define returns all locations where a symbol is defined.
func (idx *SymbolIndex) Define(name string) []SymbolLocation {
	return idx.byName[name]
}

// DefineInFile returns the definition of a symbol in a specific file, or nil.
func (idx *SymbolIndex) DefineInFile(name, file string) *SymbolLocation {
	for _, loc := range idx.byName[name] {
		if loc.File == file {
			return &loc
		}
	}
	return nil
}

// FileSymbols returns all symbols defined in a file.
func (idx *SymbolIndex) FileSymbols(path string) []SymbolInfo {
	return idx.byFile[path]
}

// FindTypes returns all type/struct/interface/class/enum definitions matching the name.
func (idx *SymbolIndex) FindTypes(name string) []SymbolLocation {
	var results []SymbolLocation
	typeKinds := map[string]bool{
		"struct": true, "interface": true, "type": true,
		"class": true, "enum": true, "trait": true,
	}
	for _, loc := range idx.byName[name] {
		if typeKinds[loc.Kind] {
			results = append(results, loc)
		}
	}
	return results
}

// FindFuncs returns all function definitions matching the name.
func (idx *SymbolIndex) FindFuncs(name string) []SymbolLocation {
	var results []SymbolLocation
	for _, loc := range idx.byName[name] {
		if loc.Kind == "func" {
			results = append(results, loc)
		}
	}
	return results
}

// AllSymbols returns all unique symbol names in the index.
func (idx *SymbolIndex) AllSymbols() []string {
	names := make([]string, 0, len(idx.byName))
	for name := range idx.byName {
		names = append(names, name)
	}
	return names
}

// SymbolCount returns the number of unique symbols.
func (idx *SymbolIndex) SymbolCount() int {
	return len(idx.byName)
}

// FileCount returns the number of indexed files.
func (idx *SymbolIndex) FileCount() int {
	return len(idx.byFile)
}

// SymbolsMatching returns symbols whose names contain the given substring (case-insensitive).
func (idx *SymbolIndex) SymbolsMatching(query string) []string {
	query = strings.ToLower(query)
	var matches []string
	seen := make(map[string]bool)
	for name := range idx.byName {
		if strings.Contains(strings.ToLower(name), query) && !seen[name] {
			matches = append(matches, name)
			seen[name] = true
		}
	}
	return matches
}

// BuildIndex creates a symbol index from a list of parsed file infos.
func BuildIndex(files []*FileInfo) *SymbolIndex {
	idx := NewSymbolIndex()
	for _, f := range files {
		idx.AddFile(f)
	}
	return idx
}
