// Package narrative transforms low-level execution events into
// human-readable progress descriptions. It is a pure Go package with
// no Bubble Tea dependency, zero global state, and is fully deterministic.
package narrative

import "time"

// Category classifies a narrative into a workflow-related domain.
type Category int

const (
	CategoryOrienting   Category = iota // Where am I?
	CategoryUnderstanding              // Reading and analyzing
	CategoryDiscussing                  // Questions and answers
	CategoryPlanning                    // Breaking down work
	CategoryResearching                 // External information
	CategoryExecuting                   // Making changes
	CategoryVerifying                   // Tests and validation
	CategoryRecovering                  // Self-healing
	CategoryShipping                    // Committing and finalizing
	CategoryLearning                    // Context management
	CategoryAlerting                    // Errors and warnings
)

// String returns the human-readable name of the category.
func (c Category) String() string {
	switch c {
	case CategoryOrienting:
		return "Orienting"
	case CategoryUnderstanding:
		return "Understanding"
	case CategoryDiscussing:
		return "Discussing"
	case CategoryPlanning:
		return "Planning"
	case CategoryResearching:
		return "Researching"
	case CategoryExecuting:
		return "Executing"
	case CategoryVerifying:
		return "Verifying"
	case CategoryRecovering:
		return "Recovering"
	case CategoryShipping:
		return "Shipping"
	case CategoryLearning:
		return "Learning"
	case CategoryAlerting:
		return "Alerting"
	default:
		return "Unknown"
	}
}

// Priority establishes visual weight ordering. Lower number = higher priority.
type Priority int

const (
	PriorityActionRequired Priority = 1 // Approvals, questions
	PriorityPhase          Priority = 2 // Current phase indicator
	PriorityOrientation    Priority = 3 // Branch, project info
	PriorityTask           Priority = 4 // In-progress tasks
	PriorityCost           Priority = 5 // Cost display
	PriorityFileStatus     Priority = 6 // File change counts
	PriorityAgent          Priority = 7 // Sub-agent status
)

// Display determines where a narrative appears.
type Display int

const (
	DisplaySidebar      Display = iota // Sidebar only
	DisplayConversation               // Conversation only
	DisplayBoth                       // Sidebar and conversation
	DisplayToast                      // Transient toast notification
	DisplayHidden                     // Not displayed (internal)
)

// Classification is the output of the classifier for a single event.
type Classification int

const (
	ClassifyHidden    Classification = iota // Event is internal, not user-facing
	ClassifyNarrative                       // Event becomes a single narrative
	ClassifyGrouped                         // Event should be batched with similar events
	ClassifyExpanded                        // Event always shows with full detail
)

// EventType identifies the kind of raw event. These map to the 121 message
// types in M31A but use simplified string identifiers for decoupling.
type EventType string

const (
	// Phase events
	EventPhaseTransitionStart    EventType = "phase_transition_start"
	EventPhaseTransitionComplete EventType = "phase_transition_complete"

	// Task events
	EventTaskStart    EventType = "task_start"
	EventTaskComplete EventType = "task_complete"
	EventTaskFailed   EventType = "task_failed"
	EventTaskDiff     EventType = "task_diff"

	// Tool events
	EventToolStart    EventType = "tool_start"
	EventToolComplete EventType = "tool_complete"

	// Self-heal events
	EventSelfHealStart    EventType = "selfheal_start"
	EventSelfHealComplete EventType = "selfheal_complete"

	// Initialize events
	EventInitAnalysis EventType = "init_analysis"
	EventInitPreflight EventType = "init_preflight"

	// Plan events
	EventPlanRevision     EventType = "plan_revision"
	EventPlanChunk        EventType = "plan_chunk"
	EventPlanReady        EventType = "plan_ready"
	EventPlanRefine       EventType = "plan_refine"

	// Execute events
	EventExecutePreflight   EventType = "execute_preflight"
	EventExecuteLoopDetect  EventType = "execute_loop_detect"

	// Verify events
	EventVerifyReport EventType = "verify_report"
	EventVerifyTests  EventType = "verify_tests"

	// Ship events
	EventShipPreflight  EventType = "ship_preflight"
	EventShipChangelog  EventType = "ship_changelog"
	EventShipCommit     EventType = "ship_commit"
	EventDemonstrationReady EventType = "demonstration_ready"

	// Runtime events
	EventRuntimeCheck EventType = "runtime_check"

	// Research events
	EventResearchProgress EventType = "research_progress"
	EventFetchUrl         EventType = "fetch_url"
	EventSearchWeb        EventType = "search_web"

	// Provider events
	EventProviderError  EventType = "provider_error"
	EventProviderSwitch EventType = "provider_switch"
	EventFallback       EventType = "fallback"

	// Context events
	EventCompactionComplete EventType = "compaction_complete"

	// Agent events
	EventAgentStart   EventType = "agent_start"
	EventAgentDone    EventType = "agent_done"
	EventAgentError   EventType = "agent_error"
	EventAgentCancel  EventType = "agent_cancel"
	EventAgentSpawn   EventType = "agent_spawn"

	// Model events
	EventModelSwitch EventType = "model_switch"

	// Config events
	EventConfigReload EventType = "config_reload"

	// Discussion events
	EventDiscussStart     EventType = "discuss_start"
	EventDiscussWaiting   EventType = "discuss_waiting"
	EventDiscussTimeout   EventType = "discuss_timeout"
	EventDiscussComplete  EventType = "discuss_complete"

	// Session events
	EventSessionRestored EventType = "session_restored"

	// Error events
	EventError EventType = "error"
)

// RawEvent is the input to the narrative engine. It carries the event type,
// a timestamp, and a flexible data map for template parameter extraction.
type RawEvent struct {
	Type      EventType
	Timestamp time.Time
	Data      map[string]interface{}
}

// NewRawEvent creates a RawEvent with the current time.
func NewRawEvent(typ EventType, data map[string]interface{}) RawEvent {
	if data == nil {
		data = make(map[string]interface{})
	}
	return RawEvent{
		Type:      typ,
		Timestamp: time.Now(),
		Data:      data,
	}
}

// GetString returns a string value from the data map, or empty string.
func (e RawEvent) GetString(key string) string {
	if v, ok := e.Data[key]; ok {
		if s, ok := v.(string); ok {
			return s
		}
	}
	return ""
}

// GetInt returns an integer value from the data map, or 0.
func (e RawEvent) GetInt(key string) int {
	if v, ok := e.Data[key]; ok {
		switch n := v.(type) {
		case int:
			return n
		case int64:
			return int(n)
		case float64:
			return int(n)
		}
	}
	return 0
}

// GetBool returns a boolean value from the data map, or false.
func (e RawEvent) GetBool(key string) bool {
	if v, ok := e.Data[key]; ok {
		if b, ok := v.(bool); ok {
			return b
		}
	}
	return false
}

// NarrativeType identifies the specific narrative template to use.
type NarrativeType string

const (
	// Orienting
	NarrativeProjectAnalyzed NarrativeType = "project_analyzed"
	NarrativePreflightPassed NarrativeType = "preflight_passed"
	NarrativePreflightFailed NarrativeType = "preflight_failed"
	NarrativeSessionRestored NarrativeType = "session_restored"

	// Understanding
	NarrativeReadingCode      NarrativeType = "reading_code"
	NarrativeReadingProject   NarrativeType = "reading_project"
	NarrativeAnalyzingCode    NarrativeType = "analyzing_code"
	NarrativeMappingCodebase  NarrativeType = "mapping_codebase"

	// Discussing
	NarrativeAskingQuestion    NarrativeType = "asking_question"
	NarrativeWaitingForAnswer  NarrativeType = "waiting_for_answer"
	NarrativeQuestionTimedOut  NarrativeType = "question_timed_out"

	// Planning
	NarrativePlanningWork NarrativeType = "planning_work"
	NarrativePlanWave     NarrativeType = "plan_wave"
	NarrativePlanRefining NarrativeType = "plan_refining"
	NarrativePlanReady    NarrativeType = "plan_ready"

	// Researching
	NarrativeResearchingTopic NarrativeType = "researching_topic"
	NarrativeFetchingUrl      NarrativeType = "fetching_url"
	NarrativeSearchingWeb     NarrativeType = "searching_web"

	// Executing
	NarrativeStartingTask   NarrativeType = "starting_task"
	NarrativeTaskComplete   NarrativeType = "task_complete"
	NarrativeTaskFailed     NarrativeType = "task_failed"
	NarrativeWritingFile    NarrativeType = "writing_file"
	NarrativeEditingFile    NarrativeType = "editing_file"
	NarrativeRunningCommand NarrativeType = "running_command"
	NarrativeReadingFile    NarrativeType = "reading_file"
	NarrativeSearchingCode  NarrativeType = "searching_code"
	NarrativeFindingFiles   NarrativeType = "finding_files"

	// Verifying
	NarrativeRunningTests NarrativeType = "running_tests"
	NarrativeTestsPassed  NarrativeType = "tests_passed"
	NarrativeTestsFailed  NarrativeType = "tests_failed"
	NarrativeBuildFailed  NarrativeType = "build_failed"

	// Recovering
	NarrativeSelfHealing     NarrativeType = "self_healing"
	NarrativeHealingAttempt  NarrativeType = "healing_attempt"
	NarrativeHealingSuccess  NarrativeType = "healing_success"
	NarrativeHealingFailed   NarrativeType = "healing_failed"

	// Shipping
	NarrativePreparingCommit NarrativeType = "preparing_commit"
	NarrativeChangelogReady  NarrativeType = "changelog_ready"
	NarrativeChangesReady    NarrativeType = "changes_ready"
	NarrativeCommitted       NarrativeType = "committed"

	// Learning
	NarrativeContextCompressed NarrativeType = "context_compressed"
	NarrativeMessagesRemoved  NarrativeType = "messages_removed"

	// Alerting
	NarrativeProviderError  NarrativeType = "provider_error"
	NarrativeProviderSwitch NarrativeType = "provider_switch"
	NarrativeConfigReloaded NarrativeType = "config_reloaded"
	NarrativeLoopDetected   NarrativeType = "loop_detected"
	NarrativeModelSwitched  NarrativeType = "model_switched"

	// Agent
	NarrativeAgentStart  NarrativeType = "agent_start"
	NarrativeAgentDone   NarrativeType = "agent_done"
	NarrativeAgentError  NarrativeType = "agent_error"
	NarrativeAgentCancel NarrativeType = "agent_cancel"

	// Generic
	NarrativeError NarrativeType = "error"
)

// AllNarrativeTypes returns all defined NarrativeType constants.
func AllNarrativeTypes() []NarrativeType {
	return []NarrativeType{
		NarrativeProjectAnalyzed, NarrativePreflightPassed, NarrativePreflightFailed,
		NarrativeSessionRestored, NarrativeReadingCode, NarrativeReadingProject,
		NarrativeAnalyzingCode, NarrativeMappingCodebase, NarrativeAskingQuestion,
		NarrativeWaitingForAnswer, NarrativeQuestionTimedOut, NarrativePlanningWork,
		NarrativePlanWave, NarrativePlanRefining, NarrativePlanReady,
		NarrativeResearchingTopic, NarrativeFetchingUrl, NarrativeSearchingWeb,
		NarrativeStartingTask, NarrativeTaskComplete, NarrativeTaskFailed,
		NarrativeWritingFile, NarrativeEditingFile, NarrativeRunningCommand,
		NarrativeReadingFile, NarrativeSearchingCode, NarrativeFindingFiles,
		NarrativeRunningTests, NarrativeTestsPassed, NarrativeTestsFailed,
		NarrativeBuildFailed, NarrativeSelfHealing, NarrativeHealingAttempt,
		NarrativeHealingSuccess, NarrativeHealingFailed, NarrativePreparingCommit,
		NarrativeChangelogReady, NarrativeChangesReady, NarrativeCommitted,
		NarrativeContextCompressed, NarrativeMessagesRemoved, NarrativeProviderError,
		NarrativeProviderSwitch, NarrativeConfigReloaded, NarrativeLoopDetected,
		NarrativeModelSwitched, NarrativeAgentStart, NarrativeAgentDone,
		NarrativeAgentError, NarrativeAgentCancel, NarrativeError,
	}
}

// NarrativeObject is the output of the narrative engine. It carries everything
// needed to render a human-readable progress description.
type NarrativeObject struct {
	Type       NarrativeType
	Category   Category
	Priority   Priority
	Display    Display
	Text       string
	Duration   time.Duration // minimum display duration
	Timestamp  time.Time
	GroupKey   string // non-empty if this was produced by grouping
	GroupCount int    // number of events grouped into this narrative
	Metadata   map[string]interface{}
}

// IsZero returns true if the NarrativeObject is the zero value.
func (n NarrativeObject) IsZero() bool {
	return n.Type == "" && n.Text == ""
}

// DisplayDuration returns the minimum time this narrative should be visible.
func (n NarrativeObject) DisplayDuration() time.Duration {
	if n.Duration > 0 {
		return n.Duration
	}
	return defaultDuration(n.Category)
}

func defaultDuration(cat Category) time.Duration {
	switch cat {
	case CategoryAlerting:
		return 5 * time.Second
	case CategoryRecovering:
		return 2 * time.Second
	case CategoryExecuting:
		return 1 * time.Second
	case CategoryShipping:
		return 2 * time.Second
	case CategoryLearning:
		return 3 * time.Second
	default:
		return 1 * time.Second
	}
}
