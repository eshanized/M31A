package narrative

// ClassifyResult is the output of classifying a single event.
type ClassifyResult struct {
	Classification Classification
	NarrativeType  NarrativeType // only set when Classification == ClassifyNarrative or ClassifyExpanded
	GroupKey       string        // only set when Classification == ClassifyGrouped
}

// Classifier determines how each raw event should be processed by the
// narrative engine. It is stateless and safe for concurrent use.
type Classifier struct {
	rules map[EventType]ClassifyResult
}

// NewClassifier creates a Classifier with the default rule set.
func NewClassifier() *Classifier {
	c := &Classifier{
		rules: make(map[EventType]ClassifyResult, 64),
	}
	c.registerDefaults()
	return c
}

// Classify returns the classification for an event type.
func (c *Classifier) Classify(event RawEvent) ClassifyResult {
	if r, ok := c.rules[event.Type]; ok {
		return r
	}
	// Unknown events are hidden by default.
	return ClassifyResult{Classification: ClassifyHidden}
}

// Register overrides the classification for an event type.
func (c *Classifier) Register(typ EventType, result ClassifyResult) {
	c.rules[typ] = result
}

func (c *Classifier) registerDefaults() {
	// Phase transitions — narrative
	c.rules[EventPhaseTransitionStart] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativePlanningWork, // resolved dynamically by engine
	}
	c.rules[EventPhaseTransitionComplete] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeTaskComplete, // resolved dynamically by engine
	}

	// Task events — narrative
	c.rules[EventTaskStart] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeStartingTask,
	}
	c.rules[EventTaskComplete] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeTaskComplete,
	}
	c.rules[EventTaskFailed] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeTaskFailed,
	}

	// Tool events — grouped
	c.rules[EventToolStart] = ClassifyResult{
		Classification: ClassifyGrouped,
		GroupKey:       "tools",
	}
	c.rules[EventToolComplete] = ClassifyResult{
		Classification: ClassifyGrouped,
		GroupKey:       "tools",
	}

	// Task diff — grouped
	c.rules[EventTaskDiff] = ClassifyResult{
		Classification: ClassifyGrouped,
		GroupKey:       "task_diff",
	}

	// Self-heal — narrative
	c.rules[EventSelfHealStart] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeSelfHealing,
	}
	c.rules[EventSelfHealComplete] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeHealingSuccess, // resolved dynamically
	}

	// Initialize — narrative
	c.rules[EventInitAnalysis] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeProjectAnalyzed,
	}
	c.rules[EventInitPreflight] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativePreflightPassed, // resolved dynamically
	}

	// Plan — narrative
	c.rules[EventPlanRevision] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativePlanRefining,
	}
	c.rules[EventPlanChunk] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativePlanWave,
	}
	c.rules[EventPlanReady] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativePlanReady,
	}
	c.rules[EventPlanRefine] = ClassifyResult{
		Classification: ClassifyHidden, // user-initiated
	}

	// Execute — narrative
	c.rules[EventExecutePreflight] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeRunningTests, // "Validating dependencies"
	}
	c.rules[EventExecuteLoopDetect] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeLoopDetected,
	}

	// Verify — narrative
	c.rules[EventVerifyReport] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeTestsPassed, // resolved dynamically
	}
	c.rules[EventVerifyTests] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeRunningTests,
	}

	// Ship — narrative
	c.rules[EventShipPreflight] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativePreparingCommit,
	}
	c.rules[EventShipChangelog] = ClassifyResult{
		Classification: ClassifyGrouped,
		GroupKey:       "ship",
	}
	c.rules[EventShipCommit] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeCommitted,
	}
	c.rules[EventDemonstrationReady] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeChangesReady,
	}

	// Runtime — narrative
	c.rules[EventRuntimeCheck] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeTestsPassed, // "Runtime check complete"
	}

	// Research — narrative
	c.rules[EventResearchProgress] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeResearchingTopic,
	}
	c.rules[EventFetchUrl] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeFetchingUrl,
	}
	c.rules[EventSearchWeb] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeSearchingWeb,
	}

	// Provider — narrative
	c.rules[EventProviderError] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeProviderError,
	}
	c.rules[EventProviderSwitch] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeProviderSwitch,
	}
	c.rules[EventFallback] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeProviderSwitch,
	}

	// Context — narrative
	c.rules[EventCompactionComplete] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeContextCompressed,
	}

	// Agent — narrative
	c.rules[EventAgentStart] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeAgentStart,
	}
	c.rules[EventAgentDone] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeAgentDone,
	}
	c.rules[EventAgentError] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeAgentError,
	}
	c.rules[EventAgentCancel] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeAgentCancel,
	}
	c.rules[EventAgentSpawn] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeAgentStart,
	}

	// Model — narrative
	c.rules[EventModelSwitch] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeModelSwitched,
	}

	// Config — narrative
	c.rules[EventConfigReload] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeConfigReloaded,
	}

	// Discussion — narrative
	c.rules[EventDiscussStart] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeAskingQuestion,
	}
	c.rules[EventDiscussWaiting] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeWaitingForAnswer,
	}
	c.rules[EventDiscussTimeout] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeQuestionTimedOut,
	}
	c.rules[EventDiscussComplete] = ClassifyResult{
		Classification: ClassifyHidden, // user-initiated
	}

	// Session — narrative
	c.rules[EventSessionRestored] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeSessionRestored,
	}

	// Error — narrative
	c.rules[EventError] = ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeError,
	}
}
