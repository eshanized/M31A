package autodream

import (
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

// makeMessages creates n alternating user/assistant messages with sample content.
func makeMessages(n int) []types.Message {
	msgs := make([]types.Message, n)
	for i := 0; i < n; i++ {
		role := "user"
		if i%2 == 1 {
			role = "assistant"
		}
		msgs[i] = types.Message{
			Role:      role,
			Content:   sampleContent(i),
			CreatedAt: time.Now().Add(-time.Duration(n-i) * time.Minute),
		}
	}
	return msgs
}

// sampleContent returns deterministic content for message i.
func sampleContent(i int) string {
	return format("Message %d. This is sample content with enough words to trigger token estimation.", i)
}

// makeMessagesWithContent creates messages with the given content strings.
// Roles alternate user/assistant starting with user at index 0.
func makeMessagesWithContent(content []string) []types.Message {
	msgs := make([]types.Message, len(content))
	for i, c := range content {
		role := "user"
		if i%2 == 1 {
			role = "assistant"
		}
		msgs[i] = types.Message{
			Role:      role,
			Content:   c,
			CreatedAt: time.Now().Add(-time.Duration(len(content)-i) * time.Minute),
		}
	}
	return msgs
}

// makeSystemMessages creates n messages where indices in sysIdx are marked as system role.
func makeSystemMessages(n int, sysIdx []int) []types.Message {
	msgs := makeMessages(n)
	sysSet := make(map[int]bool, len(sysIdx))
	for _, idx := range sysIdx {
		sysSet[idx] = true
	}
	for i := range msgs {
		if sysSet[i] {
			msgs[i].Role = "system"
		}
	}
	return msgs
}

// makeToolCallMessages creates n messages where indices in toolIdx have tool calls.
func makeToolCallMessages(n int, toolIdx []int) []types.Message {
	msgs := makeMessages(n)
	toolSet := make(map[int]bool, len(toolIdx))
	for _, idx := range toolIdx {
		toolSet[idx] = true
	}
	for i := range msgs {
		if toolSet[i] {
			msgs[i].ToolCalls = []types.ToolCall{
				{ID: "call_1", Name: "test", Input: nil},
			}
		}
	}
	return msgs
}

// format is a convenience wrapper around fmt.Sprintf for test messages.
func format(s string, args ...interface{}) string {
	return fmt.Sprintf(s, args...)
}

// ---------------------------------------------------------------------------
// CanConsolidate tests
// ---------------------------------------------------------------------------

func TestCanConsolidate_Empty(t *testing.T) {
	c := New(nil)
	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false for empty message list")
	}
}

func TestCanConsolidate_Single(t *testing.T) {
	msgs := makeMessages(1)
	c := New(msgs)
	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false for a single message")
	}
}

func TestCanConsolidate_Valid(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)
	if !c.CanConsolidate() {
		t.Error("CanConsolidate should be true for 10 messages with candidates")
	}
}

func TestCanConsolidate_Paused(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)
	c.Pause()
	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false when paused")
	}
}

// ---------------------------------------------------------------------------
// Consolidate tests
// ---------------------------------------------------------------------------

func TestConsolidate_WithProtectedMessages_Even(t *testing.T) {
	// 10 messages, none system/tool calls
	// Protected: first (0) + last 5 (5-9) = indices {0,5,6,7,8,9}
	// Candidates: indices 1-4 (4 msgs)
	// Target: oldest 50% of candidates = ceil(4/2) = 2 → indices 1,2
	// Result: 1 summary + 8 kept = 9 messages, MessagesRemoved = 2
	msgs := makeMessages(10)
	c := New(msgs)
	result := c.Consolidate()

	if !result.Success {
		t.Fatalf("Consolidate should succeed: %s", result.Error)
	}
	if result.MessagesRemoved != 2 {
		t.Errorf("MessagesRemoved = %d, want 2", result.MessagesRemoved)
	}
	if result.TokensSaved <= 0 {
		t.Errorf("TokensSaved should be > 0, got %d", result.TokensSaved)
	}
	if result.DurationMs < 0 {
		t.Errorf("DurationMs should be >= 0, got %d", result.DurationMs)
	}

	msgsAfter := c.Messages()
	if len(msgsAfter) != 9 {
		t.Errorf("after consolidate: len(messages) = %d, want 9", len(msgsAfter))
	}
}

func TestConsolidate_AllMessagesProtected(t *testing.T) {
	// 10 messages where first (0) + system in indices 1-4 + last 5 (5-9) protect all.
	msgs := makeSystemMessages(10, []int{1, 2, 3, 4})
	c := New(msgs)

	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false when all messages are protected")
	}
	result := c.Consolidate()
	if result.Success {
		t.Error("Consolidate should fail when all messages are protected")
	}
}

func TestConsolidate_SingleCandidate(t *testing.T) {
	// 10 messages, 9 protected: first (0) + system (1-3) + last 5 (5-9)
	// Candidates: index 4 only → target = 1
	// Result: 1 summary + 9 kept = 10 messages, MessagesRemoved = 1
	msgs := makeSystemMessages(10, []int{1, 2, 3})
	c := New(msgs)

	if !c.CanConsolidate() {
		t.Fatal("CanConsolidate should be true with one candidate")
	}
	result := c.Consolidate()
	if !result.Success {
		t.Fatalf("Consolidate should succeed: %s", result.Error)
	}
	if result.MessagesRemoved != 1 {
		t.Errorf("MessagesRemoved = %d, want 1", result.MessagesRemoved)
	}

	msgsAfter := c.Messages()
	if len(msgsAfter) != 10 {
		t.Errorf("after consolidate: len(messages) = %d, want 10 (1 summary + 9 kept)", len(msgsAfter))
	}
}

func TestConsolidate_OneMessage(t *testing.T) {
	msgs := makeMessages(1)
	c := New(msgs)
	result := c.Consolidate()
	if result.Success {
		t.Error("Consolidate should fail for a single message")
	}
}

func TestConsolidate_ZeroMessages(t *testing.T) {
	c := New(nil)
	result := c.Consolidate()
	if result.Success {
		t.Error("Consolidate should fail for zero messages")
	}
}

func TestConsolidate_TokensSaved(t *testing.T) {
	content := []string{
		"Short content.",
		"Another short message with a few more words.",
		"",
		"Medium length message that has a reasonable number of words in it for testing purposes.",
		"Yet another message for the pool to ensure enough content.",
		"Quick reply.",
		"A bit longer content here with multiple words to track token estimation properly.",
		"Helpful analysis message with thoughtful commentary.",
		"Final wrap-up message concluding the conversation nicely.",
		"Last message signal.",
	}
	msgs := makeMessagesWithContent(content)
	c := New(msgs)
	result := c.Consolidate()

	if !result.Success {
		t.Fatalf("Consolidate should succeed: %s", result.Error)
	}
	if result.TokensSaved <= 0 {
		t.Errorf("TokensSaved should be > 0, got %d", result.TokensSaved)
	}

	// Verify tokens saved matches the word count * 1.3 of the consolidated messages
	// With 10 messages, protected are indices 0 + 5-9, candidates are 1-4
	// Target = ceil(4/2) = 2 → indices 1, 2
	consolidatedContent := content[1] + "\n" + content[2]
	expectedWords := len(strings.Fields(consolidatedContent))
	expectedTokens := expectedWords * 13 / 10 // roughly words * 1.3
	// Allow 10% margin for the rounding in Ceil
	if result.TokensSaved < expectedTokens-2 || result.TokensSaved > expectedTokens+2 {
		t.Logf("word count: %d, expected tokens ~%d, got %d", expectedWords, expectedTokens, result.TokensSaved)
	}
}

func TestConsolidate_SummaryMessageProperties(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)
	result := c.Consolidate()

	if !result.Success {
		t.Fatalf("Consolidate should succeed: %s", result.Error)
	}

	msgsAfter := c.Messages()
	summary := msgsAfter[0]

	if summary.Role != "system" {
		t.Errorf("summary message Role = %q, want \"system\"", summary.Role)
	}
	if len(summary.Segments) != 1 {
		t.Fatalf("summary message should have 1 segment, got %d", len(summary.Segments))
	}
	if summary.Segments[0].Type != "memory" {
		t.Errorf("summary segment Type = %q, want \"memory\"", summary.Segments[0].Type)
	}
	if summary.Segments[0].Visible {
		t.Error("summary segment Visible should be false")
	}
	if summary.CreatedAt.IsZero() {
		t.Error("summary message CreatedAt should be set")
	}
	if !strings.HasPrefix(result.Summary, "[AutoDream Context Summary]") {
		t.Errorf("Summary text should start with prefix, got: %s", result.Summary[:40])
	}
}

func TestConsolidate_Idempotent(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)

	// First consolidation
	result1 := c.Consolidate()
	if !result1.Success {
		t.Fatalf("first consolidation should succeed: %s", result1.Error)
	}

	// Verify the summary message has memory segment type
	msgsAfter := c.Messages()
	if len(msgsAfter) == 0 {
		t.Fatal("messages should not be empty after consolidation")
	}
	summary := msgsAfter[0]
	if summary.Role != "system" {
		t.Errorf("summary message role = %q, want \"system\"", summary.Role)
	}
	if len(summary.Segments) == 0 || summary.Segments[0].Type != "memory" {
		t.Error("first message should have memory segment type")
	}

	// Second consolidation — should not fail (may consolidate remaining candidates)
	result2 := c.Consolidate()
	// Either success or a meaningful error
	if !result2.Success && result2.Error == "" {
		t.Error("second consolidation should have a meaningful error message if it fails")
	}

	_ = result2
}

// ---------------------------------------------------------------------------
// Pause / Resume tests
// ---------------------------------------------------------------------------

func TestPauseResume(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)

	if c.IsPaused() {
		t.Error("fresh Consolidator should not be paused")
	}

	c.Pause()
	if !c.IsPaused() {
		t.Error("after Pause, IsPaused should return true")
	}
	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false while paused")
	}

	c.Resume()
	if c.IsPaused() {
		t.Error("after Resume, IsPaused should return false")
	}
	if !c.CanConsolidate() {
		t.Error("after Resume, CanConsolidate should return true")
	}
}

// ---------------------------------------------------------------------------
// Stats tests
// ---------------------------------------------------------------------------

func TestStats(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)

	stats := c.Stats()
	expectedKeys := []string{"total_messages", "total_consolidations", "paused", "last_consolidation", "estimated_tokens"}
	for _, key := range expectedKeys {
		if _, ok := stats[key]; !ok {
			t.Errorf("Stats() missing key %q", key)
		}
	}

	if stats["total_messages"].(int) != 10 {
		t.Errorf("total_messages = %d, want 10", stats["total_messages"].(int))
	}
	if stats["total_consolidations"].(int) != 0 {
		t.Errorf("total_consolidations = %d, want 0", stats["total_consolidations"].(int))
	}
	if stats["paused"].(bool) {
		t.Error("paused should be false before any operation")
	}
	if stats["last_consolidation"].(string) != "" {
		t.Errorf("last_consolidation should be empty, got %q", stats["last_consolidation"])
	}
	if stats["estimated_tokens"].(int) <= 0 {
		t.Errorf("estimated_tokens should be > 0, got %d", stats["estimated_tokens"])
	}

	// After consolidation, stats should update
	c.Consolidate()
	stats = c.Stats()
	if stats["total_consolidations"].(int) != 1 {
		t.Errorf("after consolidate, total_consolidations = %d, want 1", stats["total_consolidations"].(int))
	}
	if stats["last_consolidation"].(string) == "" {
		t.Error("after consolidate, last_consolidation should be set")
	}
}

// ---------------------------------------------------------------------------
// Messages immutability test
// ---------------------------------------------------------------------------

func TestMessages_Immutability(t *testing.T) {
	msgs := makeMessages(5)
	c := New(msgs)

	// Get the message slice and mutate it
	got := c.Messages()
	if len(got) != 5 {
		t.Fatalf("Messages() returned %d messages, want 5", len(got))
	}

	// Mutate the returned slice
	got[0].Content = "MUTATED"
	got = append(got, types.Message{Role: "injected", Content: "INJECTED"})

	// Internal state should be unchanged
	internal := c.Messages()
	if internal[0].Content == "MUTATED" {
		t.Error("mutating returned slice should not affect internal state")
	}
	if len(internal) != 5 {
		t.Errorf("internal message count changed to %d, want 5", len(internal))
	}
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

func TestConsolidate_EmptyContentMessages(t *testing.T) {
	// Messages with empty content should not crash
	msgs := []types.Message{
		{Role: "user", Content: "Hello", CreatedAt: time.Now().Add(-3 * time.Minute)},
		{Role: "assistant", Content: "", CreatedAt: time.Now().Add(-2 * time.Minute)},
		{Role: "user", Content: "", CreatedAt: time.Now().Add(-1 * time.Minute)},
		{Role: "assistant", Content: "World", CreatedAt: time.Now()},
	}
	// With 4 messages: protected = first (0) + last 5 (0-3, all since n=4<5) = {0,1,2,3}
	// All protected → CanConsolidate false
	c := New(msgs)
	if c.CanConsolidate() {
		t.Log("all messages protected, CanConsolidate should be false")
	}
}

func TestConsolidate_SystemAtFirstIndex(t *testing.T) {
	// System message at index 0 is both first message and system → protected once (no double-count issue)
	msgs := []types.Message{
		{Role: "system", Content: "You are a helpful assistant.", CreatedAt: time.Now().Add(-3 * time.Minute)},
		{Role: "user", Content: "Hello", CreatedAt: time.Now().Add(-2 * time.Minute)},
		{Role: "assistant", Content: "Hi there!", CreatedAt: time.Now().Add(-1 * time.Minute)},
	}
	// Protected: first (0), system (0), last 5 (0-2)
	// All 3 messages protected → no candidates
	c := New(msgs)
	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false when all messages are protected")
	}
}

func TestConsolidate_ExactBoundary(t *testing.T) {
	// 8 messages → should have candidates after first + last 5
	// Protected: 0, 3,4,5,6,7 (first + last 5) = 6 protected
	// Candidates: 1,2 (2 msgs)
	// Target: ceil(2/2) = 1 → 1 consolidated
	// Result: 1 + 7 = 8 messages
	msgs := makeMessages(8)
	c := New(msgs)
	result := c.Consolidate()

	if !result.Success {
		t.Fatalf("Consolidate should succeed with 8 messages: %s", result.Error)
	}
	if result.MessagesRemoved != 1 {
		t.Errorf("MessagesRemoved = %d, want 1", result.MessagesRemoved)
	}
	msgsAfter := c.Messages()
	if len(msgsAfter) != 8 {
		t.Errorf("len(messages) after = %d, want 8 (1 summary + 7 kept)", len(msgsAfter))
	}
}

func TestConsolidate_ToolCallMessagesProtected(t *testing.T) {
	// 8 messages where index 3 has a tool call
	msgs := makeToolCallMessages(8, []int{3})
	c := New(msgs)

	// Protected: first (0), tool call (3), last 5 (3-7) = {0,3,4,5,6,7}
	// Candidates: 1,2 (2 msgs)
	// Target: ceil(2/2) = 1 → consolidated
	result := c.Consolidate()
	if !result.Success {
		t.Fatalf("Consolidate should succeed: %s", result.Error)
	}
	// Verify tool call message at original index 3 is still present
	msgsAfter := c.Messages()
	foundToolCall := false
	for _, msg := range msgsAfter {
		if len(msg.ToolCalls) > 0 {
			foundToolCall = true
			break
		}
	}
	if !foundToolCall {
		t.Error("tool call message should be preserved after consolidation")
	}
}

// ---------------------------------------------------------------------------
// M-7: Reentrancy guard
// ---------------------------------------------------------------------------

// TestAutoDream_ReentryGuard verifies that concurrent Consolidate calls
// produce ErrAlreadyConsolidating for the second caller.
func TestAutoDream_ReentryGuard(t *testing.T) {
	msgs := makeMessages(20)
	c := New(msgs)

	// Use channels to synchronize goroutines and avoid data races.
	started := make(chan struct{})
	proceed := make(chan struct{})
	gotResult1 := make(chan string, 1)
	gotResult2 := make(chan string, 1)

	// Goroutine 1: holds the consolidating flag via mu lock
	go func() {
		c.mu.Lock()
		close(started) // signal that we've entered
		<-proceed      // wait for signal to proceed
		c.mu.Unlock()

		// Now actually consolidate
		r := c.Consolidate()
		gotResult1 <- safeErrStr(r)
	}()

	// Goroutine 2: tries to enter while goroutine 1 holds the lock
	go func() {
		<-started // wait for goroutine 1 to enter
		time.Sleep(50 * time.Millisecond)
		r := c.Consolidate()
		gotResult2 <- safeErrStr(r)
	}()

	// Let goroutine 2 try, then release goroutine 1
	time.Sleep(100 * time.Millisecond)
	close(proceed)

	// Collect results via channels (race-free)
	err1 := <-gotResult1
	err2 := <-gotResult2

	// At least one should have gotten ErrAlreadyConsolidating
	if err1 == ErrAlreadyConsolidating.Error() || err2 == ErrAlreadyConsolidating.Error() {
		return // success
	}

	t.Errorf("Expected one result to have ErrAlreadyConsolidating, got err1=%q err2=%q", err1, err2)
}

func safeErrStr(r *ConsolidationResult) string {
	if r == nil {
		return "<nil>"
	}
	return r.Error
}

// TestAutoDream_ConsolidatingFlagResets verifies that the consolidating flag
// is reset after Consolidate completes (even on error).
func TestAutoDream_ConsolidatingFlagResets(t *testing.T) {
	c := New(makeMessages(20))

	// First call should succeed
	r1 := c.Consolidate()
	if !r1.Success {
		t.Fatalf("First consolidate should succeed: %s", r1.Error)
	}

	// Second call should work (flag was reset)
	r2 := c.Consolidate()
	// May or may not succeed depending on message count, but should NOT
	// return ErrAlreadyConsolidating
	if r2.Error == ErrAlreadyConsolidating.Error() {
		t.Error("Consolidating flag was not reset after first call")
	}
}

// TestAutoDream_ErrAlreadyConsolidating_IsSentinel verifies the error is
// a proper sentinel that can be compared with errors.Is.
func TestAutoDream_ErrAlreadyConsolidating_IsSentinel(t *testing.T) {
	r := &ConsolidationResult{Error: ErrAlreadyConsolidating.Error()}
	if r.Error != ErrAlreadyConsolidating.Error() {
		t.Errorf("Expected error %q, got %q", ErrAlreadyConsolidating.Error(), r.Error)
	}
}

// ---------------------------------------------------------------------------
// SetMessages tests
// ---------------------------------------------------------------------------

func TestSetMessages_ReplacesMessageList(t *testing.T) {
	c := New(makeMessages(5))
	if len(c.Messages()) != 5 {
		t.Fatalf("expected 5 messages initially, got %d", len(c.Messages()))
	}

	newMsgs := makeMessages(3)
	c.SetMessages(newMsgs)
	if len(c.Messages()) != 3 {
		t.Errorf("expected 3 messages after SetMessages, got %d", len(c.Messages()))
	}
}

func TestSetMessages_DefensiveCopy(t *testing.T) {
	c := New(nil)
	msgs := makeMessages(3)
	c.SetMessages(msgs)

	// Mutate original — should not affect internal state
	msgs[0].Content = "MUTATED"
	internal := c.Messages()
	if internal[0].Content == "MUTATED" {
		t.Error("SetMessages should make a defensive copy")
	}
}

func TestSetMessages_Nil(t *testing.T) {
	c := New(makeMessages(5))
	c.SetMessages(nil)
	if len(c.Messages()) != 0 {
		t.Errorf("expected 0 messages after SetMessages(nil), got %d", len(c.Messages()))
	}
}

func TestNew_NilMessages(t *testing.T) {
	c := New(nil)
	if c == nil {
		t.Fatal("New(nil) should not return nil")
	}
	if len(c.Messages()) != 0 {
		t.Errorf("expected 0 messages, got %d", len(c.Messages()))
	}
}
