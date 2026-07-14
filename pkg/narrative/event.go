package narrative

// WorkflowEvent is the interface that workflow engine messages must implement
// to be convertible to narrative RawEvents. This breaks the import cycle
// between pkg/narrative and internal/workflow.
type WorkflowEvent interface {
	// EventType returns the narrative event type string (e.g. "task_start").
	EventType() string
	// EventData returns the event payload as a map.
	EventData() map[string]interface{}
}
