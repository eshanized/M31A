package a11y

import "fmt"

// Announce returns an ANSI escape sequence that screen readers can use
// to announce a status message. Uses the iTerm2 SetStatusMessage protocol
// which is supported by many modern terminals.
func Announce(text string) string {
	return fmt.Sprintf("\x1b]1337;SetStatusMessage=%s\x07", text)
}

// DescribeElement returns a semantic description string suitable for
// screen reader context. This is a passive annotation that can be
// embedded in output for compatible terminals.
func DescribeElement(label, role string) string {
	if role == "" {
		role = "text"
	}
	return fmt.Sprintf("\x1b]1337;SetSemanticLabel=%s:%s\x07", role, label)
}

// RegionStart marks the beginning of a named region for screen readers.
func RegionStart(name string) string {
	return fmt.Sprintf("\x1b]1337;PushRegion=%s\x07", name)
}

// RegionEnd marks the end of a named region for screen readers.
func RegionEnd(name string) string {
	return fmt.Sprintf("\x1b]1337;PopRegion=%s\x07", name)
}
