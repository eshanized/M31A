package workflow

import (
	"sort"
	"strings"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

const maxMergedFiles = 10

// mergeRelatedTasks merges tasks with overlapping file sets into fewer, larger tasks.
// Merge criteria: tasks in the same group with >50% file overlap, tasks with no
// inter-dependencies and same Action category. When merging: union the Files slices
// (deduplicated), union the AcceptanceCriteria (deduplicated), keep the first task's
// ID and Description, set Action to the most common action among merged tasks, set
// Dependencies to the union of all dependencies (excluding IDs of merged tasks).
// Cap merged task size at maxMergedFiles — if merging would exceed this, skip the merge
// for that pair. The function is deterministic (same input produces same output regardless
// of call order).
func mergeRelatedTasks(tasks []m31types.Task) []m31types.Task {
	if len(tasks) <= 1 {
		return tasks
	}

	// Build a union-find structure to group mergeable tasks
	parent := make([]int, len(tasks))
	for i := range parent {
		parent[i] = i
	}

	var find func(int) int
	find = func(x int) int {
		if parent[x] != x {
			parent[x] = find(parent[x])
		}
		return parent[x]
	}

	union := func(x, y int) {
		rx, ry := find(x), find(y)
		if rx != ry {
			// Union by ID: lower ID becomes root for determinism
			if tasks[rx].ID > tasks[ry].ID {
				parent[rx] = ry
			} else {
				parent[ry] = rx
			}
		}
	}

	// Check all pairs for merge eligibility
	for i := 0; i < len(tasks); i++ {
		for j := i + 1; j < len(tasks); j++ {
			if canMerge(tasks[i], tasks[j]) {
				// Check if merged size would exceed cap
				mergedFiles := unionStrings(tasks[i].Files, tasks[j].Files)
				if len(mergedFiles) <= maxMergedFiles {
					union(i, j)
				}
			}
		}
	}

	// Group tasks by root
	groups := make(map[int][]int)
	for i := range tasks {
		root := find(i)
		groups[root] = append(groups[root], i)
	}

	// Build result: merge groups with 2+ tasks, keep singles as-is
	result := make([]m31types.Task, 0, len(tasks))
	for _, indices := range groups {
		if len(indices) == 1 {
			result = append(result, tasks[indices[0]])
			continue
		}

		// Sort indices by task ID for deterministic ordering
		sort.Slice(indices, func(a, b int) bool {
			return tasks[indices[a]].ID < tasks[indices[b]].ID
		})

		merged := mergeGroup(tasks, indices)
		result = append(result, merged)
	}

	// Sort result by task ID for deterministic output
	sort.Slice(result, func(i, j int) bool {
		return result[i].ID < result[j].ID
	})

	return result
}

// canMerge checks if two tasks are eligible for merging.
// Eligibility: same action category, no inter-dependencies, and >50% file overlap.
func canMerge(a, b m31types.Task) bool {
	// Same action category
	if normalizeAction(a.Action) != normalizeAction(b.Action) {
		return false
	}

	// No inter-dependencies
	for _, dep := range a.Dependencies {
		if dep == b.ID {
			return false
		}
	}
	for _, dep := range b.Dependencies {
		if dep == a.ID {
			return false
		}
	}

	// >50% file overlap
	if len(a.Files) == 0 || len(b.Files) == 0 {
		return false
	}

	overlap := countOverlap(a.Files, b.Files)
	minFiles := len(a.Files)
	if len(b.Files) < minFiles {
		minFiles = len(b.Files)
	}

	return float64(overlap)/float64(minFiles) > 0.5
}

// normalizeAction standardizes action strings for comparison.
func normalizeAction(action string) string {
	action = strings.ToLower(strings.TrimSpace(action))
	// Normalize common variations
	action = strings.ReplaceAll(action, " ", "_")
	action = strings.ReplaceAll(action, "-", "_")
	action = strings.ReplaceAll(action, ".", "_")
	return action
}

// countOverlap counts how many strings in a appear in b.
func countOverlap(a, b []string) int {
	set := make(map[string]bool, len(b))
	for _, s := range b {
		set[s] = true
	}
	count := 0
	for _, s := range a {
		if set[s] {
			count++
		}
	}
	return count
}

// unionStrings returns the deduplicated union of two string slices.
func unionStrings(a, b []string) []string {
	seen := make(map[string]bool, len(a)+len(b))
	var result []string
	for _, s := range a {
		if !seen[s] {
			seen[s] = true
			result = append(result, s)
		}
	}
	for _, s := range b {
		if !seen[s] {
			seen[s] = true
			result = append(result, s)
		}
	}
	sort.Strings(result) // Deterministic ordering
	return result
}

// mergeGroup merges a group of tasks into a single task.
func mergeGroup(tasks []m31types.Task, indices []int) m31types.Task {
	// Use first task (lowest ID) as base
	merged := tasks[indices[0]]

	// Union files (deduplicated)
	merged.Files = nil
	for _, idx := range indices {
		merged.Files = unionStrings(merged.Files, tasks[idx].Files)
	}

	// Union acceptance criteria (deduplicated)
	merged.AcceptanceCriteria = nil
	seenCriteria := make(map[string]bool)
	for _, idx := range indices {
		for _, c := range tasks[idx].AcceptanceCriteria {
			if !seenCriteria[c] {
				seenCriteria[c] = true
				merged.AcceptanceCriteria = append(merged.AcceptanceCriteria, c)
			}
		}
	}

	// Set action to most common action among merged tasks
	actionCounts := make(map[string]int)
	for _, idx := range indices {
		action := normalizeAction(tasks[idx].Action)
		actionCounts[action]++
	}
	bestAction := merged.Action
	bestCount := 0
	for action, count := range actionCounts {
		if count > bestCount || (count == bestCount && strings.ToLower(action) < strings.ToLower(bestAction)) {
			bestAction = tasks[indices[0]].Action // Keep original casing from first task
			bestCount = count
		}
	}
	merged.Action = bestAction

	// Union dependencies (excluding IDs of merged tasks)
	mergedIDs := make(map[int]bool)
	for _, idx := range indices {
		mergedIDs[tasks[idx].ID] = true
	}
	depSet := make(map[int]bool)
	for _, idx := range indices {
		for _, dep := range tasks[idx].Dependencies {
			if !mergedIDs[dep] {
				depSet[dep] = true
			}
		}
	}
	merged.Dependencies = nil
	for dep := range depSet {
		merged.Dependencies = append(merged.Dependencies, dep)
	}
	sort.Ints(merged.Dependencies) // Deterministic ordering

	// Reset status since this is a new merged task
	merged.Status = m31types.StatusPending
	merged.HealsAttempted = 0
	merged.CommitHash = ""

	return merged
}
