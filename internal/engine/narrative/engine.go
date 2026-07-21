package narrative

import (
	"sync"
	"time"
)

// Engine is the top-level orchestrator that transforms raw events into
// narrative objects. It is safe for concurrent use.
type Engine struct {
	mu         sync.Mutex
	classifier *Classifier
	resolver   *TemplateResolver
	grouper    *Grouper
	timing     *TimingGuard
	now        func() time.Time
}

// EngineConfig configures the narrative engine.
type EngineConfig struct {
	Classifier *Classifier
	Resolver   *TemplateResolver
	Grouper    *Grouper
	Timing     *TimingGuard
}

// DefaultEngineConfig returns a fully configured engine with sensible defaults.
func DefaultEngineConfig() EngineConfig {
	return EngineConfig{
		Classifier: NewClassifier(),
		Resolver:   NewTemplateResolver(),
		Grouper:    NewGrouper(DefaultGroupConfig()),
		Timing:     NewTimingGuard(DefaultTimingConfig()),
	}
}

// EngineConfigWithOverrides returns a fully configured engine with user-defined
// template and classification overrides from config.
func EngineConfigWithOverrides(templateOverrides map[string]string, classificationOverrides map[string]string) EngineConfig {
	return EngineConfig{
		Classifier: NewClassifierWithOverrides(classificationOverrides),
		Resolver:   NewTemplateResolverWithOverrides(templateOverrides),
		Grouper:    NewGrouper(DefaultGroupConfig()),
		Timing:     NewTimingGuard(DefaultTimingConfig()),
	}
}

// NewEngine creates an Engine with the given configuration.
func NewEngine(config EngineConfig) *Engine {
	if config.Classifier == nil {
		config.Classifier = NewClassifier()
	}
	if config.Resolver == nil {
		config.Resolver = NewTemplateResolver()
	}
	if config.Grouper == nil {
		config.Grouper = NewGrouper(DefaultGroupConfig())
	}
	if config.Timing == nil {
		config.Timing = NewTimingGuard(DefaultTimingConfig())
	}
	return &Engine{
		classifier: config.Classifier,
		resolver:   config.Resolver,
		grouper:    config.Grouper,
		timing:     config.Timing,
		now:        time.Now,
	}
}

// ProcessEvent processes a single raw event and returns zero or more
// narrative objects. Grouped events are buffered and returned when the
// group is ready to flush.
func (e *Engine) ProcessEvent(event RawEvent) []NarrativeObject {
	e.mu.Lock()
	defer e.mu.Unlock()

	result := e.classifier.Classify(event)

	switch result.Classification {
	case ClassifyHidden:
		return nil

	case ClassifyNarrative:
		narrative := e.resolver.Resolve(result.NarrativeType, event)
		if !e.timing.ShouldEmit(narrative) {
			return nil
		}
		e.timing.Activate(narrative)
		return []NarrativeObject{narrative}

	case ClassifyGrouped:
		e.grouper.Add(event, result.GroupKey)
		if e.grouper.ShouldFlush(result.GroupKey) {
			return e.flushGroup(result.GroupKey)
		}
		return nil

	case ClassifyExpanded:
		narrative := e.resolver.Resolve(result.NarrativeType, event)
		e.timing.Activate(narrative)
		return []NarrativeObject{narrative}

	default:
		return nil
	}
}

// ProcessEvents processes multiple raw events in order.
func (e *Engine) ProcessEvents(events []RawEvent) []NarrativeObject {
	var result []NarrativeObject
	for _, event := range events {
		result = append(result, e.ProcessEvent(event)...)
	}
	return result
}

// FlushGroups forces all pending grouped events to be emitted.
func (e *Engine) FlushGroups() []NarrativeObject {
	e.mu.Lock()
	defer e.mu.Unlock()

	var result []NarrativeObject
	for key := range e.grouper.pending {
		result = append(result, e.flushGroup(key)...)
	}
	return result
}

// flushGroup flushes a single group and returns the narrative.
// Must be called with e.mu held.
func (e *Engine) flushGroup(groupKey string) []NarrativeObject {
	events := e.grouper.Flush(groupKey)
	if len(events) == 0 {
		return nil
	}

	// Build a synthetic event from the group.
	synthetic := RawEvent{
		Type:      EventToolStart, // default type for grouped
		Timestamp: events[0].Timestamp,
		Data: map[string]interface{}{
			"count":     len(events),
			"group_key": groupKey,
			"files":     CollectFiles(events),
		},
	}

	// Determine the narrative type based on the group key.
	var narrativeType NarrativeType
	switch groupKey {
	case "tools":
		narrativeType = e.resolveToolGroupType(events)
	case "task_diff":
		narrativeType = NarrativeTaskComplete
		synthetic.Data["description"] = GroupSummary(events, groupKey)
	case "ship":
		narrativeType = NarrativeChangelogReady
		synthetic.Data["entry_count"] = e.countEntries(events)
	default:
		narrativeType = NarrativeReadingCode
		synthetic.Data["scope"] = GroupSummary(events, groupKey)
	}

	narrative := e.resolver.Resolve(narrativeType, synthetic)
	narrative.GroupKey = groupKey
	narrative.GroupCount = len(events)

	if !e.timing.ShouldEmit(narrative) {
		return nil
	}
	e.timing.Activate(narrative)
	return []NarrativeObject{narrative}
}

// resolveToolGroupType determines the narrative type for a group of tool events.
func (e *Engine) resolveToolGroupType(events []RawEvent) NarrativeType {
	reads := 0
	writes := 0
	searches := 0
	commands := 0

	for _, ev := range events {
		toolName := ev.GetString("tool_name")
		switch toolName {
		case "FileRead":
			reads++
		case "FileWrite", "Edit":
			writes++
		case "Grep", "Glob":
			searches++
		case "Bash":
			commands++
		}
	}

	// Return the dominant type.
	if reads > writes && reads > searches && reads > commands {
		return NarrativeReadingCode
	}
	if writes > reads && writes > searches && writes > commands {
		return NarrativeWritingFile
	}
	if searches > 0 {
		return NarrativeSearchingCode
	}
	if commands > 0 {
		return NarrativeRunningCommand
	}
	return NarrativeReadingCode
}

func (e *Engine) countEntries(events []RawEvent) int {
	count := 0
	for _, ev := range events {
		count += ev.GetInt("entry_count")
	}
	if count == 0 {
		return len(events)
	}
	return count
}

// Reset clears all internal state.
func (e *Engine) Reset() {
	e.mu.Lock()
	defer e.mu.Unlock()

	e.grouper = NewGrouper(DefaultGroupConfig())
	e.timing = NewTimingGuard(DefaultTimingConfig())
}

// PendingNarratives returns any narratives that should be emitted due to
// expired flush windows. This should be called periodically (e.g., on tick).
func (e *Engine) PendingNarratives() []NarrativeObject {
	e.mu.Lock()
	defer e.mu.Unlock()

	var result []NarrativeObject
	for key := range e.grouper.pending {
		if e.grouper.ShouldFlush(key) {
			result = append(result, e.flushGroup(key)...)
		}
	}

	// Check if active narrative is stale.
	if e.timing.IsStale() {
		e.timing.Deactivate()
	}

	return result
}

// ActiveNarrative returns the currently active narrative type.
func (e *Engine) ActiveNarrative() NarrativeType {
	return e.timing.ActiveNarrativeType()
}

// NarrativeForPhase maps a workflow phase to its primary narrative type.
func NarrativeForPhase(phase string) NarrativeType {
	switch phase {
	case "initialize":
		return NarrativeReadingProject
	case "discuss":
		return NarrativeAskingQuestion
	case "plan":
		return NarrativePlanningWork
	case "execute":
		return NarrativeStartingTask
	case "verify":
		return NarrativeRunningTests
	case "runtime":
		return NarrativeRunningTests
	case "ship":
		return NarrativePreparingCommit
	default:
		return NarrativeReadingProject
	}
}

// NarrativeForTool maps a tool name to its narrative type.
func NarrativeForTool(toolName string) NarrativeType {
	switch toolName {
	case "FileRead":
		return NarrativeReadingFile
	case "FileWrite":
		return NarrativeWritingFile
	case "Edit":
		return NarrativeEditingFile
	case "Grep":
		return NarrativeSearchingCode
	case "Glob":
		return NarrativeFindingFiles
	case "Bash":
		return NarrativeRunningCommand
	case "WebFetch":
		return NarrativeFetchingUrl
	case "WebSearch":
		return NarrativeSearchingWeb
	case "CodeMap":
		return NarrativeMappingCodebase
	case "Agent":
		return NarrativeAgentStart
	default:
		return NarrativeRunningCommand
	}
}
