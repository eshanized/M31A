package codeintel

import (
	"path/filepath"
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
	for _, w := range strings.Fields(text) {
		w = strings.Trim(w, ".,;:!?\"'()[]{}")
		if len(w) < 3 {
			continue
		}
		if isStopWord(strings.ToLower(w)) {
			continue
		}
		isIdent := false
		for _, r := range w {
			if (r >= 'A' && r <= 'Z') || (r >= 'a' && r <= 'z') || r == '_' {
				isIdent = true
				break
			}
		}
		if isIdent && !seen[w] {
			words = append(words, w)
			seen[w] = true
		}
	}
	return words
}

func isStopWord(w string) bool {
	stops := map[string]bool{
		"the": true, "and": true, "for": true, "with": true,
		"that": true, "this": true, "from": true, "are": true,
		"was": true, "will": true, "can": true, "has": true,
		"but": true, "not": true, "all": true, "new": true,
		"add": true, "create": true, "delete": true, "update": true,
		"modify": true, "implement": true, "build": true, "make": true,
		"file": true, "files": true, "code": true, "function": true,
		"test": true, "tests": true, "using": true, "use": true,
	}
	return stops[w]
}

func sortByScore(files []ScoredFile) {
	for i := 1; i < len(files); i++ {
		for j := i; j > 0 && files[j].Score > files[j-1].Score; j-- {
			files[j], files[j-1] = files[j-1], files[j]
		}
	}
}
