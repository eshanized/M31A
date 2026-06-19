package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
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
func RenderPage(chrome PageChrome, content string, header HeaderInfo, footer FooterInfo, t theme.Theme) string {
	bp := Detect(chrome.Width)

	headerLine := BuildHeader(header, chrome.Width, bp, t)
	footerLine := BuildFooter(footer, chrome.Width, bp, t)

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
// Layout (premium): M31A │ breadcrumb ········ model [provider] [ctx]
// Uses │ separators and leader dots to create visual depth.
func BuildHeader(info HeaderInfo, width int, bp Breakpoint, t theme.Theme) string {
	if width < 20 {
		return strings.Repeat(" ", width)
	}

	sep := lipgloss.NewStyle().Foreground(t.Border).Render(" │ ")
	sepW := 3 // visible width of " │ "

	// Left zone: brand name
	brand := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(info.Brand)
	brandW := lipgloss.Width(brand)

	// Center zone: breadcrumb (Standard+)
	var crumb string
	crumbW := 0
	if bp >= Standard && info.Breadcrumb != "" {
		crumb = lipgloss.NewStyle().Foreground(t.TextSecondary).Render(info.Breadcrumb)
		crumbW = lipgloss.Width(crumb)
	}

	// Right zone: context meter + model + provider (Full only)
	var right string
	rightW := 0
	if bp >= Full {
		var parts []string
		// Context meter — show when usage is meaningful
		if info.CtxTotal > 0 && info.CtxUsed > 0 {
			ctxMeter := renderContextMeter(info.CtxUsed, info.CtxTotal, info.CtxHistory, t)
			if ctxMeter != "" {
				parts = append(parts, ctxMeter)
			}
		}
		if info.ModelName != "" {
			parts = append(parts,
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.ModelName))
		}
		if info.Provider != "" {
			parts = append(parts, renderProvBadge(t, info.Provider))
		}
		if len(parts) > 0 {
			right = strings.Join(parts, " ")
			rightW = lipgloss.Width(right)
		}
	}

	// Compute leader-dot fill between crumb and right zone.
	// Total line: brand sep crumb dots sep right
	leftFixed := brandW + sepW + crumbW
	rightFixed := 0
	if right != "" {
		rightFixed = sepW + rightW
	}
	fillW := width - leftFixed - rightFixed
	if fillW < 1 {
		fillW = 1
	}
	dots := lipgloss.NewStyle().Foreground(t.BorderSubtle).Render(strings.Repeat("·", fillW))

	var result string
	if crumb != "" {
		result = brand + sep + crumb + dots
	} else {
		result = brand + dots
	}
	if right != "" {
		result += sep + right
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

// renderContextMeter renders a compact inline context usage bar for the header.
// Returns empty string when usage is low or total is unknown.
func renderContextMeter(used, total int, history []int, t theme.Theme) string {
	if total <= 0 || used <= 0 {
		return ""
	}
	pct := float64(used) / float64(total)
	// Only show meter when usage is notable (>30%)
	if pct < 0.30 {
		return ""
	}

	const barW = 6
	filled := int(pct * barW)
	if filled > barW {
		filled = barW
	}
	if filled < 0 {
		filled = 0
	}

	var ctxColor lipgloss.Color
	switch {
	case pct >= 0.90:
		ctxColor = t.Error
	case pct >= 0.70:
		ctxColor = t.Warning
	default:
		ctxColor = t.TextMuted
	}

	bar := strings.Repeat("█", filled) + strings.Repeat("░", barW-filled)
	pctLabel := intToStr(int(pct*100)) + "%"
	result := lipgloss.NewStyle().Foreground(ctxColor).Render(bar + " " + pctLabel)

	// Append sparkline of recent usage history
	if len(history) > 1 {
		sparkW := 5
		if len(history) < sparkW {
			sparkW = len(history)
		}
		spark := components.Sparkline{Values: history, Width: sparkW, Theme: t}
		result += " " + spark.Render()
	}

	return result
}

// BuildFooter renders the unified 1-line footer bar.
//
// Layout (premium): ⌂ cwd ⎇ branch │ ⠹ responding... │ ctrl+p · ctrl+b  $0.02
// Uses │ separators to create distinct zones with visual weight.
func BuildFooter(info FooterInfo, width int, bp Breakpoint, t theme.Theme) string {
	if width < 10 {
		return strings.Repeat(" ", width)
	}

	sep := lipgloss.NewStyle().Foreground(t.Border).Render(" │ ")
	dotSep := lipgloss.NewStyle().Foreground(t.BorderSubtle).Render(" · ")

	// Left zone: ⌂ cwd  ⎇ branch
	var leftParts []string
	if info.Cwd != "" {
		cwd := info.Cwd
		if bp < Standard {
			parts := strings.Split(cwd, "/")
			cwd = parts[len(parts)-1]
		}
		leftParts = append(leftParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("⌂ "+cwd))
	}
	if bp >= Standard && info.GitBranch != "" {
		leftParts = append(leftParts,
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render("⎇ "+info.GitBranch))
	}
	left := strings.Join(leftParts, "  ")

	// Center zone: animated operation with spinner
	var center string
	if bp >= Compact {
		switch {
		case info.LeaderActive:
			center = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("LEADER") +
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(" awaiting key")
		case info.Operation != "":
			spinner := info.SpinnerFrame
			if spinner == "" {
				spinner = "⋯"
			}
			isThinking := strings.HasPrefix(info.Operation, "thinking")
			var opStyle lipgloss.Style
			if isThinking {
				opStyle = lipgloss.NewStyle().Foreground(t.Thinking).Italic(true)
			} else {
				opStyle = lipgloss.NewStyle().Foreground(t.TextMuted)
			}
			spinStyle := lipgloss.NewStyle().Foreground(t.Brand)
			center = spinStyle.Render(spinner) + " " + opStyle.Render(info.Operation)
		}
	}

	// Right zone: keyboard hints (dot-separated) + cost
	var rightParts []string
	if ShowFooterHints(width) {
		hintStrs := make([]string, 0, len(info.KeyboardHints))
		for _, hint := range info.KeyboardHints {
			hintStrs = append(hintStrs,
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(hint))
		}
		if len(hintStrs) > 0 {
			rightParts = append(rightParts, strings.Join(hintStrs, dotSep))
		}
	}
	if ShowFooterCost(width) && info.ShowCost && info.TokenCount > 0 {
		rightParts = append(rightParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(formatTokenCount(info.TokenCount)))
		if info.Cost > 0 {
			var costStr string
			if info.Cost < 0.01 {
				costStr = "<$0.01"
			} else {
				costStr = "$" + formatCost(info.Cost)
			}
			rightParts = append(rightParts,
				lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
		}
	}
	right := strings.Join(rightParts, "  ")

	// Compose with │ separators between non-empty zones
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

	result := strings.Join(zones, sep)
	resultW := lipgloss.Width(result)

	// Overflow: drop right, then center, then truncate left
	if resultW > width && right != "" {
		zones2 := zones[:0]
		if left != "" {
			zones2 = append(zones2, left)
		}
		if center != "" {
			zones2 = append(zones2, center)
		}
		result = strings.Join(zones2, sep)
		resultW = lipgloss.Width(result)
	}
	if resultW > width && center != "" {
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

func renderProvBadge(t theme.Theme, provider string) string {
	short := shortProviderName(provider)
	// Render as a subtle [tag] with border-colored brackets
	bracket := lipgloss.NewStyle().Foreground(t.Border).Render
	text := lipgloss.NewStyle().Foreground(t.TextMuted).Render(short)
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
		return strings.TrimRight(strings.TrimRight(
			strings.ReplaceAll(
				strings.Replace(
					formatFloat1(float64(n)/1000), ".", ".", 1),
				"0", "0"),
			"0"), ".") + "K ctx"
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

// CenterText centers a styled text within a given width.
func CenterText(text string, style lipgloss.Style, width int) string {
	rendered := style.Render(text)
	textWidth := lipgloss.Width(rendered)
	padding := width - textWidth
	if padding <= 0 {
		return rendered
	}
	leftPad := padding / 2
	rightPad := padding - leftPad
	return strings.Repeat(" ", leftPad) + rendered + strings.Repeat(" ", rightPad)
}
