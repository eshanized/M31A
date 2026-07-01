package narrative

import (
	"testing"
)

func TestClassifierHiddenEvents(t *testing.T) {
	c := NewClassifier()
	hidden := []EventType{
		EventPlanRefine,
		EventDiscussComplete,
	}

	for _, et := range hidden {
		event := RawEvent{Type: et}
		result := c.Classify(event)
		if result.Classification != ClassifyHidden {
			t.Errorf("Event %q should be Hidden, got %q", et, result.Classification)
		}
	}
}

func TestClassifierNarrativeEvents(t *testing.T) {
	c := NewClassifier()
	narrativeCases := []struct {
		eventType    EventType
		expectedType NarrativeType
	}{
		{EventTaskStart, NarrativeStartingTask},
		{EventTaskComplete, NarrativeTaskComplete},
		{EventTaskFailed, NarrativeTaskFailed},
		{EventSelfHealStart, NarrativeSelfHealing},
		{EventSelfHealComplete, NarrativeHealingSuccess},
		{EventInitAnalysis, NarrativeProjectAnalyzed},
		{EventInitPreflight, NarrativePreflightPassed},
		{EventPlanRevision, NarrativePlanRefining},
		{EventPlanChunk, NarrativePlanWave},
		{EventPlanReady, NarrativePlanReady},
		{EventExecutePreflight, NarrativeRunningTests},
		{EventExecuteLoopDetect, NarrativeLoopDetected},
		{EventVerifyReport, NarrativeTestsPassed},
		{EventVerifyTests, NarrativeRunningTests},
		{EventShipPreflight, NarrativePreparingCommit},
		{EventShipCommit, NarrativeCommitted},
		{EventDemonstrationReady, NarrativeChangesReady},
		{EventRuntimeCheck, NarrativeTestsPassed},
		{EventResearchProgress, NarrativeResearchingTopic},
		{EventFetchUrl, NarrativeFetchingUrl},
		{EventSearchWeb, NarrativeSearchingWeb},
		{EventProviderError, NarrativeProviderError},
		{EventProviderSwitch, NarrativeProviderSwitch},
		{EventFallback, NarrativeProviderSwitch},
		{EventCompactionComplete, NarrativeContextCompressed},
		{EventAgentStart, NarrativeAgentStart},
		{EventAgentDone, NarrativeAgentDone},
		{EventAgentError, NarrativeAgentError},
		{EventAgentCancel, NarrativeAgentCancel},
		{EventAgentSpawn, NarrativeAgentStart},
		{EventModelSwitch, NarrativeModelSwitched},
		{EventConfigReload, NarrativeConfigReloaded},
		{EventDiscussStart, NarrativeAskingQuestion},
		{EventDiscussWaiting, NarrativeWaitingForAnswer},
		{EventDiscussTimeout, NarrativeQuestionTimedOut},
		{EventSessionRestored, NarrativeSessionRestored},
		{EventError, NarrativeError},
	}

	for _, tc := range narrativeCases {
		event := RawEvent{
			Type: tc.eventType,
			Data: map[string]interface{}{"description": "test task"},
		}
		result := c.Classify(event)
		if result.Classification != ClassifyNarrative {
			t.Errorf("Event %q should be Narrative, got %q", tc.eventType, result.Classification)
		}
		if result.NarrativeType != tc.expectedType {
			t.Errorf("Event %q should produce type %q, got %q", tc.eventType, tc.expectedType, result.NarrativeType)
		}
	}
}

func TestClassifierToolReadFile(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventToolStart,
		Data: map[string]interface{}{"tool_name": "FileRead"},
	}
	result := c.Classify(event)
	if result.Classification != ClassifyGrouped {
		t.Errorf("FileRead should be Grouped, got %q", result.Classification)
	}
	if result.GroupKey != "tools" {
		t.Errorf("FileRead group key = %q, want %q", result.GroupKey, "tools")
	}
}

func TestClassifierToolWriteFile(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventToolStart,
		Data: map[string]interface{}{"tool_name": "FileWrite"},
	}
	result := c.Classify(event)
	if result.Classification != ClassifyGrouped {
		t.Errorf("FileWrite should be Grouped, got %q", result.Classification)
	}
}

func TestClassifierToolComplete(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventToolComplete,
		Data: map[string]interface{}{"tool_name": "Edit"},
	}
	result := c.Classify(event)
	if result.Classification != ClassifyGrouped {
		t.Errorf("ToolComplete should be Grouped, got %q", result.Classification)
	}
}

func TestClassifierTaskDiff(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventTaskDiff,
		Data: map[string]interface{}{
			"files_changed": 3,
			"additions":     50,
			"deletions":     10,
		},
	}
	result := c.Classify(event)
	if result.Classification != ClassifyGrouped {
		t.Errorf("TaskDiff should be Grouped, got %q", result.Classification)
	}
	if result.GroupKey != "task_diff" {
		t.Errorf("TaskDiff group key = %q, want %q", result.GroupKey, "task_diff")
	}
}

func TestClassifierShipChangelog(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventShipChangelog,
		Data: map[string]interface{}{
			"entry_count": 5,
		},
	}
	result := c.Classify(event)
	if result.Classification != ClassifyGrouped {
		t.Errorf("ShipChangelog should be Grouped, got %q", result.Classification)
	}
	if result.GroupKey != "ship" {
		t.Errorf("ShipChangelog group key = %q, want %q", result.GroupKey, "ship")
	}
}

func TestClassifierUnknownEvent(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventType("unknown_event_type"),
		Data: map[string]interface{}{},
	}
	result := c.Classify(event)
	if result.Classification != ClassifyHidden {
		t.Errorf("Unknown event should be Hidden, got %q", result.Classification)
	}
}

func TestClassifierNilData(t *testing.T) {
	c := NewClassifier()
	event := RawEvent{
		Type: EventToolStart,
		Data: nil,
	}
	result := c.Classify(event)
	if result.Classification != ClassifyGrouped {
		t.Errorf("ToolStart with nil data should be Grouped, got %q", result.Classification)
	}
}

func TestClassifierCustomRules(t *testing.T) {
	c := NewClassifier()
	c.Register(EventType("custom_event"), ClassifyResult{
		Classification: ClassifyNarrative,
		NarrativeType:  NarrativeReadingProject,
	})

	event := RawEvent{Type: "custom_event"}
	result := c.Classify(event)
	if result.Classification != ClassifyNarrative {
		t.Errorf("Custom rule should produce Narrative, got %q", result.Classification)
	}
	if result.NarrativeType != NarrativeReadingProject {
		t.Errorf("Custom rule type = %q, want %q", result.NarrativeType, NarrativeReadingProject)
	}
}
