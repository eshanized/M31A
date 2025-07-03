package tui

import (
	"fmt"
	"io"
	"strings"

	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/lipgloss"
	tea "github.com/charmbracelet/bubbletea"
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
}

func (i ModelItem) Title() string {
	name := i.Model.Name
	if name == "" {
		name = i.Model.ID
	}
	title := fmt.Sprintf("%s [%s]", name, providerBadge(i.Provider))

	if i.Model.Variant != nil {
		title += " " + modelVariantStyle.Render(*i.Model.Variant)
	}

	return title
}

func (i ModelItem) Description() string {
	var desc string
	if i.IsFavorite {
		desc = favoriteStarStyle.Render("★") + " favorite | "
	}

	estCost := (i.Model.Pricing.InputPerMToken * 0.1) + (i.Model.Pricing.OutputPerMToken * 0.05)
	costDesc := fmt.Sprintf("$%.4f (100K in + 50K out)", estCost)
	if i.Model.Pricing.InputPerMToken == 0 && i.Model.Pricing.OutputPerMToken == 0 {
		costDesc = "pricing N/A"
	}
	ctxDesc := fmt.Sprintf("context: %dK", i.Model.ContextLength/1024)
	caps := capabilityString(i.Model.Capabilities)
	desc += fmt.Sprintf("%s · %s · %s", costDesc, ctxDesc, caps)
	return desc
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
}

func newModelItemDelegate() modelItemDelegate {
	d := list.NewDefaultDelegate()
	d.Styles.SelectedTitle = d.Styles.SelectedTitle.Foreground(lipgloss.Color("#D77757"))
	d.Styles.SelectedDesc = d.Styles.SelectedDesc.Foreground(lipgloss.Color("#D77757"))
	return modelItemDelegate{defaultDelegate: d}
}

func (d modelItemDelegate) Height() int                               { return d.defaultDelegate.Height() }
func (d modelItemDelegate) Spacing() int                              { return d.defaultDelegate.Spacing() }
func (d modelItemDelegate) Update(msg tea.Msg, m *list.Model) tea.Cmd { return d.defaultDelegate.Update(msg, m) }
func (d modelItemDelegate) Render(w io.Writer, m list.Model, index int, item list.Item) {
	d.defaultDelegate.Render(w, m, index, item)
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
