package efie

import (
	"container/heap"
	"sort"
)

// QueryType represents the type of codebase query.
type QueryType string

const (
	QueryRelevant   QueryType = "relevant"
	QueryUpstream   QueryType = "upstream"
	QueryDownstream QueryType = "downstream"
	QueryDefine     QueryType = "define"
	QueryReferences QueryType = "references"
)

// Query executes an EFIE query with adaptive expansion.
func Query(efie *EFIEIndex, targetFiles []string, taskDescription string,
	queryType QueryType, topN int) []ScoredFile {

	switch queryType {
	case QueryUpstream:
		return bfsQuery(efie, targetFiles, "imports", topN)
	case QueryDownstream:
		return bfsQuery(efie, targetFiles, "importedBy", topN)
	case QueryDefine:
		return defineQuery(efie, taskDescription)
	case QueryReferences:
		return referencesQuery(efie, taskDescription, topN)
	default:
		return adaptiveExpansionQuery(efie, targetFiles, taskDescription, topN)
	}
}

func bfsQuery(efie *EFIEIndex, targets []string, edgeType string, topN int) []ScoredFile {
	visited := make(map[string]bool)
	var result []ScoredFile

	for _, target := range targets {
		visited[target] = true
		type entry struct {
			path  string
			depth int
		}
		queue := []entry{{path: target, depth: 0}}

		for len(queue) > 0 {
			cur := queue[0]
			queue = queue[1:]

			if cur.depth > 5 {
				continue
			}

			node, ok := efie.graph.nodes[cur.path]
			if !ok {
				continue
			}

			var neighbors []string
			if edgeType == "imports" {
				neighbors = node.Imports
			} else {
				neighbors = node.ImportedBy
			}

			for _, neighbor := range neighbors {
				if !visited[neighbor] {
					visited[neighbor] = true
					result = append(result, ScoredFile{
						Path:    neighbor,
						Score:   float64(10 - cur.depth),
						Reasons: []string{"depth " + itoa(cur.depth+1)},
					})
					queue = append(queue, entry{path: neighbor, depth: cur.depth + 1})
				}
			}
		}
	}

	sortByScoreDescending(result)
	if topN > 0 && len(result) > topN {
		result = result[:topN]
	}
	return result
}

func defineQuery(efie *EFIEIndex, symbol string) []ScoredFile {
	locs := efie.index.Define(symbol)
	var result []ScoredFile
	for _, loc := range locs {
		result = append(result, ScoredFile{
			Path:    loc.File,
			Score:   100.0,
			Reasons: []string{"defines " + symbol},
		})
	}
	sortByScoreDescending(result)
	return result
}

func referencesQuery(efie *EFIEIndex, symbol string, topN int) []ScoredFile {
	locs := efie.index.Define(symbol)
	visited := make(map[string]bool)
	var result []ScoredFile

	for _, loc := range locs {
		downstream := efie.graph.Downstream(loc.File, 1)
		for _, d := range downstream {
			if !visited[d] {
				visited[d] = true
				result = append(result, ScoredFile{
					Path:    d,
					Score:   50.0,
					Reasons: []string{"references " + symbol},
				})
			}
		}
	}

	sortByScoreDescending(result)
	if topN > 0 && len(result) > topN {
		result = result[:topN]
	}
	return result
}

// adaptiveExpansionQuery is the core EFIE query algorithm.
func adaptiveExpansionQuery(efie *EFIEIndex, targetFiles []string,
	taskDescription string, topN int) []ScoredFile {

	// Step 1: Seed generation with community boost
	seeds := make(map[string]bool)
	for _, t := range targetFiles {
		seeds[t] = true
	}

	// Direct neighbor expansion
	for _, target := range targetFiles {
		node, ok := efie.graph.nodes[target]
		if !ok {
			continue
		}
		for _, imp := range node.Imports {
			seeds[imp] = true
		}
		for _, ib := range node.ImportedBy {
			seeds[ib] = true
		}
	}

	// Community member expansion
	targetCommunities := make(map[int]bool)
	for _, target := range targetFiles {
		if c, ok := efie.index.fileToCommunity[target]; ok {
			targetCommunities[c] = true
		}
	}
	for c := range targetCommunities {
		if members, ok := efie.index.communities[c]; ok {
			for _, m := range members {
				seeds[m] = true
			}
		}
	}

	// Adjacent community expansion (gradient boost)
	for c := range targetCommunities {
		if adj, ok := efie.index.communityAdj[c]; ok {
			for adjComm := range adj {
				if members, ok := efie.index.communities[adjComm]; ok {
					for _, m := range members {
						seeds[m] = true
					}
				}
			}
		}
	}

	// Symbol-based seed expansion
	if taskDescription != "" {
		identifiers := extractIdentifiers(taskDescription)
		for _, id := range identifiers {
			if efie.index.symbolTrie != nil {
				matches := efie.index.symbolTrie.PrefixSearch(id)
				for _, match := range matches {
					for _, loc := range efie.index.byName[match] {
						seeds[loc.File] = true
					}
				}
			}
		}
	}

	// Step 2: Importance-weighted expansion (max-heap BFS)
	candidates := make(maxHeap, 0)
	visited := make(map[string]bool)
	hopDistance := make(map[string]int)
	var expanded []scoredEntry

	for seed := range seeds {
		sf := Score(seed, targetFiles, taskDescription, efie.graph, efie.index, targetCommunities)
		entry := scoredEntry{path: seed, score: sf.Score, scoredFile: sf}
		heap.Push(&candidates, &entry)
		visited[seed] = true
		hopDistance[seed] = 0
	}

	// Adaptive expansion threshold
	medianPageRank := computeMedianPageRank(efie.graph)
	expansionThreshold := medianPageRank * 0.5

	expansionBudget := topN * 5
	explored := 0

	for explored < expansionBudget && candidates.Len() > 0 {
		current := heap.Pop(&candidates).(*scoredEntry)
		expanded = append(expanded, *current)

		node, ok := efie.graph.nodes[current.path]
		if !ok {
			continue
		}

		// Expand to neighbors
		for _, neighbor := range node.Imports {
			if visited[neighbor] {
				continue
			}
			visited[neighbor] = true
			hopDistance[neighbor] = hopDistance[current.path] + 1

			neighborNode, ok := efie.graph.nodes[neighbor]
			if ok && neighborNode.PageRank > expansionThreshold {
				sf := Score(neighbor, targetFiles, taskDescription, efie.graph, efie.index, targetCommunities)
				heap.Push(&candidates, &scoredEntry{path: neighbor, score: sf.Score, scoredFile: sf})
				explored++
			}
		}
		for _, neighbor := range node.ImportedBy {
			if visited[neighbor] {
				continue
			}
			visited[neighbor] = true
			hopDistance[neighbor] = hopDistance[current.path] + 1

			neighborNode, ok := efie.graph.nodes[neighbor]
			if ok && neighborNode.PageRank > expansionThreshold {
				sf := Score(neighbor, targetFiles, taskDescription, efie.graph, efie.index, targetCommunities)
				heap.Push(&candidates, &scoredEntry{path: neighbor, score: sf.Score, scoredFile: sf})
				explored++
			}
		}
	}

	// Drain any remaining candidates
	for candidates.Len() > 0 {
		entry := heap.Pop(&candidates).(*scoredEntry)
		expanded = append(expanded, *entry)
	}

	// Step 3: Return top-N
	sort.SliceStable(expanded, func(i, j int) bool {
		return expanded[i].score > expanded[j].score
	})
	n := topN
	if n > len(expanded) {
		n = len(expanded)
	}
	result := make([]ScoredFile, 0, n)
	for i := 0; i < n; i++ {
		result = append(result, expanded[i].scoredFile)
	}
	return result
}

func computeMedianPageRank(g *WeightedImportGraph) float64 {
	var values []float64
	for _, node := range g.nodes {
		values = append(values, node.PageRank)
	}
	if len(values) == 0 {
		return 0
	}
	sorted := make([]float64, len(values))
	copy(sorted, values)
	sortFloat64s(sorted)
	mid := len(sorted) / 2
	if len(sorted)%2 == 0 {
		return (sorted[mid-1] + sorted[mid]) / 2
	}
	return sorted[mid]
}

// Max-heap for scored entries
type scoredEntry struct {
	path       string
	score      float64
	scoredFile ScoredFile
	index      int
}

type maxHeap []*scoredEntry

func (h maxHeap) Len() int           { return len(h) }
func (h maxHeap) Less(i, j int) bool { return h[i].score > h[j].score }
func (h maxHeap) Swap(i, j int)      { h[i], h[j] = h[j], h[i]; h[i].index = i; h[j].index = j }
func (h *maxHeap) Push(x interface{}) {
	entry := x.(*scoredEntry)
	entry.index = len(*h)
	*h = append(*h, entry)
}
func (h *maxHeap) Pop() interface{} {
	old := *h
	n := len(old)
	entry := old[n-1]
	old[n-1] = nil
	*h = old[:n-1]
	return entry
}

func sortFloat64s(s []float64) {
	for i := 1; i < len(s); i++ {
		for j := i; j > 0 && s[j] < s[j-1]; j-- {
			s[j], s[j-1] = s[j-1], s[j]
		}
	}
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}
