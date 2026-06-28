package a11y

import "fmt"

// Announce returns an ANSI escape sequence that screen readers can use
// to announce a status message. Uses the iTerm2 SetStatusMessage protocol
// which is supported by iTerm2 and WezTerm. Returns empty string if the
// terminal does not support OSC 1337.
func Announce(text string) string {
	if !SupportsOSC1337() {
		return ""
	}
	return fmt.Sprintf("\x1b]1337;SetStatusMessage=%s\x07", text)
}

// DescribeElement returns a semantic description string suitable for
// screen reader context. Uses iTerm2's SetSemanticLabel via OSC 1337.
// Returns empty string if the terminal does not support semantic labels.
func DescribeElement(label, role string) string {
	if !SupportsSemanticLabels() {
		return ""
	}
	if role == "" {
		role = "text"
	}
	return fmt.Sprintf("\x1b]1337;SetSemanticLabel=%s:%s\x07", role, label)
}

// RegionStart marks the beginning of a named region for screen readers.
// Uses iTerm2's PushRegion via OSC 1337. Returns empty string if the
// terminal does not support region annotations.
func RegionStart(name string) string {
	if !SupportsRegions() {
		return ""
	}
	return fmt.Sprintf("\x1b]1337;PushRegion=%s\x07", name)
}

// RegionEnd marks the end of a named region for screen readers.
// Uses iTerm2's PopRegion via OSC 1337. Returns empty string if the
// terminal does not support region annotations.
func RegionEnd(name string) string {
	if !SupportsRegions() {
		return ""
	}
	return fmt.Sprintf("\x1b]1337;PopRegion=%s\x07", name)
}
