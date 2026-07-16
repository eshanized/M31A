package components

import (
	"fmt"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// BenchmarkGlamourRender measures the raw Glamour render cost for various content sizes.
func BenchmarkGlamourRender(b *testing.B) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		b.Fatalf("failed to create renderer: %v", err)
	}

	benchmarks := []struct {
		name    string
		content string
	}{
		{"short_plain", "Hello, this is a short message."},
		{"short_markdown", "**bold** and *italic* and `code`"},
		{"paragraph", "This is a paragraph with some **bold text** and *italic text* and `inline code`. It has enough words to trigger word wrapping at 80 columns, making it a realistic content segment that would be rendered during normal conversation."},
		{"code_block", "# Go Example\n```go\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n```"},
		{"list", "- Item one with some text\n- Item two with more text\n- Item three\n- Item four\n- Item five"},
		{"mixed", "# Heading\n\nSome **bold** text.\n\n```go\nfmt.Println(\"hi\")\n```\n\n- list item\n- another item"},
	}

	for _, bm := range benchmarks {
		b.Run(bm.name, func(b *testing.B) {
			b.ReportAllocs()
			for i := 0; i < b.N; i++ {
				_, _ = r.renderer.Render(bm.content)
			}
		})
	}
}

// BenchmarkGlamourRenderWithPadding measures the full renderContentSegment path.
func BenchmarkGlamourRenderWithPadding(b *testing.B) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		b.Fatalf("failed to create renderer: %v", err)
	}

	content := "This is a paragraph with some **bold text** and *italic text* and `inline code`. It has enough words to trigger word wrapping at 80 columns."
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_ = r.renderContentSegment(content, 80)
	}
}

// BenchmarkRenderMessage_SimulatedConversation measures the full message render path
// for a conversation with N messages, simulating what happens when scrolling or streaming.
func BenchmarkRenderMessage_SimulatedConversation(b *testing.B) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		b.Fatalf("failed to create renderer: %v", err)
	}

	for _, n := range []int{5, 10, 20} {
		b.Run(fmt.Sprintf("messages_%d", n), func(b *testing.B) {
			messages := make([]types.Message, n)
			for i := range messages {
				messages[i] = types.Message{
					Role:    "assistant",
					Content: fmt.Sprintf("Message %d with **markdown** and `code`.", i),
					Segments: []types.MessageSegment{
						{Type: "content", Content: fmt.Sprintf("Message %d with **markdown** and `code`.", i), Visible: true},
					},
				}
			}
			b.ReportAllocs()
			for i := 0; i < b.N; i++ {
				for _, msg := range messages {
					_ = r.RenderMessage(msg, 80)
				}
			}
		})
	}
}

// BenchmarkRenderMessage_IdleView measures what happens when View() is called repeatedly
// with the same messages (idle state — no streaming).
func BenchmarkRenderMessage_IdleView(b *testing.B) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		b.Fatalf("failed to create renderer: %v", err)
	}

	messages := make([]types.Message, 10)
	for i := range messages {
		messages[i] = types.Message{
			Role:    "assistant",
			Content: "This is a realistic assistant message with **bold**, *italic*, and `code` blocks.\n\n```go\nfunc hello() {\n\tfmt.Println(\"world\")\n}\n```",
			Segments: []types.MessageSegment{
				{Type: "content", Content: "This is a realistic assistant message with **bold**, *italic*, and `code` blocks.\n\n```go\nfunc hello() {\n\tfmt.Println(\"world\")\n}\n```", Visible: true},
			},
		}
	}

	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		for _, msg := range messages {
			_ = r.RenderMessage(msg, 80)
		}
	}
}
