package a11y

import "fmt"

// Announce returns a screen-reader-friendly escape sequence for a status
// message. Uses OSC 1337 (iTerm2/WezTerm) as primary and ANSI SGR bold as
// fallback for all other terminals.
func Announce(text string) string {
	if SupportsOSC1337() {
		return fmt.Sprintf("\x1b]1337;SetStatusMessage=%s\x07", text)
	}
	// SGR fallback: wrap in bold so screen readers can detect emphasis.
	return fmt.Sprintf("\033[1m%s\033[0m", text)
}

// AnnounceStreamContent announces a streaming response chunk.
// Uses italic SGR to distinguish streaming content from final output.
func AnnounceStreamContent(chunk string) string {
	if SupportsOSC1337() {
		return fmt.Sprintf("\x1b]1337;SetStatusMessage=stream:%s\x07", chunk)
	}
	return fmt.Sprintf("\033[3m%s\033[0m", chunk)
}

// AnnouncePhaseChange announces a workflow phase transition.
// Uses bold SGR for emphasis on phase changes.
func AnnouncePhaseChange(phase string) string {
	if SupportsOSC1337() {
		return fmt.Sprintf("\x1b]1337;SetStatusMessage=phase:%s\x07", phase)
	}
	return fmt.Sprintf("\033[1m[phase] %s\033[0m", phase)
}

// AnnounceError announces an error state.
// Uses bold+red SGR (SGR 1;31) for error emphasis.
func AnnounceError(err string) string {
	if SupportsOSC1337() {
		return fmt.Sprintf("\x1b]1337;SetStatusMessage=error:%s\x07", err)
	}
	return fmt.Sprintf("\033[1;31m[error] %s\033[0m", err)
}

// AnnounceSuccess announces a success state.
// Uses bold+green SGR (SGR 1;32) for success emphasis.
func AnnounceSuccess(msg string) string {
	if SupportsOSC1337() {
		return fmt.Sprintf("\x1b]1337;SetStatusMessage=success:%s\x07", msg)
	}
	return fmt.Sprintf("\033[1;32m[ok] %s\033[0m", msg)
}

// AnnounceProgress announces a progress update with current/total counts.
// Uses underline SGR for progress emphasis.
func AnnounceProgress(current, total int, label string) string {
	text := fmt.Sprintf("%s %d/%d", label, current, total)
	if SupportsOSC1337() {
		return fmt.Sprintf("\x1b]1337;SetStatusMessage=progress:%s\x07", text)
	}
	return fmt.Sprintf("\033[4m%s\033[0m", text)
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
