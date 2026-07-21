package theme

// ThemeDefinition represents a theme preset.
// M31A ships with a single official theme.
type ThemeDefinition struct {
	ID          string
	Name        string
	Description string
	Mode        Mode
	Constructor func() Theme
}

// M31A is the single official theme definition.
var m31aDef = ThemeDefinition{
	ID:          "m31a",
	Name:        "M31A",
	Description: "Apple-inspired dark theme — calm, elegant, premium",
	Mode:        ModeDark,
	Constructor: M31A,
}

// Available returns all available theme definitions.
// M31A ships with exactly one theme.
func Available() []ThemeDefinition {
	return []ThemeDefinition{m31aDef}
}

// ByID returns a theme by its ID.
// Only "m31a" and "dark" are valid IDs. "dark" returns the M31A theme
// for backward compatibility.
func ByID(id string) (Theme, bool) {
	switch id {
	case "m31a", "dark", "light", "auto":
		return M31A(), true
	default:
		return Theme{}, false
	}
}
