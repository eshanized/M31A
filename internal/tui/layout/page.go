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
// Layout: left (brand) · center (breadcrumb) · right (model + provider)
// Adapts based on breakpoint:
//   - UltraNarrow: not called (RenderTooNarrow handles this)
//   - Compact: brand only (left)
//   - Standard: brand + breadcrumb (left + center)
//   - Full: brand + breadcrumb + model/provider (all zones)
func BuildHeader(info HeaderInfo, width int, bp Breakpoint, t theme.Theme) string {
	if width < 20 {
		return strings.Repeat(" ", width)
	}

	// Left zone: brand
	left := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(info.Brand)

	// Center zone: breadcrumb
	var center string
	if bp >= Standard && info.Breadcrumb != "" {
		center = lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.Breadcrumb)
	}

	// Right zone: model + provider badge
	var right string
	if bp >= Full {
		var parts []string
		if info.ModelName != "" {
			parts = append(parts,
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.ModelName))
		}
		if info.Provider != "" {
			parts = append(parts, renderProvBadge(t, info.Provider))
		}
		right = strings.Join(parts, " ")
	}

	return assembleThreeZone(left, center, right, width)
}

// BuildFooter renders the unified 1-line footer bar.
//
// Layout: left (cwd + branch) · center (operation) · right (hints + cost)
// Adapts based on breakpoint:
//   - Compact: cwd only (left)
//   - Standard: cwd + branch + operation + hints
//   - Full: all zones including cost
func BuildFooter(info FooterInfo, width int, bp Breakpoint, t theme.Theme) string {
	if width < 10 {
		return strings.Repeat(" ", width)
	}

	// Left zone: cwd + branch
	var leftParts []string
	if info.Cwd != "" {
		cwd := info.Cwd
		if bp < Standard {
			// Compact: show basename only
			parts := strings.Split(cwd, "/")
			cwd = parts[len(parts)-1]
		}
		leftParts = append(leftParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(cwd))
	}
	if bp >= Standard && info.GitBranch != "" {
		leftParts = append(leftParts,
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render(info.GitBranch))
	}
	left := strings.Join(leftParts, "  ")

	// Center zone: operation status
	var center string
	if bp >= Compact {
		switch {
		case info.LeaderActive:
			center = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x") +
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(" waiting")
		case info.Operation != "":
			spinner := info.SpinnerFrame
			if spinner == "" {
				spinner = "~"
			}
			center = t.Spinner.Render(spinner) + " " +
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.Operation)
		}
	}

	// Right zone: hints + cost
	var rightParts []string
	if ShowFooterHints(width) {
		for _, hint := range info.KeyboardHints {
			rightParts = append(rightParts,
				lipgloss.NewStyle().Foreground(t.TextMuted).Render(hint))
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

	return assembleThreeZone(left, center, right, width)
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
	short := strings.ToUpper(shortProviderName(provider))
	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("[" + short + "]")
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
