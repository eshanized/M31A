package sidebar


import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"context"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

const (
	sidebarDefaultWidth = 30
	sidebarMinWidth     = 20
	sidebarMaxWidth     = 50
)

// SidebarMode controls what the sidebar displays below the header sections.
type SidebarMode int

const (
	// SidebarModeFiles shows the git file tree (default full sidebar).
	SidebarModeFiles SidebarMode = iota
	// SidebarModeTodo shows the task progress and TODO list.
	SidebarModeTodo
	// SidebarModeIdle shows minimal orientation: branch, cost, task (2-3 lines).
	SidebarModeIdle
	// SidebarModeActive shows staged files + in-progress items (5-7 lines).
	SidebarModeActive
	// SidebarModeNarrative shows the narrative engine output.
	SidebarModeNarrative
)

// SidebarTaskProgress tracks overall task execution progress.
type SidebarTaskProgress struct {
	Total     int
	Done      int
	Failed    int
	Running   int
	Elapsed   time.Duration
	StartedAt time.Time
}

// SidebarToolCall represents a tool execution in the timeline.
type SidebarToolCall struct {
	Name        string
	Description string
	StartTime   time.Time
	Duration    time.Duration
	Success     bool
	Active      bool
}

// SidebarModel manages the collapsible sidebar panel that shows git status.
type SidebarModel struct {
	git     *git.Git
	theme   theme.Theme
	files   []git.FileStatus
	branch  string
	remote  string
	visible bool
	width   int
	height  int // total terminal height passed from AppState

	// File tree for navigation
	focused bool
	tree    *components.FileTree

	// Optional fields for enhanced display
	version   string
	sessionID string

	// Token usage
	totalTokens int
	contextLen  int
	cost        float64
	showCost    bool
	modelName   string

	loading bool

	// Periodic refresh
	shutdownCtx context.Context

	// Todo list mode
	mode         SidebarMode
	todoItems    []tuitypes.SidebarTodoItem
	taskProgress SidebarTaskProgress

	// Token burn rate tracking
	tokenBurnStart    time.Time
	tokenBurnTokens   int
	tokenBurnRate     float64 // tokens per second
	tokenBurnCostRate float64 // cost per second

	// Phase pipeline tracking
	currentPhase   string
	phaseHistory   []string // completed phases
	phaseStartedAt time.Time

	// Tool call timeline
	toolCalls    []SidebarToolCall
	maxToolCalls int // max visible in sidebar (default 5)

	// Execution speed metrics
	tasksPerMinute   float64
	avgTaskDuration  time.Duration
	estimatedTimeRem time.Duration

	// Context pressure
	contextPressure float64 // 0.0 to 1.0

	// Compaction event tracking (Wave 2C)
	lastCompactionBefore int // tokens before last compaction
	lastCompactionAfter  int // tokens after last compaction
	lastCompactionTime   time.Time

	// Pending permission requests (updated from dispatcher)
	pendingPermCount int

	// Cost accumulator
	totalCost    float64
	costTrend    float64 // positive = spending faster, negative = slower
	prevCostRate float64

	// Current screen for shortcut hints
	currentScreen string

	// Smart file watcher - files changed since last LLM read, with timestamps for TTL eviction
	recentlyChanged map[string]time.Time

	// Active sub-agents
	subAgentCount  int
	subAgentActive int

	// Narrative state
	narrativeState *NarrativeState
}

// NewSidebarModel creates a new SidebarModel.
func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
	root := &components.FileNode{Name: ".", Path: ".", IsDir: true}
	return &SidebarModel{
		git:     g,
		theme:   t,
		visible: true, // visible by default on terminals >= 80 columns (D-32)
		width:   sidebarDefaultWidth,
		tree:    components.NewFileTree(root, t, sidebarDefaultWidth-4, 10),
	}
}

// SetVersion sets the version string displayed in the header.
func (s *SidebarModel) SetVersion(v string) {
	s.version = v
}

// SetSessionID sets the active session ID for display.
func (s *SidebarModel) SetSessionID(id string) {
	s.sessionID = id
}

// SetTokenUsage updates the token usage display in the sidebar.
func (s *SidebarModel) SetTokenUsage(totalTokens, contextLen int, cost float64, showCost bool, modelName string) {
	s.totalTokens = totalTokens
	s.contextLen = contextLen
	s.cost = cost
	s.showCost = showCost
	s.modelName = modelName
}

// SetShutdownContext sets the context for the periodic refresh ticker.
func (s *SidebarModel) SetShutdownContext(ctx context.Context) {
	s.shutdownCtx = ctx
}

// SetNarrativeState sets the narrative state for the narrative sidebar mode.
func (s *SidebarModel) SetNarrativeState(ns *NarrativeState) {
	s.narrativeState = ns
}

// SetMode switches the sidebar between file tree and todo list display.
func (s *SidebarModel) SetMode(mode SidebarMode) {
	s.mode = mode
}

// GetMode returns the current sidebar display mode.
func (s *SidebarModel) GetMode() SidebarMode {
	return s.mode
}

// CycleMode cycles through sidebar modes: Idle → Active → Narrative → Files → Todo → Idle.
// Used by ctrl+g key binding.
func (s *SidebarModel) CycleMode() {
	switch s.mode {
	case SidebarModeIdle:
		s.mode = SidebarModeActive
	case SidebarModeActive:
		s.mode = SidebarModeNarrative
	case SidebarModeNarrative:
		s.mode = SidebarModeFiles
	case SidebarModeFiles:
		s.mode = SidebarModeTodo
	case SidebarModeTodo:
		s.mode = SidebarModeIdle
	default:
		s.mode = SidebarModeIdle
	}
}

// SetTodoItems sets the LLM-generated TODO items for display.
func (s *SidebarModel) SetTodoItems(items []tuitypes.SidebarTodoItem) {
	s.todoItems = items
}

// AddTodoItem adds or updates a single TODO item.
func (s *SidebarModel) AddTodoItem(item tuitypes.SidebarTodoItem) {
	for i, existing := range s.todoItems {
		if existing.Source == item.Source && existing.TaskID == item.TaskID && item.Source == "task" {
			s.todoItems[i] = item
			s.recalcTaskProgress()
			return
		}
		if existing.Source == item.Source && existing.Content == item.Content && item.Source == "llm" {
			s.todoItems[i] = item
			return
		}
	}
	s.todoItems = append(s.todoItems, item)
	s.recalcTaskProgress()
}

// recalcTaskProgress recomputes taskProgress counters from the current todoItems.
func (s *SidebarModel) recalcTaskProgress() {
	s.taskProgress.Total = 0
	s.taskProgress.Done = 0
	s.taskProgress.Failed = 0
	s.taskProgress.Running = 0
	for _, item := range s.todoItems {
		if item.Source != "task" {
			continue
		}
		s.taskProgress.Total++
		switch item.Status {
		case "completed":
			s.taskProgress.Done++
		case "failed":
			s.taskProgress.Failed++
		case "in_progress":
			s.taskProgress.Running++
		}
	}
	if !s.taskProgress.StartedAt.IsZero() {
		s.taskProgress.Elapsed = time.Since(s.taskProgress.StartedAt)
	}
}

// UpdateTaskProgress updates the progress from a task status change.
func (s *SidebarModel) UpdateTaskProgress(task types.Task) {
	s.taskProgress.Elapsed = time.Since(s.taskProgress.StartedAt)

	// Recount from todo items
	s.taskProgress.Total = 0
	s.taskProgress.Done = 0
	s.taskProgress.Failed = 0
	s.taskProgress.Running = 0

	for _, item := range s.todoItems {
		if item.Source != "task" {
			continue
		}
		s.taskProgress.Total++
		switch item.Status {
		case "completed":
			s.taskProgress.Done++
		case "failed":
			s.taskProgress.Failed++
		case "in_progress":
			s.taskProgress.Running++
		}
	}

	// If no todo items yet, use direct counts
	if s.taskProgress.Total == 0 {
		s.taskProgress.Total = task.ID // fallback
	}
}

// InitTaskProgress initializes the progress tracker when execution starts.
func (s *SidebarModel) InitTaskProgress(total int) {
	s.taskProgress = SidebarTaskProgress{
		Total:     total,
		StartedAt: time.Now(),
	}
}

// RevertToFiles switches the sidebar back to file tree mode and clears todo data.
func (s *SidebarModel) RevertToFiles() {
	s.mode = SidebarModeFiles
	s.todoItems = nil
	s.taskProgress = SidebarTaskProgress{}
}

// IsAllTasksDone returns true if all tracked tasks are completed or failed.
func (s *SidebarModel) IsAllTasksDone() bool {
	if s.taskProgress.Total == 0 {
		return false
	}
	return s.taskProgress.Done+s.taskProgress.Failed >= s.taskProgress.Total
}

// computeProgress derives progress counters from the current todo items list.
// This ensures the progress bar always matches what's visible in the TODO section.
func (s *SidebarModel) computeProgress() (total, done, failed, running int) {
	for _, item := range s.todoItems {
		total++
		switch item.Status {
		case "completed":
			done++
		case "failed":
			failed++
		case "in_progress":
			running++
		}
	}
	return
}

// ─── Token Burn Rate ──────────────────────────────────────────────────────────

// StartTokenBurn begins tracking token burn rate for streaming.
func (s *SidebarModel) StartTokenBurn() {
	s.tokenBurnStart = time.Now()
	s.tokenBurnTokens = s.totalTokens
}

// UpdateTokenBurn updates the burn rate based on current token count.
func (s *SidebarModel) UpdateTokenBurn() {
	if s.tokenBurnStart.IsZero() || s.totalTokens <= s.tokenBurnTokens {
		return
	}
	elapsed := time.Since(s.tokenBurnStart).Seconds()
	if elapsed <= 0 {
		return
	}
	tokensDiff := s.totalTokens - s.tokenBurnTokens
	s.tokenBurnRate = float64(tokensDiff) / elapsed
	if elapsed > 0 && s.cost > 0 {
		s.tokenBurnCostRate = s.cost / elapsed
	}
}

// GetTokenBurnRate returns tokens per second.
func (s *SidebarModel) GetTokenBurnRate() float64 {
	return s.tokenBurnRate
}

// ─── Phase Pipeline ───────────────────────────────────────────────────────────

// SetCurrentPhase updates the current workflow phase.
func (s *SidebarModel) SetCurrentPhase(phase string) {
	if s.currentPhase != "" && s.currentPhase != phase {
		s.phaseHistory = append(s.phaseHistory, s.currentPhase)
	}
	s.currentPhase = phase
	s.phaseStartedAt = time.Now()
}

// SetPendingPermCount updates the number of pending permission requests.
func (s *SidebarModel) SetPendingPermCount(n int) {
	s.pendingPermCount = n
}

// SetCompactionEvent records a compaction event for display in the sidebar.
func (s *SidebarModel) SetCompactionEvent(tokensBefore, tokensAfter int) {
	s.lastCompactionBefore = tokensBefore
	s.lastCompactionAfter = tokensAfter
	s.lastCompactionTime = time.Now()
}

// GetPhasePipeline returns the list of all phases for display.
// Keys are lowercase to match workflow.WorkflowPhase constants.
func (s *SidebarModel) GetPhasePipeline() []string {
	return []string{"initialize", "discuss", "plan", "execute", "verify", "runtime", "ship"}
}

// IsPhaseCompleted checks if a phase is in the history.
func (s *SidebarModel) IsPhaseCompleted(phase string) bool {
	for _, p := range s.phaseHistory {
		if p == phase {
			return true
		}
	}
	return false
}

// MarkPhaseCompleted explicitly marks the given phase as completed.
// Use this when a phase finishes without a subsequent phase transition
// (e.g. the final Ship phase, which has no successor to trigger the append
// in SetCurrentPhase).
func (s *SidebarModel) MarkPhaseCompleted(phase string) {
	if !s.IsPhaseCompleted(phase) {
		s.phaseHistory = append(s.phaseHistory, phase)
	}
}

// ─── Tool Call Timeline ───────────────────────────────────────────────────────

// SetMaxToolCalls sets the maximum number of tool calls to show in timeline.
func (s *SidebarModel) SetMaxToolCalls(max int) {
	s.maxToolCalls = max
	if s.maxToolCalls == 0 {
		s.maxToolCalls = 5
	}
}

// AddToolCallStart records a new tool call starting.
func (s *SidebarModel) AddToolCallStart(name, description string) {
	if description == "" || strings.HasPrefix(description, "Executing ") {
		description = toolCallAction(name)
	}
	tc := SidebarToolCall{
		Name:        name,
		Description: description,
		StartTime:   time.Now(),
		Active:      true,
	}
	s.toolCalls = append(s.toolCalls, tc)
	// Keep only the last N calls
	if len(s.toolCalls) > s.maxToolCalls {
		s.toolCalls = s.toolCalls[len(s.toolCalls)-s.maxToolCalls:]
	}
}

// CompleteToolCall marks the most recent matching tool call as complete.
func (s *SidebarModel) CompleteToolCall(name string, success bool, duration time.Duration) {
	for i := len(s.toolCalls) - 1; i >= 0; i-- {
		if s.toolCalls[i].Name == name && s.toolCalls[i].Active {
			s.toolCalls[i].Active = false
			s.toolCalls[i].Success = success
			s.toolCalls[i].Duration = duration
			return
		}
	}
}

// ─── Execution Speed Metrics ──────────────────────────────────────────────────

// UpdateExecutionMetrics recalculates speed metrics from task progress.
func (s *SidebarModel) UpdateExecutionMetrics() {
	if s.taskProgress.StartedAt.IsZero() || s.taskProgress.Total == 0 {
		return
	}
	elapsed := time.Since(s.taskProgress.StartedAt)
	s.taskProgress.Elapsed = elapsed
	completed := s.taskProgress.Done + s.taskProgress.Failed

	if completed > 0 && elapsed > 0 {
		s.tasksPerMinute = float64(completed) / elapsed.Minutes()
		s.avgTaskDuration = time.Duration(int64(elapsed) / int64(completed))
	}

	remaining := s.taskProgress.Total - completed
	if s.tasksPerMinute > 0 && remaining > 0 {
		s.estimatedTimeRem = time.Duration(float64(time.Minute) * float64(remaining) / s.tasksPerMinute)
	}
}

// ─── Context Pressure ─────────────────────────────────────────────────────────

// UpdateContextPressure updates the context window pressure gauge.
func (s *SidebarModel) UpdateContextPressure() {
	if s.contextLen > 0 {
		s.contextPressure = float64(s.totalTokens) / float64(s.contextLen)
		if s.contextPressure > 1.0 {
			s.contextPressure = 1.0
		}
	}
}

// ─── Cost Accumulator ─────────────────────────────────────────────────────────

// UpdateCostAccumulator updates the running cost total and trend.
func (s *SidebarModel) UpdateCostAccumulator() {
	s.totalCost = s.cost
	// Calculate trend based on burn rate vs previous
	if s.prevCostRate > 0 && s.tokenBurnCostRate > 0 {
		s.costTrend = s.tokenBurnCostRate - s.prevCostRate
	}
	s.prevCostRate = s.tokenBurnCostRate
}

// ─── tuitypes.Screen & Shortcuts ──────────────────────────────────────────────────────

// SetCurrentScreen updates the current screen for shortcut hints.
func (s *SidebarModel) SetCurrentScreen(screen string) {
	s.currentScreen = screen
}

// ─── Smart File Watcher ──────────────────────────────────────────────────────

// recentlyChangedTTL is the maximum age for a recently-changed file entry
// before it is evicted during the next MarkFileChanged or refresh cycle.
const recentlyChangedTTL = 10 * time.Minute

// MarkFileChanged marks a file as recently changed and evicts stale entries.
func (s *SidebarModel) MarkFileChanged(path string) {
	if s.recentlyChanged == nil {
		s.recentlyChanged = make(map[string]time.Time)
	}
	now := time.Now()
	s.recentlyChanged[path] = now
	// Evict entries older than the TTL
	for p, t := range s.recentlyChanged {
		if now.Sub(t) > recentlyChangedTTL {
			delete(s.recentlyChanged, p)
		}
	}
}

// ClearRecentlyChanged clears the recently changed files set.
func (s *SidebarModel) ClearRecentlyChanged() {
	s.recentlyChanged = make(map[string]time.Time)
}

// IsFileRecentlyChanged checks if a file was recently changed (within the TTL window).
func (s *SidebarModel) IsFileRecentlyChanged(path string) bool {
	t, ok := s.recentlyChanged[path]
	if !ok {
		return false
	}
	return time.Since(t) <= recentlyChangedTTL
}

// ─── Sub-Agent Status ────────────────────────────────────────────────────────

// UpdateSubAgentStatus updates the sub-agent count.
func (s *SidebarModel) UpdateSubAgentStatus(total, active int) {
	s.subAgentCount = total
	s.subAgentActive = active
}

// Toggle shows/hides the sidebar.
func (s *SidebarModel) Toggle() {
	s.visible = !s.visible
}

// IsVisible returns true if the sidebar is shown.
func (s *SidebarModel) IsVisible() bool {
	return s.visible
}

// IsFocused returns true if the sidebar has keyboard focus.
func (s *SidebarModel) IsFocused() bool {
	return s.focused
}

// Focus gives keyboard focus to the sidebar.
func (s *SidebarModel) Focus() {
	s.focused = true
}

// Blur removes keyboard focus from the sidebar.
func (s *SidebarModel) Blur() {
	s.focused = false
}

// ToggleFocus toggles sidebar keyboard focus.
func (s *SidebarModel) ToggleFocus() {
	s.focused = !s.focused
	if s.focused && s.tree != nil && s.tree.Cursor < 0 {
		s.tree.Cursor = 0
	}
}

// GetWidth returns the sidebar display width (0 if hidden).
func (s *SidebarModel) GetWidth() int {
	if !s.visible {
		return 0
	}
	return s.width
}

// SelectedFile returns the currently selected file, or nil if none.
func (s *SidebarModel) SelectedFile() *git.FileStatus {
	if s.tree == nil {
		return nil
	}
	node := s.tree.SelectedNode()
	if node == nil || node.IsDir {
		return nil
	}
	// Find the git status for this file
	for _, f := range s.files {
		if f.Path == node.Path {
			return &f
		}
	}
	return nil
}

// IncreaseWidth grows the sidebar width by 2, up to the max.
func (s *SidebarModel) IncreaseWidth() {
	if s.width+2 <= sidebarMaxWidth {
		s.width += 2
	}
}

// DecreaseWidth shrinks the sidebar width by 2, down to the min.
func (s *SidebarModel) DecreaseWidth() {
	if s.width-2 >= sidebarMinWidth {
		s.width -= 2
	}
}

// SetWidth sets the sidebar width, clamped to min/max.
func (s *SidebarModel) SetWidth(w int) {
	if w < sidebarMinWidth {
		w = sidebarMinWidth
	}
	if w > sidebarMaxWidth {
		w = sidebarMaxWidth
	}
	s.width = w
}

// SetTheme updates the sidebar theme.
func (s *SidebarModel) SetTheme(t theme.Theme) {
	s.theme = t
}

// SetHeight sets the total available height for the sidebar so it can
// constrain the file list and enable scrolling.
func (s *SidebarModel) SetHeight(h int) {
	s.height = h
	if s.tree != nil {
		treeHeight := h - sidebarFixedOverhead
		if treeHeight < 3 {
			treeHeight = 3
		}
		s.tree.Height = treeHeight
	}
}

// HandleKey processes a key event when the sidebar is focused.
// Returns a command to execute (e.g., show diff) or nil.
func (s *SidebarModel) HandleKey(msg tea.KeyMsg) tea.Cmd {
	if s.tree == nil {
		return nil
	}
	switch msg.String() {
	case "up", "k":
		s.tree.MoveCursor(-1)
	case "down", "j":
		s.tree.MoveCursor(1)
	case "home", "g":
		s.tree.Cursor = 0
	case "end", "G":
		if list := s.tree.FlatList(); len(list) > 0 {
			s.tree.Cursor = len(list) - 1
		}
	case "enter", " ":
		node := s.tree.SelectedNode()
		if node != nil && !node.IsDir {
			return s.showFileDiff(node)
		}
		s.tree.Toggle()
	case "esc":
		s.focused = false
	}
	return nil
}

// showFileDiff returns a command that fetches the diff for the given file node.
func (s *SidebarModel) showFileDiff(node *components.FileNode) tea.Cmd {
	g := s.git
	filePath := node.Path
	fileStatus := node.Status
	return func() tea.Msg {
		if g == nil || !g.IsRepo() {
			return nil
		}
		diff, err := g.DiffFile(filePath, fileStatus)
		if err != nil || strings.TrimSpace(diff) == "" {
			return nil
		}
		return DiffScreenMsg{
			Diff:  diff,
			Title: "diff — " + filePath,
		}
	}
}

// sidebarFixedOverhead is the number of non-file-list lines in the sidebar.
// Layout: header(1) + sep(1) + git(≈3) + usage(≈5) + files-label(1) + session(≈2) = ~13 lines.
const sidebarFixedOverhead = 13

// refreshCmd returns a tea.Cmd that loads git status asynchronously.
func (s *SidebarModel) refreshCmd() tea.Cmd {
	g := s.git
	return func() tea.Msg {
		if g == nil || !g.IsRepo() {
			return tuitypes.SidebarRefreshMsg{}
		}
		branch, _ := g.CurrentBranch()
		remote, _ := g.RemoteTracking()
		files, err := g.StatusPorcelain()
		if err != nil {
			return tuitypes.SidebarRefreshMsg{Branch: branch, Remote: remote}
		}
		sidebarFiles := make([]tuitypes.SidebarFile, 0, len(files))
		for _, f := range files {
			sidebarFiles = append(sidebarFiles, tuitypes.SidebarFile{
				Path:   f.Path,
				Status: f.Status,
			})
		}
		return tuitypes.SidebarRefreshMsg{Files: sidebarFiles, Branch: branch, Remote: remote}
	}
}

// Update handles sidebar-specific messages.
func (s *SidebarModel) Update(msg tea.Msg) (*SidebarModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tuitypes.SidebarRefreshMsg:
		s.branch = msg.Branch
		s.remote = msg.Remote
		files := make([]git.FileStatus, 0, len(msg.Files))
		for _, f := range msg.Files {
			files = append(files, git.FileStatus{Status: f.Status, Path: f.Path})
		}
		s.files = files
		// Rebuild the tree
		root := buildSidebarTree(files)
		s.tree.Root = root
		s.tree.ExpandAll()
		s.loading = false
		return s, nil

	case tuitypes.SidebarRefreshTickMsg:
		if s.visible && s.shutdownCtx != nil {
			select {
			case <-s.shutdownCtx.Done():
				return s, nil
			default:
			}
			return s, tea.Batch(s.refreshCmd(), NextSidebarRefreshTick(s.shutdownCtx, tuitypes.SidebarRefreshInterval))
		}
		return s, nil
	}

	if s.visible && s.branch == "" && !s.loading {
		s.loading = true
		return s, s.refreshCmd()
	}

	if s.visible && s.shutdownCtx != nil && !s.loading {
		return s, NextSidebarRefreshTick(s.shutdownCtx, tuitypes.SidebarRefreshInterval)
	}

	return s, nil
}

// HandleMouse processes mouse events for the sidebar.
// Returns a tea.Cmd if a file was clicked (to show diff), or nil.
func (s *SidebarModel) HandleMouse(msg tea.MouseMsg, sidebarX int) tea.Cmd {
	if !s.visible || !s.focused {
		return nil
	}

	if msg.Action != tea.MouseActionPress || msg.Button != tea.MouseButtonLeft {
		return nil
	}

	// Check if click is within the sidebar's X range
	if msg.X < sidebarX || msg.X >= sidebarX+s.width {
		return nil
	}

	// Convert absolute Y to sidebar-relative Y (account for header)
	// The sidebar starts at Y=0 in the terminal. The file tree starts after
	// the header sections. We need to figure out which tree item was clicked.
	if s.tree == nil {
		return nil
	}

	// Count header lines to find where the file tree starts
	headerLines := s.countHeaderLines()
	relativeY := msg.Y - headerLines
	if relativeY < 0 {
		return nil
	}

	// Map click to tree cursor
	flatList := s.tree.FlatList()
	if relativeY >= len(flatList) {
		return nil
	}

	s.tree.Cursor = relativeY
	node := s.tree.SelectedNode()
	if node != nil && !node.IsDir {
		return s.showFileDiff(node)
	}
	if node != nil && node.IsDir {
		s.tree.Toggle()
	}

	return nil
}

// countHeaderLines returns the number of lines before the file tree section.
func (s *SidebarModel) countHeaderLines() int {
	n := 1 // header (M31A + version)
	n++    // separator
	if s.branch != "" {
		n++ // branch line
		modCount, addCount, delCount, untracked := countFileStatuses(s.files)
		if modCount+addCount+delCount+untracked > 0 {
			n++ // status pills
		}
	}
	if s.totalTokens > 0 {
		n += 4 // USAGE label + meter + tokens + cost/model (approximate)
	}
	n++ // blank line before FILES
	n++ // FILES label
	return n
}

func buildSidebarTree(files []git.FileStatus) *components.FileNode {
	root := &components.FileNode{
		Name:  ".",
		Path:  ".",
		IsDir: true,
	}

	// Per-level index for O(1) child lookup instead of O(C) linear scan (TU-6 fix).
	childIndex := map[*components.FileNode]map[string]*components.FileNode{
		root: {},
	}

	for _, f := range files {
		parts := strings.Split(f.Path, "/")
		current := root

		for i, part := range parts {
			isLast := i == len(parts)-1
			nodePath := strings.Join(parts[:i+1], "/")

			index := childIndex[current]
			child, found := index[part]

			if !found {
				child = &components.FileNode{
					Name:  part,
					Path:  nodePath,
					IsDir: !isLast,
				}
				current.Children = append(current.Children, child)
				index[part] = child
				childIndex[child] = map[string]*components.FileNode{}
			}

			if isLast {
				child.Status = f.Status
			}

			current = child
		}
	}

	return root
}
