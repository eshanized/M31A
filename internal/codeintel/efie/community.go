package efie

import "math/rand"

// LouvainDetect_Deterministic runs one-pass Louvain community detection
// with a fixed seed for reproducible output.
func LouvainDetect_Deterministic(g *WeightedImportGraph, seed int64) map[string]int {
	rng := rand.New(rand.NewSource(seed))

	// Each node starts in its own community
	communityOf := make(map[string]int)
	for _, node := range g.nodes {
		if node.Path == ExternalNode {
			continue
		}
		communityOf[node.Path] = hashString(node.Path)
	}

	m := g.EdgeCount()
	if m == 0 {
		return communityOf
	}

	for pass := 0; pass < 10; pass++ {
		improved := false
		// Deterministic order: sort first, then shuffle with fixed seed
		paths := g.AllPaths()
		sortedStringSlice(paths)
		rng.Shuffle(len(paths), func(i, j int) { paths[i], paths[j] = paths[j], paths[i] })

		for _, nodePath := range paths {
			if nodePath == ExternalNode {
				continue
			}
			node := g.nodes[nodePath]
			bestCommunity := communityOf[nodePath]
			bestGain := 0.0

			// Collect unique neighbor communities
			neighborComms := make(map[int]bool)
			for _, imp := range node.Imports {
				if imp == ExternalNode {
					continue
				}
				if c, ok := communityOf[imp]; ok {
					neighborComms[c] = true
				}
			}
			for _, ib := range node.ImportedBy {
				if ib == ExternalNode {
					continue
				}
				if c, ok := communityOf[ib]; ok {
					neighborComms[c] = true
				}
			}

			for targetComm := range neighborComms {
				gain := modularityGain(nodePath, targetComm, g, communityOf, m)
				if gain > bestGain {
					bestGain = gain
					bestCommunity = targetComm
				}
			}

			if bestCommunity != communityOf[nodePath] {
				communityOf[nodePath] = bestCommunity
				improved = true
			}
		}

		if !improved {
			break
		}
	}

	// Canonical renumbering: map arbitrary IDs to 0, 1, 2, ... in sorted order
	paths := g.AllPaths()
	sortedPaths := make([]string, 0, len(paths))
	for _, p := range paths {
		if p != ExternalNode {
			sortedPaths = append(sortedPaths, p)
		}
	}
	sortedStringSlice(sortedPaths)

	canonicalMap := make(map[int]int)
	nextID := 0
	for _, p := range sortedPaths {
		c := communityOf[p]
		if _, ok := canonicalMap[c]; !ok {
			canonicalMap[c] = nextID
			nextID++
		}
		communityOf[p] = canonicalMap[c]
	}

	return communityOf
}

func modularityGain(nodePath string, targetComm int, g *WeightedImportGraph, communityOf map[string]int, m int) float64 {
	kin := 0
	node := g.nodes[nodePath]
	nodeDegree := len(node.Imports) + len(node.ImportedBy)

	// Count edges from node to targetCommunity
	for _, imp := range node.Imports {
		if c, ok := communityOf[imp]; ok && c == targetComm {
			kin++
		}
	}
	for _, ib := range node.ImportedBy {
		if c, ok := communityOf[ib]; ok && c == targetComm {
			kin++
		}
	}

	// Community total degree
	sigmaTot := 0
	for _, n := range g.nodes {
		if n.Path == ExternalNode {
			continue
		}
		if c, ok := communityOf[n.Path]; ok && c == targetComm {
			sigmaTot += len(n.Imports) + len(n.ImportedBy)
		}
	}

	gain := (2*float64(kin) - float64(sigmaTot)*float64(nodeDegree)/float64(m)) / (2 * float64(m))
	return gain
}

func hashString(s string) int {
	h := 0
	for i := 0; i < len(s); i++ {
		h = h*31 + int(s[i])
	}
	if h < 0 {
		h = -h
	}
	return h
}

func sortedStringSlice(s []string) {
	for i := 1; i < len(s); i++ {
		for j := i; j > 0 && s[j] < s[j-1]; j-- {
			s[j], s[j-1] = s[j-1], s[j]
		}
	}
}
