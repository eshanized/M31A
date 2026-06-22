package efie

import (
	"math"
	"sort"
)

// ComputePageRank computes the stationary distribution probability for each node.
func ComputePageRank(g *WeightedImportGraph, iterations int, damping float64) map[string]float64 {
	N := g.NodeCount()
	if N == 0 {
		return nil
	}

	PR := make(map[string]float64, N)
	uniform := 1.0 / float64(N)
	for _, node := range g.nodes {
		PR[node.Path] = uniform
	}

	for i := 0; i < iterations; i++ {
		newPR := make(map[string]float64, N)
		for path := range g.nodes {
			newPR[path] = (1 - damping) / float64(N)
		}

		for _, node := range g.nodes {
			importers := node.ImportedBy
			if len(importers) > 0 {
				share := PR[node.Path] / float64(len(importers))
				for _, importer := range importers {
					newPR[importer] += damping * share
				}
			}
		}

		// Handle dangling nodes
		danglingSum := 0.0
		for _, node := range g.nodes {
			if len(node.ImportedBy) == 0 {
				danglingSum += PR[node.Path]
			}
		}
		if danglingSum > 0 {
			for path := range g.nodes {
				newPR[path] += damping * danglingSum / float64(N)
			}
		}

		// Check convergence
		diff := 0.0
		for path := range g.nodes {
			diff += math.Abs(newPR[path] - PR[path])
		}
		PR = newPR
		if diff < 1e-6 {
			break
		}
	}

	return PR
}

// ComputeApproxBetweenness computes approximate betweenness centrality
// using stratified random sampling.
func ComputeApproxBetweenness(g *WeightedImportGraph, sampleSize int, seed int64) map[string]float64 {
	N := g.NodeCount()
	if N == 0 {
		return nil
	}

	betweenness := make(map[string]float64, N)
	for _, node := range g.nodes {
		betweenness[node.Path] = 0.0
	}

	sources := stratifiedSample(g, sampleSize, seed)

	for _, source := range sources {
		distances := make(map[string]int, N)
		predecessors := make(map[string][]string, N)
		sigma := make(map[string]float64, N)

		for _, node := range g.nodes {
			distances[node.Path] = -1
			sigma[node.Path] = 0.0
		}
		distances[source] = 0
		sigma[source] = 1.0

		queue := []string{source}
		for len(queue) > 0 {
			v := queue[0]
			queue = queue[1:]
			node := g.nodes[v]
			for _, w := range node.Imports {
				if distances[w] == -1 {
					distances[w] = distances[v] + 1
					queue = append(queue, w)
				}
				if distances[w] == distances[v]+1 {
					sigma[w] += sigma[v]
					predecessors[w] = append(predecessors[w], v)
				}
			}
		}

		// Back-propagation
		delta := make(map[string]float64, N)
		// Get BFS order (reversed)
		bfsOrder := make([]string, 0, N)
		for _, node := range g.nodes {
			if distances[node.Path] >= 0 {
				bfsOrder = append(bfsOrder, node.Path)
			}
		}
		sort.Slice(bfsOrder, func(i, j int) bool {
			return distances[bfsOrder[i]] > distances[bfsOrder[j]]
		})

		for _, v := range bfsOrder {
			for _, u := range predecessors[v] {
				if sigma[v] > 0 {
					delta[u] += (sigma[u] / sigma[v]) * (1 + delta[v])
				}
			}
			if v != source {
				betweenness[v] += delta[v]
			}
		}
	}

	// Normalize
	normFactor := 1.0 / float64(sampleSize*(N-1))
	for path := range betweenness {
		betweenness[path] *= normFactor
	}

	return betweenness
}

func stratifiedSample(g *WeightedImportGraph, sampleSize int, seed int64) []string {
	// Group nodes by community
	communityNodes := make(map[int][]string)
	for _, node := range g.nodes {
		if node.Path == ExternalNode {
			continue
		}
		communityNodes[node.Community] = append(communityNodes[node.Community], node.Path)
	}

	rng := newSeededRng(seed)
	var samples []string
	for _, nodes := range communityNodes {
		proportion := float64(len(nodes)) / float64(g.NodeCount())
		n := int(float64(sampleSize)*proportion + 0.5)
		if n < 1 {
			n = 1
		}
		if n > len(nodes) {
			n = len(nodes)
		}
		// Simple random sample
		perm := rng.Perm(len(nodes))
		for i := 0; i < n && i < len(perm); i++ {
			samples = append(samples, nodes[perm[i]])
		}
	}

	if len(samples) > sampleSize {
		samples = samples[:sampleSize]
	}
	return samples
}

// ComputePercentile returns the p-th percentile of the values.
func ComputePercentile(values []float64, p float64) float64 {
	if len(values) == 0 {
		return 0
	}
	sorted := make([]float64, len(values))
	copy(sorted, values)
	sort.Float64s(sorted)
	idx := int(float64(len(sorted)-1) * p / 100.0)
	return sorted[idx]
}
