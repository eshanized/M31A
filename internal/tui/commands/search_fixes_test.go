package commands

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func TestHandleSearch_MatchesSegmentContent(t *testing.T) {
	// This test verifies that /search finds content inside message Segments,
	// not just the top-level Content field. This is critical for agent-mode
	// conversations where tool calls and results are stored in Segments.

	messages := []types.Message{
		{
			Content: "Hello world",
		},
		{
			Content: "",
			Segments: []types.MessageSegment{
				{Content: `{"name":"Bash","input":{"command":"go build"}}`},
			},
		},
		{
			Content: "Here is a file",
			Segments: []types.MessageSegment{
				{Content: "package main\nfunc main() {}"},
			},
		},
	}

	// Test: search for "go build" — should match the segment content
	query := "go build"
	found := 0
	for _, msg := range messages {
		text := msg.Content
		for _, seg := range msg.Segments {
			if seg.Content != "" {
				text += "\n" + seg.Content
			}
		}
		if strings.Contains(strings.ToLower(text), query) {
			found++
		}
	}
	if found != 1 {
		t.Errorf("expected 1 match for 'go build' in segments, got %d", found)
	}

	// Test: search for "package main" — should match segment in third message
	query = "package main"
	found = 0
	for _, msg := range messages {
		text := msg.Content
		for _, seg := range msg.Segments {
			if seg.Content != "" {
				text += "\n" + seg.Content
			}
		}
		if strings.Contains(strings.ToLower(text), query) {
			found++
		}
	}
	if found != 1 {
		t.Errorf("expected 1 match for 'package main', got %d", found)
	}

	// Test: search for "hello" — should match Content field only
	query = "hello"
	found = 0
	for _, msg := range messages {
		text := msg.Content
		for _, seg := range msg.Segments {
			if seg.Content != "" {
				text += "\n" + seg.Content
			}
		}
		if strings.Contains(strings.ToLower(text), query) {
			found++
		}
	}
	if found != 1 {
		t.Errorf("expected 1 match for 'hello', got %d", found)
	}

	// Test: search for "nonexistent" — should match nothing
	query = "nonexistent"
	found = 0
	for _, msg := range messages {
		text := msg.Content
		for _, seg := range msg.Segments {
			if seg.Content != "" {
				text += "\n" + seg.Content
			}
		}
		if strings.Contains(strings.ToLower(text), query) {
			found++
		}
	}
	if found != 0 {
		t.Errorf("expected 0 matches for 'nonexistent', got %d", found)
	}
}
