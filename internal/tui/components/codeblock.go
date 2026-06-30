package components

import (
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CodeBlock renders a syntax-highlighted code block with language label.
type CodeBlock struct {
	Language string
	Code     string
	Cache    *theme.StyleCache
	Width    int
}

// View renders the code block with syntax highlighting and line numbers.
func (cb CodeBlock) View() string {
	return RenderCodeBlock(cb.Code, cb.Language, cb.Cache, cb.Width)
}
