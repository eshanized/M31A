package theme

import "github.com/charmbracelet/lipgloss"

// SemanticStyles defines the complete component style library.
//
// Every component in M31A should consume these styles rather than
// creating styles inline. This ensures:
//
//   - Consistent visual language across all components
//   - Zero per-frame allocations for style creation
//   - Single point of control for the design system
//   - Components express intent, not appearance
//
// Components should describe WHAT they are (button, card, status badge)
// and the design system decides HOW they look.
type SemanticStyles struct {
	// ── Typography ──────────────────────────────────────────────────────────
	PageTitle     lipgloss.Style // full-width title text
	SectionTitle  lipgloss.Style // section heading
	Subsection    lipgloss.Style // sub-section heading
	Heading       lipgloss.Style // bold primary text
	Subheading    lipgloss.Style // bold secondary text
	Body          lipgloss.Style // normal body text
	Caption       lipgloss.Style // small muted text
	Muted         lipgloss.Style // very muted text
	Faint         lipgloss.Style // decorative dim text
	Hint          lipgloss.Style // hint/placeholder text
	Metadata      lipgloss.Style // metadata labels
	BrandText     lipgloss.Style // accent-colored text
	BrandBold     lipgloss.Style // bold accent text
	SecondaryText lipgloss.Style // secondary text
	ErrorText     lipgloss.Style // error-colored text
	SuccessText   lipgloss.Style // success-colored text
	WarningText   lipgloss.Style // warning-colored text
	ThinkingText  lipgloss.Style // thinking-colored text
	CodeInline    lipgloss.Style // inline code references
	Link          lipgloss.Style // hyperlink text
	KeyboardHint  lipgloss.Style // keyboard shortcut hint

	// ── Buttons ─────────────────────────────────────────────────────────────
	ButtonPrimary   lipgloss.Style // filled primary action
	ButtonSecondary lipgloss.Style // bordered secondary action
	ButtonGhost     lipgloss.Style // text-only action
	ButtonDanger    lipgloss.Style // destructive action

	// ── Cards ───────────────────────────────────────────────────────────────
	Card         lipgloss.Style // default card
	CardBrand    lipgloss.Style // brand-accented card
	CardSuccess  lipgloss.Style // success-accented card
	CardError    lipgloss.Style // error-accented card
	CardWarning  lipgloss.Style // warning-accented card
	CardElevated lipgloss.Style // elevated card with shadow
	CardHeader   lipgloss.Style // filled header bar

	// ── Panels ──────────────────────────────────────────────────────────────
	Panel         lipgloss.Style // generic panel
	PanelBorder   lipgloss.Style // panel with border
	PanelElevated lipgloss.Style // elevated panel

	// ── Dialogs ─────────────────────────────────────────────────────────────
	Dialog        lipgloss.Style // modal dialog
	DialogDanger  lipgloss.Style // danger confirmation dialog
	DialogWarning lipgloss.Style // warning dialog

	// ── Badges ──────────────────────────────────────────────────────────────
	BadgeBrand   lipgloss.Style // brand badge
	BadgeSuccess lipgloss.Style // success badge
	BadgeError   lipgloss.Style // error badge
	BadgeWarning lipgloss.Style // warning badge
	BadgeInfo    lipgloss.Style // info badge
	BadgeNeutral lipgloss.Style // neutral badge
	BadgeMuted   lipgloss.Style // muted badge
	BadgeRunning lipgloss.Style // running state badge

	// ── Status Indicators ───────────────────────────────────────────────────
	StatusLive    lipgloss.Style // live/active indicator
	StatusSlow    lipgloss.Style // slow/degraded indicator
	StatusOffline lipgloss.Style // offline/error indicator
	StatusBadge   lipgloss.Style // generic status badge

	// ── Input ───────────────────────────────────────────────────────────────
	Input        lipgloss.Style // text input
	InputFocused lipgloss.Style // focused text input
	InputSearch  lipgloss.Style // search input
	InputCursor  lipgloss.Style // cursor in input
	InputLabel   lipgloss.Style // input label
	InputPrompt  lipgloss.Style // input prompt character
	InputCode    lipgloss.Style // code/command input

	// ── Toolbar ─────────────────────────────────────────────────────────────
	Toolbar         lipgloss.Style // toolbar background
	ToolbarItem     lipgloss.Style // toolbar item
	ToolbarActive   lipgloss.Style // active toolbar item
	ToolbarInactive lipgloss.Style // inactive toolbar item

	// ── Sidebar ─────────────────────────────────────────────────────────────
	Sidebar       lipgloss.Style // sidebar container
	SidebarItem   lipgloss.Style // sidebar item
	SidebarActive lipgloss.Style // active sidebar item
	SidebarBorder lipgloss.Style // sidebar border

	// ── Tabs ────────────────────────────────────────────────────────────────
	TabActive   lipgloss.Style // active tab
	TabInactive lipgloss.Style // inactive tab

	// ── Lists ───────────────────────────────────────────────────────────────
	ListItem      lipgloss.Style // list item
	ListSelected  lipgloss.Style // selected list item
	ListCursor    lipgloss.Style // list cursor
	ListEmpty     lipgloss.Style // empty list state
	ListSeparator lipgloss.Style // list separator

	// ── Table ───────────────────────────────────────────────────────────────
	TableHeader lipgloss.Style // table header
	TableRow    lipgloss.Style // table row
	TableCell   lipgloss.Style // table cell

	// ── Progress ────────────────────────────────────────────────────────────
	ProgressFill  lipgloss.Style // progress bar fill
	ProgressEmpty lipgloss.Style // progress bar empty
	ProgressLabel lipgloss.Style // progress percentage label
	ProgressStep  lipgloss.Style // step indicator
	ProgressDone  lipgloss.Style // completed step

	// ── Spinner / Loading ──────────────────────────────────────────────────
	Spinner      lipgloss.Style // spinner character
	SpinnerBrand lipgloss.Style // brand-colored spinner
	SpinnerMuted lipgloss.Style // muted spinner
	Loading      lipgloss.Style // loading indicator text

	// ── Thinking ────────────────────────────────────────────────────────────
	Thinking       lipgloss.Style // thinking content
	ThinkingBorder lipgloss.Style // thinking panel border
	ThinkingHeader lipgloss.Style // thinking panel header
	ThinkingLabel  lipgloss.Style // thinking label text
	ThinkingToggle lipgloss.Style // thinking toggle icon
	ThinkingMuted  lipgloss.Style // thinking muted text

	// ── Streaming ───────────────────────────────────────────────────────────
	StreamingCursor lipgloss.Style // streaming cursor
	StreamingText   lipgloss.Style // streaming text

	// ── Tool Output ─────────────────────────────────────────────────────────
	ToolInput     lipgloss.Style // tool input text
	ToolOutput    lipgloss.Style // tool output text
	ToolLabel     lipgloss.Style // tool label badge
	ToolStatusOK  lipgloss.Style // tool success status
	ToolStatusErr lipgloss.Style // tool error status
	ToolStatusRun lipgloss.Style // tool running status
	ToolMeta      lipgloss.Style // tool metadata (duration, lines)

	// ── Permission ──────────────────────────────────────────────────────────
	PermTitle         lipgloss.Style // permission dialog title
	PermLock          lipgloss.Style // lock icon
	PermKey           lipgloss.Style // key binding hint
	PermHint          lipgloss.Style // key binding description
	PermCountdown     lipgloss.Style // countdown text
	PermCountdownWarn lipgloss.Style // urgent countdown text
	PermCountdownErr  lipgloss.Style // expired countdown text
	PermBarFill       lipgloss.Style // countdown bar fill
	PermBarEmpty      lipgloss.Style // countdown bar empty
	PermRuleMatch     lipgloss.Style // matched rule text
	PermCommand       lipgloss.Style // command text in dialog
	PermCommandOp     lipgloss.Style // command operator (pipe, &&)
	PermCommandArg    lipgloss.Style // command argument
	PermRiskDanger    lipgloss.Style // dangerous risk badge
	PermRiskDestruct  lipgloss.Style // destructive risk badge
	PermRiskMedium    lipgloss.Style // medium risk badge
	PermRiskSafe      lipgloss.Style // safe risk badge

	// ── Toast / Notification ────────────────────────────────────────────────
	Toast                lipgloss.Style // toast background
	ToastSuccess         lipgloss.Style // success toast
	ToastError           lipgloss.Style // error toast
	ToastWarning         lipgloss.Style // warning toast
	ToastInfo            lipgloss.Style // info toast
	NotificationIcon     lipgloss.Style // notification icon
	NotificationText     lipgloss.Style // notification text
	NotificationTime     lipgloss.Style // notification timestamp
	NotificationSelected lipgloss.Style // selected notification

	// ── Error / Warning Banners ─────────────────────────────────────────────
	ErrorBanner   lipgloss.Style // error banner
	WarningBanner lipgloss.Style // warning banner
	InfoBanner    lipgloss.Style // info banner

	// ── Headers / Footers ───────────────────────────────────────────────────
	Header       lipgloss.Style // header bar
	HeaderBrand  lipgloss.Style // brand in header
	HeaderCrumb  lipgloss.Style // breadcrumb text
	HeaderDots   lipgloss.Style // breadcrumb separator dots
	Footer       lipgloss.Style // footer bar
	FooterCwd    lipgloss.Style // working directory
	FooterBranch lipgloss.Style // git branch
	FooterOp     lipgloss.Style // operation status
	FooterHint   lipgloss.Style // footer hint
	FooterLeader lipgloss.Style // leader key indicator

	// ── Separators ──────────────────────────────────────────────────────────
	SeparatorH    lipgloss.Style // horizontal separator
	SeparatorV    lipgloss.Style // vertical separator
	SeparatorLine lipgloss.Style // full-width separator line
	Divider       lipgloss.Style // section divider

	// ── Selection / Cursor ──────────────────────────────────────────────────
	Selection  lipgloss.Style // selected text/item
	Cursor     lipgloss.Style // text cursor
	CursorLine lipgloss.Style // cursor line highlight

	// ── Focus ───────────────────────────────────────────────────────────────
	FocusRing    lipgloss.Style // focus ring
	FocusRingOff lipgloss.Style // unfocused ring

	// ── Disabled ────────────────────────────────────────────────────────────
	Disabled       lipgloss.Style // disabled text
	DisabledButton lipgloss.Style // disabled button

	// ── Empty State ─────────────────────────────────────────────────────────
	EmptyState       lipgloss.Style // empty state container
	EmptyStateIcon   lipgloss.Style // empty state icon
	EmptyStateTitle  lipgloss.Style // empty state title
	EmptyStateHint   lipgloss.Style // empty state hint
	EmptyStateAction lipgloss.Style // empty state action item

	// ── Diff ────────────────────────────────────────────────────────────────
	DiffAdded   lipgloss.Style // added line
	DiffRemoved lipgloss.Style // removed line
	DiffContext lipgloss.Style // context line
	DiffHunk    lipgloss.Style // hunk header

	// ── Code ────────────────────────────────────────────────────────────────
	CodeBlock   lipgloss.Style // code block container
	CodeLine    lipgloss.Style // single code line
	CodeLineNum lipgloss.Style // line number
	CodeKeyword lipgloss.Style // syntax keyword
	CodeString  lipgloss.Style // syntax string
	CodeComment lipgloss.Style // syntax comment
	CodeFunc    lipgloss.Style // syntax function
	CodeType    lipgloss.Style // syntax type

	// ── Markdown ────────────────────────────────────────────────────────────
	MarkdownH1     lipgloss.Style // markdown H1
	MarkdownH2     lipgloss.Style // markdown H2
	MarkdownH3     lipgloss.Style // markdown H3
	MarkdownBold   lipgloss.Style // bold text
	MarkdownItalic lipgloss.Style // italic text
	MarkdownLink   lipgloss.Style // link text
	MarkdownQuote  lipgloss.Style // blockquote
	MarkdownList   lipgloss.Style // list item

	// ── Scroll ──────────────────────────────────────────────────────────────
	Scrollbar      lipgloss.Style // scrollbar thumb
	ScrollbarTrack lipgloss.Style // scrollbar track

	// ── Overlay ─────────────────────────────────────────────────────────────
	DimOverlay lipgloss.Style // dimmed overlay background
	Shadow     lipgloss.Style // shadow effect
}

// BuildSemanticStyles creates a complete SemanticStyles from a Theme.
// Call this once per theme change. All styles are pre-computed.
func BuildSemanticStyles(t Theme) SemanticStyles {
	s := SemanticStyles{}

	// ── Typography ──────────────────────────────────────────────────────────
	s.PageTitle = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).PaddingLeft(PadNormal)
	s.SectionTitle = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true)
	s.Subsection = lipgloss.NewStyle().
		Foreground(t.TextSecondary).Bold(true)
	s.Heading = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true)
	s.Subheading = lipgloss.NewStyle().
		Foreground(t.TextSecondary).Bold(true)
	s.Body = lipgloss.NewStyle().
		Foreground(t.TextPrimary)
	s.Caption = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.Muted = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.Faint = lipgloss.NewStyle().
		Foreground(t.TextMuted).Faint(true)
	s.Hint = lipgloss.NewStyle().
		Foreground(t.TextMuted).Faint(true)
	s.Metadata = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.BrandText = lipgloss.NewStyle().
		Foreground(t.Brand)
	s.BrandBold = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.SecondaryText = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.ErrorText = lipgloss.NewStyle().
		Foreground(t.Error)
	s.SuccessText = lipgloss.NewStyle().
		Foreground(t.Success)
	s.WarningText = lipgloss.NewStyle().
		Foreground(t.Warning)
	s.ThinkingText = lipgloss.NewStyle().
		Foreground(t.Thinking).Italic(true)
	s.CodeInline = lipgloss.NewStyle().
		Foreground(t.Brand)
	s.Link = lipgloss.NewStyle().
		Foreground(t.Brand).Underline(true)
	s.KeyboardHint = lipgloss.NewStyle().
		Foreground(t.TextMuted)

	// ── Buttons ─────────────────────────────────────────────────────────────
	s.ButtonPrimary = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color(t.BadgeTextLight)).
		Padding(0, 2).Bold(true)
	s.ButtonSecondary = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color(t.Brand)).
		Padding(0, 1)
	s.ButtonGhost = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.TextMuted)).
		Padding(0, 1)
	s.ButtonDanger = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Error)).
		Foreground(lipgloss.Color(t.BadgeTextLight)).
		Padding(0, 2).Bold(true)

	// ── Cards ───────────────────────────────────────────────────────────────
	s.Card = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border)).
		Padding(0, 1)
	s.CardBrand = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand)).
		Padding(0, 1)
	s.CardSuccess = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Success)).
		Padding(0, 1)
	s.CardError = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Error)).
		Padding(0, 1)
	s.CardWarning = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Warning)).
		Padding(0, 1)
	s.CardElevated = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border)).
		Padding(0, 1).
		Background(lipgloss.Color(t.SurfaceElevated))
	s.CardHeader = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color(t.BadgeTextLight)).
		Padding(0, 1).Bold(true)

	// ── Panels ──────────────────────────────────────────────────────────────
	s.Panel = lipgloss.NewStyle().
		Padding(PadNormal, PadNormal)
	s.PanelBorder = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border)).
		Padding(0, 1)
	s.PanelElevated = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border)).
		Padding(0, 1).
		Background(lipgloss.Color(t.SurfaceElevated))

	// ── Dialogs ─────────────────────────────────────────────────────────────
	s.Dialog = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(PadNormal, PadNormal).
		Border(DoubleBorder).
		BorderForeground(lipgloss.Color(t.Brand))
	s.DialogDanger = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(PadNormal, PadNormal).
		Border(NormalBorder).
		BorderForeground(lipgloss.Color(t.Error))
	s.DialogWarning = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(PadNormal, PadNormal).
		Border(NormalBorder).
		BorderForeground(lipgloss.Color(t.Warning))

	// ── Badges ──────────────────────────────────────────────────────────────
	s.BadgeBrand = lipgloss.NewStyle().
		Foreground(t.Brand).Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).Padding(0, 1)
	s.BadgeSuccess = lipgloss.NewStyle().
		Foreground(t.Success).Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Success).Padding(0, 1)
	s.BadgeError = lipgloss.NewStyle().
		Foreground(t.Error).Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Error).Padding(0, 1)
	s.BadgeWarning = lipgloss.NewStyle().
		Foreground(t.Warning).Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Warning).Padding(0, 1)
	s.BadgeInfo = lipgloss.NewStyle().
		Foreground(t.Info).Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Info).Padding(0, 1)
	s.BadgeNeutral = lipgloss.NewStyle().
		Foreground(t.TextSecondary).Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).Padding(0, 1)
	s.BadgeMuted = lipgloss.NewStyle().
		Background(t.Border).Foreground(t.TextSecondary).
		Padding(0, 1).Bold(true)
	s.BadgeRunning = lipgloss.NewStyle().
		Background(t.Warning).Foreground(t.BadgeForeground).
		Padding(0, 1).Bold(true)

	// ── Status Indicators ───────────────────────────────────────────────────
	s.StatusLive = lipgloss.NewStyle().
		Foreground(t.Success).Bold(true)
	s.StatusSlow = lipgloss.NewStyle().
		Foreground(t.Warning).Bold(true)
	s.StatusOffline = lipgloss.NewStyle().
		Foreground(t.Error).Bold(true)
	s.StatusBadge = lipgloss.NewStyle().
		Padding(0, 1).Bold(true)

	// ── Input ───────────────────────────────────────────────────────────────
	s.Input = lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border)).
		Padding(0, 1)
	s.InputFocused = lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand)).
		Padding(0, 1)
	s.InputSearch = lipgloss.NewStyle().
		Foreground(t.TextPrimary)
	s.InputCursor = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.InputLabel = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.InputPrompt = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.InputCode = lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Border(DoubleBorder).
		BorderForeground(t.Border).
		Padding(0, 1)

	// ── Toolbar ─────────────────────────────────────────────────────────────
	s.Toolbar = lipgloss.NewStyle().
		Padding(0, 1)
	s.ToolbarItem = lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Padding(0, 1)
	s.ToolbarActive = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true).
		Padding(0, 1)
	s.ToolbarInactive = lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Padding(0, 1)

	// ── Sidebar ─────────────────────────────────────────────────────────────
	s.Sidebar = lipgloss.NewStyle().
		Padding(PadTight, PadTight)
	s.SidebarItem = lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Padding(0, 1)
	s.SidebarActive = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).
		Background(t.SelectionBg).
		Padding(0, 1)
	s.SidebarBorder = lipgloss.NewStyle().
		Foreground(t.Border)

	// ── Tabs ────────────────────────────────────────────────────────────────
	s.TabActive = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1)
	s.TabInactive = lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Padding(0, 1)

	// ── Lists ───────────────────────────────────────────────────────────────
	s.ListItem = lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Padding(0, 1)
	s.ListSelected = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).
		Background(t.SelectionBg).
		Padding(0, 1)
	s.ListCursor = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.ListEmpty = lipgloss.NewStyle().
		Foreground(t.TextMuted).Faint(true)
	s.ListSeparator = lipgloss.NewStyle().
		Foreground(t.Border)

	// ── Table ───────────────────────────────────────────────────────────────
	s.TableHeader = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).
		Border(NormalBorder, false, false, true, false).
		BorderForeground(t.Border)
	s.TableRow = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.TableCell = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Padding(0, 1)

	// ── Progress ────────────────────────────────────────────────────────────
	s.ProgressFill = lipgloss.NewStyle().
		Foreground(t.Brand)
	s.ProgressEmpty = lipgloss.NewStyle().
		Foreground(t.Border)
	s.ProgressLabel = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.ProgressStep = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.ProgressDone = lipgloss.NewStyle().
		Foreground(t.Success)

	// ── Spinner / Loading ──────────────────────────────────────────────────
	s.Spinner = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.SpinnerBrand = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.SpinnerMuted = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.Loading = lipgloss.NewStyle().
		Foreground(t.TextMuted)

	// ── Thinking ────────────────────────────────────────────────────────────
	s.Thinking = lipgloss.NewStyle().
		Foreground(t.Thinking).Italic(true)
	s.ThinkingBorder = lipgloss.NewStyle().
		Foreground(t.Thinking)
	s.ThinkingHeader = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.ThinkingLabel = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.ThinkingToggle = lipgloss.NewStyle().
		Foreground(t.Thinking)
	s.ThinkingMuted = lipgloss.NewStyle().
		Foreground(t.Thinking).Faint(true)

	// ── Streaming ───────────────────────────────────────────────────────────
	s.StreamingCursor = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.StreamingText = lipgloss.NewStyle().
		Foreground(t.TextPrimary)

	// ── Tool Output ─────────────────────────────────────────────────────────
	s.ToolInput = lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Padding(0, 1)
	s.ToolOutput = lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Padding(0, 1)
	s.ToolLabel = lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Background(t.SurfaceElevated).
		Padding(0, 1).Bold(true)
	s.ToolStatusOK = lipgloss.NewStyle().
		Foreground(t.Success)
	s.ToolStatusErr = lipgloss.NewStyle().
		Foreground(t.Error)
	s.ToolStatusRun = lipgloss.NewStyle().
		Foreground(t.Warning)
	s.ToolMeta = lipgloss.NewStyle().
		Foreground(t.TextMuted)

	// ── Permission ──────────────────────────────────────────────────────────
	s.PermTitle = lipgloss.NewStyle().
		Foreground(t.Warning).Bold(true)
	s.PermLock = lipgloss.NewStyle().
		Foreground(t.Warning).Bold(true)
	s.PermKey = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.PermHint = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.PermCountdown = lipgloss.NewStyle().
		Foreground(t.Warning)
	s.PermCountdownWarn = lipgloss.NewStyle().
		Foreground(t.Error)
	s.PermCountdownErr = lipgloss.NewStyle().
		Foreground(t.Error).Bold(true)
	s.PermBarFill = lipgloss.NewStyle().
		Foreground(t.Warning)
	s.PermBarEmpty = lipgloss.NewStyle().
		Foreground(t.Border)
	s.PermRuleMatch = lipgloss.NewStyle().
		Faint(true)
	s.PermCommand = lipgloss.NewStyle().
		Foreground(t.TextPrimary)
	s.PermCommandOp = lipgloss.NewStyle().
		Foreground(t.Warning)
	s.PermCommandArg = lipgloss.NewStyle().
		Foreground(t.TextPrimary)
	s.PermRiskDanger = lipgloss.NewStyle().
		Background(t.Warning).Foreground(t.BadgeForeground).
		Bold(true).Padding(0, 1)
	s.PermRiskDestruct = lipgloss.NewStyle().
		Background(t.Error).Foreground(t.BadgeForeground).
		Bold(true).Padding(0, 1)
	s.PermRiskMedium = lipgloss.NewStyle().
		Background(t.Warning).Foreground(t.BadgeForeground).
		Padding(0, 1)
	s.PermRiskSafe = lipgloss.NewStyle().
		Background(t.TextSecondary).Foreground(t.BadgeForeground).
		Padding(0, 1)

	// ── Toast / Notification ────────────────────────────────────────────────
	s.Toast = lipgloss.NewStyle().
		Padding(0, 1)
	s.ToastSuccess = lipgloss.NewStyle().
		Foreground(t.Success).Padding(0, 1)
	s.ToastError = lipgloss.NewStyle().
		Foreground(t.Error).Padding(0, 1)
	s.ToastWarning = lipgloss.NewStyle().
		Foreground(t.Warning).Padding(0, 1)
	s.ToastInfo = lipgloss.NewStyle().
		Foreground(t.Info).Padding(0, 1)
	s.NotificationIcon = lipgloss.NewStyle().
		Bold(true)
	s.NotificationText = lipgloss.NewStyle().
		Foreground(t.TextPrimary)
	s.NotificationTime = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.NotificationSelected = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true)

	// ── Error / Warning Banners ─────────────────────────────────────────────
	s.ErrorBanner = lipgloss.NewStyle().
		Foreground(t.Error).PaddingLeft(PadNormal)
	s.WarningBanner = lipgloss.NewStyle().
		Foreground(t.Warning).PaddingLeft(PadNormal)
	s.InfoBanner = lipgloss.NewStyle().
		Foreground(t.Info).PaddingLeft(PadNormal)

	// ── Headers / Footers ───────────────────────────────────────────────────
	s.Header = lipgloss.NewStyle().
		Padding(0, PadTight)
	s.HeaderBrand = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.HeaderCrumb = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.HeaderDots = lipgloss.NewStyle().
		Foreground(t.BorderSubtle)
	s.Footer = lipgloss.NewStyle().
		Padding(0, PadTight)
	s.FooterCwd = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.FooterBranch = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.FooterOp = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.FooterHint = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.FooterLeader = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)

	// ── Separators ──────────────────────────────────────────────────────────
	s.SeparatorH = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Border))
	s.SeparatorV = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Border))
	s.SeparatorLine = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.BorderSubtle))
	s.Divider = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.BorderSubtle))

	// ── Selection / Cursor ──────────────────────────────────────────────────
	s.Selection = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SelectionBg))
	s.Cursor = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Brand)).Bold(true)
	s.CursorLine = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SelectionBg))

	// ── Focus ───────────────────────────────────────────────────────────────
	s.FocusRing = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand))
	s.FocusRingOff = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.BorderSubtle))

	// ── Disabled ────────────────────────────────────────────────────────────
	s.Disabled = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.TextMuted)).Faint(true)
	s.DisabledButton = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Border)).
		Foreground(lipgloss.Color(t.TextMuted)).
		Padding(0, 2)

	// ── Empty State ─────────────────────────────────────────────────────────
	s.EmptyState = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.EmptyStateIcon = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.EmptyStateTitle = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true)
	s.EmptyStateHint = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.EmptyStateAction = lipgloss.NewStyle().
		Foreground(t.TextPrimary)

	// ── Diff ────────────────────────────────────────────────────────────────
	s.DiffAdded = lipgloss.NewStyle().
		Foreground(t.DiffAdded)
	s.DiffRemoved = lipgloss.NewStyle().
		Foreground(t.DiffRemoved)
	s.DiffContext = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.DiffHunk = lipgloss.NewStyle().
		Foreground(t.TextMuted).Faint(true)

	// ── Code ────────────────────────────────────────────────────────────────
	s.CodeBlock = lipgloss.NewStyle().
		Background(lipgloss.Color(t.CodeBG)).
		Padding(0, 1)
	s.CodeLine = lipgloss.NewStyle().
		Foreground(t.TextSecondary)
	s.CodeLineNum = lipgloss.NewStyle().
		Foreground(t.TextMuted).Faint(true)
	s.CodeKeyword = lipgloss.NewStyle().
		Foreground(t.Brand)
	s.CodeString = lipgloss.NewStyle().
		Foreground(t.Success)
	s.CodeComment = lipgloss.NewStyle().
		Foreground(t.TextMuted).Faint(true).Italic(true)
	s.CodeFunc = lipgloss.NewStyle().
		Foreground(t.Warning)
	s.CodeType = lipgloss.NewStyle().
		Foreground(t.Info)

	// ── Markdown ────────────────────────────────────────────────────────────
	s.MarkdownH1 = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.MarkdownH2 = lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true)
	s.MarkdownH3 = lipgloss.NewStyle().
		Foreground(t.TextSecondary).Bold(true)
	s.MarkdownBold = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true)
	s.MarkdownItalic = lipgloss.NewStyle().
		Foreground(t.TextPrimary).Italic(true)
	s.MarkdownLink = lipgloss.NewStyle().
		Foreground(t.Brand).Underline(true)
	s.MarkdownQuote = lipgloss.NewStyle().
		Foreground(t.TextMuted)
	s.MarkdownList = lipgloss.NewStyle().
		Foreground(t.TextPrimary)

	// ── Scroll ──────────────────────────────────────────────────────────────
	s.Scrollbar = lipgloss.NewStyle().
		Foreground(t.Border)
	s.ScrollbarTrack = lipgloss.NewStyle().
		Foreground(t.BorderSubtle)

	// ── Overlay ─────────────────────────────────────────────────────────────
	s.DimOverlay = lipgloss.NewStyle().Faint(true)
	s.Shadow = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.ShadowColor))

	return s
}
