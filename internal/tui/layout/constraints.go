package layout

import (
	"github.com/charmbracelet/lipgloss"
)

// FlexDirection controls child arrangement within a Box.
type FlexDirection int

const (
	Column FlexDirection = iota
	Row
)

// Align controls alignment of children within a Box.
type Align int

const (
	AlignStart Align = iota
	AlignCenter
	AlignEnd
)

// Insets represents padding or margin on four sides.
type Insets struct {
	Top    int
	Right  int
	Bottom int
	Left   int
}

// UniformInsets creates insets with equal values on all sides.
func UniformInsets(v int) Insets {
	return Insets{Top: v, Right: v, Bottom: v, Left: v}
}

// SymmetricInsets creates insets with equal vertical and horizontal values.
func SymmetricInsets(vertical, horizontal int) Insets {
	return Insets{Top: vertical, Right: horizontal, Bottom: vertical, Left: horizontal}
}

// Horizontal returns the sum of Left + Right.
func (i Insets) Horizontal() int { return i.Left + i.Right }

// Vertical returns the sum of Top + Bottom.
func (i Insets) Vertical() int { return i.Top + i.Bottom }

// Renderer is anything that can render to a string given constraints.
type Renderer interface {
	Render(width, height int) string
}

// RenderFunc adapts a function to the Renderer interface.
type RenderFunc func(width, height int) string

func (f RenderFunc) Render(width, height int) string {
	return f(width, height)
}

// StaticRenderer wraps a pre-rendered string as a Renderer.
type StaticRenderer struct {
	Content string
}

func (s StaticRenderer) Render(_, _ int) string {
	return s.Content
}

// Box is the fundamental layout primitive. It arranges children
// either in a Row or Column, distributing space using flex factors.
type Box struct {
	// Fixed dimensions. 0 means auto (fill available space).
	Width  int
	Height int

	// Flex grow factor. Children with Flex > 0 share remaining space
	// proportionally after fixed-size children are allocated.
	Flex int

	// Direction of child layout.
	Direction FlexDirection

	// Cross-axis alignment of children.
	Align Align

	// Gap between children in cells.
	Gap int

	// Inner padding.
	Padding Insets

	// Border style.
	Border lipgloss.Border

	// Border color.
	BorderColor lipgloss.Color

	// Background color.
	Background lipgloss.Color

	// Content is rendered when there are no children.
	Content Renderer

	// Children are laid out according to Direction and Flex.
	Children []*Box
}

// NewRow creates a row-direction Box.
func NewRow(children ...*Box) *Box {
	return &Box{Direction: Row, Children: children}
}

// NewColumn creates a column-direction Box.
func NewColumn(children ...*Box) *Box {
	return &Box{Direction: Column, Children: children}
}

// WithContent sets the content renderer and returns the Box.
func (b *Box) WithContent(r Renderer) *Box {
	b.Content = r
	return b
}

// WithFlex sets the flex grow factor and returns the Box.
func (b *Box) WithFlex(f int) *Box {
	b.Flex = f
	return b
}

// WithSize sets fixed width and height and returns the Box.
func (b *Box) WithSize(w, h int) *Box {
	b.Width = w
	b.Height = h
	return b
}

// WithPadding sets uniform padding and returns the Box.
func (b *Box) WithPadding(p int) *Box {
	b.Padding = UniformInsets(p)
	return b
}

// WithBorder sets border and color and returns the Box.
func (b *Box) WithBorder(border lipgloss.Border, color lipgloss.Color) *Box {
	b.Border = border
	b.BorderColor = color
	return b
}

// WithBackground sets the background color and returns the Box.
func (b *Box) WithBackground(bg lipgloss.Color) *Box {
	b.Background = bg
	return b
}

// intrinsicWidth returns the minimum width needed for content, or 0 if unknown.
func (b *Box) intrinsicWidth() int {
	if b.Width > 0 {
		return b.Width
	}
	if b.Content != nil && len(b.Children) == 0 {
		return 0
	}
	if b.Direction == Row {
		total := 0
		for i, child := range b.Children {
			if i > 0 && b.Gap > 0 {
				total += b.Gap
			}
			total += child.intrinsicWidth()
		}
		return total + b.Padding.Horizontal()
	}
	maxW := 0
	for _, child := range b.Children {
		cw := child.intrinsicWidth()
		if cw > maxW {
			maxW = cw
		}
	}
	return maxW + b.Padding.Horizontal()
}

// intrinsicHeight returns the minimum height needed for content, or 0 if unknown.
func (b *Box) intrinsicHeight() int {
	if b.Height > 0 {
		return b.Height
	}
	if b.Content != nil && len(b.Children) == 0 {
		return 0
	}
	if b.Direction == Column {
		total := 0
		for i, child := range b.Children {
			if i > 0 && b.Gap > 0 {
				total += b.Gap
			}
			total += child.intrinsicHeight()
		}
		return total + b.Padding.Vertical()
	}
	maxH := 0
	for _, child := range b.Children {
		ch := child.intrinsicHeight()
		if ch > maxH {
			maxH = ch
		}
	}
	return maxH + b.Padding.Vertical()
}
