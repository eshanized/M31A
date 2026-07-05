package a11y

import (
	"os"
	"strings"
	"sync"
)

// TerminalType represents the detected terminal emulator.
type TerminalType int

const (
	TermUnknown         TerminalType = iota
	TermITerm2                       // iTerm2 — OSC 1337 supported
	TermKitty                        // Kitty — different protocol
	TermWezTerm                      // WezTerm — supports some OSC 1337
	TermAlacritty                    // Alacritty — minimal OSC support
	TermWindowsTerminal              // Windows Terminal — limited OSC
	TermTMux                         // tmux wrapping — nested, unreliable
	TermSSH                          // SSH session — remote, may lose OSC
	TermGeneric                      // Other terminal — assume no support
)

var (
	detectedTerminal TerminalType
	detectOnce       sync.Once
)

// DetectTerminal returns the detected terminal type, cached after first call.
func DetectTerminal() TerminalType {
	detectOnce.Do(func() {
		detectedTerminal = detectTerminal()
	})
	return detectedTerminal
}

// detectTerminal checks TERM_PROGRAM, TERM, and SSH/TMUX env vars.
func detectTerminal() TerminalType {
	// Check for SSH session first — OSC may not reach the local terminal.
	if os.Getenv("SSH_CONNECTION") != "" || os.Getenv("SSH_CLIENT") != "" || os.Getenv("SSH_TTY") != "" {
		return TermSSH
	}

	// Check for tmux — nested OSC is unreliable.
	if os.Getenv("TMUX") != "" {
		return TermTMux
	}

	TERM_PROGRAM := strings.ToLower(os.Getenv("TERM_PROGRAM"))

	switch TERM_PROGRAM {
	case "iterm.app":
		return TermITerm2
	case "kitty":
		return TermKitty
	case "wezterm":
		return TermWezTerm
	case "alacritty":
		return TermAlacritty
	case "windowsterminal", "windows terminal":
		return TermWindowsTerminal
	}

	// Fallback: check TERM for generic classifications.
	TERM := strings.ToLower(os.Getenv("TERM"))
	if strings.HasPrefix(TERM, "xterm-kitty") {
		return TermKitty
	}

	return TermGeneric
}

// SupportsOSC1337 returns true if the detected terminal supports iTerm2 OSC 1337.
func SupportsOSC1337() bool {
	switch DetectTerminal() {
	case TermITerm2, TermWezTerm:
		return true
	default:
		return false
	}
}

// SupportsSGR returns true if the terminal can interpret ANSI SGR sequences
// for semantic emphasis (bold, italic, underline). All terminals support SGR,
// so this always returns true. It exists for explicit call-site clarity.
func SupportsSGR() bool {
	return true
}

// SupportsSemanticLabels returns true if the terminal can display semantic labels.
// Currently only iTerm2 supports SetSemanticLabel via OSC 1337.
func SupportsSemanticLabels() bool {
	return DetectTerminal() == TermITerm2
}

// SupportsRegions returns true if the terminal supports region push/pop.
// Currently only iTerm2 supports PushRegion/PopRegion via OSC 1337.
func SupportsRegions() bool {
	return DetectTerminal() == TermITerm2
}
