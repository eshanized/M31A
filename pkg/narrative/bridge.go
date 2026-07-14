package narrative

import (
	"time"
)

// Bridge converts workflow engine messages into narrative RawEvents.
// It is a pure adapter with no state of its own.
type Bridge struct{}

// NewBridge creates a new narrative bridge.
func NewBridge() *Bridge {
	return &Bridge{}
}

// MsgToRawEvent converts a workflow engine message into a RawEvent.
// The message must implement WorkflowEvent (EventType + EventData).
// Returns ok=true if the message was converted, ok=false if it should be ignored.
func (b *Bridge) MsgToRawEvent(msg interface{}) (RawEvent, bool) {
	we, ok := msg.(WorkflowEvent)
	if !ok {
		return RawEvent{}, false
	}

	return RawEvent{
		Type:      EventType(we.EventType()),
		Timestamp: time.Now(),
		Data:      we.EventData(),
	}, true
}
