package efie

import (
	"sort"
	"strings"
)

// ScoredFile represents a file ranked by relevance to a task.
type ScoredFile struct {
	Path    string
	Score   float64
	Reasons []string
}

// Score computes the EFIE relevance score for a file.
// It combines 6 weighted components: centrality, direct relevance,
// import proximity, symbol match, community boost, and Bloom cross-check.
// PERF-34: targetSet is pre-computed once per query and passed in to avoid
// rebuilding the map on every Score() call.
func Score(file string, targets []string, description string,
	graph *WeightedImportGraph, index *MultiResIndex,
	targetCommunities map[int]bool, targetSet map[string]bool) ScoredFile {

	sf := ScoredFile{Path: file}
	node, hasNode := graph.nodes[file]

	// Component 1: Graph Centrality (20% weight)
	if hasNode {
		centralityScore := 0.4*node.PageRank +
			0.3*node.Betweenness +
			0.3*node.DegreeCentrality

		if index.centralityP95 > 0 {
			normalizedCentrality := centralityScore / index.centralityP95
			if normalizedCentrality > 1.0 {
				normalizedCentrality = 1.0
			}
			sf.Score += normalizedCentrality * 10.0 * 0.20
		}
		sf.Reasons = append(sf.Reasons, "centrality")
	}

	// Component 2: Direct Relevance (35% weight)
	if targetSet[file] {
		sf.Score += 35.0
		sf.Reasons = append(sf.Reasons, "directly mentioned")
	}

	// Component 3: Import Proximity (20% weight)
	if hasNode {
		for _, target := range targets {
			for _, imp := range node.Imports {
				if imp == target {
					sf.Score += 10.0
					sf.Reasons = append(sf.Reasons, "imports "+target)
					break
				}
			}
			for _, ib := range node.ImportedBy {
				if ib == target {
					sf.Score += 10.0
					sf.Reasons = append(sf.Reasons, "imported by "+target)
					break
				}
			}
		}
	}

	// Component 4: Symbol Match (15% weight, capped)
	if description != "" {
		identifiers := extractIdentifiers(description)
		if len(identifiers) > 0 {
			matchCount := 0
			for _, id := range identifiers {
				if index.symbolTrie != nil && index.symbolTrie.HasPrefix(id) {
					matchCount++
					sf.Reasons = append(sf.Reasons, "symbol match: "+id)
				}
			}
			symbolScore := float64(matchCount) * (15.0 / float64(len(identifiers)))
			if symbolScore > 15.0 {
				symbolScore = 15.0
			}
			sf.Score += symbolScore
		}
	}

	// Component 5: Gradient Community Boost (10% weight)
	fileCommunity, hasCommunity := index.fileToCommunity[file]
	if hasCommunity {
		if targetCommunities[fileCommunity] {
			sf.Score += 10.0
			sf.Reasons = append(sf.Reasons, "same community")
		} else {
			for targetComm := range targetCommunities {
				if adj, ok := index.communityAdj[targetComm]; ok && adj[fileCommunity] {
					sf.Score += 5.0
					sf.Reasons = append(sf.Reasons, "adjacent community")
					break
				}
			}
		}
	}

	// Component 6: Bloom Filter Cross-Check (quality gate, not scoring component)
	if hasNode && node.SymbolBloom != nil && description != "" {
		identifiers := extractIdentifiers(description)
		for _, id := range identifiers {
			if node.SymbolBloom.Contains(id) {
				// Verify with exact match to avoid false positive over-scoring
				if !exactSymbolCheck(file, id, index) {
					sf.Score *= 0.95
					break
				}
			}
		}
	}

	return sf
}

func exactSymbolCheck(file, symbol string, index *MultiResIndex) bool {
	for _, loc := range index.byName[symbol] {
		if loc.File == file {
			return true
		}
	}
	return false
}

// sortByScoreDescending sorts scored files by score descending.
func sortByScoreDescending(files []ScoredFile) {
	sort.SliceStable(files, func(i, j int) bool {
		return files[i].Score > files[j].Score
	})
}

// extractIdentifiers tokenizes text and extracts camelCase/snake_case identifiers.
func extractIdentifiers(text string) []string {
	var words []string
	seen := make(map[string]bool)

	addWord := func(w string) {
		if len(w) < 2 {
			return
		}
		lower := strings.ToLower(w)
		if isStopWord(lower) {
			return
		}
		if !seen[lower] {
			words = append(words, w)
			seen[lower] = true
		}
	}

	for _, w := range strings.Fields(text) {
		w = strings.Trim(w, ".,;:!?\"'()[]{}")
		if len(w) == 0 {
			continue
		}
		isIdent := false
		for _, r := range w {
			if (r >= 'A' && r <= 'Z') || (r >= 'a' && r <= 'z') || r == '_' || (r >= '0' && r <= '9') {
				isIdent = true
				break
			}
		}
		if !isIdent {
			continue
		}
		addWord(w)
		parts := splitCamelCase(w)
		for _, p := range parts {
			addWord(p)
		}
		if strings.Contains(w, "_") {
			for _, p := range strings.Split(w, "_") {
				addWord(p)
			}
		}
	}
	return words
}

func splitCamelCase(s string) []string {
	var parts []string
	var current strings.Builder

	for i, r := range s {
		if r >= 'A' && r <= 'Z' {
			if current.Len() > 0 {
				if i+1 < len(s) {
					next := rune(s[i+1])
					if next >= 'a' && next <= 'z' {
						parts = append(parts, current.String())
						current.Reset()
					} else if current.Len() > 0 {
						lastRune := rune(current.String()[current.Len()-1])
						if lastRune >= 'a' && lastRune <= 'z' {
							parts = append(parts, current.String())
							current.Reset()
						}
					}
				} else {
					parts = append(parts, current.String())
					current.Reset()
				}
			}
		}
		current.WriteRune(r)
	}
	if current.Len() > 0 {
		parts = append(parts, current.String())
	}
	return parts
}

var stopWords = map[string]bool{
	"the": true, "and": true, "for": true, "with": true,
	"that": true, "this": true, "from": true, "are": true,
	"was": true, "will": true, "can": true, "has": true,
	"but": true, "not": true, "all": true, "new": true,
	"add": true, "create": true, "delete": true, "update": true,
	"modify": true, "implement": true, "build": true, "make": true,
	"file": true, "files": true, "code": true, "function": true,
	"test": true, "tests": true, "using": true, "use": true,
}

func isStopWord(w string) bool {
	return stopWords[w]
}
