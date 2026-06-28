package components

import (
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Context carries layout and theme information to components.
// It replaces the pattern of passing width/height/theme as separate parameters.
type Context struct {
	Width   int
	Height  int
	Theme   theme.Theme
	Cache   *theme.StyleCache
	Focused bool
	Hovered bool
}

// NewContext creates a Context with the given dimensions and theme.
func NewContext(w, h int, t theme.Theme) Context {
	return Context{
		Width:  w,
		Height: h,
		Theme:  t,
		Cache:  theme.NewStyleCache(t),
	}
}
