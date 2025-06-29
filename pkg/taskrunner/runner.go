package taskrunner

import (
	"context"
	"fmt"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// TaskResult holds the outcome of executing a single task.
type TaskResult struct {
	Success    bool
	Output     string
	Error      string
	CommitHash string
	DurationMs int64
	ToolCalls  int // number of tool calls made during task execution
}

// ExecuteFunc is the function called to execute a single task.
type ExecuteFunc func(ctx context.Context, task types.Task) TaskResult

// Runner schedules and executes tasks with dependency resolution.
type Runner struct {
	tasks       []types.Task
	status      map[int]types.TaskStatus
	results     map[int]TaskResult
	idToIdx     map[int]int
	OnTaskStart  func(task types.Task)
	OnTaskUpdate func(task types.Task, status string)
	// TaskTimeout is the per-task timeout. Zero means no timeout.
	// Defaults to 30 minutes to match the Bash tool's default timeout.
	TaskTimeout time.Duration
}

// New creates a Runner for the given tasks.
func New(tasks []types.Task) *Runner {
	r := &Runner{
		tasks:       tasks,
		status:      make(map[int]types.TaskStatus),
		results:     make(map[int]TaskResult),
		idToIdx:     make(map[int]int),
		TaskTimeout: 30 * time.Minute,
	}
	for i, t := range tasks {
		r.idToIdx[t.ID] = i
		if t.Status == "" {
			r.status[t.ID] = types.StatusPending
		} else {
			r.status[t.ID] = t.Status
		}
	}
	return r
}

// Schedule performs topological sort and returns execution groups.
// Each group contains task IDs that are ready to execute (dependencies met).
// In V1, tasks within a group run sequentially.
func (r *Runner) Schedule() ([][]int, error) {
	if len(r.tasks) == 0 {
		return [][]int{}, nil
	}

	// Self-reference detection
	for _, t := range r.tasks {
		for _, dep := range t.Dependencies {
			if dep == t.ID {
				return nil, fmt.Errorf("task %d: %w", t.ID, m31errors.ErrCircularDependency)
			}
		}
	}

	// Build adjacency list and in-degree count
	n := len(r.tasks)
	idSet := make(map[int]bool)
	for _, t := range r.tasks {
		idSet[t.ID] = true
	}

	inDegree := make(map[int]int)
	dependents := make(map[int][]int) // dep -> list of tasks that depend on it

	for _, t := range r.tasks {
		if _, ok := inDegree[t.ID]; !ok {
			inDegree[t.ID] = 0
		}
		for _, dep := range t.Dependencies {
			if !idSet[dep] {
				return nil, fmt.Errorf("task %d: references non-existent dependency %d", t.ID, dep)
			}
			dependents[dep] = append(dependents[dep], t.ID)
			inDegree[t.ID]++
		}
	}

	// Kahn's algorithm
	var groups [][]int
	var queue []int

	// Start with zero in-degree nodes
	for _, t := range r.tasks {
		if inDegree[t.ID] == 0 {
			queue = append(queue, t.ID)
		}
	}

	processed := 0
	for len(queue) > 0 {
		groups = append(groups, queue)
		processed += len(queue)

		var nextQueue []int
		for _, id := range queue {
			for _, depID := range dependents[id] {
				inDegree[depID]--
				if inDegree[depID] == 0 {
					nextQueue = append(nextQueue, depID)
				}
			}
		}
		queue = nextQueue
	}

	if processed < n {
		return nil, m31errors.ErrCircularDependency
	}

	return groups, nil
}

// ExecuteGroup runs all tasks in the group sequentially.
// Dependencies must have been completed in prior groups.
func (r *Runner) ExecuteGroup(group []int, fn ExecuteFunc) error {
	for _, id := range group {
		idx, ok := r.idToIdx[id]
		if !ok {
			return fmt.Errorf("task %d: not found in runner", id)
		}
		task := r.tasks[idx]

		// Skip tasks already in terminal state
		if curStatus := r.status[task.ID]; curStatus == types.StatusSkipped ||
			curStatus == types.StatusFailed || curStatus == types.StatusUnrecoverable ||
			curStatus == types.StatusDone {
			continue
		}

		// Check all dependencies completed
		allDepsOK := true
		for _, depID := range task.Dependencies {
			depStatus := r.status[depID]
			if depStatus == types.StatusFailed || depStatus == types.StatusSkipped || depStatus == types.StatusUnrecoverable {
				// Block this task
				r.status[task.ID] = types.StatusSkipped
				r.results[task.ID] = TaskResult{Success: false, Error: fmt.Sprintf("dependency %d failed/skipped", depID)}
				allDepsOK = false
				break
			}
			if depStatus != types.StatusDone {
				// Shouldn't happen with correct grouping, but skip if it does
				r.status[task.ID] = types.StatusSkipped
				r.results[task.ID] = TaskResult{Success: false, Error: fmt.Sprintf("dependency %d not completed", depID)}
				allDepsOK = false
				break
			}
		}

		if !allDepsOK {
			continue
		}

		// Execute the task
		r.status[task.ID] = types.StatusRunning
		if r.OnTaskStart != nil {
			r.OnTaskStart(task)
		}

		var result TaskResult
		if fn != nil {
			var cancel context.CancelFunc
			var taskCtx context.Context
			if r.TaskTimeout > 0 {
				taskCtx, cancel = context.WithTimeout(context.Background(), r.TaskTimeout)
			} else {
				taskCtx, cancel = context.Background(), func() {}
			}
			result = fn(taskCtx, task)
			cancel()
		} else {
			result = TaskResult{Success: true}
		}

		r.results[task.ID] = result

		if result.Success {
			r.status[task.ID] = types.StatusDone
			if r.OnTaskUpdate != nil {
				r.OnTaskUpdate(task, "done")
			}
			if result.CommitHash != "" {
				// Update task's commit hash
				r.tasks[idx].CommitHash = result.CommitHash
			}
		} else {
			r.status[task.ID] = types.StatusFailed
			if r.OnTaskUpdate != nil {
				r.OnTaskUpdate(task, "failed")
			}
		}
	}

	return nil
}

// Status returns the current status of a task.
func (r *Runner) Status(id int) types.TaskStatus {
	return r.status[id]
}

// Results returns a copy of all task results.
func (r *Runner) Results() map[int]TaskResult {
	out := make(map[int]TaskResult, len(r.results))
	for k, v := range r.results {
		out[k] = v
	}
	return out
}

// AllDone returns true when all tasks are done, skipped, failed, or unrecoverable.
func (r *Runner) AllDone() bool {
	for _, t := range r.tasks {
		s := r.status[t.ID]
		if s == types.StatusPending || s == types.StatusRunning {
			return false
		}
	}
	return true
}

// Summary returns task counts by status.
func (r *Runner) Summary() (total, done, failed, skipped int) {
	total = len(r.tasks)
	for _, t := range r.tasks {
		switch r.status[t.ID] {
		case types.StatusDone:
			done++
		case types.StatusFailed, types.StatusUnrecoverable:
			failed++
		case types.StatusSkipped:
			skipped++
		}
	}
	return
}

// Tasks returns the current task list with updated statuses.
func (r *Runner) Tasks() []types.Task {
	out := make([]types.Task, len(r.tasks))
	for i, t := range r.tasks {
		t.Status = r.status[t.ID]
		out[i] = t
	}
	return out
}
