package workflow

import (
	"github.com/eshanized/M31A/internal/git"
	m31types "github.com/eshanized/M31A/internal/types"

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

// ToolStartMsg is emitted when a tool call begins execution.
type ToolStartMsg struct {
	ToolName    string
	Description string
}

// ToolCompleteMsg is emitted when a tool call finishes execution.
type ToolCompleteMsg struct {
	ToolName   string
	Success    bool
	DurationMs int64
	Error      string
}

// SelfHealStartMsg is emitted when a self-heal attempt begins.
type SelfHealStartMsg struct {
	TaskID  int
	Attempt int
	Max     int
}

// SelfHealCompleteMsg is emitted when a self-heal attempt finishes.
type SelfHealCompleteMsg struct {
	TaskID  int
	Attempt int
	Max     int
	Success bool
	Error   string
}

// PhaseTransitionStartMsg is emitted when a phase transition begins.
type PhaseTransitionStartMsg struct {
	From    string
	To      string
	Context string
}

// PhaseTransitionCompleteMsg is emitted when a phase transition completes.
type PhaseTransitionCompleteMsg struct {
	From    string
	To      string
	Success bool
	Error   string
}

// IntermediateProgressMsg is emitted during long operations to show progress.
type IntermediateProgressMsg struct {
	Phase   string
	Message string
}

// ThinkingStartMsg is emitted when LLM processing begins.
type ThinkingStartMsg struct {
	Context string
}

// ThinkingCompleteMsg is emitted when LLM processing ends.
type ThinkingCompleteMsg struct {
	Context string
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
	Usage     *m31types.Usage  // token usage from LLM calls
	Cost      float64          // estimated cost
	ToolCalls int              // number of tool calls made
	Commits   []git.CommitInfo // commits created during phase
	DiffStats DiffStats        // file change statistics (Ship phase)
}

// DiffStats holds file change statistics from git diff.
type DiffStats struct {
	FilesAdded    int
	FilesModified int
	FilesDeleted  int
	Insertions    int
	Deletions     int
}

// DiscussState holds the questions and collected answers for the discuss phase.
type DiscussState struct {
	Questions []string
	Answers   map[int]string
}

// VerificationResult holds the outcome of verifying a task.
type VerificationResult struct {
	TaskID     int
	FilesExist bool
	SyntaxOK   bool
	TestsOK    bool
	Errors     []string
}
