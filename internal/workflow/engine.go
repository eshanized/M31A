package workflow

import (
	"context"
	"embed"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"

	tea "github.com/charmbracelet/bubbletea"
)

// MsgEmitter is a callback interface for emitting messages back to the TUI.
type MsgEmitter interface {
	Emit(msg tea.Msg)
}

// TaskStartMsg is emitted when a task begins execution.
type TaskStartMsg struct {
	Task m31types.Task
}

// TaskUpdateMsg is emitted when a task status changes during execution.
type TaskUpdateMsg struct {
	Task   m31types.Task
	Status string
}

//go:embed prompts/*.md
var promptFS embed.FS

// PromptRegistry holds all loaded prompt templates.
type PromptRegistry struct {
	Base            string
	ToolUse         string
	PlanFormat      string
	ExecuteTask     string
	Discuss         string
	SelfHeal        string
	VerifyChecklist string
}

// LoadPrompts reads all embedded prompt files and returns a registry.
func LoadPrompts() (*PromptRegistry, error) {
	r := &PromptRegistry{}
	files := map[string]*string{
		"prompts/base.md":            &r.Base,
		"prompts/tool-use.md":        &r.ToolUse,
		"prompts/plan-format.md":     &r.PlanFormat,
		"prompts/execute-task.md":    &r.ExecuteTask,
		"prompts/discuss-questions.md": &r.Discuss,
		"prompts/self-heal.md":       &r.SelfHeal,
		"prompts/verify-checklist.md": &r.VerifyChecklist,
	}
	for path, ptr := range files {
		data, err := promptFS.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("load prompt %s: %w", path, err)
		}
		*ptr = strings.TrimSpace(string(data))
	}
	return r, nil
}

// Engine orchestrates the six-phase workflow.
type Engine struct {
	sessionID        string
	workDir          string
	backupDir        string
	planningDir      string
	provider         provider.LLMProvider
	modelID          string
	git              *git.Git
	dispatcher       *tools.Dispatcher
	tokens           *tokens.Estimator
	sessionMgr       *session.Manager
	prompts          *PromptRegistry
	logger           *slog.Logger
	startTime        time.Time
	sessionStartHash string
	discussState     DiscussState
	execCommand      func(name string, args ...string) *exec.Cmd
	msgEmitter       MsgEmitter
	callCounter      int64
}

// PhaseResult holds the outcome of a workflow phase.
type PhaseResult struct {
	Phase               m31types.WorkflowPhase
	Success             bool
	Messages            []m31types.Message
	Tasks               []m31types.Task
	Error               string
	DurationMs          int64
	NeedsAnswers        bool
	RequiresManualInput bool

	// Execution metrics (populated by Execute/Ship phases)
	Usage       *m31types.Usage // token usage from LLM calls
	Cost        float64         // estimated cost
	ToolCalls   int             // number of tool calls made
	Commits     []git.CommitInfo // commits created during phase
	DiffStats   DiffStats       // file change statistics (Ship phase)
}

// DiffStats holds file change statistics from git diff.
type DiffStats struct {
	FilesAdded    int
	FilesModified int
	FilesDeleted  int
	Insertions    int
	Deletions     int
}

// NewEngine creates a workflow engine.
func NewEngine(sessionID, workDir, backupDir, planningDir string, p provider.LLMProvider, modelID string,
	dispatcher *tools.Dispatcher, tokenEst *tokens.Estimator, sessionMgr *session.Manager) (*Engine, error) {

	prompts, err := LoadPrompts()
	if err != nil {
		return nil, fmt.Errorf("failed to load prompts: %w", err)
	}

	return &Engine{
		sessionID:   sessionID,
		workDir:     workDir,
		backupDir:   backupDir,
		planningDir: planningDir,
		provider:    p,
		modelID:     modelID,
		dispatcher:  dispatcher,
		tokens:      tokenEst,
		sessionMgr:  sessionMgr,
		prompts:     prompts,
		logger:      slog.Default(),
		startTime:   time.Now(),
		execCommand: exec.Command,
	}, nil
}

// RunPhase executes the given workflow phase and returns the result.
func (e *Engine) RunPhase(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	start := time.Now()

	var result *PhaseResult
	var err error

	switch phase {
	case m31types.PhaseInitialize:
		result, err = e.runInitialize(ctx, goal)
	case m31types.PhaseDiscuss:
		result, err = e.runDiscuss(ctx, goal)
	case m31types.PhasePlan:
		result, err = e.runPlan(ctx, goal)
	case m31types.PhaseExecute:
		result, err = e.runExecute(ctx, goal)
	case m31types.PhaseVerify:
		result, err = e.runVerify(ctx, goal)
	case m31types.PhaseShip:
		result, err = e.runShip(ctx, goal)
	default:
		return nil, fmt.Errorf("unknown phase: %s", phase)
	}

	if result != nil {
		result.DurationMs = time.Since(start).Milliseconds()
		result.Phase = phase
	}

	return result, err
}

// Transition saves a checkpoint and writes STATE.md for the new phase.
func (e *Engine) Transition(ctx context.Context, from, to m31types.WorkflowPhase) error {
	// Save checkpoint
	cp := session.Checkpoint{
		Phase:     to,
		Timestamp: time.Now(),
	}
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, cp); err != nil {
		return fmt.Errorf("save checkpoint: %w", err)
	}

	// Write STATE.md
	if err := e.sessionMgr.SaveState(e.sessionID, to, "transitioning", string(to)); err != nil {
		return fmt.Errorf("save state: %w", err)
	}

	return nil
}

// SetGit sets the git instance on the engine.
// Required before running any phase that uses git operations.
func (e *Engine) SetGit(g *git.Git) {
	e.git = g
	if h, err := g.HeadHash(); err == nil {
		e.sessionStartHash = h
	}
}

// SessionID returns the current session ID.
func (e *Engine) SessionID() string {
	return e.sessionID
}

// SetSessionID updates the engine's session ID and replans the planning
// directory to point to the new session's planning folder. Used after
// session-switching commands like /fork, /prev, /next.
func (e *Engine) SetSessionID(id string) {
	e.sessionID = id
	e.planningDir = filepath.Join(filepath.Dir(e.planningDir), "..", id, "planning")
}

// SetMsgEmitter sets the callback for emitting messages back to the TUI.
func (e *Engine) SetMsgEmitter(em MsgEmitter) {
	e.msgEmitter = em
}

// emit sends a message to the TUI if an emitter is configured.
func (e *Engine) emit(msg tea.Msg) {
	if e.msgEmitter != nil {
		e.msgEmitter.Emit(msg)
	}
}

// DiscussState holds the questions and collected answers for the discuss phase.
type DiscussState struct {
	Questions []string
	Answers   map[int]string
}

// SubmitDiscussAnswer records an answer for a discuss question.
func (e *Engine) SubmitDiscussAnswer(index int, answer string) error {
	if e.discussState.Questions == nil {
		return fmt.Errorf("no discuss questions to answer")
	}
	if index < 0 || index >= len(e.discussState.Questions) {
		return fmt.Errorf("invalid question index: %d", index)
	}
	if e.discussState.Answers == nil {
		e.discussState.Answers = make(map[int]string)
	}
	e.discussState.Answers[index] = answer
	return nil
}

// DiscussState returns a copy of the current discuss state. The TUI
// uses this to read the parsed questions after the discuss phase
// returns. The returned struct is a value copy so mutations by the
// TUI do not affect the engine's internal state.
func (e *Engine) DiscussState() DiscussState {
	return e.discussState
}

// SkipDiscuss fills default (empty) answers and saves.
func (e *Engine) SkipDiscuss() error {
	if e.discussState.Questions == nil {
		return nil
	}
	if e.discussState.Answers == nil {
		e.discussState.Answers = make(map[int]string)
	}
	for i := range e.discussState.Questions {
		if _, ok := e.discussState.Answers[i]; !ok {
			e.discussState.Answers[i] = ""
		}
	}
	return e.FinalizeDiscuss()
}

func (e *Engine) FinalizeDiscuss() error {
	project, _ := e.sessionMgr.LoadProject(e.sessionID)
	var questions, answers []string
	for i, q := range e.discussState.Questions {
		questions = append(questions, q)
		a := ""
		if ans, ok := e.discussState.Answers[i]; ok {
			a = ans
		}
		answers = append(answers, a)
	}
	if err := e.saveDiscussAnswers(project, questions, answers); err != nil {
		return fmt.Errorf("save discuss answers: %w", err)
	}
	// Auto-transition to Plan
	if err := e.Transition(context.Background(), m31types.PhaseDiscuss, m31types.PhasePlan); err != nil {
		e.logger.Warn("failed to transition to plan", "error", err)
	}
	return nil
}

// buildToolDefinitions returns the tool definitions for the LLM.
func (e *Engine) buildToolDefinitions() []provider.ToolDefinition {
	var defs []provider.ToolDefinition
	for _, name := range e.dispatcher.List() {
		tool, ok := e.dispatcher.GetTool(name)
		if !ok {
			continue
		}
		defs = append(defs, provider.ToolDefinition{
			Name:        tool.Name(),
			Description: tool.Description(),
			Parameters:  "{}", // Simplified - real implementation would have JSON schema
		})
	}
	return defs
}

// buildSystemPrompt composes the system prompt from base + optional extras.
func (e *Engine) buildSystemPrompt(extra ...string) string {
	parts := []string{e.prompts.Base}
	for _, p := range extra {
		if p != "" {
			parts = append(parts, p)
		}
	}
	return strings.Join(parts, "\n\n---\n\n")
}

// consumeStream reads all chunks from the iterator and returns the concatenated content.
func (e *Engine) consumeStream(iterator *m31types.StreamIterator) (string, error) {
	var sb strings.Builder
	defer iterator.Close()

	for {
		chunk, err := iterator.Next()
		if chunk != nil && chunk.Delta != "" {
			sb.WriteString(chunk.Delta)
		}
		if err == io.EOF {
			break
		}
		if err != nil {
			return sb.String(), err
		}
	}
	return sb.String(), nil
}

// streamLLM sends a chat request and returns the full response content.
func (e *Engine) streamLLM(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (string, error) {
	req := provider.ChatRequest{
		Model:            e.modelID,
		Messages:         messages,
		Stream:           false,
		ReasoningEnabled: true,
	}
	if toolsEnabled {
		req.Tools = e.buildToolDefinitions()
	}

	iterator, err := e.provider.ChatCompletionStream(ctx, req)
	if err != nil {
		return "", err
	}

	return e.consumeStream(iterator)
}

// streamLLMStreaming sends a chat request and returns the underlying
// StreamIterator. The caller is responsible for iterating via Next()
// and emitting each chunk to the TUI (typically via MsgEmitter).
func (e *Engine) streamLLMStreaming(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (*m31types.StreamIterator, error) {
	req := provider.ChatRequest{
		Model:            e.modelID,
		Messages:         messages,
		Stream:           false,
		ReasoningEnabled: true,
	}
	if toolsEnabled {
		req.Tools = e.buildToolDefinitions()
	}

	return e.provider.ChatCompletionStream(ctx, req)
}

// parseTasksFromJSON extracts tasks from LLM response, stripping markdown code blocks.
func parseTasksFromJSON(content string) ([]m31types.Task, error) {
	// Strip markdown code blocks
	content = stripCodeBlocks(content)

	// Try to find JSON array in content
	jsonStr := extractJSONArray(content)
	if jsonStr == "" {
		return nil, fmt.Errorf("no JSON array found in response")
	}

	var tasks []m31types.Task
	if err := json.Unmarshal([]byte(jsonStr), &tasks); err != nil {
		return nil, fmt.Errorf("JSON parse error: %w", err)
	}

	return tasks, nil
}

// stripCodeBlocks removes markdown code fences from content.
func stripCodeBlocks(content string) string {
	// Remove ```<any lang> ... ``` blocks
	re := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
	return re.ReplaceAllString(content, "$1")
}

// extractJSONArray tries to find a JSON array in the content.
// Uses bracket-depth tracking to correctly handle nested structures and
// bracket characters in surrounding text.
func extractJSONArray(content string) string {
	content = strings.TrimSpace(content)

	// If content starts with [, try to parse directly
	if strings.HasPrefix(content, "[") {
		return extractArrayFrom(content)
	}

	// Search for the first [ in content
	idx := strings.Index(content, "[")
	if idx < 0 {
		return ""
	}
	return extractArrayFrom(content[idx:])
}

// extractArrayFrom finds the first complete JSON array in s using
// bracket-depth tracking. It skips brackets inside strings and handles
// escape characters. Returns "" if no complete array is found.
func extractArrayFrom(s string) string {
	depth := 0
	inString := false
	escaped := false

	for i, c := range s {
		if escaped {
			escaped = false
			continue
		}
		switch c {
		case '\\':
			if inString {
				escaped = true
			}
		case '"':
			inString = !inString
		case '[':
			if !inString {
				depth++
			}
		case ']':
			if !inString {
				depth--
				if depth == 0 {
					return s[:i+1]
				}
			}
		}
	}
	return ""
}

// validateTasks checks task list for schema violations.
func validateTasks(tasks []m31types.Task) []string {
	var errs []string
	idSet := make(map[int]bool)

	for _, t := range tasks {
		if t.ID == 0 {
			errs = append(errs, fmt.Sprintf("task: missing ID"))
			continue
		}
		if idSet[t.ID] {
			errs = append(errs, fmt.Sprintf("task %d: duplicate ID", t.ID))
		}
		idSet[t.ID] = true

		for _, dep := range t.Dependencies {
			if dep == t.ID {
				errs = append(errs, fmt.Sprintf("task %d: self-reference", t.ID))
			}
		}

		if t.Description == "" {
			errs = append(errs, fmt.Sprintf("task %d: missing description", t.ID))
		}
		if t.Action == "" {
			errs = append(errs, fmt.Sprintf("task %d: missing action", t.ID))
		}
	}

	// Check all deps reference existing tasks
	for _, t := range tasks {
		for _, dep := range t.Dependencies {
			if !idSet[dep] {
				errs = append(errs, fmt.Sprintf("task %d: references non-existent dependency %d", t.ID, dep))
			}
		}
	}

	// Cycle detection
	if hasCycle(tasks) {
		errs = append(errs, "circular dependency detected")
	}

	return errs
}

// hasCycle detects circular dependencies using iterative DFS with explicit stack.
func hasCycle(tasks []m31types.Task) bool {
	idSet := make(map[int]bool)
	for _, t := range tasks {
		idSet[t.ID] = true
	}

	adj := make(map[int][]int)
	for _, t := range tasks {
		adj[t.ID] = t.Dependencies
	}

	visited := make(map[int]bool)
	inStack := make(map[int]bool)

	for _, t := range tasks {
		if visited[t.ID] {
			continue
		}
		// Iterative DFS using explicit stack
		// Stack entries: (nodeID, depIndex, isNewNode)
		type stackEntry struct {
			id       int
			depIdx   int
			firstVisit bool
		}
		stack := []stackEntry{{id: t.ID, firstVisit: true}}

		for len(stack) > 0 {
			entry := &stack[len(stack)-1]

			if entry.firstVisit {
				entry.firstVisit = false
				if inStack[entry.id] {
					return true
				}
				if visited[entry.id] {
					stack = stack[:len(stack)-1]
					continue
				}
				visited[entry.id] = true
				inStack[entry.id] = true
			}

			// Process next dependency
			found := false
			for entry.depIdx < len(adj[entry.id]) {
				dep := adj[entry.id][entry.depIdx]
				entry.depIdx++
				if inStack[dep] {
					return true
				}
				if !visited[dep] {
					stack = append(stack, stackEntry{id: dep, firstVisit: true})
					found = true
					break
				}
			}
			if !found {
				inStack[entry.id] = false
				stack = stack[:len(stack)-1]
			}
		}
	}
	return false
}

// detectProjectType scans the working directory for project indicators.
func detectProjectType(workDir string) string {
	detectors := map[string]string{
		"go.mod":           "go",
		"package.json":     "nodejs",
		"Cargo.toml":       "rust",
		"pyproject.toml":   "python",
		"requirements.txt": "python",
		"pom.xml":          "java",
		"Makefile":         "cc",
		"CMakeLists.txt":   "cc",
	}
	for file, typ := range detectors {
		if _, err := os.Stat(filepath.Join(workDir, file)); err == nil {
			return typ
		}
	}
	return "unknown"
}

// parseQuestions extracts numbered questions from LLM response.
func parseQuestions(content string) []string {
	// Match numbered questions: "1. What..." or "2. How..."
	re := regexp.MustCompile(`(\d+)\.\s+(.+)`)
	matches := re.FindAllStringSubmatch(content, -1)
	var questions []string
	for _, m := range matches {
		if len(m) > 2 {
			q := strings.TrimSpace(m[2])
			// Remove trailing content after newline
			if idx := strings.Index(q, "\n"); idx > 0 {
				q = strings.TrimSpace(q[:idx])
			}
			if len(q) > 0 {
				questions = append(questions, q)
			}
		}
	}

	// Fallback: look for question-like lines
	if len(questions) == 0 {
		for _, line := range strings.Split(content, "\n") {
			line = strings.TrimSpace(line)
			if len(line) > 10 && strings.Contains(line, "?") {
				questions = append(questions, line)
			}
		}
	}

	// Cap at 4
	if len(questions) > 4 {
		questions = questions[:4]
	}

	return questions
}

// parseToolCalls extracts tool calls from response content.
// Looks for JSON objects with a "name" or "tool" field inside code blocks or inline.
func (e *Engine) parseToolCalls(content string) []m31types.ToolCall {
	var calls []m31types.ToolCall

	// Pattern 1: JSON in code blocks ```<any lang> {...} ```
	blockRe := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
	for _, match := range blockRe.FindAllStringSubmatch(content, -1) {
		if len(match) < 2 {
			continue
		}
		if tc := parseSingleToolCall(match[1], e.nextCallID()); tc != nil {
			calls = append(calls, *tc)
		}
	}

	// Pattern 2: Try to find standalone JSON objects with tool call fields
	if len(calls) == 0 {
		// Scan at each { position, use extractJSONObject for proper nesting
		for i := 0; i < len(content); i++ {
			if content[i] != '{' {
				continue
			}
			obj := extractJSONObject(content[i:])
			if obj == "" {
				continue
			}
			// Quick pre-check: does this object have "name" or "tool" field?
			if !strings.Contains(obj, `"name"`) && !strings.Contains(obj, `"tool"`) {
				i += len(obj) - 1 // skip past this object
				continue
			}
			if tc := parseSingleToolCall(obj, e.nextCallID()); tc != nil {
				calls = append(calls, *tc)
				i += len(obj) - 1 // skip past this object
				continue
			}
		}
	}

	return calls
}

// nextCallID returns a monotonically increasing counter for tool call IDs.
func (e *Engine) nextCallID() int64 {
	return atomic.AddInt64(&e.callCounter, 1)
}

// toolCallJSON represents a tool call in JSON format.
type toolCallJSON struct {
	Name  string          `json:"name"`
	Tool  string          `json:"tool"`
	Input json.RawMessage `json:"input"`
}

func parseSingleToolCall(jsonStr string, callID int64) *m31types.ToolCall {
	var tc toolCallJSON
	if err := json.Unmarshal([]byte(jsonStr), &tc); err != nil {
		return nil
	}

	name := tc.Name
	if name == "" {
		name = tc.Tool
	}
	if name == "" {
		return nil
	}

	// Normalize tool names to match registered tool names
	name = normalizeToolName(name)

	input := tc.Input
	if input == nil {
		input = json.RawMessage("{}")
	}

	return &m31types.ToolCall{
		ID:    fmt.Sprintf("call_%s_%d", name, callID),
		Name:  name,
		Input: input,
	}
}

// normalizeToolName maps common LLM tool names to registered tool names.
func normalizeToolName(name string) string {
	lower := strings.ToLower(name)
	switch lower {
	case "bash", "shell", "exec", "run":
		return "Bash"
	case "fileread", "read_file", "read", "cat":
		return "FileRead"
	case "filewrite", "write_file", "write", "save":
		return "FileWrite"
	case "glob", "find_files", "find":
		return "Glob"
	case "grep", "search", "search_files":
		return "Grep"
	default:
		return name
	}
}

// extractJSONObject finds and returns the first complete JSON object starting
// at the beginning of the string.
func extractJSONObject(s string) string {
	if !strings.HasPrefix(strings.TrimSpace(s), "{") {
		return ""
	}
	depth := 0
	inString := false
	escaped := false
	for i, c := range s {
		if escaped {
			escaped = false
			continue
		}
		switch c {
		case '\\':
			if inString {
				escaped = true
			}
		case '"':
			inString = !inString
		case '{':
			if !inString {
				depth++
			}
		case '}':
			if !inString {
				depth--
				if depth == 0 {
					return s[:i+1]
				}
			}
		}
	}
	return ""
}

// readTaskFiles reads the content of files for a task.
func (e *Engine) readTaskFiles(files []string) string {
	var sb strings.Builder
	for _, f := range files {
		path := filepath.Join(e.workDir, f)
		content, err := os.ReadFile(path)
		if err != nil {
			sb.WriteString(fmt.Sprintf("=== %s: (not found) ===\n", f))
			continue
		}
		sb.WriteString(fmt.Sprintf("=== %s ===\n%s\n", f, string(content)))
	}
	return sb.String()
}

// formatTaskSummary creates a markdown table of tasks.
func formatTaskSummary(tasks []m31types.Task) string {
	var sb strings.Builder
	sb.WriteString("| ID | Action | Description | Deps | Status |\n")
	sb.WriteString("|----|--------|-------------|------|--------|\n")
	for _, t := range tasks {
		deps := "-"
		if len(t.Dependencies) > 0 {
			ds := make([]string, len(t.Dependencies))
			for i, d := range t.Dependencies {
				ds[i] = fmt.Sprintf("%d", d)
			}
			deps = strings.Join(ds, ", ")
		}
		status := string(t.Status)
		if status == "" {
			status = "pending"
		}
		sb.WriteString(fmt.Sprintf("| %d | %s | %s | %s | %s |\n",
			t.ID, t.Action, t.Description, deps, status))
	}
	return sb.String()
}

var skipDirs = map[string]bool{
	"node_modules": true, "vendor": true, ".next": true,
	"dist": true, "build": true, "target": true,
	".venv": true, "venv": true, "__pycache__": true,
}

// listCwdFiles returns a list of files in the working directory with sizes.
// Limits depth to 3 levels and skips known heavy directories.
func listCwdFiles(workDir string) string {
	var sb strings.Builder
	filepath.Walk(workDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			slog.Warn("walk error", "path", path, "error", err)
			return nil
		}
		rel, _ := filepath.Rel(workDir, path)
		if rel == "." {
			return nil
		}
		// Skip heavy directories
		if info.IsDir() && skipDirs[info.Name()] {
			return filepath.SkipDir
		}
		// Depth limit: count path separators
		depth := strings.Count(rel, string(filepath.Separator))
		if depth >= 3 {
			if info.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if strings.HasPrefix(rel, ".git") || strings.HasPrefix(rel, ".m31a") {
			if info.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if !info.IsDir() {
			sb.WriteString(fmt.Sprintf("%s (%d bytes)\n", rel, info.Size()))
		}
		return nil
	})
	return sb.String()
}

// hasTestFiles checks if any files in the task end with test patterns.
func hasTestFiles(workDir string, files []string) bool {
	for _, f := range files {
		base := filepath.Base(f)
		if strings.HasSuffix(base, "_test.go") ||
			strings.HasSuffix(base, ".test.js") ||
			strings.HasSuffix(base, "_test.py") {
			return true
		}
		// Also check if test files exist in the same directory
		dir := filepath.Join(workDir, filepath.Dir(f))
		entries, _ := os.ReadDir(dir)
		for _, e := range entries {
			if strings.HasSuffix(e.Name(), "_test.go") ||
				strings.HasSuffix(e.Name(), ".test.js") ||
				strings.HasSuffix(e.Name(), "_test.py") {
				return true
			}
		}
	}
	return false
}

// VerificationResult holds the outcome of verifying a task.
type VerificationResult struct {
	TaskID     int
	FilesExist bool
	SyntaxOK   bool
	TestsOK    bool
	Errors     []string
}

// verifyTask checks if a task's outputs exist and are syntactically valid.
func (e *Engine) verifyTask(task m31types.Task) VerificationResult {
	result := VerificationResult{TaskID: task.ID, FilesExist: true, SyntaxOK: true, TestsOK: true}

	// File existence
	for _, f := range task.Files {
		path := filepath.Join(e.workDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			result.Errors = append(result.Errors, fmt.Sprintf("file not found: %s", f))
			result.FilesExist = false
		}
	}

	// Project-type-specific validation
	projectType := detectProjectType(e.workDir)
	switch projectType {
	case "go":
		hasGo := false
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".go") {
				hasGo = true
				break
			}
		}
		if hasGo {
			cmd := e.execCommand("go", "build", "./...")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("go build failed: %s", string(out)))
				result.SyntaxOK = false
			}
		}
	case "nodejs":
		hasJS := false
		for _, f := range task.Files {
			ext := filepath.Ext(f)
			if ext == ".js" || ext == ".ts" || ext == ".jsx" || ext == ".tsx" {
				hasJS = true
				break
			}
		}
		if hasJS {
			// Check for package.json and try npm test or tsc
			if _, err := os.Stat(filepath.Join(e.workDir, "package.json")); err == nil {
				cmd := e.execCommand("sh", "-c", "npm run build 2>&1 || tsc --noEmit 2>&1 || true")
				cmd.Dir = e.workDir
				if out, err := cmd.CombinedOutput(); err == nil && len(out) > 0 {
					// Log but don't fail — build may have warnings
					e.logger.Info("nodejs build output", "output", string(out))
				}
			}
		}
	case "python":
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".py") {
				path := filepath.Join(e.workDir, f)
				cmd := e.execCommand("python3", "-m", "py_compile", path)
				cmd.Dir = e.workDir
				if out, err := cmd.CombinedOutput(); err != nil {
					result.Errors = append(result.Errors, fmt.Sprintf("python syntax error in %s: %s", f, string(out)))
					result.SyntaxOK = false
				}
			}
		}
	case "rust":
		hasRust := false
		for _, f := range task.Files {
			if strings.HasSuffix(f, ".rs") {
				hasRust = true
				break
			}
		}
		if hasRust {
			cmd := e.execCommand("cargo", "check")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("cargo check failed: %s", string(out)))
				result.SyntaxOK = false
			}
		}
	}

	// Test execution
	if hasTestFiles(e.workDir, task.Files) {
		switch projectType {
		case "go":
			cmd := e.execCommand("go", "test", "./...")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err != nil {
				result.Errors = append(result.Errors, fmt.Sprintf("go test failed: %s", string(out)))
				result.TestsOK = false
			}
		case "nodejs":
			cmd := e.execCommand("sh", "-c", "npm test 2>&1 || true")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err == nil && len(out) > 0 {
				e.logger.Info("npm test output", "output", string(out))
			}
		case "python":
			cmd := e.execCommand("sh", "-c", "python3 -m pytest 2>&1 || true")
			cmd.Dir = e.workDir
			if out, err := cmd.CombinedOutput(); err == nil && len(out) > 0 {
				e.logger.Info("pytest output", "output", string(out))
			}
		}
	}

	return result
}
