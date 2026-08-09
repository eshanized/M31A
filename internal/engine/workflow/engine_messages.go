package workflow

import (
	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
	"github.com/eshanized/M31A/internal/integrations/git"
)

// MsgEmitter is a callback interface for emitting events back to the TUI.
// Uses `any` to avoid coupling the workflow to the Bubble Tea framework.
type MsgEmitter interface {
	Emit(msg any)
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
	FilePath   string // populated for file-writing tools (FileWrite, Edit, FileDelete, FileMove)
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
	WorkflowMode        m31types.WorkflowMode // mode to use for subsequent transitions
	Output              any                   // structured output from the phase

	// Execution metrics (populated by Execute/Ship phases)
	Usage     *m31types.Usage  // token usage from LLM calls
	Cost      float64          // estimated cost
	ToolCalls int              // number of tool calls made
	Commits   []git.CommitInfo // commits created during phase
	DiffStats DiffStats        // file change statistics (Ship phase)

	// Manual verification steps from the plan (populated by Verify phase)
	ManualVerificationSteps []string

	// Demonstration content (populated by Ship phase)
	Demonstration string

	// Runtime verification results (populated by Runtime phase)
	RuntimeSummary *RuntimeSummary
}

// DemonstrationReadyMsg carries the generated demonstration content to the TUI.
type DemonstrationReadyMsg struct {
	Content string
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

// ResearchProgressMsg is emitted during the pre-plan research sub-step.
type ResearchProgressMsg struct {
	Phase    string
	Message  string
	Complete bool
}

// PlanCheckMsg is emitted after the plan checker reviews the plan.
type PlanCheckMsg struct {
	Passed     bool
	IssueCount int
	Blockers   int
	Warnings   int
}

// PlanRevisionMsg is emitted during each plan revision iteration.
type PlanRevisionMsg struct {
	Iteration       int
	MaxIterations   int
	IssuesRemaining int
}

// PlanChunkProgressMsg is emitted during chunked plan generation.
type PlanChunkProgressMsg struct {
	Wave        int
	TotalWaves  int
	TasksInWave int
}

// DiscussQualityMsg is emitted after the question quality checker runs.
type DiscussQualityMsg struct {
	Passed   bool
	Warnings int
	Retried  bool
}

// DiscussCompletenessMsg is emitted after the answer completeness check.
type DiscussCompletenessMsg struct {
	Score        int // 0-100
	MissingAreas []string
}

// ExecutePreflightMsg is emitted after pre-execution validation.
type ExecutePreflightMsg struct {
	Passed bool
	Issues []string
}

// ExecuteQualityGateMsg is emitted after per-task acceptance criteria check.
type ExecuteQualityGateMsg struct {
	TaskID  int
	Passed  bool
	Checked int
	Failed  int
}

// ExecuteLoopDetectMsg is emitted when a tool call loop is detected.
type ExecuteLoopDetectMsg struct {
	TaskID   int
	ToolName string
	Count    int
}

// VerifyReportMsg is emitted when the verification report is generated.
type VerifyReportMsg struct {
	Report   string
	PassRate int // percentage 0-100
}

// RuntimeCheckCompleteMsg is emitted when runtime smoke tests complete.
type RuntimeCheckCompleteMsg struct {
	Summary RuntimeSummary
}

// ShipPreflightMsg is emitted when the pre-ship checklist runs.
type ShipPreflightMsg struct {
	Passed bool
	Issues []string
}

// ShipChangelogMsg is emitted when the changelog is generated.
type ShipChangelogMsg struct {
	Content string
	Entries int
}

// InitAnalysisMsg is emitted when deep project analysis completes.
type InitAnalysisMsg struct {
	ProjectType     string
	Framework       string
	Language        string
	DependencyCount int
	TestFileRatio   float64
	FileCount       int
	HealthScore     int // 0-100
}

// InitPreflightMsg is emitted when environment pre-flight checks complete.
type InitPreflightMsg struct {
	Passed         bool
	RuntimeVersion string
	DiskSpaceOK    bool
	GitRemoteOK    bool
	Issues         []string
}

// VerificationResult holds the outcome of verifying a task.
type VerificationResult struct {
	TaskID     int
	FilesExist bool
	SyntaxOK   bool
	TestsOK    bool
	LintOK     bool
	Confidence float64 // 0.0 to 1.0, computed by computeConfidence
	Errors     []string
	Warnings   []string
}

// CompactionCompleteMsg is emitted when automatic session compaction completes.
type CompactionCompleteMsg struct {
	TokensBefore    int
	TokensAfter     int
	MessagesRemoved int
}

// TaskDiffSummaryMsg is emitted after a task commit to report file changes.
type TaskDiffSummaryMsg struct {
	TaskID  int
	Summary *m31types.DiffSummary
}

// DecisionsSnapshotMsg carries a snapshot of the decision log to the TUI.
type DecisionsSnapshotMsg struct {
	Decisions []decision.DecisionReceipt
}
