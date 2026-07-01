package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// HeaderInfo carries the data needed to render the unified header.
type HeaderInfo struct {
	Brand      string // "M31A"
	Breadcrumb string // screen name, phase breadcrumb, or git branch
	ModelName  string // active model name
	Provider   string // provider short name (OR, ZEN)
	CtxUsed    int    // context tokens used
	CtxTotal   int    // context tokens total
	CtxHistory []int  // recent context usage readings for sparkline
}

// FooterInfo carries the data needed to render the unified footer.
type FooterInfo struct {
	Cwd           string   // working directory basename
	GitBranch     string   // current git branch
	Operation     string   // "thinking...", "responding...", phase name
	LeaderActive  bool     // leader key mode active
	KeyboardHints []string // e.g. "ctrl+p commands"
	TokenCount    int      // total tokens used
	Cost          float64  // session cost
	ShowCost      bool     // whether to display cost
	SpinnerFrame  string   // animated spinner character
}

// PageChrome holds the computed header and footer strings along with
// dimensions used for content layout.
type PageChrome struct {
	Width  int
	Height int
}

// ContentHeight returns the number of rows available for screen content.
func (p PageChrome) ContentHeight() int {
	h := p.Height - ChromeHeight
	if h < 1 {
		h = 1
	}
	return h
}

// ContentWidth returns the width available for screen content.
func (p PageChrome) ContentWidth() int {
	return p.Width
}

// RenderPage composes the unified page layout: 1-line header + content + 1-line footer.
// The total output is exactly Height rows.
func RenderPage(chrome PageChrome, content string, header HeaderInfo, footer FooterInfo, t theme.Theme, cache *theme.StyleCache) string {
	bp := Detect(chrome.Width)

	headerLine := BuildHeader(header, chrome.Width, bp, t, cache)
	footerLine := BuildFooter(footer, chrome.Width, bp, t, cache)

	// Ensure content fits in exactly ContentHeight rows
	contentHeight := chrome.ContentHeight()
	contentLines := strings.Split(content, "\n")

	// Pad or clip content to exact height
	if len(contentLines) > contentHeight {
		contentLines = contentLines[:contentHeight]
	}
	for len(contentLines) < contentHeight {
		contentLines = append(contentLines, "")
	}

	contentBlock := strings.Join(contentLines, "\n")

	return lipgloss.JoinVertical(lipgloss.Left,
		headerLine,
		contentBlock,
		footerLine,
	)
}

// BuildHeader renders the unified 1-line header bar.
//
// Layout: M31A  breadcrumb ···························· model
// Simplified chrome: no leader dots, no provider badge, no context meter.
func BuildHeader(info HeaderInfo, width int, bp Breakpoint, t theme.Theme, cache *theme.StyleCache) string {
	if width < 20 {
		return strings.Repeat(" ", width)
	}

	s := cache.S

	// Left zone: brand name
	brand := s.HeaderBrand.Render(info.Brand)
	brandW := lipgloss.Width(brand)

	// Center zone: breadcrumb (Standard+)
	var crumb string
	crumbW := 0
	if bp >= Standard && info.Breadcrumb != "" {
		crumb = s.HeaderCrumb.Render(info.Breadcrumb)
		crumbW = lipgloss.Width(crumb)
	}

	// Right zone: model name only (Full+), no provider badge, no context meter
	var right string
	rightW := 0
	if bp >= Full && info.ModelName != "" {
		right = s.Muted.Render(info.ModelName)
		rightW = lipgloss.Width(right)
	}

	// Compute space between crumb and right zone using spaces (no leader dots)
	leftFixed := brandW + crumbW
	if crumb != "" {
		leftFixed += 2 // spacer between brand and crumb
	}
	rightFixed := rightW
	fillW := width - leftFixed - rightFixed
	if fillW < 1 {
		fillW = 1
	}
	fill := strings.Repeat(" ", fillW)

	var result string
	if crumb != "" {
		result = brand + "  " + crumb + fill
	} else {
		result = brand + fill
	}
	if right != "" {
		result += right
	}

	// Final width guard
	resultW := lipgloss.Width(result)
	if resultW > width {
		result = truncateToWidth(result, width)
	} else if resultW < width {
		result += strings.Repeat(" ", width-resultW)
	}
	return result
}

// BuildFooter renders the unified 1-line footer bar.
//
// Layout: ⌂ cwd ⎇ branch  ·  ⠹ responding...  ·  ctrl+p · ctrl+b
// Simplified chrome: no cost, no context ring, no │ separators.
func BuildFooter(info FooterInfo, width int, bp Breakpoint, t theme.Theme, cache *theme.StyleCache) string {
	if width < 10 {
		return strings.Repeat(" ", width)
	}

	s := cache.S

	dotSep := s.SeparatorLine.Render(" · ")

	// Left zone: ⌂ cwd  ⎇ branch
	var leftParts []string
	if info.Cwd != "" {
		cwd := info.Cwd
		if bp < Standard {
			parts := strings.Split(cwd, "/")
			cwd = parts[len(parts)-1]
		}
		leftParts = append(leftParts, s.FooterCwd.Render("⌂ "+cwd))
	}
	if bp >= Standard && info.GitBranch != "" {
		leftParts = append(leftParts, s.FooterBranch.Render("⎇ "+info.GitBranch))
	}
	left := strings.Join(leftParts, "  ")

	// Center zone: animated operation with spinner
	var center string
	if bp >= Compact {
		switch {
		case info.LeaderActive:
			center = s.FooterLeader.Render("LEADER") +
				s.FooterOp.Render(" awaiting key")
		case info.Operation != "":
			spinner := info.SpinnerFrame
			if spinner == "" {
				spinner = "⋯"
			}
			isThinking := strings.HasPrefix(info.Operation, "thinking")
			var opStyle = s.FooterOp
			if isThinking {
				opStyle = s.Thinking
			}
			spinStyle := s.SpinnerBrand
			center = spinStyle.Render(spinner) + " " + opStyle.Render(info.Operation)
		}
	}

	// Right zone: keyboard hints only (no cost, no token count)
	var rightParts []string
	if ShowFooterHints(width) {
		hintStrs := make([]string, 0, len(info.KeyboardHints))
		for _, hint := range info.KeyboardHints {
			hintStrs = append(hintStrs, s.FooterHint.Render(hint))
		}
		if len(hintStrs) > 0 {
			rightParts = append(rightParts, strings.Join(hintStrs, dotSep))
		}
	}
	right := strings.Join(rightParts, "  ")

	// Compose with · separators between non-empty zones
	var zones []string
	if left != "" {
		zones = append(zones, left)
	}
	if center != "" {
		zones = append(zones, center)
	}
	if right != "" {
		zones = append(zones, right)
	}

	result := strings.Join(zones, dotSep)
	resultW := lipgloss.Width(result)

	// Overflow: progressively drop zones then truncate.
	if resultW > width && right != "" {
		zones2 := zones[:0]
		if left != "" {
			zones2 = append(zones2, left)
		}
		if center != "" {
			zones2 = append(zones2, center)
		}
		result = strings.Join(zones2, dotSep)
		resultW = lipgloss.Width(result)
	}
	if resultW > width && left != "" {
		result = left
		resultW = lipgloss.Width(result)
	}
	if resultW > width {
		result = truncateToWidth(result, width)
		resultW = lipgloss.Width(result)
	}

	if resultW < width {
		result += strings.Repeat(" ", width-resultW)
	}
	return result
}

// assembleThreeZone lays out left/center/right content in a single line.
// Overflow is handled gracefully: drop right first, then center, then truncate left.
func assembleThreeZone(left, center, right string, width int) string {
	leftW := lipgloss.Width(left)
	centerW := lipgloss.Width(center)
	rightW := lipgloss.Width(right)

	totalUsed := leftW + centerW + rightW
	padding := width - totalUsed

	// Drop right zone if overflow
	if padding < 2 && rightW > 0 {
		right = ""
		rightW = 0
		padding = width - leftW - centerW
	}

	// Drop center zone if still overflow
	if padding < 2 && centerW > 0 {
		center = ""
		centerW = 0
		padding = width - leftW - rightW
	}

	if padding < 0 {
		padding = 0
	}

	// Distribute padding: center gets balanced space, right gets remaining
	padLeft := (padding - centerW) / 2
	padRight := padding - padLeft - centerW

	if padLeft < 0 {
		padLeft = 0
	}
	if padRight < 0 {
		padRight = 0
	}

	result := left +
		strings.Repeat(" ", padLeft) +
		center +
		strings.Repeat(" ", padRight) +
		right

	// Final truncation guard
	resultW := lipgloss.Width(result)
	if resultW > width {
		result = truncateToWidth(result, width)
	} else if resultW < width {
		result += strings.Repeat(" ", width-resultW)
	}

	return result
}

func renderProvBadge(s theme.SemanticStyles, provider string) string {
	short := shortProviderName(provider)
	bracket := s.SeparatorV.Render
	text := s.Muted.Render(short)
	return bracket("[") + text + bracket("]")
}

func shortProviderName(name string) string {
	switch {
	case strings.Contains(strings.ToLower(name), "openrouter"):
		return "OR"
	case strings.Contains(strings.ToLower(name), "zen"):
		return "ZEN"
	default:
		if len(name) > 3 {
			return name[:3]
		}
		return name
	}
}

func formatTokenCount(n int) string {
	if n >= 1000 {
		f := formatFloat1(float64(n) / 1000)
		// Trim trailing zero after decimal (e.g. "1.0" → "1") but keep non-zero fractions
		f = strings.TrimSuffix(f, ".0")
		return f + "K ctx"
	}
	return intToStr(n) + " ctx"
}

func formatFloat1(f float64) string {
	whole := int(f)
	frac := int((f - float64(whole)) * 10)
	if frac < 0 {
		frac = 0
	}
	return intToStr(whole) + "." + intToStr(frac)
}

func formatCost(f float64) string {
	cents := int(f * 100)
	dollars := cents / 100
	remainder := cents % 100
	if remainder < 10 {
		return intToStr(dollars) + ".0" + intToStr(remainder)
	}
	return intToStr(dollars) + "." + intToStr(remainder)
}

func intToStr(n int) string {
	if n == 0 {
		return "0"
	}
	neg := false
	if n < 0 {
		neg = true
		n = -n
	}
	digits := make([]byte, 0, 10)
	for n > 0 {
		digits = append(digits, byte('0'+n%10))
		n /= 10
	}
	// Reverse
	for i, j := 0, len(digits)-1; i < j; i, j = i+1, j-1 {
		digits[i], digits[j] = digits[j], digits[i]
	}
	if neg {
		return "-" + string(digits)
	}
	return string(digits)
}
