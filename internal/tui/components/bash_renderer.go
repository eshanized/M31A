package components

import (
	"encoding/json"
	"runtime"
	"strings"

	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// bashPrefix returns the platform-appropriate bash prompt prefix.
func bashPrefix() string {
	if runtime.GOOS == "windows" {
		return "> "
	}
	return "$ "
}

type BashRenderer struct {
	BaseRenderer
}

func NewBashRenderer(t theme.Theme) *BashRenderer {
	return &BashRenderer{BaseRenderer: BaseRenderer{toolName: "Bash", theme: t}}
}

func (r *BashRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if cmd, ok := params["command"].(string); ok {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(bashPrefix() + cmd)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *BashRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	var output string
	var errMsg string
	if result != nil {
		output = result.Output
		if state == ToolError && result.Error != "" {
			errMsg = result.Error
		} else if state == ToolError && output != "" {
			errMsg = output
		}
	}
	parts := []string{}
	if !collapsed && output != "" {
		// Try to apply syntax highlighting for code output
		highlighted := r.applySyntaxHighlighting(output)
		parts = append(parts, r.RenderGenericOutput(highlighted, truncated, false, width))
	}
	parts = append(parts, r.RenderStatus(state, durationMs, width, errMsg))
	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}

// applySyntaxHighlighting attempts to apply syntax highlighting to code output
func (r *BashRenderer) applySyntaxHighlighting(output string) string {
	// Detect if output looks like code
	if !looksLikeCode(output) {
		return output
	}

	// Detect language from content
	lang := detectLanguage(output)
	if lang == "" {
		return output
	}

	// Use glamour with Chroma for syntax highlighting
	renderer, err := glamour.NewTermRenderer(
		glamour.WithAutoStyle(),
		glamour.WithWordWrap(80),
	)
	if err != nil {
		return output
	}

	// Wrap in code block for glamour to highlight
	codeBlock := "```" + lang + "\n" + output + "\n```"
	rendered, err := renderer.Render(codeBlock)
	if err != nil {
		return output
	}

	// Remove the code block markers and extra newlines
	rendered = strings.TrimPrefix(rendered, "```"+lang+"\n")
	rendered = strings.TrimSuffix(rendered, "\n```")
	return strings.TrimSpace(rendered)
}

// looksLikeCode checks if output looks like code
func looksLikeCode(output string) bool {
	// Check for common code patterns
	codeIndicators := []string{
		"func ", "import ", "package ", "class ", "def ", "function ",
		"if ", "for ", "while ", "return ", "err := ", "error(",
		"panic(", "fmt.", "console.", "print(",
	}
	for _, indicator := range codeIndicators {
		if strings.Contains(output, indicator) {
			return true
		}
	}
	return false
}

// detectLanguage detects the programming language from code content
func detectLanguage(output string) string {
	// Go
	if strings.Contains(output, "package ") && (strings.Contains(output, "func ") || strings.Contains(output, "import ")) {
		return "go"
	}
	// Python
	if strings.Contains(output, "def ") && strings.Contains(output, ":") {
		return "python"
	}
	// JavaScript/TypeScript
	if strings.Contains(output, "function ") || strings.Contains(output, "=>") || strings.Contains(output, "console.") {
		return "javascript"
	}
	// Shell
	if strings.Contains(output, "$ ") || strings.Contains(output, "#!/") {
		return "bash"
	}
	// JSON
	if strings.HasPrefix(strings.TrimSpace(output), "{") || strings.HasPrefix(strings.TrimSpace(output), "[") {
		return "json"
	}
	return ""
}
