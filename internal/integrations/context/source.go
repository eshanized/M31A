package context

import "context"

// ContextSource provides a named piece of context that can change over time.
// Sources are evaluated before each LLM call and changes are emitted as
// mid-conversation system messages.
type ContextSource interface {
	// Key returns a stable identifier for this source (e.g., "core/instructions").
	Key() string

	// Load fetches the current value of this source.
	Load(ctx context.Context) (string, error)

	// Render formats the value for inclusion in the system prompt.
	Render(value string) string

	// RenderUpdate formats a change notification when the value changes.
	RenderUpdate(oldValue, newValue string) string

	// RenderRemoval formats a notification when the source becomes unavailable.
	RenderRemoval(value string) string
}
