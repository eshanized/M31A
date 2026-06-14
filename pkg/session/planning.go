package session

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// PROJECT.md — goal, project type, framework, Q&A
// ---------------------------------------------------------------------------

// SaveProject writes ProjectState to planning/PROJECT.md for the given session.
func (m *Manager) SaveProject(sessionID string, project *types.ProjectState) error {
	var b strings.Builder

	b.WriteString("# Project\n\n")
	fmt.Fprintf(&b, "**Goal:** %s\n", project.Goal)
	fmt.Fprintf(&b, "**Type:** %s\n", project.ProjectType)
	fmt.Fprintf(&b, "**Framework:** %s\n", project.Framework)

	if len(project.Answers) > 0 {
		b.WriteString("\n## Questions\n\n")
		// Sort keys for deterministic output
		keys := make([]string, 0, len(project.Answers))
		for k := range project.Answers {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			fmt.Fprintf(&b, "- **Q:** %s → **A:** %s\n", k, project.Answers[k])
		}
	}

	planningDir := m.planningDirPath()
	if err := m.ensureDir(planningDir); err != nil {
		return fmt.Errorf("cannot create planning directory: %w", err)
	}

	path := filepath.Join(planningDir, "PROJECT.md")
	return m.atomicWrite(path, []byte(b.String()))
}

// LoadProject reads and parses planning/PROJECT.md for the given session.
// Returns nil without error if the file does not exist (graceful degradation).
func (m *Manager) LoadProject(sessionID string) (*types.ProjectState, error) {
	path := filepath.Join(m.planningDirPath(), "PROJECT.md")
	data, err := readFileLimited(path, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("cannot read PROJECT.md: %w", err)
	}

	project := &types.ProjectState{
		Answers: make(map[string]string),
	}

	scanner := bufio.NewScanner(strings.NewReader(string(data)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "|---") {
			continue
		}

		switch {
		case strings.HasPrefix(line, "**Goal:**"):
			project.Goal = strings.TrimSpace(line[len("**Goal:**"):])
		case strings.HasPrefix(line, "**Type:**"):
			project.ProjectType = strings.TrimSpace(line[len("**Type:**"):])
		case strings.HasPrefix(line, "**Framework:**"):
			project.Framework = strings.TrimSpace(line[len("**Framework:**"):])
		case strings.HasPrefix(line, "- **Q:**"):
			rest := strings.TrimPrefix(line, "- **Q:**")
			parts := strings.SplitN(rest, "→", 2)
			if len(parts) == 2 {
				q := strings.TrimSpace(parts[0])
				aStr := strings.TrimSpace(parts[1])
				aStr = strings.TrimPrefix(aStr, "**A:**")
				a := strings.TrimSpace(aStr)
				project.Answers[q] = a
			}
		}
	}

	return project, nil
}

// ---------------------------------------------------------------------------
// TASKS.md — markdown table
// ---------------------------------------------------------------------------

// SaveTasks writes a slice of Tasks to planning/TASKS.md as a markdown table.
func (m *Manager) SaveTasks(sessionID string, tasks []types.Task) error {
	var b strings.Builder

	b.WriteString("# Tasks\n\n")
	b.WriteString("| ID | Action | Description | Deps | Status | Files |\n")
	b.WriteString("|----|--------|-------------|------|--------|-------|\n")

	for _, task := range tasks {
		deps := "-"
		if len(task.Dependencies) > 0 {
			depStrs := make([]string, len(task.Dependencies))
			for i, d := range task.Dependencies {
				depStrs[i] = fmt.Sprintf("%d", d)
			}
			deps = strings.Join(depStrs, ", ")
		}

		files := "-"
		if len(task.Files) > 0 {
			files = strings.Join(task.Files, ", ")
		}

		fmt.Fprintf(&b, "| %d | %s | %s | %s | %s | %s |\n",
			task.ID, task.Action, task.Description, deps, string(task.Status), files)
	}

	planningDir := m.planningDirPath()
	if err := m.ensureDir(planningDir); err != nil {
		return fmt.Errorf("cannot create planning directory: %w", err)
	}

	path := filepath.Join(planningDir, "TASKS.md")
	return m.atomicWrite(path, []byte(b.String()))
}

// LoadTasks reads and parses planning/TASKS.md for the given session.
// Returns an empty slice without error if the file does not exist.
func (m *Manager) LoadTasks(sessionID string) ([]types.Task, error) {
	path := filepath.Join(m.planningDirPath(), "TASKS.md")
	data, err := readFileLimited(path, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return []types.Task{}, nil
		}
		return nil, fmt.Errorf("cannot read TASKS.md: %w", err)
	}

	var tasks []types.Task
	scanner := bufio.NewScanner(strings.NewReader(string(data)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "|---") {
			continue
		}

		// Skip the header row
		if strings.HasPrefix(line, "| ID |") {
			continue
		}

		// Must be a table row
		if !strings.HasPrefix(line, "|") || !strings.HasSuffix(line, "|") {
			continue
		}

		cells := parseTableRow(line)
		if len(cells) < 6 {
			continue
		}

		var id int
		if _, err := fmt.Sscan(cells[0], &id); err != nil {
			continue
		}

		task := types.Task{
			ID:           id,
			Action:       cells[1],
			Description:  cells[2],
			Dependencies: parseDeps(cells[3]),
			Status:       types.TaskStatus(cells[4]),
			Files:        parseFileList(cells[5]),
		}
		tasks = append(tasks, task)
	}

	return tasks, nil
}

// parseTableRow splits a markdown table row into its cell values.
func parseTableRow(line string) []string {
	// line has leading and trailing |
	inner := line[1 : len(line)-1]
	parts := strings.Split(inner, "|")
	cells := make([]string, len(parts))
	for i, p := range parts {
		cells[i] = strings.TrimSpace(p)
	}
	return cells
}

// parseDeps converts a comma-separated dependency string to []int.
func parseDeps(s string) []int {
	s = strings.TrimSpace(s)
	if s == "" || s == "-" {
		return nil
	}
	parts := strings.Split(s, ",")
	deps := make([]int, 0, len(parts))
	for _, p := range parts {
		var d int
		if _, err := fmt.Sscan(strings.TrimSpace(p), &d); err == nil {
			deps = append(deps, d)
		}
	}
	return deps
}

// parseFileList converts a comma-separated file list to []string.
// Empty strings from consecutive or trailing commas are filtered out.
func parseFileList(s string) []string {
	s = strings.TrimSpace(s)
	if s == "" || s == "-" {
		return nil
	}
	parts := strings.Split(s, ",")
	files := make([]string, 0, len(parts))
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			files = append(files, p)
		}
	}
	return files
}

// ---------------------------------------------------------------------------
// STATE.md — phase, progress, last action, timestamp
// ---------------------------------------------------------------------------

// SaveState writes workflow state to planning/STATE.md for the given session.
func (m *Manager) SaveState(sessionID string, phase types.WorkflowPhase, progress, lastAction string) error {
	var b strings.Builder

	b.WriteString("# State\n\n")
	fmt.Fprintf(&b, "**Phase:** %s\n", string(phase))
	fmt.Fprintf(&b, "**Progress:** %s\n", progress)
	fmt.Fprintf(&b, "**Last Action:** %s\n", lastAction)
	fmt.Fprintf(&b, "**Timestamp:** %s\n", time.Now().Format(time.RFC3339))

	planningDir := m.planningDirPath()
	if err := m.ensureDir(planningDir); err != nil {
		return fmt.Errorf("cannot create planning directory: %w", err)
	}

	path := filepath.Join(planningDir, "STATE.md")
	return m.atomicWrite(path, []byte(b.String()))
}

// LoadState reads and parses planning/STATE.md for the given session.
// Returns empty/default values without error if the file does not exist.
func (m *Manager) LoadState(sessionID string) (phase types.WorkflowPhase, progress, lastAction string, timestamp time.Time, err error) {
	path := filepath.Join(m.planningDirPath(), "STATE.md")
	data, readErr := readFileLimited(path, types.MaxSessionFileSize)
	if readErr != nil {
		if os.IsNotExist(readErr) {
			return types.PhaseIdle, "", "", time.Time{}, nil
		}
		return types.PhaseIdle, "", "", time.Time{}, fmt.Errorf("cannot read STATE.md: %w", readErr)
	}

	scanner := bufio.NewScanner(strings.NewReader(string(data)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		switch {
		case strings.HasPrefix(line, "**Phase:**"):
			phase = types.WorkflowPhase(strings.TrimSpace(line[len("**Phase:**"):]))
		case strings.HasPrefix(line, "**Progress:**"):
			progress = strings.TrimSpace(line[len("**Progress:**"):])
		case strings.HasPrefix(line, "**Last Action:**"):
			lastAction = strings.TrimSpace(line[len("**Last Action:**"):])
		case strings.HasPrefix(line, "**Timestamp:**"):
			ts := strings.TrimSpace(line[len("**Timestamp:**"):])
			var err error
			timestamp, err = time.Parse(time.RFC3339, ts)
			_ = err
		}
	}

	if phase == "" {
		phase = types.PhaseIdle
	}

	return
}

// ---------------------------------------------------------------------------
// plan.md — rich implementation plan (versioned)
// ---------------------------------------------------------------------------

// SavePlan writes the plan markdown to planning/plan.md and a versioned copy
// to planning/plan_v{version}.md for refinement history.
func (m *Manager) SavePlan(sessionID string, version int, markdown string) error {
	planningDir := m.planningDirPath()
	if err := m.ensureDir(planningDir); err != nil {
		return fmt.Errorf("cannot create planning directory: %w", err)
	}

	path := filepath.Join(planningDir, "plan.md")
	if err := m.atomicWrite(path, []byte(markdown)); err != nil {
		return err
	}

	if version > 0 {
		versionedPath := filepath.Join(planningDir, fmt.Sprintf("plan_v%d.md", version))
		return m.atomicWrite(versionedPath, []byte(markdown))
	}
	return nil
}

// LoadPlan reads planning/plan.md for the given session.
// Returns empty string without error if the file does not exist.
func (m *Manager) LoadPlan(sessionID string) (string, error) {
	path := filepath.Join(m.planningDirPath(), "plan.md")
	data, err := readFileLimited(path, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", fmt.Errorf("cannot read plan.md: %w", err)
	}
	return string(data), nil
}

// ---------------------------------------------------------------------------
// DEMONSTRATION.md — post-completion walkthrough
// ---------------------------------------------------------------------------

// SaveDemonstration writes the demonstration markdown to planning/DEMONSTRATION.md.
func (m *Manager) SaveDemonstration(sessionID string, markdown string) error {
	planningDir := m.planningDirPath()
	if err := m.ensureDir(planningDir); err != nil {
		return fmt.Errorf("cannot create planning directory: %w", err)
	}
	path := filepath.Join(planningDir, "DEMONSTRATION.md")
	return m.atomicWrite(path, []byte(markdown))
}

// LoadDemonstration reads planning/DEMONSTRATION.md for the given session.
// Returns empty string without error if the file does not exist.
func (m *Manager) LoadDemonstration(sessionID string) (string, error) {
	path := filepath.Join(m.planningDirPath(), "DEMONSTRATION.md")
	data, err := readFileLimited(path, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", fmt.Errorf("cannot read DEMONSTRATION.md: %w", err)
	}
	return string(data), nil
}

// ---------------------------------------------------------------------------
// tasks.md — checkbox-style task list grouped by category
// ---------------------------------------------------------------------------

// SaveTasksCheckbox writes a checkbox-style task list grouped by category
// to planning/tasks.md. Tasks are grouped by their Category field; tasks
// without a category go under "General".
func (m *Manager) SaveTasksCheckbox(sessionID string, tasks []types.Task) error {
	grouped := make(map[string][]types.Task)
	var categories []string
	seen := make(map[string]bool)
	for _, t := range tasks {
		cat := t.Category
		if cat == "" {
			cat = "General"
		}
		if !seen[cat] {
			seen[cat] = true
			categories = append(categories, cat)
		}
		grouped[cat] = append(grouped[cat], t)
	}

	var b strings.Builder
	b.WriteString("# Task List\n\n")
	for _, cat := range categories {
		allDone := true
		for _, t := range grouped[cat] {
			if t.Status != types.StatusDone && t.Status != types.StatusSkipped {
				allDone = false
				break
			}
		}
		check := " "
		if allDone {
			check = "x"
		}
		fmt.Fprintf(&b, "- [%s] **%s**\n", check, cat)
		for _, t := range grouped[cat] {
			taskCheck := " "
			if t.Status == types.StatusDone || t.Status == types.StatusSkipped {
				taskCheck = "x"
			}
			fmt.Fprintf(&b, "  - [%s] %s\n", taskCheck, t.Description)
		}
	}

	planningDir := m.planningDirPath()
	if err := m.ensureDir(planningDir); err != nil {
		return fmt.Errorf("cannot create planning directory: %w", err)
	}
	path := filepath.Join(planningDir, "tasks.md")
	return m.atomicWrite(path, []byte(b.String()))
}
