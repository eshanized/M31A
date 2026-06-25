package efie

import (
	"strings"

	"github.com/eshanized/M31A/internal/fileutil"
)

// MultiResIndex provides three-level indexing: File -> Package -> Community.
type MultiResIndex struct {
	// Level 0: File -> FileInfo
	files map[string]*fileInfo

	// Level 1: Package -> []file paths
	packages map[string][]string

	// Level 2: Community -> []file paths
	communities map[int][]string

	// Symbol -> definitions
	symbolTrie *Trie
	byName     map[string][]SymbolLocation

	// Reverse lookups
	fileToCommunity map[string]int

	// Adjacency: community -> set of neighboring community IDs
	communityAdj map[int]map[int]bool

	// Precomputed importance rankings
	importanceRank []string

	// Cached centrality percentiles
	centralityP50 float64
	centralityP95 float64
}

// SymbolLocation identifies where a symbol is defined.
type SymbolLocation struct {
	File string
	Kind string
}

// fileInfo is the internal representation of parsed file data.
type fileInfo struct {
	Path     string
	Language string
	Symbols  []SymbolInfo
}

// SymbolInfo represents a named symbol in a file.
type SymbolInfo struct {
	Name     string
	Kind     string
	Exported bool
}

// NewMultiResIndex creates a new empty multi-resolution index.
func NewMultiResIndex() *MultiResIndex {
	return &MultiResIndex{
		files:           make(map[string]*fileInfo),
		packages:        make(map[string][]string),
		communities:     make(map[int][]string),
		byName:          make(map[string][]SymbolLocation),
		fileToCommunity: make(map[string]int),
		communityAdj:    make(map[int]map[int]bool),
	}
}

// AddFile adds a file to the index.
func (idx *MultiResIndex) AddFile(path, language string, symbols []SymbolInfo) {
	fi := &fileInfo{
		Path:     path,
		Language: language,
		Symbols:  symbols,
	}
	idx.files[path] = fi

	// Package-level
	pkg := fileutil.DirOf(path)
	idx.packages[pkg] = append(idx.packages[pkg], path)

	// Symbol-level
	seen := make(map[string]bool)
	for _, s := range symbols {
		if !seen[s.Name] {
			seen[s.Name] = true
			idx.byName[s.Name] = append(idx.byName[s.Name], SymbolLocation{
				File: path,
				Kind: s.Kind,
			})
		}
	}
}

// SetCommunity sets the community ID for a file.
func (idx *MultiResIndex) SetCommunity(path string, community int) {
	idx.fileToCommunity[path] = community
	idx.communities[community] = append(idx.communities[community], path)
}

// SetCommunityAdj sets the adjacency map for communities.
func (idx *MultiResIndex) SetCommunityAdj(adj map[int]map[int]bool) {
	idx.communityAdj = adj
}

// SetImportanceRank sets the importance-ranked file list.
func (idx *MultiResIndex) SetImportanceRank(rank []string) {
	idx.importanceRank = rank
}

// SetCentralityPercentiles caches the 50th and 95th percentile centrality values.
func (idx *MultiResIndex) SetCentralityPercentiles(p50, p95 float64) {
	idx.centralityP50 = p50
	idx.centralityP95 = p95
}

// BuildTrieIndex builds the symbol trie from all indexed symbols.
func (idx *MultiResIndex) BuildTrieIndex() {
	names := make([]string, 0, len(idx.byName))
	for name := range idx.byName {
		names = append(names, name)
	}
	idx.symbolTrie = BuildTrie(names)
}

// Define returns all locations where a symbol is defined.
func (idx *MultiResIndex) Define(name string) []SymbolLocation {
	return idx.byName[name]
}

// FileSymbols returns all symbols defined in a file.
func (idx *MultiResIndex) FileSymbols(path string) []SymbolInfo {
	fi, ok := idx.files[path]
	if !ok {
		return nil
	}
	return fi.Symbols
}

// AllSymbols returns all unique symbol names.
func (idx *MultiResIndex) AllSymbols() []string {
	names := make([]string, 0, len(idx.byName))
	for name := range idx.byName {
		names = append(names, name)
	}
	return names
}

// SymbolCount returns the number of unique symbols.
func (idx *MultiResIndex) SymbolCount() int {
	return len(idx.byName)
}

// FileCount returns the number of indexed files.
func (idx *MultiResIndex) FileCount() int {
	return len(idx.files)
}

// SymbolsMatching returns symbols whose names contain the query (case-insensitive).
func (idx *MultiResIndex) SymbolsMatching(query string) []string {
	if idx.symbolTrie != nil {
		return idx.symbolTrie.PrefixSearch(query)
	}
	// Fallback to linear scan
	var matches []string
	seen := make(map[string]bool)
	queryLower := toLower(query)
	for name := range idx.byName {
		if strings.Contains(toLower(name), queryLower) && !seen[name] {
			matches = append(matches, name)
			seen[name] = true
		}
	}
	return matches
}

// SortedImportanceRank returns files sorted by PageRank descending.
func (idx *MultiResIndex) SortedImportanceRank() []string {
	rank := make([]string, len(idx.importanceRank))
	copy(rank, idx.importanceRank)
	return rank
}

// CommunityOf returns the community ID for a file.
func (idx *MultiResIndex) CommunityOf(path string) int {
	return idx.fileToCommunity[path]
}

// Communities returns all communities and their members.
func (idx *MultiResIndex) Communities() map[int][]string {
	return idx.communities
}

// CommunityAdj returns the community adjacency map.
func (idx *MultiResIndex) CommunityAdj() map[int]map[int]bool {
	return idx.communityAdj
}

func toLower(s string) string {
	b := make([]byte, len(s))
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'A' && c <= 'Z' {
			c += 'a' - 'A'
		}
		b[i] = c
	}
	return string(b)
}
