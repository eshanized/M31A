package narrative

import (
	"fmt"
	"strings"
	"time"
)

// TemplateResolver maps NarrativeType to text templates and extracts
// parameters from RawEvent data. It is stateless and safe for concurrent use.
type TemplateResolver struct {
	templates map[NarrativeType]templateEntry
}

type templateEntry struct {
	template string
	category Category
	priority Priority
	display  Display
	duration int64 // milliseconds, 0 = use default
}

// NewTemplateResolver creates a TemplateResolver with all default templates.
func NewTemplateResolver() *TemplateResolver {
	tr := &TemplateResolver{
		templates: make(map[NarrativeType]templateEntry, 64),
	}
	tr.registerDefaults()
	return tr
}

// NewTemplateResolverWithOverrides creates a TemplateResolver with default
// templates and user-defined template overrides. Overrides replace the template
// text for matching NarrativeType keys. All other fields (category, priority,
// display, duration) from the default entry are preserved.
func NewTemplateResolverWithOverrides(overrides map[string]string) *TemplateResolver {
	tr := NewTemplateResolver()
	if len(overrides) == 0 {
		return tr
	}
	for typeStr, overrideText := range overrides {
		if overrideText == "" {
			continue
		}
		typ := NarrativeType(typeStr)
		if entry, ok := tr.templates[typ]; ok {
			entry.template = overrideText
			tr.templates[typ] = entry
		} else {
			// Allow defining templates for new custom types
			tr.templates[typ] = templateEntry{
				template: overrideText,
				category: CategoryAlerting,
				priority: PriorityPhase,
				display:  DisplayBoth,
			}
		}
	}
	return tr
}

// Resolve renders the narrative text for a given type and event.
func (tr *TemplateResolver) Resolve(typ NarrativeType, event RawEvent) NarrativeObject {
	entry, ok := tr.templates[typ]
	if !ok {
		return NarrativeObject{
			Type:      typ,
			Category:  CategoryAlerting,
			Priority:  PriorityPhase,
			Display:   DisplaySidebar,
			Text:      string(typ),
			Timestamp: event.Timestamp,
		}
	}

	text := tr.render(entry.template, event)
	obj := NarrativeObject{
		Type:      typ,
		Category:  entry.category,
		Priority:  entry.priority,
		Display:   entry.display,
		Text:      text,
		Timestamp: event.Timestamp,
		Metadata:  event.Data,
	}
	if entry.duration > 0 {
		obj.Duration = time.Duration(entry.duration) * time.Millisecond
	}
	return obj
}

// render performs simple {placeholder} substitution from event data.
func (tr *TemplateResolver) render(template string, event RawEvent) string {
	result := template
	for {
		start := strings.Index(result, "{")
		if start == -1 {
			break
		}
		end := strings.Index(result[start:], "}")
		if end == -1 {
			break
		}
		end += start
		key := result[start+1 : end]
		value := event.GetString(key)
		if value == "" {
			// Try int values
			if iv := event.GetInt(key); iv != 0 {
				value = fmt.Sprintf("%d", iv)
			} else {
				value = key // keep placeholder if no data
			}
		}
		result = result[:start] + value + result[end+1:]
	}
	return result
}

func (tr *TemplateResolver) registerDefaults() {
	// Orienting
	tr.templates[NarrativeProjectAnalyzed] = templateEntry{
		template: "Analyzed {lang} project ({file_count} files)",
		category: CategoryOrienting,
		priority: PriorityOrientation,
		display:  DisplayBoth,
	}
	tr.templates[NarrativePreflightPassed] = templateEntry{
		template: "Preflight checks passed",
		category: CategoryOrienting,
		priority: PriorityOrientation,
		display:  DisplayBoth,
	}
	tr.templates[NarrativePreflightFailed] = templateEntry{
		template: "Preflight: {issue_count} issues found",
		category: CategoryOrienting,
		priority: PriorityOrientation,
		display:  DisplayBoth,
	}
	tr.templates[NarrativeSessionRestored] = templateEntry{
		template: "Session restored ({message_count} messages)",
		category: CategoryOrienting,
		priority: PriorityOrientation,
		display:  DisplayToast,
	}

	// Understanding
	tr.templates[NarrativeReadingCode] = templateEntry{
		template: "Reading {scope}",
		category: CategoryUnderstanding,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeReadingProject] = templateEntry{
		template: "Reading project structure",
		category: CategoryUnderstanding,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeAnalyzingCode] = templateEntry{
		template: "Analyzing {scope}",
		category: CategoryUnderstanding,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeMappingCodebase] = templateEntry{
		template: "Mapping codebase dependencies",
		category: CategoryUnderstanding,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}

	// Discussing
	tr.templates[NarrativeAskingQuestion] = templateEntry{
		template: "Preparing questions",
		category: CategoryDiscussing,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeWaitingForAnswer] = templateEntry{
		template: "Waiting for your input",
		category: CategoryDiscussing,
		priority: PriorityActionRequired,
		display:  DisplayBoth,
	}
	tr.templates[NarrativeQuestionTimedOut] = templateEntry{
		template: "Question timed out, proceeding",
		category: CategoryDiscussing,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 3000,
	}

	// Planning
	tr.templates[NarrativePlanningWork] = templateEntry{
		template: "Planning implementation",
		category: CategoryPlanning,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativePlanWave] = templateEntry{
		template: "Planning wave {current}/{total}",
		category: CategoryPlanning,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativePlanRefining] = templateEntry{
		template: "Refining plan (iteration {iteration}/{max_iterations})",
		category: CategoryPlanning,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativePlanReady] = templateEntry{
		template: "Plan ready: {task_count} tasks",
		category: CategoryPlanning,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 3000,
	}

	// Researching
	tr.templates[NarrativeResearchingTopic] = templateEntry{
		template: "Researching {topic}",
		category: CategoryResearching,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeFetchingUrl] = templateEntry{
		template: "Reading {url}",
		category: CategoryResearching,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeSearchingWeb] = templateEntry{
		template: "Searching for {query}",
		category: CategoryResearching,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}

	// Executing
	tr.templates[NarrativeStartingTask] = templateEntry{
		template: "Implementing {description}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeTaskComplete] = templateEntry{
		template: "{description} complete",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
		duration: 2000,
	}
	tr.templates[NarrativeTaskFailed] = templateEntry{
		template: "{description} failed",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplayBoth,
		duration: 5000,
	}
	tr.templates[NarrativeWritingFile] = templateEntry{
		template: "Writing {file}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeEditingFile] = templateEntry{
		template: "Editing {file}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeRunningCommand] = templateEntry{
		template: "Running {command}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeReadingFile] = templateEntry{
		template: "Reading {file}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeSearchingCode] = templateEntry{
		template: "Searching for {pattern}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeFindingFiles] = templateEntry{
		template: "Finding {pattern}",
		category: CategoryExecuting,
		priority: PriorityTask,
		display:  DisplaySidebar,
	}

	// Verifying
	tr.templates[NarrativeRunningTests] = templateEntry{
		template: "Running verification",
		category: CategoryVerifying,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeTestsPassed] = templateEntry{
		template: "Verification passed ({test_count} tests)",
		category: CategoryVerifying,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 3000,
	}
	tr.templates[NarrativeTestsFailed] = templateEntry{
		template: "Verification: {passed}/{total} tests passed",
		category: CategoryVerifying,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 5000,
	}
	tr.templates[NarrativeBuildFailed] = templateEntry{
		template: "Build failed",
		category: CategoryVerifying,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 5000,
	}

	// Recovering
	tr.templates[NarrativeSelfHealing] = templateEntry{
		template: "Recovering from {error}",
		category: CategoryRecovering,
		priority: PriorityPhase,
		display:  DisplayBoth,
	}
	tr.templates[NarrativeHealingAttempt] = templateEntry{
		template: "Healing attempt {attempt}/{max_attempts}",
		category: CategoryRecovering,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeHealingSuccess] = templateEntry{
		template: "Recovery successful",
		category: CategoryRecovering,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 2000,
	}
	tr.templates[NarrativeHealingFailed] = templateEntry{
		template: "Recovery failed",
		category: CategoryRecovering,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 5000,
	}

	// Shipping
	tr.templates[NarrativePreparingCommit] = templateEntry{
		template: "Preparing final commit",
		category: CategoryShipping,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeChangelogReady] = templateEntry{
		template: "Changelog: {entry_count} entries",
		category: CategoryShipping,
		priority: PriorityPhase,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeChangesReady] = templateEntry{
		template: "Changes ready for review",
		category: CategoryShipping,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 3000,
	}
	tr.templates[NarrativeCommitted] = templateEntry{
		template: "Committed: {message}",
		category: CategoryShipping,
		priority: PriorityPhase,
		display:  DisplayBoth,
		duration: 3000,
	}

	// Learning
	tr.templates[NarrativeContextCompressed] = templateEntry{
		template: "Context compressed (saved {tokens_saved} tokens)",
		category: CategoryLearning,
		priority: PriorityPhase,
		display:  DisplayToast,
		duration: 4000,
	}
	tr.templates[NarrativeMessagesRemoved] = templateEntry{
		template: "Auto-compressed: {count} messages removed",
		category: CategoryLearning,
		priority: PriorityPhase,
		display:  DisplayToast,
		duration: 4000,
	}

	// Alerting
	tr.templates[NarrativeProviderError] = templateEntry{
		template: "Provider error: {error}",
		category: CategoryAlerting,
		priority: PriorityActionRequired,
		display:  DisplayBoth,
		duration: 5000,
	}
	tr.templates[NarrativeProviderSwitch] = templateEntry{
		template: "Switched from {from} to {to}",
		category: CategoryAlerting,
		priority: PriorityPhase,
		display:  DisplayToast,
		duration: 4000,
	}
	tr.templates[NarrativeConfigReloaded] = templateEntry{
		template: "Configuration reloaded",
		category: CategoryAlerting,
		priority: PriorityPhase,
		display:  DisplayToast,
		duration: 2000,
	}
	tr.templates[NarrativeLoopDetected] = templateEntry{
		template: "Detected repeated operation, adjusting",
		category: CategoryAlerting,
		priority: PriorityActionRequired,
		display:  DisplayBoth,
		duration: 5000,
	}
	tr.templates[NarrativeModelSwitched] = templateEntry{
		template: "Switched to {model}",
		category: CategoryAlerting,
		priority: PriorityPhase,
		display:  DisplayToast,
		duration: 3000,
	}

	// Agent
	tr.templates[NarrativeAgentStart] = templateEntry{
		template: "Spawning {agent_type} agent",
		category: CategoryExecuting,
		priority: PriorityAgent,
		display:  DisplaySidebar,
	}
	tr.templates[NarrativeAgentDone] = templateEntry{
		template: "{agent_type} agent completed",
		category: CategoryExecuting,
		priority: PriorityAgent,
		display:  DisplaySidebar,
		duration: 2000,
	}
	tr.templates[NarrativeAgentError] = templateEntry{
		template: "{agent_type} agent error",
		category: CategoryAlerting,
		priority: PriorityAgent,
		display:  DisplayBoth,
		duration: 5000,
	}
	tr.templates[NarrativeAgentCancel] = templateEntry{
		template: "Agent cancelled",
		category: CategoryAlerting,
		priority: PriorityAgent,
		display:  DisplaySidebar,
		duration: 2000,
	}

	// Error (generic)
	tr.templates[NarrativeError] = templateEntry{
		template: "{error}",
		category: CategoryAlerting,
		priority: PriorityActionRequired,
		display:  DisplayBoth,
		duration: 5000,
	}
}
