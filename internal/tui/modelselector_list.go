package tui

import (
	"fmt"
	"io"
	"strings"

	"github.com/charmbracelet/bubbles/list"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

var (
	modelVariantStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#9AA0A6")).Italic(true)
	favoriteStarStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#D77757")).Bold(true)
)

type ModelItem struct {
	Model      types.ModelInfo
	Provider   string
	IsFavorite bool
	UsageData  []float64 // usage frequency for sparkline (empty = no data)
}

func (i ModelItem) Title() string {
	name := i.Model.Name
	if name == "" {
		name = i.Model.ID
	}

	// Build title: ▶ ★  model-name              [Provider]
	var b strings.Builder

	// Favorite star
	if i.IsFavorite {
		b.WriteString(favoriteStarStyle.Render("★") + "  ")
	} else {
		b.WriteString("   ")
	}

	b.WriteString(name)

	return b.String()
}

func (i ModelItem) Description() string {
	// Line 1: Context and cost info
	ctxDesc := fmt.Sprintf("Context: %dK", i.Model.ContextLength/1024)
	costDesc := fmt.Sprintf("Cost: $%.2f/$%.2f per M tokens",
		i.Model.Pricing.InputPerMToken, i.Model.Pricing.OutputPerMToken)
	if i.Model.Pricing.InputPerMToken == 0 && i.Model.Pricing.OutputPerMToken == 0 {
		costDesc = "Cost: N/A"
	}
	line1 := fmt.Sprintf("%s  %s", ctxDesc, costDesc)

	// Line 2: Capability badges
	caps := capabilityBadges(i.Model.Capabilities)
	if caps != "" {
		line1 += "  " + caps
	}

	return line1
}

// capabilityBadges returns inline capability badges with checkmarks.
func capabilityBadges(c types.CapFlags) string {
	var parts []string
	checkStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#81C995"))
	labelStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#9AA0A6"))

	if c.Reasoning {
		parts = append(parts, checkStyle.Render("✓")+" "+labelStyle.Render("Thinking"))
	}
	if c.Vision {
		parts = append(parts, checkStyle.Render("✓")+" "+labelStyle.Render("Vision"))
	}
	if c.Tools {
		parts = append(parts, checkStyle.Render("✓")+" "+labelStyle.Render("Function Calling"))
	}
	return strings.Join(parts, "  ")
}

// DescriptionWidth returns the description truncated to the given display width.
func (i ModelItem) DescriptionWidth(width int) string {
	desc := i.Description()
	if width <= 0 {
		return desc
	}
	return TruncateWithEllipsis(desc, width)
}

func (i ModelItem) FilterValue() string {
	return strings.ToLower(i.Model.Name + " " + i.Model.ID + " " + i.Model.Description + " " + i.Model.Provider)
}

type modelItemDelegate struct {
	defaultDelegate list.DefaultDelegate
	theme           theme.Theme
}

func newModelItemDelegate(t theme.Theme) modelItemDelegate {
	d := list.NewDefaultDelegate()
	d.Styles.SelectedTitle = d.Styles.SelectedTitle.Foreground(lipgloss.Color("#D77757"))
	d.Styles.SelectedDesc = d.Styles.SelectedDesc.Foreground(lipgloss.Color("#D77757"))
	d.Styles.NormalTitle = d.Styles.NormalTitle.Foreground(lipgloss.Color(t.TextPrimary))
	d.Styles.NormalDesc = d.Styles.NormalDesc.Foreground(lipgloss.Color(t.TextSecondary))
	return modelItemDelegate{defaultDelegate: d, theme: t}
}

func (d modelItemDelegate) Height() int  { return 3 } // 3 lines: name, context/caps, sparkline
func (d modelItemDelegate) Spacing() int { return 1 }
func (d modelItemDelegate) Update(msg tea.Msg, m *list.Model) tea.Cmd {
	return d.defaultDelegate.Update(msg, m)
}
func (d modelItemDelegate) Render(w io.Writer, m list.Model, index int, item list.Item) {
	mi, ok := item.(ModelItem)
	if !ok {
		d.defaultDelegate.Render(w, m, index, item)
		return
	}

	isSelected := index == m.Index()

	// Line 1: Model name with provider badge
	name := mi.Model.Name
	if name == "" {
		name = mi.Model.ID
	}

	var line1 strings.Builder
	if isSelected {
		line1.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("#D77757")).Render("▶ "))
	} else {
		line1.WriteString("  ")
	}

	if mi.IsFavorite {
		line1.WriteString(favoriteStarStyle.Render("★") + "  ")
	} else {
		line1.WriteString("   ")
	}

	line1.WriteString(name)

	// Provider badge
	badge := providerBadge(mi.Provider)
	badgeStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#1E1E2E")).
		Background(lipgloss.Color("#81C995")).
		Padding(0, 1).
		Bold(true)
	padding := m.Width() - lipgloss.Width(line1.String()) - lipgloss.Width(badge) - 4
	if padding < 0 {
		padding = 0
	}
	line1.WriteString(strings.Repeat(" ", padding))
	line1.WriteString(badgeStyle.Render(" " + badge + " "))

	// Line 2: Context and capabilities
	ctxDesc := fmt.Sprintf("Context: %dK", mi.Model.ContextLength/1024)
	costDesc := fmt.Sprintf("Cost: $%.2f/$%.2f per M tokens",
		mi.Model.Pricing.InputPerMToken, mi.Model.Pricing.OutputPerMToken)
	if mi.Model.Pricing.InputPerMToken == 0 && mi.Model.Pricing.OutputPerMToken == 0 {
		costDesc = "Cost: N/A"
	}
	caps := capabilityBadges(mi.Model.Capabilities)
	line2 := fmt.Sprintf("     %s  %s", ctxDesc, costDesc)
	if caps != "" {
		line2 += "  " + caps
	}

	// Line 3: Sparkline (only show when there's real usage data)
	line3 := "     "
	hasRealData := false
	for _, v := range mi.UsageData {
		if v > 0 {
			hasRealData = true
			break
		}
	}
	if hasRealData {
		sparkWidth := 20
		if m.Width()-10 < sparkWidth {
			sparkWidth = m.Width() - 10
			if sparkWidth < 5 {
				sparkWidth = 5
			}
		}
		line3 += components.RenderSparkline(mi.UsageData, sparkWidth, d.theme)
		usageLabel := fmt.Sprintf("  Used %dx this week", int(mi.UsageData[len(mi.UsageData)-1]))
		line3 += lipgloss.NewStyle().Foreground(lipgloss.Color("#9AA0A6")).Render(usageLabel)
	}

	// Build the 3-line item
	content := lipgloss.JoinVertical(lipgloss.Left, line1.String(), line2, line3)

	// Pad to full width
	finalStyle := lipgloss.NewStyle().Width(m.Width())
	fmt.Fprint(w, finalStyle.Render(content))
}

func providerBadge(provider string) string {
	switch provider {
	case "openrouter":
		return "OR"
	case "zen":
		return "ZEN"
	default:
		return strings.ToUpper(provider)
	}
}

func capabilityString(c types.CapFlags) string {
	var parts []string
	if c.Tools {
		parts = append(parts, "tools")
	}
	if c.Reasoning {
		parts = append(parts, "reasoning")
	}
	if c.Vision {
		parts = append(parts, "vision")
	}
	if len(parts) == 0 {
		return "basic"
	}
	return strings.Join(parts, ", ")
}
