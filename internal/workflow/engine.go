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
}

// NewEngine creates a workflow engine.
func NewEngine(sessionID, workDir, backupDir, planningDir string, p provider.LLMProvider, modelID string,
	dispatcher *tools.Dispatcher, tokenEst *tokens.Estimator, sessionMgr *session.Manager) *Engine {

	prompts, err := LoadPrompts()
	if err != nil {
		panic(fmt.Sprintf("failed to load prompts: %v", err))
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
	}
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
		e.logger.Error("failed to save checkpoint", "error", err)
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
		if err == io.EOF {
			break
		}
		if err != nil {
			return sb.String(), err
		}
		if chunk != nil {
			sb.WriteString(chunk.Delta)
		}
	}
	return sb.String(), nil
}

// streamLLM sends a chat request and returns the full response content.
func (e *Engine) streamLLM(ctx context.Context, messages []m31types.Message, toolsEnabled bool) (string, error) {
	req := provider.ChatRequest{
		Model:    e.modelID,
		Messages: messages,
		Stream:   false,
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
	// Remove ```json ... ``` blocks
	re := regexp.MustCompile("(?s)```(?:json)?\\s*\n(.*?)```")
	return re.ReplaceAllString(content, "$1")
}

// extractJSONArray tries to find a JSON array in the content.
func extractJSONArray(content string) string {
	content = strings.TrimSpace(content)

	// If content starts with [, try to parse directly
	if strings.HasPrefix(content, "[") {
		return content
	}

	// Find first [ and last ]
	first := strings.Index(content, "[")
	last := strings.LastIndex(content, "]")
	if first >= 0 && last > first {
		return content[first : last+1]
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

// hasCycle detects circular dependencies using DFS.
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

	var dfs func(int) bool
	dfs = func(id int) bool {
		if inStack[id] {
			return true
		}
		if visited[id] {
			return false
		}
		visited[id] = true
		inStack[id] = true

		for _, dep := range adj[id] {
			if dfs(dep) {
				return true
			}
		}

		inStack[id] = false
		return false
	}

	for _, t := range tasks {
		if dfs(t.ID) {
			return true
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
func parseToolCalls(content string) []m31types.ToolCall {
	var calls []m31types.ToolCall

	// Pattern 1: JSON in code blocks ```<any lang> {...} ```
	blockRe := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
	for _, match := range blockRe.FindAllStringSubmatch(content, -1) {
		if len(match) < 2 {
			continue
		}
		if tc := parseSingleToolCall(match[1]); tc != nil {
			calls = append(calls, *tc)
		}
	}

	// Pattern 2: Try to find standalone JSON objects with tool call fields
	if len(calls) == 0 {
		// Look for objects with "name" field
		objRe := regexp.MustCompile(`\{[^{}]*"name"\s*:\s*"([^"]+)"[^{}]*\}`)
		for _, match := range objRe.FindAllStringSubmatch(content, -1) {
			if len(match) < 2 {
				continue
			}
			// Try to parse the full object
			jsonStart := strings.Index(content, match[0])
			if jsonStart == -1 {
				continue
			}
			// Find the matching JSON object
			obj := extractJSONObject(content[jsonStart:])
			if obj == "" {
				continue
			}
			if tc := parseSingleToolCall(obj); tc != nil {
				calls = append(calls, *tc)
			}
		}
	}

	return calls
}

// toolCallJSON represents a tool call in JSON format.
type toolCallJSON struct {
	Name  string          `json:"name"`
	Tool  string          `json:"tool"`
	Input json.RawMessage `json:"input"`
}

func parseSingleToolCall(jsonStr string) *m31types.ToolCall {
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
		ID:    fmt.Sprintf("call_%s_%d", name, time.Now().UnixNano()),
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

// listCwdFiles returns a list of files in the working directory with sizes.
func listCwdFiles(workDir string) string {
	var sb strings.Builder
	filepath.Walk(workDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		rel, _ := filepath.Rel(workDir, path)
		if strings.HasPrefix(rel, ".git") || strings.HasPrefix(rel, ".m31a") {
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

	// Syntax validation for Go files
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

	// Test execution
	if hasTestFiles(e.workDir, task.Files) {
		cmd := e.execCommand("go", "test", "./...")
		cmd.Dir = e.workDir
		if out, err := cmd.CombinedOutput(); err != nil {
			result.Errors = append(result.Errors, fmt.Sprintf("go test failed: %s", string(out)))
			result.TestsOK = false
		}
	}

	return result
}
