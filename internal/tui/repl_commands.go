package tui

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// executeShellCommand runs a shell command via the dispatcher's Bash tool,
// bypassing the LLM entirely. The command output is displayed in chat as
// an assistant message but is marked SkipForLLM so it doesn't appear in
// subsequent LLM requests. No permission modal (user explicitly requested
// execution via ! prefix), though PermissionRule deny rules still apply.
func (m *ReplModel) executeShellCommand(command string) ([]tea.Cmd, bool) {
	// Show a "Running..." placeholder immediately
	displayMsg := types.Message{
		Role:    "assistant",
		Content: fmt.Sprintf("$ %s\nRunning...\n", command),
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: fmt.Sprintf("$ %s\nRunning...\n", command),
			Visible: true,
		}},
		CreatedAt:  time.Now(),
		SkipForLLM: true,
	}
	m.messages = append(m.messages, displayMsg)
	m.renderMessages()
	m.viewport.GotoBottom()

	// Execute via goroutine that calls dispatcher.Execute with a Bash ToolCall
	return []tea.Cmd{func() tea.Msg {
		if m.dispatcher == nil {
			return ShellResultMsg{
				Command: command,
				Output:  "",
				Err:     "dispatcher not available",
			}
		}

		// Construct ToolCall for Bash with interactive=false to skip permission modal
		params := map[string]any{
			"command":     command,
			"description": "shell mode command",
			"timeout":     300,
			"interactive": false,
		}
		paramsJSON, _ := json.Marshal(params)
		toolCall := types.ToolCall{
			ID:    fmt.Sprintf("shell_%d", time.Now().UnixNano()),
			Name:  "Bash",
			Input: paramsJSON,
		}

		ctx, cancel := context.WithTimeout(context.Background(), 300*time.Second)
		defer cancel()

		result, err := m.dispatcher.Execute(ctx, toolCall)
		if err != nil {
			return ShellResultMsg{
				Command: command,
				Output:  result.Output,
				Err:     err.Error(),
			}
		}
		if result.Error != "" {
			return ShellResultMsg{
				Command: command,
				Output:  result.Output,
				Err:     result.Error,
			}
		}
		return ShellResultMsg{
			Command: command,
			Output:  result.Output,
			Err:     "",
		}
	}}, false
}

// messagesForLLM returns only messages that should be included in LLM context,
// filtering out shell mode messages and any other messages marked SkipForLLM.
func (m *ReplModel) messagesForLLM() []types.Message {
	var filtered []types.Message
	for _, msg := range m.messages {
		if !msg.SkipForLLM {
			filtered = append(filtered, msg)
		}
	}
	return filtered
}

// expandFileRefs scans input for @filepath references, resolves file contents,
// and replaces them with inline content blocks. Unresolvable references remain as-is.
func (m *ReplModel) expandFileRefs(input string) string {
	if m.cwd == "" {
		return input
	}

	return fileRefPattern.ReplaceAllStringFunc(input, func(match string) string {
		path := match[1:] // strip the @ prefix

		// Resolve relative to cwd
		resolvedPath := path
		if !filepath.IsAbs(path) {
			resolvedPath = filepath.Join(m.cwd, path)
		}
		resolvedPath = filepath.Clean(resolvedPath)

		// Stat the file
		fi, err := os.Stat(resolvedPath)
		if err != nil {
			// File not found or inaccessible — leave reference as-is
			return match
		}

		if fi.IsDir() {
			return match // directories not supported, leave as-is
		}

		// Size check: 100KB limit
		const maxFileSize = 100 * 1024
		if fi.Size() > maxFileSize {
			return fmt.Sprintf("%s [file too large: %d bytes, max 100KB]", match, fi.Size())
		}

		// Read first 512 bytes for binary detection
		f, err := os.Open(resolvedPath)
		if err != nil {
			return match
		}
		defer f.Close()

		header := make([]byte, 512)
		n, _ := f.Read(header)

		// Detect content type via mime sniff
		contentType := http.DetectContentType(header[:n])
		if !strings.HasPrefix(contentType, "text/") &&
			contentType != "application/json" &&
			contentType != "application/xml" &&
			contentType != "application/javascript" &&
			contentType != "application/x-sh" &&
			contentType != "application/yaml" {
			// Binary file — show placeholder
			return fmt.Sprintf("%s [binary: %s, %d bytes]", match, contentType, fi.Size())
		}

		// Read full content
		f.Seek(0, 0)
		content, err := io.ReadAll(f)
		if err != nil {
			return match
		}

		// Format: --- path/to/file ---\n{content}\n---
		return fmt.Sprintf("--- %s ---\n%s\n---", path, string(content))
	})
}
