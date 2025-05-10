// Package autodream provides context consolidation for M31A sessions.
// When conversation context grows large, AutoDream summarizes older
// messages into a single memory segment to stay within the model's
// context window.
package autodream

import (
	"fmt"
	"math"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// ConsolidationResult reports the outcome of a Consolidate() call.
type ConsolidationResult struct {
	Success         bool   `json:"success"`
	MessagesRemoved int    `json:"messages_removed"`
	TokensSaved     int    `json:"tokens_saved"`
	DurationMs      int64  `json:"duration_ms"`
	Summary         string `json:"summary"`
	Error           string `json:"error,omitempty"`
}

// Consolidator manages context consolidation for a session. It holds a
// snapshot of the message history and provides thread-safe methods to
// consolidate (summarize the oldest 50% of non-protected messages), pause,
// resume, and report statistics.
type Consolidator struct {
	mu                  sync.RWMutex
	messages            []types.Message
	paused              bool
	lastConsolidation   time.Time
	totalConsolidations int
}

// New creates a Consolidator that owns a defensive copy of the given messages.
func New(messages []types.Message) *Consolidator {
	var cp []types.Message
	if messages != nil {
		cp = make([]types.Message, len(messages))
		copy(cp, messages)
	}
	return &Consolidator{
		messages: cp,
	}
}

// SetMessages replaces the Consolidator's internal message list with a
// defensive copy of the provided slice. This is the integration point for
// callers (e.g. ReplModel) that own the authoritative message history.
func (c *Consolidator) SetMessages(messages []types.Message) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if messages == nil {
		c.messages = nil
		return
	}
	cp := make([]types.Message, len(messages))
	copy(cp, messages)
	c.messages = cp
}

// canConsolidateLocked checks whether consolidation is possible.
// Caller must hold at least a read lock.
func (c *Consolidator) canConsolidateLocked() bool {
	if c.paused {
		return false
	}
	if len(c.messages) <= 1 {
		return false
	}
	protected := c.protectedIndices()
	candidates := c.candidateIndices(protected)
	if len(candidates) == 0 {
		return false
	}
	// If all candidates are already memory segments, there is nothing to consolidate.
	for _, idx := range candidates {
		if !isMemoryMessage(c.messages[idx]) {
			return true
		}
	}
	return false
}

// CanConsolidate returns true if a consolidation can be performed.
func (c *Consolidator) CanConsolidate() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.canConsolidateLocked()
}

// Consolidate compacts the oldest 50% of non-protected messages into a single
// memory segment. Protected messages (first message, system messages, tool call
// messages, and the last 5 messages) are never consolidated.
func (c *Consolidator) Consolidate() *ConsolidationResult {
	start := time.Now()

	c.mu.Lock()
	defer c.mu.Unlock()

	if !c.canConsolidateLocked() {
		return &ConsolidationResult{
			Success:    false,
			DurationMs: time.Since(start).Milliseconds(),
			Error:      "cannot consolidate: paused, too few messages, or nothing to consolidate",
		}
	}

	protected := c.protectedIndices()
	candidates := c.candidateIndices(protected)
	if len(candidates) == 0 {
		return &ConsolidationResult{
			Success:    false,
			DurationMs: time.Since(start).Milliseconds(),
			Error:      "no messages to consolidate",
		}
	}

	// Oldest 50% of candidates (ceiling so at least one is consolidated)
	targetCount := (len(candidates) + 1) / 2
	target := candidates[:targetCount]

	// Build quick-lookup set of target indices
	targetSet := make(map[int]struct{}, targetCount)
	for _, idx := range target {
		targetSet[idx] = struct{}{}
	}

	// Gather target messages
	targetMsgs := make([]types.Message, targetCount)
	for i, idx := range target {
		targetMsgs[i] = c.messages[idx]
	}

	// Estimate tokens saved
	rawContent := c.rawText(targetMsgs)
	wordCount := len(strings.Fields(rawContent))
	tokensSaved := int(math.Ceil(float64(wordCount) * 1.3))

	// Build summary text: prefix + timeframe + truncated content (~500 tokens)
	timeFrame := c.timeframeDescription(targetMsgs)
	maxWords := int(math.Floor(500.0 / 1.3)) // ~384 words ≈ 500 tokens
	allWords := strings.Fields(rawContent)
	truncatedContent := rawContent
	if len(allWords) > maxWords {
		truncatedContent = strings.Join(allWords[:maxWords], " ")
	}
	summaryText := fmt.Sprintf("[AutoDream Context Summary] %s — %s", timeFrame, truncatedContent)

	summarySegment := types.MessageSegment{
		Type:    "memory",
		Content: summaryText,
		Visible: false,
	}
	summaryMsg := types.Message{
		Role:      "system",
		Content:   summaryText,
		Segments:  []types.MessageSegment{summarySegment},
		CreatedAt: time.Now(),
	}

	// Build the new message list: summary + all non-target messages in original order
	kept := make([]types.Message, 0, len(c.messages)-targetCount)
	for i, msg := range c.messages {
		if _, inTarget := targetSet[i]; !inTarget {
			kept = append(kept, msg)
		}
	}

	c.messages = append(kept, summaryMsg)
	c.totalConsolidations++
	c.lastConsolidation = time.Now()

	return &ConsolidationResult{
		Success:         true,
		MessagesRemoved: targetCount,
		TokensSaved:     tokensSaved,
		DurationMs:      time.Since(start).Milliseconds(),
		Summary:         summaryText,
	}
}

// Pause prevents consolidation from happening. CanConsolidate will return false.
func (c *Consolidator) Pause() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.paused = true
}

// Resume re-enables consolidation.
func (c *Consolidator) Resume() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.paused = false
}

// IsPaused returns true if consolidation is paused.
func (c *Consolidator) IsPaused() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.paused
}

// Messages returns a defensive copy of the managed message slice.
// Mutating the returned slice does not affect the Consolidator's internal state.
func (c *Consolidator) Messages() []types.Message {
	c.mu.RLock()
	defer c.mu.RUnlock()
	result := make([]types.Message, len(c.messages))
	copy(result, c.messages)
	return result
}

// Stats returns a map of consolidation statistics:
//   - total_messages (int)
//   - total_consolidations (int)
//   - paused (bool)
//   - last_consolidation (string, ISO8601, empty if never)
//   - estimated_tokens (int)
func (c *Consolidator) Stats() map[string]interface{} {
	c.mu.RLock()
	defer c.mu.RUnlock()

	lastConsolidation := ""
	if !c.lastConsolidation.IsZero() {
		lastConsolidation = c.lastConsolidation.Format(time.RFC3339)
	}

	estTokens := 0
	for _, msg := range c.messages {
		estTokens += int(math.Ceil(float64(len(strings.Fields(msg.Content))) * 1.3))
	}

	return map[string]interface{}{
		"total_messages":       len(c.messages),
		"total_consolidations": c.totalConsolidations,
		"paused":               c.paused,
		"last_consolidation":   lastConsolidation,
		"estimated_tokens":     estTokens,
	}
}

// protectedIndices computes the set of message indices that must never be
// consolidated. These are: the first message (index 0), all system messages,
// the last 5 messages, and all messages with tool calls.
// Caller must hold at least a read lock.
func (c *Consolidator) protectedIndices() map[int]struct{} {
	protected := make(map[int]struct{})

	n := len(c.messages)
	if n == 0 {
		return protected
	}

	// First message (initial goal / context) is always protected
	protected[0] = struct{}{}

	for i := 0; i < n; i++ {
		msg := c.messages[i]
		if msg.Role == "system" {
			protected[i] = struct{}{}
		}
		if len(msg.ToolCalls) > 0 {
			protected[i] = struct{}{}
		}
	}

	// Last 5 messages are protected (unless they overlap with the above)
	start := n - 5
	if start < 0 {
		start = 0
	}
	for i := start; i < n; i++ {
		protected[i] = struct{}{}
	}

	return protected
}

// candidateIndices returns all indices not in the protected set, sorted ascending.
// Caller must hold at least a read lock.
func (c *Consolidator) candidateIndices(protected map[int]struct{}) []int {
	var candidates []int
	for i := range c.messages {
		if _, ok := protected[i]; !ok {
			candidates = append(candidates, i)
		}
	}
	return candidates
}

// rawText concatenates the Content fields of the given messages with newlines.
func (c *Consolidator) rawText(msgs []types.Message) string {
	var parts []string
	for _, msg := range msgs {
		if msg.Content != "" {
			parts = append(parts, msg.Content)
		}
	}
	return strings.Join(parts, "\n")
}

// timeframeDescription returns a human-friendly description of when the
// messages were created (e.g. "messages from 9 minutes ago"). Falls back to
// a role-based count when timestamps are missing.
func (c *Consolidator) timeframeDescription(msgs []types.Message) string {
	if len(msgs) == 0 {
		return "0 messages"
	}

	first := msgs[0]
	last := msgs[len(msgs)-1]
	if !first.CreatedAt.IsZero() && !last.CreatedAt.IsZero() {
		duration := last.CreatedAt.Sub(first.CreatedAt)
		if duration < time.Minute {
			secs := int(duration.Seconds())
			if secs <= 0 {
				secs = 1
			}
			return fmt.Sprintf("messages from %d seconds ago", secs)
		}
		if duration < time.Hour {
			return fmt.Sprintf("messages from %d minutes ago", int(duration.Minutes()))
		}
		return fmt.Sprintf("messages from %d hours ago", int(duration.Hours()))
	}

	// No usable timestamps → describe by role distribution
	roleCount := make(map[string]int)
	for _, msg := range msgs {
		roleCount[msg.Role]++
	}
	roles := make([]string, 0, len(roleCount))
	for role := range roleCount {
		roles = append(roles, role)
	}
	sort.Strings(roles)
	parts := make([]string, 0, len(roles))
	for _, role := range roles {
		parts = append(parts, fmt.Sprintf("%d %s", roleCount[role], role))
	}
	return fmt.Sprintf("%d messages (%s)", len(msgs), strings.Join(parts, ", "))
}

// isMemoryMessage returns true when every segment in the message has type "memory".
func isMemoryMessage(msg types.Message) bool {
	if len(msg.Segments) == 0 {
		return false
	}
	for _, seg := range msg.Segments {
		if seg.Type != "memory" {
			return false
		}
	}
	return true
}
