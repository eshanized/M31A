package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

const (
	sidebarDefaultWidth = 28
	sidebarMinWidth     = 20
	sidebarMaxWidth     = 50
)

// SidebarModel manages the collapsible sidebar panel that shows git status.
type SidebarModel struct {
	git     *git.Git
	theme   theme.Theme
	files   []git.FileStatus
	branch  string
	remote  string
	visible bool
	width   int
	height  int // total terminal height passed from AppState

	// File cursor for keyboard navigation
	focused      bool
	fileCursor   int
	scrollOffset int              // first visible file index in the scrollable file section
	flatFiles    []git.FileStatus // flattened list for cursor navigation

	// Optional fields for enhanced display
	version   string
	sessionID string

	loading bool
}

// NewSidebarModel creates a new SidebarModel.
func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
	return &SidebarModel{
		git:     g,
		theme:   t,
		visible: true,
		width:   sidebarDefaultWidth,
	}
}

// SetVersion sets the version string displayed in the header.
func (s *SidebarModel) SetVersion(v string) {
	s.version = v
}

// SetSessionID sets the active session ID for display.
func (s *SidebarModel) SetSessionID(id string) {
	s.sessionID = id
}

// Toggle shows/hides the sidebar.
func (s *SidebarModel) Toggle() {
	s.visible = !s.visible
}

// IsVisible returns true if the sidebar is shown.
func (s *SidebarModel) IsVisible() bool {
	return s.visible
}

// IsFocused returns true if the sidebar has keyboard focus.
func (s *SidebarModel) IsFocused() bool {
	return s.focused
}

// Focus gives keyboard focus to the sidebar.
func (s *SidebarModel) Focus() {
	s.focused = true
}

// Blur removes keyboard focus from the sidebar.
func (s *SidebarModel) Blur() {
	s.focused = false
}

// ToggleFocus toggles sidebar keyboard focus.
func (s *SidebarModel) ToggleFocus() {
	s.focused = !s.focused
	if s.focused && len(s.flatFiles) > 0 && s.fileCursor >= len(s.flatFiles) {
		s.fileCursor = 0
	}
}

// GetWidth returns the sidebar display width (0 if hidden).
func (s *SidebarModel) GetWidth() int {
	if !s.visible {
		return 0
	}
	return s.width
}

// SelectedFile returns the currently selected file, or nil if none.
func (s *SidebarModel) SelectedFile() *git.FileStatus {
	if len(s.flatFiles) == 0 || s.fileCursor >= len(s.flatFiles) {
		return nil
	}
	return &s.flatFiles[s.fileCursor]
}

// IncreaseWidth grows the sidebar width by 2, up to the max.
func (s *SidebarModel) IncreaseWidth() {
	if s.width+2 <= sidebarMaxWidth {
		s.width += 2
	}
}

// DecreaseWidth shrinks the sidebar width by 2, down to the min.
func (s *SidebarModel) DecreaseWidth() {
	if s.width-2 >= sidebarMinWidth {
		s.width -= 2
	}
}

// SetTheme updates the sidebar theme.
func (s *SidebarModel) SetTheme(t theme.Theme) {
	s.theme = t
}

// SetHeight sets the total available height for the sidebar so it can
// constrain the file list and enable scrolling.
func (s *SidebarModel) SetHeight(h int) {
	s.height = h
}

// HandleKey processes a key event when the sidebar is focused.
// Returns a command to execute (e.g., show diff) or nil.
func (s *SidebarModel) HandleKey(msg tea.KeyMsg) tea.Cmd {
	switch msg.String() {
	case "up", "k":
		if s.fileCursor > 0 {
			s.fileCursor--
			s.clampScroll()
		}
	case "down", "j":
		if s.fileCursor < len(s.flatFiles)-1 {
			s.fileCursor++
			s.clampScroll()
		}
	case "home", "g":
		s.fileCursor = 0
		s.scrollOffset = 0
	case "end", "G":
		if len(s.flatFiles) > 0 {
			s.fileCursor = len(s.flatFiles) - 1
			s.clampScroll()
		}
	case "enter":
		if f := s.SelectedFile(); f != nil && s.git != nil {
			file := *f
			g := s.git
			return func() tea.Msg {
				diff, err := g.DiffFile(file.Path, file.Status)
				if err != nil {
					return ToastMsg{
						Text: fmt.Sprintf("Cannot show diff for %s: %v", file.Path, err),
						Type: "error",
					}
				}
				if strings.TrimSpace(diff) == "" {
					return ToastMsg{
						Text: fmt.Sprintf("No diff available for %s", file.Path),
						Type: "info",
					}
				}
				return DiffScreenMsg{
					Diff:  diff,
					Title: fmt.Sprintf("git diff — %s", file.Path),
					Lines: strings.Split(diff, "\n"),
				}
			}
		}
	case "esc":
		s.focused = false
	}
	return nil
}

// fileListViewport returns the number of lines available for the scrollable
// file list area. Fixed chrome: header(2) + sep(1) + git-section(≈5) +
// files-label(2) + session-section(3) + shortcuts-section(6) = ~19 lines.
// We use a conservative fixed overhead and fall back to showing all files
// if height has not been set yet.
const sidebarFixedOverhead = 20

func (s *SidebarModel) fileListViewport() int {
	if s.height <= sidebarFixedOverhead {
		return 999 // height not set yet — show everything
	}
	vp := s.height - sidebarFixedOverhead
	if vp < 3 {
		vp = 3
	}
	return vp
}

// clampScroll adjusts scrollOffset so the cursor is always visible.
func (s *SidebarModel) clampScroll() {
	vp := s.fileListViewport()
	if s.fileCursor < s.scrollOffset {
		s.scrollOffset = s.fileCursor
	}
	if s.fileCursor >= s.scrollOffset+vp {
		s.scrollOffset = s.fileCursor - vp + 1
	}
	if s.scrollOffset < 0 {
		s.scrollOffset = 0
	}
}

// rebuildFlatFiles rebuilds the flat file list for cursor navigation.
func (s *SidebarModel) rebuildFlatFiles() {
	s.flatFiles = make([]git.FileStatus, 0, len(s.files))
	groups := groupFilesByStatus(s.files)
	groupOrder := []struct {
		key        string
		statusChar string
	}{
		{"modified", "M"},
		{"added", "A"},
		{"deleted", "D"},
		{"renamed", "R"},
		{"untracked", "?"},
		{"other", ""},
	}
	for _, g := range groupOrder {
		paths, ok := groups[g.key]
		if !ok {
			continue
		}
		for _, p := range paths {
			s.flatFiles = append(s.flatFiles, git.FileStatus{
				Path:   p,
				Status: g.statusChar,
			})
		}
	}
	// Clamp cursor
	if s.fileCursor >= len(s.flatFiles) {
		s.fileCursor = len(s.flatFiles) - 1
	}
	if s.fileCursor < 0 {
		s.fileCursor = 0
	}
}

// refreshCmd returns a tea.Cmd that loads git status asynchronously.
func (s *SidebarModel) refreshCmd() tea.Cmd {
	g := s.git
	return func() tea.Msg {
		if g == nil || !g.IsRepo() {
			return SidebarRefreshMsg{}
		}
		branch, _ := g.CurrentBranch()
		remote, _ := g.RemoteTracking()
		files, err := g.StatusPorcelain()
		if err != nil {
			return SidebarRefreshMsg{Branch: branch, Remote: remote}
		}
		sidebarFiles := make([]SidebarFile, 0, len(files))
		for _, f := range files {
			sidebarFiles = append(sidebarFiles, SidebarFile{
				Path:   f.Path,
				Status: f.Status,
			})
		}
		return SidebarRefreshMsg{Files: sidebarFiles, Branch: branch, Remote: remote}
	}
}

// Update handles sidebar-specific messages.
func (s *SidebarModel) Update(msg tea.Msg) (*SidebarModel, tea.Cmd) {
	switch msg := msg.(type) {
	case SidebarRefreshMsg:
		s.branch = msg.Branch
		s.remote = msg.Remote
		files := make([]git.FileStatus, 0, len(msg.Files))
		for _, f := range msg.Files {
			files = append(files, git.FileStatus{Status: f.Status, Path: f.Path})
		}
		s.files = files
		s.rebuildFlatFiles()
		s.loading = false
		return s, nil
	}

	if s.visible && s.branch == "" && !s.loading {
		s.loading = true
		return s, s.refreshCmd()
	}

	return s, nil
}

// View renders the sidebar with a thin right border.
func (s *SidebarModel) View() string {
	if !s.visible {
		return ""
	}
	t := s.theme
	w := s.width
	contentW := w - 1

	var lines []string

	// ── Header: M31A + version badge ──────────────────────────────────────────
	version := s.version
	if version == "" {
		version = "dev"
	}
	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		PaddingLeft(1).
		Render("M31A")
	versionBadge := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		PaddingLeft(1).
		Render(version)
	headerRow := lipgloss.JoinHorizontal(lipgloss.Left, title, versionBadge)
	lines = append(lines, headerRow)

	// ── Gradient separator ────────────────────────────────────────────────────
	sepChars := []string{"▓", "▒", "░"}
	var sep strings.Builder
	for i := 0; i < len(sepChars) && i < contentW; i++ {
		sep.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(sepChars[i]))
	}
	remaining := contentW - len(sepChars)
	if remaining > 0 {
		sep.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", remaining)))
	}
	lines = append(lines, sep.String())

	// ── Git section ────────────────────────────────────────────────────────────
	if s.branch != "" {
		secLabel := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Bold(true).
			PaddingLeft(1).
			Render("GIT")
		lines = append(lines, "", secLabel)

		branchLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(1).
			Width(contentW).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)

		if s.remote != "" {
			remoteLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(3).
				Width(contentW).
				Render(s.remote)
			lines = append(lines, remoteLine)
		}

		modCount, addCount, delCount, untracked := countFileStatuses(s.files)
		if modCount+addCount+delCount+untracked > 0 {
			var pills []string
			if modCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Background(t.Warning).Foreground(t.Background).
					Padding(0, 1).Bold(true).
					Render(fmt.Sprintf("●%d", modCount)))
			}
			if addCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Background(t.Success).Foreground(t.Background).
					Padding(0, 1).Bold(true).
					Render(fmt.Sprintf("+%d", addCount)))
			}
			if delCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Background(t.Error).Foreground(t.Background).
					Padding(0, 1).Bold(true).
					Render(fmt.Sprintf("-%d", delCount)))
			}
			if untracked > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Foreground(t.TextMuted).
					Render(fmt.Sprintf("?%d", untracked)))
			}
			countLine := lipgloss.NewStyle().PaddingLeft(1).Render(strings.Join(pills, " "))
			lines = append(lines, countLine)
		}
	}

	// ── Files section ──────────────────────────────────────────────────────────
	lines = append(lines, "")
	filesLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		PaddingLeft(1).
		Render("FILES")
	if s.focused && len(s.flatFiles) > 0 {
		filesLabel += lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ↑↓ enter")
	}
	lines = append(lines, filesLabel)

	if len(s.files) == 0 {
		noFiles := lipgloss.NewStyle().
			Foreground(t.Success).
			PaddingLeft(2).
			Width(contentW).
			Render("✓ working tree clean")
		lines = append(lines, noFiles)
	} else {
		// Build all file-section rows first, then apply viewport clipping.
		// groupFirstIdx stores the first flat-file index for each group row so we
		// can decide whether to show the header during scrolling.
		type fileRow struct {
			text       string
			isFile     bool // true = counts toward flat-file index
			flatIdx    int  // index in s.flatFiles (-1 for group headers)
			groupFirst int  // first flat-file index in this group (headers only)
			groupLast  int  // last flat-file index in this group (headers only)
		}
		var allRows []fileRow

		fileIdx := 0
		groups := groupFilesByStatus(s.files)
		type groupInfo struct {
			key        string
			statusChar string
		}
		groupOrder := []groupInfo{
			{"modified", "M"},
			{"added", "A"},
			{"deleted", "D"},
			{"renamed", "R"},
			{"untracked", "?"},
			{"other", ""},
		}
		for _, g := range groupOrder {
			paths, ok := groups[g.key]
			if !ok || len(paths) == 0 {
				continue
			}
			groupStart := fileIdx
			icon, color := fileStatusIcon(g.statusChar, t)
			headerText := lipgloss.NewStyle().
				Foreground(color).
				PaddingLeft(1).
				Render(icon + " " + g.key)
			// Header row placeholder — groupLast filled in after iterating paths.
			headerRowIdx := len(allRows)
			allRows = append(allRows, fileRow{text: headerText, isFile: false, flatIdx: -1, groupFirst: groupStart})

			for _, p := range paths {
				name := TruncateMiddle(p, contentW-6)
				style := lipgloss.NewStyle().
					Foreground(t.TextSecondary).
					PaddingLeft(4)

				// Highlight cursor
				if s.focused && fileIdx == s.fileCursor {
					style = style.
						Background(t.SelectionBg).
						Foreground(t.Text).
						Bold(true)
					name = "▸ " + TruncateMiddle(p, contentW-8)
				}

				allRows = append(allRows, fileRow{text: style.Render(name), isFile: true, flatIdx: fileIdx})
				fileIdx++
			}
			// Patch the header with the correct groupLast.
			allRows[headerRowIdx] = fileRow{
				text:       allRows[headerRowIdx].text,
				isFile:     false,
				flatIdx:    -1,
				groupFirst: groupStart,
				groupLast:  fileIdx - 1,
			}
		}

		// ── Viewport clipping ─────────────────────────────────────────────────
		// Map flat-file index → row index so we can find scroll boundaries.
		vp := s.fileListViewport()
		totalFiles := len(s.flatFiles)

		// Determine the range of flat-file indices that are visible.
		visStart := s.scrollOffset
		visEnd := s.scrollOffset + vp - 1
		if visEnd >= totalFiles {
			visEnd = totalFiles - 1
		}

		// Scroll-up indicator
		if visStart > 0 {
			indicator := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(contentW / 2).
				Render("▲")
			lines = append(lines, indicator)
		}

		// Emit rows visible in the current scroll window.
		// Group headers are included only when at least one of their files is visible.
		for _, row := range allRows {
			if row.isFile {
				if row.flatIdx >= visStart && row.flatIdx <= visEnd {
					lines = append(lines, row.text)
				}
			} else {
				// Show this group header only if the group overlaps the visible window.
				if row.groupLast >= visStart && row.groupFirst <= visEnd {
					lines = append(lines, row.text)
				}
			}
		}

		// Scroll-down indicator
		if visEnd < totalFiles-1 {
			indicator := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(contentW / 2).
				Render("▼")
			lines = append(lines, indicator)
		}
	}

	// ── Session section ────────────────────────────────────────────────────────
	lines = append(lines, "")
	sessionLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		PaddingLeft(1).
		Render("SESSION")
	lines = append(lines, sessionLabel)

	if s.sessionID != "" {
		sessDisplay := s.sessionID
		if len(sessDisplay) > contentW-4 {
			sessDisplay = sessDisplay[:contentW-7] + "..."
		}
		sessLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(2).
			Render("⊙ " + sessDisplay)
		lines = append(lines, sessLine)
	} else {
		noSession := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			PaddingLeft(2).
			Render("Start typing to begin a session")
		lines = append(lines, noSession)
	}

	// ── Keyboard shortcuts (bottom, muted) ─────────────────────────────────────
	lines = append(lines, "")
	shortcutDivider := lipgloss.NewStyle().
		Foreground(t.Border).
		Render(strings.Repeat("─", contentW))
	lines = append(lines, shortcutDivider)

	shortcuts := []struct {
		key  string
		desc string
	}{
		{"ctrl+p", "commands"},
		{"ctrl+b", "sidebar"},
		{"ctrl+g", "files"},
		{"ctrl+x", "leader"},
	}
	for _, sc := range shortcuts {
		k := lipgloss.NewStyle().Foreground(t.TextMuted).Render(sc.key)
		d := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Render(" " + sc.desc)
		line := lipgloss.NewStyle().PaddingLeft(1).Render(k + d)
		lines = append(lines, line)
	}

	content := strings.Join(lines, "\n")

	panel := lipgloss.NewStyle().
		Width(contentW).
		Render(content)

	// Build right border: brand accent when focused, muted otherwise
	lineCount := len(lines)
	brandSegment := 3
	borderColor := t.TextMuted
	if s.focused {
		borderColor = t.Brand
	}
	var rightBorderParts []string
	for i := 0; i < lineCount; i++ {
		if i < brandSegment {
			rightBorderParts = append(rightBorderParts,
				lipgloss.NewStyle().Foreground(t.Brand).Render("│"))
		} else {
			rightBorderParts = append(rightBorderParts,
				lipgloss.NewStyle().Foreground(borderColor).Render("│"))
		}
	}
	rightBorderStr := strings.Join(rightBorderParts, "\n")
	rightBorder := lipgloss.NewStyle().Render(rightBorderStr)

	return lipgloss.JoinHorizontal(lipgloss.Top, panel, rightBorder)
}

// groupFilesByStatus groups file paths by their git status category.
func groupFilesByStatus(files []git.FileStatus) map[string][]string {
	groups := make(map[string][]string)
	for _, f := range files {
		group := statusGroup(f.Status)
		groups[group] = append(groups[group], f.Path)
	}
	return groups
}

func statusGroup(status string) string {
	switch {
	case strings.Contains(status, "M"):
		return "modified"
	case strings.Contains(status, "A"):
		return "added"
	case strings.Contains(status, "D"):
		return "deleted"
	case strings.Contains(status, "R"):
		return "renamed"
	case strings.Contains(status, "?"):
		return "untracked"
	default:
		return "other"
	}
}

func fileStatusIcon(status string, t theme.Theme) (string, lipgloss.Color) {
	switch {
	case strings.Contains(status, "M"):
		return "●", t.Warning
	case strings.Contains(status, "A"):
		return "+", t.Success
	case strings.Contains(status, "D"):
		return "−", t.Error
	case strings.Contains(status, "R"):
		return "→", t.TextSecondary
	case strings.Contains(status, "?"):
		return "?", t.TextMuted
	default:
		return "·", t.TextMuted
	}
}

// countFileStatuses returns counts of modified, added, deleted, and untracked files.
func countFileStatuses(files []git.FileStatus) (mod, add, del, untracked int) {
	for _, f := range files {
		switch {
		case strings.Contains(f.Status, "M"):
			mod++
		case strings.Contains(f.Status, "A"):
			add++
		case strings.Contains(f.Status, "D"):
			del++
		case strings.Contains(f.Status, "?"):
			untracked++
		}
	}
	return
}
