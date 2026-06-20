package codeintel

import (
	"path/filepath"
	"sort"
	"strings"
)

// ScoredFile represents a file ranked by relevance to a task.
type ScoredFile struct {
	Path    string
	Score   float64
	Reasons []string
}

// RelevanceScorer ranks files by how relevant they are to a given task.
type RelevanceScorer struct {
	graph *ImportGraph
	index *SymbolIndex
}

// NewRelevanceScorer creates a scorer backed by the given graph and index.
func NewRelevanceScorer(graph *ImportGraph, index *SymbolIndex) *RelevanceScorer {
	return &RelevanceScorer{graph: graph, index: index}
}

// Score computes relevance scores for all files relative to a set of target
// files and a task description. Returns the top-N files sorted by score.
func (s *RelevanceScorer) Score(targetFiles []string, taskDescription string, topN int) []ScoredFile {
	scores := make(map[string]*ScoredFile)

	getOrCreate := func(path string) *ScoredFile {
		sf, ok := scores[path]
		if !ok {
			sf = &ScoredFile{Path: path}
			scores[path] = sf
		}
		return sf
	}

	targetSet := make(map[string]bool)
	for _, f := range targetFiles {
		targetSet[f] = true
		sf := getOrCreate(f)
		sf.Score += 10.0
		sf.Reasons = append(sf.Reasons, "directly mentioned in task")
	}

	for _, target := range targetFiles {
		upstream := s.graph.Upstream(target, 1)
		for _, u := range upstream {
			if targetSet[u] {
				continue
			}
			sf := getOrCreate(u)
			sf.Score += 5.0
			sf.Reasons = append(sf.Reasons, "imported by "+target)
		}

		downstream := s.graph.Downstream(target, 1)
		for _, d := range downstream {
			if targetSet[d] {
				continue
			}
			sf := getOrCreate(d)
			sf.Score += 5.0
			sf.Reasons = append(sf.Reasons, "imports "+target)
		}
	}

	if taskDescription != "" {
		words := extractIdentifiers(taskDescription)
		for _, word := range words {
			locs := s.index.Define(word)
			for _, loc := range locs {
				if targetSet[loc.File] {
					continue
				}
				sf := getOrCreate(loc.File)
				sf.Score += 7.0
				sf.Reasons = append(sf.Reasons, "defines "+word+" (referenced in task)")
			}
		}
	}

	for _, target := range targetFiles {
		targetDir := filepath.Dir(target)
		for path := range scores {
			if targetSet[path] {
				continue
			}
			if filepath.Dir(path) == targetDir {
				sf := scores[path]
				sf.Score += 3.0
				sf.Reasons = append(sf.Reasons, "same package as "+target)
			}
		}
	}

	for _, target := range targetFiles {
		upstream := s.graph.Upstream(target, 0)
		for depth, u := range upstream {
			if _, already := scores[u]; already {
				continue
			}
			sf := getOrCreate(u)
			decayScore := 2.0 / float64(depth+2)
			sf.Score += decayScore
			sf.Reasons = append(sf.Reasons, "transitive dependency")
		}
	}

	result := make([]ScoredFile, 0, len(scores))
	for _, sf := range scores {
		result = append(result, *sf)
	}

	sortByScore(result)

	if topN > 0 && len(result) > topN {
		result = result[:topN]
	}

	return result
}

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
		// Check if it's a valid identifier character mix
		isIdent := false
		for _, r := range w {
			if (r >= 'A' && r <= 'Z') || (r >= 'a' && r <= 'z') || r == '_' || r >= '0' && r <= '9' {
				isIdent = true
				break
			}
		}
		if !isIdent {
			continue
		}

		// Add the whole word first
		addWord(w)

		// Decompose camelCase: getUserByID → get, User, By, ID
		parts := splitCamelCase(w)
		for _, p := range parts {
			addWord(p)
		}

		// Decompose snake_case: get_user_by_id → get, user, by, id
		if strings.Contains(w, "_") {
			for _, p := range strings.Split(w, "_") {
				addWord(p)
			}
		}
	}
	return words
}

// splitCamelCase breaks a camelCase or PascalCase identifier into its parts.
// Examples: getUserByID → [get, User, By, ID], HTMLParser → [HTML, Parser]
func splitCamelCase(s string) []string {
	var parts []string
	var current strings.Builder

	for i, r := range s {
		if r >= 'A' && r <= 'Z' {
			if current.Len() > 0 {
				if i+1 < len(s) {
					next := rune(s[i+1])
					if next >= 'a' && next <= 'z' {
						// Upper followed by lower: word boundary (e.g., "getUser" → "get", "User")
						parts = append(parts, current.String())
						current.Reset()
					} else if current.Len() > 0 {
						// Check if current ends with lowercase (transition to acronym)
						lastRune := rune(current.String()[current.Len()-1])
						if lastRune >= 'a' && lastRune <= 'z' {
							parts = append(parts, current.String())
							current.Reset()
						}
					}
				} else {
					// End of string
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

func sortByScore(files []ScoredFile) {
	sort.SliceStable(files, func(i, j int) bool {
		return files[i].Score > files[j].Score
	})
}
