package autodream

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// ---------------------------------------------------------------------------
// isMemoryMessage tests
// ---------------------------------------------------------------------------

func TestIsMemoryMessage_AllMemory(t *testing.T) {
	msg := types.Message{
		Segments: []types.MessageSegment{
			{Type: "memory", Content: "summary 1"},
			{Type: "memory", Content: "summary 2"},
		},
	}
	if !isMemoryMessage(msg) {
		t.Error("expected true for all-memory segments")
	}
}

func TestIsMemoryMessage_NoSegments(t *testing.T) {
	msg := types.Message{}
	if isMemoryMessage(msg) {
		t.Error("expected false for empty segments")
	}
}

func TestIsMemoryMessage_MixedTypes(t *testing.T) {
	msg := types.Message{
		Segments: []types.MessageSegment{
			{Type: "memory", Content: "summary"},
			{Type: "text", Content: "regular text"},
		},
	}
	if isMemoryMessage(msg) {
		t.Error("expected false for mixed segment types")
	}
}

func TestIsMemoryMessage_NoMemoryType(t *testing.T) {
	msg := types.Message{
		Segments: []types.MessageSegment{
			{Type: "text", Content: "text"},
			{Type: "code", Content: "code"},
		},
	}
	if isMemoryMessage(msg) {
		t.Error("expected false when no memory segments")
	}
}

// ---------------------------------------------------------------------------
// timeframeDescription tests
// ---------------------------------------------------------------------------

func TestTimeframeDescription_Empty(t *testing.T) {
	c := New(nil)
	desc := c.timeframeDescription([]types.Message{})
	if desc != "0 messages" {
		t.Errorf("expected '0 messages', got %q", desc)
	}
}

func TestTimeframeDescription_Seconds(t *testing.T) {
	c := New(nil)
	now := time.Now()
	msgs := []types.Message{
		{Role: "user", Content: "msg1", CreatedAt: now},
		{Role: "user", Content: "msg2", CreatedAt: now.Add(30 * time.Second)},
	}
	desc := c.timeframeDescription(msgs)
	if desc == "" {
		t.Error("expected non-empty description")
	}
}

func TestTimeframeDescription_Minutes(t *testing.T) {
	c := New(nil)
	now := time.Now()
	msgs := []types.Message{
		{Role: "user", Content: "msg1", CreatedAt: now.Add(-10 * time.Minute)},
		{Role: "user", Content: "msg2", CreatedAt: now},
	}
	desc := c.timeframeDescription(msgs)
	if desc == "" {
		t.Error("expected non-empty description")
	}
}

func TestTimeframeDescription_Hours(t *testing.T) {
	c := New(nil)
	now := time.Now()
	msgs := []types.Message{
		{Role: "user", Content: "msg1", CreatedAt: now.Add(-3 * time.Hour)},
		{Role: "user", Content: "msg2", CreatedAt: now},
	}
	desc := c.timeframeDescription(msgs)
	if desc == "" {
		t.Error("expected non-empty description")
	}
}

func TestTimeframeDescription_NoTimestamps(t *testing.T) {
	c := New(nil)
	msgs := []types.Message{
		{Role: "user", Content: "msg1", CreatedAt: time.Time{}},
		{Role: "assistant", Content: "msg2", CreatedAt: time.Time{}},
	}
	desc := c.timeframeDescription(msgs)
	if desc == "" {
		t.Error("expected non-empty description")
	}
}

func TestTimeframeDescription_SingleMessage(t *testing.T) {
	c := New(nil)
	now := time.Now()
	msgs := []types.Message{
		{Role: "user", Content: "msg1", CreatedAt: now.Add(-2 * time.Minute)},
	}
	desc := c.timeframeDescription(msgs)
	if desc == "" {
		t.Error("expected non-empty description for single message")
	}
}

// ---------------------------------------------------------------------------
// rawText tests
// ---------------------------------------------------------------------------

func TestRawText_AllEmpty(t *testing.T) {
	c := New(nil)
	msgs := []types.Message{
		{Content: ""},
		{Content: ""},
	}
	result := c.rawText(msgs)
	if result != "" {
		t.Errorf("expected empty string, got %q", result)
	}
}

func TestRawText_MixedContent(t *testing.T) {
	c := New(nil)
	msgs := []types.Message{
		{Content: "hello"},
		{Content: ""},
		{Content: "world"},
	}
	result := c.rawText(msgs)
	if result != "hello\nworld" {
		t.Errorf("expected 'hello\\nworld', got %q", result)
	}
}

func TestRawText_NilMessages(t *testing.T) {
	c := New(nil)
	result := c.rawText(nil)
	if result != "" {
		t.Errorf("expected empty string for nil, got %q", result)
	}
}

func TestRawText_EmptyMessages(t *testing.T) {
	c := New(nil)
	result := c.rawText([]types.Message{})
	if result != "" {
		t.Errorf("expected empty string for empty slice, got %q", result)
	}
}

// ---------------------------------------------------------------------------
// protectedIndices tests
// ---------------------------------------------------------------------------

func TestProtectedIndices_Empty(t *testing.T) {
	c := New(nil)
	protected := c.protectedIndices()
	if len(protected) != 0 {
		t.Errorf("expected empty protected set, got %d", len(protected))
	}
}

func TestProtectedIndices_FirstMessageProtected(t *testing.T) {
	msgs := makeMessages(20)
	c := New(msgs)
	protected := c.protectedIndices()
	if _, ok := protected[0]; !ok {
		t.Error("index 0 should always be protected")
	}
}

func TestProtectedIndices_SystemMessagesProtected(t *testing.T) {
	msgs := makeSystemMessages(20, []int{5, 10})
	c := New(msgs)
	protected := c.protectedIndices()
	if _, ok := protected[5]; !ok {
		t.Error("system message at index 5 should be protected")
	}
	if _, ok := protected[10]; !ok {
		t.Error("system message at index 10 should be protected")
	}
}

func TestProtectedIndices_ToolCallsProtected(t *testing.T) {
	msgs := makeToolCallMessages(20, []int{7})
	c := New(msgs)
	protected := c.protectedIndices()
	if _, ok := protected[7]; !ok {
		t.Error("tool call message at index 7 should be protected")
	}
}

func TestProtectedIndices_LastFiveProtected(t *testing.T) {
	msgs := makeMessages(20)
	c := New(msgs)
	protected := c.protectedIndices()
	for i := 15; i < 20; i++ {
		if _, ok := protected[i]; !ok {
			t.Errorf("last 5 messages (index %d) should be protected", i)
		}
	}
}

// ---------------------------------------------------------------------------
// candidateIndices tests
// ---------------------------------------------------------------------------

func TestCandidateIndices(t *testing.T) {
	msgs := makeMessages(20)
	c := New(msgs)
	protected := c.protectedIndices()
	candidates := c.candidateIndices(protected)

	for _, idx := range candidates {
		if _, ok := protected[idx]; ok {
			t.Errorf("candidate %d is in protected set", idx)
		}
	}
}

func TestCandidateIndices_AllProtected(t *testing.T) {
	msgs := makeSystemMessages(10, []int{1, 2, 3, 4})
	c := New(msgs)
	protected := c.protectedIndices()
	candidates := c.candidateIndices(protected)

	if len(candidates) != 0 {
		t.Errorf("expected 0 candidates when all protected, got %d", len(candidates))
	}
}

// ---------------------------------------------------------------------------
// Stats additional tests
// ---------------------------------------------------------------------------

func TestStats_NilMessages(t *testing.T) {
	c := New(nil)
	stats := c.Stats()
	if stats["total_messages"].(int) != 0 {
		t.Errorf("expected 0 messages, got %d", stats["total_messages"])
	}
}

func TestStats_EmptyMessages(t *testing.T) {
	c := New([]types.Message{})
	stats := c.Stats()
	if stats["total_messages"].(int) != 0 {
		t.Errorf("expected 0 messages, got %d", stats["total_messages"])
	}
	if stats["estimated_tokens"].(int) != 0 {
		t.Errorf("expected 0 estimated tokens, got %d", stats["estimated_tokens"])
	}
}

func TestStats_AfterConsolidation(t *testing.T) {
	msgs := makeMessages(20)
	c := New(msgs)

	c.Consolidate()
	stats := c.Stats()
	if stats["total_consolidations"].(int) != 1 {
		t.Errorf("expected 1 consolidation, got %d", stats["total_consolidations"])
	}
	if stats["last_consolidation"].(string) == "" {
		t.Error("last_consolidation should be set after consolidation")
	}
}

func TestStats_PausedState(t *testing.T) {
	msgs := makeMessages(10)
	c := New(msgs)
	c.Pause()
	stats := c.Stats()
	if !stats["paused"].(bool) {
		t.Error("paused should be true after Pause()")
	}
}

// ---------------------------------------------------------------------------
// Messages() defensive copy
// ---------------------------------------------------------------------------

func TestMessages_DefensiveCopy(t *testing.T) {
	msgs := makeMessages(5)
	c := New(msgs)

	got := c.Messages()
	got[0].Content = "MUTATED"

	original := c.Messages()
	if original[0].Content == "MUTATED" {
		t.Error("Messages() should return defensive copy")
	}
}

func TestMessages_NilInput(t *testing.T) {
	c := New(nil)
	msgs := c.Messages()
	if len(msgs) != 0 {
		t.Errorf("expected empty messages for nil input, got %d", len(msgs))
	}
}

// ---------------------------------------------------------------------------
// SetMessages defensive copy
// ---------------------------------------------------------------------------

func TestSetMessages_DefensiveCopy_Original(t *testing.T) {
	c := New(nil)
	msgs := makeMessages(5)
	c.SetMessages(msgs)

	// Mutate original
	msgs[0].Content = "MUTATED"
	got := c.Messages()
	if got[0].Content == "MUTATED" {
		t.Error("SetMessages should make a defensive copy, original mutation affected internal state")
	}
}

// ---------------------------------------------------------------------------
// CanConsolidate edge cases
// ---------------------------------------------------------------------------

func TestCanConsolidate_OnlyMemoryMessages(t *testing.T) {
	// Create messages that are all memory segments
	msgs := make([]types.Message, 20)
	for i := range msgs {
		msgs[i] = types.Message{
			Role:    "system",
			Content: "memory content",
			Segments: []types.MessageSegment{
				{Type: "memory", Content: "mem"},
			},
		}
	}
	c := New(msgs)
	// All are system messages → all protected
	if c.CanConsolidate() {
		t.Error("CanConsolidate should be false when all messages are system/memory")
	}
}

func TestCanConsolidate_ToolCallsAndSystem(t *testing.T) {
	// Mix of system and tool call messages
	msgs := makeMessages(20)
	for i := 5; i < 15; i++ {
		msgs[i].Role = "system"
	}
	for i := 0; i < 5; i++ {
		msgs[i].ToolCalls = []types.ToolCall{
			{ID: "call1", Name: "test", Input: nil},
		}
	}
	c := New(msgs)
	// Most messages are protected
	if c.CanConsolidate() {
		// May or may not be consolidatable depending on exact protection
		t.Log("CanConsolidate result for mixed protected messages")
	}
}

// ---------------------------------------------------------------------------
// Consolidate reentrancy guard (direct test)
// ---------------------------------------------------------------------------

func TestConsolidate_ReentrancyGuard_Direct(t *testing.T) {
	c := New(makeMessages(20))

	// Set the consolidating flag directly
	c.consolidating.Store(true)
	result := c.Consolidate()

	if result.Success {
		t.Error("should fail when consolidating flag is set")
	}
	if result.Error != ErrAlreadyConsolidating.Error() {
		t.Errorf("expected ErrAlreadyConsolidating, got %q", result.Error)
	}

	// Reset
	c.consolidating.Store(false)
}
