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
	byFile map[string][]SymbolInfo     // file → exported symbols
	trie   *SymbolTrie                 // O(K) prefix search
}

// NewSymbolIndex creates an empty symbol index.
func NewSymbolIndex() *SymbolIndex {
	return &SymbolIndex{
		byName: make(map[string][]SymbolLocation),
		byFile: make(map[string][]SymbolInfo),
		trie:   NewSymbolTrie(),
	}
}

// AddFile indexes all symbols from a parsed file.
func (idx *SymbolIndex) AddFile(info *FileInfo) {
	if info == nil {
		return
	}

	var symbols []SymbolInfo
	seen := make(map[string]bool, len(info.Exports)+len(info.Funcs)+len(info.Types))

	for _, s := range info.Exports {
		symbols = append(symbols, s)
		seen[s.Name] = true
		idx.byName[s.Name] = append(idx.byName[s.Name], SymbolLocation{
			File: info.Path,
			Kind: s.Kind,
		})
		idx.trie.Insert(s.Name)
	}

	for _, f := range info.Funcs {
		if !seen[f.Name] {
			symbols = append(symbols, SymbolInfo{
				Name: f.Name, Kind: "func", Exported: f.Exported,
			})
			seen[f.Name] = true
			idx.byName[f.Name] = append(idx.byName[f.Name], SymbolLocation{
				File: info.Path, Kind: "func",
			})
			idx.trie.Insert(f.Name)
		}
	}

	for _, ti := range info.Types {
		if !seen[ti.Name] {
			symbols = append(symbols, SymbolInfo{
				Name: ti.Name, Kind: ti.Kind, Exported: true,
			})
			seen[ti.Name] = true
			idx.byName[ti.Name] = append(idx.byName[ti.Name], SymbolLocation{
				File: info.Path, Kind: ti.Kind,
			})
			idx.trie.Insert(ti.Name)
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

// typeKinds is the set of symbol kinds that represent type definitions.
var typeKinds = map[string]bool{
	"struct": true, "interface": true, "type": true,
	"class": true, "enum": true, "trait": true,
}

// FindTypes returns all type/struct/interface/class/enum definitions matching the name.
func (idx *SymbolIndex) FindTypes(name string) []SymbolLocation {
	var results []SymbolLocation
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

// RemoveFile removes all symbols defined in the given file from the index.
func (idx *SymbolIndex) RemoveFile(path string) {
	symbols, ok := idx.byFile[path]
	if !ok {
		return
	}
	delete(idx.byFile, path)

	for _, s := range symbols {
		locs := idx.byName[s.Name]
		for i := len(locs) - 1; i >= 0; i-- {
			if locs[i].File == path {
				idx.byName[s.Name] = append(locs[:i], locs[i+1:]...)
			}
		}
		if len(idx.byName[s.Name]) == 0 {
			delete(idx.byName, s.Name)
			idx.trie.Delete(s.Name)
		}
	}
}

// SymbolsMatching returns symbols whose names contain the given substring (case-insensitive).
// Uses Trie for O(K + M) performance instead of O(N) linear scan.
func (idx *SymbolIndex) SymbolsMatching(query string) []string {
	query = strings.ToLower(query)

	// Exact match: O(1) via hash map
	if _, ok := idx.byName[query]; ok {
		return []string{query}
	}

	// Prefix match via Trie: O(K + M)
	prefixMatches := idx.trie.PrefixSearch(query)

	// Also check for substring matches (Trie only does prefix)
	// For substring matching, we still need to scan, but we can use the Trie's
	// collected symbols as a starting point for common prefixes
	if len(prefixMatches) > 0 {
		return prefixMatches
	}

	// Fallback: substring match (for cases like "user" matching "GetUser")
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

// RemoveFiles removes multiple files from the index.
func (idx *SymbolIndex) RemoveFiles(paths []string) {
	for _, p := range paths {
		idx.RemoveFile(p)
	}
}

// AddFiles indexes multiple parsed files into an existing index.
func (idx *SymbolIndex) AddFiles(files []*FileInfo) {
	for _, f := range files {
		idx.AddFile(f)
	}
}
